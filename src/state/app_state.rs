use crate::state::file::{FileKrakenFile, FileKrakenFileType};
use crate::state::location::{FileKrakenLocation, FileKrakenLocationType};
use crate::utils::get_longest_parent_path;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard};

#[derive(Default)]
pub struct AppState {
    sqlite: Option<Arc<Mutex<rusqlite::Connection>>>,
    locations_list: Arc<RwLock<Vec<FileKrakenLocation>>>,
    files_by_location_by_path:
        Arc<RwLock<HashMap<String, Arc<RwLock<HashMap<String, FileKrakenFile>>>>>>,
}

impl AppState {
    pub fn connect_sqlite(&mut self, path: &str) -> Result<(), rusqlite::Error> {
        let connection = rusqlite::Connection::open(path)?;
        connection.execute(
            "CREATE TABLE IF NOT EXISTS locations (
                path TEXT PRIMARY KEY,
                location_type TEXT NOT NULL
            );",
            [],
        )?;
        connection.execute(
            "CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                location_path TEXT NOT NULL,
                file_type TEXT NOT NULL,

                FOREIGN KEY(location_path) REFERENCES locations(path)
            );",
            [],
        )?;
        connection.execute(
            "CREATE INDEX IF NOT EXISTS file_location_index 
                ON files(location_path);",
            [],
        )?;

        // load locations from sqlite
        {
            let mut select_locations =
                connection.prepare("SELECT path, location_type FROM locations")?;
            let mut locations_query = select_locations.query([])?;

            while let Some(row) = locations_query.next()? {
                let location = FileKrakenLocation {
                    path: row.get(0)?,
                    location_type: match row.get::<usize, String>(1)?.as_str() {
                        "normal" => FileKrakenLocationType::Normal,
                        "preferred" => FileKrakenLocationType::Preferred,
                        "excluded" => FileKrakenLocationType::Excluded,
                        _ => FileKrakenLocationType::Normal,
                    },
                    parent_location_path: None,
                }
                .clone();
                self.files_by_location_by_path
                    .write()
                    .unwrap()
                    .insert(location.path.clone(), Arc::new(RwLock::new(HashMap::new())));
                self.locations_list.write().unwrap().push(location);
            }

            {
                let mut locations_list = self.locations_list.write().unwrap();
                for x in 0..locations_list.len() {
                    locations_list.get_mut(x).unwrap().parent_location_path =
                        get_longest_parent_path(
                            &locations_list.get(x).unwrap().path,
                            locations_list.iter(),
                        );
                }
            }
        }
        // load files from sqlite
        {
            let mut select_files = connection.prepare("SELECT path, file_type FROM files")?;
            let mut files_query = select_files.query([])?;

            while let Some(row) = files_query.next()? {
                let file = FileKrakenFile {
                    path: row.get(0)?,
                    file_type: match row.get::<usize, String>(1)?.as_str() {
                        "normal" => FileKrakenFileType::Normal,
                        "archive" => FileKrakenFileType::Archive,
                        x => {
                            panic!("unknown file type {}", x)
                        }
                    },
                };

                let file_parent_location =
                    get_longest_parent_path(&file.path, self.get_locations_list_readonly().iter())
                        .unwrap();

                self.files_by_location_by_path
                    .write()
                    .unwrap()
                    .get_mut(&file_parent_location)
                    .unwrap()
                    .write()
                    .unwrap()
                    .insert(file.path.clone(), file);
            }
        }

        self.sqlite = Some(Arc::new(Mutex::new(connection)));
        Ok(())
    }

    pub fn is_sqlite_connected(&self) -> bool {
        self.sqlite.is_some()
    }

    pub fn add_file_to_location(&self, persist_to_db: bool, location_path: &str, file_path: &str) {
        let file = FileKrakenFile {
            path: file_path.to_string(),
            file_type: Default::default(),
        };

        if persist_to_db {
            self.sqlite.as_ref().unwrap().lock().unwrap().execute(
                "INSERT INTO files (path, location_path, file_type) VALUES (?, ?, ?) ON CONFLICT(path) DO NOTHING;",
                [file_path, location_path, "normal"],
            ).unwrap();
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
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
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

    pub fn clear_location_files(&self, location_path: &str) {
        self.sqlite
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM files WHERE location_path = ?;",
                [location_path],
            )
            .unwrap();
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
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO locations (path, location_type) VALUES (?, ?);",
                    [location_path, &location_type.to_string()],
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
            self.clear_location_files(path)
        }

        self.locations_list
            .write()
            .unwrap()
            .push(FileKrakenLocation {
                path: location_path.to_string(),
                location_type: location_type.clone(),
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
