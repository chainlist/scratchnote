//! `space.db`, what a space keeps beside its notes other than their text:
//! the note embeddings, the thread each note was placed in, where each note
//! sits on the map of the space and the categories it is in there, and what
//! the user decided about threads (SPEC 6), and the threads and views pinned
//! to the left edge (SPEC 3.13). Its text is in `search.db`, which serves
//! search alone.
//!
//! SQLite, so a change writes its own rows rather than a whole file. The
//! vectors, placements and positions are derived: made with another model or version,
//! they are replaced, and every note is embedded or placed again. The thread
//! edits and the pins are the user's, so nothing here drops them.
//!
//! It replaces `vectors.bin`, `threads.json` and `thread-edits.json`, read in
//! when the file is first made and then removed.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Transaction};

use crate::embed::threads::{Edits, Threads};
use crate::embed::vectors::Vectors;
use crate::storage::paths::meta_dir;
use crate::storage::sqlite;
use crate::Result;

/// Bumped when the tables change. A later version moves the thread edits
/// over to its tables, never drops them.
const VERSION: i64 = 4;

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

/// The tables version 2 added.
const CATEGORIES: &str = "
    -- Groups of notes close together on the map, each inside its parent,
    -- shown from level `low` to `high`, and by note the smallest it is in.
    CREATE TABLE categories (
        id INTEGER PRIMARY KEY,
        parent INTEGER,
        low INTEGER NOT NULL,
        high INTEGER NOT NULL,
        name TEXT NOT NULL
    );
    CREATE TABLE category_notes (
        note TEXT PRIMARY KEY,
        category INTEGER NOT NULL
    );
";

/// The table version 3 added.
const PINS: &str = "
    -- What is pinned to the left edge, in its order: a thread by its id, or
    -- a plugin's page by its type and query, with the title it had then.
    CREATE TABLE pins (
        position INTEGER PRIMARY KEY,
        kind TEXT NOT NULL,
        target TEXT NOT NULL,
        label TEXT
    );
";

/// The tables version 4 added: where each note was placed, and where the
/// user put it, in the threads of each name mentioned (SPEC 6.4). The
/// general scope stays in `placed` and `pinned`, which a version before
/// reads as it did.
const SCOPES: &str = "
    CREATE TABLE placed_in (
        scope TEXT NOT NULL,
        id TEXT NOT NULL,
        hash TEXT NOT NULL,
        thread TEXT,
        out INTEGER NOT NULL,
        PRIMARY KEY (scope, id)
    ) WITHOUT ROWID;
    CREATE TABLE pinned_in (
        scope TEXT NOT NULL,
        note TEXT NOT NULL,
        thread TEXT NOT NULL,
        PRIMARY KEY (scope, note)
    ) WITHOUT ROWID;
";

pub fn space_db_path(root: &Path) -> PathBuf {
    meta_dir(root).join("space.db")
}

/// The files `space.db` replaces.
fn old_files(root: &Path) -> [PathBuf; 3] {
    let dir = meta_dir(root);
    [
        dir.join("vectors.bin"),
        dir.join("threads.json"),
        dir.join("thread-edits.json"),
    ]
}

pub struct SpaceDb {
    conn: Connection,
}

impl SpaceDb {
    /// The space's file. Made, the first time, from the files it replaces,
    /// which are then removed. Unlike `search.db`, a file that cannot be
    /// used is left alone: it holds the user's thread edits.
    pub fn open(root: &Path) -> Result<Self> {
        let mut db = Self::set_up(sqlite::open(&space_db_path(root))?)?;
        let version = sqlite::version(&db.conn)?;
        if version == 0 {
            db.make(Some(root))?;
        } else if version < VERSION {
            db.transaction(|tx| {
                if version < 2 {
                    tx.execute_batch(CATEGORIES)?;
                }
                if version < 3 {
                    tx.execute_batch(PINS)?;
                }
                tx.execute_batch(SCOPES)?;
                sqlite::set_version(tx, VERSION)
            })?;
        }
        Ok(db)
    }

    /// A database held in memory, for when the file cannot be opened, and
    /// for tests.
    pub fn in_memory() -> Result<Self> {
        let mut db = Self::set_up(Connection::open_in_memory()?)?;
        db.make(None)?;
        Ok(db)
    }

    fn set_up(conn: Connection) -> Result<Self> {
        // A vector is about 3 KB, so a 4 KB page holds one and wastes a
        // quarter; a 16 KB page holds five. Only takes on a new file, so
        // before the journal mode, which fixes it.
        conn.pragma_update(None, "page_size", PAGE_SIZE)?;
        // Each save is on disk before it returns, as the files it replaces
        // were: the thread edits cannot be read again from anywhere.
        sqlite::write_ahead(&conn, "FULL")?;
        // The log is copied into the file every 1 MB and cut back to that
        // after, where by default it would keep the size of the largest
        // write, such as reading in a whole `vectors.bin`.
        conn.pragma_update(None, "wal_autocheckpoint", WAL_BYTES / PAGE_SIZE)?;
        conn.pragma_update(None, "journal_size_limit", WAL_BYTES)?;
        Ok(Self { conn })
    }

    /// Make the tables and fill them from the files of the space at `from`,
    /// in one transaction, then remove those files. Should it fail, the
    /// next open tries again: the version is only set with the tables. A
    /// file missing or unreadable is skipped, as it was read as empty.
    fn make(&mut self, from: Option<&Path>) -> Result<()> {
        let old = from.map(|root| {
            let [vectors, threads, edits] = old_files(root);
            (
                Vectors::read_bin(&vectors),
                Threads::read_json(&threads),
                Edits::read_json(&edits),
            )
        });
        self.transaction(|tx| {
            tx.execute_batch(TABLES)?;
            tx.execute_batch(CATEGORIES)?;
            tx.execute_batch(PINS)?;
            tx.execute_batch(SCOPES)?;
            sqlite::set_version(tx, VERSION)?;
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
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
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

    /// What `read` read of this database, or `T::default()` when it failed,
    /// which is logged as not reading `what`: a space whose rows cannot be
    /// read opens without them rather than not at all.
    pub(crate) fn load_or_default<T: Default>(read: rusqlite::Result<T>, what: &str) -> T {
        read.unwrap_or_else(|e| {
            log::warn!("could not read {what}: {e}");
            T::default()
        })
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Run `write` as one transaction: all of it is saved, or none.
    pub(crate) fn transaction<T>(
        &mut self,
        write: impl FnOnce(&Transaction) -> Result<T>,
    ) -> Result<T> {
        let tx = self.conn.transaction()?;
        let out = write(&tx)?;
        tx.commit()?;
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

    pub(crate) fn set_meta(tx: &Connection, key: &str, value: &str) -> Result<()> {
        tx.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
            [key, value],
        )?;
        Ok(())
    }

    /// Forget every vector, as a rebuild does, so each note is embedded
    /// again. Where notes were placed stays: placing again follows the new
    /// vectors.
    pub fn clear_vectors(&mut self) -> Result<()> {
        self.transaction(|tx| {
            tx.execute("DELETE FROM vectors", [])?;
            tx.execute(
                "DELETE FROM meta WHERE key IN ('vectors_model', 'vectors_dims')",
                [],
            )?;
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
    fn a_file_of_version_1_gets_the_categories_and_keeps_the_rest() {
        let root = scratch("version-1");
        {
            let conn = Connection::open(space_db_path(&root)).unwrap();
            conn.execute_batch(TABLES).unwrap();
            conn.pragma_update(None, "user_version", 1).unwrap();
            conn.execute("INSERT INTO kept_alone (note) VALUES ('01D')", [])
                .unwrap();
        }
        let db = SpaceDb::open(&root).unwrap();
        assert!(Edits::load(&db).alone.contains("01D"));
        let count: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM categories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
        drop(db);
        // Opened again, nothing is made twice.
        drop(SpaceDb::open(&root).unwrap());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_of_version_3_gets_the_scopes_and_keeps_the_rest() {
        let root = scratch("version-3");
        {
            let conn = Connection::open(space_db_path(&root)).unwrap();
            conn.execute_batch(TABLES).unwrap();
            conn.execute_batch(CATEGORIES).unwrap();
            conn.execute_batch(PINS).unwrap();
            conn.pragma_update(None, "user_version", 3).unwrap();
            conn.execute(
                "INSERT INTO pinned (note, thread) VALUES ('01B', '01A')",
                [],
            )
            .unwrap();
        }
        let db = SpaceDb::open(&root).unwrap();
        let edits = Edits::load(&db);
        assert_eq!(
            edits
                .put(crate::embed::threads::GENERAL, "01B")
                .map(String::as_str),
            Some("01A")
        );
        assert!(edits.pinned_in.is_empty());
        drop(db);
        drop(SpaceDb::open(&root).unwrap());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_of_version_2_gets_the_pins_and_keeps_the_rest() {
        let root = scratch("version-2");
        {
            let conn = Connection::open(space_db_path(&root)).unwrap();
            conn.execute_batch(TABLES).unwrap();
            conn.execute_batch(CATEGORIES).unwrap();
            conn.pragma_update(None, "user_version", 2).unwrap();
            conn.execute("INSERT INTO kept_alone (note) VALUES ('01D')", [])
                .unwrap();
        }
        let db = SpaceDb::open(&root).unwrap();
        assert!(Edits::load(&db).alone.contains("01D"));
        assert!(crate::storage::pins::load(&db).is_empty());
        drop(db);
        drop(SpaceDb::open(&root).unwrap());
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
