//! Chat about the open space's notes.

use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::spaces::Space;
use crate::state::AppState;
use crate::storage::index::{self, IndexEntry};

/// What a chat sends the page while it replies.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ChatEvent {
    /// The index the model reads, numbered as it sees it: note `n` is
    /// `notes[n - 1]`. First, so citations resolve while the reply streams.
    Notes { notes: Vec<IndexEntry> },
    /// The next piece of the reply.
    Token { text: String },
    /// The reply is complete, or was stopped; these are the notes it cites.
    Done { cited: Vec<usize> },
}

const NO_MODEL: &str =
    "Chatting needs a model. Download one in Settings, or wait for the download to finish.";
const MODEL_OFF: &str = "Chatting needs the model, which is turned off in Settings.";

/// The loaded model, or why chat cannot have one.
fn chat_backend(
    app: &AppHandle,
) -> Result<std::sync::Arc<dyn crate::enrich::model::Backend>, String> {
    if !app.state::<AppState>().model_enabled() {
        return Err(MODEL_OFF.to_string());
    }
    crate::enrich::worker::backend(app).ok_or_else(|| NO_MODEL.to_string())
}

/// The notes closest to the last message, as positions in `notes` with their
/// bodies filled in, for `chat::reply`. Empty without the embedding model or
/// when embedding fails, and the chat then reads the index alone.
fn retrieve(
    app: &AppHandle,
    space: &Space,
    notes: &mut [IndexEntry],
    messages: &[crate::chat::Message],
) -> Vec<usize> {
    let (Some(embedder), Some(last)) = (crate::embed::embedder(app), messages.last()) else {
        return Vec::new();
    };
    let query = match embedder.embed_query(last.content.trim()) {
        Ok(query) => query,
        Err(e) => {
            log::warn!("could not embed the question, answering from the index: {e}");
            return Vec::new();
        }
    };
    let hits = space.nearest(&query, crate::chat::MAX_RETRIEVED);
    match space.index.read() {
        Ok(index) => crate::chat::retrieved(notes, &hits, &index),
        Err(_) => Vec::new(),
    }
}

/// Reply to the last message of a conversation about the open space's
/// notes, streamed through `on_event`. The model reads that space's
/// `index.jsonl`, and with the embedding model the text of the few notes
/// closest to the last message, taken from the index in memory. It never
/// reads the markdown. A new message, or `stop_chat`, ends the reply being
/// written.
#[tauri::command]
pub async fn chat(
    app: AppHandle,
    state: State<'_, AppState>,
    messages: Vec<crate::chat::Message>,
    on_event: tauri::ipc::Channel<ChatEvent>,
) -> Result<(), String> {
    let run = crate::chat::begin();
    let space = state.space()?;
    let path = index::index_path(&space.root);

    // Reading the file, loading the models and running them all block, so
    // none of it may run on the async runtime.
    tauri::async_runtime::spawn_blocking(move || {
        let mut notes = crate::chat::read_index(&path)?;
        let _ = on_event.send(ChatEvent::Notes {
            notes: notes.clone(),
        });
        let backend = chat_backend(&app)?;
        let state = app.state::<AppState>();
        let started = std::time::Instant::now();
        let retrieved = retrieve(&app, &space, &mut notes, &messages);
        let retrieval = started.elapsed();
        let reply = crate::chat::reply(
            &space.name,
            &notes,
            &retrieved,
            &messages,
            Local::now().date_naive(),
            backend.as_ref(),
            &mut |piece| {
                // Keeps the idle unload away for as long as the reply runs.
                state.mark_used();
                let _ = on_event.send(ChatEvent::Token {
                    text: piece.to_string(),
                });
                crate::chat::is_current(run)
            },
        )?;
        log::info!(
            "chat replied over {} notes in {:.1}s, {} retrieved in {:.2}s",
            notes.len(),
            started.elapsed().as_secs_f32(),
            retrieved.len(),
            retrieval.as_secs_f32()
        );
        let _ = on_event.send(ChatEvent::Done {
            cited: crate::chat::cited(&reply, notes.len()),
        });
        Ok(())
    })
    .await
    .map_err(|e| format!("the chat failed: {e}"))?
}

/// End the reply being written, if any. It ends as a normal reply would,
/// with what was written so far.
#[tauri::command]
pub fn stop_chat() {
    crate::chat::stop();
}

/// Read the open space's index into the model's cache ahead of the first
/// message, loading the model if need be, so the first reply starts sooner.
/// The page calls it when the chat opens.
#[tauri::command]
pub async fn warm_chat(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let space = state.space()?;
    let (name, path) = (space.name.clone(), index::index_path(&space.root));
    tauri::async_runtime::spawn_blocking(move || {
        let notes = crate::chat::read_index(&path)?;
        let backend = chat_backend(&app)?;
        app.state::<AppState>().mark_used();
        let started = std::time::Instant::now();
        let prefix = crate::chat::prefix(&name, &notes, Local::now().date_naive());
        backend.prefill_long(&prefix)?;
        log::info!(
            "chat read {} notes ahead in {:.1}s",
            notes.len(),
            started.elapsed().as_secs_f32()
        );
        Ok(())
    })
    .await
    .map_err(|e| format!("warming the chat failed: {e}"))?
}
