use crate::state::duplicate::{FileKrakenDuplicate, FileKrakenDuplicateType};
use crate::state::file::{FileKrakenFile, FileKrakenFileType};
use crate::state::location::FileKrakenLocationType;
use crate::state::AppState;
use crate::utils::get_longest_parent_path;
use crate::utils::locks::{lock_rw_read_or_exit, lock_rw_write_or_exit};
use crate::utils::size_unit::SizeUnit;
use egui::ahash::HashMap;
use std::ops::DerefMut;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock, RwLockWriteGuard};

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
    let mut files_to_hash = get_files_by_sizes(&app_state, duplicate_file_sizes)?;

    let nr_total = files_to_hash.len();
    let total_bytes: u64 = files_to_hash.iter().map(|(size, _)| *size).sum();
    if nr_total == 0 {
        return Some(HashMap::default());
    }

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let started_count = Arc::new(AtomicUsize::new(0));
    let completed_bytes = Arc::new(AtomicU64::new(0));
    let mut threads = Vec::new();
    // Sort ascending so pop() takes the largest files first for better workload balancing
    files_to_hash.sort_by_key(|(size, _)| *size);
    let tasks = Arc::new(Mutex::new(files_to_hash));

    for _ in 0..num_threads {
        let app_state = app_state.clone();
        let files_by_size_by_hash = files_by_size_by_hash.clone();
        let started_count = started_count.clone();
        let completed_bytes = completed_bytes.clone();
        let tasks = tasks.clone();
        threads.push(std::thread::spawn(move || loop {
            let task = {
                let mut tasks_lock = tasks.lock().unwrap();
                tasks_lock.pop()
            };

            let (size, mut file) = match task {
                Some(t) => t,
                None => break,
            };

            let current_started = started_count.fetch_add(1, Ordering::SeqCst) + 1;

            let update_progress =
                |app_state: &Arc<AppState>, completed_bytes: u64, started_idx: usize| {
                    let total_gb = total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                    let completed_gb = completed_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

                    let progress_pct = completed_bytes as f64 * 100.0 / total_bytes as f64;
                    let progress_msg = if total_gb >= 1.0 {
                        format!(
                            "{:.2}% | Hashing file {}/{} ({:.2}/{:.2} GB)",
                            progress_pct, started_idx, nr_total, completed_gb, total_gb
                        )
                    } else {
                        format!(
                            "{:.2}% | Hashing file {}/{} ({:.2}/{:.2} MB)",
                            progress_pct,
                            started_idx,
                            nr_total,
                            completed_bytes as f64 / (1024.0 * 1024.0),
                            total_bytes as f64 / (1024.0 * 1024.0)
                        )
                    };
                    set_processing_message(app_state, progress_msg);
                };

            update_progress(
                &app_state,
                completed_bytes.load(Ordering::SeqCst),
                current_started,
            );

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

            let current_completed_bytes = completed_bytes.fetch_add(size, Ordering::SeqCst) + size;
            update_progress(&app_state, current_completed_bytes, current_started);
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

                let (preferred_files, normal_files) = {
                    let locations = app_state.get_locations_list_readonly();
                    let mut preferred = Vec::new();
                    let mut normal = Vec::new();

                    for file in files {
                        let location_type = get_longest_parent_path(&file.path, locations.iter())
                            .and_then(|p| locations.iter().find(|loc| loc.path == p))
                            .map(|loc| loc.location_type.clone())
                            .unwrap_or(FileKrakenLocationType::Normal);

                        if location_type == FileKrakenLocationType::Preferred {
                            preferred.push(file.clone());
                        } else {
                            normal.push(file.clone());
                        }
                    }
                    (preferred, normal)
                };

                if !preferred_files.is_empty() {
                    let reference = preferred_files[0].clone();

                    // Rows for other preferred files
                    for other_preferred in preferred_files.iter().skip(1) {
                        duplicates_list.push(FileKrakenDuplicate {
                            deletable_file: None,
                            other_files: vec![reference.clone(), other_preferred.clone()],
                            duplicate_type: FileKrakenDuplicateType::ExactMatch,
                        });
                    }

                    // Rows for normal files (deletable)
                    for normal in &normal_files {
                        duplicates_list.push(FileKrakenDuplicate {
                            deletable_file: Some(normal.clone()),
                            other_files: vec![reference.clone()],
                            duplicate_type: FileKrakenDuplicateType::ExactMatch,
                        });
                    }
                } else {
                    // No preferred files, use first normal as reference
                    let reference = normal_files[0].clone();
                    for other_normal in normal_files.iter().skip(1) {
                        duplicates_list.push(FileKrakenDuplicate {
                            deletable_file: None,
                            other_files: vec![reference.clone(), other_normal.clone()],
                            duplicate_type: FileKrakenDuplicateType::ExactMatch,
                        });
                    }
                }
            }
        }
    }
    Some(())
}

fn get_files_by_sizes(
    app_state: &Arc<AppState>,
    sizes: Vec<u64>,
) -> Option<Vec<(u64, FileKrakenFile)>> {
    app_state.with_sqlite_conn(|conn| {
        let mut files = Vec::new();
        for chunk in sizes.chunks(999) {
            let placeholders = vec!["?"; chunk.len()].join(", ");
            let query = format!(
                "SELECT \
                path, file_type, file_len, time_created, time_modified, hash_256 \
                FROM files \
                WHERE file_len IN ({})",
                placeholders
            );
            let mut stmt = conn.prepare(&query)?;
            let params = rusqlite::params_from_iter(chunk.iter());
            let mut select_files = stmt.query(params)?;

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
                let file_len: u64 = row.get(2)?;
                files.push((
                    file_len,
                    FileKrakenFile {
                        path: file_path,
                        file_type,
                        file_len,
                        time_created: row.get(3)?,
                        time_modified: row.get(4)?,
                        hash,
                    },
                ));
            }
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
