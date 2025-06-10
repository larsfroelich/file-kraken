use crate::state::AppState;
use crate::utils::dialogs::error_dialog;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub fn lock_rw_write_or_exit<'a, T>(
    lock: &'a RwLock<T>,
    app_state: &AppState,
) -> Option<RwLockWriteGuard<'a, T>> {
    match lock.write() {
        Ok(guard) => Some(guard),
        Err(_) => {
            error_dialog("Internal error: lock poisoned. Closing project.");
            app_state.close_project();
            None
        }
    }
}

pub fn lock_rw_read_or_exit<'a, T>(
    lock: &'a RwLock<T>,
    app_state: &AppState,
) -> Option<RwLockReadGuard<'a, T>> {
    match lock.read() {
        Ok(guard) => Some(guard),
        Err(_) => {
            error_dialog("Internal error: lock poisoned. Closing project.");
            app_state.close_project();
            None
        }
    }
}
