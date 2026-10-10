//! Mentions, SPEC 3.10: `@name` in a note or a page names a project, a
//! person or anything else, and the notes naming it are listed together.
//!
//! The names are read here as the notes are indexed, into `search.db`, so
//! listing them reads no text. They are read as the editor parses them: an
//! `@` right after a letter or a digit is no mention, so an email stays one,
//! and nothing in code, after a backslash or in a link's target is a mention.

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;

use crate::storage::daily_file::Note;
use crate::storage::index::{Index, IndexEntry};
use crate::storage::search_db::SearchDb;
use crate::Result;

/// A name as typed in a note, and the key it is found by: case does not
/// count, so `@Marie` and `@marie` are one name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    pub key: String,
    pub name: String,
}

/// The key a name is found by.
pub fn key(name: &str) -> String {
    name.trim_start_matches('@').to_lowercase()
}

/// A letter or digit of a name, in any script, or `_` and `-` inside one.
fn in_name(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// The names `body` mentions, each once, in the order first mentioned.
pub fn mentions(body: &str) -> Vec<Mention> {
    let mut found: Vec<Mention> = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for line in body.lines() {
        let indent = line.len() - line.trim_start_matches(' ').len();
        let rest = &line[indent..];
        let mark = rest.chars().next().filter(|c| *c == '`' || *c == '~');
        let run = mark.map_or(0, |c| rest.chars().take_while(|x| *x == c).count());
        match fence {
            // A fence closes on a run of its own mark as long as it, or longer.
            Some((c, len)) => {
                if indent < 4 && mark == Some(c) && run >= len && rest[run..].trim().is_empty() {
                    fence = None;
                }
                continue;
            }
            None if indent < 4 && run >= 3 => {
                fence = mark.map(|c| (c, run));
                continue;
            }
            None => {}
        }
        for name in in_line(line) {
            let key = key(&name);
            if !found.iter().any(|m| m.key == key) {
                found.push(Mention { key, name });
            }
        }
    }
    found
}

/// The names on one line outside code, escapes and link targets.
fn in_line(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut names = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' if chars.get(i + 1).is_some_and(|c| c.is_ascii_punctuation()) => i += 2,
            '`' => {
                let run = chars[i..].iter().take_while(|c| **c == '`').count();
                i += run;
                // A code span runs to the next run of as many backticks.
                let mut j = i;
                while j < chars.len() {
                    let close = chars[j..].iter().take_while(|c| **c == '`').count();
                    if close == run {
                        i = j + close;
                        break;
                    }
                    j += close.max(1);
                }
            }
            // `<https://…>` and `<me@example.com>`, up to the bracket.
            '<' => {
                let end = chars[i + 1..]
                    .iter()
                    .position(|c| *c == '>' || c.is_whitespace() || *c == '<');
                match end {
                    Some(n) if chars[i + 1 + n] == '>' && n > 0 => i += n + 2,
                    _ => i += 1,
                }
            }
            // A bare link, `https://…` or `www.…`, up to a space.
            _ if bare_link(&chars, i) => {
                while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '<' {
                    i += 1;
                }
            }
            // A link's target, `](…)`, up to its closing parenthesis.
            ']' if chars.get(i + 1) == Some(&'(') => {
                let mut depth = 0;
                let mut j = i + 1;
                while j < chars.len() {
                    match chars[j] {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                i = j + 1;
            }
            // A name starts on a letter or digit.
            '@' if (i == 0 || !in_name(chars[i - 1]))
                && chars.get(i + 1).is_some_and(|c| c.is_alphanumeric()) =>
            {
                let mut end = i + 1;
                while end < chars.len() && in_name(chars[end]) {
                    end += 1;
                }
                // A name ends on a letter or digit: `@bob-` is `@bob` and a dash.
                while end > i + 1 && matches!(chars[end - 1], '_' | '-') {
                    end -= 1;
                }
                if end > i + 1 {
                    names.push(chars[i + 1..end].iter().collect());
                }
                i = end.max(i + 1);
            }
            _ => i += 1,
        }
    }
    names
}

/// Whether a bare link starts at `i`, as GitHub's markdown finds them: at
/// the start of a word, or after `*`, `_`, `~` or `(`.
fn bare_link(chars: &[char], i: usize) -> bool {
    if i > 0 && !matches!(chars[i - 1], '*' | '_' | '~' | '(') && !chars[i - 1].is_whitespace() {
        return false;
    }
    let head: String = chars[i..].iter().take(8).collect::<String>().to_lowercase();
    head.starts_with("http://") || head.starts_with("https://") || head.starts_with("www.")
}

/// A name mentioned in the open space, with how many notes and pages
/// mention it and the day of the last.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MentionSummary {
    /// As the newest note mentioning it types it.
    pub name: String,
    pub key: String,
    pub notes: usize,
    pub last: String,
    /// The days of those notes, each once, oldest first: how often it comes
    /// up, week by week, as the list draws it.
    pub days: Vec<String>,
    /// Pinned to the left edge (SPEC 3.13), as a project in hand is.
    pub pinned: bool,
}

/// Every name the notes of `index` mention, most mentioned first, then the
/// one mentioned last.
pub fn list(index: &Index, db: &SearchDb) -> Result<Vec<MentionSummary>> {
    let entries: HashMap<&str, &IndexEntry> = index.entries().map(|e| (e.id.as_str(), e)).collect();
    // By key: the notes, their days, and the newest one's place and spelling.
    let mut names: HashMap<String, (Vec<&str>, BTreeSet<&str>, (&str, &str), String)> =
        HashMap::new();
    for (id, key, name) in db.mention_rows()? {
        let Some(entry) = entries.get(id.as_str()) else {
            continue;
        };
        let when = (entry.date.as_str(), entry.time.as_str());
        let slot = names
            .entry(key)
            .or_insert_with(|| (Vec::new(), BTreeSet::new(), when, name.clone()));
        if !slot.0.contains(&entry.id.as_str()) {
            slot.0.push(entry.id.as_str());
        }
        slot.1.insert(entry.date.as_str());
        if when > slot.2 {
            slot.2 = when;
            slot.3 = name;
        }
    }
    let mut out: Vec<MentionSummary> = names
        .into_iter()
        .map(|(key, (notes, days, (last, _), name))| MentionSummary {
            name,
            key,
            notes: notes.len(),
            last: last.to_string(),
            days: days.into_iter().map(str::to_string).collect(),
            pinned: false,
        })
        .collect();
    out.sort_by(|a, b| (b.notes, &b.last, &a.key).cmp(&(a.notes, &a.last, &b.key)));
    Ok(out)
}

/// The notes and pages mentioning a name, newest first, and the name as
/// the newest of them types it.
#[derive(Debug, Default, Serialize)]
pub struct MentionNotes {
    pub name: Option<String>,
    pub notes: Vec<Note>,
}

/// The notes and pages that mention `name`, newest first.
pub fn notes(index: &Index, db: &SearchDb, name: &str) -> Result<MentionNotes> {
    let mut names = db.mentioning(&key(name))?;
    let mut hits: Vec<&IndexEntry> = index
        .entries()
        .filter(|e| names.contains_key(&e.id))
        .collect();
    hits.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.time.cmp(&a.time)));
    let name = hits.first().and_then(|newest| names.remove(&newest.id));
    Ok(MentionNotes {
        name,
        notes: crate::search::with_bodies(db, hits)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(body: &str) -> Vec<String> {
        mentions(body).into_iter().map(|m| m.name).collect()
    }

    #[test]
    fn a_name_is_letters_and_digits_of_any_script() {
        assert_eq!(
            names("call @marie and @Zoë about @build-team"),
            ["marie", "Zoë", "build-team"]
        );
        assert_eq!(names("@bob- and @alice_."), ["bob", "alice"]);
        assert_eq!(names("(@ProjectA)"), ["ProjectA"]);
        assert_eq!(names("@ alone, @-dash"), Vec::<String>::new());
    }

    #[test]
    fn an_email_is_no_mention() {
        assert_eq!(
            names("write to me@example.com or a_b@c"),
            Vec::<String>::new()
        );
        assert_eq!(names("@@bob"), ["bob"]);
    }

    #[test]
    fn case_does_not_count_and_each_name_comes_once() {
        let found = mentions("@Marie then @marie then @MARIE");
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0],
            Mention {
                key: "marie".into(),
                name: "Marie".into()
            }
        );
    }

    #[test]
    fn code_escapes_and_link_targets_hold_no_mention() {
        assert_eq!(names("`@inline` and ``a ` @two``, then @out"), ["out"]);
        assert_eq!(names("\\@escaped @kept"), ["kept"]);
        assert_eq!(
            names("[see @alice](https://x.y/@bob) <https://x.y/@carol>"),
            ["alice"]
        );
        assert_eq!(
            names("```\n@fenced\n```\n@after\n~~~~\n@tilde\n~~~\n@still"),
            ["after"]
        );
        assert_eq!(
            names("see https://x.y/@bob, (www.x.com/@user) @carol http:/@dan"),
            ["carol", "dan"]
        );
        // An unclosed backtick is a backtick.
        assert_eq!(names("a ` @bob"), ["bob"]);
    }
}
