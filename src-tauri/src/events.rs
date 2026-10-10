//! The events the backend sends every window when something changed under
//! them. The webview listens for these names, so they stay as they are.
//! Sending is best effort: a window that is gone misses nothing it needs.

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::plugins::PluginsView;

pub const NOTE_UPDATED: &str = "note-updated";
pub const INDEX_REBUILT: &str = "index-rebuilt";
pub const THREADS_CHANGED: &str = "threads-changed";
pub const VECTORS_CHANGED: &str = "vectors-changed";
pub const MAP_CHANGED: &str = "map-changed";
pub const PINS_CHANGED: &str = "pins-changed";
pub const PLUGINS_CHANGED: &str = "plugins-changed";
pub const EMBEDDING_STATUS: &str = "embedding-status";

fn emit(app: &AppHandle, event: &str, payload: impl Serialize + Clone) {
    let _ = app.emit(event, payload);
}

/// A note or a page was added, changed, moved or deleted.
pub fn note_updated(app: &AppHandle, id: &str) {
    emit(app, NOTE_UPDATED, json!({ "id": id }));
}

/// The index changed under the views, as a rebuild or an edit made outside
/// the app changes it.
pub fn index_rebuilt(app: &AppHandle) {
    emit(app, INDEX_REBUILT, ());
}

pub fn threads_changed(app: &AppHandle, space: &str) {
    emit(app, THREADS_CHANGED, json!({ "space": space }));
}

/// Notes of `space` were embedded, or left its vectors.
pub fn vectors_changed(app: &AppHandle, space: &str) {
    emit(app, VECTORS_CHANGED, json!({ "space": space }));
}

pub fn map_changed(app: &AppHandle, space: &str) {
    emit(app, MAP_CHANGED, json!({ "space": space }));
}

pub fn pins_changed(app: &AppHandle, space: &str) {
    emit(app, PINS_CHANGED, json!({ "space": space }));
}

/// What is installed and on now, so each window loads and unloads plugins
/// to match.
pub fn plugins_changed(app: &AppHandle, view: &PluginsView) {
    emit(app, PLUGINS_CHANGED, view);
}

/// Where the embedding model's download is: `downloading` with how far,
/// then `installed` or `absent`.
pub fn embedding_status(app: &AppHandle, state: &str, percent: Option<u8>) {
    let status = match percent {
        Some(percent) => json!({ "state": state, "percent": percent }),
        None => json!({ "state": state }),
    };
    emit(app, EMBEDDING_STATUS, status);
}
