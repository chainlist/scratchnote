//! Threads, SPEC 6.4: the notes about one thing, gathered across days.

use std::collections::HashMap;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::embed::threads::{edits_path, when_written, Edits, Thread};
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
/// notes: without the embedding model, or with the model switched off.
#[tauri::command]
pub fn list_threads(state: State<'_, AppState>) -> Result<ThreadsView, String> {
    let space = state.space()?;
    Ok(view(&space))
}

fn view(space: &Space) -> ThreadsView {
    let edits = Edits::load(&space.root);
    let (when, subjects) = match space.index.read() {
        Ok(index) if !index.is_closed() => {
            let subjects: HashMap<String, String> = index
                .entries()
                .filter_map(|entry| Some((entry.id.clone(), entry.subject.clone()?)))
                .collect();
            (when_written(&index), subjects)
        }
        _ => return ThreadsView::default(),
    };
    let threads = space.vectors.lock().ok().and_then(|vectors| {
        let vectors = vectors.as_ref()?;
        let threads = space.threads.lock().ok()?;
        Some(
            threads
                .as_ref()?
                .list(vectors, &when, &subjects, &edits.titles),
        )
    });
    ThreadsView {
        threads: threads.unwrap_or_default(),
        alone: edits.alone.into_iter().collect(),
    }
}

/// A thread and its notes, or `None` once it is gone.
#[tauri::command]
pub fn get_thread(state: State<'_, AppState>, id: String) -> Result<Option<ThreadNotes>, String> {
    let space = state.space()?;
    let Some(thread) = view(&space).threads.into_iter().find(|t| t.id == id) else {
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
