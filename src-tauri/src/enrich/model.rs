//! The model behind enrichment.
//!
//! Everything above this file talks to the `Backend` trait, which is what lets
//! the golden tests in SPEC 12 run a stub that returns fixed JSON, and what
//! keeps the app fully usable with no model installed (SPEC 11).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// SPEC 5.1.
pub const CONTEXT_TOKENS: u32 = 4096;
/// The context a chat reads the whole index in, with room for the talk.
/// Around 200 notes fit; its cache costs about 1.2 GB with the 4B model, and
/// only once a chat has started.
pub const LONG_CONTEXT_TOKENS: u32 = 8192;
pub const TEMPERATURE: f32 = 0.2;
pub const MAX_OUTPUT_TOKENS: u32 = 200;
/// SPEC 5.6: a job that takes longer than this counts as a failure.
pub const TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Variant {
    /// Qwen3-4B-Instruct-2507, the default.
    Default,
    /// Qwen3-1.7B, thinking disabled through the chat template.
    Light,
}

impl Variant {
    /// Qwen publishes these models as safetensors, not GGUF, so the
    /// quantised builds come from unsloth, which ships both at the sizes the
    /// spec quotes. Qwen's own `Qwen3-1.7B-GGUF` only has a Q8_0.
    pub fn repo(self) -> &'static str {
        match self {
            Variant::Default => "unsloth/Qwen3-4B-Instruct-2507-GGUF",
            Variant::Light => "unsloth/Qwen3-1.7B-GGUF",
        }
    }

    pub fn file(self) -> &'static str {
        match self {
            Variant::Default => "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
            Variant::Light => "Qwen3-1.7B-Q4_K_M.gguf",
        }
    }
}

/// SPEC 8. `Absent` is a normal state, not an error.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum ModelStatus {
    Absent,
    Downloading {
        percent: u8,
    },
    Loaded,
    Idle,
    /// Turned off in settings: nothing is loaded, notes stay queued, and
    /// chat is unavailable, whether or not a model is on disk.
    Disabled,
}

/// Turns a prompt into text. The grammar is passed through to the sampler so
/// the output cannot be anything but the enrichment object.
pub trait Backend: Send + Sync {
    fn generate(&self, prompt: &str, grammar: &str) -> Result<String, String>;

    /// Free text, no grammar, handed to `on_piece` as it is written, in a
    /// larger context of its own whose cache enrichment never touches. A
    /// long start that stays the same from one call to the next, such as the
    /// whole index, is then only read once. Stops early when `on_piece`
    /// returns false, and returns what was written.
    ///
    /// Stubs answer in one piece, from `generate`.
    fn stream_long(
        &self,
        prompt: &str,
        _max_tokens: u32,
        on_piece: &mut dyn FnMut(&str) -> bool,
    ) -> Result<String, String> {
        let out = self.generate(prompt, "")?;
        on_piece(&out);
        Ok(out)
    }

    /// Read a prompt into that context's cache without answering, so a
    /// `stream_long` that starts with it only reads the rest.
    fn prefill_long(&self, _prompt: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Returns fixed JSON without loading anything, for the golden tests in
/// SPEC 12.
#[cfg(test)]
pub struct StubBackend {
    response: String,
}

#[cfg(test)]
impl StubBackend {
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
        }
    }
}

#[cfg(test)]
impl Backend for StubBackend {
    fn generate(&self, _prompt: &str, _grammar: &str) -> Result<String, String> {
        Ok(self.response.clone())
    }
}

/// Where a downloaded model lives. Outside `notes/` so sync tools can skip it
/// (SPEC 4.1).
pub fn models_dir(root: &std::path::Path) -> PathBuf {
    root.join("models")
}

pub fn model_file(root: &std::path::Path, variant: Variant) -> PathBuf {
    models_dir(root).join(variant.file())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These are the exact paths the download hits. A typo here is a 401 at
    /// runtime and nothing earlier, because Hugging Face answers 401 for a
    /// repo that does not exist just as it does for a private one.
    #[test]
    fn the_catalog_points_at_the_repos_that_actually_publish_these_files() {
        assert_eq!(
            Variant::Default.repo(),
            "unsloth/Qwen3-4B-Instruct-2507-GGUF"
        );
        assert_eq!(
            Variant::Default.file(),
            "Qwen3-4B-Instruct-2507-Q4_K_M.gguf"
        );
        assert_eq!(Variant::Light.repo(), "unsloth/Qwen3-1.7B-GGUF");
        assert_eq!(Variant::Light.file(), "Qwen3-1.7B-Q4_K_M.gguf");
    }

    #[test]
    fn each_variant_names_a_repo_and_a_quantised_file() {
        for variant in [Variant::Default, Variant::Light] {
            assert!(variant.repo().contains('/'), "{:?}", variant);
            assert!(variant.file().ends_with(".gguf"), "{:?}", variant);
            assert!(variant.file().contains("Q4_K_M"), "{:?}", variant);
        }
    }

    #[test]
    fn the_stub_returns_what_it_was_given() {
        let stub = StubBackend::new(r#"{"subject":"a","summary":"b","tags":["c"]}"#);
        assert_eq!(
            stub.generate("any prompt", "any grammar").unwrap(),
            r#"{"subject":"a","summary":"b","tags":["c"]}"#
        );
    }

    #[test]
    fn models_live_outside_the_notes_tree() {
        let root = std::path::Path::new("/root");
        let file = model_file(root, Variant::Light);
        assert!(file.starts_with(root.join("models")));
        assert!(!file.starts_with(root.join("notes")));
    }

    #[test]
    fn status_serialises_the_way_the_frontend_reads_it() {
        let json = serde_json::to_string(&ModelStatus::Downloading { percent: 42 }).unwrap();
        assert_eq!(json, r#"{"state":"downloading","percent":42}"#);
        assert_eq!(
            serde_json::to_string(&ModelStatus::Absent).unwrap(),
            r#"{"state":"absent"}"#
        );
    }
}
