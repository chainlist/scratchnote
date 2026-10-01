//! The day ahead, SPEC 5.7: the later day a note looks forward to, which
//! brings the note back on that day's view.
//!
//! The model does no arithmetic and looks nothing up. It copies the words of
//! the note that name the day and picks, under a grammar, what kind of day
//! they name: tomorrow, in two weeks, a weekday, the 12th of March, or a day
//! the note does not look forward to. The code checks that the words are the
//! note's own, reads the weekday and the week off them in the six languages,
//! and works the date out from the day the note was written, which a job run
//! days later still knows. A note that names no day in any of the languages
//! is not put to the model at all.

use std::path::{Path, PathBuf};

use chrono::{Datelike, Days, Months, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

use super::model::Backend;
use super::prompt;
use super::queue::Job;
use crate::search::fold;
use crate::storage::daily_file::{Kind as NoteKind, Status};
use crate::storage::index::Index;
use crate::storage::search_db::SearchDb;

/// A day further ahead than this from the note is taken for a misreading.
const MAX_AHEAD_DAYS: i64 = 730;
/// The longest run of words the model may give for the day.
pub const MAX_WORDS: usize = 60;

/// Words that name a day, a week or a month in the six languages, folded
/// (lowercase, accents stripped). A note holding none is asked nothing.
const DAY_WORDS: &[&str] = &[
    // English
    "tomorrow",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
    "tues",
    "thurs",
    "january",
    "february",
    "march",
    "april",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
    "jan",
    "feb",
    "apr",
    "aug",
    "sept",
    "oct",
    "nov",
    "dec",
    "week",
    "weeks",
    "weekend",
    "month",
    "months",
    "fortnight",
    "tonight",
    // French
    "demain",
    "apres-demain",
    "lundi",
    "mardi",
    "mercredi",
    "jeudi",
    "vendredi",
    "samedi",
    "dimanche",
    "janvier",
    "fevrier",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "aout",
    "septembre",
    "octobre",
    "novembre",
    "decembre",
    "semaine",
    "semaines",
    "mois",
    "prochain",
    "prochaine",
    "week-end",
    // Spanish
    "manana",
    "lunes",
    "martes",
    "miercoles",
    "jueves",
    "viernes",
    "sabado",
    "domingo",
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "setiembre",
    "octubre",
    "noviembre",
    "diciembre",
    "semana",
    "semanas",
    "mes",
    "meses",
    "proximo",
    "proxima",
    // German
    "morgen",
    "ubermorgen",
    "montag",
    "dienstag",
    "mittwoch",
    "donnerstag",
    "freitag",
    "samstag",
    "sonnabend",
    "sonntag",
    "januar",
    "februar",
    "marz",
    "juni",
    "juli",
    "oktober",
    "dezember",
    "woche",
    "wochen",
    "monat",
    "monate",
    "monaten",
    "nachste",
    "nachsten",
    "nachster",
    "nachstes",
    "wochenende",
    // Italian
    "domani",
    "dopodomani",
    "lunedi",
    "martedi",
    "mercoledi",
    "giovedi",
    "venerdi",
    "sabato",
    "domenica",
    "gennaio",
    "febbraio",
    "aprile",
    "maggio",
    "giugno",
    "luglio",
    "settembre",
    "ottobre",
    "dicembre",
    "settimana",
    "settimane",
    "mese",
    "mesi",
    "prossimo",
    "prossima",
    // Portuguese
    "amanha",
    "segunda",
    "terca",
    "quarta",
    "quinta",
    "sexta",
    "janeiro",
    "fevereiro",
    "marco",
    "maio",
    "junho",
    "julho",
    "setembro",
    "outubro",
    "novembro",
    "dezembro",
];

/// Each day of the week by its names, folded. Portuguese counts its days,
/// segunda to sexta, so those words are days only once the model took them
/// for one.
const WEEKDAYS: &[(Weekday, &[&str])] = &[
    (
        Weekday::Mon,
        &[
            "monday", "mon", "lundi", "lunes", "montag", "lunedi", "segunda",
        ],
    ),
    (
        Weekday::Tue,
        &[
            "tuesday", "tues", "tue", "mardi", "martes", "dienstag", "martedi", "terca",
        ],
    ),
    (
        Weekday::Wed,
        &[
            "wednesday",
            "wed",
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
            "thu",
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
            "friday", "fri", "vendredi", "viernes", "freitag", "venerdi", "sexta",
        ],
    ),
    (
        Weekday::Sat,
        &[
            "saturday",
            "sat",
            "samedi",
            "sabado",
            "samstag",
            "sonnabend",
            "sabato",
        ],
    ),
    (
        Weekday::Sun,
        &[
            "sunday", "sun", "dimanche", "domingo", "sonntag", "domenica",
        ],
    ),
];

/// A week, and what makes it the next one: "next week", "semaine prochaine",
/// "semana que viene", "nächste Woche", "settimana prossima", "semana que
/// vem". Both in the words put a weekday in the week after the note's.
const WEEK_WORDS: &[&str] = &["week", "semaine", "semana", "woche", "settimana"];
const NEXT_WORDS: &[&str] = &[
    "next",
    "prochaine",
    "prochain",
    "proxima",
    "proximo",
    "viene",
    "vem",
    "nachste",
    "nachsten",
    "prossima",
    "prossimo",
];

/// Tomorrow, which the words must say for the model to be taken at its word.
const TOMORROW_WORDS: &[&str] = &[
    "tomorrow", "tmrw", "tmr", "demain", "manana", "morgen", "domani", "amanha",
];

/// Days, weeks and months as a count's unit, which the words must hold for
/// "in two weeks" or "next month" to be read so: "this weekend" is not in
/// two weeks.
const DAY_UNIT_WORDS: &[&str] = &[
    "day", "days", "jour", "jours", "dia", "dias", "tag", "tage", "tagen", "giorno", "giorni",
];
const WEEK_UNIT_WORDS: &[&str] = &[
    "week",
    "weeks",
    "fortnight",
    "semaine",
    "semaines",
    "semana",
    "semanas",
    "woche",
    "wochen",
    "settimana",
    "settimane",
];
const MONTH_UNIT_WORDS: &[&str] = &[
    "month", "months", "mois", "mes", "meses", "monat", "monate", "monaten", "mese", "mesi",
];

/// Each month by its names, folded, from January.
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

/// The day after tomorrow, which the model takes for tomorrow: each phrase
/// as the words it is made of.
const DAY_AFTER_TOMORROW: &[&[&str]] = &[
    &["day", "after", "tomorrow"],
    &["apres", "demain"],
    &["pasado", "manana"],
    &["ubermorgen"],
    &["dopodomani"],
    &["depois", "amanha"],
];

/// Counted in days only after a number: "in 3 days", "dans 15 jours".
const DAY_UNITS: &[&str] = &[
    "day", "days", "jour", "jours", "dia", "dias", "tag", "tage", "tagen", "giorno", "giorni",
];

/// Whether the text names a day in a way the model may read: a word for a
/// day, week or month, a number of days, or a date written in figures such
/// as `12/03` or `2026-10-06`.
pub fn mentions_a_day(text: &str) -> bool {
    let folded = fold(text);
    let words: Vec<&str> = folded
        .split(|c: char| !(c.is_alphanumeric() || c == '-'))
        .flat_map(|word| {
            // "apres-demain" whole, and its parts, as other editors split it.
            std::iter::once(word).chain(word.split('-'))
        })
        .filter(|word| !word.is_empty())
        .collect();
    let numbered = words
        .windows(2)
        .any(|pair| pair[0].chars().all(|c| c.is_ascii_digit()) && DAY_UNITS.contains(&pair[1]));
    numbered || words.iter().any(|word| DAY_WORDS.contains(word)) || has_figure_date(&folded)
}

/// `12/03`, `12.03`, `12.03.2026`, `2026-10-06` and the like: numbers of
/// one or two figures, or a year first, joined by `/`, `.` or `-`. With a
/// dot, `1.2` and `3.50` are figures, not days: only a day and a month that
/// could be one, or three numbers, count.
fn has_figure_date(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    // The number starting at `from`, and how many figures it has.
    let number = |from: usize| {
        let len = chars[from..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        let value: String = chars[from..from + len].iter().collect();
        (value.parse::<u32>().unwrap_or(0), len)
    };
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() || (i > 0 && chars[i - 1].is_ascii_digit()) {
            i += 1;
            continue;
        }
        let (first, first_len) = number(i);
        let sep = i + first_len;
        let joined = sep + 1 < chars.len()
            && matches!(chars[sep], '/' | '.' | '-')
            && chars[sep + 1].is_ascii_digit();
        if joined && (first_len <= 2 || first_len == 4) {
            let (second, second_len) = number(sep + 1);
            let after = sep + 1 + second_len;
            let third = after + 1 < chars.len()
                && chars[after] == chars[sep]
                && chars[after + 1].is_ascii_digit();
            let found = match chars[sep] {
                '.' => {
                    third
                        || (first_len <= 2
                            && second_len == 2
                            && (1..=31).contains(&first)
                            && (1..=12).contains(&second))
                }
                _ => (1..=2).contains(&second_len),
            };
            if found {
                return true;
            }
        }
        i += first_len.max(1);
    }
    false
}

/// What the model read: the note's words for the day, and the kind of day
/// they name, with what that kind needs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Said {
    pub words: String,
    pub kind: Kind,
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(default)]
    pub unit: Option<Unit>,
    #[serde(default)]
    pub weekday: Option<Day>,
    #[serde(default)]
    pub month: Option<u32>,
    #[serde(default)]
    pub day: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    None,
    Past,
    Today,
    Tomorrow,
    In,
    Weekday,
    Next,
    Date,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Day,
    Week,
    Month,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Day {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Day {
    fn weekday(self) -> Weekday {
        match self {
            Day::Mon => Weekday::Mon,
            Day::Tue => Weekday::Tue,
            Day::Wed => Weekday::Wed,
            Day::Thu => Weekday::Thu,
            Day::Fri => Weekday::Fri,
            Day::Sat => Weekday::Sat,
            Day::Sun => Weekday::Sun,
        }
    }
}

/// The grammar the answer is sampled under (SPEC 5.3 for labels): the words
/// first, so the model reads the day off the note before it says what kind
/// it is, then the kind with what it needs, in that order.
pub fn grammar() -> String {
    format!(
        r#"root ::= "{{" ws "\"words\":" ws words ws "," ws "\"kind\":" ws kind ws "}}"
kind ::= "\"none\"" | "\"past\"" | "\"today\"" | "\"tomorrow\"" | in | weekday | next | date
in ::= "\"in\"" ws "," ws "\"count\":" ws count ws "," ws "\"unit\":" ws unit
weekday ::= "\"weekday\"" ws "," ws "\"weekday\":" ws day
next ::= "\"next\"" ws "," ws "\"unit\":" ws period
date ::= "\"date\"" ws "," ws "\"month\":" ws month ws "," ws "\"day\":" ws dom
count ::= [1-9] [0-9]?
unit ::= "\"day\"" | "\"week\"" | "\"month\""
period ::= "\"week\"" | "\"month\""
day ::= "\"mon\"" | "\"tue\"" | "\"wed\"" | "\"thu\"" | "\"fri\"" | "\"sat\"" | "\"sun\""
month ::= [1-9] | "1" [0-2]
dom ::= [1-9] | [12] [0-9] | "3" [01]
words ::= "\"" char{{0,{MAX_WORDS}}} "\""
char ::= [^"\\] | "\\" (["\\/bfnrt] | "u" [0-9a-fA-F]{{4}})
ws ::= " "?
"#
    )
}

const SYSTEM: &str = "\
You read a short personal note and find the later day it looks forward to,
if it names one: an appointment, a deadline, a plan, something to do or to
remember on a day after the one it was written. Return JSON only.
- words: the words of the note that name that day, copied exactly as the
  note writes them. Empty when it names none.
- kind: what those words say.
  - \"tomorrow\": the day after the note was written.
  - \"in\": a number of days, weeks or months from then; give count and unit.
  - \"weekday\": a day of the week; give weekday.
  - \"next\": next week or next month, with no day of it named; give unit.
  - \"date\": a day of a month, such as the 12th of March; give month and day.
  - \"today\": the day the note was written.
  - \"past\": a day before the note was written.
  - \"none\": no day, or none the note looks forward to.";

/// The prompt for one note, written on `written`.
pub fn build(body: &str, written: NaiveDate) -> String {
    let user = format!(
        "WRITTEN ON: {} {}\n\nNOTE:\n{}",
        written.format("%A"),
        written.format("%Y-%m-%d"),
        prompt::truncate(body)
    );
    prompt::chat(SYSTEM, &user)
}

/// The grammar rules out anything but the object; this also covers a
/// backend that ignores it, as the stub in tests does.
pub fn parse(raw: &str) -> Result<Said, String> {
    serde_json::from_str(raw.trim()).map_err(|e| {
        format!(
            "not the expected JSON ({e}), the model returned {}",
            super::excerpt(raw)
        )
    })
}

/// The day `said` names, counted from `written`, the note's own day. `None`
/// for no day, today or a past one, for words that are not the note's, and
/// for a day that does not exist or lies too far ahead. Where the words name
/// a weekday or the week after, they are taken over the model's reading.
pub fn resolve(said: &Said, body: &str, written: NaiveDate) -> Option<NaiveDate> {
    let words = fold(said.words.trim());
    if words.is_empty() || !fold(body).contains(&words) {
        return None;
    }
    let said_words: Vec<&str> = words
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    let has = |word: &str| said_words.contains(&word);
    let named = WEEKDAYS
        .iter()
        .find(|(_, names)| names.iter().any(|name| has(name)))
        .map(|(day, _)| *day);
    // Words that name a weekday name that day, though the model may take
    // "next Tuesday" for next week, "Friday" for tomorrow or for a count of
    // days. "Next month" and "next week" are the start of it, though the
    // model may count one month or week from the note.
    let next = NEXT_WORDS.iter().any(|word| has(word));
    let kind = match (said.kind, said.unit, named) {
        (Kind::Next | Kind::Tomorrow | Kind::In, _, Some(_)) => Kind::Weekday,
        (Kind::In, Some(unit), None) if said.count == Some(1) && next && holds_unit(&has, unit) => {
            Kind::Next
        }
        (kind, ..) => kind,
    };
    let day = match kind {
        Kind::None | Kind::Past | Kind::Today => return None,
        _ if DAY_AFTER_TOMORROW
            .iter()
            .any(|phrase| phrase.iter().all(|word| has(word))) =>
        {
            written.checked_add_days(Days::new(2))?
        }
        // "End of the week" is not tomorrow.
        Kind::Tomorrow if !TOMORROW_WORDS.iter().any(|word| has(word)) => return None,
        Kind::Tomorrow => written.checked_add_days(Days::new(1))?,
        Kind::In => {
            let count = said.count.filter(|count| *count > 0)?;
            let unit = said.unit.filter(|unit| holds_unit(&has, *unit))?;
            match unit {
                Unit::Day => written.checked_add_days(Days::new(count.into()))?,
                Unit::Week => written.checked_add_days(Days::new(u64::from(count) * 7))?,
                Unit::Month => written.checked_add_months(Months::new(count))?,
            }
        }
        Kind::Weekday => {
            let wanted = named.or(said.weekday.map(Day::weekday))?;
            let following =
                WEEK_WORDS.iter().any(|word| has(word)) && NEXT_WORDS.iter().any(|word| has(word));
            if following {
                // That day of the week after the note's.
                monday_after(written)?
                    .checked_add_days(Days::new(wanted.num_days_from_monday().into()))?
            } else {
                // The first such day after the note's, a week on at most:
                // "Tuesday", "next Tuesday", "mardi prochain" alike.
                let ahead = (7 + wanted.num_days_from_monday()
                    - written.weekday().num_days_from_monday())
                    % 7;
                written.checked_add_days(Days::new(if ahead == 0 { 7 } else { ahead.into() }))?
            }
        }
        Kind::Next => match said.unit.filter(|unit| holds_unit(&has, *unit))? {
            Unit::Week => monday_after(written)?,
            Unit::Month => written.with_day(1)?.checked_add_months(Months::new(1))?,
            Unit::Day => written.checked_add_days(Days::new(1))?,
        },
        Kind::Date => {
            // The number the words hold is the day, when they hold one.
            let numbers: Vec<u32> = said_words
                .iter()
                .filter_map(|word| number_in(word))
                .collect();
            let day = match numbers.as_slice() {
                [day] => *day,
                _ => said.day?,
            };
            let named = MONTHS
                .iter()
                .position(|names| names.iter().any(|name| has(name)))
                .map(|month| month as u32 + 1);
            // Figures say the month too, which the model read off them.
            let month = named.or_else(|| has_figure_date(&words).then_some(said.month).flatten());
            match month {
                Some(month) => {
                    let this_year = NaiveDate::from_ymd_opt(written.year(), month, day);
                    match this_year {
                        Some(date) if date > written => date,
                        _ => NaiveDate::from_ymd_opt(written.year() + 1, month, day)?,
                    }
                }
                // "By the 5th": the next 5th to come, whatever month the
                // model guessed.
                None => next_day_of_month(written, day)?,
            }
        }
    };
    let ahead = (day - written).num_days();
    (1..=MAX_AHEAD_DAYS).contains(&ahead).then_some(day)
}

/// Whether the words hold a word for `unit`.
fn holds_unit(has: &impl Fn(&str) -> bool, unit: Unit) -> bool {
    let words = match unit {
        Unit::Day => DAY_UNIT_WORDS,
        Unit::Week => WEEK_UNIT_WORDS,
        Unit::Month => MONTH_UNIT_WORDS,
    };
    words.iter().any(|word| has(word))
}

/// A day of the month a word gives: `12`, `12th`, `1st`, `1er`, `3rd`.
fn number_in(word: &str) -> Option<u32> {
    let figures: String = word.chars().take_while(|c| c.is_ascii_digit()).collect();
    let rest = &word[figures.len()..];
    let ordinal = ["", "st", "nd", "rd", "th", "er", "e", "o", "a"].contains(&rest);
    let day: u32 = figures.parse().ok()?;
    (ordinal && (1..=31).contains(&day)).then_some(day)
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

/// Bumped when looking for days ahead changes enough to look at the notes
/// already labelled again.
const LOOKED: u32 = 1;
/// How far back notes already labelled are looked at, once.
pub const LOOK_BACK_DAYS: u64 = 60;

pub fn looked_path(root: &Path) -> PathBuf {
    root.join(".scratchnote").join("days-ahead.json")
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Looked {
    looked: u32,
}

/// Whether the space's notes already labelled were looked at for days ahead.
pub fn looked(root: &Path) -> bool {
    std::fs::read_to_string(looked_path(root))
        .ok()
        .and_then(|raw| serde_json::from_str::<Looked>(&raw).ok())
        .is_some_and(|marker| marker.looked >= LOOKED)
}

/// Record that the space's notes were looked at. Written straight, as
/// `threads.json` is: losing it only looks again.
pub fn mark_looked(root: &Path) -> std::io::Result<()> {
    let json = serde_json::to_string(&Looked { looked: LOOKED }).map_err(std::io::Error::other)?;
    std::fs::write(looked_path(root), json)
}

/// Jobs for the day ahead alone, for the notes and pages of the last
/// `LOOK_BACK_DAYS` labelled before days ahead were looked for, which name a
/// day. A note still pending is left to its own job, which looks too.
pub fn look_back(index: &Index, db: &SearchDb, today: NaiveDate) -> Vec<Job> {
    let since = today
        .checked_sub_days(Days::new(LOOK_BACK_DAYS))
        .unwrap_or(NaiveDate::MIN);
    let recent: Vec<_> = index
        .entries()
        .filter(|entry| entry.on.is_none() && entry.status != Status::Pending)
        .filter(|entry| {
            NaiveDate::parse_from_str(&entry.date, "%Y-%m-%d")
                .is_ok_and(|date| since <= date && date <= today)
        })
        .collect();
    let mut bodies = db
        .bodies(recent.iter().map(|entry| entry.id.as_str()))
        .unwrap_or_default();
    recent
        .into_iter()
        .filter(|entry| {
            let body = bodies.remove(&entry.id).unwrap_or_default();
            // A page is read with its title, as the model labels it.
            match entry.kind {
                NoteKind::Page => mentions_a_day(&format!(
                    "{}\n{body}",
                    entry.subject.as_deref().unwrap_or_default()
                )),
                NoteKind::Note => mentions_a_day(&body),
            }
        })
        .map(|entry| Job::day(entry.id.clone(), entry.date.clone()))
        .collect()
}

/// The later day a note written on `written` looks forward to, asking the
/// model only when the note names a day at all.
pub fn day_ahead(
    body: &str,
    written: NaiveDate,
    backend: &dyn Backend,
) -> Result<Option<NaiveDate>, String> {
    if !mentions_a_day(body) {
        return Ok(None);
    }
    let raw = backend.generate(&build(body, written), &grammar())?;
    let said = parse(&raw)?;
    let day = resolve(&said, body, written);
    log::info!("day ahead: {:?} {:?} -> {day:?}", said.words, said.kind);
    Ok(day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enrich::model::StubBackend;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    /// Wednesday.
    const WRITTEN: &str = "2026-09-23";

    fn said(json: &str) -> Said {
        parse(json).unwrap()
    }

    fn resolved(json: &str, body: &str) -> Option<String> {
        resolve(&said(json), body, date(WRITTEN)).map(|d| d.to_string())
    }

    #[test]
    fn a_note_that_names_no_day_is_not_asked_about() {
        for text in [
            "Buy a new USB-C hub for the homelab",
            "Le déploiement de staging a encore échoué",
            "API latency is 1.2s at p99, 3 retries",
            "Version 2.4 is out",
            "Costs 3.50 per user, up from 2.99",
        ] {
            assert!(!mentions_a_day(text), "{text}");
        }
        for text in [
            "Dentist next Tuesday 3pm",
            "Réunion demain à 14h",
            "Revoir la config la semaine prochaine",
            "rappeler le plombier dans 15 jours",
            "Abgabe am Freitag",
            "Cita el próximo martes",
            "consegna dopodomani",
            "reunião amanhã cedo",
            "Passport expires 12/03",
            "Abgabe am 12.03.",
            "Rendu le 12.03.2026",
            "Release on 2026-10-06",
            "Call back après-demain",
            "in 3 days",
        ] {
            assert!(mentions_a_day(text), "{text}");
        }
    }

    #[test]
    fn days_are_counted_from_the_note_s_own_day() {
        let body = "Dentist tomorrow, then Tuesday, Wednesday, Friday next week, the day after tomorrow, in 2 weeks, next week, next month, 12 March, 30 February, by the 5th";
        let cases = [
            (
                r#"{"words":"tomorrow","kind":"tomorrow"}"#,
                Some("2026-09-24"),
            ),
            (
                r#"{"words":"in 2 weeks","kind":"in","count":2,"unit":"week"}"#,
                Some("2026-10-07"),
            ),
            // Days the words do not speak of.
            (
                r#"{"words":"in 2 weeks","kind":"in","count":3,"unit":"day"}"#,
                None,
            ),
            (
                r#"{"words":"in 2 weeks","kind":"in","count":1,"unit":"week"}"#,
                Some("2026-09-30"),
            ),
            // From a Wednesday, the coming Tuesday, the words over the
            // model's weekday, and a Friday in the week after.
            (
                r#"{"words":"Tuesday","kind":"weekday","weekday":"mon"}"#,
                Some("2026-09-29"),
            ),
            (
                r#"{"words":"Friday next week","kind":"weekday","weekday":"fri"}"#,
                Some("2026-10-02"),
            ),
            (
                r#"{"words":"then","kind":"weekday","weekday":"fri"}"#,
                Some("2026-09-25"),
            ),
            // The same weekday as the note's is a week on.
            (
                r#"{"words":"Wednesday","kind":"weekday","weekday":"wed"}"#,
                Some("2026-09-30"),
            ),
            // The day after tomorrow, whatever the model took it for.
            (
                r#"{"words":"day after tomorrow","kind":"tomorrow"}"#,
                Some("2026-09-25"),
            ),
            // The words must back the kind: "end of the week" is not
            // tomorrow, nor "this weekend" in two weeks.
            (r#"{"words":"then","kind":"tomorrow"}"#, None),
            (
                r#"{"words":"next week","kind":"in","count":2,"unit":"month"}"#,
                None,
            ),
            (
                r#"{"words":"next month","kind":"next","unit":"week"}"#,
                None,
            ),
            // A day of the month without a month is the next one to come,
            // and a month the words name wins over the model's.
            (
                r#"{"words":"by the 5th","kind":"date","month":9,"day":5}"#,
                Some("2026-10-05"),
            ),
            (
                r#"{"words":"by the 5th","kind":"date","month":12,"day":5}"#,
                Some("2026-10-05"),
            ),
            // A weekday named, taken for a month ahead, and next month
            // taken for a month from the note.
            (
                r#"{"words":"Friday","kind":"in","count":1,"unit":"month"}"#,
                Some("2026-09-25"),
            ),
            (
                r#"{"words":"next month","kind":"in","count":1,"unit":"month"}"#,
                Some("2026-10-01"),
            ),
            // A weekday named, taken for next week or for tomorrow.
            (
                r#"{"words":"then Tuesday","kind":"next","unit":"week"}"#,
                Some("2026-09-29"),
            ),
            (
                r#"{"words":"Friday next week","kind":"next","unit":"week"}"#,
                Some("2026-10-02"),
            ),
            (
                r#"{"words":"Wednesday","kind":"tomorrow"}"#,
                Some("2026-09-30"),
            ),
            (
                r#"{"words":"next week","kind":"next","unit":"week"}"#,
                Some("2026-09-28"),
            ),
            (
                r#"{"words":"next month","kind":"next","unit":"month"}"#,
                Some("2026-10-01"),
            ),
            // Before the note's day this year, so next year's.
            (
                r#"{"words":"12 March","kind":"date","month":3,"day":12}"#,
                Some("2027-03-12"),
            ),
            (
                r#"{"words":"12 March","kind":"date","month":10,"day":2}"#,
                Some("2027-03-12"),
            ),
            (
                r#"{"words":"30 February","kind":"date","month":2,"day":30}"#,
                None,
            ),
            (r#"{"words":"tomorrow","kind":"today"}"#, None),
            (r#"{"words":"tomorrow","kind":"past"}"#, None),
            (r#"{"words":"","kind":"none"}"#, None),
        ];
        for (json, wanted) in cases {
            assert_eq!(resolved(json, body).as_deref(), wanted, "{json}");
        }
    }

    #[test]
    fn words_that_are_not_the_note_s_name_no_day() {
        let body = "Réunion demain à 14h";
        assert_eq!(
            resolved(r#"{"words":"DEMAIN","kind":"tomorrow"}"#, body).as_deref(),
            Some("2026-09-24"),
            "case and accents aside"
        );
        assert_eq!(
            resolved(r#"{"words":"tomorrow","kind":"tomorrow"}"#, body),
            None
        );
        assert_eq!(resolved(r#"{"words":"","kind":"tomorrow"}"#, body), None);
    }

    #[test]
    fn a_month_ahead_keeps_to_the_month_s_last_day() {
        let said = said(r#"{"words":"in a month","kind":"in","count":1,"unit":"month"}"#);
        let day = resolve(&said, "in a month", date("2026-01-31")).unwrap();
        assert_eq!(day, date("2026-02-28"));
    }

    #[test]
    fn a_day_too_far_ahead_is_a_misreading() {
        let said = said(r#"{"words":"in 99 months","kind":"in","count":99,"unit":"month"}"#);
        assert_eq!(resolve(&said, "in 99 months", date(WRITTEN)), None);
    }

    #[test]
    fn the_grammar_states_every_rule_it_uses() {
        let grammar = grammar();
        let defined: Vec<&str> = grammar
            .lines()
            .filter_map(|line| line.split_once("::=").map(|(name, _)| name.trim()))
            .collect();
        for line in grammar.lines() {
            let Some((_, body)) = line.split_once("::=") else {
                continue;
            };
            // Rule names are the bare words outside quotes and classes.
            let mut bare = String::new();
            let (mut quoted, mut class) = (false, false);
            let mut chars = body.chars().peekable();
            while let Some(c) = chars.next() {
                match c {
                    '\\' if quoted => {
                        chars.next();
                    }
                    '"' if !class => quoted = !quoted,
                    '[' if !quoted => class = true,
                    ']' if class => class = false,
                    c if !quoted && !class => bare.push(c),
                    _ => {}
                }
            }
            for word in bare.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
                if !word.is_empty() && !word.chars().all(|c| c.is_ascii_digit()) {
                    assert!(defined.contains(&word), "{word} is used but not defined");
                }
            }
        }
    }

    #[test]
    fn the_prompt_says_when_the_note_was_written() {
        let prompt = build("Dentist Tuesday", date(WRITTEN));
        assert!(
            prompt.contains("WRITTEN ON: Wednesday 2026-09-23"),
            "{prompt}"
        );
        assert!(prompt.contains("NOTE:\nDentist Tuesday"), "{prompt}");
    }

    /// Notes in four languages, each written on a day, and the later day it
    /// looks forward to, or none: a past day, today, no day at all. A wrong
    /// day is worse than none, so every day found must be right, and nearly
    /// every one there is must be found. Needs a downloaded model;
    /// `cargo test reads_the_day_ahead -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs a downloaded model"]
    fn reads_the_day_ahead_in_either_language() {
        use crate::enrich::download;
        use crate::enrich::llama::LlamaCpp;
        use crate::enrich::model::model_file;

        let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))
        else {
            return;
        };
        let root = std::path::PathBuf::from(home).join("Scratchnote");
        let Some(variant) = download::installed_variant(&root) else {
            eprintln!("no model installed, skipping");
            return;
        };
        let backend = LlamaCpp::load(&model_file(&root, variant)).expect("the model should load");

        // Written on Wednesday 23 September 2026 unless said.
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
            // Written after the cases above shaped the rules, to check them.
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
        ];

        let (mut right, mut found_of, mut wrong) = (0, 0, Vec::new());
        for (body, written, wanted) in cases {
            let written = date(written);
            let raw = backend
                .generate(&build(body, written), &grammar())
                .expect("should answer");
            let got = resolve(&parse(&raw).expect("valid"), body, written).map(|d| d.to_string());
            eprintln!(
                "{:<5} {got:?} <- {raw} <- {body}",
                if got.as_deref() == *wanted {
                    "ok"
                } else {
                    "MISS"
                }
            );
            if wanted.is_some() {
                found_of += 1;
            }
            match (&got, wanted) {
                (got, wanted) if got.as_deref() == *wanted => {
                    right += usize::from(wanted.is_some())
                }
                (None, Some(_)) => {}
                _ => wrong.push(format!("{body:?}: {got:?} not {wanted:?}")),
            }
        }
        assert!(wrong.is_empty(), "wrong days:\n  {}", wrong.join("\n  "));
        assert!(right * 10 >= found_of * 8, "found {right} of {found_of}");
    }

    #[test]
    fn looking_back_takes_the_recent_labelled_notes_that_name_a_day() {
        use crate::storage::daily_file::{body_hash, Kind as NoteKind, Note};
        use crate::storage::index::IndexEntry;

        let note = |id: &str, date: &str, body: &str, status: Status, on: Option<&str>| Note {
            id: id.to_string(),
            date: date.to_string(),
            time: "09:00".to_string(),
            file: crate::storage::relative_day_path(date),
            subject: None,
            category: None,
            status,
            hash: body_hash(body),
            lang: None,
            on: on.map(str::to_string),
            body: body.to_string(),
            kind: NoteKind::Note,
            missing: false,
        };
        let notes = [
            note(
                "01A",
                "2026-09-20",
                "Dentist on Tuesday",
                Status::Done,
                None,
            ),
            note("01B", "2026-09-21", "Rappeler demain", Status::Manual, None),
            // Long ago, already read, still pending, or naming no day.
            note(
                "01C",
                "2026-06-01",
                "Dentist on Tuesday",
                Status::Done,
                None,
            ),
            note(
                "01D",
                "2026-09-21",
                "Dentist on Tuesday",
                Status::Done,
                Some("2026-09-22"),
            ),
            note(
                "01E",
                "2026-09-22",
                "Dentist on Tuesday",
                Status::Pending,
                None,
            ),
            note(
                "01F",
                "2026-09-22",
                "Bought coffee beans",
                Status::Done,
                None,
            ),
        ];
        let mut index = Index::default();
        let mut db = SearchDb::in_memory().unwrap();
        for note in &notes {
            index.push(IndexEntry::from(note));
            db.add_note(note, None).unwrap();
        }
        let ids: Vec<String> = look_back(&index, &db, date(WRITTEN))
            .into_iter()
            .map(|job| {
                assert_eq!(job.work, crate::enrich::queue::Work::Day);
                job.id
            })
            .collect();
        assert_eq!(ids, ["01A", "01B"]);
    }

    #[test]
    fn a_space_is_looked_back_at_once() {
        let root = std::env::temp_dir().join("scratchnote-days-looked");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
        assert!(!looked(&root));
        mark_looked(&root).unwrap();
        assert!(looked(&root));
        // A marker from before a change to looking is looked past.
        std::fs::write(looked_path(&root), r#"{"looked":0}"#).unwrap();
        assert!(!looked(&root));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_note_naming_no_day_never_reaches_the_model() {
        struct Never;
        impl Backend for Never {
            fn generate(&self, _: &str, _: &str) -> Result<String, String> {
                panic!("asked about a note that names no day");
            }
        }
        assert_eq!(
            day_ahead("Buy coffee beans", date(WRITTEN), &Never),
            Ok(None)
        );
        let stub = StubBackend::new(r#"{"words":"Friday","kind":"weekday","weekday":"fri"}"#);
        assert_eq!(
            day_ahead("Release on Friday", date(WRITTEN), &stub),
            Ok(Some(date("2026-09-25")))
        );
    }
}
