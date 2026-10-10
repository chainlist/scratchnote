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
mod tests;
