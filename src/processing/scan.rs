use crate::state::AppState;
use jwalk::WalkDir;
use std::sync::{Arc, RwLock};

pub fn scan_location_files(app_state: Arc<RwLock<AppState>>, location_path: &str) {
    let location_to_scan = app_state
        .read()
        .unwrap()
        .get_location_clone(location_path)
        .unwrap();

    for entry in WalkDir::new(location_to_scan.path).sort(true) {
        if let Ok(entry) = entry {
            if entry.file_type.is_file() {
                app_state.write().unwrap().add_file_to_location(
                    true,
                    location_path,
                    entry.path().to_str().unwrap(),
                );
            }
        }
    }
}
