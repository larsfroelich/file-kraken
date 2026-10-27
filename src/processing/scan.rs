use crate::state::file::FileKrakenFileType;
use crate::state::location::FileKrakenLocationState;
use crate::state::AppState;
use crate::utils::dialogs::error_dialog;
use jwalk::WalkDir;
use log::error;
use std::path::Path;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

/// Checks if the given path points to a known archive file type.
fn is_archive_path(path: &Path) -> bool {
    // Check extensions first for common types
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        let ext = ext.to_lowercase();
        if ext == "zip" || ext == "7z" {
            return true;
        }
    }
    // Check filename for double extensions like .tar.xz
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_lowercase().ends_with(".tar.xz"))
        .unwrap_or(false)
}

use crate::state::file::FileKrakenFile;
use crate::utils::parent_path::is_ancestor_of;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use zip::ZipArchive;

/// Scans a directory for files and updates the application state.
///
/// This function is optimized for network drives by using bulk updates
/// and in-memory deletion tracking to minimize syscalls and database roundtrips.
pub fn scan_location_files(app_state: Arc<AppState>, location_path: &str) {
    let current_location = match app_state.get_location_clone(location_path) {
        Some(loc) => loc,
        None => {
            error_dialog(&format!("Location {location_path} not found"));
            return;
        }
    };
    if current_location.location_state == FileKrakenLocationState::Scanning {
        return error_dialog("Already scanning this location");
    }
    app_state.modify_location_state(true, location_path, FileKrakenLocationState::Scanning);

    // --- SETUP: Relevant locations and deletion tracking ---

    // identify descendant locations that might overlap
    let mut relevant_locations: Vec<_> = {
        // NOTE: we scope the read lock to this block to avoid deadlocks.
        // Subsequent calls (like add_files_to_location) may need a write lock
        // on the same locations list.
        let all_locations = app_state.get_locations_list_readonly();
        all_locations
            .iter()
            .filter(|l| l.path == location_path || is_ancestor_of(location_path, &l.path))
            .cloned()
            .collect()
    };
    // sort by path length descending so the longest match is found first
    relevant_locations.sort_by_key(|l| std::cmp::Reverse(l.path.len()));

    // pre-load all existing files across all relevant locations to track deletions globally
    let mut all_existing_files: HashSet<String> = HashSet::new();
    for loc in &relevant_locations {
        let files = app_state.with_sqlite_conn(|conn| {
            let mut stmt = conn.prepare("SELECT path FROM files WHERE location_path = ?")?;
            let rows = stmt.query_map([&loc.path], |row| row.get::<_, String>(0))?;
            Ok(rows.flatten().collect::<Vec<_>>())
        });
        if let Some(files) = files {
            all_existing_files.extend(files);
        }
    }

    // --- PROCESSING: Walk directory and discover files ---

    let mut discovered_files_by_location: HashMap<String, Vec<FileKrakenFile>> = HashMap::new();
    let mut failed_paths = Vec::new();

    const BATCH_SIZE: usize = 1000;

    for entry in WalkDir::new(location_path)
        .skip_hidden(false)
        .into_iter()
        .flatten()
    {
        if entry.file_type.is_file() {
            let path = entry.path();
            let file_path = match path.to_str() {
                Some(p) => p.to_string(),
                None => {
                    failed_paths.push(path.to_string_lossy().to_string());
                    continue;
                }
            };

            // find the correct location for this file (the longest matching path)
            let target_location = relevant_locations
                .iter()
                .find(|l| is_ancestor_of(&l.path, &file_path))
                .map(|l| &l.path)
                .unwrap_or(&current_location.path);

            let file_type = if is_archive_path(&path) {
                FileKrakenFileType::Archive
            } else {
                FileKrakenFileType::Normal
            };

            let file_metadata = match entry.metadata() {
                Ok(meta) => meta,
                Err(err) => {
                    error!("Failed to get file metadata for file {:?}: {}", path, err);
                    continue;
                }
            };

            let file = FileKrakenFile {
                path: file_path.clone(),
                file_type: file_type.clone(),
                file_len: file_metadata.len(),
                time_created: file_metadata
                    .created()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .unwrap_or_default()
                    .as_secs(),
                time_modified: file_metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .unwrap_or_default()
                    .as_secs(),
                hash: None,
                archive_path: None,
                archive_size: None,
                archive_mtime: None,
            };

            // mark file as found in the global set
            all_existing_files.remove(&file_path);

            let mut files_to_add = vec![file.clone()];

            if file_type == FileKrakenFileType::Archive
                && file_path.to_lowercase().ends_with(".zip")
            {
                if let Ok(fs_file) = File::open(&path) {
                    if let Ok(mut archive) = ZipArchive::new(fs_file) {
                        for i in 0..archive.len() {
                            if let Ok(member) = archive.by_index(i) {
                                if member.is_file() {
                                    // Properly decode UTF-8 to avoid breaking hashing of non-ascii filenames
                                    let member_name =
                                        String::from_utf8_lossy(member.name_raw()).into_owned();

                                    let member_path =
                                        format!("{}::{}::{}", file_path, i, member_name);
                                    let member_len = member.size();

                                    let time_modified = file.time_modified;

                                    let member_file = FileKrakenFile {
                                        path: member_path.clone(),
                                        file_type: FileKrakenFileType::ArchiveMember,
                                        file_len: member_len,
                                        time_created: file.time_created,
                                        time_modified,
                                        hash: None,
                                        archive_path: Some(file_path.clone()),
                                        archive_size: Some(file.file_len),
                                        archive_mtime: Some(file.time_modified),
                                    };

                                    all_existing_files.remove(&member_path);
                                    files_to_add.push(member_file);
                                }
                            }
                        }
                    }
                }
            }

            let batch = if let Some(b) = discovered_files_by_location.get_mut(target_location) {
                b
            } else {
                discovered_files_by_location
                    .entry(target_location.clone())
                    .or_default()
            };
            for f in files_to_add {
                batch.push(f);
            }

            // periodically flush batches to database and memory
            if batch.len() >= BATCH_SIZE {
                let to_add = std::mem::take(batch);
                app_state.add_files_to_location(true, target_location, to_add);
            }
        }
    }

    // --- CLEANUP: Finalize state ---

    // flush remaining batches
    for (loc_path, batch) in discovered_files_by_location {
        if !batch.is_empty() {
            app_state.add_files_to_location(true, &loc_path, batch);
        }
    }

    // identify and remove deleted files (those that were in DB but not found during scan)
    if !all_existing_files.is_empty() {
        let paths: Vec<_> = all_existing_files.into_iter().collect();
        app_state.remove_files(true, &paths);
    }

    if !failed_paths.is_empty() {
        let msg = if failed_paths.len() > 10 {
            format!(
                "Failed to get file path for {} files. Example: {}",
                failed_paths.len(),
                failed_paths[0]
            )
        } else {
            format!(
                "Failed to get file path for files: {}",
                failed_paths.join(", ")
            )
        };
        error_dialog(&msg);
    }

    app_state.modify_location_state(true, location_path, FileKrakenLocationState::Scanned);
}

#[test]
fn test_is_archive_path_basic() {
    assert!(is_archive_path(Path::new("/tmp/test.tar.xz")));
    assert!(is_archive_path(Path::new("C:/files/archive.zip")));
    assert!(is_archive_path(Path::new("foo/bar.7z")));
    assert!(!is_archive_path(Path::new("/tmp/image.png")));
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_scan_zip_members() {
        use std::io::Write;
        use zip::write::FileOptions;
        use zip::ZipWriter;

        let temp_dir = tempdir().unwrap();
        let root_path = temp_dir.path().to_str().unwrap().to_string();
        let archive_path = temp_dir.path().join("test.zip");

        // create archive with 2 members
        {
            let file = std::fs::File::create(&archive_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options: FileOptions<()> = FileOptions::default();
            zip.start_file("member1.txt", options.clone()).unwrap();
            zip.write_all(b"hello").unwrap();
            zip.start_file("member2.txt", options).unwrap();
            zip.write_all(b"world").unwrap();
            zip.finish().unwrap();
        }

        let db_dir = tempdir().unwrap();
        let sqlite_path = db_dir.path().join("test.fkproj");
        let app_state = Arc::new(AppState::default());
        app_state
            .connect_sqlite(sqlite_path.to_str().unwrap())
            .unwrap();

        app_state.add_location(
            true,
            &root_path,
            &FileKrakenLocationType::Normal,
            &FileKrakenLocationState::Unscanned,
        );

        scan_location_files(app_state.clone(), &root_path);

        let files = app_state.get_files_by_location(&root_path).unwrap();
        let files_lock = files.read().unwrap();

        assert!(files_lock.contains_key(archive_path.to_str().unwrap()));

        let m1_path = format!("{}::0::member1.txt", archive_path.to_str().unwrap());
        let m2_path = format!("{}::1::member2.txt", archive_path.to_str().unwrap());

        assert!(files_lock.contains_key(&m1_path), "Missing member 1");
        assert!(files_lock.contains_key(&m2_path), "Missing member 2");

        let m1 = files_lock.get(&m1_path).unwrap();
        assert_eq!(
            m1.file_type,
            crate::state::file::FileKrakenFileType::ArchiveMember
        );
        assert_eq!(m1.file_len, 5);
        assert_eq!(
            m1.archive_path.as_ref().unwrap(),
            archive_path.to_str().unwrap()
        );
    }
    use super::*;
    use crate::state::location::FileKrakenLocationType;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_scan_overlapping_locations() {
        let temp_dir = tempdir().unwrap();
        let root_path = temp_dir.path().to_str().unwrap().to_string();
        let photos_path = temp_dir.path().join("photos").to_str().unwrap().to_string();
        fs::create_dir(&photos_path).unwrap();

        let root_file = temp_dir.path().join("root.txt");
        let photo_file = temp_dir.path().join("photos").join("photo.jpg");
        fs::write(&root_file, "root").unwrap();
        fs::write(&photo_file, "photo").unwrap();

        let db_dir = tempdir().unwrap();
        let sqlite_path = db_dir.path().join("test.fkproj");
        let app_state = Arc::new(AppState::default());
        app_state
            .connect_sqlite(sqlite_path.to_str().unwrap())
            .unwrap();

        // /root is Preferred, /root/photos is Excluded
        app_state.add_location(
            true,
            &root_path,
            &FileKrakenLocationType::Preferred,
            &FileKrakenLocationState::Unscanned,
        );
        app_state.add_location(
            true,
            &photos_path,
            &FileKrakenLocationType::Excluded,
            &FileKrakenLocationState::Unscanned,
        );

        // Scan the root
        scan_location_files(app_state.clone(), &root_path);

        let root_files = app_state.get_files_by_location(&root_path).unwrap();
        let photo_loc_files = app_state.get_files_by_location(&photos_path).unwrap();

        assert!(root_files
            .read()
            .unwrap()
            .contains_key(root_file.to_str().unwrap()));
        assert!(!root_files
            .read()
            .unwrap()
            .contains_key(photo_file.to_str().unwrap()));

        assert!(photo_loc_files
            .read()
            .unwrap()
            .contains_key(photo_file.to_str().unwrap()));
    }

    #[test]
    fn test_scan_updates_file_size() {
        let temp_dir = tempdir().unwrap();
        let root_path = temp_dir.path().to_str().unwrap().to_string();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "initial").unwrap(); // size 7

        let db_dir = tempdir().unwrap();
        let sqlite_path = db_dir.path().join("test.fkproj");
        let app_state = Arc::new(AppState::default());
        app_state
            .connect_sqlite(sqlite_path.to_str().unwrap())
            .unwrap();

        app_state.add_location(
            true,
            &root_path,
            &FileKrakenLocationType::Normal,
            &FileKrakenLocationState::Unscanned,
        );

        // First scan
        scan_location_files(app_state.clone(), &root_path);
        let files = app_state.get_files_by_location(&root_path).unwrap();
        assert_eq!(
            files
                .read()
                .unwrap()
                .get(test_file.to_str().unwrap())
                .unwrap()
                .file_len,
            7
        );

        // Update file size
        fs::write(&test_file, "updated size").unwrap(); // size 12

        // Second scan
        scan_location_files(app_state.clone(), &root_path);
        let files = app_state.get_files_by_location(&root_path).unwrap();
        assert_eq!(
            files
                .read()
                .unwrap()
                .get(test_file.to_str().unwrap())
                .unwrap()
                .file_len,
            12
        );
    }

    #[test]
    fn test_scan_removes_deleted_files() {
        let temp_dir = tempdir().unwrap();
        let root_path = temp_dir.path().to_str().unwrap().to_string();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "content").unwrap();

        let db_dir = tempdir().unwrap();
        let sqlite_path = db_dir.path().join("test.fkproj");
        let app_state = Arc::new(AppState::default());
        app_state
            .connect_sqlite(sqlite_path.to_str().unwrap())
            .unwrap();

        app_state.add_location(
            true,
            &root_path,
            &FileKrakenLocationType::Normal,
            &FileKrakenLocationState::Unscanned,
        );

        // First scan
        scan_location_files(app_state.clone(), &root_path);
        assert_eq!(app_state.get_total_files_count(), 1);

        // Delete file
        fs::remove_file(&test_file).unwrap();

        // Second scan
        scan_location_files(app_state.clone(), &root_path);
        assert_eq!(app_state.get_total_files_count(), 0);
    }
}
