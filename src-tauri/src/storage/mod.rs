pub mod daily_file;
pub mod writer;

use std::path::{Path, PathBuf};

/// Path of a day's markdown file relative to the notes root, e.g.
/// `notes/2026/2026-09-22.md`.
pub fn relative_day_path(date: &str) -> String {
    format!("notes/{}/{}.md", &date[..4], date)
}

pub fn day_path(root: &Path, date: &str) -> PathBuf {
    root.join(relative_day_path(date))
}
