//! The language notes are labelled in, from the language setting.

/// One of the interface's locales: its code, as settings.json, the note
/// markers and categories.json spell it, and its English name for the prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language {
    pub code: &'static str,
    pub name: &'static str,
}

/// What the interface falls back to, and what notes labelled before the
/// language was recorded count as.
pub const ENGLISH: Language = Language {
    code: "en",
    name: "English",
};

/// Keep in step with `LANGUAGE_NAMES` in `src/lib/i18n.svelte.ts`.
const ALL: [Language; 6] = [
    ENGLISH,
    Language {
        code: "fr",
        name: "French",
    },
    Language {
        code: "es",
        name: "Spanish",
    },
    Language {
        code: "de",
        name: "German",
    },
    Language {
        code: "it",
        name: "Italian",
    },
    Language {
        code: "pt",
        name: "Portuguese",
    },
];

/// "fr", "fr-FR" and "fr_CA" all give French; "system" and unknown ones none.
pub fn find(locale: &str) -> Option<Language> {
    let code = locale.split(['-', '_']).next()?.to_ascii_lowercase();
    ALL.into_iter().find(|language| language.code == code)
}

/// The language the setting names, else the OS one, else English, the way
/// the interface resolves it.
pub fn resolve(setting: &str) -> Language {
    find(setting)
        .or_else(|| sys_locale::get_locale().and_then(|os| find(&os)))
        .unwrap_or(ENGLISH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_locale_finds_its_language_by_its_first_part() {
        assert_eq!(resolve("fr").name, "French");
        assert_eq!(find("pt-BR").map(|l| l.name), Some("Portuguese"));
        assert_eq!(find("de_AT").map(|l| l.code), Some("de"));
        assert_eq!(find("system"), None);
        assert_eq!(find("nl"), None);
    }
}
