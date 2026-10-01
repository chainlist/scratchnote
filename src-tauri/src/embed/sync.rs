//! Keeps each space's vectors in line with its index.
//!
//! Rather than hooking every way a note can change, one task compares the
//! index with the vectors whenever it is woken: whatever the index holds that
//! the vectors do not is embedded, whatever it lost is forgotten. A missed
//! wake only delays a note until the next one.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, RwLock};

use tauri::{AppHandle, Emitter, Manager};

use super::threads::{self, threads_path, Edits, Threads};
use super::vectors::{vectors_path, Vectors};
use super::Embedder;
use crate::enrich::worker::Wake;
use crate::spaces::Space;
use crate::state::AppState;
use crate::storage::index::Index;
use crate::storage::search_db::SearchDb;

/// One task for every space, so only one ever saves a space's `vectors.bin`.
pub fn spawn(app: AppHandle, wake: Wake) {
    tauri::async_runtime::spawn(async move {
        loop {
            // A wake during a pass is kept as a permit, so the notes saved
            // meanwhile get a pass of their own straight after.
            wake.notified().await;
            let passing = app.clone();
            // Loading the model and embedding both block, so they stay off
            // the async runtime.
            let pass = tauri::async_runtime::spawn_blocking(move || {
                let Some(embedder) = super::embedder(&passing) else {
                    return Vec::new();
                };
                let state = passing.state::<AppState>();
                // Stamped as the worker stamps a job, so the idle unload
                // counts embedding as use.
                state.mark_used();
                let threaded: Vec<String> = state
                    .all_spaces()
                    .iter()
                    .filter(|space| sync_space(&state, space, embedder.as_ref()))
                    .map(|space| space.name.clone())
                    .collect();
                state.mark_used();
                threaded
            });
            match pass.await {
                Ok(threaded) => {
                    for name in threaded {
                        let _ = app.emit("threads-changed", serde_json::json!({ "space": name }));
                    }
                }
                Err(e) => log::warn!("embedding notes failed: {e}"),
            }
        }
    });
}

/// Only the open space is embedded: the vectors serve its chat and similar
/// notes, and another space catches up when it opens (SPEC 4.6). Its notes
/// are then placed in threads. True when its threads changed.
fn sync_space(state: &AppState, space: &Space, embedder: &dyn Embedder) -> bool {
    if space.is_retired() || !space.is_open() {
        return false;
    }
    let path = vectors_path(&space.root);
    let keep_going = || !space.is_retired() && space.is_open() && state.model_enabled();
    let held = space.held();
    match reconcile(
        &space.index,
        &space.search,
        &space.vectors,
        &path,
        embedder,
        &held,
        keep_going,
    ) {
        // A space renamed or deleted meanwhile: its folder is gone from here.
        Ok(true) if !space.is_retired() => {
            if let Ok(vectors) = space.vectors.lock() {
                if let Some(vectors) = vectors.as_ref() {
                    match vectors.save(&path) {
                        Ok(()) => log::info!("updated the vectors of {}", space.name),
                        Err(e) => log::warn!("could not save the vectors of {}: {e}", space.name),
                    }
                }
            }
        }
        Ok(_) => {}
        Err(e) => log::warn!("could not embed the notes of {}: {e}", space.name),
    }
    sync_threads(space)
}

/// Place the open space's notes in threads from its vectors as they are
/// (SPEC 6.4), and save where they went. True when the threads changed, or
/// were read from disk, which the views have not seen either. Without
/// vectors in memory, nothing is placed.
pub(crate) fn sync_threads(space: &Space) -> bool {
    let when = match space.index.read() {
        Ok(index) if !index.is_closed() => threads::when_written(&index),
        _ => return false,
    };
    let alone: HashSet<String> = Edits::load(&space.root).alone.into_iter().collect();
    let path = threads_path(&space.root);
    let Ok(vectors) = space.vectors.lock() else {
        return false;
    };
    let Some(vectors) = vectors.as_ref() else {
        return false;
    };
    let Ok(mut slot) = space.threads.lock() else {
        return false;
    };
    let read = slot.is_none();
    let threads = slot.get_or_insert_with(|| Threads::load(&path));
    let changed = threads.reconcile(vectors, &when, &alone);
    if changed && !space.is_retired() {
        if let Err(e) = threads.save(&path) {
            log::warn!("could not save the threads of {}: {e}", space.name);
        }
    }
    read || changed
}

/// A first pass over thousands of notes takes minutes, so it saves as it
/// goes: quitting midway then loses at most this many notes, not the pass.
/// Each save rewrites the whole file, hence not after every note.
const SAVE_EVERY: usize = 500;

/// Embed every note the vectors lack or hold for an older body, its text read
/// from `texts`, and forget notes the index no longer has. An empty body has
/// nothing to embed. Pages open in the editor, `held`, keep the vector they
/// had until their view closes. The store is loaded from `path` first if
/// it is not in memory yet or holds another model's vectors, and saved there
/// every `SAVE_EVERY` notes. Stops before the next note once `keep_going`
/// says so. True when the vectors hold changes not saved yet.
pub fn reconcile(
    index: &RwLock<Index>,
    texts: &Mutex<Option<SearchDb>>,
    vectors: &Mutex<Option<Vectors>>,
    path: &Path,
    embedder: &dyn Embedder,
    held: &HashSet<String>,
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
        // A space closed meanwhile holds no notes, which must not read as
        // every note deleted and cost the vectors on disk.
        if index.is_closed() {
            return Ok(false);
        }
        let present = index.entries().map(|e| e.id.clone()).collect();
        let dropped = store.retain(&present);
        let stale: Vec<(String, String)> = store
            .stale(index.entries())
            .into_iter()
            .filter(|e| !held.contains(&e.id))
            .map(|e| (e.id.clone(), e.hash.clone()))
            .collect();
        drop(index);

        let mut bodies = match texts.lock() {
            Ok(texts) => match texts.as_ref() {
                Some(db) => db.bodies(stale.iter().map(|(id, _)| id.as_str()))?,
                None => return Ok(false),
            },
            Err(_) => return Err("search.db lock poisoned".to_string()),
        };
        let todo: Vec<(String, String, String)> = stale
            .into_iter()
            .filter_map(|(id, hash)| {
                let body = bodies.remove(&id)?;
                (!body.trim().is_empty()).then_some((id, hash, body))
            })
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
    use crate::storage::daily_file::{body_hash, Kind, Note, Status};
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
        IndexEntry::from(&note(id, body))
    }

    fn note(id: &str, body: &str) -> Note {
        Note {
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
            kind: Kind::Note,
            missing: false,
        }
    }

    /// The index of an open space holding `notes`, and their text.
    fn index_of(notes: &[(&str, &str)]) -> (RwLock<Index>, Mutex<Option<SearchDb>>) {
        let mut index = Index::default();
        let mut db = SearchDb::in_memory().unwrap();
        for (id, body) in notes {
            index.push(entry(id, body));
            db.add_note(&note(id, body), None).unwrap();
        }
        (RwLock::new(index), Mutex::new(Some(db)))
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
        assert!(reconcile(&index.0, &index.1, &vectors, &path, &embedder, &HashSet::new(), || true).unwrap());
        assert_eq!(embedder.calls(), 2);

        // Nothing changed, so nothing is embedded and nothing is reported.
        assert!(!reconcile(&index.0, &index.1, &vectors, &path, &embedder, &HashSet::new(), || true).unwrap());
        assert_eq!(embedder.calls(), 2);

        // 01B edited, 01C new: only those two go through the model.
        let index = index_of(&[
            ("01A", "coffee beans"),
            ("01B", "deploy on monday"),
            ("01C", "grandma's birthday"),
        ]);
        assert!(reconcile(&index.0, &index.1, &vectors, &path, &embedder, &HashSet::new(), || true).unwrap());
        assert_eq!(embedder.calls(), 4);
        assert_eq!(ids(&vectors), vec!["01A", "01B", "01C"]);
    }

    #[test]
    fn a_closed_space_keeps_its_vectors() {
        let root = scratch_root("closed");
        let path = vectors_path(&root);
        let embedder = Counting::new("stub-64");
        let vectors = Mutex::new(None);
        let index = index_of(&[("01A", "coffee beans")]);
        reconcile(&index.0, &index.1, &vectors, &path, &embedder, &HashSet::new(), || true).unwrap();

        let closed = RwLock::new(Index::closed());
        assert!(!reconcile(&closed, &index.1, &vectors, &path, &embedder, &HashSet::new(), || true).unwrap());
        assert_eq!(ids(&vectors), vec!["01A"], "no note of it counts as deleted");
    }

    #[test]
    fn a_page_being_edited_keeps_its_vector_until_its_view_closes() {
        let root = scratch_root("held");
        let path = vectors_path(&root);
        let embedder = Counting::new("stub-64");
        let vectors = Mutex::new(None);
        let index = index_of(&[("01P", "first draft")]);
        reconcile(&index.0, &index.1, &vectors, &path, &embedder, &HashSet::new(), || true).unwrap();

        // Every autosave changes the text; none of them reaches the model.
        let held: HashSet<String> = ["01P".to_string()].into();
        for text in ["first draft, more", "first draft, more and more"] {
            let index = index_of(&[("01P", text)]);
            assert!(!reconcile(&index.0, &index.1, &vectors, &path, &embedder, &held, || true).unwrap());
        }
        assert_eq!(embedder.calls(), 1);
        assert_eq!(ids(&vectors), vec!["01P"], "the old vector stays meanwhile");

        let index = index_of(&[("01P", "first draft, more and more")]);
        assert!(reconcile(&index.0, &index.1, &vectors, &path, &embedder, &HashSet::new(), || true).unwrap());
        assert_eq!(embedder.calls(), 2);
    }

    #[test]
    fn a_blank_note_is_not_embedded() {
        let root = scratch_root("blank");
        let embedder = Counting::new("stub-64");
        let vectors = Mutex::new(None);

        let index = index_of(&[("01A", "  \n")]);
        let changed = reconcile(&index.0, &index.1, &vectors, &vectors_path(&root), &embedder, &HashSet::new(), || true);
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
        reconcile(&index.0, &index.1, &vectors, &path, &embedder, &HashSet::new(), || true).unwrap();

        let index = index_of(&[("01A", "coffee beans")]);
        assert!(reconcile(&index.0, &index.1, &vectors, &path, &embedder, &HashSet::new(), || true).unwrap());
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

        let changed = reconcile(&index.0, &index.1, &vectors, &path, &embedder, &HashSet::new(), || embedder.calls() < 1);
        assert!(changed.unwrap());
        assert_eq!(embedder.calls(), 1);

        // The next pass carries on where that one stopped.
        reconcile(&index.0, &index.1, &vectors, &path, &embedder, &HashSet::new(), || true).unwrap();
        assert_eq!(embedder.calls(), 3);
    }

    #[test]
    fn saved_vectors_load_back_and_need_no_embedding() {
        let root = scratch_root("round-trip");
        let path = vectors_path(&root);
        let index = index_of(&[("01A", "coffee beans"), ("01B", "deploy on friday")]);

        let first = Mutex::new(None);
        reconcile(&index.0, &index.1, &first, &path, &Counting::new("stub-64"), &HashSet::new(), || true).unwrap();
        first.lock().unwrap().as_ref().unwrap().save(&path).unwrap();

        let embedder = Counting::new("stub-64");
        let loaded = Mutex::new(None);
        assert!(!reconcile(&index.0, &index.1, &loaded, &path, &embedder, &HashSet::new(), || true).unwrap());
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
        let unsaved = reconcile(&index.0, &index.1, &vectors, &path, &first, &HashSet::new(), || {
            first.calls() < SAVE_EVERY + 10
        });
        assert!(unsaved.unwrap());

        // The next launch reads the file and embeds only what it lacks.
        let next = Counting::new("stub-64");
        let reloaded = Mutex::new(None);
        reconcile(&index.0, &index.1, &reloaded, &path, &next, &HashSet::new(), || true).unwrap();
        assert_eq!(next.calls(), 100);

        let _ = std::fs::remove_dir_all(&root);
    }

    /// From a space's index and vectors to `threads.json` and back: what
    /// the embed task does after each pass.
    #[test]
    fn a_pass_places_the_open_space_in_threads_and_saves_them() {
        use crate::spaces::Space;
        use crate::storage::daily_file::append_note;

        let root = scratch_root("threads");
        let bodies = [
            ("01A", "2026-09-10", "kitchen renovation tiles plumber quote"),
            ("01B", "2026-09-12", "kitchen renovation plumber tiles started"),
            ("01C", "2026-09-14", "kitchen renovation tiles plumber done"),
            ("01D", "2026-09-11", "coffee beans market"),
            ("01E", "2026-09-13", "dentist appointment moved"),
            ("01F", "2026-09-15", "rust borrow checker lifetimes"),
            ("01G", "2026-09-16", "marathon long run shin"),
            ("01H", "2026-09-17", "electricity bill went up"),
            ("01I", "2026-09-18", "severance season finale goats"),
            ("01J", "2026-09-19", "lisbon flights booked october"),
            ("01K", "2026-09-20", "garage brake pads discs"),
            ("01L", "2026-09-21", "postgres connection pool exhausted"),
            ("01M", "2026-09-22", "mom birthday dinner italian"),
            ("01N", "2026-09-23", "usb hub homelab ethernet"),
        ];
        for (id, date, body) in bodies {
            let mut written = note(id, body);
            written.date = date.to_string();
            written.file = crate::storage::relative_day_path(date);
            let path = crate::storage::day_path(&root, date);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, append_note("", &written, date)).unwrap();
        }
        let (space, _) = Space::open("Test", root.clone(), std::sync::Arc::new(tokio::sync::Notify::new()));
        reconcile(&space.index, &space.search, &space.vectors, &vectors_path(&root), &StubEmbedder, &HashSet::new(), || true).unwrap();

        assert!(sync_threads(&space), "read for the first time and placed");
        assert!(threads_path(&root).exists());
        assert!(!sync_threads(&space), "nothing new");
        let placed = Threads::load(&threads_path(&root));
        assert_eq!(*space.threads.lock().unwrap(), Some(placed));
        {
            let threads = space.threads.lock().unwrap();
            let threads = threads.as_ref().unwrap();
            assert!(["01A", "01B", "01C"].iter().all(|id| threads.of(id) == Some("01A")));
            let others = ["01D", "01E", "01F", "01G", "01H", "01I", "01J", "01K", "01L", "01M", "01N"];
            // The stub's buckets may pair two of these by chance, but none
            // shares the kitchen's words.
            assert!(others.iter().all(|id| threads.of(id) != Some("01A")));
        }
        drop(space);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn another_model_starts_over() {
        let root = scratch_root("other-model");
        let path = vectors_path(&root);
        let index = index_of(&[("01A", "coffee beans"), ("01B", "deploy on friday")]);

        let vectors = Mutex::new(None);
        reconcile(&index.0, &index.1, &vectors, &path, &Counting::new("stub-64"), &HashSet::new(), || true).unwrap();
        vectors.lock().unwrap().as_ref().unwrap().save(&path).unwrap();

        // Once from disk, once from what is already in memory.
        for slot in [Mutex::new(None), vectors] {
            let other = Counting::new("other-64");
            assert!(reconcile(&index.0, &index.1, &slot, &path, &other, &HashSet::new(), || true).unwrap());
            assert_eq!(other.calls(), 2);
            assert!(slot.lock().unwrap().as_ref().unwrap().is_from("other-64", 64));
        }

        let _ = std::fs::remove_dir_all(&root);
    }
}
