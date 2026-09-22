//! The model behind enrichment.
//!
//! Everything above this file talks to the `Backend` trait, which is what lets
//! the golden tests in SPEC 12 run a stub that returns fixed JSON, and what
//! keeps the app fully usable with no model installed (SPEC 11).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// SPEC 5.1.
pub const CONTEXT_TOKENS: u32 = 4096;
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
    pub fn repo(self) -> &'static str {
        match self {
            Variant::Default => "Qwen/Qwen3-4B-Instruct-2507-GGUF",
            Variant::Light => "Qwen/Qwen3-1.7B-GGUF",
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
    Downloading { percent: u8 },
    Loaded,
    Idle,
}

/// Turns a prompt into text. The grammar is passed through to the sampler so
/// the output cannot be anything but the enrichment object.
pub trait Backend: Send + Sync {
    fn generate(&self, prompt: &str, grammar: &str) -> Result<String, String>;
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
