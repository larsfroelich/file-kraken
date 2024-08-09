use crate::state::file::FileKrakenFileType;
use crate::state::location::FileKrakenLocationState;
use crate::state::AppState;
use crate::utils::dialogs::error_dialog;
use jwalk::WalkDir;
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

    for entry in WalkDir::new(location_path).sort(true) {
        if let Ok(entry) = entry {
            if entry.file_type.is_file() {
                let file_type = if let Some(file_extension) = entry.path().extension() {
                    if [".tar.xz", ".zip", ".7z"].contains(&file_extension.to_str().unwrap()) {
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

                app_state.add_file(
                    true,
                    entry.path().to_str().unwrap(),
                    &file_type,
                    file_metadata.len(),
                    file_metadata
                        .created()
                        .unwrap()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    file_metadata
                        .modified()
                        .unwrap()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    None,
                );
            }
        }
    }
    app_state.modify_location_state(true, location_path, FileKrakenLocationState::Scanned);
}
