//! Where the notes were placed, saved in the `placed` and `placed_in`
//! tables of `space.db`, and read in once from `threads.json`.

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use super::{flat, Placed, Scope, Threads, GENERAL};
use crate::storage::space_db::SpaceDb;
use crate::Result;

/// A placement changed since the last save, by scope and note, with where
/// the note is now: `None` once it is forgotten.
type Change = ((String, String), Option<Placed>);

/// `threads.json`, which held the general scope alone, and is read in once.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Legacy {
    version: u32,
    model: String,
    notes: Scope,
}

/// The placements as last loaded or saved.
#[derive(Debug, Clone, Default)]
pub(super) struct Saved {
    version: u32,
    model: String,
    scopes: BTreeMap<String, Scope>,
}

impl Saved {
    fn is_of(&self, version: u32, model: &str) -> bool {
        self.version == version && self.model == model
    }
}

impl Threads {
    /// The saved placements, or none, which places every note again. Never
    /// an error: they can always be placed again.
    pub fn load(db: &SpaceDb) -> Self {
        let version = db.meta("threads_version").and_then(|v| v.parse().ok());
        let (Some(version), Some(model)) = (version, db.meta("threads_model")) else {
            return Self::default();
        };
        let read = Self::read(db.conn()).map(Some);
        let Some(scopes) = SpaceDb::load_or_default(read, "where notes were placed") else {
            return Self::default();
        };
        Self {
            saved: Some(Saved {
                version,
                model: model.clone(),
                scopes: scopes.clone(),
            }),
            version,
            model,
            scopes,
        }
    }

    /// The general scope from `placed`, the others from `placed_in`.
    fn read(conn: &Connection) -> rusqlite::Result<BTreeMap<String, Scope>> {
        let mut scopes: BTreeMap<String, Scope> = BTreeMap::new();
        let mut statement = conn.prepare(
            "SELECT '' AS scope, id, hash, thread, out FROM placed
             UNION ALL SELECT scope, id, hash, thread, out FROM placed_in",
        )?;
        let rows = statement.query_map([], |row| {
            let placed = Placed {
                hash: row.get(2)?,
                thread: row.get(3)?,
                out: row.get(4)?,
            };
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, placed))
        })?;
        for row in rows {
            let (scope, id, placed) = row?;
            scopes.entry(scope).or_default().insert(id, placed);
        }
        Ok(scopes)
    }

    /// Write the notes placed since the last save, all of it in one go.
    pub fn save(&mut self, db: &mut SpaceDb) -> Result<()> {
        let changes = self.changes();
        db.transaction(|tx| self.write_changes(tx, changes.as_deref()))?;
        match (changes, self.saved.as_mut()) {
            (Some(changes), Some(saved)) => {
                for ((scope, id), placed) in changes {
                    let notes = saved.scopes.entry(scope).or_default();
                    match placed {
                        Some(placed) => notes.insert(id, placed),
                        None => notes.remove(&id),
                    };
                }
                saved.scopes.retain(|_, notes| !notes.is_empty());
            }
            _ => {
                self.saved = Some(Saved {
                    version: self.version,
                    model: self.model.clone(),
                    scopes: self.scopes.clone(),
                });
            }
        }
        Ok(())
    }

    /// The notes placed or forgotten since the last save, by scope and
    /// note, each with where it is now. `None` when every placement is to
    /// be written: none is saved yet, or they were made another way. Both
    /// sides run in scope then note order, so one pass along them finds
    /// every difference.
    fn changes(&self) -> Option<Vec<Change>> {
        let saved = self
            .saved
            .as_ref()
            .filter(|saved| saved.is_of(self.version, &self.model))?;
        let mut changes = Vec::new();
        let mut now = flat(&self.scopes).peekable();
        let mut then = flat(&saved.scopes).peekable();
        let owned = |(scope, id): (&str, &str)| (scope.to_string(), id.to_string());
        loop {
            let order = match (now.peek(), then.peek()) {
                (None, None) => break,
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some((a, _)), Some((b, _))) => a.cmp(b),
            };
            match order {
                std::cmp::Ordering::Less => {
                    let (key, placed) = now.next().expect("peeked");
                    changes.push((owned(key), Some(placed.clone())));
                }
                std::cmp::Ordering::Greater => {
                    let (key, _) = then.next().expect("peeked");
                    changes.push((owned(key), None));
                }
                std::cmp::Ordering::Equal => {
                    let ((key, placed), (_, before)) =
                        (now.next().expect("peeked"), then.next().expect("peeked"));
                    if placed != before {
                        changes.push((owned(key), Some(placed.clone())));
                    }
                }
            }
        }
        Some(changes)
    }

    /// What `save` writes, in the caller's transaction.
    pub(crate) fn write(&self, tx: &Connection) -> Result<()> {
        self.write_changes(tx, self.changes().as_deref())
    }

    /// Write `changes`, or every placement when there are none to go by.
    /// The general scope goes in `placed`, the others in `placed_in`.
    fn write_changes(&self, tx: &Connection, changes: Option<&[Change]>) -> Result<()> {
        let mut put = tx.prepare_cached(
            "INSERT OR REPLACE INTO placed (id, hash, thread, out) VALUES (?1, ?2, ?3, ?4)",
        )?;
        let mut put_in = tx.prepare_cached(
            "INSERT OR REPLACE INTO placed_in (scope, id, hash, thread, out) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut row = |scope: &str, id: &str, placed: &Placed| -> Result<()> {
            let _ = if scope == GENERAL {
                put.execute(rusqlite::params![
                    id,
                    placed.hash,
                    placed.thread,
                    placed.out
                ])
            } else {
                put_in.execute(rusqlite::params![
                    scope,
                    id,
                    placed.hash,
                    placed.thread,
                    placed.out
                ])
            }?;
            Ok(())
        };
        let forget = |scope: &str, id: &str| -> Result<()> {
            let _ = if scope == GENERAL {
                tx.execute("DELETE FROM placed WHERE id = ?1", [id])
            } else {
                tx.execute(
                    "DELETE FROM placed_in WHERE scope = ?1 AND id = ?2",
                    [scope, id],
                )
            }?;
            Ok(())
        };
        match changes {
            Some(changes) => changes
                .iter()
                .try_for_each(|((scope, id), placed)| match placed {
                    Some(placed) => row(scope, id, placed),
                    None => forget(scope, id),
                }),
            None => {
                tx.execute_batch("DELETE FROM placed; DELETE FROM placed_in;")?;
                SpaceDb::set_meta(tx, "threads_version", &self.version.to_string())?;
                SpaceDb::set_meta(tx, "threads_model", &self.model)?;
                flat(&self.scopes).try_for_each(|((scope, id), placed)| row(scope, id, placed))
            }
        }
    }

    /// The placements `threads.json` holds, `None` for a file that is
    /// missing or unreadable. The file was saved through serde, and held
    /// the general scope alone.
    pub fn read_json(path: &Path) -> Option<Self> {
        let legacy: Legacy = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        let mut scopes = BTreeMap::new();
        if !legacy.notes.is_empty() {
            scopes.insert(GENERAL.to_string(), legacy.notes);
        }
        Some(Self {
            version: legacy.version,
            model: legacy.model,
            scopes,
            saved: None,
        })
    }

    /// The file `read_json` reads, as the app saved it before `space.db`.
    #[cfg(test)]
    pub fn to_json(&self) -> String {
        serde_json::to_string(&Legacy {
            version: self.version,
            model: self.model.clone(),
            notes: self.scopes.get(GENERAL).cloned().unwrap_or_default(),
        })
        .unwrap()
    }
}
