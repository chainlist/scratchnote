//! The embedding model the app fetches, and where it lives.

use std::path::PathBuf;

/// A model the app can fetch from Hugging Face: where it is published and
/// the file it is saved as.
pub trait Catalogued: Copy {
    fn repo(self) -> &'static str;
    fn file(self) -> &'static str;
}

/// EmbeddingGemma-300M, which turns notes into vectors for search by
/// meaning. One model, so nothing to choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddingModel;

impl Catalogued for EmbeddingModel {
    /// Google's own repo is gated behind a licence click, which a plain
    /// download cannot do; ggml-org publishes the same weights openly, about
    /// 330 MB at Q8_0.
    fn repo(self) -> &'static str {
        "ggml-org/embeddinggemma-300M-GGUF"
    }

    fn file(self) -> &'static str {
        "embeddinggemma-300M-Q8_0.gguf"
    }
}

/// The embedding model the app shipped before, removed once its successor is
/// in.
pub const LEGACY_EMBEDDING_FILE: &str = "Qwen3-Embedding-0.6B-Q8_0.gguf";

/// Where a downloaded model lives. Outside `notes/` so sync tools can skip it
/// (SPEC 4.1).
pub fn models_dir(root: &std::path::Path) -> PathBuf {
    root.join("models")
}

pub fn model_file(root: &std::path::Path, model: impl Catalogued) -> PathBuf {
    models_dir(root).join(model.file())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These are the exact paths the download hits. A typo here is a 401 at
    /// runtime and nothing earlier, because Hugging Face answers 401 for a
    /// repo that does not exist just as it does for a private one.
    #[test]
    fn the_catalog_points_at_the_repo_that_actually_publishes_the_file() {
        assert_eq!(EmbeddingModel.repo(), "ggml-org/embeddinggemma-300M-GGUF");
        assert_eq!(EmbeddingModel.file(), "embeddinggemma-300M-Q8_0.gguf");
    }

    #[test]
    fn models_live_outside_the_notes_tree() {
        let root = std::path::Path::new("/root");
        let file = model_file(root, EmbeddingModel);
        assert!(file.starts_with(root.join("models")));
        assert!(!file.starts_with(root.join("notes")));
    }
}
