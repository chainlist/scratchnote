//! What reading and writing the markdown files share: the attributes of a
//! block's marker, and keeping a file's line endings and text as they are.

/// In a marker, the user cleared the day ahead (SPEC 5.3).
pub(crate) const AHEAD_OFF: &str = "ahead=off";

/// The attributes of a marker line such as
/// `<!-- sn:note id=01J8Z3K6Q9X2 time=14:32 -->`. Any other attribute is
/// read past, and the last of one named twice wins.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Marker<'a> {
    pub id: Option<&'a str>,
    pub time: Option<&'a str>,
    /// A page file's day. Notes and stubs take theirs from the day's file.
    pub day: Option<&'a str>,
    pub ahead_off: bool,
}

impl<'a> Marker<'a> {
    /// The marker `header` is, opened by `open`. `None` when it is not one.
    pub fn parse(header: &'a str, open: &str) -> Option<Self> {
        let attrs = header.strip_prefix(open)?.strip_suffix("-->")?.trim();
        let mut marker = Self::default();
        for pair in attrs.split_whitespace() {
            match pair.split_once('=') {
                Some(("id", v)) => marker.id = Some(v),
                Some(("time", v)) => marker.time = Some(v),
                Some(("day", v)) => marker.day = Some(v),
                _ => {}
            }
            marker.ahead_off |= pair == AHEAD_OFF;
        }
        Some(marker)
    }
}

/// The line ending a file uses: an editor on Windows may well have written
/// it with CRLF, and a rewrite keeps whatever it found.
pub fn newline_of(content: &str) -> &'static str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// `content` without the byte order mark some editors start a file with.
pub fn strip_bom(content: &str) -> &str {
    content.strip_prefix('\u{feff}').unwrap_or(content)
}

/// `text` on one line: surrounding whitespace dropped, and every run of it
/// inside made one space.
pub fn collapse_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_attributes_of_a_marker() {
        let open = "<!-- sn:page ";
        let marker = Marker::parse(
            "<!-- sn:page id=01A day=2026-09-22 time=10:00 ahead=off -->",
            open,
        )
        .unwrap();
        assert_eq!(
            marker,
            Marker {
                id: Some("01A"),
                time: Some("10:00"),
                day: Some("2026-09-22"),
                ahead_off: true,
            }
        );
        assert_eq!(Marker::parse("<!-- sn:note id=01A -->", open), None);
        assert_eq!(Marker::parse("<!-- sn:page id=01A", open), None);
    }

    #[test]
    fn keeps_line_endings_and_tidies_text() {
        assert_eq!(newline_of("a\r\nb"), "\r\n");
        assert_eq!(newline_of("a\nb"), "\n");
        assert_eq!(strip_bom("\u{feff}# Title"), "# Title");
        assert_eq!(collapse_spaces("  Weekly \n sync "), "Weekly sync");
    }
}
