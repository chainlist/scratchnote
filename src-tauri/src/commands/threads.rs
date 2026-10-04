//! Threads, SPEC 6.4: the notes about one thing, gathered across days.

use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::embed::threads::{edits_path, when_written, Edits, Thread, Threads, When};
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

/// A thread with its first notes and their text, to tell it from others
/// that have no title.
#[derive(Debug, Serialize)]
pub struct ThreadCard {
    #[serde(flatten)]
    pub thread: Thread,
    /// Its first two notes or pages.
    pub first: Vec<Note>,
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
    let Some(when) = dates(space) else {
        return ThreadsView::default();
    };
    let threads = placed(space, |threads| Some(threads.list(&when, &edits)));
    ThreadsView {
        threads: threads.unwrap_or_default(),
        alone: edits.alone.into_iter().collect(),
    }
}

/// When each note of the space was written, which threads are shown with.
/// `None` while the space is closed.
fn dates(space: &Space) -> Option<HashMap<String, When>> {
    let index = space.index.read().ok().filter(|index| !index.is_closed())?;
    Some(when_written(&index))
}

/// What `show` makes of where the notes were placed, `None` until the embed
/// task has loaded it.
fn placed<T>(space: &Space, show: impl FnOnce(&Threads) -> Option<T>) -> Option<T> {
    let threads = space.threads.lock().ok()?;
    show(threads.as_ref()?)
}

/// A thread and its notes, or `None` once it is gone.
#[tauri::command]
pub fn get_thread(state: State<'_, AppState>, id: String) -> Result<Option<ThreadNotes>, String> {
    let space = state.space()?;
    let Some(when) = dates(&space) else {
        return Ok(None);
    };
    let edits = Edits::load(&space.root);
    let Some(thread) = placed(&space, |threads| threads.get(&id, &when, &edits)) else {
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

/// Give a thread a title of the user's, or an empty one to take it off.
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
        let pinned = edits.pinned.remove(&id).is_some();
        edits.alone.insert(id) || pinned
    } else {
        edits.alone.remove(&id)
    };
    if !changed {
        return Ok(());
    }
    place_again(&app, &state, &space, &edits).await
}

/// Save what the user decided and place the notes again at once, rather
/// than at the next change to the notes.
async fn place_again(
    app: &AppHandle,
    state: &AppState,
    space: &Arc<Space>,
    edits: &Edits,
) -> Result<(), String> {
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

/// The notes of thread `id` as the space has it now, empty once it is gone.
fn notes_of(space: &Space, id: &str) -> Vec<String> {
    let edits = Edits::load(&space.root);
    dates(space)
        .and_then(|when| placed(space, |threads| threads.get(id, &when, &edits)))
        .map(|thread| thread.notes)
        .unwrap_or_default()
}

/// Make a suggested thread the user's: its notes stay in it, and notes
/// like them keep joining.
#[tauri::command]
pub async fn keep_thread(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let space = state.space()?;
    let notes = notes_of(&space, &id);
    if notes.is_empty() {
        return Ok(());
    }
    let mut edits = Edits::load(&space.root);
    for note in notes {
        edits.pinned.insert(note, id.clone());
    }
    edits.dismissed.remove(&id);
    place_again(&app, &state, &space, &edits).await
}

/// Stop suggesting a thread until it holds another note.
#[tauri::command]
pub async fn dismiss_thread(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let space = state.space()?;
    let notes = notes_of(&space, &id);
    if notes.is_empty() {
        return Ok(());
    }
    let mut edits = Edits::load(&space.root);
    edits.dismissed.insert(id, notes.into_iter().collect());
    place_again(&app, &state, &space, &edits).await
}

/// Put notes in thread `into`, or a new thread when it is `None`, where
/// they stay whatever they score. Gives the thread's id: a new one is
/// named after the earliest of the notes.
#[tauri::command]
pub async fn put_in_thread(
    app: AppHandle,
    state: State<'_, AppState>,
    notes: Vec<String>,
    into: Option<String>,
) -> Result<String, String> {
    let space = state.space()?;
    let when = dates(&space).ok_or("the space is closed")?;
    let mut notes: Vec<String> = notes
        .into_iter()
        .filter(|id| when.contains_key(id))
        .collect();
    notes.sort_by(|a, b| (&when[a], a).cmp(&(&when[b], b)));
    let first = notes.first().ok_or("no note to put in a thread")?.clone();
    let thread = match into {
        Some(thread) => thread,
        None => {
            let taken: std::collections::HashSet<String> =
                placed(&space, |threads| Some(threads.names())).unwrap_or_default();
            std::iter::once(first.clone())
                .chain((2..).map(|n| format!("{first}-{n}")))
                .find(|name| !taken.contains(name))
                .expect("a free name")
        }
    };
    let mut edits = Edits::load(&space.root);
    for note in notes {
        edits.alone.remove(&note);
        edits.pinned.insert(note, thread.clone());
    }
    place_again(&app, &state, &space, &edits).await?;
    Ok(thread)
}

/// Put every note of thread `from` in thread `into`, which keeps its title,
/// or takes the other's when it has none.
#[tauri::command]
pub async fn merge_threads(
    app: AppHandle,
    state: State<'_, AppState>,
    from: String,
    into: String,
) -> Result<(), String> {
    let space = state.space()?;
    if from == into {
        return Ok(());
    }
    let notes = notes_of(&space, &from);
    let mut edits = Edits::load(&space.root);
    for note in notes {
        edits.alone.remove(&note);
        edits.pinned.insert(note, into.clone());
    }
    if let Some(title) = edits.titles.remove(&from) {
        edits.titles.entry(into).or_insert(title);
    }
    edits.dismissed.remove(&from);
    place_again(&app, &state, &space, &edits).await
}

/// Every thread of the open space with its first notes, newest first.
#[tauri::command]
pub fn thread_cards(state: State<'_, AppState>) -> Result<Vec<ThreadCard>, String> {
    let space = state.space()?;
    let mut threads = view(&space).threads;
    threads.sort_by(|a, b| (&b.until, &b.id).cmp(&(&a.until, &a.id)));
    cards(&space, threads)
}

/// `threads` with their first notes.
fn cards(space: &Space, threads: Vec<Thread>) -> Result<Vec<ThreadCard>, String> {
    let first = space.read(|idx, db| {
        let entries: HashMap<&str, &IndexEntry> =
            idx.entries().map(|e| (e.id.as_str(), e)).collect();
        let shown = threads
            .iter()
            .flat_map(|thread| thread.notes.iter().take(2))
            .filter_map(|id| entries.get(id.as_str()).copied());
        crate::search::with_bodies(db, shown)
    })?;
    let mut first: HashMap<String, Note> = first
        .unwrap_or_default()
        .into_iter()
        .map(|note| (note.id.clone(), note))
        .collect();
    Ok(threads
        .into_iter()
        .map(|thread| ThreadCard {
            first: thread
                .notes
                .iter()
                .take(2)
                .filter_map(|id| first.remove(id))
                .collect(),
            thread,
        })
        .collect())
}

/// The threads a note could be put in, the one it fits best first.
#[tauri::command]
pub fn threads_for_note(state: State<'_, AppState>, id: String) -> Result<Vec<ThreadCard>, String> {
    let space = state.space()?;
    let Some(when) = dates(&space) else {
        return Ok(Vec::new());
    };
    let edits = Edits::load(&space.root);
    // The vectors are let go before the index is read for the notes' text.
    let ranked: Vec<Thread> = {
        let Ok(vectors) = space.vectors.lock() else {
            return Ok(Vec::new());
        };
        let Some(vectors) = vectors.as_ref() else {
            return Ok(Vec::new());
        };
        placed(&space, |threads| {
            Some(
                threads
                    .ranked_for(&id, vectors)
                    .iter()
                    .filter_map(|thread| threads.get(thread, &when, &edits))
                    .collect(),
            )
        })
        .unwrap_or_default()
    };
    cards(&space, ranked)
}
