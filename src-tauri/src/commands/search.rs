//! Search by words and by meaning, similar notes, and categories, SPEC 6.

use std::collections::HashMap;

use tauri::{AppHandle, State};

use crate::state::AppState;
use crate::storage::categories;
use crate::storage::daily_file::Note;
use crate::storage::index::IndexEntry;

/// The categories notes are filed under, with how many carry each, most used
/// first. Categories no note carries yet are left out.
#[tauri::command]
pub fn list_categories(state: State<'_, AppState>) -> Result<Vec<(String, u32)>, String> {
    let space = state.space()?;
    let counts = space
        .index
        .read()
        .map_err(|_| "index lock poisoned".to_string())?
        .category_counts();
    Ok(categories::in_use(&categories::load(&space.root), &counts))
}

/// Every category on the list, in file order, used or not, for the note
/// editor to offer.
#[tauri::command]
pub fn category_names(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(categories::load(&state.space()?.root))
}

/// Words and a `#category` filter across every day (SPEC 6).
#[tauri::command]
pub fn search(state: State<'_, AppState>, query: String) -> Result<Vec<Note>, String> {
    let space = state.space()?;
    let idx = space
        .index
        .read()
        .map_err(|_| "index lock poisoned".to_string())?;
    Ok(crate::search::search(&idx, &query))
}

/// Every note and page with a task box, newest first, for the tasks view to
/// pick the open tasks out of (SPEC 3.8).
#[tauri::command]
pub fn task_notes(state: State<'_, AppState>) -> Result<Vec<Note>, String> {
    let space = state.space()?;
    let idx = space
        .index
        .read()
        .map_err(|_| "index lock poisoned".to_string())?;
    Ok(crate::search::with_tasks(&idx))
}

/// How many notes search by meaning adds at most.
const MEANING_NOTES: usize = 5;
/// How far a note's cosine to the query must sit above the mean of the
/// space's other notes. Checked with EmbeddingGemma on 85 real notes and 31
/// short queries: the notes 23 of them were after led by 0.15 to 0.41, and 3
/// of the 8 with no answer in the notes left a note above 0.15. The lead
/// holds from five notes up.
const MIN_LEAD: f32 = 0.15;

/// Notes close in meaning to a query's words that the words themselves do
/// not find, best first, within its `#category` filter. Empty without the
/// embedding model, with the model switched off, or for a query of `#` tokens
/// only.
#[tauri::command]
pub async fn search_meaning(
    app: AppHandle,
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<Note>, String> {
    let words = crate::search::words(&query);
    if words.is_empty() {
        return Ok(Vec::new());
    }
    let space = state.space()?;

    // Loading and running the model both block.
    let embedder_app = app.clone();
    let vector = tauri::async_runtime::spawn_blocking(move || {
        let embedder = crate::embed::embedder(&embedder_app)?;
        embedder
            .embed_query(&words)
            .map_err(|e| log::warn!("could not embed the search query: {e}"))
            .ok()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(vector) = vector else {
        return Ok(Vec::new());
    };
    state.mark_used();

    let hits = space.related(&vector, usize::MAX, MIN_LEAD);
    let idx = space
        .index
        .read()
        .map_err(|_| "index lock poisoned".to_string())?;
    Ok(crate::search::by_meaning(
        &idx,
        &query,
        &hits,
        MEANING_NOTES,
    ))
}

/// How many notes "Similar notes" lists at most.
const SIMILAR_NOTES: usize = 8;
/// The score, a cosine once the mean of the space's other notes is removed,
/// below which a note is not shown as similar. Set on 75 real notes: notes on
/// the same thing scored 0.35 and up, unrelated ones mostly under 0.25. It
/// holds in small spaces too: in 10-note samples of 82 real notes, 99.5% of
/// the same-topic pairs passed it and 2.5% of the unrelated ones.
const MIN_SIMILARITY: f32 = 0.28;

/// The notes closest in meaning to this one, best first. Empty while the note
/// has no vector yet, or without the embedding model.
#[tauri::command]
pub fn similar_notes(state: State<'_, AppState>, id: String) -> Result<Vec<Note>, String> {
    let space = state.space()?;
    let hits = space.similar(&id, SIMILAR_NOTES, MIN_SIMILARITY);
    let idx = space
        .index
        .read()
        .map_err(|_| "index lock poisoned".to_string())?;
    let notes: HashMap<&str, &IndexEntry> = idx.entries().map(|e| (e.id.as_str(), e)).collect();
    Ok(hits
        .iter()
        .filter_map(|(hit, _)| notes.get(hit.as_str()).map(|e| e.to_note()))
        .collect())
}
