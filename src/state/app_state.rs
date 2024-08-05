use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard};
use crate::state::file::FileKrakenFile;
use crate::state::location::{FileKrakenLocation, FileKrakenLocationType};

#[derive(Default)]
pub struct AppState {
    sqlite : Option<rusqlite::Connection>,
    locations_list : Arc<RwLock<Vec<FileKrakenLocation>>>,
    files_by_location_by_path : Arc<RwLock<HashMap<String, Arc<RwLock<HashMap<String, FileKrakenFile>>>>>>,
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
            let mut select_locations = connection
                .prepare("SELECT path, location_type FROM locations")?;
            let mut locations_query = select_locations.query([])?;

            while let Some(row) = locations_query.next()? {
                let location = FileKrakenLocation {
                    path: row.get(0)?,
                    location_type: match row.get::<usize, String>(1)?.as_str() {
                        "normal" => FileKrakenLocationType::Normal,
                        "preferred" => FileKrakenLocationType::Preferred,
                        "excluded" => FileKrakenLocationType::Excluded,
                        _ => FileKrakenLocationType::Normal,
                    }
                }.clone();
                self.files_by_location_by_path.write().unwrap().insert(
                    location.path.clone(),
                    Arc::new(RwLock::new(HashMap::new())),
                );
                self.locations_list.write().unwrap().push(location );
            }
        }

        self.sqlite = Some(connection);
        Ok(())
    }

    pub fn is_sqlite_connected(&self) -> bool {
        self.sqlite.is_some()
    }

    pub fn get_locations_list_readonly(&self) -> RwLockReadGuard<'_, Vec<FileKrakenLocation>> {
        self.locations_list.read().unwrap()
    }
    
    pub fn modify_location_type(&self, location_path: &str, location_type: FileKrakenLocationType) {
        // do nothing if type is the same
        let current_location_type = self.locations_list.read().unwrap().iter()
            .find(|x| x.path == location_path)
            .map(|x| x.location_type.clone())
            .unwrap();
        if current_location_type == location_type {
            return;
        }
        
        self.sqlite.as_ref().unwrap().execute(
            "UPDATE locations SET location_type = ? WHERE path = ?;",
            [&location_type.to_string(), location_path],
        ).unwrap();
        let mut locations_list = self.locations_list.write().unwrap();
        for location in locations_list.iter_mut() {
            if location.path == location_path {
                location.location_type = location_type;
                break;
            }
        }
    }
    
    pub fn add_location(&self, location: FileKrakenLocation) {
        self.sqlite.as_ref().unwrap().execute(
            "INSERT INTO locations (path, location_type) VALUES (?, ?);",
            [&location.path, &location.location_type.to_string()],
        ).unwrap();
        self.files_by_location_by_path.write().unwrap().insert(
            location.path.clone(),
            Arc::new(RwLock::new(HashMap::new())),
        );
        self.locations_list.write().unwrap().push(location);
    }

    pub fn get_files_by_location(&self, location: &str)
        -> Option<Arc<RwLock<HashMap<String, FileKrakenFile>>>> {
        self.files_by_location_by_path
            // get readonly access to the by-location hashmap
            .read().unwrap()

            // look for the by-filepath hashmap for the given location
            .get(location)

            // clone the Arc reference of the Hashmap if it exists
            .map(|x| x.clone())
    }
}

