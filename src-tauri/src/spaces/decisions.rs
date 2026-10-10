//! What the user decided about threads and pins in a space (SPEC 6.4,
//! 3.13), changed under one lock, and the last change kept to take back.

use super::Space;
use crate::embed::threads::Edits;
use crate::state::lock;
use crate::storage::pins::{self, Pin};
use crate::{Error, Result};

/// The thread edits and the pins together: what one change to threads, a
/// merge say, may change both of.
pub type Decided = (Edits, Vec<Pin>);

/// What the threads were before a change, and what the change left.
pub(super) struct ThreadUndo {
    before: Decided,
    after: Decided,
}

impl Space {
    /// What the user decided about threads, none while the space is not
    /// open.
    pub fn edits(&self) -> Edits {
        match self.db.lock().as_deref() {
            Ok(Some(db)) => Edits::load(db),
            _ => Edits::default(),
        }
    }

    /// Change what the user decided about threads, read and saved under
    /// one lock so that two changes never undo each other. `change` says
    /// whether it changed anything, and nothing is saved when it did not.
    pub fn change_edits(&self, change: impl FnOnce(&mut Edits) -> bool) -> Result<bool> {
        let mut db = lock(&self.db, "space.db")?;
        let db = db.as_mut().ok_or(Error::SpaceClosed)?;
        let mut edits = Edits::load(db);
        if !change(&mut edits) {
            return Ok(false);
        }
        edits.save(db)?;
        Ok(true)
    }

    /// What is pinned to the left edge, in its order (SPEC 3.13).
    pub fn pins(&self) -> Vec<Pin> {
        match self.db.lock().as_deref() {
            Ok(Some(db)) => pins::load(db),
            _ => Vec::new(),
        }
    }

    /// Change the pins under one lock, as `change_edits` changes the edits.
    /// Returns them as saved, or `None` when `change` changed nothing.
    pub fn change_pins(
        &self,
        change: impl FnOnce(&mut Vec<Pin>) -> bool,
    ) -> Result<Option<Vec<Pin>>> {
        let mut db = lock(&self.db, "space.db")?;
        let db = db.as_mut().ok_or(Error::SpaceClosed)?;
        let mut pins = pins::load(db);
        if !change(&mut pins) {
            return Ok(None);
        }
        let pins = pins::tidy(pins);
        pins::save(db, &pins)?;
        Ok(Some(pins))
    }

    /// Change what the user decided about threads, as `change_edits` does,
    /// and remember the change for `undo_threads` when there was one. Says
    /// whether there was.
    pub fn decide(&self, change: impl FnOnce(&mut Edits) -> bool) -> Result<bool> {
        let before = self.decided();
        let changed = self.change_edits(change)?;
        if changed {
            self.remember_change(before);
        }
        Ok(changed)
    }

    /// The thread edits and the pins as they are now.
    pub fn decided(&self) -> Decided {
        (self.edits(), self.pins())
    }

    /// Remember the change to threads made since `before`, so that
    /// `undo_threads` can take it back. It replaces the one before it.
    pub fn remember_change(&self, before: Decided) {
        let after = self.decided();
        if let Ok(mut undo) = self.undo.lock() {
            *undo = (before != after).then_some(ThreadUndo { before, after });
        }
    }

    /// Put the thread edits and the pins back as they were before the last
    /// change remembered, once, and only while nothing has changed them
    /// since: a later change is never lost to an older undo. `None` when
    /// there was nothing to take back, else whether the pins changed too.
    pub fn undo_threads(&self) -> Result<Option<bool>> {
        let Some(undo) = lock(&self.undo, "undo")?.take() else {
            return Ok(None);
        };
        if self.decided() != undo.after {
            return Ok(None);
        }
        let (edits, pins) = undo.before;
        self.change_edits(|now| {
            *now = edits;
            true
        })?;
        let pins_changed = self
            .change_pins(|now| {
                if *now == pins {
                    return false;
                }
                *now = pins;
                true
            })?
            .is_some();
        Ok(Some(pins_changed))
    }
}
