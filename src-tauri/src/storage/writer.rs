//! Where the app writes the notes, the pages, the index and its own JSON
//! files under the notes root. Moving whole folders, copying attachments in,
//! installing plugins and moving an old root's notes into a space at launch,
//! before the writer runs, go straight to disk instead: none of those is a
//! file the user may be editing at the same time.
//!
//! Every write is funnelled through one task over an mpsc channel, so two
//! saves can never interleave on the same file. Each write is atomic:
//! temp file, fsync, rename.
//!
//! The task also records the fingerprint of whatever it last wrote to each
//! path. The file watcher reads that to tell the app's own writes apart from
//! somebody editing a note in another editor.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};

use super::daily_file::{self, Note};
use super::fingerprint;
use crate::Result;

/// Fingerprint of the last content the app wrote to each path.
pub type SelfWrites = Arc<Mutex<HashMap<PathBuf, String>>>;

enum WriteRequest {
    /// The whole file, in place of whatever was there.
    Write {
        path: PathBuf,
        contents: String,
        reply: oneshot::Sender<io::Result<()>>,
    },
    /// Read a file, `None` when it is missing, and write back what `edit`
    /// makes of it. Read and write happen in this task, so no other write can
    /// land between them.
    Rewrite {
        path: PathBuf,
        edit: Edit,
        /// False when `edit` left the file alone.
        reply: oneshot::Sender<io::Result<bool>>,
    },
    /// Move a file, refusing to replace another.
    Rename {
        from: PathBuf,
        to: PathBuf,
        reply: oneshot::Sender<io::Result<()>>,
    },
    Remove {
        path: PathBuf,
        /// False when the file was not there.
        reply: oneshot::Sender<io::Result<bool>>,
    },
}

/// What `Writer::rewrite` does to a file's contents; `None` leaves it alone.
pub type Edit = Box<dyn FnOnce(Option<&str>) -> Option<String> + Send>;

#[derive(Clone)]
pub struct Writer {
    tx: mpsc::Sender<WriteRequest>,
    self_writes: SelfWrites,
}

impl Writer {
    pub fn spawn() -> Self {
        let (tx, mut rx) = mpsc::channel::<WriteRequest>(64);
        let self_writes: SelfWrites = Arc::new(Mutex::new(HashMap::new()));

        let seen = self_writes.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(request) = rx.recv().await {
                match request {
                    WriteRequest::Write {
                        path,
                        contents,
                        reply,
                    } => {
                        let _ = reply.send(write_atomic(&path, &contents, &seen).await);
                    }
                    WriteRequest::Rewrite { path, edit, reply } => {
                        let _ = reply.send(rewrite(&path, edit, &seen).await);
                    }
                    WriteRequest::Rename { from, to, reply } => {
                        let _ = reply.send(rename(&from, &to, &seen).await);
                    }
                    WriteRequest::Remove { path, reply } => {
                        let _ = reply.send(remove(&path, &seen).await);
                    }
                }
            }
        });

        Self { tx, self_writes }
    }

    /// Shared with the watcher so it can skip events the app caused itself.
    pub fn self_writes(&self) -> SelfWrites {
        self.self_writes.clone()
    }

    /// Add a note at the end of a day's file, starting the file if there is
    /// none.
    pub async fn append_note(&self, path: PathBuf, date: String, note: Note) -> Result<()> {
        self.rewrite(path, move |existing| {
            Some(daily_file::append_note(
                existing.unwrap_or_default(),
                &note,
                &date,
            ))
        })
        .await?;
        Ok(())
    }

    /// Returns false when the file or the id is not there.
    pub async fn delete_note(&self, path: PathBuf, id: String) -> Result<bool> {
        self.rewrite(path, move |existing| {
            daily_file::remove_note(existing?, &id)
        })
        .await
    }

    /// Swap one note's body after the user edits it in the app. Returns
    /// false when the file or the id is not there.
    pub async fn replace_body(&self, path: PathBuf, id: String, body: String) -> Result<bool> {
        self.rewrite(path, move |existing| {
            daily_file::replace_body(existing?, &id, &body)
        })
        .await
    }

    /// Write the whole file, as settings and the other JSON files are.
    pub async fn write(&self, path: PathBuf, contents: String) -> Result<()> {
        let (reply, response) = oneshot::channel();
        self.send(
            WriteRequest::Write {
                path,
                contents,
                reply,
            },
            response,
        )
        .await
    }

    /// Returns false when `edit` left the file alone.
    pub async fn rewrite(
        &self,
        path: PathBuf,
        edit: impl FnOnce(Option<&str>) -> Option<String> + Send + 'static,
    ) -> Result<bool> {
        let (reply, response) = oneshot::channel();
        self.send(
            WriteRequest::Rewrite {
                path,
                edit: Box::new(edit),
                reply,
            },
            response,
        )
        .await
    }

    /// Fails rather than replace a file already at `to`.
    pub async fn rename(&self, from: PathBuf, to: PathBuf) -> Result<()> {
        let (reply, response) = oneshot::channel();
        self.send(WriteRequest::Rename { from, to, reply }, response)
            .await
    }

    /// Returns false when the file was not there.
    pub async fn remove(&self, path: PathBuf) -> Result<bool> {
        let (reply, response) = oneshot::channel();
        self.send(WriteRequest::Remove { path, reply }, response)
            .await
    }

    async fn send<T>(
        &self,
        request: WriteRequest,
        response: oneshot::Receiver<io::Result<T>>,
    ) -> Result<T> {
        self.tx
            .send(request)
            .await
            .map_err(|_| "writer task is gone")?;
        Ok(response
            .await
            .map_err(|_| "writer task dropped the request")??)
    }
}

/// Read the file, `None` when it is missing, and write back what `edit`
/// makes of it. False when `edit` left it alone.
async fn rewrite(path: &Path, edit: Edit, seen: &SelfWrites) -> io::Result<bool> {
    let existing = match tokio::fs::read_to_string(path).await {
        Ok(contents) => Some(contents),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    match edit(existing.as_deref()) {
        Some(updated) => {
            write_atomic(path, &updated, seen).await?;
            Ok(true)
        }
        None => Ok(false),
    }
}

async fn rename(from: &Path, to: &Path, seen: &SelfWrites) -> io::Result<()> {
    // A rename replaces its target on Windows, so check first. A change of
    // case only is the same file there and has to go through.
    let same_file = from.to_string_lossy().to_lowercase() == to.to_string_lossy().to_lowercase();
    if !same_file && tokio::fs::try_exists(to).await? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} is already there", to.display()),
        ));
    }
    if let Some(parent) = to.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::rename(from, to).await?;
    // The file at `to` is the app's own doing, as a write there would be.
    let contents = tokio::fs::read_to_string(to).await?;
    if let Ok(mut seen) = seen.lock() {
        seen.remove(from);
        seen.insert(to.to_path_buf(), fingerprint(&contents));
    }
    Ok(())
}

async fn remove(path: &Path, seen: &SelfWrites) -> io::Result<bool> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => {
            if let Ok(mut seen) = seen.lock() {
                seen.remove(path);
            }
            Ok(true)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

async fn write_atomic(path: &Path, contents: &str, seen: &SelfWrites) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let tmp = path.with_file_name(format!("{}.tmp", name.to_string_lossy()));

    let mut file = tokio::fs::File::create(&tmp).await?;
    {
        use tokio::io::AsyncWriteExt;
        file.write_all(contents.as_bytes()).await?;
        file.sync_all().await?;
    }
    drop(file);

    tokio::fs::rename(&tmp, path).await?;

    // Recorded after the rename, so the watcher sees this before or alongside
    // the event the rename produces.
    if let Ok(mut seen) = seen.lock() {
        seen.insert(path.to_path_buf(), fingerprint(contents));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::daily_file::{body_hash, parse_notes, Kind};

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("scratchnote-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn note(id: &str, time: &str, body: &str) -> Note {
        Note {
            id: id.to_string(),
            date: "2026-09-22".to_string(),
            time: time.to_string(),
            file: "notes/2026/2026-09-22.md".to_string(),
            subject: None,
            hash: body_hash(body),
            on: None,
            ahead_off: false,
            body: body.to_string(),
            kind: Kind::Note,
            missing: false,
        }
    }

    #[tokio::test]
    async fn rewrites_renames_and_removes_files() {
        let root = scratch_dir("files");
        let a = root.join("pages/2026/a.md");
        let b = root.join("pages/2026/b.md");
        let writer = Writer::spawn();

        // Missing, so the edit sees None and makes the file.
        let made = writer
            .rewrite(a.clone(), |existing| {
                assert!(existing.is_none());
                Some("one".to_string())
            })
            .await
            .unwrap();
        assert!(made);
        // Left alone when the edit says so.
        assert!(!writer.rewrite(a.clone(), |_| None).await.unwrap());
        assert!(writer
            .rewrite(a.clone(), |existing| Some(format!(
                "{} two",
                existing.unwrap()
            )))
            .await
            .unwrap());
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "one two");

        std::fs::write(&b, "other").unwrap();
        assert!(
            writer.rename(a.clone(), b.clone()).await.is_err(),
            "a rename must not replace another file"
        );
        std::fs::remove_file(&b).unwrap();
        writer.rename(a.clone(), b.clone()).await.unwrap();
        assert!(!a.exists());
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "one two");
        let recorded = writer.self_writes().lock().unwrap().get(&b).cloned();
        assert_eq!(
            recorded,
            Some(fingerprint("one two")),
            "the watcher skips it"
        );

        assert!(writer.remove(b.clone()).await.unwrap());
        assert!(!b.exists());
        assert!(!writer.remove(b).await.unwrap());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn appends_notes_through_the_writer_and_reads_them_back() {
        let root = scratch_dir("append");
        let path = crate::storage::day_path(&root, "2026-09-22");
        let writer = Writer::spawn();

        for (id, time, body) in [
            ("01AAA", "08:00", "first note"),
            ("01BBB", "09:30", "second note\nwith two lines"),
        ] {
            writer
                .append_note(path.clone(), "2026-09-22".to_string(), note(id, time, body))
                .await
                .expect("append should succeed");
        }

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.starts_with("# 2026-09-22\n"));

        let parsed = parse_notes(&contents, "2026-09-22", "notes/2026/2026-09-22.md");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].id, "01AAA");
        assert_eq!(parsed[1].body, "second note\nwith two lines");

        // The temp file must not survive a successful write.
        assert!(!path.with_file_name("2026-09-22.md.tmp").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn deletes_a_note_through_the_writer() {
        let root = scratch_dir("delete");
        let path = crate::storage::day_path(&root, "2026-09-22");
        let writer = Writer::spawn();

        for (id, body) in [("01AAA", "keep me"), ("01BBB", "delete me")] {
            writer
                .append_note(
                    path.clone(),
                    "2026-09-22".to_string(),
                    note(id, "08:00", body),
                )
                .await
                .unwrap();
        }

        assert!(writer
            .delete_note(path.clone(), "01BBB".to_string())
            .await
            .unwrap());

        let contents = std::fs::read_to_string(&path).unwrap();
        let left = parse_notes(&contents, "2026-09-22", "notes/2026/2026-09-22.md");
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, "01AAA");
        assert!(!path.with_file_name("2026-09-22.md.tmp").exists());

        // A second delete of the same id is a no-op, not an error.
        assert!(!writer
            .delete_note(path.clone(), "01BBB".to_string())
            .await
            .unwrap());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn deleting_from_a_missing_file_reports_not_found() {
        let root = scratch_dir("delete-missing");
        let path = crate::storage::day_path(&root, "2026-09-22");
        let writer = Writer::spawn();
        assert!(!writer.delete_note(path, "01AAA".to_string()).await.unwrap());
    }

    #[tokio::test]
    async fn rewrites_the_whole_file() {
        let root = scratch_dir("whole-writes");
        let path = root.join(".scratchnote").join("settings.json");
        let writer = Writer::spawn();

        writer
            .write(path.clone(), "{\"id\":\"01AAA\"}\n".to_string())
            .await
            .unwrap();
        writer
            .write(path.clone(), "{\"id\":\"01CCC\"}\n".to_string())
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\"id\":\"01CCC\"}\n"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// The watcher leans on this to ignore the app's own writes.
    #[tokio::test]
    async fn records_the_fingerprint_of_what_it_wrote() {
        let root = scratch_dir("self-writes");
        let path = crate::storage::day_path(&root, "2026-09-22");
        let writer = Writer::spawn();

        writer
            .append_note(
                path.clone(),
                "2026-09-22".to_string(),
                note("01AAA", "08:00", "first"),
            )
            .await
            .unwrap();

        let on_disk = std::fs::read_to_string(&path).unwrap();
        let recorded = writer.self_writes().lock().unwrap().get(&path).cloned();
        assert_eq!(recorded, Some(fingerprint(&on_disk)));

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Twenty writers racing on one file: the single writer task has to
    /// serialise them, so no append may be lost to a read-modify-write overlap.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_appends_all_land_in_the_file() {
        let root = scratch_dir("concurrent");
        let path = crate::storage::day_path(&root, "2026-09-22");
        let writer = Writer::spawn();

        let handles: Vec<_> = (0..20)
            .map(|i| {
                let writer = writer.clone();
                let path = path.clone();
                tokio::spawn(async move {
                    writer
                        .append_note(
                            path,
                            "2026-09-22".to_string(),
                            note(&format!("01ID{i:02}"), "08:00", &format!("body {i}")),
                        )
                        .await
                })
            })
            .collect();

        for handle in handles {
            handle.await.expect("task").expect("append should succeed");
        }

        let contents = std::fs::read_to_string(&path).unwrap();
        let parsed = parse_notes(&contents, "2026-09-22", "notes/2026/2026-09-22.md");
        assert_eq!(
            parsed.len(),
            20,
            "every append should survive serialisation"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
