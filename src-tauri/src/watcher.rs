//! Watches the notes directory so edits made in another editor show up in the
//! app (SPEC 4.3, and the one-second bound in SPEC 11).
//!
//! Events are debounced, because one save from an editor typically produces
//! several, and the app's own writes are filtered out by comparing the file
//! against the fingerprint the writer recorded.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

use crate::state::AppState;
use crate::storage::{check_date, fingerprint, index};

/// An editor writing a file emits several events; wait for quiet before
/// reparsing. Well inside the one second the spec allows.
const QUIET: Duration = Duration::from_millis(250);

/// Returns the watcher, which the caller must keep alive. Dropping it stops
/// the watching.
pub fn start(app: AppHandle, root: PathBuf) -> notify::Result<RecommendedWatcher> {
    let notes_dir = root.join("notes");
    // notify cannot watch a directory that is not there yet.
    if let Err(e) = std::fs::create_dir_all(&notes_dir) {
        log::warn!("could not create {}: {e}", notes_dir.display());
    }

    let (tx, rx) = mpsc::unbounded_channel::<PathBuf>();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        let event = match result {
            Ok(event) => event,
            Err(e) => {
                log::warn!("watch error: {e}");
                return;
            }
        };
        if !matches!(
            event.kind,
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
        ) {
            return;
        }
        for path in event.paths {
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                let _ = tx.send(path);
            }
        }
    })?;

    watcher.watch(&notes_dir, RecursiveMode::Recursive)?;
    tauri::async_runtime::spawn(process(app, rx));
    Ok(watcher)
}

async fn process(app: AppHandle, mut rx: mpsc::UnboundedReceiver<PathBuf>) {
    let mut batch: HashSet<PathBuf> = HashSet::new();

    while let Some(first) = rx.recv().await {
        batch.insert(first);
        // Collect until the filesystem goes quiet.
        while let Ok(Some(path)) = tokio::time::timeout(QUIET, rx.recv()).await {
            batch.insert(path);
        }

        let mut touched = false;
        for path in batch.drain() {
            touched |= reindex(&app, &path);
        }
        if touched {
            let _ = app.emit("index-rebuilt", ());
        }
    }
}

/// Reparse one daily file into the index. Returns whether anything changed.
fn reindex(app: &AppHandle, path: &Path) -> bool {
    let Some(date) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    if check_date(date).is_err() {
        return false;
    }
    let date = date.to_string();

    let Some(state) = app.try_state::<AppState>() else {
        return false;
    };

    if wrote_it_ourselves(&state, path) {
        return false;
    }

    let entries = index::parse_day(path, &date);
    {
        let Ok(mut idx) = state.index.write() else {
            return false;
        };
        idx.replace_day(&date, entries);
    }

    // Persist the refreshed cache. Failing to write it is not fatal: the file
    // is derived, and startup reparses anything newer than it.
    let contents = match state.index.read() {
        Ok(idx) => idx.to_jsonl(),
        Err(_) => return false,
    };
    let writer = state.writer.clone();
    let index_file = index::index_path(&state.settings.root);
    tauri::async_runtime::spawn(async move {
        if let Err(e) = writer.write_index(index_file, contents).await {
            log::warn!("could not persist the index after an external edit: {e}");
        }
    });

    log::info!("reindexed {date} after an external edit");
    true
}

/// True when the file on disk still matches what the app last wrote there, so
/// the event came from us rather than from another editor.
fn wrote_it_ourselves(state: &AppState, path: &Path) -> bool {
    let Ok(contents) = std::fs::read_to_string(path) else {
        // A file that vanished is somebody else's doing, so let it through.
        return false;
    };
    let current = fingerprint(&contents);
    state
        .writer
        .self_writes()
        .lock()
        .map(|seen| seen.get(path) == Some(&current))
        .unwrap_or(false)
}
