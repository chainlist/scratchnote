//! Embeddings for chat retrieval: one vector per note, so a chat can pick the
//! few notes that matter out of thousands instead of reading the whole index.
//!
//! Only the body is embedded, so a vector is tied to the body hash: editing a
//! note's text embeds it again, editing its subject or category does not.

pub mod llama;
pub mod sync;
pub mod vectors;

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::enrich::download;
use crate::enrich::model::{model_file, EmbeddingModel};
use crate::state::AppState;

/// A model that turns text into vectors.
pub trait Embedder: Send + Sync {
    /// Names the model, its dimensions and how it embeds a note.
    /// `vectors.bin` records it, so vectors made another way are never
    /// compared with this one's.
    fn model_id(&self) -> &str;
    fn dims(&self) -> usize;
    /// A note, embedded as it is.
    fn embed_document(&self, text: &str) -> Result<Vec<f32>, String>;
    /// A question. EmbeddingGemma frames queries and documents differently,
    /// hence a method of its own.
    fn embed_query(&self, text: &str) -> Result<Vec<f32>, String>;
}

/// The model that embeds notes, or `None` while there is none or the model
/// is switched off: the one switch covers embeddings too. Loaded on first
/// use, as `enrich::worker::backend` loads the chat model, and dropped with
/// it by `AppState::unload_model`. A load that fails is tried again on the
/// next call, which comes with the next change to a note or the next chat
/// message, never in a loop. One load runs at a time: a search that comes
/// while the embed task loads the model waits for that one.
pub(crate) fn embedder(app: &AppHandle) -> Option<Arc<dyn Embedder>> {
    let state = app.state::<AppState>();
    if !state.model_enabled() {
        return None;
    }
    crate::state::load_once(&state.embedder, &state.embedder_loading, || {
        if !download::is_installed(&state.root, EmbeddingModel) {
            return None;
        }
        let path = model_file(&state.root, EmbeddingModel);
        log::info!("loading {}", path.display());

        match llama::LlamaEmbedder::load_with(&path, state.use_gpu()) {
            Ok(loaded) => {
                // Otherwise the idle unload counts from before the load.
                state.mark_used();
                let loaded: Arc<dyn Embedder> = Arc::new(loaded);
                Some(loaded)
            }
            Err(e) => {
                log::error!("could not load the embedding model: {e}");
                None
            }
        }
    })
}

/// Scale to unit length, so similarity is a plain dot product. A zero vector
/// has no direction and is left as it is: it scores zero against anything.
pub fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in vector {
            *x /= norm;
        }
    }
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
        normalize(&mut vector);
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

    fn embed_document(&self, text: &str) -> Result<Vec<f32>, String> {
        Ok(self.embed(text))
    }

    fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
        Ok(self.embed(text))
    }
}
