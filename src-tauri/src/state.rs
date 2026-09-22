use std::sync::{Arc, Mutex, RwLock};

use crate::enrich::model::{Backend, ModelStatus};
use crate::enrich::queue::Queue;
use crate::enrich::worker::Wake;
use crate::settings::Settings;
use crate::storage::index::Index;
use crate::storage::writer::Writer;

pub struct AppState {
    pub settings: Settings,
    pub writer: Writer,
    /// The derived cache from SPEC 4.4. Guards are held only for the length of
    /// a read or a swap, never across an await.
    pub index: RwLock<Index>,
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

    pub fn model_status(&self) -> ModelStatus {
        self.model_status
            .read()
            .map(|status| status.clone())
            .unwrap_or(ModelStatus::Absent)
    }
}
