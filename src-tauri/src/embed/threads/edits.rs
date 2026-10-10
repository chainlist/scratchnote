//! What the user decided about threads, SPEC 6.4: titles, notes kept out
//! of threads or put in one, and suggestions dismissed. Placing never
//! overrides them, and nothing derives them, so they are saved as they are.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use super::{scope_of, GENERAL};
use crate::storage::space_db::SpaceDb;
use crate::Result;

/// What the user decided about threads, which placing never overrides.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Edits {
    /// Titles given to threads, by thread.
    pub titles: BTreeMap<String, String>,
    /// Notes taken out of threads, which stay out.
    pub alone: BTreeSet<String>,
    /// Notes the user put in a thread of the general scope, by note, which
    /// stay there whatever they score. A thread holding one is the user's,
    /// as is a titled one; any other is only suggested.
    pub pinned: BTreeMap<String, String>,
    /// The same for the scopes of mentions, by scope, then note.
    pub pinned_in: BTreeMap<String, BTreeMap<String, String>>,
    /// Suggested threads the user dismissed, with the notes each held then.
    /// One is suggested again once it holds another.
    pub dismissed: BTreeMap<String, BTreeSet<String>>,
}

impl Edits {
    /// The threads that are the user's: titled, or holding a note they put
    /// there.
    pub fn kept(&self) -> HashSet<&str> {
        self.titles
            .iter()
            .filter(|(_, title)| !title.trim().is_empty())
            .map(|(thread, _)| thread.as_str())
            .chain(self.puts().map(|(_, _, thread)| thread))
            .collect()
    }

    /// The thread of `scope` the user put `note` in, if any.
    pub fn put(&self, scope: &str, note: &str) -> Option<&str> {
        let thread = if scope == GENERAL {
            self.pinned.get(note)
        } else {
            self.pinned_in.get(scope)?.get(note)
        };
        thread.map(String::as_str)
    }

    /// Every note the user put in a thread: `(scope, note, thread)`.
    pub fn puts(&self) -> impl Iterator<Item = (&str, &str, &str)> {
        let general = self
            .pinned
            .iter()
            .map(|(note, thread)| (GENERAL, note.as_str(), thread.as_str()));
        let scoped = self.pinned_in.iter().flat_map(|(scope, notes)| {
            notes
                .iter()
                .map(move |(note, thread)| (scope.as_str(), note.as_str(), thread.as_str()))
        });
        general.chain(scoped)
    }

    /// Put `note` in `thread`, in place of the thread of the same scope it
    /// was put in before, if any.
    pub fn put_in(&mut self, note: String, thread: String) {
        let scope = scope_of(&thread).to_string();
        if scope == GENERAL {
            self.pinned.insert(note, thread);
        } else {
            self.pinned_in
                .entry(scope)
                .or_default()
                .insert(note, thread);
        }
    }

    /// Forget every thread the user put `note` in. True when it was in one.
    pub fn take_out(&mut self, note: &str) -> bool {
        let mut was = self.pinned.remove(note).is_some();
        for notes in self.pinned_in.values_mut() {
            was |= notes.remove(note).is_some();
        }
        self.pinned_in.retain(|_, notes| !notes.is_empty());
        was
    }

    /// What the user decided. Edits that cannot be read decide nothing.
    pub fn load(db: &SpaceDb) -> Self {
        SpaceDb::load_or_default(Self::read(db.conn()), "the thread edits")
    }

    fn read(conn: &Connection) -> rusqlite::Result<Self> {
        let mut edits = Self::default();
        let pairs = |sql: &str| -> rusqlite::Result<Vec<(String, String)>> {
            let mut statement = conn.prepare(sql)?;
            let rows = statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect();
            rows
        };
        edits.titles = pairs("SELECT thread, title FROM thread_titles")?
            .into_iter()
            .collect();
        edits.pinned = pairs("SELECT note, thread FROM pinned")?
            .into_iter()
            .collect();
        let mut statement = conn.prepare("SELECT scope, note, thread FROM pinned_in")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (scope, note, thread) = row?;
            edits
                .pinned_in
                .entry(scope)
                .or_default()
                .insert(note, thread);
        }
        for (thread, note) in pairs("SELECT thread, note FROM dismissed")? {
            edits.dismissed.entry(thread).or_default().insert(note);
        }
        let mut statement = conn.prepare("SELECT note FROM kept_alone")?;
        edits.alone = statement
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(edits)
    }

    /// Save these in place of the edits saved before.
    pub fn save(&self, db: &mut SpaceDb) -> Result<()> {
        db.transaction(|tx| self.write(tx))
    }

    /// What `save` writes, in the caller's transaction. A suggestion
    /// dismissed with no notes is not kept: holding any note, it is
    /// suggested again anyway.
    pub(crate) fn write(&self, tx: &Connection) -> Result<()> {
        tx.execute_batch(
            "DELETE FROM thread_titles; DELETE FROM kept_alone;
             DELETE FROM pinned; DELETE FROM pinned_in; DELETE FROM dismissed;",
        )?;
        let run = |sql: &str, a: &str, b: Option<&str>| -> Result<()> {
            let mut statement = tx.prepare_cached(sql)?;
            let _ = match b {
                Some(b) => statement.execute([a, b]),
                None => statement.execute([a]),
            }?;
            Ok(())
        };
        for (thread, title) in &self.titles {
            run(
                "INSERT INTO thread_titles (thread, title) VALUES (?1, ?2)",
                thread,
                Some(title),
            )?;
        }
        for note in &self.alone {
            run("INSERT INTO kept_alone (note) VALUES (?1)", note, None)?;
        }
        for (note, thread) in &self.pinned {
            run(
                "INSERT INTO pinned (note, thread) VALUES (?1, ?2)",
                note,
                Some(thread),
            )?;
        }
        let mut put =
            tx.prepare_cached("INSERT INTO pinned_in (scope, note, thread) VALUES (?1, ?2, ?3)")?;
        for (scope, notes) in &self.pinned_in {
            for (note, thread) in notes {
                put.execute([scope, note, thread])?;
            }
        }
        for (thread, notes) in &self.dismissed {
            for note in notes {
                run(
                    "INSERT INTO dismissed (thread, note) VALUES (?1, ?2)",
                    thread,
                    Some(note),
                )?;
            }
        }
        Ok(())
    }

    /// The edits `thread-edits.json` holds, `None` for a file that is
    /// missing or unreadable.
    pub fn read_json(path: &Path) -> Option<Self> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }
}
