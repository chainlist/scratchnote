use super::*;
use crate::storage::space_db::SpaceDb;

/// Every note leans on the first axis, as real notes share a lot; the
/// axis a note names after that is what it is about.
fn vector(about: usize, dims: usize) -> Vec<f32> {
    let mut v = vec![0.0; dims];
    v[0] = 1.0;
    v[about] = 1.0;
    v
}

/// Notes as `(id, date, the axis it is about)`, spread over enough other
/// axes that a mean says something.
fn space(notes: &[(&str, &str, usize)]) -> (Vectors, HashMap<String, When>) {
    let mut vectors = Vectors::new("m", 16);
    let mut when = HashMap::new();
    for (id, date, about) in notes {
        vectors
            .insert(id.to_string(), format!("h{id}"), vector(*about, 16))
            .unwrap();
        let written = When {
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            time: "09:00".to_string(),
        };
        when.insert(id.to_string(), written);
    }
    (vectors, when)
}

/// Unrelated notes, one per axis, to say what is usual.
const OTHERS: [(&str, &str, usize); 10] = [
    ("01Z1", "2026-09-01", 6),
    ("01Z2", "2026-09-02", 7),
    ("01Z3", "2026-09-03", 8),
    ("01Z4", "2026-09-04", 9),
    ("01Z5", "2026-09-05", 10),
    ("01Z6", "2026-09-06", 11),
    ("01Z7", "2026-09-07", 12),
    ("01Z8", "2026-09-08", 13),
    ("01Z9", "2026-09-09", 14),
    ("01ZA", "2026-09-10", 15),
];

fn with_others(
    notes: &[(&'static str, &'static str, usize)],
) -> Vec<(&'static str, &'static str, usize)> {
    notes.iter().chain(OTHERS.iter()).copied().collect()
}

fn titled(thread: &str, title: &str) -> Edits {
    Edits {
        titles: [(thread.to_string(), title.to_string())].into(),
        ..Edits::default()
    }
}

fn threads_of(threads: &Threads, when: &HashMap<String, When>) -> Vec<Vec<String>> {
    threads
        .list(when, &Edits::default())
        .into_iter()
        .map(|thread| thread.notes)
        .collect()
}

#[test]
fn notes_about_one_thing_make_a_thread_and_the_rest_stay_on_their_own() {
    let notes = with_others(&[
        ("01A", "2026-09-10", 1),
        ("01B", "2026-09-12", 1),
        ("01C", "2026-09-20", 1),
        ("01D", "2026-09-15", 2),
    ]);
    let (vectors, when) = space(&notes);
    let mut threads = Threads::default();
    assert!(threads.reconcile(&vectors, &when, &Edits::default()));
    assert_eq!(threads_of(&threads, &when), vec![vec!["01A", "01B", "01C"]]);
    // Named after the note it started with.
    assert_eq!(threads.of("01C"), Some("01A"));
    assert_eq!(threads.of("01D"), None);

    // Nothing new, nothing changes.
    assert!(!threads.reconcile(&vectors, &when, &Edits::default()));
}

#[test]
fn a_note_long_after_a_thread_starts_its_own() {
    let notes = with_others(&[
        ("01A", "2026-01-10", 1),
        ("01B", "2026-01-12", 1),
        // Well past the window after the thread's last note.
        ("01C", "2026-06-01", 1),
        ("01D", "2026-06-03", 1),
    ]);
    let (vectors, when) = space(&notes);
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &Edits::default());
    assert_eq!(
        threads_of(&threads, &when),
        vec![vec!["01A", "01B"], vec!["01C", "01D"]]
    );
}

#[test]
fn a_note_is_placed_again_once_edited_and_a_thread_of_one_is_none() {
    let notes = with_others(&[
        ("01A", "2026-09-10", 1),
        ("01B", "2026-09-12", 1),
        ("01C", "2026-09-14", 1),
    ]);
    let (mut vectors, when) = space(&notes);
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &Edits::default());

    // 01C now says something else.
    vectors
        .insert("01C".into(), "edited".into(), vector(3, 16))
        .unwrap();
    assert!(threads.reconcile(&vectors, &when, &Edits::default()));
    assert_eq!(threads_of(&threads, &when), vec![vec!["01A", "01B"]]);
    assert_eq!(threads.of("01C"), None);

    // 01B deleted: 01A is left on its own.
    let present: HashSet<String> = notes
        .iter()
        .map(|(id, ..)| id.to_string())
        .filter(|id| id != "01B")
        .collect();
    vectors.retain(&present);
    assert!(threads.reconcile(&vectors, &when, &Edits::default()));
    assert!(threads_of(&threads, &when).is_empty());
    assert_eq!(threads.of("01A"), None);
}

#[test]
fn a_note_taken_out_stays_out_and_starts_nothing() {
    let notes = with_others(&[
        ("01A", "2026-09-10", 1),
        ("01B", "2026-09-12", 1),
        ("01C", "2026-09-14", 1),
    ]);
    let (vectors, when) = space(&notes);
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &Edits::default());

    let alone = Edits {
        alone: ["01B".to_string()].into(),
        ..Edits::default()
    };
    assert!(threads.reconcile(&vectors, &when, &alone));
    assert_eq!(threads_of(&threads, &when), vec![vec!["01A", "01C"]]);

    // Taken out with the thread down to two, the other is left alone too.
    let alone = Edits {
        alone: ["01B".to_string(), "01C".to_string()].into(),
        ..Edits::default()
    };
    threads.reconcile(&vectors, &when, &alone);
    assert!(threads_of(&threads, &when).is_empty());

    // Let back in, they gather again.
    assert!(threads.reconcile(&vectors, &when, &Edits::default()));
    assert_eq!(threads_of(&threads, &when), vec![vec!["01A", "01B", "01C"]]);
}

#[test]
fn another_model_places_every_note_again_and_the_placements_round_trip() {
    let mut db = SpaceDb::in_memory().unwrap();
    let notes = with_others(&[("01A", "2026-09-10", 1), ("01B", "2026-09-12", 1)]);
    let (vectors, when) = space(&notes);
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &Edits::default());
    threads.save(&mut db).unwrap();
    assert_eq!(Threads::load(&db), threads);

    let mut other = Vectors::new("other", 16);
    for (id, _, about) in &notes {
        other
            .insert(id.to_string(), format!("h{id}"), vector(*about, 16))
            .unwrap();
    }
    let mut loaded = Threads::load(&db);
    assert!(loaded.reconcile(&other, &when, &Edits::default()));
    assert_eq!(loaded.model, "other");
    assert_eq!(threads_of(&loaded, &when), vec![vec!["01A", "01B"]]);
    loaded.save(&mut db).unwrap();
    assert_eq!(Threads::load(&db), loaded);

    // A broken file is no threads at all.
    let path = std::env::temp_dir().join("scratchnote-threads-broken.json");
    std::fs::write(&path, "not json").unwrap();
    assert_eq!(Threads::read_json(&path), None);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_save_writes_only_the_notes_placed_since() {
    let mut db = SpaceDb::in_memory().unwrap();
    let notes = with_others(&[
        ("01A", "2026-09-10", 1),
        ("01B", "2026-09-12", 1),
        ("01C", "2026-09-14", 1),
    ]);
    let (mut vectors, mut when) = space(&notes);
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &Edits::default());
    threads.save(&mut db).unwrap();
    let rows = |db: &SpaceDb| db.conn().total_changes();
    let before = rows(&db);

    // One note joins the thread, one goes.
    vectors
        .insert("01Z".into(), "h01Z".into(), vector(1, 16))
        .unwrap();
    let written = When {
        date: NaiveDate::from_ymd_opt(2026, 9, 15).unwrap(),
        time: "09:00".into(),
    };
    when.insert("01Z".into(), written);
    let present = vectors
        .iter()
        .map(|(id, _, _)| id.to_string())
        .filter(|id| id != "01C")
        .collect();
    vectors.retain(&present);
    when.remove("01C");
    assert!(threads.reconcile(&vectors, &when, &Edits::default()));
    threads.save(&mut db).unwrap();
    assert_eq!(rows(&db), before + 2);
    assert_eq!(Threads::load(&db), threads);
    assert_eq!(threads_of(&threads, &when), vec![vec!["01A", "01B", "01Z"]]);
}

#[test]
fn a_thread_has_no_title_until_the_user_gives_it_one() {
    let notes = with_others(&[
        ("01A", "2026-09-10", 1),
        ("01B", "2026-09-12", 1),
        ("01C", "2026-09-14", 1),
    ]);
    let (vectors, when) = space(&notes);
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &Edits::default());

    let listed = threads.list(&when, &Edits::default());
    assert_eq!(listed.len(), 1);
    assert_eq!((listed[0].title.as_deref(), listed[0].named), (None, false));
    assert!(!listed[0].kept, "only suggested");

    assert_eq!(threads.list(&when, &titled("01A", "  "))[0].title, None);
    let named = threads.list(&when, &titled("01A", "Kitchen"));
    assert_eq!(named[0].title.as_deref(), Some("Kitchen"));
    assert!(named[0].named);
    assert!(named[0].kept, "a titled thread is the user's");
}

#[test]
fn one_thread_is_got_as_it_is_listed() {
    let notes = with_others(&[
        ("01A", "2026-09-10", 1),
        ("01B", "2026-09-12", 1),
        ("01C", "2026-09-14", 2),
        ("01D", "2026-09-16", 2),
    ]);
    let (vectors, when) = space(&notes);
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &Edits::default());
    let edits = titled("01C", "Mine");

    let listed = threads.list(&when, &edits);
    assert_eq!(listed.len(), 2);
    for thread in &listed {
        let got = threads.get(&thread.id, &when, &edits);
        assert_eq!(got.as_ref(), Some(thread));
    }
    // A note on its own is no thread.
    assert_eq!(threads.get("01Z1", &when, &edits), None);
}

#[test]
fn a_note_put_in_a_thread_stays_there_and_makes_it_the_users() {
    let notes = with_others(&[
        ("01A", "2026-09-10", 1),
        ("01B", "2026-09-12", 1),
        ("01C", "2026-09-14", 2),
        ("01D", "2026-09-15", 3),
        ("01E", "2026-09-16", 4),
    ]);
    let (vectors, when) = space(&notes);
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &Edits::default());
    assert_eq!(threads.of("01C"), None);

    // Into a thread, and two notes into a new one named after the first.
    let edits = Edits {
        pinned: [
            ("01C".to_string(), "01A".to_string()),
            ("01D".to_string(), "01D".to_string()),
            ("01E".to_string(), "01D".to_string()),
        ]
        .into(),
        ..Edits::default()
    };
    assert!(threads.reconcile(&vectors, &when, &edits));
    let listed = threads.list(&when, &edits);
    let notes: Vec<&Vec<String>> = listed.iter().map(|thread| &thread.notes).collect();
    assert_eq!(notes, [&vec!["01A", "01B", "01C"], &vec!["01D", "01E"]]);
    assert!(listed.iter().all(|thread| thread.kept));

    // Edited, it stays where it was put.
    let mut vectors = vectors;
    vectors
        .insert("01C".into(), "edited".into(), vector(5, 16))
        .unwrap();
    threads.reconcile(&vectors, &when, &edits);
    assert_eq!(threads.of("01C"), Some("01A"));
}

#[test]
fn a_dismissed_suggestion_comes_back_once_it_grows() {
    let edits = Edits {
        dismissed: [(
            "01A".to_string(),
            ["01A".to_string(), "01B".to_string()].into(),
        )]
        .into(),
        ..Edits::default()
    };
    let two = with_others(&[("01A", "2026-09-10", 1), ("01B", "2026-09-12", 1)]);
    let (vectors, when) = space(&two);
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &edits);
    assert!(threads.list(&when, &edits).is_empty());
    assert!(threads.get("01A", &when, &edits).is_some(), "still got");

    let mut three = two.clone();
    three.push(("01C", "2026-09-14", 1));
    let (vectors, when) = space(&three);
    threads.reconcile(&vectors, &when, &edits);
    assert_eq!(threads.list(&when, &edits).len(), 1);
}

#[test]
fn the_threads_a_note_could_go_in_are_ranked_by_fit() {
    let notes = with_others(&[
        ("01A", "2026-01-10", 1),
        ("01B", "2026-01-12", 1),
        ("01C", "2026-09-14", 2),
        ("01D", "2026-09-15", 2),
        // Far from both in time, closer to the first in meaning.
        ("01E", "2027-06-01", 1),
    ]);
    let (vectors, when) = space(&notes);
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &Edits::default());
    assert_eq!(threads.ranked_for("01E", &vectors), ["01A", "01C"]);
    assert_eq!(threads.ranked_for("01A", &vectors), ["01C"], "not its own");
}

#[test]
fn two_notes_wait_for_a_third_to_say_what_is_usual() {
    let two = [("01A", "2026-09-10", 1), ("01B", "2026-09-12", 1)];
    let (vectors, when) = space(&two);
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &Edits::default());
    assert!(threads.scopes.is_empty(), "nothing placed yet");

    let (vectors, when) = space(&[two[0], two[1], ("01Z1", "2026-09-01", 9)]);
    assert!(threads.reconcile(&vectors, &when, &Edits::default()));
    assert_eq!(threads_of(&threads, &when), vec![vec!["01A", "01B"]]);
}

/// Five notes about one thing make the mean mostly that thing; with it
/// taken off, two notes about anything else share not being about it.
#[test]
fn a_space_mostly_about_one_thing_does_not_tie_the_rest_together() {
    let notes = [
        ("01A", "2026-09-10", 1),
        ("01B", "2026-09-11", 1),
        ("01C", "2026-09-12", 1),
        ("01D", "2026-09-13", 1),
        ("01E", "2026-09-14", 1),
        // An apple, then Angular.
        ("01P", "2026-09-15", 2),
        ("01Q", "2026-09-16", 3),
    ];
    let (vectors, when) = space(&notes);

    let mut mean_only = Threads::default();
    let cuts = Cuts {
        lead: f32::NEG_INFINITY,
        ..CUTS
    };
    mean_only.reconcile_with(&vectors, &when, &Edits::default(), &Mentioned::new(), cuts);
    assert!(mean_only.of("01P").is_some(), "the old mistake");

    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &Edits::default());
    assert_eq!(
        threads_of(&threads, &when),
        vec![vec!["01A", "01B", "01C", "01D", "01E"]]
    );
}

/// Notes as `(id, date, the axis it is about)`.
type Notes = Vec<(&'static str, &'static str, usize)>;

/// Two threads of one thing, apart by more than the window, until a
/// note between them draws one towards the other.
fn bridged() -> (Notes, Notes) {
    let apart = with_others(&[
        ("01A", "2026-01-01", 1),
        ("01B", "2026-01-02", 1),
        ("01C", "2026-04-01", 1),
        ("01D", "2026-04-02", 1),
    ]);
    let mut between = apart.clone();
    between.push(("01E", "2026-02-15", 1));
    (apart, between)
}

#[test]
fn threads_that_grow_towards_each_other_become_one() {
    let (apart, between) = bridged();
    let (vectors, when) = space(&apart);
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &Edits::default());
    assert_eq!(
        threads_of(&threads, &when),
        vec![vec!["01A", "01B"], vec!["01C", "01D"]]
    );

    let (vectors, when) = space(&between);
    assert!(threads.reconcile(&vectors, &when, &Edits::default()));
    assert_eq!(
        threads_of(&threads, &when),
        vec![vec!["01A", "01B", "01E", "01C", "01D"]]
    );
    assert_eq!(threads.of("01D"), Some("01A"), "the older one goes on");
}

#[test]
fn a_titled_thread_keeps_its_name_and_two_never_become_one() {
    let (apart, between) = bridged();
    let titled = |threads: &[&str]| Edits {
        titles: threads
            .iter()
            .map(|thread| (thread.to_string(), "Mine".to_string()))
            .collect(),
        ..Edits::default()
    };

    let one = titled(&["01C"]);
    let (vectors, when) = space(&apart);
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &one);
    let (vectors, when) = space(&between);
    threads.reconcile(&vectors, &when, &one);
    assert_eq!(threads_of(&threads, &when).len(), 1);
    assert_eq!(threads.of("01A"), Some("01C"));

    let both = titled(&["01A", "01C"]);
    let (vectors, when) = space(&apart);
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &both);
    let (vectors, when) = space(&between);
    threads.reconcile(&vectors, &when, &both);
    assert_eq!(threads_of(&threads, &when).len(), 2);
}

#[test]
fn a_note_without_a_day_waits_for_a_later_pass() {
    let notes = with_others(&[("01A", "2026-09-10", 1), ("01B", "2026-09-12", 1)]);
    let (vectors, mut when) = space(&notes);
    when.remove("01B");
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &Edits::default());
    assert!(threads_of(&threads, &when).is_empty());

    let (_, when) = space(&notes);
    assert!(threads.reconcile(&vectors, &when, &Edits::default()));
    assert_eq!(threads_of(&threads, &when), vec![vec!["01A", "01B"]]);
}

#[test]
fn edits_round_trip_and_load_empty_when_none_are_saved() {
    let mut db = SpaceDb::in_memory().unwrap();
    assert_eq!(Edits::load(&db), Edits::default());

    let mut edits = Edits::default();
    edits.titles.insert("01A".into(), "Kitchen".into());
    edits.alone.insert("01D".into());
    edits.pinned.insert("01B".into(), "01A".into());
    edits
        .dismissed
        .insert("01E".into(), ["01E".to_string(), "01F".to_string()].into());
    edits.save(&mut db).unwrap();
    assert_eq!(Edits::load(&db), edits);

    // A save replaces what was saved.
    edits.titles.clear();
    edits.dismissed.clear();
    edits.save(&mut db).unwrap();
    assert_eq!(Edits::load(&db), edits);
}

/// A note as `(id, date, the axes it is about)`, each axis weighed.
type Weighed<'a> = (&'a str, &'a str, &'a [(usize, f32)]);

/// Notes with each axis weighed as given, over the first axis every note
/// leans on.
fn weighed(notes: &[Weighed]) -> (Vectors, HashMap<String, When>) {
    let mut vectors = Vectors::new("m", 16);
    let mut when = HashMap::new();
    for (id, date, axes) in notes {
        let mut v = vec![0.0; 16];
        v[0] = 1.0;
        for (axis, weight) in *axes {
            v[*axis] += weight;
        }
        vectors.insert(id.to_string(), format!("h{id}"), v).unwrap();
        let written = When {
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            time: "09:00".to_string(),
        };
        when.insert(id.to_string(), written);
    }
    (vectors, when)
}

/// Six notes of one project, axis 3, three about its 404s, axis 1, and
/// three about its performance, axis 2, among unrelated others.
fn project() -> (Vectors, HashMap<String, When>, Mentioned) {
    let mut notes: Vec<Weighed> = vec![
        ("01A", "2026-09-10", &[(3, 1.0), (1, 1.0)]),
        ("01B", "2026-09-11", &[(3, 1.0), (1, 1.0)]),
        ("01C", "2026-09-12", &[(3, 1.0), (1, 1.0)]),
        ("01D", "2026-09-13", &[(3, 1.0), (2, 1.0)]),
        ("01E", "2026-09-14", &[(3, 1.0), (2, 1.0)]),
        ("01F", "2026-09-15", &[(3, 1.0), (2, 1.0)]),
    ];
    const OTHER: [&[(usize, f32)]; 8] = [
        &[(6, 1.0)],
        &[(7, 1.0)],
        &[(8, 1.0)],
        &[(9, 1.0)],
        &[(10, 1.0)],
        &[(11, 1.0)],
        &[(12, 1.0)],
        &[(13, 1.0)],
    ];
    let others = [
        "01Z1", "01Z2", "01Z3", "01Z4", "01Z5", "01Z6", "01Z7", "01Z8",
    ];
    for (id, axes) in others.iter().zip(OTHER) {
        notes.push((id, "2026-09-05", axes));
    }
    let (vectors, when) = weighed(&notes);
    let mentioned: Mentioned = ["01A", "01B", "01C", "01D", "01E", "01F"]
        .iter()
        .map(|id| (id.to_string(), vec!["atlas".to_string()]))
        .collect();
    (vectors, when, mentioned)
}

#[test]
fn a_projects_subjects_part_among_its_own_notes() {
    let (vectors, when, mentioned) = project();

    // Across the space, what the project's notes share ties them into one.
    let mut whole = Threads::default();
    whole.reconcile(&vectors, &when, &Edits::default());
    assert_eq!(
        threads_of(&whole, &when),
        vec![vec!["01A", "01B", "01C", "01D", "01E", "01F"]]
    );

    // Among its own notes, that is taken off, and its subjects part.
    let mut threads = Threads::default();
    assert!(threads.reconcile_mentioned(&vectors, &when, &Edits::default(), &mentioned));
    assert_eq!(
        threads_of(&threads, &when),
        vec![vec!["01A", "01B", "01C"], vec!["01D", "01E", "01F"]]
    );
    let listed = threads.list(&when, &Edits::default());
    assert_eq!(listed[0].id, "@atlas:01A");
    assert_eq!(listed[0].scope.as_deref(), Some("atlas"));
    assert_eq!(threads.in_scope("atlas", "01F"), Some("@atlas:01D"));
    assert_eq!(
        threads.in_scope(GENERAL, "01A"),
        None,
        "not in the general scope"
    );
    assert!(!threads.reconcile_mentioned(&vectors, &when, &Edits::default(), &mentioned));
}

#[test]
fn a_draft_close_to_a_names_notes_is_given_that_name() {
    let (vectors, _, mentioned) = project();
    let mut names: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, keys) in &mentioned {
        for key in keys {
            names.entry(key.clone()).or_default().push(id.clone());
        }
    }
    names.insert("bob".into(), vec!["01Z1".into(), "01Z2".into()]);
    let draft = |axes: &[usize]| {
        let mut v = vec![0.0; 16];
        v[0] = 1.0;
        for axis in axes {
            v[*axis] = 1.0;
        }
        v
    };
    let suggest = |text: &[f32], exclude: Option<&str>| {
        closest_name(&vectors, &names, text, exclude, CUTS.join, CUTS.lead).map(|(name, _)| name)
    };
    assert_eq!(suggest(&draft(&[3, 1]), None).as_deref(), Some("atlas"));
    assert_eq!(
        suggest(&draft(&[3, 1]), Some("01A")).as_deref(),
        Some("atlas")
    );
    assert_eq!(suggest(&draft(&[14]), None), None, "about nothing named");
    assert_eq!(
        suggest(&draft(&[6]), None),
        None,
        "two notes are too few to name"
    );
}

#[test]
fn a_note_naming_two_names_is_in_the_threads_of_both() {
    let (vectors, when, mut mentioned) = project();
    // Marie in the 404s and once in the performance work.
    for id in ["01A", "01B", "01C", "01D"] {
        mentioned
            .entry(id.to_string())
            .or_default()
            .push("marie".into());
    }
    let mut threads = Threads::default();
    threads.reconcile_mentioned(&vectors, &when, &Edits::default(), &mentioned);
    assert_eq!(threads.in_scope("atlas", "01B"), Some("@atlas:01A"));
    assert_eq!(threads.in_scope("marie", "01B"), Some("@marie:01A"));
    let marie = threads.get("@marie:01A", &when, &Edits::default()).unwrap();
    assert_eq!(marie.notes, ["01A", "01B", "01C"]);
    assert_eq!(
        threads.ranked_for("01B", &vectors),
        ["@atlas:01D"],
        "only the threads of its own names, and not those it is in"
    );
}

#[test]
fn a_name_no_longer_mentioned_takes_its_threads_along() {
    let (vectors, when, mut mentioned) = project();
    let mut threads = Threads::default();
    threads.reconcile_mentioned(&vectors, &when, &Edits::default(), &mentioned);
    mentioned.clear();
    assert!(threads.reconcile_mentioned(&vectors, &when, &Edits::default(), &mentioned));
    assert!(!threads.scopes.contains_key("atlas"));
    assert_eq!(threads.of("01A"), Some("01A"), "back in the general scope");
}

#[test]
fn a_note_put_in_a_names_thread_stays_there_and_scopes_round_trip() {
    let (vectors, when, mentioned) = project();
    let mut edits = Edits::default();
    edits.put_in("01Z1".into(), "@atlas:01D".into());
    edits.put_in("01A".into(), "@atlas:01D".into());
    edits.put_in("01A".into(), "@atlas:01A-2".into());
    assert_eq!(
        edits.put("atlas", "01A"),
        Some("@atlas:01A-2"),
        "one per scope"
    );
    let mut threads = Threads::default();
    threads.reconcile_mentioned(&vectors, &when, &edits, &mentioned);
    assert_eq!(threads.in_scope("atlas", "01Z1"), Some("@atlas:01D"));
    assert!(edits.kept().contains("@atlas:01D"));

    let mut db = SpaceDb::in_memory().unwrap();
    threads.save(&mut db).unwrap();
    assert_eq!(Threads::load(&db), threads);
    edits.save(&mut db).unwrap();
    assert_eq!(Edits::load(&db), edits);
    assert!(edits.take_out("01A"));
    assert_eq!(edits.put("atlas", "01A"), None);
}

/// Each made-up note that mentions a name, written with the name as a
/// plain word instead, the note left out as if it were being written:
/// nearly every time its name is suggested, and never another. The
/// notes that mention none get no name, but for planting tomatoes on a
/// balcony, which looks like the garden's. Needs the embedding model;
/// `cargo test a_draft_is_given -- --ignored --nocapture`.
#[test]
#[ignore = "needs the downloaded embedding model"]
fn a_draft_is_given_the_name_it_looks_like() {
    use crate::embed::{installed_embedder, samples, Embedder};

    let Some(embedder) = installed_embedder() else {
        return;
    };
    let mut notes = samples::notes();
    notes.extend(samples::mentioned());
    let mut vectors = Vectors::new(embedder.model_id(), embedder.dims());
    let mut names: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (_, note) in &notes {
        let vector = embedder.embed_document(&note.body).unwrap();
        vectors
            .insert(note.id.clone(), note.hash.clone(), vector)
            .unwrap();
        for mention in crate::mentions::mentions(&note.body) {
            names.entry(mention.key).or_default().push(note.id.clone());
        }
    }
    let suggest = |text: &str, exclude: &str| {
        let vector = embedder.embed_document(text).unwrap();
        closest_name(
            &vectors,
            &names,
            &vector,
            Some(exclude),
            CUTS.join,
            CUTS.lead,
        )
        .map(|(name, _)| name)
    };

    let (mut named, mut asked) = (0, 0);
    for (subject, note) in samples::mentioned() {
        let mentioned = crate::mentions::mentions(&note.body);
        let mut draft = note.body.clone();
        for mention in &mentioned {
            draft = draft.replace(&format!("@{}", mention.name), &mention.name);
        }
        let got = suggest(&draft, &note.id);
        eprintln!("{subject:>9} {got:?}");
        if let Some(got) = got {
            assert!(
                mentioned.iter().any(|mention| mention.key == got),
                "{draft:?} given @{got}"
            );
            named += 1;
        }
        asked += 1;
    }
    assert!(named * 10 >= asked * 9, "{named} of {asked} named");

    for (thing, note) in samples::notes() {
        let got = suggest(&note.body, &note.id);
        assert!(
            got.is_none() || (thing == "-balcony" && got.as_deref() == Some("garden")),
            "{:?} given {got:?}",
            note.body
        );
    }
}

/// The made-up notes that mention names, placed with the real model
/// beside the others: among each name's notes, every thread is about one
/// of its subjects, and the subjects with notes enough have theirs; the
/// general samples gather as they do without them. Needs the embedding
/// model; `cargo test threads_part -- --ignored --nocapture`.
#[test]
#[ignore = "needs the downloaded embedding model"]
fn threads_part_the_subjects_of_a_name() {
    use crate::embed::{installed_embedder, samples, Embedder};

    let Some(embedder) = installed_embedder() else {
        return;
    };
    let mut notes = samples::notes();
    notes.extend(samples::mentioned());
    let mut vectors = Vectors::new(embedder.model_id(), embedder.dims());
    let mut when = HashMap::new();
    let mut mentioned = Mentioned::new();
    for (_, note) in &notes {
        let vector = embedder.embed_document(&note.body).unwrap();
        vectors
            .insert(note.id.clone(), note.hash.clone(), vector)
            .unwrap();
        let written = When {
            date: NaiveDate::parse_from_str(&note.date, "%Y-%m-%d").unwrap(),
            time: note.time.clone(),
        };
        when.insert(note.id.clone(), written);
        let keys: Vec<String> = crate::mentions::mentions(&note.body)
            .into_iter()
            .map(|mention| mention.key)
            .collect();
        if !keys.is_empty() {
            mentioned.insert(note.id.clone(), keys);
        }
    }
    let about: HashMap<&str, &str> = notes
        .iter()
        .map(|(thing, note)| (note.id.as_str(), *thing))
        .collect();

    let mut threads = Threads::default();
    threads.reconcile_mentioned(&vectors, &when, &Edits::default(), &mentioned);
    let mut found: BTreeSet<(String, &str)> = BTreeSet::new();
    for thread in threads.list(&when, &Edits::default()) {
        let things: BTreeSet<&str> = thread.notes.iter().map(|id| about[id.as_str()]).collect();
        let scope = thread.scope.clone().unwrap_or_default();
        eprintln!("{scope:>8} {things:?} {}", thread.notes.len());
        assert_eq!(things.len(), 1, "a thread of {scope:?} mixes {things:?}");
        found.insert((scope, things.into_iter().next().unwrap()));
    }
    for (scope, subject) in [
        ("atlas", "404"),
        ("atlas", "release"),
        ("atlas", "perf"),
        ("marie", "leave"),
        ("marie", "review"),
        ("garden", "water"),
        ("", "argocd"),
        ("", "kitchen"),
    ] {
        assert!(
            found.contains(&(scope.to_string(), subject)),
            "no thread of {scope:?} about {subject}"
        );
    }
}

/// The made-up notes, placed with the real model: every thread is about
/// one thing, and nearly every note about a thing is in its thread, while
/// a note on its own stays so. Needs the embedding model;
/// `cargo test threads_gather -- --ignored --nocapture`.
#[test]
#[ignore = "needs the downloaded embedding model"]
fn threads_gather_the_notes_about_each_thing() {
    use crate::embed::{installed_embedder, samples, Embedder};

    let Some(embedder) = installed_embedder() else {
        return;
    };
    let notes = samples::notes();
    let mut vectors = Vectors::new(embedder.model_id(), embedder.dims());
    let mut when = HashMap::new();
    for (_, note) in &notes {
        let vector = embedder.embed_document(&note.body).unwrap();
        vectors
            .insert(note.id.clone(), note.hash.clone(), vector)
            .unwrap();
        let written = When {
            date: NaiveDate::parse_from_str(&note.date, "%Y-%m-%d").unwrap(),
            time: note.time.clone(),
        };
        when.insert(note.id.clone(), written);
    }
    let about: HashMap<&str, &str> = notes
        .iter()
        .map(|(thing, note)| (note.id.as_str(), *thing))
        .collect();

    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &Edits::default());
    let listed = threads.list(&when, &Edits::default());

    let mut threaded = 0;
    for thread in &listed {
        let things: BTreeSet<&str> = thread.notes.iter().map(|id| about[id.as_str()]).collect();
        eprintln!("{things:?} {}", thread.notes.len());
        assert_eq!(things.len(), 1, "a thread mixes {things:?}");
        assert!(
            !things.iter().any(|thing| thing.starts_with('-')),
            "{things:?}"
        );
        threaded += thread.notes.len();
    }
    let on_a_thing = notes
        .iter()
        .filter(|(thing, _)| !thing.starts_with('-'))
        .count();
    assert!(
        threaded * 10 >= on_a_thing * 9,
        "{threaded} of {on_a_thing} threaded"
    );
}
