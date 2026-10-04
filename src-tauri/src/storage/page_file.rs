//! Page files, SPEC 4.7: a note with a title and a markdown file of its own.
//!
//! The first line is the marker, `<!-- sn:page ... -->`, with a note's
//! attributes plus `day`. Then come the `# Title` line and the text. The
//! whole file is the page's, so unlike a day's file there is nothing of the
//! user's around it to keep. A page written while a model labelled the notes
//! carries a `status` in its marker and a category line under its title, as
//! a note does, which are read past.

use super::check_date;
use super::daily_file::{body_hash, is_labelled, strip_category, Kind, Note, AHEAD_OFF, PAGE_OPEN};

/// The longest title part of a page's file name, in characters.
const MAX_NAME_TITLE: usize = 80;

/// A title as typed, on one line. `None` when nothing is left of it.
pub fn clean_title(raw: &str) -> Option<String> {
    let title = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    (!title.is_empty()).then_some(title)
}

/// The file name for a page of `date` called `title`: `<date> <title>.md`,
/// with ` 2`, ` 3` and so on added for `n` above 1, when the plain name is
/// taken. Characters Windows forbids become spaces, so the name works on
/// every platform; the date in front keeps it clear of reserved names and
/// leading dots.
pub fn file_name(date: &str, title: &str, n: usize) -> String {
    let spaced: String = title
        .chars()
        .map(|c| {
            if c.is_control() || r#"<>:"/\|?*"#.contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let collapsed = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = collapsed.chars().take(MAX_NAME_TITLE).collect();
    let safe = cut.trim_end_matches(|c: char| c == '.' || c.is_whitespace());
    let safe = if safe.is_empty() { "page" } else { safe };
    let suffix = if n > 1 {
        format!(" {n}")
    } else {
        String::new()
    };
    format!("{date} {safe}{suffix}.md")
}

/// Where a page of `date` named `name` goes, relative to the space's root.
pub fn relative_path(date: &str, name: &str) -> String {
    format!("pages/{}/{name}", &date[..4])
}

/// The title a file's name carries, without its date, for a page file that
/// has lost its heading.
fn title_from_file(file: &str) -> String {
    let name = file.rsplit('/').next().unwrap_or(file);
    let stem = name.strip_suffix(".md").unwrap_or(name);
    match stem.split_once(' ') {
        Some((date, rest)) if check_date(date).is_ok() && !rest.trim().is_empty() => {
            rest.trim().to_string()
        }
        _ => stem.to_string(),
    }
}

pub fn render_page(page: &Note) -> String {
    let off = if page.ahead_off {
        format!(" {AHEAD_OFF}")
    } else {
        String::new()
    };
    let mut out = format!(
        "{PAGE_OPEN}id={} day={} time={}{off} -->\n",
        page.id, page.date, page.time
    );
    out.push_str("# ");
    out.push_str(page.subject.as_deref().unwrap_or_default());
    out.push('\n');
    let body = page.body.trim_end();
    if !body.is_empty() {
        out.push('\n');
        out.push_str(body);
        out.push('\n');
    }
    out
}

/// Parse a page file. `file` is its path relative to the space's root, which
/// the page carries and falls back on for a title. `None` when the file does
/// not start with a page's marker, so it is not a page.
pub fn parse_page(content: &str, file: &str) -> Option<Note> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut lines = content.lines();
    let header = lines.next()?.trim();
    let attrs = header.strip_prefix(PAGE_OPEN)?.strip_suffix("-->")?.trim();

    let (mut id, mut day, mut time) = (None, None, None);
    for pair in attrs.split_whitespace() {
        match pair.split_once('=') {
            Some(("id", v)) => id = Some(v.to_string()),
            Some(("day", v)) => day = Some(v.to_string()),
            Some(("time", v)) => time = Some(v.to_string()),
            _ => {}
        }
    }
    let day = day.filter(|d| check_date(d).is_ok())?;
    let ahead_off = attrs.split_whitespace().any(|pair| pair == AHEAD_OFF);

    let all: Vec<&str> = lines.collect();
    let mut rest = skip_blank(&all);
    let heading = rest.first().and_then(|l| l.strip_prefix("# "));
    if heading.is_some() {
        rest = &rest[1..];
    }
    let title = heading
        .and_then(clean_title)
        .unwrap_or_else(|| title_from_file(file));
    let body = if is_labelled(header) {
        strip_category(skip_blank(rest))
    } else {
        skip_blank(rest).join("\n").trim_end().to_string()
    };
    let on = (!ahead_off)
        .then(|| day_ahead(&title, &body, &day))
        .flatten();

    Some(Note {
        id: id?,
        date: day,
        time: time?,
        file: file.to_string(),
        subject: Some(title),
        hash: body_hash(&body),
        on,
        ahead_off,
        body,
        kind: Kind::Page,
        missing: false,
    })
}

/// The later day a page of `day` looks forward to (SPEC 5.3), in its title
/// as much as in its text.
pub fn day_ahead(title: &str, body: &str, day: &str) -> Option<String> {
    crate::ahead::day_ahead(&format!("{title}\n{body}"), day)
}

fn skip_blank<'a, 'b>(mut lines: &'a [&'b str]) -> &'a [&'b str] {
    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines = &lines[1..];
    }
    lines
}

/// Parse, edit and render a page file, keeping its line endings. `None` when
/// it is not a page file.
fn rewrite(content: &str, file: &str, edit: impl FnOnce(&mut Note)) -> Option<String> {
    let mut page = parse_page(content, file)?;
    edit(&mut page);
    page.hash = body_hash(&page.body);
    let rendered = render_page(&page);
    Some(if content.contains("\r\n") {
        rendered.replace('\n', "\r\n")
    } else {
        rendered
    })
}

/// Clear the page's day ahead for good. The title and text stay.
pub fn clear_day_ahead(content: &str, file: &str) -> Option<String> {
    rewrite(content, file, |page| page.ahead_off = true)
}

/// The page written as pages are now, without the category a model gave
/// it. `None` when it is not a page, or carries no labels.
pub fn drop_labels(content: &str, file: &str) -> Option<String> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let header = content.lines().next()?;
    is_labelled(header)
        .then(|| rewrite(content, file, |_| {}))
        .flatten()
}

/// Swap the text. The title stays.
pub fn replace_body(content: &str, file: &str, body: &str) -> Option<String> {
    rewrite(content, file, |page| page.body = body.trim().to_string())
}

pub fn set_title(content: &str, file: &str, title: &str) -> Option<String> {
    rewrite(content, file, |page| page.subject = Some(title.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "pages/2026/2026-09-22 Weekly sync, platform team.md";

    fn page(body: &str) -> Note {
        Note {
            id: "01J9ABC0".to_string(),
            date: "2026-09-22".to_string(),
            time: "10:00".to_string(),
            file: FILE.to_string(),
            subject: Some("Weekly sync, platform team".to_string()),
            hash: body_hash(body),
            on: None,
            ahead_off: false,
            body: body.to_string(),
            kind: Kind::Page,
            missing: false,
        }
    }

    /// A page as it was written while a model labelled the notes.
    const LABELLED: &str = concat!(
        "<!-- sn:page id=01J9ABC0 day=2026-09-22 time=10:00 status=done hash=0badc0de lang=en -->\n",
        "# Weekly sync, platform team\n",
        "\n",
        "> #infrastructure\n",
        "\n",
        "Attendees: Sara, Marc, Kevin.\n",
        "\n",
        "Staging broke again.\n",
    );

    #[test]
    fn renders_the_layout_given_in_the_spec() {
        assert_eq!(
            render_page(&page(
                "Attendees: Sara, Marc, Kevin.\n\nStaging broke again."
            )),
            concat!(
                "<!-- sn:page id=01J9ABC0 day=2026-09-22 time=10:00 -->\n",
                "# Weekly sync, platform team\n",
                "\n",
                "Attendees: Sara, Marc, Kevin.\n",
                "\n",
                "Staging broke again.\n",
            )
        );
    }

    #[test]
    fn round_trips_with_and_without_text() {
        for note in [
            page("just text"),
            page(""),
            page("> #1 priority\n> ship it"),
            page("# a heading inside the text"),
            Note {
                ahead_off: true,
                ..page("Review on Friday")
            },
        ] {
            assert_eq!(parse_page(&render_page(&note), FILE), Some(note));
        }
    }

    #[test]
    fn a_file_without_the_marker_is_not_a_page() {
        assert_eq!(parse_page("# Just some markdown\n\ntext\n", FILE), None);
        assert_eq!(parse_page("", FILE), None);
        let no_day = "<!-- sn:page id=01A time=10:00 -->\n# T\n";
        assert_eq!(parse_page(no_day, FILE), None);
    }

    #[test]
    fn a_page_without_its_heading_takes_its_title_from_the_file_name() {
        let doc = "<!-- sn:page id=01A day=2026-09-22 time=10:00 -->\nthe text\n";
        let note = parse_page(doc, FILE).unwrap();
        assert_eq!(note.subject.as_deref(), Some("Weekly sync, platform team"));
        assert_eq!(note.body, "the text");
        assert_eq!(title_from_file("pages/2026/notes.md"), "notes");
    }

    #[test]
    fn a_labelled_page_is_read_without_its_category_and_written_without_it() {
        let read = parse_page(LABELLED, FILE).unwrap();
        assert_eq!(
            read,
            page("Attendees: Sara, Marc, Kevin.\n\nStaging broke again.")
        );

        let out = drop_labels(LABELLED, FILE).unwrap();
        assert_eq!(out, render_page(&read));
        assert_eq!(drop_labels(&out, FILE), None, "nothing left to drop");
    }

    #[test]
    fn the_day_ahead_is_read_off_the_title_and_the_text() {
        let in_text = page("Review on Friday");
        let read = parse_page(&render_page(&in_text), FILE).unwrap();
        assert_eq!(read.on.as_deref(), Some("2026-09-25"));

        let titled = Note {
            subject: Some("Demo tomorrow".to_string()),
            ..page("slides")
        };
        let read = parse_page(&render_page(&titled), FILE).unwrap();
        assert_eq!(read.on.as_deref(), Some("2026-09-23"));

        let out = clear_day_ahead(&render_page(&titled), FILE).unwrap();
        assert!(
            out.starts_with("<!-- sn:page id=01J9ABC0 day=2026-09-22 time=10:00 ahead=off -->\n")
        );
        assert_eq!(parse_page(&out, FILE).unwrap().on, None);
    }

    #[test]
    fn edits_keep_everything_else_and_the_line_endings() {
        let doc = render_page(&page("old text")).replace('\n', "\r\n");

        let out = replace_body(&doc, FILE, "  new text \n").unwrap();
        assert!(
            !out.replace("\r\n", "").contains('\n'),
            "a bare LF in a CRLF file"
        );
        let after = parse_page(&out, FILE).unwrap();
        assert_eq!(after.body, "new text");
        assert_eq!(after.hash, body_hash("new text"));

        let out = set_title(&out, FILE, "Renamed").unwrap();
        let after = parse_page(&out, FILE).unwrap();
        assert_eq!(after.subject.as_deref(), Some("Renamed"));
        assert_eq!(after.body, "new text");
    }

    #[test]
    fn titles_are_tidied_onto_one_line() {
        assert_eq!(clean_title("  Weekly \n sync "), Some("Weekly sync".into()));
        assert_eq!(clean_title(" \n "), None);
    }

    #[test]
    fn file_names_work_on_every_platform() {
        let date = "2026-09-22";
        assert_eq!(
            file_name(date, "Weekly sync, platform team", 1),
            "2026-09-22 Weekly sync, platform team.md"
        );
        assert_eq!(
            file_name(date, "Q3 review: infra/ops?", 1),
            "2026-09-22 Q3 review infra ops.md"
        );
        assert_eq!(
            file_name(date, "Weekly sync", 3),
            "2026-09-22 Weekly sync 3.md"
        );
        assert_eq!(file_name(date, "Wait...", 1), "2026-09-22 Wait.md");
        assert_eq!(file_name(date, "???", 1), "2026-09-22 page.md");
        let long = file_name(date, &"x".repeat(200), 1);
        assert_eq!(
            long.chars().count(),
            "2026-09-22 ".len() + MAX_NAME_TITLE + ".md".len()
        );
        assert_eq!(
            relative_path(date, &file_name(date, "Sync", 1)),
            "pages/2026/2026-09-22 Sync.md"
        );
    }
}
