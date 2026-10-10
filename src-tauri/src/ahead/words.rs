//! The words the day ahead is read by, in English, French, Spanish, German,
//! Italian and Portuguese, folded as `search::fold` folds them: lowercase,
//! accents stripped.

use chrono::Weekday;

/// Each day of the week by its names, folded (lowercase, accents stripped).
/// Portuguese counts its days, segunda to sexta, which are days only as
/// `segunda-feira` and the like.
pub(super) const WEEKDAYS: &[(Weekday, &[&str])] = &[
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
pub(super) const COUNTED_DAYS: &[&str] = &["segunda", "terca", "quarta", "quinta", "sexta"];

/// Each month by its names, folded, from January. Only ever read next to a
/// day's number, so `may` and `mars` are safe.
pub(super) const MONTHS: &[&[&str]] = &[
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
pub(super) const TOMORROW: &[&str] = &[
    "tomorrow", "tmrw", "tmr", "demain", "manana", "morgen", "domani", "amanha",
];
pub(super) const MORNING_BEFORE: &[&str] = &[
    "la", "esta", "por", "cada", "una", "toda", "heute", "guten", "am", "jeden", "gestern", "den",
    "einen",
];

/// The day after tomorrow, each phrase as the words it is made of.
pub(super) const DAY_AFTER_TOMORROW: &[&[&str]] = &[
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
pub(super) const PAST: &[&str] = &[
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
pub(super) const NEXT: &[&str] = &[
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
pub(super) const COMING: &[&str] = &["viene", "vem"];

pub(super) const DAY_UNITS: &[&str] = &[
    "day", "days", "jour", "jours", "dia", "dias", "tag", "tage", "tagen", "giorno", "giorni",
];
pub(super) const WEEK_UNITS: &[&str] = &[
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
pub(super) const MONTH_UNITS: &[&str] = &[
    "month", "months", "mois", "mes", "meses", "monat", "monate", "monaten", "mese", "mesi",
];

/// What comes before a count of days, weeks or months ahead: "in", "dans",
/// "en", "tra", "fra", "em". "Daqui a" and "dentro de" take one more word.
pub(super) const IN: &[&str] = &["in", "dans", "en", "tra", "fra", "em"];

/// Numbers in words, for a count ahead.
pub(super) const NUMBERS: &[(&str, u32)] = &[
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
pub(super) const BY: &[&str] = &[
    "by", "before", "until", "till", "due", "on", "avant", "pour", "jusqu", "bis", "am", "entro",
    "prima", "per", "ate", "antes", "hasta", "para",
];
pub(super) const ARTICLES: &[&str] = &[
    "the", "le", "el", "il", "dem", "den", "o", "del", "al", "au", "zum", "dia", "de",
];

/// What joins a day and its month: "12th of October", "12 de marzo".
pub(super) const OF: &[&str] = &["of", "de", "di", "do", "da"];

/// Where a stretch of the note ends: a day it names belongs to its own
/// sentence or clause.
pub(super) const BREAKS: &[char] = &['.', '!', '?', ';', ':', ',', '\n', '(', ')', '[', ']'];
