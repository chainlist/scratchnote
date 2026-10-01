//! The background job loop, SPEC 5.6.
//!
//! One job at a time, FIFO, surviving restarts. Nothing here ever blocks a
//! capture: the worker only ever reads a note after it is already on disk.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

use crate::spaces::Space;
use crate::state::AppState;
use crate::storage::activity::{self, Action, Actor, Event};
use crate::storage::daily_file::{self, Kind, Status};
use crate::storage::index::IndexEntry;
use crate::storage::{day_path, index, page_file, relative_day_path};

use super::model::{Backend, TIMEOUT_SECS};
use super::queue::Job;
use super::runner;

/// Woken when a job is added or a model becomes available.
pub type Wake = Arc<Notify>;

pub fn spawn(app: AppHandle, wake: Wake) {
    tauri::async_runtime::spawn(async move {
        loop {
            // Switched off: leave the queue alone until settings turn it on.
            if !app.state::<AppState>().model_enabled() {
                set_busy(&app, false);
                wake.notified().await;
                continue;
            }

            // Nothing to do: sleep until something is queued.
            let Some((space, job)) = next_job(&app) else {
                set_busy(&app, false);
                wake.notified().await;
                continue;
            };

            // Set before the load so the first job after startup or an idle
            // unload shows as work too.
            set_busy(&app, true);

            // No model yet is not a failure. Put the job back untouched and
            // wait; a finished download wakes us (SPEC 5.2, SPEC 11).
            let Some(backend) = backend(&app) else {
                put_back_unchanged(&space, job);
                set_busy(&app, false);
                wake.notified().await;
                continue;
            };

            let state = app.state::<AppState>();
            state.mark_used();
            let _ = app.emit("enrich-progress", state.progress());
            run(&app, &space, job, backend).await;
            state
                .batch_done
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            state.mark_used();
        }
    });
}

/// Cleared only once the queue is empty, and emitted only on a change, so a
/// run of queued jobs reads as one stretch of work.
fn set_busy(app: &AppHandle, busy: bool) {
    let state = app.state::<AppState>();
    if state.busy.swap(busy, std::sync::atomic::Ordering::SeqCst) != busy {
        let _ = app.emit("enrich-busy", busy);
    }
    // A new stretch of work counts from 1 again.
    if !busy {
        state
            .batch_done
            .store(0, std::sync::atomic::Ordering::SeqCst);
    }
}

/// The open space's notes go first, then the other spaces' in turn.
fn next_job(app: &AppHandle) -> Option<(Arc<Space>, Job)> {
    let state = app.state::<AppState>();
    state.all_spaces().into_iter().find_map(|space| {
        let job = space.queue.lock().ok()?.pop()?;
        Some((space, job))
    })
}

/// SPEC 5.1: the model is loaded once, lazily, on the first job, and then
/// stays resident. Returns `None` when there is nothing on disk to load, or
/// the model is switched off. An ask loads it the same way when no job has
/// yet. One load runs at a time: a chat opened while a job loads the model
/// waits for that one.
pub(crate) fn backend(app: &AppHandle) -> Option<Arc<dyn Backend>> {
    let state = app.state::<AppState>();
    if !state.model_enabled() {
        return None;
    }

    crate::state::load_once(&state.backend, &state.backend_loading, || {
        // Read under the lock `update_model` takes to set it, so no load
        // starts once a swap has begun.
        if state.swapping.load(std::sync::atomic::Ordering::SeqCst) {
            return None;
        }
        let path = state.active_model()?.path;
        log::info!("loading {}", path.display());

        match super::llama::LlamaCpp::load_with(&path, state.use_gpu()) {
            Ok(loaded) => {
                state.set_model_status(super::model::ModelStatus::Loaded);
                let _ = app.emit("model-status", serde_json::json!({ "state": "loaded" }));
                let backend: Arc<dyn Backend> = Arc::new(loaded);
                Some(backend)
            }
            Err(e) => {
                log::error!("could not load the model: {e}");
                None
            }
        }
    })
}

fn put_back_unchanged(space: &Space, job: Job) {
    if let Ok(mut queue) = space.queue.lock() {
        queue.requeue_unchanged(job);
    }
}

async fn run(app: &AppHandle, space: &Space, job: Job, backend: Arc<dyn Backend>) {
    let state = app.state::<AppState>();
    let Some((place, note)) = locate(space, &job) else {
        // The note is gone; so is the job.
        space.persist_queue(&state.writer).await;
        return;
    };

    // SPEC 5.6: never overwrite a note the user has taken over.
    if note.status == Status::Manual {
        log::info!("skipping {}: the user edited it", job.id);
        space.persist_queue(&state.writer).await;
        return;
    }

    // A page open in the editor is queued again when its view closes.
    if space.is_held(&job.id) {
        log::info!("skipping {}: the page is being edited", job.id);
        space.persist_queue(&state.writer).await;
        return;
    }

    let language = state.note_language();
    let categories = space.categories();
    // A page is labelled from its title too (SPEC 5.6).
    let body = match &place {
        Place::Day(_) => note.body.clone(),
        Place::Page(..) => format!(
            "# {}\n\n{}",
            note.subject.as_deref().unwrap_or_default(),
            note.body
        ),
    };
    let started_with = note.hash.clone();

    // Inference is blocking and must never run on the async runtime's
    // reactor, nor outlive the timeout in SPEC 5.6.
    let work = tauri::async_runtime::spawn_blocking(move || {
        runner::enrich(&body, language, &categories, backend.as_ref())
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
            match read_note(&place, &job) {
                // A page is queued again by whatever changed it: its view
                // closing, or the watcher for an edit made elsewhere.
                Some(current) if current.hash != started_with && current.kind == Kind::Page => {
                    log::info!("{} changed while enriching, dropping", job.id);
                }
                Some(current) if current.hash != started_with => {
                    log::info!("{} changed while enriching, requeuing", job.id);
                    put_back_unchanged(space, job);
                }
                // The user set the subject or category by hand in the meantime.
                Some(current) if current.status == Status::Manual => {
                    log::info!("{} was edited by hand while enriching, dropping", job.id);
                }
                Some(_) => {
                    let patch = runner::patch(&enrichment, language);
                    if write_back(&state, space, &job, &place, &patch).await {
                        if let Some(event) = labelled(&note, &patch) {
                            space.record(&state.writer, event).await;
                        }
                    }
                    emit_enriched(app, space, &job);
                }
                None => {}
            }
        }
        Err(e) => {
            log::warn!("enriching {} failed: {e}", job.id);
            let backoff = job.backoff_secs();
            let give_up = match space.queue.lock() {
                Ok(mut queue) => !queue.requeue(job.clone()),
                Err(_) => true,
            };
            if give_up {
                log::warn!("giving up on {}", job.id);
                if write_back(&state, space, &job, &place, &runner::failed_patch(&note)).await {
                    let failed = Event::of_note(Actor::Model, Action::Fail, &note);
                    space.record(&state.writer, failed).await;
                }
                emit_enriched(app, space, &job);
            } else {
                tokio::time::sleep(Duration::from_secs(backoff)).await;
            }
        }
    }

    space.persist_queue(&state.writer).await;
}

/// Where a job's note lives: a block in its day's file, or a page's own
/// file (SPEC 4.7).
enum Place {
    Day(PathBuf),
    /// The page's file, and that path relative to the space's root.
    Page(PathBuf, String),
}

fn place(space: &Space, job: &Job) -> Place {
    let page = space
        .index
        .read()
        .ok()
        .and_then(|idx| idx.page(&job.id).map(|page| page.file.clone()));
    match page {
        Some(file) => Place::Page(space.root.join(&file), file),
        None => Place::Day(day_path(&space.root, &job.date)),
    }
}

/// Where a job's note is, and the note as its file holds it now. A space
/// that is not open has no index to tell its pages' jobs apart, so its page
/// files are looked through when the day's file does not hold the note.
fn locate(space: &Space, job: &Job) -> Option<(Place, daily_file::Note)> {
    let place = place(space, job);
    if let Some(note) = read_note(&place, job) {
        return Some((place, note));
    }
    if space.is_open() {
        return None;
    }
    let (path, file) = index::page_files(&space.root)
        .into_iter()
        .find_map(|path| {
            let page = index::parse_page(&space.root, &path)?;
            (page.id == job.id).then_some((path, page.file))
        })?;
    let place = Place::Page(path, file);
    let note = read_note(&place, job)?;
    Some((place, note))
}

fn read_note(place: &Place, job: &Job) -> Option<daily_file::Note> {
    match place {
        Place::Day(path) => {
            let contents = std::fs::read_to_string(path).ok()?;
            daily_file::parse_notes(&contents, &job.date, &relative_day_path(&job.date))
                .into_iter()
                .find(|note| note.id == job.id)
        }
        Place::Page(path, file) => {
            let contents = std::fs::read_to_string(path).ok()?;
            page_file::parse_page(&contents, file).filter(|page| page.id == job.id)
        }
    }
}

/// What the model's labels changed, for the activity log (SPEC 4.11).
/// `None` when they came out as they were.
fn labelled(note: &daily_file::Note, patch: &daily_file::NotePatch) -> Option<Event> {
    let before = IndexEntry::from(note);
    let after = IndexEntry {
        // A page's title is the user's.
        subject: match note.kind {
            Kind::Note => patch.subject.clone(),
            Kind::Page => before.subject.clone(),
        },
        category: patch.category.clone(),
        ..before.clone()
    };
    let (action, changes) = activity::compare(&before, &after)?;
    Some(Event::of_entry(Actor::Model, action, &after).with_changes(changes))
}

fn emit_enriched(app: &AppHandle, space: &Space, job: &Job) {
    let _ = app.emit(
        "note-enriched",
        serde_json::json!({ "id": job.id, "space": space.name }),
    );
}

/// Write the model's labels into the note's file. False when they could not
/// be written.
async fn write_back(
    state: &AppState,
    space: &Space,
    job: &Job,
    place: &Place,
    patch: &daily_file::NotePatch,
) -> bool {
    // A space renamed or deleted mid-job: its notes are no longer at this
    // path, and a pending note is picked up again where they went.
    if space.is_retired() {
        return false;
    }

    match place {
        Place::Day(path) => {
            if let Err(e) = state
                .writer
                .update_note(path.clone(), job.id.clone(), patch.clone())
                .await
            {
                log::warn!("could not write enrichment for {}: {e}", job.id);
                return false;
            }
            if space.day_changed(&job.date).is_err() {
                return true;
            }
        }
        // Only the category is the model's to write; the title is the user's.
        Place::Page(path, file) => {
            let (file, patch) = (file.clone(), patch.clone());
            if let Err(e) = state
                .writer
                .rewrite(path.clone(), move |existing| {
                    page_file::update_meta(
                        existing?,
                        &file,
                        patch.category,
                        patch.status,
                        patch.lang,
                    )
                })
                .await
            {
                log::warn!("could not write enrichment for {}: {e}", job.id);
                return false;
            }
            let Some(page) = index::parse_page(&space.root, path) else {
                return true;
            };
            if space.page_changed(&page).is_err() {
                return true;
            }
        }
    }
    if let Err(e) = space.persist_index(&state.writer).await {
        log::warn!("could not persist the index after enriching: {e}");
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::daily_file::{body_hash, Note};

    #[test]
    fn a_space_not_open_finds_the_page_a_job_is_for_among_its_files() {
        let root = std::env::temp_dir().join("scratchnote-worker-closed-page");
        let _ = std::fs::remove_dir_all(&root);
        let date = "2026-09-22";
        let page = Note {
            id: "01PPP".to_string(),
            date: date.to_string(),
            time: "10:00".to_string(),
            file: page_file::relative_path(date, &page_file::file_name(date, "Weekly sync", 1)),
            subject: Some("Weekly sync".to_string()),
            category: None,
            status: Status::Pending,
            hash: body_hash("the meeting"),
            lang: None,
            body: "the meeting".to_string(),
            kind: Kind::Page,
            missing: false,
        };
        let path = root.join(&page.file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, page_file::render_page(&page)).unwrap();

        let space = Space::new("Test", root.clone(), Arc::new(Notify::new()));
        let (place, note) = locate(&space, &Job::new("01PPP", date)).expect("the page is found");
        assert!(matches!(place, Place::Page(_, file) if file == page.file));
        assert_eq!(note.body, "the meeting");

        // A note that is nowhere still costs its job.
        assert!(locate(&space, &Job::new("01ZZZ", date)).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_log_hears_of_labels_only_when_they_change() {
        let note = Note {
            id: "01AAA".to_string(),
            date: "2026-09-22".to_string(),
            time: "14:32".to_string(),
            file: relative_day_path("2026-09-22"),
            subject: None,
            category: None,
            status: Status::Pending,
            hash: body_hash("pin the chart"),
            lang: None,
            body: "pin the chart".to_string(),
            kind: Kind::Note,
            missing: false,
        };
        let patch = daily_file::NotePatch {
            subject: Some("Pin the chart".to_string()),
            category: Some("infrastructure".to_string()),
            status: Status::Done,
            lang: Some("en".to_string()),
        };
        let event = labelled(&note, &patch).expect("a first label");
        assert_eq!(event.actor, Actor::Model);
        assert_eq!(event.action, Action::Label);
        assert_eq!(event.subject.as_deref(), Some("Pin the chart"));
        assert_eq!(
            event.changes.keys().copied().collect::<Vec<_>>(),
            ["category", "subject"]
        );

        let relabelled = Note {
            subject: patch.subject.clone(),
            category: patch.category.clone(),
            ..note.clone()
        };
        assert!(labelled(&relabelled, &patch).is_none(), "nothing changed");

        // A page keeps its title, so only its category is the model's.
        let page = Note {
            kind: Kind::Page,
            subject: Some("Weekly sync".to_string()),
            ..note
        };
        let event = labelled(&page, &patch).unwrap();
        assert_eq!(event.subject.as_deref(), Some("Weekly sync"));
        assert_eq!(
            event.changes.keys().copied().collect::<Vec<_>>(),
            ["category"]
        );
    }
}
