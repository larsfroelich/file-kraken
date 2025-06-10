use crate::state::duplicate::{FileKrakenDuplicate, FileKrakenDuplicateType};
use crate::state::file::{FileKrakenFile, FileKrakenFileType};
use crate::state::location::{FileKrakenLocation, FileKrakenLocationType};
use crate::state::AppState;
use crate::utils::get_longest_parent_path;
use crate::utils::locks::{lock_rw_read_or_exit, lock_rw_write_or_exit};
use egui::ahash::HashMap;
use std::cmp::max;
use std::ops::DerefMut;
use std::sync::{Arc, RwLock, RwLockWriteGuard};

type FilesByHash = HashMap<String, Vec<FileKrakenFile>>;
type FilesBySizeByHash = HashMap<u64, FilesByHash>;

#[derive(Default)]
pub struct FindDuplicatesState {
    pub duplicates: RwLock<Vec<FileKrakenDuplicate>>,
    pub state: RwLock<FindDuplicatesStateType>,
}

#[derive(Default, PartialEq)]
pub enum FindDuplicatesStateType {
    #[default]
    None,
    Processing(String),
    Processed,
}

pub fn find_file_duplicates(app_state: Arc<AppState>) {
    if run_find_file_duplicates(app_state.clone()).is_none() {
        rfd::MessageDialog::new()
            .set_title("Failed to find duplicates")
            .set_description("Failed to find duplicates")
            .show();
        clear_previous_duplicates(&app_state);
        *get_duplicates_processing_state(&app_state).deref_mut() = FindDuplicatesStateType::None;
    }
}

pub fn run_find_file_duplicates(app_state: Arc<AppState>) -> Option<()> {
    if let FindDuplicatesStateType::Processing(_) =
        get_duplicates_processing_state(&app_state).deref_mut()
    {
        rfd::MessageDialog::new()
            .set_title("Already processing")
            .set_description("Already processing duplicates")
            .show();
        return Some(());
    }
    app_state
        .find_duplicates_processing
        .duplicates
        .write()
        .ok()?
        .clear();

    set_processing_message(&app_state, "Scanning for file size matches...".to_string());
    let mut duplicate_file_sizes = find_duplicate_file_sizes(&app_state.sqlite)?;

    let files_by_size_by_hash = Arc::new(RwLock::new(HashMap::default()));
    let mut duplicates_search_by_filesize_threads = vec![];
    let nr_total_sizes_to_check = duplicate_file_sizes.len();
    let chunk_size = nr_total_sizes_to_check / 16;
    let sizes_checked_so_far = Arc::new(RwLock::new(0f64));
    let mut threads = vec![];

    for duplicate_file_size_chunk in duplicate_file_sizes
        .chunks(max(1, chunk_size))
        .map(|x| x.to_vec())
    {
        let app_state = app_state.clone();
        let files_by_size_by_hash = files_by_size_by_hash.clone();
        let nr_total_sizes_to_check = nr_total_sizes_to_check as f64;
        let sizes_checked_so_far = sizes_checked_so_far.clone();
        threads.push(std::thread::spawn(move || {
            for duplicate_file_size in duplicate_file_size_chunk {
                let Some(mut files_by_size) = get_files_by_size(&app_state, duplicate_file_size)
                else {
                    continue;
                };
                let nr_files_by_size = files_by_size.len();
                set_processing_message(
                    &app_state,
                    format!(
                        "{:.2}% | Calculating hashes for {} files of size {}",
                        sizes_checked_so_far.read().unwrap().clone() * 100.0
                            / nr_total_sizes_to_check,
                        nr_files_by_size,
                        duplicate_file_size
                    ),
                );
                let mut inner_threads = vec![];
                for files in files_by_size
                    .chunks(max(1, nr_files_by_size / 4))
                    .map(|x| x.to_vec())
                {
                    let files_by_size_by_hash = files_by_size_by_hash
                        .write()
                        .unwrap()
                        .entry(duplicate_file_size)
                        .or_insert_with(|| Arc::new(RwLock::new(HashMap::default())))
                        .clone();
                    let _app_state = app_state.clone();
                    inner_threads.push(std::thread::spawn(move || {
                        for mut file in files {
                            if let Some(hash) = _app_state.calculate_file_hash(&file.path) {
                                file.hash = Some(hash.clone());
                                files_by_size_by_hash
                                    .write()
                                    .unwrap()
                                    .entry(hash)
                                    .or_insert(vec![])
                                    .push(file.clone());
                            }
                        }
                    }));
                }
                for thread in threads {
                    if let Err(err) = thread.join() {
                        log::error!("Hashing thread panicked: {:?}", err);
                    }
                }
                *sizes_checked_so_far.write().unwrap() += 1.0;
            }
        }));
    }

    for thread in threads {
        thread.join().ok()?;
    }

    let files_by_size_by_hash = Arc::try_unwrap(files_by_size_by_hash)
        .ok()?
        .into_inner()
        .ok()?;
    let mut result = HashMap::default();
    for (size, hashes_arc) in files_by_size_by_hash {
        let hashes = Arc::try_unwrap(hashes_arc).ok()?.into_inner().ok()?;
        result.insert(size, hashes);
    }
    Some(result)
}

/// From the hashed buckets, populate the duplicate list in the application state.
fn detect_duplicates(
    app_state: &Arc<AppState>,
    files_by_size_by_hash: &FilesBySizeByHash,
) -> Option<()> {
    set_processing_message(
        app_state,
        "Checking file-hashes for duplicates...".to_string(),
    );
    for files_by_size in files_by_size_by_hash.values() {
        for files in files_by_size.values() {
            if files.len() > 1 {
                let mut duplicates_list = match lock_rw_write_or_exit(
                    &app_state.find_duplicates_processing.duplicates,
                    &app_state,
                ) {
                    Some(lock) => lock,
                    None => return None,
                };

                let deletable_file = get_deletable_file(app_state, files);
                let other_files = if let Some(ref deletable_file) = deletable_file {
                    files
                        .iter()
                        .filter(|x| x.path != deletable_file.path)
                        .cloned()
                        .collect()
                } else {
                    files.clone()
                };
                let duplicate = FileKrakenDuplicate {
                    other_files,
                    deletable_file,
                    duplicate_type: FileKrakenDuplicateType::ExactMatch,
                };
                log::trace!(
                    "found duplicate type {:?} size {}",
                    duplicate.duplicate_type,
                    duplicate
                        .deletable_file
                        .as_ref()
                        .or_else(|| duplicate.other_files.get(0))
                        .map(|f| f.file_len)
                        .unwrap_or(0)
                );
                duplicates_list.push(duplicate);
            }
        }
    }
    Some(())
}

fn get_deletable_file(
    app_state: &Arc<AppState>,
    files: &Vec<FileKrakenFile>,
) -> Option<FileKrakenFile> {
    let (preferred_file, normal_file) = {
        let file_locations: Vec<(FileKrakenFile, Option<FileKrakenLocation>)> = {
            let locations = app_state.get_locations_list_readonly();
            files
                .iter()
                .map(|file| {
                    (
                        file.clone(),
                        get_longest_parent_path(&file.path, locations.iter())
                            .and_then(|p| locations.iter().find(|loc| loc.path == p).cloned()),
                    )
                })
                .collect()
        };

        (
            file_locations
                .iter()
                .filter(|(_, location)| location.is_some())
                .find(|(_, location)| {
                    location
                        .as_ref()
                        .is_some_and(|loc| loc.location_type == FileKrakenLocationType::Preferred)
                })
                .map(|(file, _)| file.clone()),
            file_locations
                .iter()
                .filter(|(_, location)| location.is_some())
                .find(|(_, location)| {
                    location
                        .as_ref()
                        .is_some_and(|loc| loc.location_type == FileKrakenLocationType::Normal)
                })
                .map(|(file, _)| file.clone()),
        )
    };

    if preferred_file.is_some() && normal_file.is_some() {
        normal_file
    } else {
        None
    }
}

fn get_files_by_size(app_state: &Arc<AppState>, size: u64) -> Option<Vec<FileKrakenFile>> {
    app_state.with_sqlite_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT \
        path, file_type, file_len, time_created, time_modified, hash_256 \
        FROM files \
        WHERE file_len = ?1",
        )?;
        let mut select_files = stmt.query([size])?;
        let mut files = vec![];
        while let Some(row) = select_files.next()? {
            let file_path: String = row.get(0)?;
            let file_type = match row.get::<usize, String>(1)?.as_str() {
                "normal" => FileKrakenFileType::Normal,
                "archive" => FileKrakenFileType::Archive,
                x => {
                    panic!("unknown file type {}", x)
                }
            };
            files.push(FileKrakenFile {
                path: file_path,
                file_type,
                file_len: row.get(2)?,
                time_created: row.get(3)?,
                time_modified: row.get(4)?,
                hash: row.get(5)?,
            });
        }
        Ok(files)
    })
}

pub fn get_duplicates_processing_state(
    app_state: &Arc<AppState>,
) -> RwLockWriteGuard<'_, FindDuplicatesStateType> {
    app_state.find_duplicates_processing.state.write().unwrap()
}

pub fn set_processing_message(app_state: &Arc<AppState>, message: String) {
    *get_duplicates_processing_state(app_state).deref_mut() =
        FindDuplicatesStateType::Processing(message);
}

fn find_duplicate_file_sizes(app_state: &Arc<AppState>) -> Option<Vec<u64>> {
    app_state.with_sqlite_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT file_len, COUNT(*) c FROM files f GROUP BY file_len HAVING c > 1")?;
        let mut query = stmt.query([])?;
        let mut sizes = vec![];
        while let Some(row) = query.next()? {
            sizes.push(row.get(0)?);
        }
        Ok(sizes)
    })
}

pub fn delete_duplicate(app_state: &Arc<AppState>, duplicate: &FileKrakenDuplicate) {
    // delete from duplicates list
    let duplicate_index = {
        let duplicates_list = app_state
            .find_duplicates_processing
            .duplicates
            .read()
            .unwrap();

        match duplicates_list.iter().position(|x| {
            x.deletable_file.as_ref().is_some_and(|file| {
                duplicate
                    .deletable_file
                    .as_ref()
                    .map(|d| d.path.as_str())
                    .unwrap_or("")
                    == file.path
            })
        }) {
            Some(index) => index,
            None => return,
        }
    };
    {
        app_state
            .find_duplicates_processing
            .duplicates
            .write()
            .unwrap()
            .remove(duplicate_index);
    }

    if let Some(file) = duplicate.deletable_file.as_ref() {
        app_state.remove_file(true, true, &file.path);
    }
}
