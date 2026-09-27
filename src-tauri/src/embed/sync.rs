//! Keeps each space's vectors in line with its index.
//!
//! Rather than hooking every way a note can change, one task compares the
//! index with the vectors whenever it is woken: whatever the index holds that
//! the vectors do not is embedded, whatever it lost is forgotten. A missed
//! wake only delays a note until the next one.

use std::path::Path;
use std::sync::{Mutex, RwLock};

use tauri::{AppHandle, Manager};

use super::vectors::{vectors_path, Vectors};
use super::Embedder;
use crate::enrich::worker::Wake;
use crate::spaces::Space;
use crate::state::AppState;
use crate::storage::index::Index;

/// One task for every space, so only one ever saves a space's `vectors.bin`.
pub fn spawn(app: AppHandle, wake: Wake) {
    tauri::async_runtime::spawn(async move {
        loop {
            // A wake during a pass is kept as a permit, so the notes saved
            // meanwhile get a pass of their own straight after.
            wake.notified().await;
            let app = app.clone();
            // Loading the model and embedding both block, so they stay off
            // the async runtime.
            let pass = tauri::async_runtime::spawn_blocking(move || {
                let Some(embedder) = super::embedder(&app) else {
                    return;
                };
                let state = app.state::<AppState>();
                // Stamped as the worker stamps a job, so the idle unload
                // counts embedding as use.
                state.mark_used();
                for space in state.all_spaces() {
                    sync_space(&state, &space, embedder.as_ref());
                }
                state.mark_used();
            });
            if let Err(e) = pass.await {
                log::warn!("embedding notes failed: {e}");
            }
        }
    });
}

fn sync_space(state: &AppState, space: &Space, embedder: &dyn Embedder) {
    if space.is_retired() {
        return;
    }
    let path = vectors_path(&space.root);
    let keep_going = || !space.is_retired() && state.model_enabled();
    match reconcile(&space.index, &space.vectors, &path, embedder, keep_going) {
        // A space renamed or deleted meanwhile: its folder is gone from here.
        Ok(true) if !space.is_retired() => {
            let Ok(vectors) = space.vectors.lock() else {
                return;
            };
            if let Some(vectors) = vectors.as_ref() {
                match vectors.save(&path) {
                    Ok(()) => log::info!("updated the vectors of {}", space.name),
                    Err(e) => log::warn!("could not save the vectors of {}: {e}", space.name),
                }
            }
        }
        Ok(_) => {}
        Err(e) => log::warn!("could not embed the notes of {}: {e}", space.name),
    }
}

/// A first pass over thousands of notes takes minutes, so it saves as it
/// goes: quitting midway then loses at most this many notes, not the pass.
/// Each save rewrites the whole file, hence not after every note.
const SAVE_EVERY: usize = 500;

/// Embed every note the vectors lack or hold for an older body, and forget
/// notes the index no longer has. The store is loaded from `path` first if
/// it is not in memory yet or holds another model's vectors, and saved there
/// every `SAVE_EVERY` notes. Stops before the next note once `keep_going`
/// says so. True when the vectors hold changes not saved yet.
pub fn reconcile(
    index: &RwLock<Index>,
    vectors: &Mutex<Option<Vectors>>,
    path: &Path,
    embedder: &dyn Embedder,
    keep_going: impl Fn() -> bool,
) -> Result<bool, String> {
    let (model_id, dims) = (embedder.model_id(), embedder.dims());

    // The work is cloned out so that neither lock is held while the model
    // runs: the index is wanted by every command, the vectors by the chat.
    let (mut changed, todo) = {
        let mut slot = vectors
            .lock()
            .map_err(|_| "vectors lock poisoned".to_string())?;
        if slot.as_ref().is_some_and(|v| !v.is_from(model_id, dims)) {
            *slot = None;
        }
        let store = slot.get_or_insert_with(|| Vectors::load(path, model_id, dims));

        let index = index
            .read()
            .map_err(|_| "index lock poisoned".to_string())?;
        let present = index.entries().map(|e| e.id.clone()).collect();
        let dropped = store.retain(&present);
        let todo: Vec<(String, String, String)> = store
            .stale(index.entries())
            .into_iter()
            .map(|e| (e.id.clone(), e.hash.clone(), e.body.clone()))
            .collect();
        (dropped, todo)
    };

    let mut embedded = 0;
    for (id, hash, body) in todo {
        if !keep_going() {
            break;
        }
        // One note failing, too long say, must not hold back the rest. It is
        // tried again on the next pass.
        let vector = match embedder.embed_document(&body) {
            Ok(vector) => vector,
            Err(e) => {
                log::warn!("could not embed {id}: {e}");
                continue;
            }
        };
        let mut slot = vectors
            .lock()
            .map_err(|_| "vectors lock poisoned".to_string())?;
        if let Some(store) = slot.as_mut() {
            match store.insert(id.clone(), hash, vector) {
                Ok(()) => changed = true,
                Err(e) => log::warn!("could not store the vector of {id}: {e}"),
            }
            embedded += 1;
            // `keep_going` is false for a retired space, whose folder is gone.
            if embedded % SAVE_EVERY == 0 && keep_going() {
                match store.save(path) {
                    Ok(()) => changed = false,
                    Err(e) => log::warn!("could not save the vectors midway: {e}"),
                }
            }
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::StubEmbedder;
    use crate::storage::daily_file::{body_hash, Note, Status};
    use crate::storage::index::IndexEntry;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// The stub under any model id, counting the notes it embeds.
    struct Counting {
        model_id: &'static str,
        calls: AtomicUsize,
    }

    impl Counting {
        fn new(model_id: &'static str) -> Self {
            Self {
                model_id,
                calls: AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl Embedder for Counting {
        fn model_id(&self) -> &str {
            self.model_id
        }

        fn dims(&self) -> usize {
            StubEmbedder.dims()
        }

        fn embed_document(&self, text: &str) -> Result<Vec<f32>, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            StubEmbedder.embed_document(text)
        }

        fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
            StubEmbedder.embed_query(text)
        }
    }

    fn entry(id: &str, body: &str) -> IndexEntry {
        IndexEntry::from(&Note {
            id: id.to_string(),
            date: "2026-09-22".to_string(),
            time: "08:00".to_string(),
            file: "notes/2026/2026-09-22.md".to_string(),
            subject: None,
            category: None,
            status: Status::Done,
            hash: body_hash(body),
            lang: None,
            body: body.to_string(),
        })
    }

    fn index_of(notes: &[(&str, &str)]) -> RwLock<Index> {
        let mut index = Index::default();
        for (id, body) in notes {
            index.push(entry(id, body));
        }
        RwLock::new(index)
    }

    fn scratch_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("scratchnote-sync-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn ids(vectors: &Mutex<Option<Vectors>>) -> Vec<String> {
        let slot = vectors.lock().unwrap();
        let store = slot.as_ref().unwrap();
        let mut ids: Vec<String> = store
            .top(&vec![1.0; StubEmbedder.dims()], usize::MAX)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        ids.sort();
        ids
    }

    #[test]
    fn embeds_new_and_edited_notes_and_leaves_the_rest_alone() {
        let root = scratch_root("edits");
        let path = vectors_path(&root);
        let embedder = Counting::new("stub-64");
        let vectors = Mutex::new(None);

        let index = index_of(&[("01A", "coffee beans"), ("01B", "deploy on friday")]);
        assert!(reconcile(&index, &vectors, &path, &embedder, || true).unwrap());
        assert_eq!(embedder.calls(), 2);

        // Nothing changed, so nothing is embedded and nothing is reported.
        assert!(!reconcile(&index, &vectors, &path, &embedder, || true).unwrap());
        assert_eq!(embedder.calls(), 2);

        // 01B edited, 01C new: only those two go through the model.
        let index = index_of(&[
            ("01A", "coffee beans"),
            ("01B", "deploy on monday"),
            ("01C", "grandma's birthday"),
        ]);
        assert!(reconcile(&index, &vectors, &path, &embedder, || true).unwrap());
        assert_eq!(embedder.calls(), 4);
        assert_eq!(ids(&vectors), vec!["01A", "01B", "01C"]);
    }

    #[test]
    fn a_blank_note_is_not_embedded() {
        let root = scratch_root("blank");
        let embedder = Counting::new("stub-64");
        let vectors = Mutex::new(None);

        let index = index_of(&[("01A", "  \n")]);
        let changed = reconcile(&index, &vectors, &vectors_path(&root), &embedder, || true);
        assert!(!changed.unwrap());
        assert_eq!(embedder.calls(), 0);
    }

    #[test]
    fn a_deleted_note_is_forgotten() {
        let root = scratch_root("deleted");
        let path = vectors_path(&root);
        let embedder = Counting::new("stub-64");
        let vectors = Mutex::new(None);

        let index = index_of(&[("01A", "coffee beans"), ("01B", "deploy on friday")]);
        reconcile(&index, &vectors, &path, &embedder, || true).unwrap();

        let index = index_of(&[("01A", "coffee beans")]);
        assert!(reconcile(&index, &vectors, &path, &embedder, || true).unwrap());
        assert_eq!(embedder.calls(), 2);
        assert_eq!(ids(&vectors), vec!["01A"]);
    }

    #[test]
    fn stops_before_the_next_note_when_told_to() {
        let root = scratch_root("stop");
        let path = vectors_path(&root);
        let embedder = Counting::new("stub-64");
        let vectors = Mutex::new(None);
        let index = index_of(&[("01A", "one"), ("01B", "two"), ("01C", "three")]);

        let changed = reconcile(&index, &vectors, &path, &embedder, || embedder.calls() < 1);
        assert!(changed.unwrap());
        assert_eq!(embedder.calls(), 1);

        // The next pass carries on where that one stopped.
        reconcile(&index, &vectors, &path, &embedder, || true).unwrap();
        assert_eq!(embedder.calls(), 3);
    }

    #[test]
    fn saved_vectors_load_back_and_need_no_embedding() {
        let root = scratch_root("round-trip");
        let path = vectors_path(&root);
        let index = index_of(&[("01A", "coffee beans"), ("01B", "deploy on friday")]);

        let first = Mutex::new(None);
        reconcile(&index, &first, &path, &Counting::new("stub-64"), || true).unwrap();
        first.lock().unwrap().as_ref().unwrap().save(&path).unwrap();

        let embedder = Counting::new("stub-64");
        let loaded = Mutex::new(None);
        assert!(!reconcile(&index, &loaded, &path, &embedder, || true).unwrap());
        assert_eq!(embedder.calls(), 0);
        assert_eq!(*loaded.lock().unwrap(), *first.lock().unwrap());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_long_pass_cut_short_keeps_what_it_saved_along_the_way() {
        let root = scratch_root("resume");
        let path = vectors_path(&root);
        let bodies: Vec<(String, String)> = (0..SAVE_EVERY + 100)
            .map(|i| (format!("{i:05}"), format!("note number {i}")))
            .collect();
        let pairs: Vec<(&str, &str)> = bodies
            .iter()
            .map(|(id, body)| (id.as_str(), body.as_str()))
            .collect();
        let index = index_of(&pairs);

        // Quit after one save's worth and a bit more, without the final save.
        let first = Counting::new("stub-64");
        let vectors = Mutex::new(None);
        let unsaved = reconcile(&index, &vectors, &path, &first, || {
            first.calls() < SAVE_EVERY + 10
        });
        assert!(unsaved.unwrap());

        // The next launch reads the file and embeds only what it lacks.
        let next = Counting::new("stub-64");
        let reloaded = Mutex::new(None);
        reconcile(&index, &reloaded, &path, &next, || true).unwrap();
        assert_eq!(next.calls(), 100);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn another_model_starts_over() {
        let root = scratch_root("other-model");
        let path = vectors_path(&root);
        let index = index_of(&[("01A", "coffee beans"), ("01B", "deploy on friday")]);

        let vectors = Mutex::new(None);
        reconcile(&index, &vectors, &path, &Counting::new("stub-64"), || true).unwrap();
        vectors.lock().unwrap().as_ref().unwrap().save(&path).unwrap();

        // Once from disk, once from what is already in memory.
        for slot in [Mutex::new(None), vectors] {
            let other = Counting::new("other-64");
            assert!(reconcile(&index, &slot, &path, &other, || true).unwrap());
            assert_eq!(other.calls(), 2);
            assert!(slot.lock().unwrap().as_ref().unwrap().is_from("other-64", 64));
        }

        let _ = std::fs::remove_dir_all(&root);
    }
}
