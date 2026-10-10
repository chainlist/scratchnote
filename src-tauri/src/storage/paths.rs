//! Where things go in a space's folder, and file names that work on every
//! platform.

use std::path::{Path, PathBuf};

use super::markdown::collapse_spaces;

/// The characters Windows refuses in a file name.
const FORBIDDEN: &str = r#"<>:"/\|?*"#;

/// The app's own folder in a notes root or a space: the index, the
/// databases, settings and plugins.
pub fn meta_dir(root: &Path) -> PathBuf {
    root.join(".scratchnote")
}

/// Whether `c` cannot go in a file name on some platform.
pub fn forbidden(c: char) -> bool {
    c.is_control() || FORBIDDEN.contains(c)
}

/// `part` of a file name made safe everywhere: the characters Windows
/// forbids, and any of `also`, become spaces. Whitespace collapses, and it
/// is cut to `max` characters without trailing dots or spaces.
pub fn safe_part(part: &str, max: usize, also: &str) -> String {
    let spaced: String = part
        .chars()
        .map(|c| {
            if forbidden(c) || also.contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let cut: String = collapse_spaces(&spaced).chars().take(max).collect();
    cut.trim_end_matches(|c: char| c == '.' || c.is_whitespace())
        .to_string()
}

/// `stem` and `extension` put back together, with ` 2`, ` 3` and so on
/// before the extension for `n` above 1.
pub fn numbered(stem: &str, extension: Option<&str>, n: usize) -> String {
    let suffix = if n > 1 {
        format!(" {n}")
    } else {
        String::new()
    };
    match extension {
        Some(ext) => format!("{stem}{suffix}.{ext}"),
        None => format!("{stem}{suffix}"),
    }
}

/// The first of `name(1)`, `name(2)` and so on that `taken` does not hold.
pub fn first_free(name: impl Fn(usize) -> String, taken: impl Fn(&str) -> bool) -> String {
    (1..)
        .map(name)
        .find(|candidate| !taken(candidate))
        .expect("some numbered name is free")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_made_safe_and_numbered() {
        assert_eq!(
            safe_part("Q3 review: infra/ops?", 80, ""),
            "Q3 review infra ops"
        );
        assert_eq!(safe_part("a#b%c", 80, "#%"), "a b c");
        assert_eq!(safe_part("Wait...", 80, ""), "Wait");
        assert_eq!(numbered("shot", Some("png"), 1), "shot.png");
        assert_eq!(numbered("shot", Some("png"), 2), "shot 2.png");
        assert_eq!(numbered("README", None, 3), "README 3");
        let taken = ["a", "a 2"];
        let free = first_free(|n| numbered("a", None, n), |name| taken.contains(&name));
        assert_eq!(free, "a 3");
    }
}
