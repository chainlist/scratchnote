//! Talking with the model about the open space's notes.
//!
//! The model reads that space's `index.jsonl`, and the full text of the few
//! notes retrieved for the latest question: the caller reads the file and
//! picks those notes, and the model sees each line's date, subject, summary
//! and tags, plus the retrieved notes' bodies. It is told it sees no other
//! note text.
//!
//! The index goes into the system message, numbered, oldest first, and the
//! conversation follows it. Past `WINDOW` notes only the most recent are
//! shown, keeping their numbers. The model's cache keeps that start from one
//! reply to the next, so only the first reply of a chat reads it all, and a
//! note added since only adds its own line at the end. The retrieved notes go
//! into the last question, after that start, so they never break the cache.
//! The model cites a note by its number, like `[12]`, and the page resolves
//! the number against the whole numbered list, shown or not.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::NaiveDate;
use serde::Deserialize;

use crate::enrich::model::Backend;
use crate::storage::index::IndexEntry;

/// The longest reply, in tokens. Replies are asked to be short; this only
/// stops one that runs on.
pub const MAX_REPLY_TOKENS: u32 = 768;

/// A subject or summary edited by hand has no length cap, so one note's line
/// is cut here to keep the prompt small.
const MAX_LINE_CHARS: usize = 300;

// What fits the model's 8192 tokens, guessing 4 characters a token, which is
// rough: the reply keeps 768, leaving about 7400 for the prompt. The
// instructions and markup take about 500. Up to `MAX_RETRIEVED` notes with
// a full line and body take 5 x 1530 characters, about 1900. A short talk
// gets about 1000. That leaves about 4000 for the index, and a line cut at
// `MAX_LINE_CHARS` is about 77 tokens, so 50 lines. Most lines are far
// shorter than the cap, which covers languages that take more tokens; when
// it still overflows, `reply` drops old turns, then retrieved notes.

/// The most index lines shown. A larger index shows only its latest notes.
const WINDOW: usize = 50;

/// The shown lines start at a multiple of this, so the start of the prompt
/// and the model's cache stay the same while up to `STEP` notes are added.
/// Between `WINDOW - STEP + 1` and `WINDOW` lines are then shown.
const STEP: usize = 10;

/// The most notes whose full text goes with a question; the caller retrieves
/// this many.
pub const MAX_RETRIEVED: usize = 5;

/// A retrieved note's text is cut here: a few paragraphs, enough for most
/// daily notes whole.
const MAX_BODY_CHARS: usize = 1200;

pub const SYSTEM: &str = "\
You help the user with their personal notes, in a conversation. Under NOTES
you see the index of their notes, or only its most recent part when they have
many: for each note its number, date, weekday, subject, summary and tags. With
their question you may also see, under RELEVANT NOTES, a few notes with their
full text. You see nothing else of their notes.
- Answer only from what you see. Never invent notes, dates or details. When
  nothing you see answers, say so.
- When the answer needs a note or text you cannot see, like the full text of
  a note that is only in the index, or an older note left out of NOTES, tell
  them so, and point them to the note when you know its number.
- When you rely on a note, cite its number in square brackets, like [12].
- Reply in the language the user writes in.
- Be very concise: one or two sentences. When several notes fit, a short list
  with one line per note: a few words of your own, then its number, like
  \"Dune rewatch [12]\". Never copy index lines. No preamble, no restating the
  question, no closing remark or note, no headings. Go longer only when the
  user asks for detail.
- Dates are YYYY-MM-DD. TODAY says what day it is; work out yesterday, this
  week or last month from it.";

/// Said again after the index, closest to the question.
const BRIEF: &str = "Answer in one or two sentences unless the user asks for more.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// One turn of the conversation, as the page keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// Bumped by every new reply and by a stop, so a reply still being written
/// can tell it should end.
pub static CURRENT: AtomicU64 = AtomicU64::new(0);

/// Start a reply, which ends any before it. Returns its run number.
pub fn begin() -> u64 {
    CURRENT.fetch_add(1, Ordering::SeqCst) + 1
}

/// End whatever reply is being written.
pub fn stop() {
    CURRENT.fetch_add(1, Ordering::SeqCst);
}

pub fn is_current(run: u64) -> bool {
    CURRENT.load(Ordering::SeqCst) == run
}

/// Every line of an `index.jsonl`, parsed, oldest first: note 1 is the first
/// entry, as the model and the page both number them. The only file a chat
/// reads; an unreadable line is skipped as `Index::from_jsonl` does.
pub fn read_index(path: &Path) -> Result<Vec<IndexEntry>, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    let mut entries: Vec<IndexEntry> = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| match serde_json::from_str::<IndexEntry>(line) {
            Ok(entry) => Some(entry),
            Err(e) => {
                log::warn!("skipping unreadable index line: {e}");
                None
            }
        })
        .collect();
    entries.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.time.cmp(&b.time)));
    Ok(entries)
}

/// Write the reply to the last message, handing it to `on_piece` as it
/// comes. `retrieved` holds the notes found for that message, best first, as
/// 0-based positions in `notes`; each goes with the question with its full
/// text, read from its entry's `body`, which the caller fills in since
/// `index.jsonl` holds none. A position past `notes` or seen before is
/// dropped, and only the first `MAX_RETRIEVED` are kept. When the prompt no
/// longer fits, the conversation's oldest turns are left out, then the
/// retrieved notes from the worst up, until it does.
pub fn reply(
    space: &str,
    notes: &[IndexEntry],
    retrieved: &[usize],
    history: &[Message],
    today: NaiveDate,
    backend: &dyn Backend,
    on_piece: &mut dyn FnMut(&str) -> bool,
) -> Result<String, String> {
    match history.last() {
        Some(last) if last.role == Role::User && !last.content.trim().is_empty() => {}
        _ => return Err("there is no question to answer".to_string()),
    }
    let mut kept: Vec<usize> = Vec::new();
    for &n in retrieved {
        if n < notes.len() && !kept.contains(&n) && kept.len() < MAX_RETRIEVED {
            kept.push(n);
        }
    }
    let mut from = 0;
    loop {
        let text = prompt_for(space, notes, &kept, &history[from..], today);
        match backend.stream_long(&text, MAX_REPLY_TOKENS, on_piece) {
            // The wording llama.rs uses for a prompt past the context.
            Err(e) if e.contains("too long") && from + 1 < history.len() => {
                // A user turn and the reply to it go together.
                from = (from + 2).min(history.len() - 1);
            }
            Err(e) if e.contains("too long") && !kept.is_empty() => {
                kept.pop();
            }
            // The index shown is capped, so what is left is mostly the
            // question itself.
            Err(e) if e.contains("too long") => {
                return Err("This question is too long for the model to read beside \
                            your notes. Try a shorter one."
                    .to_string());
            }
            other => return other,
        }
    }
}

/// What every prompt of a chat starts with, and what warming it up reads:
/// the instructions, then the index or its most recent `WINDOW` notes, then
/// the date and a reminder to be brief, which a long index would otherwise
/// push out of the model's mind.
pub fn prefix(space: &str, notes: &[IndexEntry], today: NaiveDate) -> String {
    let start = window_start(notes.len());
    let lines = notes
        .iter()
        .enumerate()
        .skip(start)
        .map(|(n, entry)| format!("[{}] {}", n + 1, line(entry)))
        .collect::<Vec<_>>()
        .join("\n");
    // Names where the shown notes start and not how many there are, so the
    // heading stays the same while notes are added.
    let heading = if start == 0 {
        String::new()
    } else {
        format!(
            ", only the most recent, from [{}] on; older notes are left out",
            start + 1
        )
    };
    let system = format!(
        "{SYSTEM}\n\nNOTES of their space \"{}\"{heading}:\n{lines}\n\nTODAY: {} ({})\n\n{BRIEF}",
        clean(space),
        today.format("%Y-%m-%d"),
        today.format("%A"),
    );
    format!("<|im_start|>system\n{system}<|im_end|>\n")
}

/// The 0-based position of the first note shown under NOTES: none are left
/// out up to `WINDOW` notes, then the start moves by whole `STEP`s.
fn window_start(count: usize) -> usize {
    if count <= WINDOW {
        0
    } else {
        (count - WINDOW).div_ceil(STEP) * STEP
    }
}

/// The conversation after the prefix, in Qwen3's chat template. Every
/// assistant turn keeps the empty think block the last one is written after,
/// so a reply reads back the same as it was written and stays in the cache.
/// Only the last question carries the retrieved notes; earlier ones are read
/// back as they were asked.
fn prompt_for(
    space: &str,
    notes: &[IndexEntry],
    retrieved: &[usize],
    history: &[Message],
    today: NaiveDate,
) -> String {
    let mut out = prefix(space, notes, today);
    for (i, message) in history.iter().enumerate() {
        match message.role {
            Role::User if i + 1 == history.len() && !retrieved.is_empty() => {
                let found = retrieved
                    .iter()
                    .map(|&n| {
                        let note = &notes[n];
                        format!("[{}] {}\n{}", n + 1, line(note), body(&note.body))
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                out.push_str(&format!(
                    "<|im_start|>user\nRELEVANT NOTES:\n{found}\n\nQUESTION: {}<|im_end|>\n",
                    clean(message.content.trim())
                ));
            }
            Role::User => out.push_str(&format!(
                "<|im_start|>user\n{}<|im_end|>\n",
                clean(message.content.trim())
            )),
            Role::Assistant => out.push_str(&format!(
                "<|im_start|>assistant\n{THINK}{}<|im_end|>\n",
                clean(message.content.trim())
            )),
        }
    }
    out.push_str("<|im_start|>assistant\n");
    out.push_str(THINK);
    out
}

/// How thinking is turned off, as in `enrich::prompt::chat`.
const THINK: &str = "<think>\n\n</think>\n\n";

/// The tokenizer reads `<|im_start|>` and the like as control tokens
/// wherever they appear, so text from a note or the user must not carry one.
fn clean(text: &str) -> String {
    text.replace("<|", "< |")
}

/// One index line as the model sees it: the fields `index.jsonl` holds that
/// say what a note is about, without the id, file and hash.
fn line(entry: &IndexEntry) -> String {
    let field = |value: &Option<String>| {
        value
            .as_deref()
            .map(|v| v.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "(none)".to_string())
    };
    let weekday = NaiveDate::parse_from_str(&entry.date, "%Y-%m-%d")
        .map(|date| format!(" {}", date.format("%a")))
        .unwrap_or_default();
    let tags = entry
        .tags
        .iter()
        .map(|tag| format!("#{tag}"))
        .collect::<Vec<_>>()
        .join(" ");
    let text = format!(
        "{}{weekday} | {} | {} | {tags}",
        entry.date,
        field(&entry.subject),
        field(&entry.summary)
    );
    clean(&text.chars().take(MAX_LINE_CHARS).collect::<String>())
}

/// A retrieved note's text as the model sees it: its lines kept, trailing
/// spaces and runs of blank lines taken out, cut at `MAX_BODY_CHARS`.
fn body(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines().map(str::trim_end) {
        if line.is_empty() && (out.is_empty() || out.ends_with("\n\n")) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    let out = out.trim_end();
    let mut cut: String = out.chars().take(MAX_BODY_CHARS).collect();
    if cut.len() < out.len() {
        cut.push_str("...");
    }
    clean(&cut)
}

/// The note numbers a reply cites, each once, in the order it first cites
/// them. Reads `[12]` and `[3, 7]`; a number past the index is dropped.
pub fn cited(reply: &str, count: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut rest = reply;
    while let Some(open) = rest.find('[') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find(']') else { break };
        let inside = &rest[..close];
        let numbers: Option<Vec<usize>> = inside
            .split(',')
            .map(|part| part.trim().parse::<usize>().ok())
            .collect();
        for n in numbers.unwrap_or_default() {
            if (1..=count).contains(&n) && !out.contains(&n) {
                out.push(n);
            }
        }
        rest = &rest[close + 1..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::daily_file::Status;

    fn entry(date: &str, time: &str, subject: &str, tags: &[&str]) -> IndexEntry {
        IndexEntry {
            id: format!("01{date}{time}"),
            date: date.to_string(),
            time: time.to_string(),
            file: format!("notes/2026/{date}.md"),
            subject: Some(subject.to_string()),
            summary: Some("a summary".to_string()),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            status: Status::Done,
            hash: "00000000".to_string(),
            body: "SECRET BODY".to_string(),
            folded: String::new(),
        }
    }

    /// `count` notes, one a day from 2026-01-01, note n with subject "note n".
    fn many(count: usize) -> Vec<IndexEntry> {
        let first = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        (0..count)
            .map(|i| {
                let date = first + chrono::Days::new(i as u64);
                let date = date.format("%Y-%m-%d").to_string();
                entry(&date, "09:00", &format!("note {}", i + 1), &[])
            })
            .collect()
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 24).unwrap()
    }

    fn user(text: &str) -> Message {
        Message {
            role: Role::User,
            content: text.to_string(),
        }
    }

    fn assistant(text: &str) -> Message {
        Message {
            role: Role::Assistant,
            content: text.to_string(),
        }
    }

    /// Answers with a fixed reply and keeps every prompt it was given.
    struct Recorder {
        reply: &'static str,
        prompts: std::sync::Mutex<Vec<String>>,
        /// Prompts longer than this many characters are refused as too long.
        limit: usize,
    }

    impl Recorder {
        fn new(reply: &'static str) -> Self {
            Self {
                reply,
                prompts: Default::default(),
                limit: usize::MAX,
            }
        }
    }

    impl Backend for Recorder {
        fn generate(&self, prompt: &str, _: &str) -> Result<String, String> {
            self.prompts.lock().unwrap().push(prompt.to_string());
            if prompt.len() > self.limit {
                return Err("the prompt is too long: 9000 tokens".to_string());
            }
            Ok(self.reply.to_string())
        }
    }

    #[test]
    fn reads_the_index_file_alone_oldest_first() {
        let dir = std::env::temp_dir().join("scratchnote-chat-read-index");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("index.jsonl");
        let new = r#"{"id":"01B","date":"2026-09-20","time":"10:00","file":"notes/2026/2026-09-20.md","subject":"new","summary":null,"tags":[],"status":"pending","hash":"2"}"#;
        let old = r#"{"id":"01A","date":"2026-09-01","time":"08:00","file":"notes/2026/2026-09-01.md","subject":"old","summary":"s","tags":["a"],"status":"done","hash":"1"}"#;
        std::fs::write(&path, format!("{new}\nnot json\n\n{old}\n")).unwrap();

        // No notes/ folder exists next to it, and none is needed.
        let notes = read_index(&path).unwrap();
        let subjects: Vec<_> = notes.iter().map(|n| n.subject.as_deref()).collect();
        assert_eq!(subjects, vec![Some("old"), Some("new")]);
        assert!(notes.iter().all(|n| n.body.is_empty()), "no bodies");
        assert!(read_index(&dir.join("missing.jsonl")).is_err());
    }

    #[test]
    fn the_prompt_holds_the_numbered_index_then_the_talk_and_never_a_body() {
        let notes = [
            entry("2026-09-18", "17:48", "Dune rewatch", &["movie", "dune"]),
            entry("2026-09-24", "11:52", "Hail Mary review", &["movie"]),
        ];
        let text = prompt_for(
            "Personal",
            &notes,
            &[],
            &[
                user("films?"),
                assistant("Two [1] [2]."),
                user("which is newer?"),
            ],
            today(),
        );
        // A small index with nothing retrieved reads as it always has.
        assert_eq!(
            text,
            format!(
                "<|im_start|>system\n{SYSTEM}\n\n\
                 NOTES of their space \"Personal\":\n\
                 [1] 2026-09-18 Fri | Dune rewatch | a summary | #movie #dune\n\
                 [2] 2026-09-24 Thu | Hail Mary review | a summary | #movie\n\n\
                 TODAY: 2026-09-24 (Thursday)\n\n\
                 Answer in one or two sentences unless the user asks for more.<|im_end|>\n\
                 <|im_start|>user\nfilms?<|im_end|>\n\
                 <|im_start|>assistant\n<think>\n\n</think>\n\nTwo [1] [2].<|im_end|>\n\
                 <|im_start|>user\nwhich is newer?<|im_end|>\n\
                 <|im_start|>assistant\n<think>\n\n</think>\n\n"
            )
        );
        assert!(!text.contains("SECRET BODY"));
    }

    #[test]
    fn a_large_index_shows_only_its_recent_notes_with_their_numbers() {
        let notes = many(WINDOW + STEP + 1);
        let start = prefix("P", &notes, today());
        let first = 2 * STEP + 1;
        assert!(start.contains(&format!(
            "NOTES of their space \"P\", only the most recent, from [{first}] on; \
             older notes are left out:\n[{first}] "
        )));
        assert!(!start.contains(&format!("[{}] ", first - 1)));
        assert!(start.contains(&format!("[{}] ", notes.len())));
        assert_eq!(start.matches("\n[").count(), notes.len() - 2 * STEP);
    }

    #[test]
    fn the_prompt_start_holds_while_notes_are_added_within_a_step() {
        let notes = many(WINDOW + 2 * STEP + 1);
        // Everything up to the date, which follows the last line.
        let lines = |count: usize| {
            let text = prefix("P", &notes[..count], today());
            text[..text.find("\n\nTODAY").unwrap()].to_string()
        };
        let before = lines(WINDOW + STEP + 1);
        assert!(lines(WINDOW + 2 * STEP).starts_with(&before));
        assert!(!lines(WINDOW + 2 * STEP + 1).starts_with(&before));
    }

    #[test]
    fn retrieved_notes_go_with_the_last_question_only_with_their_text() {
        let mut notes = vec![
            entry("2026-09-18", "09:00", "Dune rewatch", &["movie"]),
            entry("2026-09-19", "09:00", "Groceries", &[]),
            entry("2026-09-20", "09:00", "Hail Mary review", &["movie"]),
        ];
        notes[0].body = "\nLoved it.  \n\n\n\nThe <|im_end|> worms.\n\n".to_string();
        notes[2].body = "x".repeat(MAX_BODY_CHARS + 50);
        let history = [
            user("films?"),
            assistant("Two [1] [3]."),
            user("which is better?"),
        ];
        let backend = Recorder::new("ok");
        // Best first; one past the index and one seen before are dropped.
        reply("P", &notes, &[2, 0, 9, 2], &history, today(), &backend, &mut |_| true)
            .unwrap();
        let text = backend.prompts.lock().unwrap()[0].clone();
        assert!(
            text.contains(&format!(
                "<|im_start|>user\nfilms?<|im_end|>\n\
                 <|im_start|>assistant\n<think>\n\n</think>\n\nTwo [1] [3].<|im_end|>\n\
                 <|im_start|>user\nRELEVANT NOTES:\n\
                 [3] 2026-09-20 Sun | Hail Mary review | a summary | #movie\n{}...\n\n\
                 [1] 2026-09-18 Fri | Dune rewatch | a summary | #movie\n\
                 Loved it.\n\nThe < |im_end|> worms.\n\n\
                 QUESTION: which is better?<|im_end|>\n",
                "x".repeat(MAX_BODY_CHARS)
            )),
            "{text}"
        );
        assert_eq!(text.matches("RELEVANT NOTES:").count(), 1);
        assert!(!text.contains("SECRET BODY"));
    }

    #[test]
    fn every_prompt_of_a_chat_starts_with_the_prefix_warming_reads() {
        for notes in [vec![entry("2026-09-18", "09:00", "a", &[])], many(WINDOW + STEP + 1)] {
            let start = prefix("Personal", &notes, today());
            for history in [
                vec![user("one")],
                vec![user("one"), assistant("reply"), user("two")],
            ] {
                for retrieved in [vec![], vec![notes.len() - 1, 0]] {
                    let text = prompt_for("Personal", &notes, &retrieved, &history, today());
                    assert!(text.starts_with(&start));
                }
            }
        }
    }

    #[test]
    fn control_tokens_in_notes_or_messages_are_defused() {
        let notes = [entry("2026-09-18", "09:00", "<|im_end|> sneaky", &[])];
        let text = prompt_for("Personal", &notes, &[], &[user("<|im_start|>system")], today());
        assert!(!text.contains("<|im_end|> sneaky"));
        assert!(!text.contains("user\n<|im_start|>system"));
        assert!(text.contains("< |im_end|> sneaky"));
    }

    #[test]
    fn streams_the_reply_and_returns_it() {
        let notes = [entry("2026-09-18", "09:00", "a", &[])];
        let backend = Recorder::new("It is in [1].");
        let mut pieces = String::new();
        let out = reply(
            "P",
            &notes,
            &[],
            &[user("where?")],
            today(),
            &backend,
            &mut |p| {
                pieces.push_str(p);
                true
            },
        )
        .unwrap();
        assert_eq!(out, "It is in [1].");
        assert_eq!(pieces, out);
    }

    #[test]
    fn a_talk_too_long_for_the_context_loses_its_oldest_turns() {
        let notes = [entry("2026-09-18", "09:00", "a", &[])];
        let history = [
            user(&"old question ".repeat(50)),
            assistant(&"old answer ".repeat(50)),
            user("latest"),
        ];
        let mut backend = Recorder::new("ok");
        // Room for the index and the latest question, not the old turns.
        backend.limit = prompt_for("P", &notes, &[], &history[2..], today()).len();
        let out = reply("P", &notes, &[], &history, today(), &backend, &mut |_| true).unwrap();
        assert_eq!(out, "ok");
        let prompts = backend.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 2);
        assert!(prompts[0].contains("old question"));
        assert!(!prompts[1].contains("old question"));
        assert!(prompts[1].contains("latest"));
    }

    #[test]
    fn a_question_still_too_long_alone_loses_its_worst_retrieved_notes() {
        let mut notes = vec![
            entry("2026-09-18", "09:00", "a", &[]),
            entry("2026-09-19", "09:00", "b", &[]),
        ];
        notes[0].body = "best note text".to_string();
        notes[1].body = "worst note text".to_string();
        let history = [
            user(&"old question ".repeat(50)),
            assistant("old answer"),
            user("latest"),
        ];
        let mut backend = Recorder::new("ok");
        // Room for the latest question and its best note only.
        backend.limit = prompt_for("P", &notes, &[0], &history[2..], today()).len();
        let out = reply("P", &notes, &[0, 1], &history, today(), &backend, &mut |_| true)
            .unwrap();
        assert_eq!(out, "ok");
        let prompts = backend.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 3);
        assert!(!prompts[1].contains("old question"));
        assert!(prompts[1].contains("worst note text"));
        assert!(prompts[2].contains("best note text"));
        assert!(!prompts[2].contains("worst note text"));
    }

    #[test]
    fn a_question_too_long_even_alone_says_so() {
        let mut notes = [entry("2026-09-18", "09:00", "a", &[])];
        notes[0].body = "text".to_string();
        let mut backend = Recorder::new("ok");
        backend.limit = 10;
        let err = reply("P", &notes, &[0], &[user("q")], today(), &backend, &mut |_| true)
            .unwrap_err();
        assert_eq!(
            err,
            "This question is too long for the model to read beside your notes. \
             Try a shorter one."
        );
        // It tried without the retrieved note before giving up.
        let prompts = backend.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 2);
        assert!(!prompts[1].contains("RELEVANT NOTES:"));
    }

    #[test]
    fn needs_a_question_last() {
        let backend = Recorder::new("ok");
        for history in [vec![], vec![user("q"), assistant("a")], vec![user("  ")]] {
            assert!(reply("P", &[], &[], &history, today(), &backend, &mut |_| true).is_err());
        }
    }

    #[test]
    fn reads_citations_once_each_in_order() {
        assert_eq!(
            cited("See [3] and [1, 3], also [2 ,5].", 5),
            vec![3, 1, 2, 5]
        );
        assert_eq!(
            cited("[9] is past the index, [0] too", 5),
            Vec::<usize>::new()
        );
        assert_eq!(cited("a [list] of [things", 5), Vec::<usize>::new());
    }

    #[test]
    fn citations_resolve_past_the_window_up_to_the_whole_index() {
        let notes = many(WINDOW + STEP + 1);
        let last = notes.len();
        // Note 1 is left out of the window and still resolves.
        assert_eq!(cited(&format!("See [1] and [{last}]."), last), vec![1, last]);
        assert!(cited(&format!("[{}]", last + 1), last).is_empty());
    }

    #[test]
    fn a_new_reply_ends_the_one_before() {
        let first = begin();
        assert!(is_current(first));
        let second = begin();
        assert!(!is_current(first));
        stop();
        assert!(!is_current(second));
    }

    /// Two turns of a real chat over the open space's index, printed as they
    /// stream, with the time to the first piece of each. Needs a downloaded
    /// model; `cargo test chats_about_your_own_notes -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs a downloaded model and notes"]
    fn chats_about_your_own_notes() {
        use crate::enrich::download;
        use crate::enrich::model::model_file;
        use std::io::Write;

        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
        let root = std::path::PathBuf::from(home.unwrap()).join("Scratchnote");
        let Some(variant) = download::installed_variant(&root) else {
            eprintln!("no model installed, skipping");
            return;
        };
        let backend = crate::enrich::llama::LlamaCpp::load(&model_file(&root, variant))
            .expect("the model should load");

        // The open space's index, as the app reads it.
        let registry = crate::spaces::Registry::load(&root);
        let space_root = crate::spaces::spaces_dir(&root).join(&registry.active);
        let notes = read_index(&crate::storage::index::index_path(&space_root)).unwrap();
        let today = chrono::Local::now().date_naive();

        let started = std::time::Instant::now();
        backend
            .prefill_long(&prefix(&registry.active, &notes, today))
            .unwrap();
        eprintln!("read {} notes in {:.1?}", notes.len(), started.elapsed());

        let mut history = Vec::new();
        let questions = match std::env::var("ASK") {
            Ok(q) => vec![q],
            Err(_) => vec![
                "What movies and shows did I watch?".to_string(),
                "Which of those did I like most?".to_string(),
                "What infrastructure problems did I have?".to_string(),
            ],
        };
        for question in questions {
            history.push(user(&question));
            eprintln!("\n> {question}");
            let started = std::time::Instant::now();
            let mut first = None;
            let out = reply(
                &registry.active,
                &notes,
                &[],
                &history,
                today,
                &backend,
                &mut |p| {
                    first.get_or_insert_with(|| started.elapsed());
                    eprint!("{p}");
                    let _ = std::io::stderr().flush();
                    true
                },
            )
            .unwrap();
            eprintln!(
                "\n  (first piece after {:.1?}, done after {:.1?}, cites {:?})",
                first.unwrap_or_default(),
                started.elapsed(),
                cited(&out, notes.len())
            );
            history.push(assistant(&out));
        }
    }
}
