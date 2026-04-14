use crate::processing::find_duplicates::{FindDuplicatesState, FindDuplicatesStateType};
use crate::state::file::{FileKrakenFile, FileKrakenFileType};
use crate::state::location::{FileKrakenLocation, FileKrakenLocationState, FileKrakenLocationType};
use crate::utils::dialogs::error_dialog;
use crate::utils::get_longest_parent_path;
use crate::utils::hashing::hash_file;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard};

#[derive(Default)]
pub struct AppState {
    pub find_duplicates_processing: Arc<FindDuplicatesState>,
    pub sqlite: Arc<Mutex<Option<rusqlite::Connection>>>,
    locations_list: Arc<RwLock<Vec<FileKrakenLocation>>>,
    #[allow(clippy::type_complexity)]
    files_by_location_by_path:
        Arc<RwLock<HashMap<String, Arc<RwLock<HashMap<String, FileKrakenFile>>>>>>,
}

impl AppState {
    pub fn connect_sqlite(&self, path: &str) -> Result<(), rusqlite::Error> {
        let connection = rusqlite::Connection::open(path)?;
        connection.execute(
            "CREATE TABLE IF NOT EXISTS locations (
                path TEXT PRIMARY KEY,
                location_type TEXT NOT NULL,
                location_state TEXT NOT NULL
            );",
            [],
        )?;
        connection.execute(
            "CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                location_path TEXT NOT NULL,
                file_type TEXT NOT NULL,
                file_len INTEGER NOT NULL,
                time_created INTEGER NOT NULL,
                time_modified INTEGER NOT NULL,
                hash_256 TEXT,

                FOREIGN KEY(location_path) REFERENCES locations(path)
            );",
            [],
        )?;
        connection.execute(
            "CREATE INDEX IF NOT EXISTS file_location_index 
                ON files(location_path);",
            [],
        )?;
        connection.execute(
            "CREATE INDEX IF NOT EXISTS file_len_index
                ON files(file_len);",
            [],
        )?;
        connection.execute(
            "CREATE INDEX IF NOT EXISTS file_hash_index
                ON files(hash_256);",
            [],
        )?;

        // load locations from sqlite
        {
            let mut select_locations =
                connection.prepare("SELECT path, location_type, location_state FROM locations")?;
            let mut locations_query = select_locations.query([])?;

            while let Some(row) = locations_query.next()? {
                let location_path: String = row.get(0)?;
                let location_type = match row.get::<usize, String>(1)?.as_str() {
                    "normal" => FileKrakenLocationType::Normal,
                    "preferred" => FileKrakenLocationType::Preferred,
                    "excluded" => FileKrakenLocationType::Excluded,
                    _ => FileKrakenLocationType::Normal,
                };
                let location_state = match row.get::<usize, String>(2)?.as_str() {
                    "unscanned" => FileKrakenLocationState::Unscanned,
                    "partial_scanned" => FileKrakenLocationState::PartialScanned,
                    "scanned" => FileKrakenLocationState::Scanned,
                    _ => FileKrakenLocationState::Unscanned,
                };
                self.add_location(false, &location_path, &location_type, &location_state);
            }
        }

        // load files from sqlite
        {
            let mut select_files = connection.prepare("SELECT path, file_type, file_len, time_created, time_modified, hash_256 FROM files")?;
            let mut files_query = select_files.query([])?;

            while let Some(row) = files_query.next()? {
                let file_path: String = row.get(0)?;
                let file_type = match row.get::<usize, String>(1)?.as_str() {
                    "normal" => FileKrakenFileType::Normal,
                    "archive" => FileKrakenFileType::Archive,
                    x => {
                        panic!("unknown file type {}", x)
                    }
                };

                self.add_file(
                    false,
                    &file_path,
                    &file_type,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                )
            }
        }

        for i in 0..self.locations_list.read().unwrap().len() {
            let lock = self.locations_list.read().unwrap();
            let location = lock.get(i).unwrap();
            if location.location_state == FileKrakenLocationState::Unscanned {
                self.clear_location_files(false, &location.path);
            }
        }

        *self.sqlite.lock().unwrap().deref_mut() = Some(connection);
        Ok(())
    }

    /// Acquire the SQLite connection mutex or exit the project on failure.
    pub fn sqlite_lock_or_exit(
        &self,
    ) -> Option<std::sync::MutexGuard<'_, Option<rusqlite::Connection>>> {
        match self.sqlite.lock() {
            Ok(g) => Some(g),
            Err(_) => {
                error_dialog("Internal error: database lock poisoned. Closing project.");
                self.close_project();
                None
            }
        }
    }

    /// Execute a closure with a reference to the SQLite connection. If any
    /// error occurs while accessing the connection or running the closure, the
    /// project is closed and `None` is returned.
    pub fn with_sqlite_conn<F, T>(&self, func: F) -> Option<T>
    where
        F: FnOnce(&rusqlite::Connection) -> rusqlite::Result<T>,
    {
        let guard = self.sqlite_lock_or_exit()?;
        let conn = match guard.as_ref() {
            Some(c) => c,
            None => {
                error_dialog("Internal error: database connection missing. Closing project.");
                self.close_project();
                return None;
            }
        };
        match func(conn) {
            Ok(val) => Some(val),
            Err(err) => {
                error_dialog(&format!("Database error: {err}. Closing project."));
                self.close_project();
                None
            }
        }
    }

    pub fn calculate_file_hash(&self, file_path: &str) -> Option<String> {
        // get file to check if its already hashed
        let hash: String = self.with_sqlite_conn(|conn| {
            conn.query_row(
                "SELECT hash_256 FROM files WHERE path = ?1;",
                [file_path],
                |x| x.get(0),
            )
        })?;

        if hash == "NULL" {
            match hash_file(file_path) {
                Ok(hash) => {
                    let _ = self.with_sqlite_conn(|conn| {
                        conn.execute(
                            "UPDATE files SET hash_256 = ?1 WHERE path = ?2;",
                            [&hash, file_path],
                        )
                    });
                    Some(hash)
                }
                Err(err) => {
                    error_dialog(&format!("Failed to hash file {file_path}: {err}"));
                    None
                }
            }
        } else {
            Some(hash)
        }
    }

    pub fn is_sqlite_connected(&self) -> bool {
        self.sqlite.lock().unwrap().is_some()
    }

    pub fn remove_location(&self, persist_to_db: bool, location_path: &str) {
        self.locations_list
            .write()
            .unwrap()
            .iter_mut()
            .find(|x| x.path == location_path)
            .unwrap_or_else(|| panic!("location {} not found", location_path))
            .location_state = FileKrakenLocationState::Deleting;

        self.clear_location_files(persist_to_db, location_path);

        self.files_by_location_by_path
            .write()
            .unwrap()
            .remove(location_path);

        let mut locations_list = self.locations_list.write().unwrap();
        locations_list.retain(|x| x.path != location_path);
    }

    pub fn remove_file(&self, persist_to_db: bool, persist_to_disk: bool, file_path: &str) {
        if persist_to_disk {
            match std::fs::remove_file(file_path) {
                Ok(_) => {}
                Err(_) => {
                    rfd::MessageDialog::new()
                        .set_title("Error")
                        .set_description("Failed to delete file")
                        .show();
                    return;
                }
            }
        }

        if persist_to_db {
            let sqlite_lock = self.sqlite.lock().unwrap();
            let mut delete_file = sqlite_lock
                .as_ref()
                .unwrap()
                .prepare("DELETE FROM files WHERE path = ?1")
                .unwrap();
            delete_file.execute([file_path]).unwrap();
        }

        let file_parent_location_path =
            get_longest_parent_path(file_path, self.get_locations_list_readonly().iter())
                .unwrap_or_else(|| panic!("no parent location found for file {}", file_path));

        self.files_by_location_by_path
            .read()
            .unwrap()
            .get(&file_parent_location_path)
            .unwrap()
            .write()
            .unwrap()
            .remove(file_path);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_file(
        &self,
        persist_to_db: bool,
        file_path: &str,
        file_type: &FileKrakenFileType,
        file_len: u64,
        time_created: u64,
        time_modified: u64,
        hash: Option<String>,
    ) {
        let parent_location =
            get_longest_parent_path(file_path, self.get_locations_list_readonly().iter())
                .unwrap_or_else(|| panic!("no parent location found for file {}", file_path));

        self.add_file_to_location(
            persist_to_db,
            parent_location.as_ref(),
            file_path,
            file_type,
            file_len,
            time_created,
            time_modified,
            hash,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_file_to_location(
        &self,
        persist_to_db: bool,
        location_path: &str,
        file_path: &str,
        file_type: &FileKrakenFileType,
        file_len: u64,
        time_created: u64,
        time_modified: u64,
        hash: Option<String>,
    ) {
        let file = FileKrakenFile {
            path: file_path.to_string(),
            file_type: file_type.clone(),
            file_len,
            time_created,
            time_modified,
            hash,
        };
        log::trace!(
            "adding file {:?} len {} created {} modified {}",
            file.file_type,
            file.file_len,
            file.time_created,
            file.time_modified
        );

        if persist_to_db {
            if let Some((_, existing_location)) = {
                self.sqlite
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .query_row(
                        "SELECT path, location_path FROM files WHERE path = ?1;",
                        [file_path],
                        |x| Ok((x.get::<_, String>(0)?.to_string(), x.get::<_, String>(1)?)),
                    )
                    .ok()
            } {
                if existing_location != location_path {
                    self.remove_file(true, false, file_path);
                }
            }

            self.sqlite
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .execute(
                    "INSERT INTO files (\
                    path, \
                    location_path, \
                    file_type,\
                    file_len,\
                    time_created,\
                    time_modified,\
                    hash_256\
                ) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(path) DO NOTHING;",
                    [
                        file_path,
                        location_path,
                        "normal",
                        &file_len.to_string(),
                        &time_created.to_string(),
                        &time_modified.to_string(),
                        &(match &file.hash {
                            Some(x) => x.to_string(),
                            None => "NULL".to_string(),
                        }),
                    ],
                )
                .unwrap();
        }

        let location_state = self
            .get_location_clone(location_path)
            .unwrap()
            .location_state;
        if location_state == FileKrakenLocationState::Unscanned {
            self.modify_location_state(
                persist_to_db,
                location_path,
                FileKrakenLocationState::PartialScanned,
            );
        }

        let location_files = {
            self.files_by_location_by_path
                .read()
                .unwrap()
                .get(location_path)
                .unwrap()
                .clone()
        };
        location_files
            .write()
            .unwrap()
            .insert(file_path.to_string(), file);
    }

    pub fn get_location_clone(&self, location_path: &str) -> Option<FileKrakenLocation> {
        self.locations_list
            .read()
            .unwrap()
            .iter()
            .find(|x| x.path == location_path)
            .cloned()
    }

    pub fn get_locations_list_readonly(&self) -> RwLockReadGuard<'_, Vec<FileKrakenLocation>> {
        self.locations_list.read().unwrap()
    }

    pub fn get_location_files_count(&self, location: &str) -> usize {
        let location_files = {
            self.files_by_location_by_path
                .read()
                .unwrap()
                .get(location)
                .cloned()
        };

        location_files
            .map(|files| files.read().unwrap().len())
            .unwrap_or_default()
    }

    pub fn get_total_files_count(&self) -> usize {
        let location_files = self
            .files_by_location_by_path
            .read()
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>();

        location_files
            .iter()
            .map(|files| files.read().unwrap().len())
            .sum()
    }

    pub fn has_active_background_work(&self) -> bool {
        // Keep repainting while duplicate processing is running.
        if matches!(
            &*self.find_duplicates_processing.state.read().unwrap(),
            FindDuplicatesStateType::Processing(_)
        ) {
            return true;
        }

        // Keep repainting while any location is in a transient state.
        self.locations_list.read().unwrap().iter().any(|location| {
            location.location_state == FileKrakenLocationState::Scanning
                || location.location_state == FileKrakenLocationState::Deleting
        })
    }

    pub fn modify_location_type(&self, location_path: &str, location_type: FileKrakenLocationType) {
        // do nothing if type is the same
        let current_location_type = self
            .locations_list
            .read()
            .unwrap()
            .iter()
            .find(|x| x.path == location_path)
            .map(|x| x.location_type.clone())
            .unwrap();
        if current_location_type == location_type {
            return;
        }

        self.sqlite
            .lock()
            .unwrap()
            .as_mut()
            .expect("sqlite connection not set")
            .execute(
                "UPDATE locations SET location_type = ? WHERE path = ?;",
                [&location_type.to_string(), location_path],
            )
            .unwrap();
        let mut locations_list = self.locations_list.write().unwrap();
        for location in locations_list.iter_mut() {
            if location.path == location_path {
                location.location_type = location_type;
                break;
            }
        }
        // TODO update affected locations
    }

    pub fn modify_location_state(
        &self,
        persist_to_db: bool,
        location_path: &str,
        location_state: FileKrakenLocationState,
    ) {
        // do nothing if state is the same
        let current_location_state = self
            .locations_list
            .read()
            .unwrap()
            .iter()
            .find(|x| x.path == location_path)
            .map(|x| x.location_state.clone())
            .unwrap();
        if current_location_state == location_state {
            return;
        }

        if persist_to_db {
            self.sqlite
                .lock()
                .unwrap()
                .as_mut()
                .expect("sqlite connection not set")
                .execute(
                    "UPDATE locations SET location_state = ? WHERE path = ?;",
                    [&location_state.to_string(), location_path],
                )
                .unwrap();
        }

        let mut locations_list = self.locations_list.write().unwrap();
        for location in locations_list.iter_mut() {
            if location.path == location_path {
                location.location_state = location_state;
                break;
            }
        }
        // TODO update affected locations
    }

    pub fn clear_location_files(&self, persist_to_db: bool, location_path: &str) {
        if persist_to_db {
            self.sqlite
                .lock()
                .unwrap()
                .as_ref()
                .expect("Sql connection not set")
                .execute(
                    "DELETE FROM files WHERE location_path = ?;",
                    [location_path],
                )
                .unwrap();
        }

        let files_by_location = self.files_by_location_by_path.read().unwrap();
        files_by_location
            .get(location_path)
            .unwrap()
            .write()
            .unwrap()
            .clear();
    }

    pub fn add_location(
        &self,
        persist_to_db: bool,
        location_path: &str,
        location_type: &FileKrakenLocationType,
        location_state: &FileKrakenLocationState,
    ) {
        assert!(!self
            .locations_list
            .read()
            .unwrap()
            .iter()
            .any(|x| x.path == location_path));

        if persist_to_db {
            self.sqlite
                .lock()
                .unwrap()
                .as_ref()
                .expect("Sql connection not set")
                .execute(
                    "INSERT INTO locations (path, location_type, location_state) VALUES (?, ?, ?);",
                    [
                        location_path,
                        &location_type.to_string(),
                        &location_state.to_string(),
                    ],
                )
                .unwrap();
        }

        self.files_by_location_by_path.write().unwrap().insert(
            location_path.to_string(),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let parent_location_path =
            get_longest_parent_path(location_path, self.get_locations_list_readonly().iter());

        if let Some(path) = &parent_location_path {
            self.clear_location_files(persist_to_db, path)
        }

        self.locations_list
            .write()
            .unwrap()
            .push(FileKrakenLocation {
                path: location_path.to_string(),
                location_type: location_type.clone(),
                location_state: location_state.clone(),
                parent_location_path,
            });
    }

    pub fn get_files_by_location(
        &self,
        location: &str,
    ) -> Option<Arc<RwLock<HashMap<String, FileKrakenFile>>>> {
        self.files_by_location_by_path
            // get readonly access to the by-location hashmap
            .read()
            .unwrap()
            // look for the by-filepath hashmap for the given location
            .get(location)
            // clone the Arc reference of the Hashmap if it exists
            .cloned()
    }

    /// Close the currently open project and clear all in-memory state.
    pub fn close_project(&self) {
        if let Ok(mut sqlite) = self.sqlite.lock() {
            *sqlite = None;
        }
        if let Ok(mut locations) = self.locations_list.write() {
            locations.clear();
        }
        if let Ok(mut files) = self.files_by_location_by_path.write() {
            files.clear();
        }
        if let Ok(mut dups) = self.find_duplicates_processing.duplicates.write() {
            dups.clear();
        }
        if let Ok(mut state) = self.find_duplicates_processing.state.write() {
            *state = FindDuplicatesStateType::None;
        }
    }
}
