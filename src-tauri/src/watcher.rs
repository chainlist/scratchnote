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

use crate::spaces::Space;
use crate::state::AppState;
use crate::storage::daily_file::Note;
use crate::storage::search_db::Stamp;
use crate::storage::writer::{SelfWrites, Writer};
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
        let Some(state) = app.try_state::<AppState>() else {
            batch.clear();
            continue;
        };
        // Reading the files and putting them in search.db both block.
        let paths: Vec<PathBuf> = batch.drain().collect();
        let (reading, self_writes) = (space.clone(), state.writer.self_writes());
        let read = tauri::async_runtime::spawn_blocking(move || {
            let mut batch = Reindexed::default();
            for path in &paths {
                reindex(&reading, &self_writes, path, &mut batch);
            }
            batch
        })
        .await;
        let read = match read {
            Ok(read) => read,
            Err(e) => {
                log::warn!("could not read the files edited outside the app: {e}");
                continue;
            }
        };
        if !read.changed {
            continue;
        }
        let _ = app.emit("index-rebuilt", ());

        for restub in read.stubs {
            restub.write(&state.writer, &space).await;
        }
        // Once for the whole batch. Failing to write it is not fatal: the
        // file is derived, and startup reparses anything newer than it.
        if let Err(e) = space.persist_index(&state.writer).await {
            log::warn!("could not persist the index after an external edit: {e}");
        }
    }
}

/// What a batch of changed files did to the index: whether it changed at
/// all, and the stubs to write again for the pages it moved.
#[derive(Default)]
struct Reindexed {
    changed: bool,
    stubs: Vec<Restub>,
}

/// A page whose title, file, day or time changed, so its stub follows it.
struct Restub {
    page: Note,
    /// The day it was on before, when it moved to another, whose stub goes.
    left: Option<String>,
}

impl Restub {
    async fn write(self, writer: &Writer, space: &Space) {
        let Restub { page, left } = self;
        if let Some(left) = left {
            if let Err(e) = crate::pages::drop_stub(writer, space, &left, &page.id).await {
                log::warn!("could not take page {} off {left}: {e}", page.id);
            }
        }
        if let Err(e) = crate::pages::sync_stub(writer, space, &page).await {
            log::warn!("could not write the stub of page {}: {e}", page.id);
        }
    }
}

/// Reparse one changed file into the index: a daily file or a page file.
/// Anything else in the space's folder is not the watcher's business.
fn reindex(space: &Space, self_writes: &SelfWrites, path: &Path, batch: &mut Reindexed) {
    if path.starts_with(space.root.join("pages")) {
        reindex_page(space, self_writes, path, batch);
    } else if path.starts_with(space.root.join("notes")) {
        batch.changed |= reindex_day(space, self_writes, path);
    }
}

/// Reparse one page file (SPEC 4.3). A page renamed or moved is found again
/// by its id, and its stub follows it. A page file that is gone leaves the
/// index, and its stub stays.
fn reindex_page(space: &Space, self_writes: &SelfWrites, path: &Path, batch: &mut Reindexed) {
    if wrote_it_ourselves(self_writes, path) {
        return;
    }
    let Some(file) = index::relative(&space.root, path) else {
        return;
    };

    let parsed = index::parse_page(&space.root, path);
    let before = match &parsed {
        Some(page) => match space.page_changed(page) {
            Ok(before) => before,
            Err(_) => return,
        },
        // Gone, or no longer a page: whatever page this file held goes.
        None => match space.page_file_gone(&file) {
            Ok(Some(gone)) => Some(gone),
            _ => return,
        },
    };
    batch.changed = true;
    log::info!("reindexed {file} in {} after an external edit", space.name);

    // The stub follows the page's title, file, day and time.
    let Some(page) = parsed else {
        return;
    };
    let moved = before.as_ref().is_none_or(|before| {
        before.file != page.file
            || before.subject != page.subject
            || before.date != page.date
            || before.time != page.time
    });
    if moved {
        let left = before.map(|b| b.date).filter(|date| *date != page.date);
        batch.stubs.push(Restub { page, left });
    }
}

/// Reparse one daily file into the index. Returns whether anything changed.
fn reindex_day(space: &Space, self_writes: &SelfWrites, path: &Path) -> bool {
    let Some(date) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    if check_date(date).is_err() {
        return false;
    }
    if wrote_it_ourselves(self_writes, path) {
        return false;
    }

    // Taken before the file is read, so a write in between makes the day
    // look older than it is, and read again.
    let stamp = Stamp::of(path);
    let notes = index::parse_day(path, date);
    if space.set_day(date, &notes, stamp).is_err() {
        return false;
    }

    log::info!("reindexed {date} in {} after an external edit", space.name);
    true
}

/// True when the file on disk still matches what the app last wrote there, so
/// the event came from us rather than from another editor.
fn wrote_it_ourselves(self_writes: &SelfWrites, path: &Path) -> bool {
    let Ok(contents) = std::fs::read_to_string(path) else {
        // A file that vanished is somebody else's doing, so let it through.
        return false;
    };
    let current = fingerprint(&contents);
    self_writes
        .lock()
        .map(|seen| seen.get(path) == Some(&current))
        .unwrap_or(false)
}
