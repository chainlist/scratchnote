//! Embeddings: one vector per note, for search by meaning, similar notes,
//! recall and threads.
//!
//! Only the body is embedded, so a vector is tied to the body hash: editing a
//! note's text embeds it again, renaming a page does not.

#[cfg(test)]
mod bench;
pub mod download;
pub mod llama;
pub mod model;
#[cfg(test)]
pub mod samples;
pub mod sync;
pub mod threads;
pub mod vectors;

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::state::AppState;
use model::{model_file, EmbeddingModel};

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

/// The model that embeds notes, or `None` while there is none. Loaded on
/// first use, then kept until the app quits, since it is small and every new
/// note and search by meaning needs it. A load that fails is tried again on
/// the next call, which comes with the next change to a note or the next
/// search, never in a loop. One load runs at a time: a search that comes
/// while the embed task loads the model waits for that one.
pub(crate) fn embedder(app: &AppHandle) -> Option<Arc<dyn Embedder>> {
    let state = app.state::<AppState>();
    crate::state::load_once(&state.embedder, &state.embedder_loading, || {
        if !download::is_installed(&state.root, EmbeddingModel) {
            return None;
        }
        let path = model_file(&state.root, EmbeddingModel);
        log::info!("loading {}", path.display());

        match llama::LlamaEmbedder::load(&path) {
            Ok(loaded) => {
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

/// The embedding model for the tests that need real weights: the file
/// `SCRATCHNOTE_EMBEDDING_MODEL` names, else the one the app downloads into
/// `~/Scratchnote/models`. `None`, and the test skips, without either.
#[cfg(test)]
pub fn installed_embedder() -> Option<llama::LlamaEmbedder> {
    let path = match std::env::var_os("SCRATCHNOTE_EMBEDDING_MODEL") {
        Some(path) => std::path::PathBuf::from(path),
        None => {
            let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
            model_file(
                &std::path::PathBuf::from(home).join("Scratchnote"),
                EmbeddingModel,
            )
        }
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
