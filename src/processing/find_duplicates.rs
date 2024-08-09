use crate::state::duplicate::FileKrakenDuplicate;
use crate::state::file::{FileKrakenFile, FileKrakenFileType};
use crate::state::AppState;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex, RwLock};

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
    set_processing_message(&app_state, "Scanning for file size matches...");
    let duplicate_file_sizes = find_duplicate_file_sizes(&app_state.sqlite);

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
        }
    }

    *app_state
        .find_duplicates_processing
        .state
        .write()
        .unwrap()
        .deref_mut() = FindDuplicatesStateType::Processed;
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

fn set_processing_message(app_state: &Arc<AppState>, message: &str) {
    *app_state
        .find_duplicates_processing
        .state
        .write()
        .unwrap()
        .deref_mut() = FindDuplicatesStateType::Processing(message.to_string());
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
