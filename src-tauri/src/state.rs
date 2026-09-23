use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crate::enrich::model::{Backend, ModelStatus};
use crate::enrich::normalize::Vocabulary;
use crate::enrich::queue::Queue;
use crate::enrich::worker::Wake;
use crate::settings::Settings;
use crate::storage::index::{self, Index};
use crate::storage::tags;
use crate::storage::writer::Writer;

pub struct AppState {
    pub settings: Settings,
    pub writer: Writer,
    /// The derived cache from SPEC 4.4. Guards are held only for the length of
    /// a read or a swap, never across an await.
    pub index: RwLock<Index>,
    /// The user's half of `tags.json` (SPEC 4.5), read at startup and again on
    /// a rebuild.
    pub aliases: RwLock<HashMap<String, String>>,
    /// Pending enrichment jobs (SPEC 5.6).
    pub queue: Mutex<Queue>,
    /// `None` until a model is loaded. Absent is a normal state: capture,
    /// browsing and search all work without one (SPEC 11).
    pub backend: RwLock<Option<Arc<dyn Backend>>>,
    pub model_status: RwLock<ModelStatus>,
    /// Nudges the worker when a job is queued or a model becomes available.
    pub wake: Wake,
}

impl AppState {
    pub fn set_model_status(&self, status: ModelStatus) {
        if let Ok(mut current) = self.model_status.write() {
            *current = status;
        }
    }

    pub fn aliases(&self) -> HashMap<String, String> {
        self.aliases
            .read()
            .map(|aliases| aliases.clone())
            .unwrap_or_default()
    }

    /// What enrichment normalises against (SPEC 5.5).
    pub fn vocabulary(&self) -> Vocabulary {
        Vocabulary {
            counts: self
                .index
                .read()
                .map(|idx| idx.tag_counts())
                .unwrap_or_default(),
            aliases: self.aliases(),
        }
    }

    /// Write `index.jsonl` and `tags.json` from what is in memory. The tag
    /// counts are derived from the index, so they are rewritten whenever it is
    /// and the two never drift apart.
    pub async fn persist_index(&self) -> Result<(), String> {
        let (jsonl, counts) = {
            let idx = self
                .index
                .read()
                .map_err(|_| "index lock poisoned".to_string())?;
            (idx.to_jsonl(), idx.tag_counts())
        };
        let root = &self.settings.root;
        self.writer
            .write_index(index::index_path(root), jsonl)
            .await?;
        self.writer
            .write_index(
                tags::tags_path(root),
                tags::render(&counts, &self.aliases()),
            )
            .await
    }

    pub fn model_status(&self) -> ModelStatus {
        self.model_status
            .read()
            .map(|status| status.clone())
            .unwrap_or(ModelStatus::Absent)
    }
}
