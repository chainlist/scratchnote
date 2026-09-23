//! Parser and renderer for the daily markdown file, the source of truth.
//!
//! Note blocks are delimited by `<!-- sn:note ... -->` / `<!-- sn:end -->`.
//! Parsing relies only on those markers, so anything the user writes between
//! blocks is never interpreted and never touched.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const NOTE_OPEN: &str = "<!-- sn:note ";
const NOTE_END: &str = "<!-- sn:end -->";
const UNTITLED: &str = "(untitled)";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pending,
    Done,
    Failed,
    Manual,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::Done => "done",
            Status::Failed => "failed",
            Status::Manual => "manual",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
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
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub status: Status,
    pub hash: String,
    pub body: String,
}

/// First 8 hex chars of SHA-256 over the trimmed body.
pub fn body_hash(body: &str) -> String {
    let digest = Sha256::digest(body.trim().as_bytes());
    digest[..4].iter().map(|b| format!("{b:02x}")).collect()
}

pub fn render_note(note: &Note) -> String {
    let mut out = format!(
        "{}id={} time={} status={} hash={} -->\n",
        NOTE_OPEN,
        note.id,
        note.time,
        note.status.as_str(),
        note.hash
    );
    out.push_str("### ");
    out.push_str(note.subject.as_deref().unwrap_or(UNTITLED));
    out.push('\n');
    // Tags set by hand on a note the model never reached have no summary to
    // go with them, so the summary line is left bare rather than dropped.
    if note.summary.is_some() || !note.tags.is_empty() {
        match note.summary.as_deref().filter(|s| !s.is_empty()) {
            Some(summary) => {
                out.push_str("> ");
                out.push_str(summary);
            }
            None => out.push('>'),
        }
        out.push('\n');
        out.push_str("> ");
        for (i, tag) in note.tags.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push('#');
            out.push_str(tag);
        }
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
    out.push_str(&render_note(note));
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
    let lines: Vec<&str> = content.lines().collect();
    let (start, end) = find_block(&lines, id)?;

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
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub status: Status,
}

/// Rewrite one note's block in place, keeping its body and every byte of the
/// file outside that block. Returns `None` when the id is not in this file.
pub fn update_note(content: &str, id: &str, patch: &NotePatch) -> Option<String> {
    rewrite_block(content, id, |note| {
        note.subject = patch.subject.clone();
        note.summary = patch.summary.clone();
        note.tags = patch.tags.clone();
        note.status = patch.status;
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
    let (start, end) = find_block(&lines, id)?;

    let mut note = build_note(lines[start].trim(), &lines[start + 1..end], "", "")?;
    edit(&mut note);

    let replacement = render_note(&note);
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
    Some(joined)
}

/// Line span of the block carrying `id`, opener and end marker included.
fn find_block(lines: &[&str], id: &str) -> Option<(usize, usize)> {
    let mut start = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with(NOTE_OPEN) && trimmed.ends_with("-->") {
            // A non-matching opener clears the candidate, which also drops a
            // previous block that was never closed.
            start = header_id(trimmed).filter(|found| *found == id).map(|_| i);
        } else if trimmed == NOTE_END {
            if let Some(start) = start {
                return Some((start, i));
            }
        }
    }
    None
}

fn header_id(header: &str) -> Option<&str> {
    let attrs = header.strip_prefix(NOTE_OPEN)?.strip_suffix("-->")?.trim();
    attrs.split_whitespace().find_map(|p| p.strip_prefix("id="))
}

fn build_note(header: &str, block: &[&str], date: &str, file: &str) -> Option<Note> {
    let attrs = header.strip_prefix(NOTE_OPEN)?.strip_suffix("-->")?.trim();

    let (mut id, mut time, mut status, mut hash) = (None, None, None, None);
    for pair in attrs.split_whitespace() {
        match pair.split_once('=') {
            Some(("id", v)) => id = Some(v.to_string()),
            Some(("time", v)) => time = Some(v.to_string()),
            Some(("status", v)) => status = Status::parse(v),
            Some(("hash", v)) => hash = Some(v.to_string()),
            _ => {}
        }
    }

    let (subject, summary, tags, body) = parse_block(block);

    Some(Note {
        id: id?,
        date: date.to_string(),
        time: time?,
        file: file.to_string(),
        subject,
        summary,
        tags,
        status: status.unwrap_or(Status::Pending),
        hash: hash.unwrap_or_else(|| body_hash(&body)),
        body,
    })
}

fn parse_block(block: &[&str]) -> (Option<String>, Option<String>, Vec<String>, String) {
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

    // The enrichment block is exactly two blockquote lines, the second holding
    // the tags. Requiring that shape keeps a body that merely starts with a
    // quote from being swallowed.
    let mut summary = None;
    let mut tags = Vec::new();
    let quote = |line: &str| line.starts_with("> ") || line.trim_end() == ">";
    if rest.len() >= 2 && quote(rest[0]) && rest[1].starts_with("> #") {
        summary = Some(rest[0][1..].trim().to_string()).filter(|s| !s.is_empty());
        tags = rest[1][2..]
            .split_whitespace()
            .map(|t| t.trim_start_matches('#'))
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect();
        rest = &rest[2..];
    }

    if rest.first().is_some_and(|l| l.trim().is_empty()) {
        rest = &rest[1..];
    }

    (
        subject,
        summary,
        tags,
        rest.join("\n").trim_end().to_string(),
    )
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
            summary: None,
            tags: Vec::new(),
            status: Status::Pending,
            hash: body_hash(body),
            body: body.to_string(),
        }
    }

    fn enriched() -> Note {
        Note {
            subject: Some("Rollback plan for ArgoCD sync issue".to_string()),
            summary: Some(
                "Decided to pin the chart version and roll back staging before Friday.".to_string(),
            ),
            tags: vec![
                "argocd".to_string(),
                "deployment".to_string(),
                "staging".to_string(),
            ],
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
                "> Decided to pin the chart version and roll back staging before Friday.\n",
                "> #argocd #deployment #staging\n",
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
    fn renders_a_pending_note_without_a_summary_block() {
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
        assert_eq!(parsed.summary, None);
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
            summary: Some("Pin the chart version and roll back staging.".to_string()),
            tags: vec!["argocd".to_string(), "staging".to_string()],
            status: Status::Done,
        }
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
        assert_eq!(after.tags, vec!["argocd", "staging"]);
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
    fn hand_set_tags_survive_on_a_note_with_no_summary() {
        let note = Note {
            subject: Some("Hub shopping".to_string()),
            tags: vec!["homelab".to_string()],
            status: Status::Manual,
            ..pending("01J8Z4P1M7T0", "15:10", "Buy a new USB-C hub")
        };
        let rendered = render_note(&note);
        assert!(rendered.contains("### Hub shopping\n>\n> #homelab\n\nBuy"));
        assert_eq!(parse_one(&rendered), note);
    }

    #[test]
    fn a_bare_summary_line_with_trailing_spaces_still_parses() {
        let doc = "<!-- sn:note id=01AAA time=08:00 status=manual hash=dead -->\n### s\n>   \n> #infra\n\nbody\n<!-- sn:end -->\n";
        let note = parse_one(doc);
        assert_eq!(note.summary, None);
        assert_eq!(note.tags, vec!["infra"]);
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
        assert_eq!(after.summary, before.summary);
        assert_eq!(after.tags, before.tags);
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
}
