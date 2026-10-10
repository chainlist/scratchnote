//! Embeddings: one vector per note, for search by meaning, similar notes,
//! recall and threads.
//!
//! Only the body is embedded, so a vector is tied to the body hash: editing a
//! note's text embeds it again, renaming a page does not.

pub mod activity;
#[cfg(test)]
mod bench;
pub mod categories;
pub mod classify;
pub mod download;
pub mod llama;
pub mod map;
pub mod math;
pub mod model;
#[cfg(test)]
pub mod samples;
pub mod sync;
pub mod threads;
pub mod vectors;

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::state::AppState;
use activity::Activity;
use model::{model_file, EmbeddingModel};
use crate::Result;

/// A model that turns text into vectors.
pub trait Embedder: Send + Sync {
    /// Names the model, its dimensions and how it embeds a note.
    /// `space.db` records it, so vectors made another way are never
    /// compared with this one's.
    fn model_id(&self) -> &str;
    fn dims(&self) -> usize;
    /// A note, embedded as it is.
    fn embed_document(&self, text: &str) -> Result<Vec<f32>>;
    /// A question. EmbeddingGemma frames queries and documents differently,
    /// hence a method of its own.
    fn embed_query(&self, text: &str) -> Result<Vec<f32>>;
}

/// The model that embeds notes, or `None` while there is none. Loaded on
/// first use, then kept until the app quits, since it is small and every new
/// note and search by meaning needs it. A load that fails is tried again on
/// the next call, which comes with the next change to a note or the next
/// search, never in a loop. One load runs at a time: a search that comes
/// while the embed task loads the model waits for that one.
pub(crate) fn embedder(app: &AppHandle) -> Option<Arc<dyn Embedder>> {
    let state = app.state::<AppState>();
    crate::state::load_once(&state.embedder, &state.embedder_loading, || {
        if !download::is_installed(&state.local_data, EmbeddingModel) {
            activity::report(app, Activity::NoModel);
            return None;
        }
        let path = model_file(&state.local_data, EmbeddingModel);
        log::info!("loading {}", path.display());
        activity::report(app, Activity::Loading);

        match llama::LlamaEmbedder::load(&path) {
            Ok(loaded) => {
                let loaded: Arc<dyn Embedder> = Arc::new(loaded);
                Some(loaded)
            }
            Err(e) => {
                log::error!("could not load the embedding model: {e}");
                activity::report(app, Activity::Failed { error: e.to_string() });
                None
            }
        }
    })
}

/// The embedding model for the tests that need real weights: the file
/// `SCRATCHNOTE_EMBEDDING_MODEL` names, else the one the app downloads into
/// the app's own folder on this computer. `None`, and the test skips,
/// without either.
#[cfg(test)]
pub fn installed_embedder() -> Option<llama::LlamaEmbedder> {
    let path = match std::env::var_os("SCRATCHNOTE_EMBEDDING_MODEL") {
        Some(path) => std::path::PathBuf::from(path),
        None => model_file(&model::installed_local_data()?, EmbeddingModel),
    };
    if !path.is_file() {
        eprintln!("no embedding model at {}, skipping", path.display());
        return None;
    }
    Some(llama::LlamaEmbedder::load(&path).expect("the embedding model should load"))
}

/// A bag of words for tests: each folded word lands in one of a few buckets,
/// so texts that share words point the same way and unrelated ones do not.
#[cfg(test)]
pub struct StubEmbedder;

#[cfg(test)]
impl StubEmbedder {
    const DIMS: usize = 64;

    fn embed(&self, text: &str) -> Vec<f32> {
        use std::hash::{DefaultHasher, Hash, Hasher};

        let mut vector = vec![0.0; Self::DIMS];
        let folded = crate::search::fold(text);
        for word in folded.split(|c: char| !c.is_alphanumeric()) {
            if word.is_empty() {
                continue;
            }
            // `DefaultHasher::new` has fixed keys, so a word always lands in
            // the same bucket.
            let mut hasher = DefaultHasher::new();
            word.hash(&mut hasher);
            vector[(hasher.finish() % Self::DIMS as u64) as usize] += 1.0;
        }
        math::normalize(&mut vector);
        vector
    }
}

#[cfg(test)]
impl Embedder for StubEmbedder {
    fn model_id(&self) -> &str {
        "stub-64"
    }

    fn dims(&self) -> usize {
        Self::DIMS
    }

    fn embed_document(&self, text: &str) -> Result<Vec<f32>> {
        Ok(self.embed(text))
    }

    fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        Ok(self.embed(text))
    }
}
