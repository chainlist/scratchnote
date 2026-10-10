//! Parser and renderer for the daily markdown file, the source of truth.
//!
//! Note blocks are delimited by `<!-- sn:note ... -->` / `<!-- sn:end -->`.
//! Parsing relies only on those markers, so anything the user writes between
//! blocks is never interpreted and never touched. A page's stub (SPEC 4.7)
//! opens with `<!-- sn:page ... -->` instead, so the note parser skips it.
//!
//! Blocks written while a model labelled the notes carry a `status` in their
//! marker and the labels above the text: a `### subject` heading and a
//! `> #category` line. They are still read, without the labels, and
//! rebuilding the index writes them as blocks are written now.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::hex;
use super::markdown::{newline_of, Marker, AHEAD_OFF};

const NOTE_OPEN: &str = "<!-- sn:note ";
/// Opens a page's stub here, and the marker line of a page file.
pub const PAGE_OPEN: &str = "<!-- sn:page ";
const NOTE_END: &str = "<!-- sn:end -->";

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub date: String,
    pub time: String,
    pub file: String,
    /// A page's title. Notes have none.
    pub subject: Option<String>,
    pub hash: String,
    /// The later day the note looks forward to (SPEC 5.3), such as
    /// "2026-10-06", read off its text each time it is parsed. `None` when
    /// it names none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
    /// The user cleared the day ahead, so none is read off the text.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ahead_off: bool,
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
    hex(&Sha256::digest(body.trim().as_bytes())[..4])
}

pub fn render_note(note: &Note) -> String {
    let off = if note.ahead_off {
        format!(" {AHEAD_OFF}")
    } else {
        String::new()
    };
    let mut out = format!("{NOTE_OPEN}id={} time={}{off} -->\n", note.id, note.time);
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
    let newline = newline_of(content);
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

/// Clear one note's day ahead for good, keeping its body and every byte of
/// the file outside its block. Returns `None` when the id is not in this file.
pub fn clear_day_ahead(content: &str, id: &str) -> Option<String> {
    rewrite_block(content, id, |note| note.ahead_off = true)
}

/// The file with every labelled block written as blocks are now, without its
/// labels. `None` when it holds none.
pub fn drop_labels(content: &str) -> Option<String> {
    let labelled: Vec<String> = content
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with(NOTE_OPEN) && line.ends_with("-->") && is_labelled(line))
        .filter_map(|line| header_id(line, NOTE_OPEN).map(str::to_string))
        .collect();
    if labelled.is_empty() {
        return None;
    }
    let mut out = content.to_string();
    for id in labelled {
        if let Some(rewritten) = rewrite_block(&out, &id, |_| {}) {
            out = rewritten;
        }
    }
    Some(out)
}

/// A marker written while a model labelled the notes: those all carried a
/// `status`.
pub(crate) fn is_labelled(marker: &str) -> bool {
    marker
        .split_whitespace()
        .any(|pair| pair.starts_with("status="))
}

/// Swap one note's body, and its hash with it, leaving the rest and every
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
    let newline = newline_of(content);
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
    let marker = Marker::parse(header, PAGE_OPEN)?;
    // A link edited out of shape still leaves the stub, which the app then
    // rewrites from the page.
    let (title, target) = block
        .iter()
        .find(|line| !line.trim().is_empty())
        .and_then(|line| parse_link(line.trim()))
        .unwrap_or_default();
    Some(Stub {
        id: marker.id?.to_string(),
        time: marker.time?.to_string(),
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
    let marker = Marker::parse(header, NOTE_OPEN)?;
    let ahead_off = marker.ahead_off;

    let body = if is_labelled(header) {
        unlabelled_body(block)
    } else {
        block.join("\n").trim_end().to_string()
    };

    Some(Note {
        id: marker.id?.to_string(),
        date: date.to_string(),
        time: marker.time?.to_string(),
        file: file.to_string(),
        subject: None,
        hash: body_hash(&body),
        on: (!ahead_off)
            .then(|| crate::ahead::day_ahead(&body, date))
            .flatten(),
        ahead_off,
        body,
        kind: Kind::Note,
        missing: false,
    })
}

/// The text of a labelled block, under its `### subject` heading and its
/// category line.
fn unlabelled_body(block: &[&str]) -> String {
    let rest = match block.first() {
        Some(line) if line.starts_with("### ") => &block[1..],
        _ => block,
    };
    strip_category(rest)
}

/// The text under a labelled note's heading, or a labelled page's, without
/// its category line.
pub(crate) fn strip_category(block: &[&str]) -> String {
    let mut rest = block;

    // The category is one `> #category` line and then a blank one. Requiring
    // that shape keeps a body that merely starts with a quote from being
    // swallowed. Notes labelled before tags were dropped carry a summary line
    // and then a line of tags.
    let quote = |line: &str| line.starts_with("> ") || line.trim_end() == ">";
    if rest.first().is_some_and(|l| l.starts_with("> #"))
        && rest.get(1).is_none_or(|l| l.trim().is_empty())
    {
        rest = &rest[1..];
    } else if rest.len() >= 2 && quote(rest[0]) && rest[1].starts_with("> #") {
        rest = &rest[2..];
    }

    if rest.first().is_some_and(|l| l.trim().is_empty()) {
        rest = &rest[1..];
    }

    rest.join("\n").trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "notes/2026/2026-09-22.md";
    const DATE: &str = "2026-09-22";

    fn note(id: &str, time: &str, body: &str) -> Note {
        Note {
            id: id.to_string(),
            date: DATE.to_string(),
            time: time.to_string(),
            file: FILE.to_string(),
            subject: None,
            hash: body_hash(body),
            on: None,
            ahead_off: false,
            body: body.to_string(),
            kind: Kind::Note,
            missing: false,
        }
    }

    /// A block as it was written while a model labelled the notes.
    const LABELLED: &str = concat!(
        "<!-- sn:note id=01J8Z3K6Q9X2 time=14:32 status=done hash=0badc0de lang=fr on=2026-09-25 -->\n",
        "### Rollback plan for ArgoCD sync issue\n",
        "> #infrastructure\n",
        "\n",
        "Talked with the team, the auto-sync broke staging again.\n",
        "We pin the chart version and roll back.\n",
        "<!-- sn:end -->\n",
    );

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
    fn round_trips_a_note() {
        let note = note(
            "01J8Z4P1M7T0",
            "15:10",
            "Buy a new USB-C hub for the homelab",
        );
        assert_eq!(parse_one(&render_note(&note)), note);
    }

    #[test]
    fn round_trips_a_multiline_body() {
        let note = note(
            "01J8Z4P1M7T1",
            "09:01",
            "line one\n\nline three\n### not a heading",
        );
        assert_eq!(parse_one(&render_note(&note)), note);
    }

    #[test]
    fn renders_the_layout_given_in_the_spec() {
        let note = note("01J8Z3K6Q9X2", "14:32", "Buy a USB-C hub");
        assert_eq!(
            render_note(&note),
            "<!-- sn:note id=01J8Z3K6Q9X2 time=14:32 -->\nBuy a USB-C hub\n<!-- sn:end -->\n"
        );
    }

    #[test]
    fn preserves_user_text_between_blocks() {
        let mut doc = String::from("# 2026-09-22\n\nmy own notes here\n\n");
        doc.push_str(&render_note(&note("01AAA", "08:00", "first")));
        doc.push_str("\nstray prose the user typed\n\n");
        doc.push_str(&render_note(&note("01BBB", "09:00", "second")));

        let notes = parse_notes(&doc, DATE, FILE);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].body, "first");
        assert_eq!(notes[1].body, "second");

        // Appending must leave every earlier byte alone.
        let appended = append_note(&doc, &note("01CCC", "10:00", "third"), DATE);
        assert!(appended.starts_with(&doc));
        assert!(appended.contains("stray prose the user typed"));
    }

    #[test]
    fn a_body_starting_with_a_heading_or_a_quoted_hashtag_is_kept_whole() {
        for body in ["### My heading\ntext", "> #1 priority\n> ship it"] {
            let note = note("01DDD", "11:00", body);
            assert_eq!(parse_one(&render_note(&note)), note);
        }
    }

    #[test]
    fn skips_a_block_with_no_end_marker() {
        let doc = format!(
            "# 2026-09-22\n\n<!-- sn:note id=01EEE time=08:00 -->\ndangling\n\n{}",
            render_note(&note("01FFF", "09:00", "intact"))
        );
        let notes = parse_notes(&doc, DATE, FILE);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, "01FFF");
    }

    #[test]
    fn skips_a_block_missing_required_attributes() {
        let doc = "<!-- sn:note time=08:00 -->\nno id\n<!-- sn:end -->\n";
        assert!(parse_notes(doc, DATE, FILE).is_empty());
    }

    #[test]
    fn a_labelled_block_is_read_without_its_labels() {
        let note = parse_one(LABELLED);
        assert_eq!(note.subject, None);
        assert_eq!(
            note.body,
            "Talked with the team, the auto-sync broke staging again.\nWe pin the chart version and roll back."
        );
        assert_eq!(note.hash, body_hash(&note.body));
        // The day the model wrote is not taken: this text names none.
        assert_eq!(note.on, None);
    }

    #[test]
    fn a_note_labelled_with_a_summary_and_tags_loses_them_all() {
        let doc = "<!-- sn:note id=01AAA time=08:00 status=done hash=dead -->\n### s\n> A summary.\n> #infra #argocd #staging\n\nbody\n<!-- sn:end -->\n";
        assert_eq!(parse_one(doc).body, "body");
        let bare = "<!-- sn:note id=01AAA time=08:00 status=manual hash=dead -->\n### s\n>   \n> #infra\n\nbody\n<!-- sn:end -->\n";
        assert_eq!(parse_one(bare).body, "body");
    }

    #[test]
    fn dropping_labels_rewrites_only_the_labelled_blocks() {
        let mut doc = String::from("# 2026-09-22\n\nmy prose\n\n");
        doc.push_str(&render_note(&note("01AAA", "08:00", "### kept heading")));
        doc.push('\n');
        doc.push_str(LABELLED);
        doc.push_str("\ntrailing prose\n");

        let out = drop_labels(&doc).expect("one block is labelled");
        assert!(out.starts_with(
            "# 2026-09-22\n\nmy prose\n\n<!-- sn:note id=01AAA time=08:00 -->\n### kept heading\n"
        ));
        assert!(out.contains("<!-- sn:note id=01J8Z3K6Q9X2 time=14:32 -->\nTalked with the team"));
        assert!(out.ends_with("\ntrailing prose\n"));
        assert!(!out.contains("status=") && !out.contains("> #infrastructure"));
        assert_eq!(parse_notes(&out, DATE, FILE), parse_notes(&doc, DATE, FILE));
        assert_eq!(drop_labels(&out), None, "nothing left to drop");

        let crlf = drop_labels(&doc.replace('\n', "\r\n")).unwrap();
        assert!(!crlf.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn the_day_ahead_is_read_off_the_text_unless_cleared() {
        let doc = append_note("", &note("01AAA", "08:00", "Dentist on Friday"), DATE);
        assert_eq!(parse_one(&doc).on.as_deref(), Some("2026-09-25"));

        let out = clear_day_ahead(&doc, "01AAA").unwrap();
        assert!(out.contains("time=08:00 ahead=off -->"), "{out}");
        let cleared = parse_one(&out);
        assert_eq!(cleared.on, None);
        assert!(cleared.ahead_off);
        // Still off once the text changes.
        let edited = replace_body(&out, "01AAA", "Dentist on Monday").unwrap();
        assert_eq!(parse_one(&edited).on, None);
        assert!(clear_day_ahead(&doc, "01ZZZ").is_none());
    }

    #[test]
    fn parses_an_empty_file_as_no_notes() {
        assert!(parse_notes("", DATE, FILE).is_empty());
        assert!(parse_notes("# 2026-09-22\n", DATE, FILE).is_empty());
    }

    /// Three notes with the user's own prose wrapped around them.
    fn day_with_prose() -> String {
        let mut doc = String::from("# 2026-09-22\n\nmy own notes here\n\n");
        doc.push_str(&render_note(&note("01AAA", "08:00", "first")));
        doc.push('\n');
        doc.push_str(&render_note(&note("01BBB", "09:00", "second")));
        doc.push_str("\nstray prose the user typed\n\n");
        doc.push_str(&render_note(&note("01CCC", "10:00", "third")));
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
        let doc = append_note("", &note("01AAA", "08:00", "alone"), DATE);
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

    #[test]
    fn replacing_the_body_rehashes_and_keeps_the_rest() {
        let before = Note {
            ahead_off: true,
            ..note("01BBB", "09:00", "the body")
        };
        let doc = format!("# {DATE}\n\nmy prose\n\n{}", render_note(&before));

        let out = replace_body(&doc, &before.id, "  a new body\n").expect("id is present");
        assert!(out.starts_with("# 2026-09-22\n\nmy prose\n"));

        let after = parse_one(&out);
        assert_eq!(after.body, "a new body");
        assert_eq!(after.hash, body_hash("a new body"));
        assert_eq!(after.time, before.time);
        assert!(after.ahead_off);
    }

    #[test]
    fn replacing_the_body_of_an_unknown_id_changes_nothing() {
        assert!(replace_body(&day_with_prose(), "01ZZZ", "x").is_none());
    }

    #[test]
    fn appends_a_day_header_to_a_new_file() {
        let out = append_note("", &note("01HHH", "08:00", "first"), DATE);
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
