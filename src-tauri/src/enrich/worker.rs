//! The background job loop, SPEC 5.6.
//!
//! One job at a time, FIFO, surviving restarts. Nothing here ever blocks a
//! capture: the worker only ever reads a note after it is already on disk.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

use crate::state::AppState;
use crate::storage::daily_file::{self, Status};
use crate::storage::{day_path, index, relative_day_path};

use super::model::{Backend, TIMEOUT_SECS};
use super::queue::{queue_path, Job};
use super::runner;

/// Woken when a job is added or a model becomes available.
pub type Wake = Arc<Notify>;

pub fn spawn(app: AppHandle, wake: Wake) {
    tauri::async_runtime::spawn(async move {
        loop {
            // Nothing to do: sleep until something is queued.
            let Some(job) = next_job(&app) else {
                wake.notified().await;
                continue;
            };

            // No model yet is not a failure. Put the job back untouched and
            // wait; a finished download wakes us (SPEC 5.2, SPEC 11).
            let Some(backend) = backend(&app) else {
                put_back_unchanged(&app, job);
                wake.notified().await;
                continue;
            };

            run(&app, job, backend).await;
        }
    });
}

fn next_job(app: &AppHandle) -> Option<Job> {
    let state = app.state::<AppState>();
    let mut queue = state.queue.lock().ok()?;
    queue.pop()
}

/// SPEC 5.1: the model is loaded once, lazily, on the first job, and then
/// stays resident. Returns `None` when there is nothing on disk to load.
fn backend(app: &AppHandle) -> Option<Arc<dyn Backend>> {
    let state = app.state::<AppState>();

    if let Ok(guard) = state.backend.read() {
        if let Some(backend) = guard.clone() {
            return Some(backend);
        }
    }

    let variant = super::download::installed_variant(&state.settings.root)?;
    let path = super::model::model_file(&state.settings.root, variant);
    log::info!("loading {}", path.display());

    match super::llama::LlamaCpp::load(&path) {
        Ok(loaded) => {
            let backend: Arc<dyn Backend> = Arc::new(loaded);
            if let Ok(mut slot) = state.backend.write() {
                *slot = Some(backend.clone());
            }
            state.set_model_status(super::model::ModelStatus::Loaded);
            let _ = app.emit("model-status", serde_json::json!({ "state": "loaded" }));
            Some(backend)
        }
        Err(e) => {
            log::error!("could not load the model: {e}");
            None
        }
    }
}

fn put_back_unchanged(app: &AppHandle, job: Job) {
    let state = app.state::<AppState>();
    // Bound rather than used inline so the guard drops before `state` does.
    let locked = state.queue.lock();
    if let Ok(mut queue) = locked {
        queue.requeue_unchanged(job);
    }
}

async fn run(app: &AppHandle, job: Job, backend: Arc<dyn Backend>) {
    let state = app.state::<AppState>();
    let path = day_path(&state.settings.root, &job.date);

    let Some(note) = read_note(&path, &job) else {
        // The note is gone; so is the job.
        persist_queue(app).await;
        return;
    };

    // SPEC 5.6: never overwrite a note the user has taken over.
    if note.status == Status::Manual {
        log::info!("skipping {}: the user edited it", job.id);
        persist_queue(app).await;
        return;
    }

    let vocabulary = state.vocabulary();
    let body = note.body.clone();
    let started_with = note.hash.clone();

    // Inference is blocking and must never run on the async runtime's
    // reactor, nor outlive the timeout in SPEC 5.6.
    let work = tauri::async_runtime::spawn_blocking(move || {
        runner::enrich(&body, &vocabulary, backend.as_ref())
    });
    let outcome = match tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), work).await {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => Err(format!("enrichment task failed: {e}")),
        Err(_) => Err(format!("enrichment timed out after {TIMEOUT_SECS}s")),
    };

    match outcome {
        Ok(enrichment) => {
            // SPEC 5.6: if the note changed while we were thinking, the answer
            // describes text that no longer exists. Throw it away.
            match read_note(&path, &job) {
                Some(current) if current.hash != started_with => {
                    log::info!("{} changed while enriching, requeuing", job.id);
                    put_back_unchanged(app, job);
                }
                // The user set the subject or tags by hand in the meantime.
                Some(current) if current.status == Status::Manual => {
                    log::info!("{} was edited by hand while enriching, dropping", job.id);
                }
                Some(_) => {
                    write_back(app, &job, &runner::patch(&enrichment)).await;
                    let _ = app.emit("note-enriched", serde_json::json!({ "id": job.id }));
                }
                None => {}
            }
        }
        Err(e) => {
            log::warn!("enriching {} failed: {e}", job.id);
            let backoff = job.backoff_secs();
            let give_up = match state.queue.lock() {
                Ok(mut queue) => !queue.requeue(job.clone()),
                Err(_) => true,
            };
            if give_up {
                log::warn!("giving up on {}", job.id);
                write_back(app, &job, &runner::failed_patch(&note)).await;
                let _ = app.emit("note-enriched", serde_json::json!({ "id": job.id }));
            } else {
                tokio::time::sleep(Duration::from_secs(backoff)).await;
            }
        }
    }

    persist_queue(app).await;
}

fn read_note(path: &std::path::Path, job: &Job) -> Option<daily_file::Note> {
    let contents = std::fs::read_to_string(path).ok()?;
    daily_file::parse_notes(&contents, &job.date, &relative_day_path(&job.date))
        .into_iter()
        .find(|note| note.id == job.id)
}

async fn write_back(app: &AppHandle, job: &Job, patch: &daily_file::NotePatch) {
    let state = app.state::<AppState>();
    let path = day_path(&state.settings.root, &job.date);

    if let Err(e) = state
        .writer
        .update_note(path.clone(), job.id.clone(), patch.clone())
        .await
    {
        log::warn!("could not write enrichment for {}: {e}", job.id);
        return;
    }

    let entries = index::parse_day(&path, &job.date);
    {
        let Ok(mut idx) = state.index.write() else {
            return;
        };
        idx.replace_day(&job.date, entries);
    }
    if let Err(e) = state.persist_index().await {
        log::warn!("could not persist the index after enriching: {e}");
    }
}

async fn persist_queue(app: &AppHandle) {
    let state = app.state::<AppState>();
    let contents = match state.queue.lock() {
        Ok(queue) => queue.to_json(),
        Err(_) => return,
    };
    if let Err(e) = state
        .writer
        .write_index(queue_path(&state.settings.root), contents)
        .await
    {
        log::warn!("could not persist the queue: {e}");
    }
}
