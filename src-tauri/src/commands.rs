//! Tauri command API. Every argument is validated here; every fallible
//! command returns a small structured `CommandError` (never a raw Rust
//! error string) so the frontend gets a stable shape to render.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::app_state::AppState;
use crate::database::{Database, DbError};
use crate::models::{ClipboardItemSummary, CollectionStatus, StorageStats};
use crate::tray;

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 500;
const MAX_QUERY_CHARS: usize = 500;

#[derive(Debug, Serialize)]
pub struct CommandError {
    pub message: String,
}

impl CommandError {
    fn new(message: impl Into<String>) -> Self {
        CommandError {
            message: message.into(),
        }
    }
}

impl From<DbError> for CommandError {
    fn from(err: DbError) -> Self {
        log::error!("command failed: {err}");
        CommandError::new("A local storage error occurred.")
    }
}

fn clamp_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

/// Every command goes through this instead of touching `state.db` directly,
/// so a failed-to-open database degrades to one consistent structured error
/// everywhere instead of a panic.
fn require_db(state: &AppState) -> Result<&Arc<Database>, CommandError> {
    state.db.as_ref().ok_or_else(|| {
        CommandError::new("Local storage is unavailable. Clipboard collection is disabled.")
    })
}

fn to_summaries(items: Vec<crate::models::ClipboardItem>) -> Vec<ClipboardItemSummary> {
    items.iter().map(ClipboardItemSummary::from).collect()
}

#[tauri::command]
pub fn get_recent_clipboard_items(
    state: State<AppState>,
    limit: Option<i64>,
) -> Result<Vec<ClipboardItemSummary>, CommandError> {
    let limit = clamp_limit(limit);
    Ok(to_summaries(require_db(&state)?.recent_items(limit)?))
}

#[tauri::command]
pub fn search_clipboard_items(
    state: State<AppState>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<ClipboardItemSummary>, CommandError> {
    let limit = clamp_limit(limit);
    let query = query.trim();
    let db = require_db(&state)?;

    if query.is_empty() {
        return Ok(to_summaries(db.recent_items(limit)?));
    }
    if query.chars().count() > MAX_QUERY_CHARS {
        return Err(CommandError::new("Search query is too long."));
    }

    Ok(to_summaries(db.search_items(query, limit)?))
}

#[tauri::command]
pub fn set_collection_paused(
    app: AppHandle,
    state: State<AppState>,
    paused: bool,
) -> Result<(), CommandError> {
    require_db(&state)?.set_paused(paused)?;
    state.paused.store(paused, Ordering::SeqCst);
    tray::set_paused_visual(&app, paused);

    let event_name = if paused {
        "collector:paused"
    } else {
        "collector:resumed"
    };
    let _ = app.emit(event_name, ());
    Ok(())
}

#[tauri::command]
pub fn get_collection_status(state: State<AppState>) -> CollectionStatus {
    CollectionStatus {
        paused: state.paused.load(Ordering::SeqCst),
    }
}

#[tauri::command]
pub fn delete_all_clipboard_items(
    app: AppHandle,
    state: State<AppState>,
) -> Result<(), CommandError> {
    require_db(&state)?.delete_all()?;
    let _ = app.emit("storage:cleared", ());
    Ok(())
}

#[tauri::command]
pub fn copy_item_again(state: State<AppState>, id: i64) -> Result<(), CommandError> {
    if id <= 0 {
        return Err(CommandError::new("Invalid item id."));
    }
    let text = require_db(&state)?
        .item_text(id)?
        .ok_or_else(|| CommandError::new("Item not found."))?;
    state.clipboard.copy_text(&text);
    Ok(())
}

#[tauri::command]
pub fn get_storage_stats(state: State<AppState>) -> Result<StorageStats, CommandError> {
    Ok(require_db(&state)?.storage_stats()?)
}

#[tauri::command]
pub fn open_data_folder(app: AppHandle) -> Result<(), CommandError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| CommandError::new("Could not resolve the data folder."))?;
    app.opener().reveal_item_in_dir(&dir).map_err(|err| {
        log::error!("failed to reveal data folder: {err}");
        CommandError::new("Could not open the data folder.")
    })
}
