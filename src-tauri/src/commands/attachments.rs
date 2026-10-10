//! Attachments, SPEC 3.7 and 4.8: files dropped, picked or pasted into an
//! editor, moving them with a draft sent to another space, and opening
//! them. Where they go is `crate::attachments`.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use chrono::Local;
use percent_encoding::percent_decode_str;
use tauri::ipc::InvokeBody;
use tauri::{AppHandle, State};

use crate::attachments::{relocate, resolve, store, Attachment};
use crate::state::AppState;

fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// Copy files from disk into the open space, or the one named: dropped on an
/// editor, or picked with its attach button. Every path is checked before
/// anything is copied, so a folder among them copies nothing.
#[tauri::command]
pub async fn add_attachments(
    state: State<'_, AppState>,
    paths: Vec<String>,
    space: Option<String>,
) -> Result<Vec<Attachment>, String> {
    let root = state.space_or_open(space.as_deref())?.root.clone();
    tauri::async_runtime::spawn_blocking(move || {
        for path in &paths {
            match fs::metadata(path) {
                Ok(meta) if meta.is_file() => {}
                Ok(_) => return Err(format!("{path} is not a file")),
                Err(e) => return Err(format!("could not attach {path}: {e}")),
            }
        }
        let date = today();
        paths
            .iter()
            .map(|path| {
                let source = Path::new(path);
                let name = source
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                File::open(source)
                    .and_then(|mut from| {
                        store(&root, &date, &name, |to| {
                            io::copy(&mut from, to).map(|_| ())
                        })
                    })
                    .map_err(|e| format!("could not attach {path}: {e}"))
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A URI-encoded header of the request, decoded.
fn header(request: &tauri::ipc::Request<'_>, name: &str) -> Option<String> {
    request
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(|value| percent_decode_str(value).decode_utf8_lossy().into_owned())
}

/// Save a file pasted into an editor, a screenshot say, into the open space,
/// or the one its `x-space` header names. The request's body is the file;
/// its `x-name` header is the name it came with. Both headers are
/// URI-encoded.
#[tauri::command]
pub async fn save_attachment(
    state: State<'_, AppState>,
    request: tauri::ipc::Request<'_>,
) -> Result<Attachment, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected the file's bytes".to_string());
    };
    let name = header(&request, "x-name").unwrap_or_default();
    let space = header(&request, "x-space");
    let root = state.space_or_open(space.as_deref())?.root.clone();
    let bytes = bytes.clone();
    let original = name.clone();
    tauri::async_runtime::spawn_blocking(move || {
        store(&root, &today(), &original, |to| to.write_all(&bytes))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("could not attach {name}: {e}"))
}

/// Move files a draft attached in one space into another, where the draft
/// is going instead (SPEC 3.1). Says where each one went, in order, for the
/// draft's links to follow.
#[tauri::command]
pub async fn move_attachments(
    state: State<'_, AppState>,
    from: String,
    to: String,
    paths: Vec<String>,
) -> Result<Vec<String>, String> {
    let from = state.space_or_open(Some(&from))?.root.clone();
    let to = state.space_or_open(Some(&to))?.root.clone();
    tauri::async_runtime::spawn_blocking(move || {
        paths
            .iter()
            .map(|path| {
                relocate(&from, &to, path).map_err(|e| format!("could not move {path}: {e}"))
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Open an attachment of the open space in its own app, as a double click
/// in the file manager would.
#[tauri::command]
pub async fn open_attachment(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let file =
        resolve(&state.space()?.root, &path).ok_or_else(|| format!("not an attachment: {path}"))?;
    if !file.is_file() {
        return Err(format!("{path} is not there"));
    }
    app.opener()
        .open_path(file.to_string_lossy(), None::<&str>)
        .map_err(|e| format!("could not open {path}: {e}"))
}
