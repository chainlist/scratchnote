//! The day ahead, SPEC 5.3: the later day a note looks forward to, which
//! brings the note back on that day's view.
//!
//! Read off the note's own words, by rules, in the six languages: tomorrow,
//! the day after, in two weeks, a weekday, next week or next month, the 12th
//! of March, `12.03.2026`, `2026-10-06`, by the 5th. The day is worked out
//! from the day the note was written. A wrong day is worse than none, so a
//! stretch of the note that speaks of the past is passed over, and figures
//! that could be read two ways, such as `12/03`, are left alone.

use chrono::{Datelike, Days, Months, NaiveDate, Weekday};

use crate::search::fold;

/// A day further ahead than this from the note is taken for a misreading.
const MAX_AHEAD_DAYS: i64 = 730;

/// Each day of the week by its names, folded (lowercase, accents stripped).
/// Portuguese counts its days, segunda to sexta, which are days only as
/// `segunda-feira` and the like.
const WEEKDAYS: &[(Weekday, &[&str])] = &[
    (
        Weekday::Mon,
        &["monday", "lundi", "lunes", "montag", "lunedi", "segunda"],
    ),
    (
        Weekday::Tue,
        &[
            "tuesday", "tues", "mardi", "martes", "dienstag", "martedi", "terca",
        ],
    ),
    (
        Weekday::Wed,
        &[
            "wednesday",
            "mercredi",
            "miercoles",
            "mittwoch",
            "mercoledi",
            "quarta",
        ],
    ),
    (
        Weekday::Thu,
        &[
            "thursday",
            "thurs",
            "jeudi",
            "jueves",
            "donnerstag",
            "giovedi",
            "quinta",
        ],
    ),
    (
        Weekday::Fri,
        &[
            "friday", "vendredi", "viernes", "freitag", "venerdi", "sexta",
        ],
    ),
    (
        Weekday::Sat,
        &[
            "saturday",
            "samedi",
            "sabado",
            "samstag",
            "sonnabend",
            "sabato",
        ],
    ),
    (
        Weekday::Sun,
        &["sunday", "dimanche", "domingo", "sonntag", "domenica"],
    ),
];
/// The Portuguese days that need `feira` after them.
const COUNTED_DAYS: &[&str] = &["segunda", "terca", "quarta", "quinta", "sexta"];

/// Each month by its names, folded, from January. Only ever read next to a
/// day's number, so `may` and `mars` are safe.
const MONTHS: &[&[&str]] = &[
    &[
        "january", "jan", "janvier", "enero", "januar", "gennaio", "janeiro",
    ],
    &[
        "february",
        "feb",
        "fevrier",
        "febrero",
        "februar",
        "febbraio",
        "fevereiro",
    ],
    &["march", "mar", "mars", "marzo", "marz", "marco"],
    &["april", "apr", "avril", "abril", "aprile"],
    &["may", "mai", "mayo", "maggio", "maio"],
    &["june", "jun", "juin", "junio", "juni", "giugno", "junho"],
    &["july", "jul", "juillet", "julio", "juli", "luglio", "julho"],
    &["august", "aug", "aout", "agosto"],
    &[
        "september",
        "sep",
        "sept",
        "septembre",
        "septiembre",
        "setiembre",
        "settembre",
        "setembro",
    ],
    &[
        "october", "oct", "octobre", "octubre", "oktober", "ottobre", "outubro",
    ],
    &["november", "nov", "novembre", "noviembre", "novembro"],
    &[
        "december",
        "dec",
        "decembre",
        "diciembre",
        "dezember",
        "dicembre",
        "dezembro",
    ],
];

/// Tomorrow. `manana` and `morgen` are also the morning, which the word
/// before them tells apart.
const TOMORROW: &[&str] = &[
    "tomorrow", "tmrw", "tmr", "demain", "manana", "morgen", "domani", "amanha",
];
const MORNING_BEFORE: &[&str] = &[
    "la", "esta", "por", "cada", "una", "toda", "heute", "guten", "am", "jeden", "gestern", "den",
    "einen",
];

/// The day after tomorrow, each phrase as the words it is made of.
const DAY_AFTER_TOMORROW: &[&[&str]] = &[
    &["day", "after", "tomorrow"],
    &["apres", "demain"],
    &["pasado", "manana"],
    &["ubermorgen"],
    &["dopodomani"],
    &["depois", "de", "amanha"],
    &["depois", "amanha"],
];

/// Words that put a stretch of the note in the past: "last Tuesday", "lundi
/// dernier", "since Monday", "met Sara on Monday". A day it names is not
/// one to look forward to.
const PAST: &[&str] = &[
    // English
    "yesterday",
    "ago",
    "last",
    "previous",
    "since",
    "was",
    "were",
    "went",
    "met",
    "had",
    "did",
    // French
    "hier",
    "dernier",
    "derniere",
    "passee",
    "depuis",
    // Spanish, with `pasado` that is not `pasado manana`
    "ayer",
    "anoche",
    "pasado",
    "pasada",
    "desde",
    "fui",
    "fue",
    // German
    "gestern",
    "vorgestern",
    "letzte",
    "letzten",
    "letzter",
    "letztes",
    "vergangene",
    "vergangenen",
    "seit",
    "war",
    "waren",
    "hatte",
    "hatten",
    // Italian
    "ieri",
    "scorso",
    "scorsa",
    "passato",
    "passata",
    // Portuguese
    "ontem",
    "anteontem",
    "passado",
    "passada",
    "atras",
];

/// What makes a week or a month the next one: "next week", "semaine
/// prochaine", "nächste Woche", "settimana prossima", "la semana que viene",
/// "o mês que vem".
const NEXT: &[&str] = &[
    "next",
    "prochaine",
    "prochain",
    "proxima",
    "proximo",
    "nachste",
    "nachsten",
    "nachster",
    "nachstes",
    "kommende",
    "kommenden",
    "prossima",
    "prossimo",
];
/// After `que`: "semana que viene", "semana que vem".
const COMING: &[&str] = &["viene", "vem"];

const DAY_UNITS: &[&str] = &[
    "day", "days", "jour", "jours", "dia", "dias", "tag", "tage", "tagen", "giorno", "giorni",
];
const WEEK_UNITS: &[&str] = &[
    "week",
    "weeks",
    "semaine",
    "semaines",
    "semana",
    "semanas",
    "woche",
    "wochen",
    "settimana",
    "settimane",
];
const MONTH_UNITS: &[&str] = &[
    "month", "months", "mois", "mes", "meses", "monat", "monate", "monaten", "mese", "mesi",
];

/// What comes before a count of days, weeks or months ahead: "in", "dans",
/// "en", "tra", "fra", "em". "Daqui a" and "dentro de" take one more word.
const IN: &[&str] = &["in", "dans", "en", "tra", "fra", "em"];

/// Numbers in words, for a count ahead.
const NUMBERS: &[(&str, u32)] = &[
    ("a", 1),
    ("an", 1),
    ("one", 1),
    ("un", 1),
    ("une", 1),
    ("una", 1),
    ("uno", 1),
    ("um", 1),
    ("uma", 1),
    ("ein", 1),
    ("eine", 1),
    ("einem", 1),
    ("einer", 1),
    ("two", 2),
    ("deux", 2),
    ("dos", 2),
    ("zwei", 2),
    ("due", 2),
    ("dois", 2),
    ("duas", 2),
    ("three", 3),
    ("trois", 3),
    ("tres", 3),
    ("drei", 3),
    ("tre", 3),
    ("four", 4),
    ("quatre", 4),
    ("cuatro", 4),
    ("vier", 4),
    ("quattro", 4),
    ("quatro", 4),
    ("five", 5),
    ("cinq", 5),
    ("cinco", 5),
    ("funf", 5),
    ("cinque", 5),
    ("six", 6),
    ("seis", 6),
    ("sechs", 6),
    ("sei", 6),
    ("seven", 7),
    ("sept", 7),
    ("siete", 7),
    ("sieben", 7),
    ("sette", 7),
    ("sete", 7),
    ("eight", 8),
    ("huit", 8),
    ("ocho", 8),
    ("acht", 8),
    ("otto", 8),
    ("oito", 8),
    ("nine", 9),
    ("neuf", 9),
    ("nueve", 9),
    ("neun", 9),
    ("nove", 9),
    ("ten", 10),
    ("dix", 10),
    ("diez", 10),
    ("zehn", 10),
    ("dieci", 10),
    ("dez", 10),
    ("fifteen", 15),
    ("quinze", 15),
    ("quince", 15),
    ("quindici", 15),
];

/// What comes before a day of the month alone: "by the 5th", "avant le 5",
/// "bis zum 5.", "entro il 5", "hasta el 5", "até o dia 5".
const BY: &[&str] = &[
    "by", "before", "until", "till", "due", "on", "avant", "pour", "jusqu", "bis", "am", "entro",
    "prima", "per", "ate", "antes", "hasta", "para",
];
const ARTICLES: &[&str] = &[
    "the", "le", "el", "il", "dem", "den", "o", "del", "al", "au", "zum", "dia", "de",
];

/// What joins a day and its month: "12th of October", "12 de marzo".
const OF: &[&str] = &["of", "de", "di", "do", "da"];

/// Where a stretch of the note ends: a day it names belongs to its own
/// sentence or clause.
const BREAKS: &[char] = &['.', '!', '?', ';', ':', ',', '\n', '(', ')', '[', ']'];

/// A word, or numbers joined by `/`, `.` or `-`, as in `12/03`,
/// `12.03.2026` or `2026-10-06`: each number and how many figures it has.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word(String),
    Figures(Vec<(u32, usize)>, char),
}

/// The day ahead of a note written on `written`, a `YYYY-MM-DD` date, in the
/// same form. `None` when it names none, or `written` is no date.
pub fn day_ahead(text: &str, written: &str) -> Option<String> {
    let written = NaiveDate::parse_from_str(written, "%Y-%m-%d").ok()?;
    read(text, written).map(|day| day.format("%Y-%m-%d").to_string())
}

/// The first later day the note names, in a stretch of it that does not
/// speak of the past.
pub fn read(text: &str, written: NaiveDate) -> Option<NaiveDate> {
    clauses(&fold(text))
        .iter()
        .filter(|clause| !in_the_past(clause))
        .find_map(|clause| in_clause(clause, written))
        .filter(|day| (1..=MAX_AHEAD_DAYS).contains(&(*day - written).num_days()))
}

/// The folded text as clauses of tokens.
fn clauses(text: &str) -> Vec<Vec<Token>> {
    let chars: Vec<char> = text.chars().collect();
    let mut clauses = vec![Vec::new()];
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_alphanumeric() {
            if BREAKS.contains(&chars[i]) && !clauses.last().is_some_and(Vec::is_empty) {
                clauses.push(Vec::new());
            }
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_alphanumeric() {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();
        let clause = clauses.last_mut().expect("there is always one");
        if word.chars().all(|c| c.is_ascii_digit()) {
            if let Some((figures, end)) = figures(&chars, start) {
                clause.push(figures);
                i = end;
                continue;
            }
            // "12. März": the dot of a German day does not end the clause.
            let german_day = word.len() <= 2
                && chars.get(i) == Some(&'.')
                && chars.get(i + 1) == Some(&' ')
                && chars.get(i + 2).is_some_and(|c| c.is_alphabetic());
            if german_day {
                i += 1;
            }
        }
        clause.push(Token::Word(word));
    }
    clauses.retain(|clause| !clause.is_empty());
    clauses
}

/// Numbers joined by one of `/`, `.` or `-` starting at `from`, and where
/// they end. `None` for a lone number, and for four or more, such as an
/// address or a version.
fn figures(chars: &[char], from: usize) -> Option<(Token, usize)> {
    let number = |at: usize| {
        let len = chars[at..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        let value: String = chars[at..at + len].iter().collect();
        (value.parse::<u32>().unwrap_or(0), len)
    };
    let joined = |at: usize, joiner: char| {
        chars.get(at) == Some(&joiner) && chars.get(at + 1).is_some_and(|c| c.is_ascii_digit())
    };
    let first = number(from);
    let mut end = from + first.1;
    let joiner = *chars.get(end).filter(|c| matches!(c, '/' | '.' | '-'))?;
    let mut parts = vec![first];
    while joined(end, joiner) {
        let next = number(end + 1);
        parts.push(next);
        end += 1 + next.1;
    }
    let ends_clean = !chars.get(end).is_some_and(|c| c.is_alphanumeric());
    (parts.len() >= 2 && parts.len() <= 3 && ends_clean)
        .then(|| (Token::Figures(parts, joiner), end))
}

fn word(tokens: &[Token], at: usize) -> Option<&str> {
    match tokens.get(at)? {
        Token::Word(word) => Some(word),
        Token::Figures(..) => None,
    }
}

fn is(tokens: &[Token], at: usize, words: &[&str]) -> bool {
    word(tokens, at).is_some_and(|word| words.contains(&word))
}

fn in_the_past(clause: &[Token]) -> bool {
    (0..clause.len()).any(|i| {
        let after_tomorrow = is(clause, i, &["pasado"]) && is(clause, i + 1, &["manana"]);
        is(clause, i, PAST) && !after_tomorrow
    })
}

/// The first day the clause names, reading from its start.
fn in_clause(clause: &[Token], written: NaiveDate) -> Option<NaiveDate> {
    let next_week = (0..clause.len()).any(|i| next_period(clause, i, WEEK_UNITS));
    let names_a_weekday = (0..clause.len()).any(|i| weekday(clause, i).is_some());
    (0..clause.len()).find_map(|i| {
        after_tomorrow(clause, i, written)
            .or_else(|| tomorrow(clause, i, written))
            .or_else(|| counted(clause, i, written))
            .or_else(|| date(clause, i, written))
            .or_else(|| in_figures(clause, i, written))
            .or_else(|| on_weekday(clause, i, written, next_week))
            .or_else(|| next_week_or_month(clause, i, written, names_a_weekday))
            .or_else(|| day_of_month(clause, i, written))
    })
}

fn after_tomorrow(clause: &[Token], i: usize, written: NaiveDate) -> Option<NaiveDate> {
    DAY_AFTER_TOMORROW
        .iter()
        .any(|phrase| {
            phrase
                .iter()
                .enumerate()
                .all(|(k, part)| word(clause, i + k) == Some(*part))
        })
        .then(|| written.checked_add_days(Days::new(2)))
        .flatten()
}

fn tomorrow(clause: &[Token], i: usize, written: NaiveDate) -> Option<NaiveDate> {
    let morning = i > 0 && is(clause, i - 1, MORNING_BEFORE);
    (is(clause, i, TOMORROW) && !morning)
        .then(|| written.checked_add_days(Days::new(1)))
        .flatten()
}

/// "In 3 days", "dans deux semaines", "in einem Monat", "daqui a 2 dias",
/// "in a fortnight".
fn counted(clause: &[Token], i: usize, written: NaiveDate) -> Option<NaiveDate> {
    let at = if is(clause, i, IN) {
        i + 1
    } else if (is(clause, i, &["daqui"]) && is(clause, i + 1, &["a"]))
        || (is(clause, i, &["dentro"]) && is(clause, i + 1, &["de"]))
    {
        i + 2
    } else {
        return None;
    };
    let count = word(clause, at).and_then(|word| {
        NUMBERS
            .iter()
            .find(|(name, _)| *name == word)
            .map(|(_, n)| *n)
            .or_else(|| {
                word.chars()
                    .all(|c| c.is_ascii_digit())
                    .then(|| word.parse().ok())
                    .flatten()
            })
    })?;
    if !(1..100).contains(&count) {
        return None;
    }
    if is(clause, at + 1, DAY_UNITS) {
        written.checked_add_days(Days::new(count.into()))
    } else if is(clause, at + 1, WEEK_UNITS) {
        written.checked_add_days(Days::new(u64::from(count) * 7))
    } else if is(clause, at + 1, &["fortnight", "fortnights"]) {
        written.checked_add_days(Days::new(u64::from(count) * 14))
    } else if is(clause, at + 1, MONTH_UNITS) {
        written.checked_add_months(Months::new(count))
    } else {
        None
    }
}

/// A day of the month a word gives: `12`, `12th`, `1st`, `1er`, `3rd`.
fn day_number(clause: &[Token], at: usize) -> Option<u32> {
    let word = word(clause, at)?;
    let figures: String = word.chars().take_while(|c| c.is_ascii_digit()).collect();
    let rest = &word[figures.len()..];
    let ordinal = ["", "st", "nd", "rd", "th", "er", "e", "o", "a", "º", "ª"].contains(&rest);
    let day: u32 = figures.parse().ok()?;
    (ordinal && figures.len() <= 2 && (1..=31).contains(&day)).then_some(day)
}

fn month(clause: &[Token], at: usize) -> Option<u32> {
    let word = word(clause, at)?;
    MONTHS
        .iter()
        .position(|names| names.contains(&word))
        .map(|month| month as u32 + 1)
}

/// A year written after a date: `2027`.
fn year(clause: &[Token], at: usize) -> Option<i32> {
    let word = word(clause, at)?;
    let year: i32 = word.parse().ok()?;
    (word.len() == 4 && (2000..2100).contains(&year)).then_some(year)
}

/// "12 March", "12th of October", "le 27 septembre", "12 de marzo",
/// "October 3", "March the 12th", with a year after or not.
fn date(clause: &[Token], i: usize, written: NaiveDate) -> Option<NaiveDate> {
    let (day, month, end) = if let Some(day) = day_number(clause, i) {
        let at = if is(clause, i + 1, OF) { i + 2 } else { i + 1 };
        (day, month(clause, at)?, at + 1)
    } else {
        let month = month(clause, i)?;
        let at = if is(clause, i + 1, &["the"]) {
            i + 2
        } else {
            i + 1
        };
        (day_number(clause, at)?, month, at + 1)
    };
    on_date(written, year(clause, end), month, day)
}

/// The date, in `year` when given, else the first one after `written`.
fn on_date(written: NaiveDate, year: Option<i32>, month: u32, day: u32) -> Option<NaiveDate> {
    if let Some(year) = year {
        return NaiveDate::from_ymd_opt(year, month, day);
    }
    match NaiveDate::from_ymd_opt(written.year(), month, day) {
        Some(date) if date > written => Some(date),
        _ => NaiveDate::from_ymd_opt(written.year() + 1, month, day),
    }
}

/// `2026-10-06` in any joiner, `12.03` and `12.03.2026` as days come first
/// with dots, and `25/12` or `12/25/2026` when only one reading is a date.
/// `12/03` could be either, and `24/7` is no day. Without a year, the
/// figures must follow a word that leads to a day, as in "le 25/12" or "am
/// 12.03.", since `12.10` may as well be a price.
fn in_figures(clause: &[Token], i: usize, written: NaiveDate) -> Option<NaiveDate> {
    let Some(Token::Figures(parts, joiner)) = clause.get(i) else {
        return None;
    };
    let introduced = i > 0
        && (is(clause, i - 1, BY)
            || is(clause, i - 1, ARTICLES)
            || weekday(clause, i - 1).is_some());
    let year = |&(value, len): &(u32, usize)| match len {
        2 => Some(2000 + value as i32),
        4 => Some(value as i32),
        _ => None,
    };
    let (day, month, year) = match (parts.as_slice(), joiner) {
        ([y, m, d], _) if y.1 == 4 => (d.0, m.0, Some(y.0 as i32)),
        ([d, m], '.') if d.1 <= 2 && m.1 == 2 => (d.0, m.0, None),
        ([d, m, y], '.') => (d.0, m.0, Some(year(y)?)),
        ([a, b, rest @ ..], '/') if a.1 <= 2 && b.1 <= 2 && rest.len() <= 1 => {
            if (a.0, b.0) == (24, 7) {
                return None;
            }
            let year = match rest.first() {
                Some(y) => Some(year(y)?),
                None => None,
            };
            match (a.0 > 12, b.0 > 12) {
                (true, false) => (a.0, b.0, year),
                (false, true) => (b.0, a.0, year),
                _ => return None,
            }
        }
        _ => return None,
    };
    if year.is_none() && !introduced {
        return None;
    }
    on_date(written, year, month, day)
}

/// The weekday a word names, unless it is every one of them: "every Monday",
/// "chaque lundi".
fn weekday(clause: &[Token], i: usize) -> Option<Weekday> {
    let word = word(clause, i)?;
    if COUNTED_DAYS.contains(&word) && !is(clause, i + 1, &["feira"]) {
        return None;
    }
    let every = ["every", "each", "chaque", "cada", "jeden", "jedem", "ogni"];
    if i > 0 && is(clause, i - 1, &every) {
        return None;
    }
    WEEKDAYS
        .iter()
        .find(|(_, names)| names.contains(&word))
        .map(|(day, _)| *day)
}

/// "Tuesday", "next Tuesday", "mardi prochain": the first such day after
/// the note's, a week on at most. With next week in the clause, that day of
/// the week after the note's: "Friday next week". "Monday 12 October" is
/// the date.
fn on_weekday(
    clause: &[Token],
    i: usize,
    written: NaiveDate,
    next_week: bool,
) -> Option<NaiveDate> {
    let wanted = weekday(clause, i)?;
    let at = if is(clause, i + 1, &["feira"]) {
        i + 2
    } else {
        i + 1
    };
    if let Some(day) = date(clause, at, written).or_else(|| in_figures(clause, at, written)) {
        return Some(day);
    }
    if next_week {
        return monday_after(written)?
            .checked_add_days(Days::new(wanted.num_days_from_monday().into()));
    }
    let ahead = (7 + wanted.num_days_from_monday() - written.weekday().num_days_from_monday()) % 7;
    written.checked_add_days(Days::new(if ahead == 0 { 7 } else { ahead.into() }))
}

/// Whether the word at `i` is one of `units` made the next one: "next
/// week", "semaine prochaine", "la semana que viene". A weekend is not a
/// week.
fn next_period(clause: &[Token], i: usize, units: &[&str]) -> bool {
    if !is(clause, i, units) || is(clause, i + 1, &["end"]) {
        return false;
    }
    (i > 0 && is(clause, i - 1, NEXT))
        || is(clause, i + 1, NEXT)
        || (is(clause, i + 1, &["que"]) && is(clause, i + 2, COMING))
}

/// The Monday of next week, or the first of next month, unless the clause
/// names a weekday of it.
fn next_week_or_month(
    clause: &[Token],
    i: usize,
    written: NaiveDate,
    names_a_weekday: bool,
) -> Option<NaiveDate> {
    if next_period(
        clause,
        i,
        &["week", "semaine", "semana", "woche", "settimana"],
    ) {
        return (!names_a_weekday).then(|| monday_after(written)).flatten();
    }
    if next_period(clause, i, &["month", "mois", "mes", "monat", "mese"]) {
        return written.with_day(1)?.checked_add_months(Months::new(1));
    }
    None
}

/// "By the 5th", "avant le 5", "bis zum 5.": the next such day of a month to
/// come. A bare number needs an article before it, as "by 5" may be a time.
fn day_of_month(clause: &[Token], i: usize, written: NaiveDate) -> Option<NaiveDate> {
    if !is(clause, i, BY) {
        return None;
    }
    let mut at = i + 1;
    while at < i + 3 && is(clause, at, ARTICLES) {
        at += 1;
    }
    if let Some(day) = date(clause, at, written) {
        return Some(day);
    }
    let day = day_number(clause, at)?;
    let bare = word(clause, at).is_some_and(|word| word.chars().all(|c| c.is_ascii_digit()));
    let introduced = at > i + 1 || is(clause, i, &["am"]);
    if bare && !introduced {
        return None;
    }
    next_day_of_month(written, day)
}

/// The first day after `date` that is the `day`th of its month, skipping
/// months too short for it.
fn next_day_of_month(date: NaiveDate, day: u32) -> Option<NaiveDate> {
    let first = date.with_day(1)?;
    (0..13).find_map(|ahead| {
        let month = first.checked_add_months(Months::new(ahead))?;
        NaiveDate::from_ymd_opt(month.year(), month.month(), day).filter(|found| *found > date)
    })
}

/// The Monday of the week after `date`'s.
fn monday_after(date: NaiveDate) -> Option<NaiveDate> {
    date.checked_add_days(Days::new(
        7 - u64::from(date.weekday().num_days_from_monday()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    /// Wednesday.
    const WRITTEN: &str = "2026-09-23";

    fn ahead(text: &str) -> Option<String> {
        day_ahead(text, WRITTEN)
    }

    /// Notes in the six languages, each written on a day, and the later day
    /// it looks forward to, or none: a past day, today, no day at all. These
    /// are the cases the model was measured on before the rules replaced it,
    /// and a few more.
    #[test]
    fn reads_the_day_ahead_in_every_language() {
        let cases: &[(&str, &str, Option<&str>)] = &[
            ("Dentist next Tuesday at 3pm", WRITTEN, Some("2026-09-29")),
            (
                "Call Marc back tomorrow about the budget",
                WRITTEN,
                Some("2026-09-24"),
            ),
            (
                "Release moved to Friday because QA found a regression",
                WRITTEN,
                Some("2026-09-25"),
            ),
            (
                "Renew the car insurance before November 1, compare quotes first",
                WRITTEN,
                Some("2026-11-01"),
            ),
            (
                "Revisit the firewall config in two weeks",
                WRITTEN,
                Some("2026-10-07"),
            ),
            ("Met Sara on Monday, she liked the proposal", WRITTEN, None),
            ("Today: standup moved to 10am", WRITTEN, None),
            (
                "Pick up the dry cleaning on Saturday",
                WRITTEN,
                Some("2026-09-26"),
            ),
            (
                "Team offsite next week, book the train",
                WRITTEN,
                Some("2026-09-28"),
            ),
            (
                "Book club on the 12th of October, finish the last chapters",
                WRITTEN,
                Some("2026-10-12"),
            ),
            (
                "Follow up with the landlord in 3 days",
                WRITTEN,
                Some("2026-09-26"),
            ),
            (
                "Last week's retro was rough, fix the on-call rota",
                WRITTEN,
                None,
            ),
            ("Q4 planning starts next month", WRITTEN, Some("2026-10-01")),
            (
                "Watched two episodes of Severance last night",
                WRITTEN,
                None,
            ),
            ("Dentist Tuesday", "2026-09-25", Some("2026-09-29")),
            (
                "Demo for the client next Friday",
                "2026-09-25",
                Some("2026-10-02"),
            ),
            (
                "Réunion demain à 14h avec l'équipe infra",
                WRITTEN,
                Some("2026-09-24"),
            ),
            (
                "Revoir la configuration du pare-feu la semaine prochaine",
                WRITTEN,
                Some("2026-09-28"),
            ),
            (
                "Rendez-vous chez le dentiste mardi à 14h",
                WRITTEN,
                Some("2026-09-29"),
            ),
            (
                "Appeler le comptable dans 15 jours pour la TVA",
                WRITTEN,
                Some("2026-10-08"),
            ),
            (
                "Noter les chiffres du trimestre avant vendredi",
                WRITTEN,
                Some("2026-09-25"),
            ),
            (
                "Anniversaire de maman le 27 septembre",
                WRITTEN,
                Some("2026-09-27"),
            ),
            (
                "La réunion de lundi dernier a duré trois heures",
                WRITTEN,
                None,
            ),
            (
                "Abgabe des Berichts am Freitag",
                WRITTEN,
                Some("2026-09-25"),
            ),
            (
                "Cita con el médico el próximo martes",
                WRITTEN,
                Some("2026-09-29"),
            ),
            (
                "Consegna del progetto dopodomani",
                WRITTEN,
                Some("2026-09-25"),
            ),
            ("Pay the rent by the 5th", WRITTEN, Some("2026-10-05")),
            ("Board meeting Thursday at 9", WRITTEN, Some("2026-09-24")),
            ("Vacation starts in a month", WRITTEN, Some("2026-10-23")),
            ("Flight to Lisbon on October 3", WRITTEN, Some("2026-10-03")),
            (
                "Yesterday's demo went well, the client wants a follow-up",
                WRITTEN,
                None,
            ),
            (
                "Le loyer est à payer avant le 5",
                WRITTEN,
                Some("2026-10-05"),
            ),
            ("Mardi dernier, on a fini le sprint", WRITTEN, None),
            (
                "Lundi prochain, réunion budget",
                WRITTEN,
                Some("2026-09-28"),
            ),
            ("In zwei Wochen Zahnarzt", WRITTEN, Some("2026-10-07")),
            ("Mañana llamo al banco", WRITTEN, Some("2026-09-24")),
            (
                "The release went out on Monday without issues",
                WRITTEN,
                None,
            ),
            // Beyond the cases the model was measured on.
            ("Call back après-demain", WRITTEN, Some("2026-09-25")),
            ("Friday next week: demo", WRITTEN, Some("2026-10-02")),
            ("Next week on Friday: demo", WRITTEN, Some("2026-10-02")),
            ("Release on 2026-10-06", WRITTEN, Some("2026-10-06")),
            ("Abgabe am 12.03.", WRITTEN, Some("2027-03-12")),
            ("Rendu le 12.03.2027", WRITTEN, Some("2027-03-12")),
            ("Abgabe am 12. März", WRITTEN, Some("2027-03-12")),
            ("Fête le 25/12", WRITTEN, Some("2026-12-25")),
            ("Treffen nächste Woche", WRITTEN, Some("2026-09-28")),
            ("Revisión la semana que viene", WRITTEN, Some("2026-09-28")),
            ("Reunião na segunda-feira", WRITTEN, Some("2026-09-28")),
            ("Consegna tra due settimane", WRITTEN, Some("2026-10-07")),
            ("Entrega daqui a 3 dias", WRITTEN, Some("2026-09-26")),
            ("Concert lundi 12 octobre", WRITTEN, Some("2026-10-12")),
            ("Por la mañana fui al banco", WRITTEN, None),
            ("Heute Morgen war es kalt", WRITTEN, None),
        ];
        let wrong: Vec<String> = cases
            .iter()
            .filter_map(|(text, written, wanted)| {
                let got = day_ahead(text, written);
                (got.as_deref() != *wanted).then(|| format!("{text:?}: {got:?} not {wanted:?}"))
            })
            .collect();
        assert!(wrong.is_empty(), "wrong days:\n  {}", wrong.join("\n  "));
    }

    #[test]
    fn figures_that_are_no_day_name_none() {
        for text in [
            "Buy a new USB-C hub for the homelab",
            "Le déploiement de staging a encore échoué",
            "API latency is 1.2s at p99, 3 retries",
            "Version 2.4 is out, then 2.4.1",
            "Costs 3.50 per user, up from 2.99",
            "Passport expires 12/03",
            "Support 24/7 from now on",
            "It takes 10-15 minutes",
            "Server at 192.168.1.10",
            "Costs 12.10 per seat",
            "Holidays 25/12",
            "Standup every Monday at 10",
            "Ships by 5",
            "Plans for this weekend",
            "Le week-end prochain on part",
            "Una reunión el mes pasado",
            "Quinta vez que falha",
        ] {
            assert_eq!(ahead(text), None, "{text}");
        }
    }

    #[test]
    fn the_first_day_named_wins() {
        assert_eq!(
            ahead("Dentist tomorrow, then Tuesday").as_deref(),
            Some("2026-09-24")
        );
        // A clause in the past is passed over, not the whole note.
        assert_eq!(
            ahead("Met Sara on Monday. Lunch with her on Friday").as_deref(),
            Some("2026-09-25")
        );
    }

    #[test]
    fn a_day_before_the_next_is_next_year_s() {
        assert_eq!(ahead("12 March").as_deref(), Some("2027-03-12"));
        assert_eq!(ahead("30 February"), None);
        assert_eq!(ahead("by the 31st").as_deref(), Some("2026-10-31"));
        assert_eq!(
            read("in a month", date("2026-01-31")),
            Some(date("2026-02-28"))
        );
    }

    #[test]
    fn a_day_too_far_ahead_or_already_gone_is_none() {
        assert_eq!(ahead("in 99 months"), None);
        assert_eq!(ahead("Release on 2026-09-01"), None);
        assert_eq!(ahead("Release on 2026-09-23"), None);
        assert_eq!(day_ahead("tomorrow", "not a date"), None);
    }
}
