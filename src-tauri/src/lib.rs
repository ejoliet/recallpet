mod app_state;
mod clipboard;
mod commands;
mod database;
mod migrations;
mod models;
mod tray;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{Emitter, Manager};

use app_state::AppState;
use clipboard::{ClipboardBackend, ClipboardPoller, PollOutcome};
use database::{Database, UpsertOutcome};
use models::ClipboardItemSummary;

/// How often the clipboard is polled for a `changeCount` change.
const POLL_INTERVAL: Duration = Duration::from_millis(1000);
const DB_FILE_NAME: &str = "recallpet.db";

#[cfg(target_os = "macos")]
fn make_backend() -> Arc<dyn ClipboardBackend> {
    Arc::new(clipboard::macos::NsPasteboardBackend)
}

#[cfg(not(target_os = "macos"))]
fn make_backend() -> Arc<dyn ClipboardBackend> {
    Arc::new(clipboard::mock::MockClipboard::new())
}

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let shutdown = Arc::new(AtomicBool::new(false));
    let poll_thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>> = Arc::new(Mutex::new(None));

    let shutdown_for_setup = shutdown.clone();
    let poll_thread_for_setup = poll_thread.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_recent_clipboard_items,
            commands::search_clipboard_items,
            commands::set_collection_paused,
            commands::get_collection_status,
            commands::delete_all_clipboard_items,
            commands::copy_item_again,
            commands::get_storage_stats,
            commands::open_data_folder,
        ])
        .setup(move |app| {
            // Menu-bar utility: no Dock icon, no app-switcher entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let db_path = app.path().app_data_dir()?.join(DB_FILE_NAME);
            let db = match Database::open(&db_path) {
                Ok(db) => Some(Arc::new(db)),
                Err(err) => {
                    // AIDEV-NOTE: this is the "DB open/migration failure" path -
                    // the app still launches, but every command reports the
                    // storage error and the poll loop is never spawned.
                    log::error!("database unavailable at {db_path:?}: {err}");
                    None
                }
            };

            let backend = make_backend();
            let pending_self_write = Arc::new(Mutex::new(None));
            let state = AppState::new(db.clone(), backend.clone(), pending_self_write.clone());
            let paused = state.paused.clone();
            app.manage(state);

            tray::build(app.handle())?;
            tray::set_paused_visual(app.handle(), paused.load(Ordering::SeqCst));

            match db {
                Some(db) => {
                    let handle = spawn_poll_loop(
                        app.handle().clone(),
                        db,
                        backend,
                        pending_self_write,
                        paused,
                        shutdown_for_setup.clone(),
                    );
                    *poll_thread_for_setup.lock().unwrap() = Some(handle);
                }
                None => log::error!("clipboard collection disabled: no database"),
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the RecallPet application")
        .run(move |_app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                shutdown.store(true, Ordering::SeqCst);
                if let Some(handle) = poll_thread.lock().unwrap().take() {
                    let _ = handle.join();
                }
            }
        });
}

/// Background clipboard poller: one OS thread, woken once a second, doing
/// no work at all when `changeCount` hasn't moved. This is the only
/// recurring background work in the app.
fn spawn_poll_loop(
    app: tauri::AppHandle,
    db: Arc<Database>,
    backend: Arc<dyn ClipboardBackend>,
    pending_self_write: Arc<Mutex<Option<i64>>>,
    paused: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut poller = ClipboardPoller::new(backend, pending_self_write, paused.clone());

        while !shutdown.load(Ordering::SeqCst) {
            match poller.poll_once() {
                PollOutcome::Captured(raw_text) => {
                    handle_capture(&app, &db, &raw_text, paused.load(Ordering::SeqCst))
                }
                PollOutcome::Unchanged
                | PollOutcome::SelfWrite
                | PollOutcome::Paused
                | PollOutcome::NoText => {}
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    })
}

fn handle_capture(app: &tauri::AppHandle, db: &Arc<Database>, raw_text: &str, paused: bool) {
    let normalized = match models::prepare_capture(raw_text) {
        Ok(text) => text,
        Err(_) => return, // empty/whitespace-only or over 100,000 chars: silently ignored
    };

    match db.upsert_capture(&normalized, now_ms()) {
        Ok(UpsertOutcome::Created(item)) => {
            let _ = app.emit("clipboard:item-created", ClipboardItemSummary::from(&item));
            tray::signal_capture(app, paused);
        }
        Ok(UpsertOutcome::DuplicateUpdated(item)) => {
            let _ = app.emit("clipboard:item-updated", ClipboardItemSummary::from(&item));
            tray::signal_capture(app, paused);
        }
        Err(err) => log::error!("failed to store captured clipboard text: {err}"),
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
