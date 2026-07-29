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
    if !relevant_locations.is_empty() {
        let paths: Vec<&str> = relevant_locations.iter().map(|l| l.path.as_str()).collect();
        for chunk in paths.chunks(900) {
            let files = app_state.with_sqlite_conn(|conn| {
                let mut stmt = conn.prepare(&format!(
                    "SELECT path FROM files WHERE location_path IN ({})",
                    vec!["?"; chunk.len()].join(",")
                ))?;
                let rows = stmt.query_map(rusqlite::params_from_iter(chunk.iter().copied()), |row| {
                    row.get::<_, String>(0)
                })?;
                Ok(rows.flatten().collect::<Vec<_>>())
            });

            if let Some(files) = files {
                all_existing_files.extend(files);
            }
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
                file_type,
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
            };

            // mark file as found in the global set
            all_existing_files.remove(&file_path);

            let batch = discovered_files_by_location
                .entry(target_location.clone())
                .or_default();
            batch.push(file);

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
