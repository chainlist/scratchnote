//! Made-up notes about a few things each, over a few months, in English and
//! French, for the tests that run the real embedding model: what recall
//! names and what threads gather can be checked against the thing each note
//! is about. A note whose thing starts with `-` is the only one about it.

use crate::storage::daily_file::{body_hash, Kind, Note};
use crate::storage::relative_day_path;

/// The thing a note is about, its day and time, and its text.
pub const SAMPLES: &[(&str, &str, &str, &str)] = &[
    ("argocd", "2026-09-08", "09:12", "ArgoCD auto-sync broke staging again after the chart bump. Rolled back by hand, need a real fix."),
    ("argocd", "2026-09-09", "14:30", "Looked into the ArgoCD sync failure: the sync wave annotations on the CRDs are in the wrong order, so the operator starts before its CRDs exist."),
    ("argocd", "2026-09-11", "10:05", "Talked with Marc about pinning the Helm chart version in the ArgoCD app until we fix the sync waves."),
    ("argocd", "2026-09-15", "16:40", "PR for the sync-wave order is up. Tested on a throwaway cluster, staging sync is green now."),
    ("argocd", "2026-09-22", "11:20", "Postmortem for the staging outage: auto-sync + unpinned chart. Action items: pin charts, alert on sync failures, document the rollback."),
    ("kitchen", "2026-07-20", "18:02", "Got two quotes for the kitchen renovation: 14k from Duval, 17k from the other one. Duval can start in September."),
    ("kitchen", "2026-08-04", "12:45", "Choisi le carrelage pour la cuisine : grès cérame gris clair, 60x60. Commande passée chez Leroy Merlin."),
    ("kitchen", "2026-09-02", "19:10", "Kitchen demolition done. The plumber found the old pipes behind the sink are lead, he has to replace them first."),
    ("kitchen", "2026-09-10", "08:30", "Le plombier a remplacé les tuyaux de la cuisine. Les travaux reprennent lundi avec le carrelage."),
    ("kitchen", "2026-09-25", "20:15", "Kitchen is finished! Countertop installed, just the backsplash grout left. Final invoice from Duval: 14.6k."),
    ("marathon", "2026-07-05", "07:30", "Signed up for the Lyon marathon on September 28. 12 weeks of training, plan says 4 runs a week."),
    ("marathon", "2026-07-26", "11:00", "Long run 24 km at 5:45/km. Left shin hurts a bit on the downhill parts."),
    ("marathon", "2026-08-09", "17:20", "Shin splints confirmed by the physio. Two weeks of no running, cycling instead, then build back slowly."),
    ("marathon", "2026-09-06", "10:40", "Back to long runs: 30 km without pain. Marathon pace felt fine at 5:20/km."),
    ("marathon", "2026-09-28", "15:05", "Finished the Lyon marathon in 3:52! Wall at km 35 but kept going. Legs destroyed."),
    ("lisbon", "2026-08-15", "21:30", "Booked flights to Lisbon for October 3 to 8, TAP from Lyon, 186€ each."),
    ("lisbon", "2026-08-20", "13:15", "Hôtel à Lisbonne réservé dans l'Alfama, 5 nuits, petit-déjeuner inclus."),
    ("lisbon", "2026-09-19", "22:00", "Lisbon itinerary: Belém on day 2, Sintra day trip on day 3, LX Factory and Time Out Market. Book the Sintra train in advance."),
    ("lisbon", "2026-09-29", "19:45", "Pack for Lisbon: adapter not needed, light jacket, comfortable shoes for the hills."),
    ("hiring", "2026-09-01", "09:50", "Opened the senior backend role. Job description reviewed with Sara, posting goes live tomorrow."),
    ("hiring", "2026-09-12", "15:30", "First interview for the senior backend role: Julien, strong on Go and Kafka, a bit light on system design."),
    ("hiring", "2026-09-17", "16:10", "Second candidate for the backend role, Amira: excellent system design round, wants remote two days a week."),
    ("hiring", "2026-09-24", "11:45", "Debrief with Sara: we make an offer to Amira for the senior backend role. Julien goes to the pool for the mid-level opening."),
    ("rust", "2026-08-10", "22:10", "Rust: finally understood why the borrow checker rejects two mutable references, the aliasing rule makes sense now."),
    ("rust", "2026-08-24", "21:40", "std::mem::take swaps a value out of a &mut and leaves Default in its place, handy to move out of a struct field."),
    ("rust", "2026-09-20", "23:05", "Rust lifetimes: a function returning a reference needs the lifetime tied to one of its inputs, elision covers the simple cases."),
    ("birthday", "2026-09-05", "20:30", "Mom's 60th birthday on September 27. Idea: surprise dinner with the whole family at the Italian place she likes."),
    ("birthday", "2026-09-14", "12:10", "Réservé la table pour l'anniversaire de maman, 12 personnes, samedi 27 à 19h30."),
    ("birthday", "2026-09-27", "23:40", "Mom's birthday dinner was perfect, she cried when everyone sang. The photo album gift was a hit."),
    ("car", "2026-08-28", "08:15", "Car makes a grinding noise when braking, front left. Booked the garage for Tuesday."),
    ("car", "2026-09-01", "17:30", "Garage: front brake pads and discs worn out, 420€ for both sides. Car ready Thursday."),
    ("car", "2026-09-16", "18:20", "Picked up the car, the brakes are quiet now. Also asked them to check the AC next time."),
    ("postgres", "2026-09-03", "09:40", "API latency spikes at 9am: Postgres connection pool exhausted, max 20 connections and the workers hold them too long."),
    ("postgres", "2026-09-10", "14:05", "Raised the pool to 50 and added PgBouncer in transaction mode. The 9am spike is gone in the staging load test."),
    ("postgres", "2026-09-18", "10:30", "Production pool change rolled out. p99 latency down from 1.2s to 180ms."),
    ("severance", "2026-08-20", "23:15", "Started Severance season 2. The opening episode is so tense, Mark's double life is getting messier."),
    ("severance", "2026-09-03", "22:50", "Finished Severance season 2. That finale! Still no idea what the goats are for."),
    ("-hub", "2026-08-12", "19:00", "Buy a new USB-C hub for the homelab, the current one drops the ethernet."),
    ("-shakshuka", "2026-08-30", "11:20", "Recipe to try: shakshuka with feta and smoked paprika."),
    ("-cli", "2026-09-04", "21:00", "Idea for a side project: a CLI that turns a folder of markdown into a static site with backlinks."),
    ("-dentist", "2026-09-13", "10:00", "Dentist appointment moved to October 14 at 9:30."),
    ("-apollo", "2026-09-21", "22:30", "Watched the documentary about the Apollo 13 mission, incredible how they improvised the CO2 scrubber."),
    ("-insurance", "2026-09-23", "13:00", "Renew the car insurance before November 1, compare quotes first."),
    ("-bill", "2026-09-26", "09:10", "Pay the electricity bill, it went up 18% since last year."),
    ("-balcony", "2026-07-15", "18:40", "Planted tomatoes and basil on the balcony."),
];

/// The samples as notes, oldest first, each with the thing it is about.
pub fn notes() -> Vec<(&'static str, Note)> {
    let mut notes: Vec<(&str, Note)> = SAMPLES
        .iter()
        .enumerate()
        .map(|(i, (group, date, time, body))| {
            let note = Note {
                id: format!("01S{i:03}"),
                date: date.to_string(),
                time: time.to_string(),
                file: relative_day_path(date),
                subject: Some(body.chars().take(48).collect()),
                hash: body_hash(body),
                ahead_off: false,
                body: body.to_string(),
                kind: Kind::Note,
                on: None,
                missing: false,
            };
            (*group, note)
        })
        .collect();
    notes.sort_by(|(_, a), (_, b)| (&a.date, &a.time).cmp(&(&b.date, &b.time)));
    notes
}
