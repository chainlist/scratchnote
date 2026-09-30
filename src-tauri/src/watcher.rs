//! Watches the notes directory so edits made in another editor show up in the
//! app (SPEC 4.3, and the one-second bound in SPEC 11).
//!
//! Events are debounced, because one save from an editor typically produces
//! several, and the app's own writes are filtered out by comparing the file
//! against the fingerprint the writer recorded.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::Duration;

use notify::{EventKind, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

use crate::enrich::queue::Job;
use crate::spaces::Space;
use crate::state::AppState;
use crate::storage::daily_file::Status;
use crate::storage::search_db::Stamp;
use crate::storage::{check_date, fingerprint, index};

/// An editor writing a file emits several events; wait for quiet before
/// reparsing. Well inside the one second the spec allows.
const QUIET: Duration = Duration::from_millis(250);

/// Watch one space's notes and pages. The watcher is stored on the space, so
/// retiring the space stops it, and the processing task ends with it. It
/// watches the space's whole folder, since `pages/` may not be there yet.
pub fn start(app: AppHandle, space: &Arc<Space>) -> notify::Result<()> {
    let notes_dir = space.root.join("notes");
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

    watcher.watch(&space.root, RecursiveMode::Recursive)?;
    if let Ok(mut slot) = space.watcher.lock() {
        *slot = Some(watcher);
    }
    tauri::async_runtime::spawn(process(app, Arc::downgrade(space), rx));
    Ok(())
}

async fn process(app: AppHandle, space: Weak<Space>, mut rx: mpsc::UnboundedReceiver<PathBuf>) {
    let mut batch: HashSet<PathBuf> = HashSet::new();

    while let Some(first) = rx.recv().await {
        batch.insert(first);
        // Collect until the filesystem goes quiet.
        while let Ok(Some(path)) = tokio::time::timeout(QUIET, rx.recv()).await {
            batch.insert(path);
        }

        let Some(space) = space.upgrade().filter(|s| !s.is_retired()) else {
            return;
        };
        let mut touched = false;
        for path in batch.drain() {
            touched |= reindex(&app, &space, &path);
        }
        if touched {
            let _ = app.emit("index-rebuilt", ());
        }
    }
}

/// Reparse one changed file into the index: a daily file or a page file.
/// Anything else in the space's folder is not the watcher's business.
/// Returns whether anything changed.
fn reindex(app: &AppHandle, space: &Arc<Space>, path: &Path) -> bool {
    if path.starts_with(space.root.join("pages")) {
        reindex_page(app, space, path)
    } else if path.starts_with(space.root.join("notes")) {
        reindex_day(app, space, path)
    } else {
        false
    }
}

/// Reparse one page file (SPEC 4.3). A page renamed or moved is found again
/// by its id, and its stub follows it. A page file that is gone leaves the
/// index, and its stub stays.
fn reindex_page(app: &AppHandle, space: &Arc<Space>, path: &Path) -> bool {
    let Some(state) = app.try_state::<AppState>() else {
        return false;
    };
    if wrote_it_ourselves(&state, path) {
        return false;
    }
    let Some(file) = index::relative(&space.root, path) else {
        return false;
    };

    let parsed = index::parse_page(&space.root, path);
    let before = match &parsed {
        Some(page) => match space.page_changed(page) {
            Ok(before) => before,
            Err(_) => return false,
        },
        // Gone, or no longer a page: whatever page this file held goes.
        None => match space.page_file_gone(&file) {
            Ok(Some(gone)) => Some(gone),
            _ => return false,
        },
    };
    // Edited text parses as pending, as in a daily file.
    let queued = parsed.as_ref().is_some_and(|entry| {
        entry.status == Status::Pending
            && space
                .queue
                .lock()
                .is_ok_and(|mut queue| queue.push(Job::new(entry.id.clone(), entry.date.clone())))
    });

    // The stub follows the page's title, file, day and time.
    let moved = match (&before, &parsed) {
        (Some(before), Some(after)) => {
            before.file != after.file
                || before.subject != after.subject
                || before.date != after.date
                || before.time != after.time
        }
        (None, Some(_)) => true,
        _ => false,
    };

    let app = app.clone();
    let owned = space.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let (true, Some(page)) = (moved, parsed) {
            if let Some(before) = before.filter(|b| b.date != page.date) {
                if let Err(e) =
                    crate::pages::drop_stub(&state.writer, &owned, &before.date, &page.id).await
                {
                    log::warn!("could not take page {} off {}: {e}", page.id, before.date);
                }
            }
            if let Err(e) = crate::pages::sync_stub(&state.writer, &owned, &page).await {
                log::warn!("could not write the stub of page {}: {e}", page.id);
            }
        }
        if let Err(e) = owned.persist_index(&state.writer).await {
            log::warn!("could not persist the index after an external edit: {e}");
        }
        if queued {
            owned.persist_queue(&state.writer).await;
            state.wake.notify_one();
        }
    });

    log::info!("reindexed {file} in {} after an external edit", space.name);
    true
}

/// Reparse one daily file into the index. Returns whether anything changed.
fn reindex_day(app: &AppHandle, space: &Arc<Space>, path: &Path) -> bool {
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

    // Taken before the file is read, so a write in between makes the day
    // look older than it is, and read again.
    let stamp = Stamp::of(path);
    let notes = index::parse_day(path, &date);
    // A labelled note whose body was edited parses as pending, and so does a
    // block typed in by hand: both go to the model.
    let pending: Vec<String> = notes
        .iter()
        .filter(|entry| entry.status == Status::Pending)
        .map(|entry| entry.id.clone())
        .collect();
    if space.set_day(&date, &notes, stamp).is_err() {
        return false;
    }
    let queued = match space.queue.lock() {
        Ok(mut queue) => pending.into_iter().fold(false, |any, id| {
            queue.push(Job::new(id, date.clone())) || any
        }),
        Err(_) => false,
    };

    // Persist the refreshed cache. Failing to write it is not fatal: the file
    // is derived, and startup reparses anything newer than it.
    let app = app.clone();
    let owned = space.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(e) = owned.persist_index(&state.writer).await {
            log::warn!("could not persist the index after an external edit: {e}");
        }
        if queued {
            owned.persist_queue(&state.writer).await;
            state.wake.notify_one();
        }
    });

    log::info!("reindexed {date} in {} after an external edit", space.name);
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
