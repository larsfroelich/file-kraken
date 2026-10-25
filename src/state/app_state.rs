use crate::processing::find_duplicates::{FindDuplicatesState, FindDuplicatesStateType};
use crate::state::file::{FileKrakenFile, FileKrakenFileType};
use crate::state::location::{FileKrakenLocation, FileKrakenLocationState, FileKrakenLocationType};
use crate::utils::dialogs::error_dialog;
use crate::utils::hashing::hash_file;
use crate::utils::parent_path::get_longest_parent_path;
use rusqlite::OptionalExtension;
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
        // Create schema objects
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
        connection.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
            [],
        )?;
        // Normalize legacy placeholder values to real SQL NULL
        connection.execute(
            "UPDATE files SET hash_256 = NULL WHERE hash_256 = 'NULL';",
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
                let hash: Option<String> = row.get(5)?;

                self.add_file(
                    false,
                    &file_path,
                    &file_type,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    hash,
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

        // Load settings
        if let Some(min_size) = self.get_setting("min_file_size_input") {
            *self
                .find_duplicates_processing
                .min_file_size_input
                .write()
                .unwrap() = min_size;
        }
        if let Some(min_unit) = self.get_setting("min_file_size_unit") {
            if let Ok(unit) = min_unit.parse() {
                *self
                    .find_duplicates_processing
                    .min_file_size_unit
                    .write()
                    .unwrap() = unit;
            }
        }
        if let Some(include_same) = self.get_setting("include_same_location_duplicates") {
            if let Ok(val) = include_same.parse() {
                *self
                    .find_duplicates_processing
                    .include_same_location_duplicates
                    .write()
                    .unwrap() = val;
            }
        }
        if let Some(scan_zips) = self.get_setting("scan_inside_zip_files") {
            if let Ok(val) = scan_zips.parse() {
                *self
                    .find_duplicates_processing
                    .scan_inside_zip_files
                    .write()
                    .unwrap() = val;
            }
        }

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
        // Check if this file has an existing hash in DB
        let hash_result = self.with_sqlite_conn(|conn| {
            conn.prepare_cached("SELECT hash_256 FROM files WHERE path = ?1;")?
                .query_row([file_path], |row| row.get::<_, Option<String>>(0))
                .optional()
        });

        if let Some(Some(Some(existing_hash))) = hash_result {
            return Some(existing_hash);
        }

        // Compute hash only when DB has SQL NULL
        match hash_file(file_path) {
            Ok(hash) => {
                let _ = self.with_sqlite_conn(|conn| {
                    conn.execute(
                        "UPDATE files SET hash_256 = ?1 WHERE path = ?2;",
                        rusqlite::params![hash, file_path],
                    )
                });
                Some(hash)
            }
            Err(err) => {
                error_dialog(&format!("Failed to hash file {file_path}: {err}"));
                None
            }
        }
    }

    pub fn is_sqlite_connected(&self) -> bool {
        self.sqlite.lock().unwrap().is_some()
    }

    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.with_sqlite_conn(|conn| {
            conn.prepare_cached("SELECT value FROM settings WHERE key = ?1;")?
                .query_row([key], |row| row.get(0))
                .optional()
        })
        .flatten()
    }

    pub fn set_setting(&self, key: &str, value: &str) {
        let res = self.with_sqlite_conn(|conn| {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value;",
                rusqlite::params![key, value],
            )
        });
        if res.is_none() {
            // with_sqlite_conn already shows an error dialog and closes the project
            // but we panic here to make sure we don't continue in a broken state if called from a background thread
            // that doesn't respect the project closing.
            log::error!("Failed to persist setting {}={}", key, value);
            panic!("Failed to persist setting {}={}", key, value);
        }
    }

    pub fn remove_location(&self, persist_to_db: bool, location_path: &str) {
        // mark location as deleting before removing any related state
        self.locations_list
            .write()
            .unwrap()
            .iter_mut()
            .find(|x| x.path == location_path)
            .unwrap_or_else(|| panic!("location {} not found", location_path))
            .location_state = FileKrakenLocationState::Deleting;

        // remove dependent files first
        self.clear_location_files(persist_to_db, location_path);

        // remove location row from sqlite when persistence is enabled
        if persist_to_db {
            self.sqlite
                .lock()
                .unwrap()
                .as_ref()
                .expect("Sql connection not set")
                .execute("DELETE FROM locations WHERE path = ?1;", [location_path])
                .unwrap();
        }

        // clear in-memory file index for this location
        self.files_by_location_by_path
            .write()
            .unwrap()
            .remove(location_path);

        // remove location from in-memory list
        let mut locations_list = self.locations_list.write().unwrap();
        locations_list.retain(|x| x.path != location_path);
    }

    pub fn remove_file(&self, persist_to_db: bool, persist_to_disk: bool, file_path: &str) {
        if persist_to_disk {
            if let Some((zip_path, inner_rest)) = file_path.split_once("::") {
                // Format is zip_path::index::name
                let (inner_idx_str, _) = inner_rest.split_once("::").unwrap_or(("0", inner_rest));
                let inner_idx: usize = inner_idx_str.parse().unwrap_or(0);

                // Generate a unique temp path to prevent race conditions or collisions
                let unique_id = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let temp_zip_path = format!("{}.tmp.{}", zip_path, unique_id);

                let success = (|| -> Result<(), Box<dyn std::error::Error>> {
                    let file = std::fs::File::open(zip_path)?;
                    let mut archive = zip::ZipArchive::new(file)?;
                    let mut temp_zip = zip::ZipWriter::new(std::fs::File::create(&temp_zip_path)?);

                    for i in 0..archive.len() {
                        let inner_file = archive.by_index(i)?;
                        // Use raw copy to preserve compression and metadata (like symlinks)
                        if inner_file.name_raw().is_empty() {
                            continue;
                        }

                        if i != inner_idx {
                            temp_zip.raw_copy_file(inner_file)?;
                        }
                    }
                    temp_zip.finish()?;
                    Ok(())
                })();

                if success.is_ok() {
                    if std::fs::rename(&temp_zip_path, zip_path).is_err() {
                        let _ = std::fs::remove_file(&temp_zip_path);
                        rfd::MessageDialog::new()
                            .set_title("Error")
                            .set_description("Failed to replace zip archive")
                            .show();
                        return;
                    }

                    let zip_parent_location_path =
                        crate::utils::parent_path::get_longest_parent_path(
                            zip_path,
                            self.get_locations_list_readonly().iter(),
                        );

                    // Stale member cleanup: After rewriting a ZIP, the internal indexes are now compacted.
                    // We must remove the existing tracking of all inner files from memory and DB to avoid
                    // subsequent deletions accidentally targeting the wrong file due to a shifted index.
                    let mut inner_paths_to_remove = Vec::new();
                    let prefix = format!("{}::", zip_path);
                    if let Some(parent_path) = &zip_parent_location_path {
                        if let Some(location_files) = self
                            .files_by_location_by_path
                            .read()
                            .unwrap()
                            .get(parent_path)
                        {
                            for path in location_files.read().unwrap().keys() {
                                if path.starts_with(&prefix) {
                                    inner_paths_to_remove.push(path.clone());
                                }
                            }
                        }
                    }

                    // Actually perform the removal of these paths
                    self.remove_files(persist_to_db, &inner_paths_to_remove);

                    // Re-read and re-add internal files using the new indexes so they are stable
                    if let Ok(fs_file) = std::fs::File::open(zip_path) {
                        if let Ok(mut archive) = zip::ZipArchive::new(fs_file) {
                            let mut batch = Vec::new();
                            for i in 0..archive.len() {
                                if let Ok(inner_file) = archive.by_index(i) {
                                    if !inner_file.is_dir() {
                                        if let Ok(inner_name) = inner_file.name() {
                                            if let Some(name) = inner_name.split('/').next_back() {
                                                if !name.is_empty() {
                                                    let inner_path = format!(
                                                        "{}::{}::{}",
                                                        zip_path, i, inner_name
                                                    );
                                                    let is_nested_zip =
                                                        name.to_lowercase().ends_with(".zip");
                                                    let inner_file_type = if is_nested_zip {
                                                        crate::state::file::FileKrakenFileType::Archive
                                                    } else {
                                                        crate::state::file::FileKrakenFileType::Normal
                                                    };

                                                    // Hash inner file right away to maintain integrity
                                                    let inner_hash =
                                                        crate::utils::hashing::hash_file(
                                                            &inner_path,
                                                        )
                                                        .ok();

                                                    batch.push(
                                                        crate::state::file::FileKrakenFile {
                                                            path: inner_path,
                                                            file_type: inner_file_type,
                                                            file_len: inner_file.size(),
                                                            time_created: 0, // In practice, inherits from zip or leaves default
                                                            time_modified: 0,
                                                            hash: inner_hash,
                                                        },
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // Re-add to DB
                            if let Some(parent_path) = &zip_parent_location_path {
                                self.add_files_to_location(persist_to_db, parent_path, batch);
                            }
                        }
                    }

                    // Stale member cleanup: After rewriting a ZIP, the internal indexes are now compacted.
                    // We must remove the existing tracking of all inner files from memory and DB to avoid
                    // subsequent deletions accidentally targeting the wrong file due to a shifted index.
                    let mut inner_paths_to_remove = Vec::new();
                    let prefix = format!("{}::", zip_path);
                    if let Some(parent_path) = &zip_parent_location_path {
                        if let Some(location_files) = self
                            .files_by_location_by_path
                            .read()
                            .unwrap()
                            .get(parent_path)
                        {
                            for path in location_files.read().unwrap().keys() {
                                if path.starts_with(&prefix) {
                                    inner_paths_to_remove.push(path.clone());
                                }
                            }
                        }
                    }

                    // Actually perform the removal of these paths
                    self.remove_files(persist_to_db, &inner_paths_to_remove);

                    // Re-read and re-add internal files using the new indexes so they are stable
                    if let Ok(fs_file) = std::fs::File::open(zip_path) {
                        if let Ok(mut archive) = zip::ZipArchive::new(fs_file) {
                            let mut batch = Vec::new();
                            for i in 0..archive.len() {
                                if let Ok(inner_file) = archive.by_index(i) {
                                    if !inner_file.is_dir() {
                                        if let Ok(inner_name) = inner_file.name() {
                                            if let Some(name) = inner_name.split('/').next_back() {
                                                if !name.is_empty() {
                                                    let inner_path = format!(
                                                        "{}::{}::{}",
                                                        zip_path, i, inner_name
                                                    );
                                                    let is_nested_zip =
                                                        name.to_lowercase().ends_with(".zip");
                                                    let inner_file_type = if is_nested_zip {
                                                        crate::state::file::FileKrakenFileType::Archive
                                                    } else {
                                                        crate::state::file::FileKrakenFileType::Normal
                                                    };

                                                    // Hash inner file right away to maintain integrity
                                                    let inner_hash =
                                                        crate::utils::hashing::hash_file(
                                                            &inner_path,
                                                        )
                                                        .ok();

                                                    batch.push(
                                                        crate::state::file::FileKrakenFile {
                                                            path: inner_path,
                                                            file_type: inner_file_type,
                                                            file_len: inner_file.size(),
                                                            time_created: 0, // In practice, inherits from zip or leaves default
                                                            time_modified: 0,
                                                            hash: inner_hash,
                                                        },
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // Re-add to DB
                            if let Some(parent_path) = &zip_parent_location_path {
                                self.add_files_to_location(persist_to_db, parent_path, batch);
                            }
                        }
                    }

                    // Update in-memory state for the zip file BEFORE we attempt DB writes
                    if let Ok(metadata) = std::fs::metadata(zip_path) {
                        let new_len = metadata.len();
                        let new_hash = crate::utils::hashing::hash_file(zip_path).ok();

                        if let Some(parent_path) = zip_parent_location_path {
                            if let Some(location_files) = self
                                .files_by_location_by_path
                                .read()
                                .unwrap()
                                .get(&parent_path)
                            {
                                if let Some(zip_file) =
                                    location_files.write().unwrap().get_mut(zip_path)
                                {
                                    zip_file.file_len = new_len;
                                    zip_file.hash = new_hash.clone();
                                }
                            }
                        }

                        let _ = self.with_sqlite_conn(|conn| {
                            conn.execute(
                                "UPDATE files SET file_len = ?1, hash_256 = ?2 WHERE path = ?3",
                                rusqlite::params![new_len, new_hash, zip_path],
                            )
                        });
                    }
                } else {
                    let _ = std::fs::remove_file(&temp_zip_path);
                    rfd::MessageDialog::new()
                        .set_title("Error")
                        .set_description("Failed to delete file from zip archive")
                        .show();
                    return;
                }
            } else {
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

        // Skip final generic memory removal for zip archives, because the zip branch above
        // already re-added the surviving members. If a surviving member has the exact same name,
        // it would get erroneously deleted by this generic trailing cleanup.
        let is_zip_member = file_path.contains("::");
        if !is_zip_member {
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

    /// Adds a single file to a specific location and persists it to the database if required.
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
        self.add_files_to_location(
            persist_to_db,
            location_path,
            vec![FileKrakenFile {
                path: file_path.to_string(),
                file_type: file_type.clone(),
                file_len,
                time_created,
                time_modified,
                hash,
            }],
        );
    }

    /// Adds multiple files to a location in bulk, using a single transaction for persistence.
    pub fn add_files_to_location(
        &self,
        persist_to_db: bool,
        location_path: &str,
        files: Vec<FileKrakenFile>,
    ) {
        if files.is_empty() {
            return;
        }

        // --- SQL: Persist in bulk via transaction ---
        if persist_to_db {
            let res = self.sqlite_lock_or_exit().and_then(|mut guard| {
                let conn = guard.as_mut().unwrap();
                let tx = conn.transaction().ok()?;
                {
                    let mut stmt = tx
                        .prepare_cached(
                            "INSERT INTO files (
                                path,
                                location_path,
                                file_type,
                                file_len,
                                time_created,
                                time_modified,
                                hash_256
                            ) VALUES (?, ?, ?, ?, ?, ?, ?)
                            ON CONFLICT(path) DO UPDATE SET
                                location_path = excluded.location_path,
                                file_type = excluded.file_type,
                                file_len = excluded.file_len,
                                time_created = excluded.time_created,
                                time_modified = excluded.time_modified,
                                hash_256 = NULL;",
                        )
                        .ok()?;

                    for file in &files {
                        stmt.execute(rusqlite::params![
                            file.path,
                            location_path,
                            file.file_type.as_str(),
                            file.file_len,
                            file.time_created,
                            file.time_modified,
                            file.hash,
                        ])
                        .ok()?;
                    }
                }
                tx.commit().ok()
            });
            if res.is_none() {
                error_dialog("Failed to save files to database. Closing project.");
                self.close_project();
                return;
            }
        }

        // --- STATE: Update in-memory location metadata ---
        let location_state = self
            .get_location_clone(location_path)
            .map(|l| l.location_state);
        if location_state == Some(FileKrakenLocationState::Unscanned) {
            self.modify_location_state(
                persist_to_db,
                location_path,
                FileKrakenLocationState::PartialScanned,
            );
        }

        // --- STATE: Update in-memory file indexes ---
        let locations = self.get_locations_list_readonly();
        let files_by_location = self.files_by_location_by_path.read().unwrap();
        let location_files = files_by_location.get(location_path).cloned();

        if let Some(location_files) = location_files {
            let mut writer = location_files.write().unwrap();
            for file in files {
                // Ensure consistency by removing file from old location index if it changed
                if let Some(old_loc_path) = get_longest_parent_path(&file.path, locations.iter()) {
                    if old_loc_path != location_path {
                        if let Some(old_loc_files) = files_by_location.get(&old_loc_path) {
                            old_loc_files.write().unwrap().remove(&file.path);
                        }
                    }
                }
                writer.insert(file.path.clone(), file);
            }
        }
    }

    /// Removes multiple files from the database and memory in bulk.
    pub fn remove_files(&self, persist_to_db: bool, file_paths: &[String]) {
        if file_paths.is_empty() {
            return;
        }

        // --- SQL: Remove in bulk via transaction ---
        if persist_to_db {
            let res = self.sqlite_lock_or_exit().and_then(|mut guard| {
                let conn = guard.as_mut().unwrap();
                let tx = conn.transaction().ok()?;
                {
                    let mut stmt = tx
                        .prepare_cached("DELETE FROM files WHERE path = ?1")
                        .ok()?;
                    for path in file_paths {
                        stmt.execute([path]).ok()?;
                    }
                }
                tx.commit().ok()
            });
            if res.is_none() {
                error_dialog("Failed to remove files from database. Closing project.");
                self.close_project();
                return;
            }
        }

        // --- STATE: Update in-memory file indexes ---
        let locations = self.get_locations_list_readonly();
        let files_by_location = self.files_by_location_by_path.read().unwrap();

        for file_path in file_paths {
            if let Some(parent_location_path) = get_longest_parent_path(file_path, locations.iter())
            {
                if let Some(location_files) = files_by_location.get(&parent_location_path) {
                    location_files.write().unwrap().remove(file_path);
                }
            }
        }
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

    #[cfg(test)]
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

#[test]
fn add_file_to_location_persists_archive_file_type() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sqlite_path = temp_dir.path().join("state.sqlite");
    let sqlite_path_str = sqlite_path.to_str().unwrap();

    let state = AppState::default();
    state.connect_sqlite(sqlite_path_str).unwrap();
    state.add_location(
        true,
        "/test/location",
        &FileKrakenLocationType::Normal,
        &FileKrakenLocationState::Scanned,
    );
    state.add_file_to_location(
        true,
        "/test/location",
        "/test/location/archive.zip",
        &FileKrakenFileType::Archive,
        10,
        20,
        30,
        None,
    );
    state.close_project();

    let reloaded_state = AppState::default();
    reloaded_state.connect_sqlite(sqlite_path_str).unwrap();

    let files = reloaded_state
        .get_files_by_location("/test/location")
        .unwrap();
    let loaded_file = files
        .read()
        .unwrap()
        .get("/test/location/archive.zip")
        .unwrap()
        .clone();

    assert_eq!(loaded_file.file_type, FileKrakenFileType::Archive);
}

#[cfg(test)]
fn temp_project_path() -> std::path::PathBuf {
    let file = tempfile::Builder::new()
        .prefix("file-kraken-remove-location")
        .suffix(".fkproj")
        .tempfile()
        .unwrap();
    file.into_temp_path().keep().unwrap()
}

#[test]
fn remove_location_persisted_does_not_reload_from_same_project_file() {
    let project_path = temp_project_path();
    let project_path_str = project_path.to_str().unwrap();

    // create a project with one location and one file
    let app_state = AppState::default();
    app_state.connect_sqlite(project_path_str).unwrap();
    app_state.add_location(
        true,
        "/tmp/remove-me",
        &FileKrakenLocationType::Normal,
        &FileKrakenLocationState::Unscanned,
    );
    app_state.add_file_to_location(
        true,
        "/tmp/remove-me",
        "/tmp/remove-me/file.txt",
        &FileKrakenFileType::Normal,
        10,
        100,
        100,
        None,
    );
    app_state.remove_location(true, "/tmp/remove-me");
    app_state.close_project();

    // reconnect to same project and ensure location is gone
    let reloaded = AppState::default();
    reloaded.connect_sqlite(project_path_str).unwrap();
    assert!(reloaded.get_locations_list_readonly().is_empty());
    assert_eq!(reloaded.get_total_files_count(), 0);

    std::fs::remove_file(project_path).unwrap();
}
