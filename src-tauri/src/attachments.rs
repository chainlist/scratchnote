//! Attachments, SPEC 3.7 and 4.8: files copied into the space's
//! `attachments/<year>/` folder and linked from a note's markdown.
//!
//! Day files and page files both sit two folders down in the space, so one
//! relative link, `../../attachments/...`, reaches the file from either, in
//! the app and in any other markdown editor. Nothing indexes attachments:
//! the links in the notes are all there is, and a file stays when its note
//! goes.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::Local;
use percent_encoding::percent_decode_str;
use serde::Serialize;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::ipc::InvokeBody;
use tauri::{AppHandle, Manager, State};

use crate::spaces::Space;
use crate::state::AppState;
use crate::storage::activity::{Action, Actor, Event, Target};

/// The folder in a space that holds its attachments.
const DIR: &str = "attachments";
/// The URI scheme the webview loads attached images from.
pub const SCHEME: &str = "attachment";
/// The longest name part of an attachment's file name, in characters, as for
/// a page's file.
const MAX_STEM: usize = 80;
/// Longer than any real extension: past it, the dot is part of the name.
const MAX_EXTENSION: usize = 10;

/// A file copied into the open space, for the link the editor writes.
#[derive(Debug, Serialize)]
pub struct Attachment {
    /// The name the file came with, for the link's text.
    pub name: String,
    /// Where it went, from the space's folder:
    /// `attachments/2026/2026-09-28 shot.png`.
    pub path: String,
}

/// `part` of a file name made safe everywhere: the characters Windows
/// forbids, and the `#` and `%` that break a link in other editors, become
/// spaces. Whitespace collapses, and it is cut to `max` characters without
/// trailing dots or spaces.
fn clean(part: &str, max: usize) -> String {
    let spaced: String = part
        .chars()
        .map(|c| {
            if c.is_control() || r#"<>:"/\|?*#%"#.contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let collapsed = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = collapsed.chars().take(max).collect();
    cut.trim_end_matches(|c: char| c == '.' || c.is_whitespace())
        .to_string()
}

/// The file name for `original` attached on `date`: `<date> <name>.<ext>`,
/// with ` 2`, ` 3` and so on before the extension for `n` above 1, when the
/// plain name is taken.
pub fn file_name(date: &str, original: &str, n: usize) -> String {
    let (stem, extension) = match original.rsplit_once('.') {
        Some((stem, ext))
            if !stem.trim().is_empty()
                && !ext.is_empty()
                && ext.len() <= MAX_EXTENSION
                && ext.chars().all(|c| c.is_ascii_alphanumeric()) =>
        {
            (stem, Some(ext))
        }
        _ => (original, None),
    };
    let stem = clean(stem, MAX_STEM);
    let stem = if stem.is_empty() { "attachment" } else { &stem };
    let suffix = if n > 1 {
        format!(" {n}")
    } else {
        String::new()
    };
    match extension {
        Some(ext) => format!("{date} {stem}{suffix}.{ext}"),
        None => format!("{date} {stem}{suffix}"),
    }
}

/// A new file in `dir` for `original`, under the first free name.
/// `create_new` claims the name, so two attachments never share a file.
fn create(dir: &Path, date: &str, original: &str) -> io::Result<(File, String)> {
    fs::create_dir_all(dir)?;
    let mut n = 1;
    loop {
        let name = file_name(date, original, n);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dir.join(&name))
        {
            Ok(file) => return Ok((file, name)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(e),
        }
    }
}

/// Write `original` into the space at `root`, its contents from `write`,
/// and say where it went. A write that fails leaves no file behind.
fn store(
    root: &Path,
    date: &str,
    original: &str,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<Attachment> {
    let year = &date[..4];
    let dir = root.join(DIR).join(year);
    let (mut file, name) = create(&dir, date, original)?;
    if let Err(e) = write(&mut file).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(dir.join(&name));
        return Err(e);
    }
    Ok(Attachment {
        name: original.to_string(),
        path: format!("{DIR}/{year}/{name}"),
    })
}

fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// Put the files copied in on the space's activity log (SPEC 4.11).
async fn record(state: &AppState, space: &Space, attached: &[Attachment]) {
    for attachment in attached {
        let event = Event {
            file: Some(attachment.path.clone()),
            subject: Some(attachment.name.clone()),
            ..Event::new(Actor::User, Action::Attach, Target::Attachment)
        };
        space.record(&state.writer, event).await;
    }
}

/// Copy files from disk into the open space: dropped on an editor, or picked
/// with its attach button. Every path is checked before anything is copied,
/// so a folder among them copies nothing.
#[tauri::command]
pub async fn add_attachments(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<Vec<Attachment>, String> {
    let space = state.space()?;
    let root = space.root.clone();
    let attached = tauri::async_runtime::spawn_blocking(move || {
        for path in &paths {
            match fs::metadata(path) {
                Ok(meta) if meta.is_file() => {}
                Ok(_) => return Err(format!("{path} is not a file")),
                Err(e) => return Err(format!("could not attach {path}: {e}")),
            }
        }
        let date = today();
        paths
            .iter()
            .map(|path| {
                let source = Path::new(path);
                let name = source
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                File::open(source)
                    .and_then(|mut from| {
                        store(&root, &date, &name, |to| {
                            io::copy(&mut from, to).map(|_| ())
                        })
                    })
                    .map_err(|e| format!("could not attach {path}: {e}"))
            })
            .collect::<Result<Vec<_>, _>>()
    })
    .await
    .map_err(|e| e.to_string())??;
    record(&state, &space, &attached).await;
    Ok(attached)
}

/// Save a file pasted into an editor, a screenshot say, into the open space.
/// The request's body is the file; its `x-name` header, URI-encoded, is the
/// name it came with.
#[tauri::command]
pub async fn save_attachment(
    state: State<'_, AppState>,
    request: tauri::ipc::Request<'_>,
) -> Result<Attachment, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected the file's bytes".to_string());
    };
    let name = request
        .headers()
        .get("x-name")
        .and_then(|value| value.to_str().ok())
        .map(|value| percent_decode_str(value).decode_utf8_lossy().into_owned())
        .unwrap_or_default();
    let space = state.space()?;
    let root = space.root.clone();
    let bytes = bytes.clone();
    let original = name.clone();
    let attached = tauri::async_runtime::spawn_blocking(move || {
        store(&root, &today(), &original, |to| to.write_all(&bytes))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("could not attach {name}: {e}"))?;
    record(&state, &space, std::slice::from_ref(&attached)).await;
    Ok(attached)
}

/// The file a space-relative `path` names, when it is inside the space's
/// attachments folder: `attachments/2026/x.png`. Anything that could climb
/// out of it is refused.
fn resolve(root: &Path, path: &str) -> Option<PathBuf> {
    let mut parts = path.split('/');
    if parts.next() != Some(DIR) {
        return None;
    }
    let mut file = root.join(DIR);
    let mut named = false;
    for part in parts {
        let safe = !part.is_empty()
            && part != "."
            && part != ".."
            && !part.contains(['\\', ':'])
            && !part.chars().any(char::is_control);
        if !safe {
            return None;
        }
        file.push(part);
        named = true;
    }
    named.then_some(file)
}

/// Open an attachment of the open space in its own app, as a double click
/// in the file manager would.
#[tauri::command]
pub fn open_attachment(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let file =
        resolve(&state.space()?.root, &path).ok_or_else(|| format!("not an attachment: {path}"))?;
    if !file.is_file() {
        return Err(format!("{path} is not there"));
    }
    app.opener()
        .open_path(file.to_string_lossy(), None::<&str>)
        .map_err(|e| format!("could not open {path}: {e}"))
}

/// What the webview is told an attached file is, so it draws the images.
fn content_type(file: &Path) -> &'static str {
    let extension = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = code;
    response
}

/// Answer the webview's request for an attached file, sent to
/// `attachment://localhost/<space>/attachments/<year>/<file>` by
/// `convertFileSrc`, which URI-encodes the path. Only files in a space's
/// attachments folder are served, and a sandbox keeps an SVG opened as a
/// page from running scripts.
pub async fn serve(app: AppHandle, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let path = percent_decode_str(request.uri().path())
        .decode_utf8_lossy()
        .into_owned();
    let found = path
        .trim_start_matches('/')
        .split_once('/')
        .and_then(|(space, rest)| {
            let space = app.try_state::<AppState>()?.find_space(space)?;
            resolve(&space.root, rest)
        });
    let Some(file) = found else {
        return status(StatusCode::NOT_FOUND);
    };
    match tokio::fs::read(&file).await {
        Ok(bytes) => Response::builder()
            .header(header::CONTENT_TYPE, content_type(&file))
            .header(header::CONTENT_SECURITY_POLICY, "sandbox")
            .body(bytes)
            .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR)),
        Err(_) => status(StatusCode::NOT_FOUND),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("scratchnote-attachments-{name}"));
        let _ = fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn names_a_file_after_its_day_and_the_name_it_came_with() {
        assert_eq!(
            file_name("2026-09-28", "shot.png", 1),
            "2026-09-28 shot.png"
        );
        assert_eq!(
            file_name("2026-09-28", "shot.png", 2),
            "2026-09-28 shot 2.png"
        );
        assert_eq!(
            file_name("2026-09-28", "archive.tar.gz", 1),
            "2026-09-28 archive.tar.gz"
        );
        assert_eq!(file_name("2026-09-28", "README", 3), "2026-09-28 README 3");
    }

    #[test]
    fn a_name_that_would_break_a_path_or_a_link_is_made_safe() {
        assert_eq!(
            file_name("2026-09-28", "a<b>:c#1 %done?.pdf", 1),
            "2026-09-28 a b c 1 done.pdf"
        );
        assert_eq!(file_name("2026-09-28", "", 1), "2026-09-28 attachment");
        assert_eq!(
            file_name("2026-09-28", "???.png", 1),
            "2026-09-28 attachment.png"
        );
        let long = format!("{}.png", "x".repeat(200));
        assert_eq!(
            file_name("2026-09-28", &long, 1),
            format!("2026-09-28 {}.png", "x".repeat(MAX_STEM))
        );
    }

    #[test]
    fn a_dot_that_starts_no_extension_stays_in_the_name() {
        assert_eq!(
            file_name("2026-09-28", "notes.v2 from the meeting", 1),
            "2026-09-28 notes.v2 from the meeting"
        );
        assert_eq!(file_name("2026-09-28", ".env", 1), "2026-09-28 .env");
    }

    #[test]
    fn stores_under_the_year_and_never_over_an_existing_file() {
        let root = scratch("store");
        let first = store(&root, "2026-09-28", "shot.png", |f| f.write_all(b"one")).unwrap();
        let second = store(&root, "2026-09-28", "shot.png", |f| f.write_all(b"two")).unwrap();
        assert_eq!(first.path, "attachments/2026/2026-09-28 shot.png");
        assert_eq!(second.path, "attachments/2026/2026-09-28 shot 2.png");
        assert_eq!(second.name, "shot.png");
        assert_eq!(fs::read(root.join(&first.path)).unwrap(), b"one");
        assert_eq!(fs::read(root.join(&second.path)).unwrap(), b"two");
    }

    #[test]
    fn a_failed_write_leaves_no_file() {
        let root = scratch("failed");
        let result = store(&root, "2026-09-28", "shot.png", |_| {
            Err(io::Error::other("disk full"))
        });
        assert!(result.is_err());
        let dir = root.join("attachments").join("2026");
        assert_eq!(fs::read_dir(dir).unwrap().count(), 0);
    }

    #[test]
    fn resolves_only_files_inside_the_attachments_folder() {
        let root = Path::new("/space");
        assert_eq!(
            resolve(root, "attachments/2026/a b.png"),
            Some(root.join("attachments").join("2026").join("a b.png"))
        );
        for bad in [
            "",
            "attachments",
            "attachments/",
            "notes/2026/2026-09-28.md",
            "attachments/../notes/2026/2026-09-28.md",
            "attachments/2026/..",
            "attachments/./x.png",
            "attachments//x.png",
            "attachments/C:/x.png",
            "attachments/2026\\..\\..\\x.png",
        ] {
            assert_eq!(resolve(root, bad), None, "{bad} should be refused");
        }
    }

    #[test]
    fn images_are_served_as_images() {
        assert_eq!(content_type(Path::new("a.PNG")), "image/png");
        assert_eq!(content_type(Path::new("a.jpeg")), "image/jpeg");
        assert_eq!(content_type(Path::new("a.svg")), "image/svg+xml");
        assert_eq!(content_type(Path::new("a.pdf")), "application/octet-stream");
        assert_eq!(content_type(Path::new("a")), "application/octet-stream");
    }
}
