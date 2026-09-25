//! Embeddings for chat retrieval: one vector per note, so a chat can pick the
//! few notes that matter out of thousands instead of reading the whole index.
//!
//! Only the body is embedded, so a vector is tied to the body hash: editing a
//! note's text embeds it again, editing its subject or tags does not.

pub mod llama;
pub mod sync;
pub mod vectors;

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::state::AppState;

/// A model that turns text into vectors.
pub trait Embedder: Send + Sync {
    /// Names the model and its dimensions. `vectors.bin` records it, so
    /// vectors from another model are never compared with this one's.
    fn model_id(&self) -> &str;
    fn dims(&self) -> usize;
    /// A note, embedded as it is.
    fn embed_document(&self, text: &str) -> Result<Vec<f32>, String>;
    /// A question. Qwen3-Embedding puts an instruction before queries and not
    /// before documents, hence a method of its own.
    // Not called yet: the chat embeds its question with it next.
    #[allow(dead_code)]
    fn embed_query(&self, text: &str) -> Result<Vec<f32>, String>;
}

/// The model that embeds notes, or `None` while there is none or the model
/// is switched off: the one switch covers embeddings too.
pub(crate) fn embedder(app: &AppHandle) -> Option<Arc<dyn Embedder>> {
    let state = app.state::<AppState>();
    if !state.model_enabled() {
        return None;
    }
    // Lazy loading goes here when the slot is empty, as in `enrich::worker::backend`.
    let loaded = state.embedder.read().ok()?.clone();
    loaded
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
