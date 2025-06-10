use crate::state::file::FileKrakenFileType;
use crate::state::location::FileKrakenLocationState;
use crate::state::AppState;
use crate::utils::dialogs::error_dialog;
use jwalk::WalkDir;
use log::error;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

pub fn scan_location_files(app_state: Arc<AppState>, location_path: &str) {
    let current_state = match app_state.get_location_clone(location_path) {
        Some(loc) => loc.location_state,
        None => {
            error_dialog(&format!("Location {location_path} not found"));
            return;
        }
    };
    if current_state == FileKrakenLocationState::Scanning {
        return error_dialog("Already scanning this location");
    }
    app_state.modify_location_state(true, location_path, FileKrakenLocationState::Scanning);

    let mut failed_paths = Vec::new();
    for entry in WalkDir::new(location_path) {
        if let Ok(entry) = entry {
            if entry.file_type.is_file() {
                let file_type = if let Some(file_extension) =
                    entry.path().extension().and_then(|x| x.to_str())
                {
                    if [".tar.xz", ".zip", ".7z"].contains(&file_extension) {
                        FileKrakenFileType::Archive
                    } else {
                        FileKrakenFileType::Normal
                    }
                } else {
                    FileKrakenFileType::Normal
                };
                let file_metadata = match entry.metadata() {
                    Ok(meta) => meta,
                    Err(err) => {
                        error!(
                            "Failed to get file metadata for file {:?}: {}",
                            entry.path(),
                            err
                        );
                        continue;
                    }
                };

                if let Some(file_path) = entry.path().to_str() {
                    app_state.add_file(
                        true,
                        file_path,
                        &file_type,
                        file_metadata.len(),
                        file_metadata
                            .created()
                            .ok()
                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                            .unwrap_or_default()
                            .as_secs(),
                        file_metadata
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                            .unwrap_or_default()
                            .as_secs(),
                        None,
                    );
                } else {
                    error!(
                        "Failed to get file path for file {:?}",
                        entry.path().to_string_lossy().as_ref()
                    );
                    failed_paths.push(entry.path().to_string_lossy().to_string());
                }
            }
        }
    }

    if !failed_paths.is_empty() {
        if failed_paths.len() > 10 {
            error_dialog(&format!(
                "Failed to get file path for {} files. Example: {}",
                failed_paths.len(),
                failed_paths[0]
            ));
        } else {
            for failed_path in failed_paths {
                error_dialog(&format!("Failed to get file path for file {}", failed_path));
            }
        }
    }

    // check if files were removed
    let files: Vec<String> = {
        if let Some(sqlite_lock) = app_state.sqlite_lock_or_close() {
            if let Some(conn) = sqlite_lock.as_ref() {
                if let Ok(mut files_query) =
                    conn.prepare("SELECT path FROM files WHERE location_path = ?")
                {
                    if let Ok(rows) = files_query.query_map(&[&location_path], |row| row.get(0)) {
                        rows.filter_map(|x| x.ok()).collect()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    };
    for file in files {
        // check filesystem
        if !std::path::Path::new(&file).exists() {
            app_state.remove_file(true, false, &file);
        }
    }

    app_state.modify_location_state(true, location_path, FileKrakenLocationState::Scanned);
}
