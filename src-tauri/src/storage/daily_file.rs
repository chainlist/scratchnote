//! Parser and renderer for the daily markdown file, the source of truth.
//!
//! Note blocks are delimited by `<!-- sn:note ... -->` / `<!-- sn:end -->`.
//! Parsing relies only on those markers, so anything the user writes between
//! blocks is never interpreted and never touched. A page's stub (SPEC 4.7)
//! opens with `<!-- sn:page ... -->` instead, so the note parser skips it.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const NOTE_OPEN: &str = "<!-- sn:note ";
/// Opens a page's stub here, and the marker line of a page file.
pub const PAGE_OPEN: &str = "<!-- sn:page ";
const NOTE_END: &str = "<!-- sn:end -->";
const UNTITLED: &str = "(untitled)";

/// A note in a day's file, or a page with a file of its own (SPEC 4.7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Note,
    Page,
}

impl Kind {
    /// Notes leave `kind` out of the index, which predates pages.
    pub fn is_note(&self) -> bool {
        *self == Kind::Note
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pending,
    Done,
    Failed,
    Manual,
}

impl Status {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::Done => "done",
            Status::Failed => "failed",
            Status::Manual => "manual",
        }
    }

    pub(crate) fn parse(raw: &str) -> Option<Self> {
        match raw {
            "pending" => Some(Status::Pending),
            "done" => Some(Status::Done),
            "failed" => Some(Status::Failed),
            "manual" => Some(Status::Manual),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub date: String,
    pub time: String,
    pub file: String,
    pub subject: Option<String>,
    /// The broad subject the note is filed under, one of `categories.json`.
    pub category: Option<String>,
    pub status: Status,
    pub hash: String,
    /// The locale the model labelled the note in, such as "fr". `None` on a
    /// note it has not labelled, or labelled before this was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    pub body: String,
    /// A page's subject is its title, and `file` its own file.
    #[serde(default, skip_serializing_if = "Kind::is_note")]
    pub kind: Kind,
    /// A page whose stub is in the day's file but whose file is gone. Only
    /// the day view has these; they are never indexed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub missing: bool,
}

/// First 8 hex chars of SHA-256 over the trimmed body.
pub fn body_hash(body: &str) -> String {
    let digest = Sha256::digest(body.trim().as_bytes());
    digest[..4].iter().map(|b| format!("{b:02x}")).collect()
}

pub fn render_note(note: &Note) -> String {
    let lang = note
        .lang
        .as_deref()
        .map(|lang| format!(" lang={lang}"))
        .unwrap_or_default();
    let mut out = format!(
        "{}id={} time={} status={} hash={}{lang} -->\n",
        NOTE_OPEN,
        note.id,
        note.time,
        note.status.as_str(),
        note.hash
    );
    out.push_str("### ");
    out.push_str(note.subject.as_deref().unwrap_or(UNTITLED));
    out.push('\n');
    // Written as a tag, so other markdown tools see it as one too.
    if let Some(category) = &note.category {
        out.push_str("> #");
        out.push_str(category);
        out.push_str("\n\n");
    }
    out.push_str(note.body.trim_end());
    out.push('\n');
    out.push_str(NOTE_END);
    out.push('\n');
    out
}

/// Append a rendered note, preserving every existing byte of the file.
pub fn append_note(existing: &str, note: &Note, date: &str) -> String {
    append_block(existing, &render_note(note), date)
}

/// Append a rendered block after a blank line, starting a new file with the
/// day's heading.
fn append_block(existing: &str, block: &str, date: &str) -> String {
    let mut out = if existing.trim().is_empty() {
        format!("# {date}\n")
    } else {
        existing.to_string()
    };
    if !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.ends_with("\n\n") {
        out.push('\n');
    }
    out.push_str(block);
    out
}

/// Parse every well-formed note block. Malformed blocks are skipped rather
/// than guessed at, so a broken marker costs one note and not the whole day.
pub fn parse_notes(content: &str, date: &str, file: &str) -> Vec<Note> {
    let mut notes = Vec::new();
    let mut open: Option<(&str, Vec<&str>)> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(NOTE_OPEN) && trimmed.ends_with("-->") {
            // A second opener before an end marker means the previous block was
            // never closed; drop it.
            open = Some((trimmed, Vec::new()));
        } else if trimmed == NOTE_END {
            if let Some((header, block)) = open.take() {
                if let Some(note) = build_note(header, &block, date, file) {
                    notes.push(note);
                }
            }
        } else if let Some((_, block)) = open.as_mut() {
            block.push(line);
        }
    }

    notes
}

/// Remove one note block by id. Works on lines rather than reparsing and
/// re-rendering the file, so every other byte, including the user's own prose
/// and any hand edits inside other blocks, is carried across verbatim.
/// Returns `None` when the id is not in this file.
pub fn remove_note(content: &str, id: &str) -> Option<String> {
    remove_block(content, NOTE_OPEN, id)
}

/// Remove the block opened by `open` that carries `id`, as `remove_note`.
fn remove_block(content: &str, open: &str, id: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let (start, end) = find_block(&lines, open, id)?;

    // Also take the blank line the writer puts between blocks, or deleting
    // would leave a growing gap. Prefer the one after the block; fall back to
    // the one before it when the block was last.
    let mut first = start;
    let mut last = end;
    if lines.get(end + 1).is_some_and(|l| l.trim().is_empty()) {
        last = end + 1;
    } else if end + 1 == lines.len() && start > 0 && lines[start - 1].trim().is_empty() {
        first = start - 1;
    }

    // `lines()` strips line endings, so rejoin with whatever the file used.
    // An external Windows editor may well have rewritten it as CRLF.
    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let kept: Vec<&str> = lines[..first]
        .iter()
        .chain(&lines[last + 1..])
        .copied()
        .collect();

    let mut out = kept.join(newline);
    if !out.is_empty() {
        out.push_str(newline);
    }
    Some(out)
}

/// What enrichment, or a manual edit, changes about a note. The body is never
/// touched here, so the hash stays valid.
#[derive(Debug, Clone)]
pub struct NotePatch {
    pub subject: Option<String>,
    pub category: Option<String>,
    pub status: Status,
    pub lang: Option<String>,
}

/// Rewrite one note's block in place, keeping its body and every byte of the
/// file outside that block. Returns `None` when the id is not in this file.
pub fn update_note(content: &str, id: &str, patch: &NotePatch) -> Option<String> {
    rewrite_block(content, id, |note| {
        note.subject = patch.subject.clone();
        note.category = patch.category.clone();
        note.status = patch.status;
        note.lang = patch.lang.clone();
    })
}

/// Swap one note's body, and its hash with it, leaving the metadata and every
/// byte outside the block alone. Returns `None` when the id is not in this file.
pub fn replace_body(content: &str, id: &str, body: &str) -> Option<String> {
    rewrite_block(content, id, |note| {
        note.body = body.trim().to_string();
        note.hash = body_hash(body);
    })
}

fn rewrite_block(content: &str, id: &str, edit: impl FnOnce(&mut Note)) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let (start, end) = find_block(&lines, NOTE_OPEN, id)?;

    let mut note = build_note(lines[start].trim(), &lines[start + 1..end], "", "")?;
    edit(&mut note);

    Some(splice(content, &lines, start, end, &render_note(&note)))
}

/// `content` with lines `start..=end` swapped for `replacement`, in the line
/// endings the file already uses.
fn splice(content: &str, lines: &[&str], start: usize, end: usize, replacement: &str) -> String {
    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };

    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    out.extend_from_slice(&lines[..start]);
    out.extend(replacement.lines());
    out.extend_from_slice(&lines[end + 1..]);

    let mut joined = out.join(newline);
    if !joined.is_empty() {
        joined.push_str(newline);
    }
    joined
}

/// Line span of the block opened by `open` that carries `id`, opener and end
/// marker included.
fn find_block(lines: &[&str], open: &str, id: &str) -> Option<(usize, usize)> {
    let mut start = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with(open) && trimmed.ends_with("-->") {
            // A non-matching opener clears the candidate, which also drops a
            // previous block that was never closed.
            start = header_id(trimmed, open)
                .filter(|found| *found == id)
                .map(|_| i);
        } else if trimmed == NOTE_END {
            if let Some(start) = start {
                return Some((start, i));
            }
        }
    }
    None
}

fn header_id<'a>(header: &'a str, open: &str) -> Option<&'a str> {
    let attrs = header.strip_prefix(open)?.strip_suffix("-->")?.trim();
    attrs.split_whitespace().find_map(|p| p.strip_prefix("id="))
}

/// Where a page sits in a day's file (SPEC 4.7): its id and time, and a link
/// to its file for other editors. The page file holds everything else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stub {
    pub id: String,
    pub time: String,
    /// The link's text: the page's title when the stub was written.
    pub title: String,
    /// The link's target, relative to the day's file.
    pub target: String,
}

impl Stub {
    /// The stub a page should have in its day's file.
    pub fn for_page(page: &Note) -> Self {
        Self {
            id: page.id.clone(),
            time: page.time.clone(),
            title: page.subject.clone().unwrap_or_default(),
            // Day files sit two folders down, in `notes/<year>/`.
            target: format!("../../{}", page.file),
        }
    }
}

pub fn render_stub(stub: &Stub) -> String {
    // The target goes in angle brackets, since page file names have spaces.
    format!(
        "{PAGE_OPEN}id={} time={} -->\n[{}](<{}>)\n{NOTE_END}\n",
        stub.id,
        stub.time,
        escape_link_text(&stub.title),
        stub.target
    )
}

/// Every well-formed stub in a day's file, in file order.
pub fn parse_stubs(content: &str) -> Vec<Stub> {
    let mut stubs = Vec::new();
    let mut open: Option<(&str, Vec<&str>)> = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(PAGE_OPEN) && trimmed.ends_with("-->") {
            open = Some((trimmed, Vec::new()));
        } else if trimmed.starts_with(NOTE_OPEN) {
            // A note block starting means the stub before it was never closed.
            open = None;
        } else if trimmed == NOTE_END {
            if let Some((header, block)) = open.take() {
                stubs.extend(build_stub(header, &block));
            }
        } else if let Some((_, block)) = open.as_mut() {
            block.push(line);
        }
    }
    stubs
}

fn build_stub(header: &str, block: &[&str]) -> Option<Stub> {
    let attrs = header.strip_prefix(PAGE_OPEN)?.strip_suffix("-->")?.trim();
    let (mut id, mut time) = (None, None);
    for pair in attrs.split_whitespace() {
        match pair.split_once('=') {
            Some(("id", v)) => id = Some(v.to_string()),
            Some(("time", v)) => time = Some(v.to_string()),
            _ => {}
        }
    }
    // A link edited out of shape still leaves the stub, which the app then
    // rewrites from the page.
    let (title, target) = block
        .iter()
        .find(|line| !line.trim().is_empty())
        .and_then(|line| parse_link(line.trim()))
        .unwrap_or_default();
    Some(Stub {
        id: id?,
        time: time?,
        title,
        target,
    })
}

/// `[text](<target>)`, or `[text](target)` as another editor may leave it.
fn parse_link(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix('[')?;
    let mut text = String::new();
    let mut chars = rest.char_indices();
    let close = loop {
        let (i, c) = chars.next()?;
        match c {
            '\\' => text.push(chars.next()?.1),
            ']' => break i,
            _ => text.push(c),
        }
    };
    let target = rest[close + 1..].strip_prefix('(')?.strip_suffix(')')?;
    let target = target
        .strip_prefix('<')
        .and_then(|t| t.strip_suffix('>'))
        .unwrap_or(target);
    Some((text, target.to_string()))
}

fn escape_link_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '[' | ']') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Add a stub at the end of a day's file, as a note is appended.
pub fn append_stub(existing: &str, stub: &Stub, date: &str) -> String {
    append_block(existing, &render_stub(stub), date)
}

/// Rewrite one stub in place. `None` when it is not in this file.
pub fn replace_stub(content: &str, stub: &Stub) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let (start, end) = find_block(&lines, PAGE_OPEN, &stub.id)?;
    Some(splice(content, &lines, start, end, &render_stub(stub)))
}

/// Remove one stub, as `remove_note` removes a note. `None` when it is not
/// in this file.
pub fn remove_stub(content: &str, id: &str) -> Option<String> {
    remove_block(content, PAGE_OPEN, id)
}

/// Put a stub where a note's block was, for a note turned into a page.
/// `None` when the note is not in this file.
pub fn note_to_stub(content: &str, note_id: &str, stub: &Stub) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let (start, end) = find_block(&lines, NOTE_OPEN, note_id)?;
    Some(splice(content, &lines, start, end, &render_stub(stub)))
}

fn build_note(header: &str, block: &[&str], date: &str, file: &str) -> Option<Note> {
    let attrs = header.strip_prefix(NOTE_OPEN)?.strip_suffix("-->")?.trim();

    let (mut id, mut time, mut status, mut hash, mut lang) = (None, None, None, None, None);
    for pair in attrs.split_whitespace() {
        match pair.split_once('=') {
            Some(("id", v)) => id = Some(v.to_string()),
            Some(("time", v)) => time = Some(v.to_string()),
            Some(("status", v)) => status = Status::parse(v),
            Some(("hash", v)) => hash = Some(v.to_string()),
            Some(("lang", v)) => lang = Some(v.to_string()),
            _ => {}
        }
    }

    let (subject, category, body) = parse_block(block);

    // A body that no longer matches the hash written with it was edited in
    // another editor (SPEC 4.3). Labels the model wrote describe the old text,
    // so the note is pending again; labels set by hand are the user's to keep.
    let actual = body_hash(&body);
    let edited = hash.is_some_and(|stored| stored != actual);
    let status = match status.unwrap_or(Status::Pending) {
        Status::Done | Status::Failed if edited => Status::Pending,
        status => status,
    };

    Some(Note {
        id: id?,
        date: date.to_string(),
        time: time?,
        file: file.to_string(),
        subject,
        category,
        status,
        hash: actual,
        lang,
        body,
        kind: Kind::Note,
        missing: false,
    })
}

fn parse_block(block: &[&str]) -> (Option<String>, Option<String>, String) {
    let mut rest = block;

    let subject = match rest.first().and_then(|l| l.strip_prefix("### ")) {
        Some(raw) => {
            let raw = raw.trim();
            rest = &rest[1..];
            if raw == UNTITLED {
                None
            } else {
                Some(raw.to_string())
            }
        }
        None => None,
    };

    let (category, body) = split_category(rest);
    (subject, category, body)
}

/// The category line and the text under a note's heading, or a page's.
pub(crate) fn split_category(block: &[&str]) -> (Option<String>, String) {
    let mut rest = block;

    // The category is one `> #category` line and then a blank one. Requiring
    // that shape keeps a body that merely starts with a quote from being
    // swallowed. Notes labelled before tags were dropped carry a summary line
    // and then a line of tags, the category first; the other tags are let go.
    let first_tag = |line: &str| {
        line[2..]
            .split_whitespace()
            .map(|t| t.trim_start_matches('#'))
            .find(|t| !t.is_empty())
            .map(str::to_string)
    };
    let quote = |line: &str| line.starts_with("> ") || line.trim_end() == ">";
    let mut category = None;
    if rest.first().is_some_and(|l| l.starts_with("> #"))
        && rest.get(1).is_none_or(|l| l.trim().is_empty())
    {
        category = first_tag(rest[0]);
        rest = &rest[1..];
    } else if rest.len() >= 2 && quote(rest[0]) && rest[1].starts_with("> #") {
        category = first_tag(rest[1]);
        rest = &rest[2..];
    }

    if rest.first().is_some_and(|l| l.trim().is_empty()) {
        rest = &rest[1..];
    }

    (category, rest.join("\n").trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "notes/2026/2026-09-22.md";
    const DATE: &str = "2026-09-22";

    fn pending(id: &str, time: &str, body: &str) -> Note {
        Note {
            id: id.to_string(),
            date: DATE.to_string(),
            time: time.to_string(),
            file: FILE.to_string(),
            subject: None,
            category: None,
            status: Status::Pending,
            hash: body_hash(body),
            lang: None,
            body: body.to_string(),
            kind: Kind::Note,
            missing: false,
        }
    }

    fn enriched() -> Note {
        Note {
            subject: Some("Rollback plan for ArgoCD sync issue".to_string()),
            category: Some("infrastructure".to_string()),
            status: Status::Done,
            ..pending(
                "01J8Z3K6Q9X2",
                "14:32",
                "Talked with the team, the auto-sync broke staging again.\nWe pin the chart version and roll back before Friday release.",
            )
        }
    }

    fn parse_one(content: &str) -> Note {
        let notes = parse_notes(content, DATE, FILE);
        assert_eq!(notes.len(), 1, "expected one note in {content:?}");
        notes.into_iter().next().unwrap()
    }

    #[test]
    fn hash_is_eight_hex_chars_over_the_trimmed_body() {
        let h = body_hash("  hello  ");
        assert_eq!(h.len(), 8);
        assert_eq!(h, body_hash("hello"));
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn round_trips_a_pending_note() {
        let note = pending(
            "01J8Z4P1M7T0",
            "15:10",
            "Buy a new USB-C hub for the homelab",
        );
        assert_eq!(parse_one(&render_note(&note)), note);
    }

    #[test]
    fn round_trips_an_enriched_note() {
        let note = enriched();
        assert_eq!(parse_one(&render_note(&note)), note);
    }

    #[test]
    fn round_trips_a_multiline_body() {
        let note = pending(
            "01J8Z4P1M7T1",
            "09:01",
            "line one\n\nline three\n### not a heading",
        );
        assert_eq!(parse_one(&render_note(&note)), note);
    }

    #[test]
    fn renders_the_layout_given_in_the_spec() {
        let note = enriched();
        let expected = format!(
            concat!(
                "<!-- sn:note id=01J8Z3K6Q9X2 time=14:32 status=done hash={} -->\n",
                "### Rollback plan for ArgoCD sync issue\n",
                "> #infrastructure\n",
                "\n",
                "Talked with the team, the auto-sync broke staging again.\n",
                "We pin the chart version and roll back before Friday release.\n",
                "<!-- sn:end -->\n",
            ),
            note.hash
        );
        assert_eq!(render_note(&note), expected);
    }

    #[test]
    fn renders_a_pending_note_without_a_category_line() {
        let rendered = render_note(&pending("01J8Z4P1M7T0", "15:10", "Buy a USB-C hub"));
        assert!(rendered.contains("### (untitled)\nBuy a USB-C hub\n"));
        assert!(!rendered.lines().any(|l| l.starts_with("> ")));
    }

    #[test]
    fn preserves_user_text_between_blocks() {
        let mut doc = String::from("# 2026-09-22\n\nmy own notes here\n\n");
        doc.push_str(&render_note(&pending("01AAA", "08:00", "first")));
        doc.push_str("\nstray prose the user typed\n\n");
        doc.push_str(&render_note(&pending("01BBB", "09:00", "second")));

        let notes = parse_notes(&doc, DATE, FILE);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].body, "first");
        assert_eq!(notes[1].body, "second");

        // Appending must leave every earlier byte alone.
        let appended = append_note(&doc, &pending("01CCC", "10:00", "third"), DATE);
        assert!(appended.starts_with(&doc));
        assert!(appended.contains("stray prose the user typed"));
    }

    #[test]
    fn a_body_starting_with_a_blockquote_is_not_read_as_enrichment() {
        let note = pending("01DDD", "11:00", "> quoted thought\n> and more");
        let parsed = parse_one(&render_note(&note));
        assert_eq!(parsed.category, None);
        assert_eq!(parsed, note);
    }

    #[test]
    fn a_body_starting_with_a_quoted_hashtag_is_not_read_as_a_category() {
        let note = pending("01DDD", "11:00", "> #1 priority\n> ship it");
        let parsed = parse_one(&render_note(&note));
        assert_eq!(parsed.category, None);
        assert_eq!(parsed, note);
    }

    #[test]
    fn skips_a_block_with_no_end_marker() {
        let doc = format!(
            "# 2026-09-22\n\n<!-- sn:note id=01EEE time=08:00 status=pending hash=deadbeef -->\n### (untitled)\ndangling\n\n{}",
            render_note(&pending("01FFF", "09:00", "intact"))
        );
        let notes = parse_notes(&doc, DATE, FILE);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, "01FFF");
    }

    #[test]
    fn skips_a_block_missing_required_attributes() {
        let doc =
            "<!-- sn:note time=08:00 status=pending -->\n### (untitled)\nno id\n<!-- sn:end -->\n";
        assert!(parse_notes(doc, DATE, FILE).is_empty());
    }

    #[test]
    fn defaults_unknown_status_to_pending_and_recomputes_a_missing_hash() {
        let doc = "<!-- sn:note id=01GGG time=08:00 status=wat -->\n### (untitled)\nbody text\n<!-- sn:end -->\n";
        let note = parse_one(doc);
        assert_eq!(note.status, Status::Pending);
        assert_eq!(note.hash, body_hash("body text"));
    }

    #[test]
    fn a_labelled_note_edited_elsewhere_is_pending_again() {
        let rendered = render_note(&enriched());
        let edited = rendered.replace("Friday release", "Monday release");
        let note = parse_one(&edited);
        assert_eq!(note.status, Status::Pending);
        assert_eq!(note.hash, body_hash(&note.body));

        let failed = rendered.replace("status=done", "status=failed");
        let note = parse_one(&failed.replace("Friday release", "Monday release"));
        assert_eq!(note.status, Status::Pending);
    }

    #[test]
    fn a_manual_note_edited_elsewhere_keeps_its_labels() {
        let rendered = render_note(&enriched()).replace("status=done", "status=manual");
        let note = parse_one(&rendered.replace("Friday release", "Monday release"));
        assert_eq!(note.status, Status::Manual);
        assert_eq!(
            note.hash,
            body_hash(&note.body),
            "the hash describes the new body"
        );
    }

    #[test]
    fn parses_an_empty_file_as_no_notes() {
        assert!(parse_notes("", DATE, FILE).is_empty());
        assert!(parse_notes("# 2026-09-22\n", DATE, FILE).is_empty());
    }

    /// Three notes with the user's own prose wrapped around them.
    fn day_with_prose() -> String {
        let mut doc = String::from("# 2026-09-22\n\nmy own notes here\n\n");
        doc.push_str(&render_note(&pending("01AAA", "08:00", "first")));
        doc.push('\n');
        doc.push_str(&render_note(&pending("01BBB", "09:00", "second")));
        doc.push_str("\nstray prose the user typed\n\n");
        doc.push_str(&render_note(&pending("01CCC", "10:00", "third")));
        doc
    }

    #[test]
    fn removes_only_the_named_block() {
        let out = remove_note(&day_with_prose(), "01BBB").expect("id is present");
        let left = parse_notes(&out, DATE, FILE);
        assert_eq!(
            left.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            ["01AAA", "01CCC"]
        );
        assert!(!out.contains("01BBB"));
    }

    #[test]
    fn removing_a_block_keeps_the_user_prose_around_it() {
        let out = remove_note(&day_with_prose(), "01BBB").expect("id is present");
        assert!(out.starts_with("# 2026-09-22\n\nmy own notes here\n"));
        assert!(out.contains("stray prose the user typed"));
    }

    #[test]
    fn removing_a_block_does_not_leave_a_growing_gap() {
        let out = remove_note(&day_with_prose(), "01BBB").expect("id is present");
        assert!(!out.contains("\n\n\n"), "blank lines piled up:\n{out}");
    }

    #[test]
    fn removes_the_last_block_without_stranding_a_blank_line() {
        let out = remove_note(&day_with_prose(), "01CCC").expect("id is present");
        assert!(out.ends_with("stray prose the user typed\n"), "got:\n{out}");
    }

    #[test]
    fn removes_the_only_block() {
        let doc = append_note("", &pending("01AAA", "08:00", "alone"), DATE);
        let out = remove_note(&doc, "01AAA").expect("id is present");
        assert_eq!(out, "# 2026-09-22\n");
        assert!(parse_notes(&out, DATE, FILE).is_empty());
    }

    #[test]
    fn keeps_crlf_when_the_file_uses_it() {
        let doc = day_with_prose().replace('\n', "\r\n");
        let out = remove_note(&doc, "01BBB").expect("id is present");
        assert!(out.contains("\r\n"));
        assert!(
            !out.replace("\r\n", "").contains('\n'),
            "a bare LF survived in a CRLF file"
        );
        assert_eq!(parse_notes(&out, DATE, FILE).len(), 2);
    }

    #[test]
    fn returns_none_for_an_id_that_is_not_there() {
        assert!(remove_note(&day_with_prose(), "01ZZZ").is_none());
    }

    #[test]
    fn does_not_remove_a_block_whose_end_marker_is_missing() {
        let doc = "<!-- sn:note id=01AAA time=08:00 status=pending hash=dead -->\n### (untitled)\nno end marker\n";
        assert!(remove_note(doc, "01AAA").is_none());
    }

    fn enrich_patch() -> NotePatch {
        NotePatch {
            subject: Some("Rollback plan for ArgoCD sync issue".to_string()),
            category: Some("infrastructure".to_string()),
            status: Status::Done,
            lang: Some("fr".to_string()),
        }
    }

    #[test]
    fn the_label_language_is_written_in_the_marker_and_read_back() {
        let doc = append_note("", &pending("01AAA", "08:00", "body"), DATE);
        assert!(!doc.contains("lang="), "a pending note has no language");

        let out = update_note(&doc, "01AAA", &enrich_patch()).unwrap();
        assert!(out.contains(" lang=fr -->"), "{out}");
        assert_eq!(parse_one(&out).lang.as_deref(), Some("fr"));
        // Notes labelled before the language was recorded have none.
        assert_eq!(parse_one(&doc).lang, None);
    }

    #[test]
    fn enrichment_replaces_the_metadata_and_keeps_the_body() {
        let before = pending("01BBB", "09:00", "the body\nover two lines");
        let doc = append_note("", &before, DATE);

        let out = update_note(&doc, "01BBB", &enrich_patch()).expect("id is present");
        let after = parse_one(&out);

        assert_eq!(after.body, before.body, "the body must be untouched");
        assert_eq!(after.hash, before.hash, "the hash describes the body");
        assert_eq!(after.id, before.id);
        assert_eq!(after.time, before.time);
        assert_eq!(after.status, Status::Done);
        assert_eq!(
            after.subject.as_deref(),
            Some("Rollback plan for ArgoCD sync issue")
        );
        assert_eq!(after.category.as_deref(), Some("infrastructure"));
    }

    #[test]
    fn enrichment_leaves_other_notes_and_user_prose_alone() {
        let doc = day_with_prose();
        let out = update_note(&doc, "01BBB", &enrich_patch()).expect("id is present");

        assert!(out.starts_with("# 2026-09-22\n\nmy own notes here\n"));
        assert!(out.contains("stray prose the user typed"));

        let notes = parse_notes(&out, DATE, FILE);
        assert_eq!(notes.len(), 3);
        assert_eq!(notes[0].subject, None, "01AAA should be untouched");
        assert_eq!(notes[2].subject, None, "01CCC should be untouched");
        assert_eq!(notes[1].status, Status::Done);
    }

    #[test]
    fn enrichment_round_trips_through_the_parser() {
        let doc = append_note("", &pending("01BBB", "09:00", "body"), DATE);
        let out = update_note(&doc, "01BBB", &enrich_patch()).unwrap();

        // Rendering what we parsed back must produce the same bytes.
        assert_eq!(
            render_note(&parse_one(&out)),
            out.trim_start_matches("# 2026-09-22\n\n")
        );
    }

    #[test]
    fn enrichment_keeps_crlf() {
        let doc = append_note("", &pending("01BBB", "09:00", "body"), DATE).replace('\n', "\r\n");
        let out = update_note(&doc, "01BBB", &enrich_patch()).unwrap();
        assert!(!out.replace("\r\n", "").contains('\n'));
        assert_eq!(parse_one(&out).status, Status::Done);
    }

    #[test]
    fn updating_an_unknown_id_changes_nothing() {
        let doc = day_with_prose();
        assert!(update_note(&doc, "01ZZZ", &enrich_patch()).is_none());
    }

    #[test]
    fn a_note_labelled_with_a_summary_and_tags_keeps_its_first_tag_as_category() {
        let doc = "<!-- sn:note id=01AAA time=08:00 status=done hash=dead -->\n### s\n> A summary.\n> #infra #argocd #staging\n\nbody\n<!-- sn:end -->\n";
        let note = parse_one(doc);
        assert_eq!(note.category.as_deref(), Some("infra"));
        assert_eq!(note.body, "body");
        // Written back, the summary and the other tags are gone.
        assert!(render_note(&note).contains("### s\n> #infra\n\nbody\n"));
    }

    #[test]
    fn a_bare_summary_line_with_trailing_spaces_still_parses() {
        let doc = "<!-- sn:note id=01AAA time=08:00 status=manual hash=dead -->\n### s\n>   \n> #infra\n\nbody\n<!-- sn:end -->\n";
        let note = parse_one(doc);
        assert_eq!(note.category.as_deref(), Some("infra"));
        assert_eq!(note.body, "body");
    }

    #[test]
    fn replacing_the_body_rehashes_and_keeps_the_metadata() {
        let before = enriched();
        let doc = format!("# {DATE}\n\nmy prose\n\n{}", render_note(&before));

        let out = replace_body(&doc, &before.id, "  a new body\n").expect("id is present");
        assert!(out.starts_with("# 2026-09-22\n\nmy prose\n"));

        let after = parse_one(&out);
        assert_eq!(after.body, "a new body");
        assert_eq!(after.hash, body_hash("a new body"));
        assert_eq!(after.subject, before.subject);
        assert_eq!(after.category, before.category);
        assert_eq!(after.status, before.status);
    }

    #[test]
    fn replacing_the_body_of_an_unknown_id_changes_nothing() {
        assert!(replace_body(&day_with_prose(), "01ZZZ", "x").is_none());
    }

    #[test]
    fn appends_a_day_header_to_a_new_file() {
        let out = append_note("", &pending("01HHH", "08:00", "first"), DATE);
        assert!(out.starts_with("# 2026-09-22\n\n<!-- sn:note "));
    }

    fn stub(id: &str, title: &str) -> Stub {
        Stub {
            id: id.to_string(),
            time: "10:00".to_string(),
            title: title.to_string(),
            target: format!("../../pages/2026/2026-09-22 {title}.md"),
        }
    }

    /// Notes and prose with a stub between the second and third note.
    fn day_with_stub() -> String {
        let mut doc = day_with_prose();
        let at = doc.find("<!-- sn:note id=01CCC").unwrap();
        doc.insert_str(
            at,
            &format!("{}\n", render_stub(&stub("01PPP", "Weekly sync"))),
        );
        doc
    }

    #[test]
    fn renders_the_stub_given_in_the_spec() {
        assert_eq!(
            render_stub(&stub("01PPP", "Weekly sync, platform team")),
            concat!(
                "<!-- sn:page id=01PPP time=10:00 -->\n",
                "[Weekly sync, platform team](<../../pages/2026/2026-09-22 Weekly sync, platform team.md>)\n",
                "<!-- sn:end -->\n",
            )
        );
    }

    #[test]
    fn stubs_round_trip_and_the_note_parser_skips_them() {
        let doc = day_with_stub();
        assert_eq!(parse_stubs(&doc), vec![stub("01PPP", "Weekly sync")]);
        let notes = parse_notes(&doc, DATE, FILE);
        assert_eq!(
            notes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            ["01AAA", "01BBB", "01CCC"]
        );
        assert!(notes.iter().all(|n| !n.body.contains("sn:page")));
    }

    #[test]
    fn brackets_in_a_title_survive_the_link() {
        let tricky = stub("01PPP", r"Q3 [draft] review \ notes");
        let doc = append_stub("", &tricky, DATE);
        assert!(doc.contains(r"[Q3 \[draft\] review \\ notes]"), "{doc}");
        assert_eq!(parse_stubs(&doc), vec![tricky]);
    }

    #[test]
    fn a_stub_whose_link_was_edited_out_of_shape_is_still_a_stub() {
        let doc = "<!-- sn:page id=01PPP time=10:00 -->\nsomething else\n<!-- sn:end -->\n";
        let stubs = parse_stubs(doc);
        assert_eq!(stubs.len(), 1);
        assert_eq!(
            (stubs[0].title.as_str(), stubs[0].target.as_str()),
            ("", "")
        );
        // A link without angle brackets, as another editor may write it.
        let plain = "<!-- sn:page id=01PPP time=10:00 -->\n[Sync](../../pages/2026/a.md)\n<!-- sn:end -->\n";
        assert_eq!(parse_stubs(plain)[0].target, "../../pages/2026/a.md");
    }

    #[test]
    fn replacing_and_removing_a_stub_leaves_the_notes_alone() {
        let doc = day_with_stub();
        let renamed = stub("01PPP", "Renamed");
        let out = replace_stub(&doc, &renamed).expect("stub is present");
        assert_eq!(parse_stubs(&out), vec![renamed]);
        assert_eq!(parse_notes(&out, DATE, FILE), parse_notes(&doc, DATE, FILE));

        let out = remove_stub(&doc, "01PPP").expect("stub is present");
        assert!(parse_stubs(&out).is_empty());
        assert_eq!(out, day_with_prose(), "the file is back as it was");
        assert!(remove_stub(&out, "01PPP").is_none());
    }

    #[test]
    fn removing_a_note_does_not_touch_a_stub_with_its_id() {
        let doc = append_stub(&day_with_prose(), &stub("01AAA", "Clash"), DATE);
        let out = remove_note(&doc, "01AAA").unwrap();
        assert_eq!(parse_stubs(&out).len(), 1);
    }

    #[test]
    fn a_note_turned_into_a_page_leaves_its_stub_in_its_place() {
        let doc = day_with_prose();
        let out = note_to_stub(&doc, "01BBB", &stub("01PPP", "Second")).unwrap();
        assert!(out.contains("stray prose the user typed"));
        let notes = parse_notes(&out, DATE, FILE);
        assert_eq!(
            notes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            ["01AAA", "01CCC"]
        );
        let first = out.find("01AAA").unwrap();
        let page = out.find("01PPP").unwrap();
        let third = out.find("01CCC").unwrap();
        assert!(
            first < page && page < third,
            "the stub keeps the note's place"
        );
        assert!(note_to_stub(&out, "01BBB", &stub("01PPP", "x")).is_none());
    }

    #[test]
    fn a_stub_appended_to_an_empty_day_starts_the_file() {
        let out = append_stub("", &stub("01PPP", "Sync"), DATE);
        assert!(out.starts_with("# 2026-09-22\n\n<!-- sn:page id=01PPP"));
    }
}
