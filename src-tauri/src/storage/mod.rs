pub mod daily_file;
pub mod index;
pub mod page_file;
pub mod search_db;
pub mod writer;

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Path of a day's markdown file relative to the notes root, e.g.
/// `notes/2026/2026-09-22.md`.
pub fn relative_day_path(date: &str) -> String {
    format!("notes/{}/{}.md", &date[..4], date)
}

pub fn day_path(root: &Path, date: &str) -> PathBuf {
    root.join(relative_day_path(date))
}

/// Guards the `YYYY-MM-DD` shape the file layout is built on, so a bad date
/// can neither reach into the filesystem nor panic `relative_day_path`.
pub fn check_date(date: &str) -> Result<(), String> {
    let ok = date.len() == 10
        && date.as_bytes()[4] == b'-'
        && date.as_bytes()[7] == b'-'
        && date.char_indices().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        });
    if ok {
        Ok(())
    } else {
        Err(format!("not a YYYY-MM-DD date: {date}"))
    }
}

/// Full SHA-256 of a file's contents, used to tell the app's own writes apart
/// from somebody editing the file in another editor.
pub fn fingerprint(contents: &str) -> String {
    let digest = Sha256::digest(contents.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_well_formed_date() {
        assert!(check_date("2026-09-22").is_ok());
    }

    #[test]
    fn rejects_anything_that_could_escape_the_notes_directory() {
        for bad in ["", "2026-9-22", "2026/09/22", "../../etc", "2026-09-2x"] {
            assert!(check_date(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn a_valid_date_maps_onto_the_layout_from_the_spec() {
        assert_eq!(relative_day_path("2026-09-22"), "notes/2026/2026-09-22.md");
    }

    #[test]
    fn fingerprints_differ_for_different_contents() {
        assert_eq!(fingerprint("a"), fingerprint("a"));
        assert_ne!(fingerprint("a"), fingerprint("b"));
        assert_eq!(fingerprint("a").len(), 64);
    }
}
