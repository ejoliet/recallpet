//! Shared state handed to every Tauri command: the database handle, the
//! paused flag (mirrored from `app_settings` at startup), and the clipboard
//! write controller used by `copy_item_again`.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crate::clipboard::{ClipboardBackend, ClipboardController};
use crate::database::Database;

/// `db` is `None` when opening/migrating the database failed at startup.
/// Every command degrades to a structured error in that case instead of
/// panicking, and the clipboard poll loop is never spawned.
pub struct AppState {
    pub db: Option<Arc<Database>>,
    pub paused: Arc<AtomicBool>,
    pub clipboard: ClipboardController,
}

impl AppState {
    pub fn new(
        db: Option<Arc<Database>>,
        backend: Arc<dyn ClipboardBackend>,
        pending_self_write: Arc<Mutex<Option<i64>>>,
    ) -> Self {
        let initially_paused = db
            .as_ref()
            .map(|db| db.is_paused().unwrap_or(false))
            .unwrap_or(true);
        AppState {
            db,
            paused: Arc::new(AtomicBool::new(initially_paused)),
            clipboard: ClipboardController::new(backend, pending_self_write),
        }
    }
}
