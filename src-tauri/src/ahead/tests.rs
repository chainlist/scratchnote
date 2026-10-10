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
