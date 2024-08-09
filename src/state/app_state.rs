use crate::processing::find_duplicates::FindDuplicatesState;
use crate::state::file::{FileKrakenFile, FileKrakenFileType};
use crate::state::location::{FileKrakenLocation, FileKrakenLocationState, FileKrakenLocationType};
use crate::utils::get_longest_parent_path;
use crate::utils::hashing::hash_file;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard};

#[derive(Default)]
pub struct AppState {
    pub find_duplicates_processing: Arc<FindDuplicatesState>,
    pub sqlite: Arc<Mutex<Option<rusqlite::Connection>>>,
    locations_list: Arc<RwLock<Vec<FileKrakenLocation>>>,
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

    pub fn calculate_file_hash(&self, file_path: &str) -> String {
        // get file to check if its already hashed
        let hash: String = self
            .sqlite
            .lock()
            .unwrap()
            .as_ref()
            .expect("sqlite connection not set")
            .query_row(
                "SELECT hash_256 FROM files WHERE path = ?1;",
                [file_path],
                |x| x.get(0),
            )
            .unwrap();

        match hash.as_str() {
            "NULL" => {
                // calculate hash
                let hash = hash_file(&file_path);

                // update hash in sqlite
                self.sqlite
                    .lock()
                    .unwrap()
                    .as_ref()
                    .expect("sqlite connection not set")
                    .execute(
                        "UPDATE files SET hash_256 = ?1 WHERE path = ?2;",
                        [&hash, file_path],
                    )
                    .unwrap();

                hash
            }
            x => x.to_string(),
        }
    }

    pub fn is_sqlite_connected(&self) -> bool {
        self.sqlite.lock().unwrap().is_some()
    }

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
                .expect(&format!("no parent location found for file {}", file_path));

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

        if persist_to_db {
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

        let files_by_location = self.files_by_location_by_path.read().unwrap();
        files_by_location
            .get(location_path)
            .unwrap()
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
            .map(|x| x.clone())
    }

    pub fn get_locations_list_readonly(&self) -> RwLockReadGuard<'_, Vec<FileKrakenLocation>> {
        self.locations_list.read().unwrap()
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
        assert!(self
            .locations_list
            .read()
            .unwrap()
            .iter()
            .find(|x| x.path == location_path)
            .is_none());

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
            .map(|x| x.clone())
    }
}
