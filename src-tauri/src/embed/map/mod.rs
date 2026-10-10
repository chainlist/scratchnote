//! Where each note sits on a 2D map of the space by meaning, saved in the
//! `positions` table of `space.db` so that no view lays the space out
//! itself, with each note's closest notes in `links`, and the categories
//! the notes make on it (`categories.rs`), found again whenever it changes.
//!
//! The map is laid out whole only when a space has none yet, or its notes
//! were embedded by another model: each note is pulled towards its closest
//! notes and pushed away from others, as UMAP does, starting from the two
//! directions the notes differ most along. That compares every note with
//! every other, seconds for 10,000 notes, so it runs in the background.
//!
//! After that it is never laid out again. A note new or written again gets
//! its closest notes, joins the lists of the notes it is closer to than
//! their own last, starts at the average of its closest notes' places,
//! weighted by how close, and then it and the notes around it settle for a
//! few rounds of the same pulls and pushes. A deleted note leaves every
//! list it was in, and those lists take the next closest note. Measured on
//! 10,269 notes, adding the newest half that way keeps nearly as many of
//! each note's closest notes nearby as laying them all out afresh, at about
//! 2 ms a note.
//!
//! Closeness is the cosine once the mean of every note is taken off, as for
//! Similar notes: notes share a lot just by being short notes by one person.
//! A note's list keeps the scores it was made with as the mean moves.

mod layout;

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use rusqlite::Connection;

use super::categories::Categories;
use super::vectors::Vectors;
use crate::state::lock;
use crate::storage::space_db::SpaceDb;
use crate::Result;
use layout::Snapshot;

/// The map, by slot: a note keeps its slot while it is on the map, so the
/// lists of closest notes hold slots, and placing a note looks nothing up
/// by id. Ids are only for loading and saving.
#[derive(Debug, Default, Clone)]
pub struct Map {
    /// The model of the vectors the places came from.
    model: String,
    /// Note id to its slot.
    slots: HashMap<String, usize>,
    /// By slot: the note's id and the body hash it was placed from, `None`
    /// for a slot free since its note went.
    notes: Vec<Option<(String, String)>>,
    /// By slot: the note's place.
    at: Vec<[f32; 2]>,
    /// By slot: its closest notes and how close, closest first.
    near: Vec<Near>,
    /// Slots free for the next notes.
    free: Vec<usize>,
    /// The categories of the notes in their places, `None` until found.
    categories: Option<Categories>,
}

/// The same notes in the same places with the same closest notes and
/// categories, whatever their slots.
impl PartialEq for Map {
    fn eq(&self, other: &Self) -> bool {
        self.model == other.model
            && self.by_id() == other.by_id()
            && self.categories == other.categories
    }
}

/// A note as `Map::by_id` gives it: its body hash, place and closest
/// notes by id.
type Entry<'a> = (&'a str, [f32; 2], Vec<(&'a str, f32)>);

/// A list of closest notes and how close, closest first.
type Near = Vec<(f32, usize)>;

/// What an update changed, to be saved, by note id.
struct Changes {
    moved: Vec<String>,
    gone: Vec<String>,
    lists: Vec<String>,
}

impl Map {
    /// The saved map. Never an error: it can always be laid out again.
    pub fn load(db: &SpaceDb) -> Self {
        let Some(model) = db.meta("map_model") else {
            return Self::default();
        };
        let mut map = SpaceDb::load_or_default(Self::read(db.conn(), model), "the map");
        map.categories = Categories::load(db);
        map
    }

    fn read(conn: &Connection, model: String) -> rusqlite::Result<Self> {
        let mut map = Self {
            model,
            ..Self::default()
        };
        let mut statement = conn.prepare("SELECT id, hash, x, y FROM positions ORDER BY id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            map.slots.insert(id.clone(), map.notes.len());
            map.notes.push(Some((id, row.get(1)?)));
            map.at
                .push([row.get::<_, f64>(2)? as f32, row.get::<_, f64>(3)? as f32]);
            map.near.push(Vec::new());
        }
        let mut statement = conn.prepare("SELECT id, other, score FROM links")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let (id, other): (String, String) = (row.get(0)?, row.get(1)?);
            if let (Some(&slot), Some(&to)) = (map.slots.get(&id), map.slots.get(&other)) {
                map.near[slot].push((row.get::<_, f64>(2)? as f32, to));
            }
        }
        for near in &mut map.near {
            near.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        }
        Ok(map)
    }

    /// Every note on the map and its place, for the map view.
    pub fn places(&self) -> impl Iterator<Item = (&str, [f32; 2])> {
        self.taken().map(|(slot, id, _)| (id, self.at[slot]))
    }

    /// Each note linked to its `k` closest notes, each pair once, by id.
    pub fn links(&self, k: usize) -> Vec<(&str, &str)> {
        let mut pairs: Vec<(usize, usize)> = self
            .taken()
            .flat_map(|(slot, _, _)| {
                self.near[slot]
                    .iter()
                    .take(k)
                    .map(move |&(_, to)| (slot.min(to), slot.max(to)))
            })
            .collect();
        pairs.sort_unstable();
        pairs.dedup();
        pairs
            .into_iter()
            .map(|(a, b)| (self.id(a), self.id(b)))
            .collect()
    }

    /// The categories of the notes on the map, `None` until found.
    pub fn categories(&self) -> Option<&Categories> {
        self.categories.as_ref()
    }

    /// Where a note is, `None` until it is placed. Only tests look up one.
    #[cfg(test)]
    pub fn get(&self, id: &str) -> Option<[f32; 2]> {
        self.slots.get(id).map(|&slot| self.at[slot])
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Every note by id: its body hash, place and closest notes.
    fn by_id(&self) -> BTreeMap<&str, Entry<'_>> {
        self.taken()
            .map(|(slot, id, hash)| {
                let near = self.near[slot]
                    .iter()
                    .map(|&(s, to)| (self.id(to), s))
                    .collect();
                (id, (hash, self.at[slot], near))
            })
            .collect()
    }

    /// Every slot in use, with its note's id and body hash.
    fn taken(&self) -> impl Iterator<Item = (usize, &str, &str)> {
        self.notes.iter().enumerate().filter_map(|(slot, note)| {
            note.as_ref()
                .map(|(id, hash)| (slot, id.as_str(), hash.as_str()))
        })
    }

    fn id(&self, slot: usize) -> &str {
        self.notes[slot].as_ref().map_or("", |(id, _)| id)
    }

    /// Whether the vectors hold a note this map does not have as it is, or
    /// the map a note they no longer hold.
    fn behind(&self, vectors: &Vectors) -> bool {
        let empty = vectors.len() == 0 && self.slots.is_empty();
        (self.model != vectors.model_id() && !empty)
            || self.taken().any(|(_, id, _)| vectors.get(id).is_none())
            || vectors.iter().any(|(id, hash, _)| {
                self.slots
                    .get(id)
                    .is_none_or(|&slot| self.notes[slot].as_ref().is_none_or(|(_, h)| h != hash))
            })
    }

    /// Write the whole map in place of what is saved.
    fn write_all(&self, tx: &Connection) -> Result<()> {
        tx.execute_batch("DELETE FROM positions; DELETE FROM links;")?;
        SpaceDb::set_meta(tx, "map_model", &self.model)?;
        let slots: Vec<usize> = self.taken().map(|(slot, _, _)| slot).collect();
        self.write_places(tx, &slots)?;
        self.write_lists(tx, &slots)
    }

    /// Write what an update changed.
    fn write_changes(&self, tx: &Connection, changes: &Changes) -> Result<()> {
        for id in &changes.gone {
            tx.execute("DELETE FROM positions WHERE id = ?1", [id])?;
            tx.execute("DELETE FROM links WHERE id = ?1", [id])?;
        }
        let slots =
            |ids: &[String]| -> Vec<usize> { ids.iter().map(|id| self.slots[id]).collect() };
        self.write_places(tx, &slots(&changes.moved))?;
        self.write_lists(tx, &slots(&changes.lists))
    }

    fn write_places(&self, tx: &Connection, slots: &[usize]) -> Result<()> {
        let mut put = tx.prepare_cached(
            "INSERT OR REPLACE INTO positions (id, hash, x, y) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for &slot in slots {
            let Some((id, hash)) = &self.notes[slot] else {
                continue;
            };
            let [x, y] = self.at[slot];
            put.execute(rusqlite::params![id, hash, x as f64, y as f64])?;
        }
        Ok(())
    }

    fn write_lists(&self, tx: &Connection, slots: &[usize]) -> Result<()> {
        let mut clear = tx.prepare_cached("DELETE FROM links WHERE id = ?1")?;
        let mut put =
            tx.prepare_cached("INSERT INTO links (id, other, score) VALUES (?1, ?2, ?3)")?;
        for &slot in slots {
            let id = self.id(slot);
            clear.execute([id])?;
            for &(score, to) in &self.near[slot] {
                put.execute(rusqlite::params![id, self.id(to), score as f64])?;
            }
        }
        Ok(())
    }
}

/// Bring the saved map in line with the vectors, and find its categories
/// again, named from `bodies`, the notes' text by id. The work is done on a
/// copy of the map, which then takes its place: the vectors and the map are
/// each held only for an instant, so search and the map view go on
/// meanwhile, however long a full layout takes. The map is loaded from `db`
/// first if it is not in memory yet. True when any note moved, or the
/// categories were found for the first time.
pub fn reconcile(
    vectors: &Mutex<Option<Vectors>>,
    map: &Mutex<Option<Map>>,
    db: &Mutex<Option<SpaceDb>>,
    bodies: impl FnOnce() -> HashMap<String, String>,
) -> Result<bool> {
    let (mut working, snapshot) = {
        let held = lock(vectors, "vectors")?;
        let Some(store) = held.as_ref() else {
            return Ok(false);
        };
        let mut slot = lock(map, "map")?;
        if slot.is_none() {
            let db = lock(db, "space.db")?;
            let Some(db) = db.as_ref() else {
                return Ok(false);
            };
            *slot = Some(Map::load(db));
        }
        let current = slot.as_ref().expect("loaded just above");
        let behind = current.behind(store);
        if !behind && (current.categories.is_some() || current.slots.is_empty()) {
            return Ok(false);
        }
        (current.clone(), behind.then(|| Snapshot::of(store)))
    };

    let started = std::time::Instant::now();
    // What an update changed. `None` after a full layout, or when the notes
    // stay where they are and only the categories were never found.
    let mut changes = None;
    let mut whole = false;
    if let Some(snapshot) = &snapshot {
        if working.model != snapshot.model || working.slots.is_empty() {
            working.lay_out(snapshot);
            whole = true;
        } else {
            changes = Some(working.update(snapshot));
        }
    }
    // After a full layout the levels are found again from the new places;
    // otherwise they stay as they were, and so do the ids.
    let before = working.categories.take();
    let categories = {
        let places: Vec<(&str, [f32; 2])> = working.places().collect();
        let base = before.as_ref().map(|c| c.base);
        Categories::find(&places, base, before.as_ref(), &bodies())
    };

    {
        let mut db = lock(db, "space.db")?;
        // The space was closed meanwhile.
        let Some(db) = db.as_mut() else {
            return Ok(false);
        };
        db.transaction(|tx| {
            if whole {
                working.write_all(tx)?;
            } else if let Some(changes) = &changes {
                working.write_changes(tx, changes)?;
            }
            categories.write(tx, before.as_ref())
        })?;
        if whole {
            log::info!(
                "laid out {} notes in {:?}",
                working.slots.len(),
                started.elapsed()
            );
        }
    }
    working.categories = Some(categories);
    let mut slot = lock(map, "map")?;
    // Only this task changes the map; a space closed meanwhile has none.
    if slot.is_some() {
        *slot = Some(working);
    }
    Ok(true)
}

#[cfg(test)]
mod tests;
