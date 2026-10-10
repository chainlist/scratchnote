//! Search by words and by meaning, and similar notes, SPEC 6.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;
use tauri::{AppHandle, State};

use super::blocking;
use crate::search::Found;
use crate::spaces::Space;
use crate::state::AppState;
use crate::storage::daily_file::Note;
use crate::storage::index::IndexEntry;
use crate::Result;

/// Words across every day (SPEC 6): the `limit`
/// matches from `offset` on, and how many there are in all. Without a limit
/// every match comes back, as a plugin's search asks for them.
#[tauri::command]
pub async fn search(
    state: State<'_, AppState>,
    query: String,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<Found> {
    let (offset, limit) = (offset.unwrap_or(0), limit.unwrap_or(usize::MAX));
    let found = state
        .space()?
        .read(|idx, db| crate::search::search(idx, db, &query, offset, limit))?;
    Ok(found.unwrap_or_default())
}

/// Every note and page whose body holds any of `needles`, newest first, for
/// a plugin to pick what it reads out of, as the tasks view picks the open
/// tasks (SPEC 3.8).
#[tauri::command]
pub async fn notes_containing(
    state: State<'_, AppState>,
    needles: Vec<String>,
) -> Result<Vec<Note>> {
    let found = state
        .space()?
        .read(|idx, db| crate::search::containing(idx, db, &needles))?;
    Ok(found.unwrap_or_default())
}

/// What a text to embed is, which says how the model frames it.
enum Framed {
    Query,
    Draft,
}

/// `text` embedded off the async runtime, since loading and running the
/// model both block. `None` without the embedding model, or when it fails,
/// which is logged.
async fn embed_blocking(app: &AppHandle, text: String, framed: Framed) -> Result<Option<Vec<f32>>> {
    let app = app.clone();
    blocking(move || {
        let embedder = crate::embed::embedder(&app)?;
        let (vector, what) = match framed {
            Framed::Query => (embedder.embed_query(&text), "the search query"),
            Framed::Draft => (embedder.embed_document(&text), "the draft"),
        };
        vector
            .map_err(|e| log::warn!("could not embed {what}: {e}"))
            .ok()
    })
    .await
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
/// not find, best first. Empty without the embedding model.
#[tauri::command]
pub async fn search_meaning(
    app: AppHandle,
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<Note>> {
    let words = query.trim().to_string();
    if words.is_empty() {
        return Ok(Vec::new());
    }
    let space = state.space()?;
    let Some(vector) = embed_blocking(&app, words, Framed::Query).await? else {
        return Ok(Vec::new());
    };

    let hits = space.related(&vector, usize::MAX, MIN_LEAD);
    let found =
        space.read(|idx, db| crate::search::by_meaning(idx, db, &query, &hits, MEANING_NOTES))?;
    Ok(found.unwrap_or_default())
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
pub async fn similar_notes(state: State<'_, AppState>, id: String) -> Result<Vec<Note>> {
    let space = state.space()?;
    let hits = space.similar(&id, SIMILAR_NOTES, MIN_SIMILARITY);
    notes_of(&space, &hits)
}

/// `hits` as notes with their text, in their order. A hit the index no
/// longer has is left out.
fn notes_of(space: &Space, hits: &[(String, f32)]) -> Result<Vec<Note>> {
    let found = space.read(|idx, db| {
        let notes: HashMap<&str, &IndexEntry> = idx.entries().map(|e| (e.id.as_str(), e)).collect();
        let found = hits
            .iter()
            .filter_map(|(hit, _)| notes.get(hit.as_str()).copied());
        crate::search::with_bodies(db, found)
    })?;
    Ok(found.unwrap_or_default())
}

/// The score, as Similar notes scores, an old note needs against a draft for
/// recall to name it. Stricter than Similar notes, which is asked for:
/// recall speaks up unasked, while the note is being written.
const MIN_RECALL: f32 = 0.35;
/// A draft shorter than this, in characters, says too little to go on.
const MIN_RECALL_CHARS: usize = 12;

/// What an editor's footer says of a draft as it is written.
#[derive(Debug, Default, Serialize)]
pub struct DraftHints {
    /// The old note the draft is about, when one stands out (SPEC 6.3).
    pub note: Option<Note>,
    /// For a draft that mentions no name, the name it looks like, as the
    /// newest note mentioning it types it (SPEC 3.10).
    pub mention: Option<String>,
}

/// What a draft calls to mind (SPEC 6.3, 3.10): the old note closest to it
/// in meaning, if it is close enough, and for a draft that mentions no
/// name, the name whose notes it fits, if one does as a note joins a
/// thread. The draft is embedded once for both. `exclude` is the note the
/// draft is an edit of. Nothing for a draft written into a space that is
/// not open, whose vectors are not in memory, or without the embedding
/// model.
#[tauri::command]
pub async fn draft_hints(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
    exclude: Option<String>,
    space: Option<String>,
) -> Result<DraftHints> {
    let open = state.space()?;
    if space.is_some_and(|name| name != open.name) {
        return Ok(DraftHints::default());
    }
    let text = text.trim().to_string();
    if text.chars().count() < MIN_RECALL_CHARS {
        return Ok(DraftHints::default());
    }
    let unnamed = crate::mentions::mentions(&text).is_empty();
    let Some(vector) = embed_blocking(&app, text, Framed::Draft).await? else {
        return Ok(DraftHints::default());
    };

    let hits = open.closest(&vector, exclude.as_deref(), 1, MIN_RECALL);
    let note = notes_of(&open, &hits)?.into_iter().next();
    let mention = if unnamed {
        name_for(&open, &vector, exclude.as_deref())?
    } else {
        None
    };
    Ok(DraftHints { note, mention })
}

/// The name whose notes a draft's vector fits, as it is typed: scored as a
/// note joins a thread, against every name with a few notes.
fn name_for(space: &Space, vector: &[f32], exclude: Option<&str>) -> Result<Option<String>> {
    let Some((rows, listed)) =
        space.read(|idx, db| Ok((db.mention_rows()?, crate::mentions::list(idx, db)?)))?
    else {
        return Ok(None);
    };
    let mut names: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, key, _) in rows {
        let notes = names.entry(key).or_default();
        if !notes.contains(&id) {
            notes.push(id);
        }
    }
    let cuts = crate::embed::threads::CUTS;
    let found = space.vectors.lock().ok().and_then(|vectors| {
        crate::embed::threads::closest_name(
            vectors.as_ref()?,
            &names,
            vector,
            exclude,
            cuts.join,
            cuts.lead,
        )
    });
    Ok(found.and_then(|(key, _)| {
        listed
            .into_iter()
            .find(|mention| mention.key == key)
            .map(|mention| mention.name)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::vectors::Vectors;
    use crate::embed::{installed_embedder, samples, Embedder};

    /// What recall names for the first half of each made-up note, the note
    /// left out as if it were being written: a note on the same thing nearly
    /// every time, and never one on another thing. Needs the embedding
    /// model; `cargo test recall_names -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the downloaded embedding model"]
    fn recall_names_a_note_on_the_same_thing_and_never_another() {
        let Some(embedder) = installed_embedder() else {
            return;
        };
        let notes = samples::notes();
        let mut vectors = Vectors::new(embedder.model_id(), embedder.dims());
        for (_, note) in &notes {
            let vector = embedder.embed_document(&note.body).unwrap();
            vectors
                .insert(note.id.clone(), note.hash.clone(), vector)
                .unwrap();
        }
        let about: HashMap<&str, &str> = notes
            .iter()
            .map(|(thing, note)| (note.id.as_str(), *thing))
            .collect();

        let (mut found, mut on_a_thing, mut wrong) = (0, 0, Vec::new());
        for (thing, note) in &notes {
            let words: Vec<&str> = note.body.split_whitespace().collect();
            let draft = words[..words.len().div_ceil(2)].join(" ");
            let vector = embedder.embed_document(&draft).unwrap();
            if !thing.starts_with('-') {
                on_a_thing += 1;
            }
            match vectors
                .closest(&vector, Some(&note.id), 1, MIN_RECALL)
                .first()
            {
                Some((id, score)) if about[id.as_str()] == *thing => {
                    eprintln!("{score:.2} {thing:>10} {draft}");
                    found += 1;
                }
                Some((id, score)) => wrong.push(format!(
                    "{draft:?} named {} at {score:.2}",
                    about[id.as_str()]
                )),
                None => eprintln!("     {thing:>10} {draft}"),
            }
        }
        assert!(
            wrong.is_empty(),
            "named another thing:
  {}",
            wrong.join(
                "
  "
            )
        );
        assert!(
            found * 100 >= on_a_thing * 85,
            "found {found} of {on_a_thing}"
        );
    }
}
