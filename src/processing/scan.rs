use crate::state::file::FileKrakenFileType;
use crate::state::location::FileKrakenLocationState;
use crate::state::AppState;
use crate::utils::dialogs::error_dialog;
use jwalk::WalkDir;
use log::error;
use std::borrow::Cow;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

pub fn scan_location_files(app_state: Arc<AppState>, location_path: &str) {
    let current_state = app_state
        .get_location_clone(location_path)
        .unwrap()
        .location_state;
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
                let file_metadata = entry.metadata().expect(&format!(
                    "Failed to get file metadata for file {:?}",
                    entry.path()
                ));

                if let Some(file_path) = entry.path().to_str() {
                    app_state.add_file(
                        true,
                        file_path,
                        &file_type,
                        file_metadata.len(),
                        file_metadata
                            .created()
                            .unwrap()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or(std::time::Duration::new(0, 0))
                            .as_secs(),
                        file_metadata
                            .modified()
                            .unwrap()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or(std::time::Duration::new(0, 0))
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
        let sqlite_lock = app_state.sqlite.lock().unwrap();
        let mut files_query = sqlite_lock
            .as_ref()
            .unwrap()
            .prepare("SELECT path FROM files WHERE location_path = ?")
            .unwrap();
        files_query
            .query_map(&[&location_path], |row| row.get(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    for file in files {
        // check filesystem
        if !std::path::Path::new(&file).exists() {
            app_state.remove_file(true, false, &file);
        }
    }

    app_state.modify_location_state(true, location_path, FileKrakenLocationState::Scanned);
}
