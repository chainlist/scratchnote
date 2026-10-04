//! `space.db`, what a space keeps beside its notes other than their text:
//! the note embeddings, the thread each note was placed in, where each note
//! sits on the map of the space, and what the user decided about threads
//! (SPEC 6). Its text is in `search.db`, which
//! serves search alone.
//!
//! SQLite, so a change writes its own rows rather than a whole file. The
//! vectors, placements and positions are derived: made with another model or version,
//! they are replaced, and every note is embedded or placed again. The thread
//! edits are the user's, so nothing here drops them.
//!
//! It replaces `vectors.bin`, `threads.json` and `thread-edits.json`, read in
//! when the file is first made and then removed.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Transaction};

use crate::embed::threads::{Edits, Threads};
use crate::embed::vectors::Vectors;

/// Bumped when the tables change. A later version moves the thread edits
/// over to its tables, never drops them.
const VERSION: i64 = 1;

const PAGE_SIZE: i64 = 16 * 1024;
const WAL_BYTES: i64 = 1024 * 1024;

const TABLES: &str = "
    -- What the vectors and placements were made with, by name.
    CREATE TABLE meta (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    -- A note's unit vector as little-endian f32, and the body hash it was
    -- made from.
    CREATE TABLE vectors (
        id TEXT PRIMARY KEY,
        hash TEXT NOT NULL,
        vector BLOB NOT NULL
    );
    -- Where a note was placed, and from which body. `thread` is NULL for a
    -- note on its own, `out` set when it was placed while kept out.
    CREATE TABLE placed (
        id TEXT PRIMARY KEY,
        hash TEXT NOT NULL,
        thread TEXT,
        out INTEGER NOT NULL
    );
    -- Where each note sits on the map of the space, and from which body.
    CREATE TABLE positions (
        id TEXT PRIMARY KEY,
        hash TEXT NOT NULL,
        x REAL NOT NULL,
        y REAL NOT NULL
    );
    -- Each note's closest notes on the map, and how close.
    CREATE TABLE links (
        id TEXT NOT NULL,
        other TEXT NOT NULL,
        score REAL NOT NULL,
        PRIMARY KEY (id, other)
    ) WITHOUT ROWID;
    -- The thread edits: titles, notes kept out of threads, notes put in one,
    -- and the notes a dismissed suggestion held.
    CREATE TABLE thread_titles (
        thread TEXT PRIMARY KEY,
        title TEXT NOT NULL
    );
    CREATE TABLE kept_alone (
        note TEXT PRIMARY KEY
    );
    CREATE TABLE pinned (
        note TEXT PRIMARY KEY,
        thread TEXT NOT NULL
    );
    CREATE TABLE dismissed (
        thread TEXT NOT NULL,
        note TEXT NOT NULL,
        PRIMARY KEY (thread, note)
    );
";

pub fn space_db_path(root: &Path) -> PathBuf {
    root.join(".scratchnote").join("space.db")
}

/// The files `space.db` replaces.
fn old_files(root: &Path) -> [PathBuf; 3] {
    let dir = root.join(".scratchnote");
    [
        dir.join("vectors.bin"),
        dir.join("threads.json"),
        dir.join("thread-edits.json"),
    ]
}

pub struct SpaceDb {
    conn: Connection,
}

pub(crate) fn to_string(e: rusqlite::Error) -> String {
    e.to_string()
}

impl SpaceDb {
    /// The space's file. Made, the first time, from the files it replaces,
    /// which are then removed. Unlike `search.db`, a file that cannot be
    /// used is left alone: it holds the user's thread edits.
    pub fn open(root: &Path) -> Result<Self, String> {
        let path = space_db_path(root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut db = Self::set_up(Connection::open(&path).map_err(to_string)?)?;
        let version: i64 = db
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(to_string)?;
        if version == 0 {
            db.make(Some(root))?;
        }
        Ok(db)
    }

    /// A database held in memory, for when the file cannot be opened, and
    /// for tests.
    pub fn in_memory() -> Result<Self, String> {
        let mut db = Self::set_up(Connection::open_in_memory().map_err(to_string)?)?;
        db.make(None)?;
        Ok(db)
    }

    fn set_up(conn: Connection) -> Result<Self, String> {
        // A vector is about 3 KB, so a 4 KB page holds one and wastes a
        // quarter; a 16 KB page holds five. Only takes on a new file, so
        // before the journal mode, which fixes it.
        conn.pragma_update(None, "page_size", PAGE_SIZE)
            .map_err(to_string)?;
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))
            .map_err(to_string)?;
        // Each save is on disk before it returns, as the files it replaces
        // were: the thread edits cannot be read again from anywhere.
        conn.pragma_update(None, "synchronous", "FULL")
            .map_err(to_string)?;
        // The log is copied into the file every 1 MB and cut back to that
        // after, where by default it would keep the size of the largest
        // write, such as reading in a whole `vectors.bin`.
        conn.pragma_update(None, "wal_autocheckpoint", WAL_BYTES / PAGE_SIZE)
            .map_err(to_string)?;
        conn.pragma_update(None, "journal_size_limit", WAL_BYTES)
            .map_err(to_string)?;
        Ok(Self { conn })
    }

    /// Make the tables and fill them from the files of the space at `from`,
    /// in one transaction, then remove those files. Should it fail, the
    /// next open tries again: the version is only set with the tables. A
    /// file missing or unreadable is skipped, as it was read as empty.
    fn make(&mut self, from: Option<&Path>) -> Result<(), String> {
        let old = from.map(|root| {
            let [vectors, threads, edits] = old_files(root);
            (
                Vectors::read_bin(&vectors),
                Threads::read_json(&threads),
                Edits::read_json(&edits),
            )
        });
        self.transaction(|tx| {
            tx.execute_batch(TABLES).map_err(to_string)?;
            tx.pragma_update(None, "user_version", VERSION)
                .map_err(to_string)?;
            if let Some((vectors, threads, edits)) = &old {
                if let Some(vectors) = vectors {
                    vectors.write(tx)?;
                }
                if let Some(threads) = threads {
                    threads.write(tx)?;
                }
                if let Some(edits) = edits {
                    edits.write(tx)?;
                }
            }
            Ok(())
        })?;
        // The log now holds all that was read in: copied into the file and
        // emptied at once rather than at the next write.
        if old.is_some() {
            self.conn
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
                .map_err(to_string)?;
        }
        for file in from.iter().flat_map(|root| old_files(root)) {
            match std::fs::remove_file(&file) {
                Ok(()) => log::info!("moved {} into space.db", file.display()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => log::warn!("could not remove {}: {e}", file.display()),
            }
        }
        Ok(())
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Run `write` as one transaction: all of it is saved, or none.
    pub(crate) fn transaction<T>(
        &mut self,
        write: impl FnOnce(&Transaction) -> Result<T, String>,
    ) -> Result<T, String> {
        let tx = self.conn.transaction().map_err(to_string)?;
        let out = write(&tx)?;
        tx.commit().map_err(to_string)?;
        Ok(out)
    }

    pub(crate) fn meta(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .ok()
            .flatten()
    }

    pub(crate) fn set_meta(tx: &Connection, key: &str, value: &str) -> Result<(), String> {
        tx.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
            [key, value],
        )
        .map(|_| ())
        .map_err(to_string)
    }

    /// Forget every vector, as a rebuild does, so each note is embedded
    /// again. Where notes were placed stays: placing again follows the new
    /// vectors.
    pub fn clear_vectors(&mut self) -> Result<(), String> {
        self.transaction(|tx| {
            tx.execute("DELETE FROM vectors", []).map_err(to_string)?;
            tx.execute(
                "DELETE FROM meta WHERE key IN ('vectors_model', 'vectors_dims')",
                [],
            )
            .map_err(to_string)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use chrono::NaiveDate;

    use crate::embed::threads::When;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("scratchnote-space-db-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
        root
    }

    /// Three notes about one thing among others, placed.
    fn placed() -> (Vectors, Threads, HashMap<String, When>) {
        let mut vectors = Vectors::new("m", 8);
        let mut when = HashMap::new();
        for (n, (id, about)) in [
            ("01A", 1),
            ("01B", 1),
            ("01C", 1),
            ("01D", 2),
            ("01E", 3),
            ("01F", 4),
        ]
        .into_iter()
        .enumerate()
        {
            let mut v = vec![0.0; 8];
            v[0] = 1.0;
            v[about] = 1.0;
            vectors.insert(id.into(), format!("h{id}"), v).unwrap();
            let date = NaiveDate::from_ymd_opt(2026, 9, 10 + n as u32).unwrap();
            when.insert(
                id.to_string(),
                When {
                    date,
                    time: "09:00".into(),
                },
            );
        }
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &Edits::default());
        (vectors, threads, when)
    }

    fn edits() -> Edits {
        let mut edits = Edits::default();
        edits.titles.insert("01A".into(), "Kitchen".into());
        edits.alone.insert("01D".into());
        edits.pinned.insert("01B".into(), "01A".into());
        edits
            .dismissed
            .insert("01E".into(), ["01E".to_string(), "01F".to_string()].into());
        edits
    }

    #[test]
    fn the_old_files_are_read_in_once_then_removed() {
        let root = scratch("import");
        let (vectors, threads, when) = placed();
        let [bin, json, edits_json] = old_files(&root);
        vectors.write_bin(&bin);
        std::fs::write(&json, threads.to_json()).unwrap();
        std::fs::write(&edits_json, serde_json::to_string(&edits()).unwrap()).unwrap();

        let db = SpaceDb::open(&root).unwrap();
        assert_eq!(Vectors::load(&db, "m", 8), vectors);
        let loaded = Threads::load(&db);
        assert_eq!(loaded, threads);
        assert_eq!(loaded.list(&when, &edits()), threads.list(&when, &edits()));
        assert_eq!(Edits::load(&db), edits());
        assert!(old_files(&root).iter().all(|file| !file.exists()));
        drop(db);

        // Files that turn up again later are not read: the database holds
        // what the space has.
        std::fs::write(&edits_json, "{}").unwrap();
        let db = SpaceDb::open(&root).unwrap();
        assert_eq!(Edits::load(&db), edits());
        drop(db);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_space_without_old_files_starts_empty() {
        let root = scratch("fresh");
        let db = SpaceDb::open(&root).unwrap();
        assert_eq!(Vectors::load(&db, "m", 8), Vectors::new("m", 8));
        assert_eq!(Threads::load(&db), Threads::default());
        assert_eq!(Edits::load(&db), Edits::default());
        drop(db);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn another_model_or_a_rebuild_leaves_the_thread_edits_alone() {
        let mut db = SpaceDb::in_memory().unwrap();
        edits().save(&mut db).unwrap();
        let (mut vectors, _, _) = placed();
        vectors.save(&mut db).unwrap();

        let mut other = Vectors::new("other", 8);
        other.save(&mut db).unwrap();
        assert_eq!(Vectors::load(&db, "m", 8), Vectors::new("m", 8));
        db.clear_vectors().unwrap();
        assert_eq!(Edits::load(&db), edits());
    }
}
