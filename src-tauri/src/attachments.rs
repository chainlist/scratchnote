//! Attachments, SPEC 3.7 and 4.8: files copied into the space's
//! `attachments/<year>/` folder and linked from a note's markdown.
//!
//! Day files and page files both sit two folders down in the space, so one
//! relative link, `../../attachments/...`, reaches the file from either, in
//! the app and in any other markdown editor. Nothing indexes attachments:
//! the links in the notes are all there is, and a file stays when its note
//! goes, though it follows one moved to another space. The commands that
//! add and open them are in `commands::attachments`.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};

use percent_encoding::percent_decode_str;
use serde::Serialize;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{AppHandle, Manager};

use crate::spaces::Space;
use crate::state::AppState;

/// The folder in a space that holds its attachments.
const DIR: &str = "attachments";
/// The way up from a day's file or a page's file to the space's folder,
/// where a link to an attachment starts.
const TO_SPACE: &str = "../../";
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

/// A file name split before its extension, when it has one: a short run of
/// letters and digits after the last dot, with a name before it.
fn split_extension(name: &str) -> (&str, Option<&str>) {
    match name.rsplit_once('.') {
        Some((stem, ext))
            if !stem.trim().is_empty()
                && !ext.is_empty()
                && ext.len() <= MAX_EXTENSION
                && ext.chars().all(|c| c.is_ascii_alphanumeric()) =>
        {
            (stem, Some(ext))
        }
        _ => (name, None),
    }
}

/// `stem` and `extension` put back together, with ` 2`, ` 3` and so on
/// before the extension for `n` above 1.
fn numbered(stem: &str, extension: Option<&str>, n: usize) -> String {
    let suffix = if n > 1 {
        format!(" {n}")
    } else {
        String::new()
    };
    match extension {
        Some(ext) => format!("{stem}{suffix}.{ext}"),
        None => format!("{stem}{suffix}"),
    }
}

/// The file name for `original` attached on `date`: `<date> <name>.<ext>`,
/// with ` 2`, ` 3` and so on before the extension for `n` above 1, when the
/// plain name is taken.
pub fn file_name(date: &str, original: &str, n: usize) -> String {
    let (stem, extension) = split_extension(original);
    let stem = clean(stem, MAX_STEM);
    let stem = if stem.is_empty() { "attachment" } else { &stem };
    numbered(&format!("{date} {stem}"), extension, n)
}

/// A new file in `dir` under the first free name of `name_for(1)`,
/// `name_for(2)` and so on. `create_new` claims the name, so two
/// attachments never share a file.
fn create(dir: &Path, name_for: impl Fn(usize) -> String) -> io::Result<(File, String)> {
    fs::create_dir_all(dir)?;
    let mut n = 1;
    loop {
        let name = name_for(n);
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
pub fn store(
    root: &Path,
    date: &str,
    original: &str,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<Attachment> {
    let year = &date[..4];
    let dir = root.join(DIR).join(year);
    let (mut file, name) = create(&dir, |n| file_name(date, original, n))?;
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

/// The file a space-relative `path` names, when it is inside the space's
/// attachments folder: `attachments/2026/x.png`. Anything that could climb
/// out of it is refused.
pub fn resolve(root: &Path, path: &str) -> Option<PathBuf> {
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

/// An attachment on its way from one space to another: the file, and the
/// one claimed for it in the other space's same folder.
struct Transfer {
    source: PathBuf,
    /// Holds the name until the file takes its place.
    claimed: File,
    target: PathBuf,
    /// The target, from its space's folder.
    path: String,
}

/// Claim a name for the attachment at `path` of the space at `from` in the
/// same folder of the space at `to`: its own unless that is taken there.
/// `None` when there is no file to take, as when it was moved or deleted by
/// hand.
fn transfer(from: &Path, to: &Path, path: &str) -> io::Result<Option<Transfer>> {
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("not an attachment: {path}"),
        )
    };
    let source = resolve(from, path).ok_or_else(invalid)?;
    if !source.is_file() {
        return Ok(None);
    }
    let wanted = resolve(to, path).ok_or_else(invalid)?;
    let (Some(dir), Some(name)) = (wanted.parent(), wanted.file_name().and_then(|n| n.to_str()))
    else {
        return Err(invalid());
    };
    let (stem, extension) = split_extension(name);
    let (claimed, name) = create(dir, |n| numbered(stem, extension, n))?;
    let folder = path.rsplit_once('/').map_or(DIR, |(folder, _)| folder);
    Ok(Some(Transfer {
        source,
        claimed,
        target: dir.join(&name),
        path: format!("{folder}/{name}"),
    }))
}

/// Copy `source` into `file`, the new file at `at`, and sync it. A copy that
/// fails leaves no file at `at`.
fn copy_into(source: &Path, mut file: File, at: &Path) -> io::Result<()> {
    let copied = File::open(source)
        .and_then(|mut from| io::copy(&mut from, &mut file))
        .and_then(|_| file.sync_all());
    if copied.is_err() {
        drop(file);
        let _ = fs::remove_file(at);
    }
    copied
}

/// Move the attachment at `path` from the space at `from` to the same folder
/// of the space at `to`, under its name unless that is taken there, and say
/// where it went. With no file there to move, as when it was moved or
/// deleted by hand, the path comes back as it was.
pub fn relocate(from: &Path, to: &Path, path: &str) -> io::Result<String> {
    let Some(transfer) = transfer(from, to, path)? else {
        return Ok(path.to_string());
    };
    drop(transfer.claimed);
    // The move takes the place of the empty file that claimed the name.
    if fs::rename(&transfer.source, &transfer.target).is_err() {
        // Across drives, as with a space folder linked from another one.
        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&transfer.target)?;
        copy_into(&transfer.source, file, &transfer.target)?;
        let _ = fs::remove_file(&transfer.source);
    }
    Ok(transfer.path)
}

/// Copy the attachment at `path` of the space at `from` into the same folder
/// of the space at `to`, under its name unless that is taken there, and say
/// where the copy went. `None` when there is no file to copy.
fn duplicate(from: &Path, to: &Path, path: &str) -> io::Result<Option<String>> {
    let Some(transfer) = transfer(from, to, path)? else {
        return Ok(None);
    };
    copy_into(&transfer.source, transfer.claimed, &transfer.target)?;
    Ok(Some(transfer.path))
}

/// Every link in `text` to an attachment: where its destination is written,
/// and the file it names from the space's folder. The app writes them as
/// `(<../../attachments/...>)`; other editors may leave out the brackets and
/// encode the spaces, as `(../../attachments/2026/a%20b.png)`.
fn links(text: &str) -> Vec<(Range<usize>, String)> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find("](") {
        let start = from + at + 2;
        let rest = &text[start..];
        // The destination ends its line's link: `>)` in brackets, else the
        // first `)`, with no space before it.
        let len = if rest.starts_with('<') {
            rest.find(['>', '\n'])
                .filter(|&i| rest[i..].starts_with(">)"))
                .map(|i| i + 1)
        } else {
            rest.find(|c: char| c == ')' || c.is_whitespace())
                .filter(|&i| rest[i..].starts_with(')'))
        };
        from = start;
        let Some(len) = len else { continue };
        let written = &rest[..len];
        let url = written
            .strip_prefix('<')
            .and_then(|url| url.strip_suffix('>'))
            .unwrap_or(written);
        let url = percent_decode_str(url).decode_utf8_lossy();
        if let Some(path) = url.strip_prefix(TO_SPACE) {
            if resolve(Path::new(""), path).is_some() {
                found.push((start..start + len, path.to_string()));
            }
        }
        from = start + len;
    }
    found
}

/// The attachments `text` links, each once, in the order it first links them.
pub fn linked(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    links(text)
        .into_iter()
        .map(|(_, path)| path)
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

/// `text` with its links to each attachment of `moved` pointed at the file it
/// maps to, written as the app writes them. Every other byte stays.
pub fn relink(text: &str, moved: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut kept = 0;
    for (range, path) in links(text) {
        if let Some(to) = moved.get(&path).filter(|to| **to != path) {
            out.push_str(&text[kept..range.start]);
            out.push_str(&format!("<{TO_SPACE}{to}>"));
            kept = range.end;
        }
    }
    out.push_str(&text[kept..]);
    out
}

/// Copy the files `text` links from the space at `from` into the space at
/// `to`, for a note or a page moving there. Returns its text linking the
/// copies, and the files copied, which leave `from` once the note has (see
/// `drop_carried`). A copy that fails takes back those made before it.
pub fn carry(from: &Path, to: &Path, text: &str) -> io::Result<(String, Vec<String>)> {
    let mut moved = HashMap::new();
    let mut copies = Vec::new();
    for path in linked(text) {
        match duplicate(from, to, &path) {
            Ok(Some(copy)) => {
                copies.push(copy.clone());
                moved.insert(path, copy);
            }
            Ok(None) => {}
            Err(e) => {
                for copy in &copies {
                    let _ = resolve(to, copy).map(fs::remove_file);
                }
                return Err(e);
            }
        }
    }
    let text = relink(text, &moved);
    Ok((text, moved.into_keys().collect()))
}

/// Remove the files a note took to another space (`carry`) from `space`,
/// those none of its notes and pages links any more. Nothing goes while the
/// space is not open, since there is then no telling.
pub fn drop_carried(space: &Space, carried: &[String]) {
    for path in carried {
        // As the app writes it, and with its spaces encoded, as other
        // editors may.
        let needles = vec![path.clone(), path.replace(' ', "%20")];
        let unlinked = space
            .read(|_, db| db.containing(&needles))
            .ok()
            .flatten()
            .is_some_and(|ids| ids.is_empty());
        if !unlinked {
            continue;
        }
        if let Some(file) = resolve(&space.root, path) {
            if let Err(e) = fs::remove_file(&file) {
                log::warn!("could not remove {path} from {}: {e}", space.name);
            }
        }
    }
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
    use std::io::Write;

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
    fn a_move_to_another_space_keeps_the_name_unless_it_is_taken() {
        let root = scratch("relocate");
        let (work, home) = (root.join("Work"), root.join("Home"));
        let shot = store(&work, "2026-09-28", "shot.png", |f| f.write_all(b"one")).unwrap();
        let moved = relocate(&work, &home, &shot.path).unwrap();
        assert_eq!(moved, shot.path);
        assert_eq!(fs::read(home.join(&moved)).unwrap(), b"one");
        assert!(!work.join(&shot.path).exists());

        // Home already has a file of that name, which stays as it is.
        let again = store(&work, "2026-09-28", "shot.png", |f| f.write_all(b"two")).unwrap();
        assert_eq!(again.path, shot.path, "free again in Work");
        let renamed = relocate(&work, &home, &again.path).unwrap();
        assert_eq!(renamed, "attachments/2026/2026-09-28 shot 2.png");
        assert_eq!(fs::read(home.join(&moved)).unwrap(), b"one");
        assert_eq!(fs::read(home.join(&renamed)).unwrap(), b"two");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_move_with_nothing_to_move_leaves_the_path_and_refuses_others() {
        let root = scratch("relocate-missing");
        let (work, home) = (root.join("Work"), root.join("Home"));
        let gone = "attachments/2026/2026-09-28 gone.png";
        assert_eq!(relocate(&work, &home, gone).unwrap(), gone);
        assert!(!home.join("attachments").exists());
        assert!(relocate(&work, &home, "notes/2026/2026-09-28.md").is_err());
        assert!(relocate(&work, &home, "attachments/../x.png").is_err());
    }

    #[test]
    fn finds_the_attachments_a_text_links_written_either_way() {
        let text = "![shot.png](<../../attachments/2026/2026-09-28 shot.png>)\n\
                    [Manual](../../attachments/2026/2026-09-28%20manual.pdf) and \
                    [again](<../../attachments/2026/2026-09-28 shot.png>)\n\
                    [web](https://example.com) [day](../../notes/2026/2026-09-28.md) \
                    [up](<../../attachments/../notes/x.md>) [open](<../../attachments/2026/x.png";
        assert_eq!(
            linked(text),
            [
                "attachments/2026/2026-09-28 shot.png",
                "attachments/2026/2026-09-28 manual.pdf"
            ]
        );
    }

    #[test]
    fn relinks_only_the_files_that_moved_and_keeps_every_other_byte() {
        let text = "Before ![a](<../../attachments/2026/a.png>) mid \
                    [b](../../attachments/2026/b%20c.pdf) [k](<../../attachments/2026/k.png>) after";
        let moved = HashMap::from([
            (
                "attachments/2026/a.png".to_string(),
                "attachments/2026/a 2.png".to_string(),
            ),
            (
                "attachments/2026/b c.pdf".to_string(),
                "attachments/2026/b c 2.pdf".to_string(),
            ),
            (
                "attachments/2026/k.png".to_string(),
                "attachments/2026/k.png".to_string(),
            ),
        ]);
        assert_eq!(
            relink(text, &moved),
            "Before ![a](<../../attachments/2026/a 2.png>) mid \
             [b](<../../attachments/2026/b c 2.pdf>) [k](<../../attachments/2026/k.png>) after"
        );
    }

    #[test]
    fn a_carried_note_links_copies_named_free_in_the_other_space() {
        let root = scratch("carry");
        let (work, home) = (root.join("Work"), root.join("Home"));
        let shot = store(&work, "2026-09-28", "shot.png", |f| f.write_all(b"work")).unwrap();
        store(&home, "2026-09-28", "shot.png", |f| f.write_all(b"home")).unwrap();
        let gone = "attachments/2026/2026-09-28 gone.png";
        let text = format!("![shot](<../../{}>) ![gone](<../../{gone}>)", shot.path);

        let (carried, copied) = carry(&work, &home, &text).unwrap();
        let copy = "attachments/2026/2026-09-28 shot 2.png";
        assert_eq!(
            carried,
            format!("![shot](<../../{copy}>) ![gone](<../../{gone}>)")
        );
        assert_eq!(
            copied,
            [shot.path.as_str()],
            "nothing to copy for a gone file"
        );
        assert_eq!(fs::read(home.join(copy)).unwrap(), b"work");
        assert_eq!(fs::read(home.join(&shot.path)).unwrap(), b"home");
        // The original stays until the note has left.
        assert_eq!(fs::read(work.join(&shot.path)).unwrap(), b"work");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_carried_file_leaves_once_no_other_note_links_it() {
        use crate::storage::daily_file::{append_note, body_hash, Kind, Note};
        let root = scratch("drop-carried");
        let shared = store(&root, "2026-09-28", "shared shot.png", |f| {
            f.write_all(b"x")
        })
        .unwrap();
        let own = store(&root, "2026-09-28", "own file.png", |f| f.write_all(b"y")).unwrap();
        // Another note still links the shared file, with its spaces encoded.
        let body = format!("![s](../../{})", shared.path.replace(' ', "%20"));
        let note = Note {
            id: "01STAYS".to_string(),
            date: "2026-09-28".to_string(),
            time: "09:00".to_string(),
            file: crate::storage::relative_day_path("2026-09-28"),
            subject: None,
            hash: body_hash(&body),
            ahead_off: false,
            body,
            kind: Kind::Note,
            on: None,
            missing: false,
        };
        let day = crate::storage::day_path(&root, "2026-09-28");
        fs::create_dir_all(day.parent().unwrap()).unwrap();
        fs::write(&day, append_note("", &note, "2026-09-28")).unwrap();

        let wake = std::sync::Arc::new(tokio::sync::Notify::new());
        let closed = Space::new("Test", root.clone(), wake.clone());
        drop_carried(&closed, std::slice::from_ref(&own.path));
        assert!(root.join(&own.path).exists(), "no telling while closed");
        drop(closed);

        let (space, _) = Space::open("Test", root.clone(), wake);
        drop_carried(&space, &[shared.path.clone(), own.path.clone()]);
        assert!(root.join(&shared.path).exists());
        assert!(!root.join(&own.path).exists());
        drop(space);
        let _ = fs::remove_dir_all(&root);
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
