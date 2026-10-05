//! `search.db`, where a space's note text lives while the space is open, so
//! the index in memory holds none (SPEC 6).
//!
//! Derived like `index.jsonl`: every row can be made again from the
//! markdown, so a file that is missing, damaged or from another version is
//! made again, and every day is read into it anew.
//!
//! `texts` holds each note's and page's body, and its subject and body folded
//! as search matches them. `texts_fts` indexes the folded text with the
//! trigram tokenizer, which finds any run of three characters or more, so a
//! search still matches inside words. `days` records each day file's time and
//! length as last read, and `page_files` each file under `pages/`, which is
//! how a launch tells the files to read again. `mentions` holds the names
//! each note and page mentions (SPEC 3.10), read as its text goes in.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rusqlite::{params, Connection, OptionalExtension};

use super::daily_file::Note;
use crate::mentions::mentions;
use crate::search::fold;

/// Bumped when the tables change: a file of another version is made again.
const VERSION: i64 = 3;

/// An id is not unique: a note block copied by hand into another day keeps
/// its id, and both copies are still found.
const TABLES: &str = "
    CREATE TABLE texts (
        rowid INTEGER PRIMARY KEY,
        id TEXT NOT NULL,
        -- The day a note is in, or NULL for a page, which has a file of its own.
        day TEXT,
        body TEXT NOT NULL,
        folded TEXT NOT NULL
    );
    CREATE INDEX texts_id ON texts(id);
    CREATE INDEX texts_day ON texts(day);
    CREATE VIRTUAL TABLE texts_fts USING fts5(
        folded, content = 'texts', content_rowid = 'rowid', tokenize = 'trigram'
    );
    CREATE TABLE days (
        day TEXT PRIMARY KEY,
        modified INTEGER NOT NULL,
        len INTEGER NOT NULL
    );
    -- `id` is the page the file held, or NULL for a file that holds none.
    CREATE TABLE page_files (
        path TEXT PRIMARY KEY,
        id TEXT,
        modified INTEGER NOT NULL,
        len INTEGER NOT NULL
    );
    -- Each name a `texts` row mentions: `key` as it is found, lowercase,
    -- and `name` as first typed there. Goes with its row.
    CREATE TABLE mentions (
        text INTEGER NOT NULL,
        key TEXT NOT NULL,
        name TEXT NOT NULL
    );
    CREATE INDEX mentions_key ON mentions(key);
    CREATE INDEX mentions_text ON mentions(text);
    CREATE TRIGGER mentions_removed AFTER DELETE ON texts BEGIN
        DELETE FROM mentions WHERE text = old.rowid;
    END;
";

/// What keeps `texts_fts` in step with `texts`, row by row.
const TRIGGERS: &str = "
    CREATE TRIGGER texts_added AFTER INSERT ON texts BEGIN
        INSERT INTO texts_fts(rowid, folded) VALUES (new.rowid, new.folded);
    END;
    CREATE TRIGGER texts_removed AFTER DELETE ON texts BEGIN
        INSERT INTO texts_fts(texts_fts, rowid, folded) VALUES ('delete', old.rowid, old.folded);
    END;
";

/// The page cache while filling a database, in KiB, against SQLite's usual
/// 2 MB. Given back once the fill is done.
const FILL_CACHE_KIB: i64 = 256 * 1024;

const DROP: &str = "
    DROP TABLE IF EXISTS mentions;
    DROP TABLE IF EXISTS texts_fts;
    DROP TABLE IF EXISTS texts;
    DROP TABLE IF EXISTS days;
    DROP TABLE IF EXISTS page_files;
";

pub fn search_db_path(root: &Path) -> PathBuf {
    root.join(".scratchnote").join("search.db")
}

/// When a day file was last written and how long it was, as last read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    /// Nanoseconds since the epoch.
    pub modified: i64,
    pub len: i64,
}

impl Stamp {
    /// `None` for a file that is not there.
    pub fn of(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        Some(Self {
            modified: i64::try_from(modified.as_nanos()).ok()?,
            len: i64::try_from(meta.len()).ok()?,
        })
    }
}

pub struct SearchDb {
    conn: Connection,
}

fn to_string(e: rusqlite::Error) -> String {
    e.to_string()
}

/// A word as an FTS5 string, which takes it as typed: `"` is doubled.
fn quoted(word: &str) -> String {
    format!("\"{}\"", word.replace('"', "\"\""))
}

/// The trigram tokenizer finds runs of three characters or more. A shorter
/// word is looked for without it.
fn indexable(word: &str) -> bool {
    word.chars().count() >= 3
}

impl SearchDb {
    /// The space's file, made again when it cannot be used.
    pub fn open(root: &Path) -> Result<Self, String> {
        let path = search_db_path(root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        match Self::set_up(Connection::open(&path).map_err(to_string)?) {
            Ok(db) => Ok(db),
            Err(e) => {
                log::warn!("{} is not usable ({e}), making it again", path.display());
                for suffix in ["", "-wal", "-shm"] {
                    let mut file = path.clone().into_os_string();
                    file.push(suffix);
                    let _ = std::fs::remove_file(file);
                }
                Self::set_up(Connection::open(&path).map_err(to_string)?)
            }
        }
    }

    /// A database held in memory, for when the file cannot be opened at all,
    /// and for tests.
    pub fn in_memory() -> Result<Self, String> {
        Self::set_up(Connection::open_in_memory().map_err(to_string)?)
    }

    fn set_up(conn: Connection) -> Result<Self, String> {
        // A derived cache: a crash may lose the last writes, which the next
        // launch reads again, but never leaves the file broken.
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))
            .map_err(to_string)?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(to_string)?;
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(to_string)?;
        if version != VERSION {
            conn.execute_batch(DROP).map_err(to_string)?;
            conn.execute_batch(TABLES).map_err(to_string)?;
            conn.execute_batch(TRIGGERS).map_err(to_string)?;
            conn.pragma_update(None, "user_version", VERSION)
                .map_err(to_string)?;
        }
        Ok(Self { conn })
    }

    /// Run `f` as one transaction, so reading thousands of days costs one
    /// commit rather than one each.
    pub fn at_once<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let began = self.conn.execute_batch("BEGIN").is_ok();
        let out = f(self);
        if began {
            if let Err(e) = self.conn.execute_batch("COMMIT") {
                log::warn!("could not commit to search.db: {e}");
                let _ = self.conn.execute_batch("ROLLBACK");
            }
        }
        out
    }

    /// Run `f` as one transaction with `texts_fts` left alone, then build it
    /// in one pass from `texts`. For filling an empty database, which this
    /// makes several times faster than keeping the index up row by row:
    /// whatever the index held before is built again.
    pub fn in_bulk<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let _ = self.conn.pragma_update(None, "cache_size", -FILL_CACHE_KIB);
        let out = self.at_once(|db| {
            if let Err(e) = db
                .conn
                .execute_batch("DROP TRIGGER texts_added; DROP TRIGGER texts_removed;")
            {
                log::warn!("could not set search.db up for filling: {e}");
            }
            let out = f(db);
            if let Err(e) = db
                .conn
                .execute("INSERT INTO texts_fts(texts_fts) VALUES ('rebuild')", [])
            {
                log::error!("could not build the index of search.db: {e}");
            }
            if let Err(e) = db.conn.execute_batch(TRIGGERS) {
                log::error!("could not restore the triggers of search.db: {e}");
            }
            out
        });
        let _ = self.conn.pragma_update(None, "cache_size", -2000);
        let _ = self.conn.execute_batch("PRAGMA shrink_memory");
        out
    }

    /// Forget everything, as a rebuild does.
    pub fn clear(&mut self) -> Result<(), String> {
        self.conn.execute_batch(DROP).map_err(to_string)?;
        self.conn.execute_batch(TABLES).map_err(to_string)?;
        self.conn.execute_batch(TRIGGERS).map_err(to_string)
    }

    /// Every day file as it was last read.
    pub fn days(&self) -> Result<HashMap<String, Stamp>, String> {
        let mut statement = self
            .conn
            .prepare("SELECT day, modified, len FROM days")
            .map_err(to_string)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    Stamp {
                        modified: row.get(1)?,
                        len: row.get(2)?,
                    },
                ))
            })
            .map_err(to_string)?;
        rows.collect::<Result<_, _>>().map_err(to_string)
    }

    /// Every file under `pages/` as it was last read, by its path relative
    /// to the root.
    pub fn page_files(&self) -> Result<HashMap<String, Stamp>, String> {
        let mut statement = self
            .conn
            .prepare("SELECT path, modified, len FROM page_files")
            .map_err(to_string)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    Stamp {
                        modified: row.get(1)?,
                        len: row.get(2)?,
                    },
                ))
            })
            .map_err(to_string)?;
        rows.collect::<Result<_, _>>().map_err(to_string)
    }

    /// Put a day's notes in place of what it had, and record its file as
    /// read at `stamp`: `None` for a file that is gone.
    pub fn replace_day(
        &mut self,
        date: &str,
        notes: &[Note],
        stamp: Option<Stamp>,
    ) -> Result<(), String> {
        let tx = self.conn.savepoint().map_err(to_string)?;
        tx.execute("DELETE FROM texts WHERE day = ?1", [date])
            .map_err(to_string)?;
        for note in notes {
            insert(&tx, note, Some(date))?;
        }
        stamp_day(&tx, date, stamp)?;
        tx.commit().map_err(to_string)
    }

    /// A note just added to its day, whose file is now at `stamp`.
    pub fn add_note(&mut self, note: &Note, stamp: Option<Stamp>) -> Result<(), String> {
        let tx = self.conn.savepoint().map_err(to_string)?;
        tx.execute("DELETE FROM texts WHERE id = ?1", [&note.id])
            .map_err(to_string)?;
        insert(&tx, note, Some(&note.date))?;
        stamp_day(&tx, &note.date, stamp)?;
        tx.commit().map_err(to_string)
    }

    /// A page's text in place of what it had, if anything, and its file as
    /// read at `stamp`. The file it was read from before, if another, is
    /// forgotten.
    pub fn replace_page(&mut self, page: &Note, stamp: Option<Stamp>) -> Result<(), String> {
        let tx = self.conn.savepoint().map_err(to_string)?;
        tx.execute("DELETE FROM texts WHERE id = ?1", [&page.id])
            .map_err(to_string)?;
        tx.execute(
            "DELETE FROM page_files WHERE id = ?1 OR path = ?2",
            [&page.id, &page.file],
        )
        .map_err(to_string)?;
        insert(&tx, page, None)?;
        if let Some(stamp) = stamp {
            tx.execute(
                "INSERT INTO page_files (path, id, modified, len) VALUES (?1, ?2, ?3, ?4)",
                params![page.file, page.id, stamp.modified, stamp.len],
            )
            .map_err(to_string)?;
        }
        tx.commit().map_err(to_string)
    }

    /// A file under `pages/` that holds no page, as read at `stamp`, so it is
    /// not read again while it stays so. `None` forgets it, and it is read
    /// at the next launch.
    pub fn stamp_page_file(&mut self, path: &str, stamp: Option<Stamp>) -> Result<(), String> {
        match stamp {
            Some(stamp) => self.conn.execute(
                "INSERT INTO page_files (path, id, modified, len) VALUES (?1, NULL, ?2, ?3) \
                 ON CONFLICT(path) DO UPDATE SET id = NULL, modified = excluded.modified, len = excluded.len",
                params![path, stamp.modified, stamp.len],
            ),
            None => self
                .conn
                .execute("DELETE FROM page_files WHERE path = ?1", [path]),
        }
        .map(|_| ())
        .map_err(to_string)
    }

    /// A page gone: its text, and its file as read.
    pub fn remove_page(&mut self, id: &str) -> Result<(), String> {
        let tx = self.conn.savepoint().map_err(to_string)?;
        tx.execute("DELETE FROM texts WHERE id = ?1", [id])
            .map_err(to_string)?;
        tx.execute("DELETE FROM page_files WHERE id = ?1", [id])
            .map_err(to_string)?;
        tx.commit().map_err(to_string)
    }

    /// Forget the files under `pages/` not among `files`, and the text of the
    /// pages not among `ids`.
    pub fn retain_pages(
        &mut self,
        files: &HashSet<&str>,
        ids: &HashSet<&str>,
    ) -> Result<(), String> {
        let known_files = self.ids("SELECT path FROM page_files", &[])?;
        let known_ids = self.ids("SELECT id FROM texts WHERE day IS NULL", &[])?;
        let tx = self.conn.savepoint().map_err(to_string)?;
        for path in known_files
            .iter()
            .filter(|path| !files.contains(path.as_str()))
        {
            tx.execute("DELETE FROM page_files WHERE path = ?1", [path])
                .map_err(to_string)?;
        }
        for id in known_ids.iter().filter(|id| !ids.contains(id.as_str())) {
            tx.execute("DELETE FROM texts WHERE day IS NULL AND id = ?1", [id])
                .map_err(to_string)?;
        }
        tx.commit().map_err(to_string)
    }

    /// Forget the days whose file is gone.
    pub fn retain_days(&mut self, present: &HashSet<String>) -> Result<(), String> {
        let known: Vec<String> = {
            let mut statement = self
                .conn
                .prepare("SELECT day FROM days UNION SELECT DISTINCT day FROM texts WHERE day IS NOT NULL")
                .map_err(to_string)?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(to_string)?;
            rows.collect::<Result<_, _>>().map_err(to_string)?
        };
        let tx = self.conn.savepoint().map_err(to_string)?;
        for day in known.iter().filter(|day| !present.contains(*day)) {
            tx.execute("DELETE FROM texts WHERE day = ?1", [day])
                .map_err(to_string)?;
            tx.execute("DELETE FROM days WHERE day = ?1", [day])
                .map_err(to_string)?;
        }
        tx.commit().map_err(to_string)
    }

    /// The ids whose subject and body, folded, hold every one of `words`,
    /// folded as well.
    pub fn matching(&self, words: &[String]) -> Result<HashSet<String>, String> {
        let long: Vec<String> = words
            .iter()
            .filter(|w| indexable(w))
            .map(|w| quoted(w))
            .collect();
        // The index narrows, `instr` decides, so a match is a plain
        // substring whatever the tokenizer makes of the text.
        let checks = vec!["instr(t.folded, ?) > 0"; words.len()].join(" AND ");
        let (sql, mut args): (String, Vec<String>) = if long.is_empty() {
            (
                format!("SELECT t.id FROM texts t WHERE {checks}"),
                Vec::new(),
            )
        } else {
            (
                format!(
                    "SELECT t.id FROM texts_fts JOIN texts t ON t.rowid = texts_fts.rowid \
                     WHERE texts_fts MATCH ? AND {checks}"
                ),
                vec![long.join(" AND ")],
            )
        };
        args.extend(words.iter().cloned());
        self.ids(&sql, &args)
    }

    /// The ids whose body holds any of `needles` as typed. An empty needle
    /// matches every note.
    pub fn containing(&self, needles: &[String]) -> Result<HashSet<String>, String> {
        let mut found = HashSet::new();
        for needle in needles {
            let ids = if needle.is_empty() {
                self.ids("SELECT id FROM texts", &[])?
            } else if indexable(&fold(needle)) {
                // Folded, the needle narrows on the index; `instr` on the
                // body then keeps only the notes holding it as typed.
                self.ids(
                    "SELECT t.id FROM texts_fts JOIN texts t ON t.rowid = texts_fts.rowid \
                     WHERE texts_fts MATCH ? AND instr(t.body, ?) > 0",
                    &[quoted(&fold(needle)), needle.clone()],
                )?
            } else {
                self.ids(
                    "SELECT id FROM texts WHERE instr(body, ?) > 0",
                    std::slice::from_ref(needle),
                )?
            };
            found.extend(ids);
        }
        Ok(found)
    }

    /// Every name mentioned, by the id of the note or page mentioning it:
    /// `(id, key, name)`.
    pub fn mention_rows(&self) -> Result<Vec<(String, String, String)>, String> {
        let mut statement = self
            .conn
            .prepare("SELECT t.id, m.key, m.name FROM mentions m JOIN texts t ON t.rowid = m.text")
            .map_err(to_string)?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(to_string)?;
        rows.collect::<Result<_, _>>().map_err(to_string)
    }

    /// The notes and pages mentioning the name with this key, by id, with
    /// the name as each types it.
    pub fn mentioning(&self, key: &str) -> Result<HashMap<String, String>, String> {
        let mut statement = self
            .conn
            .prepare("SELECT t.id, m.name FROM mentions m JOIN texts t ON t.rowid = m.text WHERE m.key = ?1")
            .map_err(to_string)?;
        let rows = statement
            .query_map([key], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(to_string)?;
        rows.collect::<Result<_, _>>().map_err(to_string)
    }

    /// The bodies it holds of `ids`.
    pub fn bodies<'a>(
        &self,
        ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<HashMap<String, String>, String> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT body FROM texts WHERE id = ?1")
            .map_err(to_string)?;
        let mut out = HashMap::new();
        for id in ids {
            let body: Option<String> = statement
                .query_row([id], |row| row.get(0))
                .optional()
                .map_err(to_string)?;
            if let Some(body) = body {
                out.insert(id.to_string(), body);
            }
        }
        Ok(out)
    }

    fn ids(&self, sql: &str, args: &[String]) -> Result<HashSet<String>, String> {
        let mut statement = self.conn.prepare(sql).map_err(to_string)?;
        let rows = statement
            .query_map(rusqlite::params_from_iter(args), |row| {
                row.get::<_, String>(0)
            })
            .map_err(to_string)?;
        rows.collect::<Result<_, _>>().map_err(to_string)
    }
}

fn insert(conn: &Connection, note: &Note, day: Option<&str>) -> Result<(), String> {
    let folded = fold(&format!(
        "{}\n{}",
        note.subject.as_deref().unwrap_or(""),
        note.body
    ));
    conn.execute(
        "INSERT INTO texts (id, day, body, folded) VALUES (?1, ?2, ?3, ?4)",
        params![note.id, day, note.body, folded],
    )
    .map_err(to_string)?;
    let text = conn.last_insert_rowid();
    let mut statement = conn
        .prepare_cached("INSERT INTO mentions (text, key, name) VALUES (?1, ?2, ?3)")
        .map_err(to_string)?;
    for mention in mentions(&note.body) {
        statement
            .execute(params![text, mention.key, mention.name])
            .map_err(to_string)?;
    }
    Ok(())
}

fn stamp_day(conn: &Connection, date: &str, stamp: Option<Stamp>) -> Result<(), String> {
    match stamp {
        Some(stamp) => conn.execute(
            "INSERT INTO days (day, modified, len) VALUES (?1, ?2, ?3) \
             ON CONFLICT(day) DO UPDATE SET modified = excluded.modified, len = excluded.len",
            params![date, stamp.modified, stamp.len],
        ),
        None => conn.execute("DELETE FROM days WHERE day = ?1", [date]),
    }
    .map(|_| ())
    .map_err(to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::daily_file::{body_hash, Kind};
    use crate::storage::relative_day_path;

    fn note(id: &str, date: &str, body: &str) -> Note {
        Note {
            id: id.to_string(),
            date: date.to_string(),
            time: "08:00".to_string(),
            file: relative_day_path(date),
            subject: None,
            hash: body_hash(body),
            ahead_off: false,
            body: body.to_string(),
            kind: Kind::Note,
            on: None,
            missing: false,
        }
    }

    fn sorted(ids: HashSet<String>) -> Vec<String> {
        let mut ids: Vec<String> = ids.into_iter().collect();
        ids.sort();
        ids
    }

    fn words(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| fold(w)).collect()
    }

    #[test]
    fn words_match_inside_words_short_ones_too() {
        let mut db = SearchDb::in_memory().unwrap();
        let day = [
            note("01A", "2026-09-22", "Réunion avec l'équipe"),
            note("01B", "2026-09-22", "Buy coffee, 2 bags"),
        ];
        db.replace_day("2026-09-22", &day, None).unwrap();

        assert_eq!(sorted(db.matching(&words(&["REUNION"])).unwrap()), ["01A"]);
        assert_eq!(sorted(db.matching(&words(&["offe"])).unwrap()), ["01B"]);
        assert_eq!(sorted(db.matching(&words(&["of"])).unwrap()), ["01B"]);
        assert_eq!(
            sorted(db.matching(&words(&["2", "bags"])).unwrap()),
            ["01B"]
        );
        assert!(db
            .matching(&words(&["coffee", "reunion"]))
            .unwrap()
            .is_empty());
        assert!(db.matching(&words(&["\"quoted\""])).unwrap().is_empty());
    }

    #[test]
    fn a_bulk_fill_is_found_and_later_changes_still_are() {
        let mut db = SearchDb::in_memory().unwrap();
        db.in_bulk(|db| {
            let day = [note("01A", "2026-09-21", "filled in bulk")];
            db.replace_day("2026-09-21", &day, None).unwrap();
        });
        assert_eq!(sorted(db.matching(&words(&["bulk"])).unwrap()), ["01A"]);

        let day = [note("01B", "2026-09-21", "changed after")];
        db.replace_day("2026-09-21", &day, None).unwrap();
        assert!(db.matching(&words(&["bulk"])).unwrap().is_empty());
        assert_eq!(sorted(db.matching(&words(&["after"])).unwrap()), ["01B"]);
    }

    #[test]
    fn a_note_copied_into_another_day_is_kept_in_both() {
        let mut db = SearchDb::in_memory().unwrap();
        db.replace_day(
            "2026-09-21",
            &[note("01A", "2026-09-21", "copied text")],
            None,
        )
        .unwrap();
        db.replace_day(
            "2026-09-22",
            &[note("01A", "2026-09-22", "copied text")],
            None,
        )
        .unwrap();
        db.replace_day("2026-09-21", &[], None).unwrap();
        assert_eq!(sorted(db.matching(&words(&["copied"])).unwrap()), ["01A"]);
    }

    #[test]
    fn a_day_replaced_loses_what_it_no_longer_holds() {
        let mut db = SearchDb::in_memory().unwrap();
        db.replace_day("2026-09-22", &[note("01A", "2026-09-22", "first")], None)
            .unwrap();
        db.replace_day("2026-09-22", &[note("01B", "2026-09-22", "second")], None)
            .unwrap();
        assert!(db.matching(&words(&["first"])).unwrap().is_empty());
        assert_eq!(sorted(db.matching(&words(&["second"])).unwrap()), ["01B"]);

        db.retain_days(&HashSet::new()).unwrap();
        assert!(db.matching(&words(&["second"])).unwrap().is_empty());
    }

    #[test]
    fn the_names_a_note_mentions_come_and_go_with_it() {
        let mut db = SearchDb::in_memory().unwrap();
        let day = [
            note("01A", "2026-09-22", "@ProjectA is late, ask @marie"),
            note("01B", "2026-09-22", "@projecta again"),
        ];
        db.replace_day("2026-09-22", &day, None).unwrap();
        let found = db.mentioning("projecta").unwrap();
        assert_eq!(sorted(found.keys().cloned().collect()), ["01A", "01B"]);
        assert_eq!(found["01A"], "ProjectA");
        assert_eq!(db.mention_rows().unwrap().len(), 3);

        db.replace_day("2026-09-22", &[note("01B", "2026-09-22", "nothing")], None)
            .unwrap();
        assert!(db.mention_rows().unwrap().is_empty());
        db.add_note(&note("01C", "2026-09-23", "@marie"), None)
            .unwrap();
        db.add_note(&note("01C", "2026-09-23", "@bob"), None)
            .unwrap();
        assert!(db.mentioning("marie").unwrap().is_empty());
        assert_eq!(db.mentioning("bob").unwrap().len(), 1);
    }

    #[test]
    fn needles_match_as_typed() {
        let mut db = SearchDb::in_memory().unwrap();
        let day = [
            note("01A", "2026-09-21", "- [ ] call the bank"),
            note("01B", "2026-09-21", "no tasks here, ask @anna"),
            note("01C", "2026-09-21", "Shopping\n- [X] milk"),
        ];
        db.replace_day("2026-09-21", &day, None).unwrap();

        let find = |needles: &[&str]| {
            let needles: Vec<String> = needles.iter().map(|n| n.to_string()).collect();
            sorted(db.containing(&needles).unwrap())
        };
        assert_eq!(find(&["[ ]", "[x]", "[X]"]), ["01A", "01C"]);
        assert_eq!(find(&["[x]"]), Vec::<String>::new(), "the case is as typed");
        assert_eq!(find(&["@"]), ["01B"]);
        assert_eq!(find(&[""]), ["01A", "01B", "01C"]);
    }

    #[test]
    fn bodies_and_stamps_come_back() {
        let mut db = SearchDb::in_memory().unwrap();
        let stamp = Stamp {
            modified: 42,
            len: 7,
        };
        db.add_note(&note("01A", "2026-09-22", "the body"), Some(stamp))
            .unwrap();
        let mut page = note("01P", "2026-09-22", "page text");
        page.kind = Kind::Page;
        page.file = "pages/2026/plan.md".into();
        db.replace_page(&page, Some(stamp)).unwrap();

        let bodies = db.bodies(["01A", "01P", "01Z"]).unwrap();
        assert_eq!(bodies["01A"], "the body");
        assert_eq!(bodies["01P"], "page text");
        assert!(!bodies.contains_key("01Z"));
        assert_eq!(db.days().unwrap()["2026-09-22"], stamp);
        assert_eq!(db.page_files().unwrap()["pages/2026/plan.md"], stamp);

        // Renamed: the old file is forgotten.
        page.file = "pages/2026/the plan.md".into();
        db.replace_page(&page, Some(stamp)).unwrap();
        let files = db.page_files().unwrap();
        assert_eq!(files.keys().collect::<Vec<_>>(), ["pages/2026/the plan.md"]);

        db.retain_pages(&HashSet::from(["pages/2026/the plan.md"]), &HashSet::new())
            .unwrap();
        assert!(db.bodies(["01P"]).unwrap().is_empty());
        assert_eq!(db.bodies(["01A"]).unwrap().len(), 1, "the notes stay");
        assert_eq!(db.page_files().unwrap().len(), 1);
        db.retain_pages(&HashSet::new(), &HashSet::new()).unwrap();
        assert!(db.page_files().unwrap().is_empty());
    }

    #[test]
    fn a_file_from_another_version_is_made_again() {
        let root = std::env::temp_dir().join("scratchnote-search-db-version");
        let _ = std::fs::remove_dir_all(&root);
        {
            let mut db = SearchDb::open(&root).unwrap();
            db.add_note(&note("01A", "2026-09-22", "kept"), None)
                .unwrap();
        }
        assert_eq!(
            SearchDb::open(&root)
                .unwrap()
                .bodies(["01A"])
                .unwrap()
                .len(),
            1
        );
        {
            let conn = Connection::open(search_db_path(&root)).unwrap();
            conn.pragma_update(None, "user_version", VERSION + 1)
                .unwrap();
        }
        assert!(SearchDb::open(&root)
            .unwrap()
            .bodies(["01A"])
            .unwrap()
            .is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
