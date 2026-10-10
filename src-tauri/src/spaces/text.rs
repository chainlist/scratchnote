//! The notes of an open space as the index and search.db hold them: every
//! change to either goes through here.

use std::collections::HashMap;
use std::sync::RwLockWriteGuard;

use super::Space;
use crate::state::{lock, read_lock, write_lock};
use crate::storage::daily_file::Note;
use crate::storage::day_path;
use crate::storage::index::{self, Index, IndexEntry};
use crate::storage::search_db::{SearchDb, Stamp};
use crate::Result;

impl Space {
    /// Tell the embed task the index changed. Every change to the index
    /// ends with it, so the vectors follow the index from this one place.
    pub fn index_changed(&self) {
        self.embed_wake.notify_one();
    }

    // The index is changed through the methods below only, so search.db
    // follows it from one place. Each writes the text first, then takes the
    // index lock for its own change; the caller then calls `index_changed`
    // once. A space that is not open drops the change: its files are then
    // not as search.db last read them, so `index::load` reads them again
    // when it opens.

    /// The index to change, or `None` while the space is not open.
    fn index_to_change(&self) -> Result<Option<RwLockWriteGuard<'_, Index>>> {
        let idx = write_lock(&self.index, "index")?;
        Ok((!idx.is_closed()).then_some(idx))
    }

    /// Change the text in search.db, if the space is open. A write that fails
    /// is only logged: its file stays marked as read before it, so it is read
    /// again at the next open.
    fn write_text(&self, change: impl FnOnce(&mut SearchDb) -> Result<()>) {
        let Ok(mut search) = self.search.lock() else {
            return;
        };
        if let Some(db) = search.as_mut() {
            if let Err(e) = change(db) {
                log::warn!("could not update the search.db of {}: {e}", self.name);
            }
        }
    }

    /// A note just captured: added rather than its day reparsed, since
    /// capture has a latency budget.
    pub fn note_added(&self, note: &Note) -> Result<()> {
        let stamp = Stamp::of(&day_path(&self.root, &note.date));
        self.write_text(|db| db.add_note(note, stamp));
        match self.index_to_change()? {
            Some(mut idx) => idx.push(IndexEntry::from(note)),
            None => return Ok(()),
        }
        self.index_changed();
        Ok(())
    }

    /// Read a day's file again after it was written, in place of what the
    /// index had for that day.
    pub fn day_changed(&self, date: &str) -> Result<()> {
        if !self.is_open() {
            return Ok(());
        }
        let path = day_path(&self.root, date);
        // Taken before the file is read, so a write in between leaves the day
        // looking older than it is, and read again, never the other way.
        let stamp = Stamp::of(&path);
        self.set_day(date, &index::parse_day(&path, date), stamp)
    }

    /// Put `notes`, parsed from a day's file as it was at `stamp`, in place
    /// of that day's.
    pub fn set_day(&self, date: &str, notes: &[Note], stamp: Option<Stamp>) -> Result<()> {
        self.write_text(|db| db.replace_day(date, notes, stamp));
        if let Some(mut idx) = self.index_to_change()? {
            idx.replace_day(date, index::entries(notes));
        }
        Ok(())
    }

    /// A page added or changed. Returns what the index had for it before.
    pub fn page_changed(&self, page: &Note) -> Result<Option<IndexEntry>> {
        let stamp = Stamp::of(&self.root.join(&page.file));
        self.write_text(|db| db.replace_page(page, stamp));
        Ok(self
            .index_to_change()?
            .and_then(|mut idx| idx.replace_page(IndexEntry::from(page))))
    }

    /// A page deleted. Returns what the index had for it.
    pub fn page_removed(&self, id: &str) -> Result<Option<IndexEntry>> {
        self.write_text(|db| db.remove_page(id));
        Ok(self
            .index_to_change()?
            .and_then(|mut idx| idx.remove_page(id)))
    }

    /// Whatever page the index had at `file`, a path relative to the root,
    /// is gone from there. Returns its entry.
    pub fn page_file_gone(&self, file: &str) -> Result<Option<IndexEntry>> {
        let gone = {
            let Some(mut idx) = self.index_to_change()? else {
                return Ok(None);
            };
            let id = idx.page_at(file).map(|page| page.id.clone());
            id.and_then(|id| idx.remove_page(&id))
        };
        if let Some(page) = &gone {
            self.write_text(|db| db.remove_page(&page.id));
        }
        Ok(gone)
    }

    /// Reparse every file of the space, into the index and search.db alike,
    /// and drop its vectors, which the embed task then makes again from every
    /// note once told the index changed. Blocks for as long as that takes.
    /// Returns how many notes and pages it holds.
    pub fn rebuild(&self) -> Result<usize> {
        let rebuilt = {
            let mut search = lock(&self.search, "search.db")?;
            let Some(db) = search.as_mut() else {
                return Ok(0);
            };
            index::rebuild(&self.root, db)
        };
        let count = rebuilt.len();
        if let Some(mut idx) = self.index_to_change()? {
            *idx = rebuilt;
        }
        // Under the lock, so a pass under way neither stores into the old
        // vectors nor loads them before they are gone.
        let mut vectors = lock(&self.vectors, "vectors")?;
        *vectors = None;
        if let Some(db) = lock(&self.db, "space.db")?.as_mut() {
            db.clear_vectors()
                .map_err(|e| format!("could not remove the vectors: {e}"))?;
        }
        Ok(count)
    }

    /// Run `read` on the index and the text together, index first as the
    /// locks go. `None` while the space is not open.
    pub fn read<T>(&self, read: impl FnOnce(&Index, &SearchDb) -> Result<T>) -> Result<Option<T>> {
        let idx = read_lock(&self.index, "index")?;
        let search = lock(&self.search, "search.db")?;
        match (idx.is_closed(), search.as_ref()) {
            (false, Some(db)) => read(&idx, db).map(Some),
            _ => Ok(None),
        }
    }

    /// Fill in the text of `notes`, taken from the index without it.
    pub fn fill_bodies(&self, notes: &mut [Note]) {
        let mut bodies = self.bodies(notes.iter().map(|note| note.id.as_str()));
        for note in notes {
            if let Some(body) = bodies.remove(&note.id) {
                note.body = body;
            }
        }
    }

    /// The text of `ids`, those search.db holds. Empty while the space is not
    /// open.
    pub fn bodies<'a>(&self, ids: impl IntoIterator<Item = &'a str>) -> HashMap<String, String> {
        let Ok(search) = self.search.lock() else {
            return HashMap::new();
        };
        search
            .as_ref()
            .and_then(|db| db.bodies(ids).ok())
            .unwrap_or_default()
    }
}
