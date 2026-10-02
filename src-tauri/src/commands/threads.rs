//! Threads, SPEC 6.4: the notes about one thing, gathered across days.

use std::collections::HashMap;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::embed::threads::{edits_path, when_written, Edits, Thread, Threads, When};
use crate::embed::vectors::Vectors;
use crate::spaces::Space;
use crate::state::AppState;
use crate::storage::daily_file::Note;
use crate::storage::index::IndexEntry;

/// The longest title a thread takes, in characters.
const MAX_TITLE: usize = 120;

#[derive(Debug, Default, Serialize)]
pub struct ThreadsView {
    /// Every thread of the open space.
    pub threads: Vec<Thread>,
    /// The notes the user took out of threads.
    pub alone: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ThreadNotes {
    pub thread: Thread,
    /// Its notes and pages with their text, oldest first.
    pub notes: Vec<Note>,
}

/// The open space's threads, empty until the embed task has placed its
/// notes, which it never does without the embedding model.
#[tauri::command]
pub fn list_threads(state: State<'_, AppState>) -> Result<ThreadsView, String> {
    let space = state.space()?;
    Ok(view(&space))
}

fn view(space: &Space) -> ThreadsView {
    let edits = Edits::load(&space.root);
    let Some((when, subjects)) = dates_and_subjects(space) else {
        return ThreadsView::default();
    };
    let threads = placed(space, |threads, vectors| {
        Some(threads.list(vectors, &when, &subjects, &edits.titles))
    });
    ThreadsView {
        threads: threads.unwrap_or_default(),
        alone: edits.alone.into_iter().collect(),
    }
}

/// When each note of the space was written, and the notes' subjects, which
/// threads are shown with. `None` while the space is closed.
fn dates_and_subjects(space: &Space) -> Option<(HashMap<String, When>, HashMap<String, String>)> {
    let index = space.index.read().ok().filter(|index| !index.is_closed())?;
    let subjects = index
        .entries()
        .filter_map(|entry| Some((entry.id.clone(), entry.subject.clone()?)))
        .collect();
    Some((when_written(&index), subjects))
}

/// What `show` makes of where the notes were placed and their vectors,
/// `None` until the embed task has loaded them.
fn placed<T>(space: &Space, show: impl FnOnce(&Threads, &Vectors) -> Option<T>) -> Option<T> {
    let vectors = space.vectors.lock().ok()?;
    let threads = space.threads.lock().ok()?;
    show(threads.as_ref()?, vectors.as_ref()?)
}

/// A thread and its notes, or `None` once it is gone.
#[tauri::command]
pub fn get_thread(state: State<'_, AppState>, id: String) -> Result<Option<ThreadNotes>, String> {
    let space = state.space()?;
    let Some((when, subjects)) = dates_and_subjects(&space) else {
        return Ok(None);
    };
    let titles = Edits::load(&space.root).titles;
    let Some(thread) = placed(&space, |threads, vectors| {
        threads.get(&id, vectors, &when, &subjects, &titles)
    }) else {
        return Ok(None);
    };
    let notes = space.read(|idx, db| {
        let entries: HashMap<&str, &IndexEntry> =
            idx.entries().map(|e| (e.id.as_str(), e)).collect();
        let members = thread
            .notes
            .iter()
            .filter_map(|id| entries.get(id.as_str()).copied());
        crate::search::with_bodies(db, members)
    })?;
    Ok(Some(ThreadNotes {
        thread,
        notes: notes.unwrap_or_default(),
    }))
}

/// Give a thread a title of the user's, or an empty one to go back to the
/// subject of its most typical note.
#[tauri::command]
pub async fn rename_thread(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> Result<(), String> {
    let space = state.space()?;
    let title: String = title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_TITLE)
        .collect();
    let mut edits = Edits::load(&space.root);
    if title.is_empty() {
        edits.titles.remove(&id);
    } else {
        edits.titles.insert(id, title);
    }
    state
        .writer
        .write_index(edits_path(&space.root), edits.to_json())
        .await?;
    let _ = app.emit(
        "threads-changed",
        serde_json::json!({ "space": space.name }),
    );
    Ok(())
}

/// Take a note out of threads, where it then stays, or let it back in. It
/// is placed again at once rather than at the next change to the notes.
#[tauri::command]
pub async fn keep_out_of_threads(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    out: bool,
) -> Result<(), String> {
    let space = state.space()?;
    let mut edits = Edits::load(&space.root);
    let changed = if out {
        edits.alone.insert(id)
    } else {
        edits.alone.remove(&id)
    };
    if !changed {
        return Ok(());
    }
    state
        .writer
        .write_index(edits_path(&space.root), edits.to_json())
        .await?;
    let placing = space.clone();
    tauri::async_runtime::spawn_blocking(move || crate::embed::sync::sync_threads(&placing))
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit(
        "threads-changed",
        serde_json::json!({ "space": space.name }),
    );
    Ok(())
}
