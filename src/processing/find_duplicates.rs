use crate::state::duplicate::{FileKrakenDuplicate, FileKrakenDuplicateType};
use crate::state::file::{FileKrakenFile, FileKrakenFileType};
use crate::state::location::{FileKrakenLocation, FileKrakenLocationType};
use crate::state::AppState;
use crate::utils::get_longest_parent_path;
use egui::ahash::HashMap;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex, RwLock, RwLockWriteGuard};

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
    if let FindDuplicatesStateType::Processing(_) =
        get_duplicates_processing_state(&app_state).deref_mut()
    {
        rfd::MessageDialog::new()
            .set_title("Already processing")
            .set_description("Already processing duplicates")
            .show();
        return;
    }
    app_state
        .find_duplicates_processing
        .duplicates
        .write()
        .unwrap()
        .clear();

    set_processing_message(&app_state, "Scanning for file size matches...");
    let duplicate_file_sizes = find_duplicate_file_sizes(&app_state.sqlite);

    let mut files_by_size_by_hash = HashMap::default();
    for duplicate_file_size in duplicate_file_sizes.expect("Failed to get duplicate file sizes") {
        set_processing_message(
            &app_state,
            format!(
                "Calculating hashes for files of size {}",
                duplicate_file_size
            )
            .as_str(),
        );
        let mut duplicate_files = get_files_by_size(&app_state, duplicate_file_size).unwrap();
        for file in &mut duplicate_files {
            // calc hash
            file.hash = Some(app_state.calculate_file_hash(&file.path));
            files_by_size_by_hash
                .entry(duplicate_file_size)
                .or_insert(HashMap::default())
                .entry(file.hash.clone().unwrap())
                .or_insert(vec![])
                .push(file.clone());
        }
    }
    set_processing_message(&app_state, "Checking file-hashes for duplicates...");
    for (_, files_by_hash) in files_by_size_by_hash.iter() {
        for (_, files) in files_by_hash.iter() {
            if files.len() > 1 {
                let mut duplicates_list = app_state
                    .find_duplicates_processing
                    .duplicates
                    .write()
                    .unwrap();

                let deletable_file = get_deletable_file(&app_state, &files);
                let other_files = if deletable_file.is_some() {
                    files
                        .iter()
                        .filter(|x| x.path != deletable_file.as_ref().unwrap().path)
                        .cloned()
                        .collect()
                } else {
                    files.clone()
                };
                duplicates_list.push(FileKrakenDuplicate {
                    other_files,
                    deletable_file,
                    duplicate_type: FileKrakenDuplicateType::ExactMatch,
                });
            }
        }
    }

    *get_duplicates_processing_state(&app_state).deref_mut() = FindDuplicatesStateType::Processed;
}

fn get_deletable_file(
    app_state: &Arc<AppState>,
    files: &Vec<FileKrakenFile>,
) -> Option<FileKrakenFile> {
    let (preferred_file, normal_file) = {
        let locations = app_state.get_locations_list_readonly();
        let file_locations: Vec<(FileKrakenFile, Option<FileKrakenLocation>)> = files
            .iter()
            .map(|file| {
                (
                    file.clone(),
                    get_longest_parent_path(&file.path, locations.iter())
                        .map(|x| locations.iter().find(|loc| loc.path == x).unwrap().clone()),
                )
            })
            .collect();

        (
            file_locations
                .iter()
                .filter(|(_, location)| location.is_some())
                .find(|(file, location)| {
                    location
                        .as_ref()
                        .is_some_and(|loc| loc.location_type == FileKrakenLocationType::Preferred)
                })
                .map(|(file, _)| file.clone()),
            file_locations
                .iter()
                .filter(|(_, location)| location.is_some())
                .find(|(file, location)| {
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
    let sqlite_lock = app_state.sqlite.lock().unwrap();
    let mut files = vec![];

    let mut sqlite_query = sqlite_lock
        .as_ref()?
        .prepare(
            "SELECT \
        path, file_type, file_len, time_created, time_modified, hash_256 \
        FROM files \
        WHERE file_len = ?1",
        )
        .unwrap();
    let mut select_files = sqlite_query.query([size]).ok()?;

    while let Some(row) = select_files.next().ok()? {
        let file_path: String = row.get(0).ok()?;
        let file_type = match row.get::<usize, String>(1).ok()?.as_str() {
            "normal" => FileKrakenFileType::Normal,
            "archive" => FileKrakenFileType::Archive,
            x => {
                panic!("unknown file type {}", x)
            }
        };

        files.push(FileKrakenFile {
            path: file_path,
            file_type,
            file_len: row.get(2).ok()?,
            time_created: row.get(3).ok()?,
            time_modified: row.get(4).ok()?,
            hash: row.get(5).ok()?,
        });
    }

    Some(files)
}

fn get_duplicates_processing_state(
    app_state: &Arc<AppState>,
) -> RwLockWriteGuard<'_, FindDuplicatesStateType> {
    app_state.find_duplicates_processing.state.write().unwrap()
}

fn set_processing_message(app_state: &Arc<AppState>, message: &str) {
    *get_duplicates_processing_state(app_state).deref_mut() =
        FindDuplicatesStateType::Processing(message.to_string());
}

fn find_duplicate_file_sizes(
    sqlite: &Arc<Mutex<Option<rusqlite::Connection>>>,
) -> Option<Vec<u64>> {
    let sqlite_lock = sqlite.lock().unwrap();
    let mut find_duplicate_file_sizes = sqlite_lock
        .as_ref()
        .unwrap()
        .prepare("SELECT file_len, COUNT(*) c FROM files f GROUP BY file_len HAVING c > 1")
        .ok()?;
    let mut find_duplicate_file_sizes_query = find_duplicate_file_sizes.query([]).ok()?;

    let mut duplicate_file_sizes = vec![];
    while let Some(row) = find_duplicate_file_sizes_query.next().ok()? {
        duplicate_file_sizes.push(row.get(0).ok()?);
    }

    Some(duplicate_file_sizes)
}

pub fn delete_duplicate(app_state: &Arc<AppState>, duplicate: &FileKrakenDuplicate) {
    // delete from duplicates list
    let mut duplicates_list = app_state
        .find_duplicates_processing
        .duplicates
        .write()
        .unwrap();
    let duplicate_index = duplicates_list
        .iter()
        .position(|x| {
            x.deletable_file
                .as_ref()
                .is_some_and(|file| file.path == duplicate.deletable_file.as_ref().unwrap().path)
        })
        .unwrap();
    duplicates_list.remove(duplicate_index);

    app_state.remove_file(true, true, &duplicate.deletable_file.as_ref().unwrap().path);
}
