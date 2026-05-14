use crate::state::duplicate::{FileKrakenDuplicate, FileKrakenDuplicateType};
use crate::state::file::{FileKrakenFile, FileKrakenFileType};
use crate::state::location::{FileKrakenLocation, FileKrakenLocationType};
use crate::state::AppState;
use crate::utils::get_longest_parent_path;
use crate::utils::locks::{lock_rw_read_or_exit, lock_rw_write_or_exit};
use crate::utils::size_unit::SizeUnit;
use egui::ahash::HashMap;
use std::cmp::max;
use std::ops::DerefMut;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock, RwLockWriteGuard};

pub struct FindDuplicatesState {
    pub duplicates: RwLock<Vec<FileKrakenDuplicate>>,
    pub state: RwLock<FindDuplicatesStateType>,
    pub min_file_size_input: RwLock<String>,
    pub min_file_size_unit: RwLock<SizeUnit>,
    pub include_same_location_duplicates: RwLock<bool>,
}

impl Default for FindDuplicatesState {
    fn default() -> Self {
        Self {
            duplicates: RwLock::new(vec![]),
            state: RwLock::new(FindDuplicatesStateType::None),
            min_file_size_input: RwLock::new("0".to_string()),
            min_file_size_unit: RwLock::new(SizeUnit::MB),
            include_same_location_duplicates: RwLock::new(false),
        }
    }
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
        clear_duplicates_list(&app_state);
        *get_duplicates_processing_state(&app_state).deref_mut() = FindDuplicatesStateType::None;
    }
}

pub fn run_find_file_duplicates(app_state: Arc<AppState>) -> Option<()> {
    if !prepare_duplicate_search(&app_state) {
        return Some(());
    }

    set_processing_message(&app_state, "Scanning for file size matches...".to_string());
    let duplicate_file_sizes = find_duplicate_file_sizes(&app_state)?;

    let files_by_size_by_hash = hash_potential_duplicates(app_state.clone(), duplicate_file_sizes)?;
    detect_hashed_duplicates(&app_state, files_by_size_by_hash)?;

    *get_duplicates_processing_state(&app_state).deref_mut() = FindDuplicatesStateType::Processed;

    Some(())
}

fn clear_duplicates_list(app_state: &Arc<AppState>) -> bool {
    if let Some(mut dups) =
        lock_rw_write_or_exit(&app_state.find_duplicates_processing.duplicates, app_state)
    {
        dups.clear();
        true
    } else {
        false
    }
}

fn prepare_duplicate_search(app_state: &Arc<AppState>) -> bool {
    if let FindDuplicatesStateType::Processing(_) =
        get_duplicates_processing_state(app_state).deref_mut()
    {
        rfd::MessageDialog::new()
            .set_title("Already processing")
            .set_description("Already processing duplicates")
            .show();
        return false;
    }

    clear_duplicates_list(app_state)
}

type FilesBySizeByHash = HashMap<u64, Arc<RwLock<HashMap<String, Vec<FileKrakenFile>>>>>;

fn hash_potential_duplicates(
    app_state: Arc<AppState>,
    duplicate_file_sizes: Vec<u64>,
) -> Option<FilesBySizeByHash> {
    let files_by_size_by_hash = Arc::new(RwLock::new(HashMap::default()));
    let mut files_to_hash = Vec::new();
    for size in duplicate_file_sizes {
        if let Some(files) = get_files_by_size(&app_state, size) {
            for file in files {
                files_to_hash.push((size, file));
            }
        }
    }

    let nr_total = files_to_hash.len();
    if nr_total == 0 {
        return Some(HashMap::default());
    }

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let hashed_count = Arc::new(AtomicUsize::new(0));
    let mut threads = Vec::new();

    for chunk in files_to_hash
        .chunks(max(1, nr_total / num_threads))
        .map(|c| c.to_vec())
    {
        let app_state = app_state.clone();
        let files_by_size_by_hash = files_by_size_by_hash.clone();
        let hashed_count = hashed_count.clone();
        threads.push(std::thread::spawn(move || {
            for (size, mut file) in chunk {
                let current = hashed_count.fetch_add(1, Ordering::SeqCst);
                if current % 10 == 0 {
                    set_processing_message(
                        &app_state,
                        format!(
                            "{:.2}% | Hashing file {}/{}",
                            current as f64 * 100.0 / nr_total as f64,
                            current,
                            nr_total
                        ),
                    );
                }
                if let Some(hash) = app_state.calculate_file_hash(&file.path) {
                    file.hash = Some(hash.clone());
                    let size_entry = {
                        let mut map = files_by_size_by_hash.write().unwrap();
                        map.entry(size)
                            .or_insert_with(|| Arc::new(RwLock::new(HashMap::default())))
                            .clone()
                    };
                    size_entry
                        .write()
                        .unwrap()
                        .entry(hash)
                        .or_insert(Vec::new())
                        .push(file);
                }
            }
        }));
    }

    for thread in threads {
        thread.join().ok()?;
    }

    Arc::try_unwrap(files_by_size_by_hash)
        .ok()
        .map(|lock| lock.into_inner().unwrap())
}

fn detect_hashed_duplicates(
    app_state: &Arc<AppState>,
    files_by_size_by_hash: FilesBySizeByHash,
) -> Option<()> {
    set_processing_message(
        app_state,
        "Checking file-hashes for duplicates...".to_string(),
    );
    for (_, files_by_size) in files_by_size_by_hash.iter() {
        for (_, files) in lock_rw_read_or_exit(files_by_size, app_state)?.iter() {
            if files.len() > 1 {
                let mut duplicates_list = lock_rw_write_or_exit(
                    &app_state.find_duplicates_processing.duplicates,
                    app_state,
                )?;

                let deletable_file = get_deletable_file(app_state, files);
                let other_files = if let Some(ref deletable) = deletable_file {
                    files
                        .iter()
                        .filter(|x| x.path != deletable.path)
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
                        .or_else(|| duplicate.other_files.first())
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
    files: &[FileKrakenFile],
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
            let hash: Option<String> = row.get(5)?;
            files.push(FileKrakenFile {
                path: file_path,
                file_type,
                file_len: row.get(2)?,
                time_created: row.get(3)?,
                time_modified: row.get(4)?,
                hash,
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
    let min_size = {
        let input = app_state
            .find_duplicates_processing
            .min_file_size_input
            .read()
            .unwrap();
        let unit = app_state
            .find_duplicates_processing
            .min_file_size_unit
            .read()
            .unwrap();
        input.parse::<u64>().unwrap_or(0) * unit.multiplier()
    };

    let include_same_location = *app_state
        .find_duplicates_processing
        .include_same_location_duplicates
        .read()
        .unwrap();

    app_state.with_sqlite_conn(|conn| {
        let mut stmt = if include_same_location {
            conn.prepare(
                "SELECT file_len, COUNT(*) c FROM files f \
                 WHERE file_len >= ?1 \
                 GROUP BY file_len HAVING c > 1",
            )?
        } else {
            conn.prepare(
                "SELECT file_len, COUNT(*) c FROM files f \
                 WHERE file_len >= ?1 \
                 GROUP BY file_len HAVING c > 1 AND COUNT(DISTINCT location_path) > 1",
            )?
        };

        let mut query = stmt.query([min_size])?;
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
