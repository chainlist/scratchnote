//! The only place that writes note files.
//!
//! Every write is funnelled through one task over an mpsc channel, so two
//! saves can never interleave on the same file. Each write is atomic:
//! temp file, fsync, rename.

use std::io;
use std::path::{Path, PathBuf};

use tokio::sync::{mpsc, oneshot};

use super::daily_file::{self, Note};

enum WriteRequest {
    AppendNote {
        path: PathBuf,
        date: String,
        note: Box<Note>,
        reply: oneshot::Sender<io::Result<()>>,
    },
    DeleteNote {
        path: PathBuf,
        id: String,
        /// False when the file or the id is not there.
        reply: oneshot::Sender<io::Result<bool>>,
    },
}

#[derive(Clone)]
pub struct Writer {
    tx: mpsc::Sender<WriteRequest>,
}

impl Writer {
    pub fn spawn() -> Self {
        let (tx, mut rx) = mpsc::channel::<WriteRequest>(64);
        tauri::async_runtime::spawn(async move {
            while let Some(request) = rx.recv().await {
                match request {
                    WriteRequest::AppendNote {
                        path,
                        date,
                        note,
                        reply,
                    } => {
                        let _ = reply.send(append_note(&path, &date, &note).await);
                    }
                    WriteRequest::DeleteNote { path, id, reply } => {
                        let _ = reply.send(delete_note(&path, &id).await);
                    }
                }
            }
        });
        Self { tx }
    }

    pub async fn append_note(&self, path: PathBuf, date: String, note: Note) -> Result<(), String> {
        let (reply, response) = oneshot::channel();
        self.tx
            .send(WriteRequest::AppendNote {
                path,
                date,
                note: Box::new(note),
                reply,
            })
            .await
            .map_err(|_| "writer task is gone".to_string())?;
        response
            .await
            .map_err(|_| "writer task dropped the request".to_string())?
            .map_err(|e| e.to_string())
    }

    /// Returns false when the file or the id is not there.
    pub async fn delete_note(&self, path: PathBuf, id: String) -> Result<bool, String> {
        let (reply, response) = oneshot::channel();
        self.tx
            .send(WriteRequest::DeleteNote { path, id, reply })
            .await
            .map_err(|_| "writer task is gone".to_string())?;
        response
            .await
            .map_err(|_| "writer task dropped the request".to_string())?
            .map_err(|e| e.to_string())
    }
}

async fn append_note(path: &Path, date: &str, note: &Note) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let existing = match tokio::fs::read_to_string(path).await {
        Ok(contents) => contents,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    write_atomic(path, &daily_file::append_note(&existing, note, date)).await
}

async fn delete_note(path: &Path, id: &str) -> io::Result<bool> {
    let existing = match tokio::fs::read_to_string(path).await {
        Ok(contents) => contents,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    match daily_file::remove_note(&existing, id) {
        Some(updated) => {
            write_atomic(path, &updated).await?;
            Ok(true)
        }
        None => Ok(false),
    }
}

async fn write_atomic(path: &Path, contents: &str) -> io::Result<()> {
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

    tokio::fs::rename(&tmp, path).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::daily_file::{body_hash, parse_notes, Status};

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
            summary: None,
            tags: Vec::new(),
            status: Status::Pending,
            hash: body_hash(body),
            body: body.to_string(),
        }
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
        assert!(parsed.iter().all(|n| n.status == Status::Pending));

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
