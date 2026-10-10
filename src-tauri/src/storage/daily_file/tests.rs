use super::*;

const FILE: &str = "notes/2026/2026-09-22.md";
const DATE: &str = "2026-09-22";

fn note(id: &str, time: &str, body: &str) -> Note {
    Note {
        id: id.to_string(),
        date: DATE.to_string(),
        time: time.to_string(),
        file: FILE.to_string(),
        subject: None,
        hash: body_hash(body),
        on: None,
        ahead_off: false,
        body: body.to_string(),
        kind: Kind::Note,
        missing: false,
    }
}

/// A block as it was written while a model labelled the notes.
const LABELLED: &str = concat!(
    "<!-- sn:note id=01J8Z3K6Q9X2 time=14:32 status=done hash=0badc0de lang=fr on=2026-09-25 -->\n",
    "### Rollback plan for ArgoCD sync issue\n",
    "> #infrastructure\n",
    "\n",
    "Talked with the team, the auto-sync broke staging again.\n",
    "We pin the chart version and roll back.\n",
    "<!-- sn:end -->\n",
);

fn parse_one(content: &str) -> Note {
    let notes = parse_notes(content, DATE, FILE);
    assert_eq!(notes.len(), 1, "expected one note in {content:?}");
    notes.into_iter().next().unwrap()
}

#[test]
fn hash_is_eight_hex_chars_over_the_trimmed_body() {
    let h = body_hash("  hello  ");
    assert_eq!(h.len(), 8);
    assert_eq!(h, body_hash("hello"));
    assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn round_trips_a_note() {
    let note = note(
        "01J8Z4P1M7T0",
        "15:10",
        "Buy a new USB-C hub for the homelab",
    );
    assert_eq!(parse_one(&render_note(&note)), note);
}

#[test]
fn round_trips_a_multiline_body() {
    let note = note(
        "01J8Z4P1M7T1",
        "09:01",
        "line one\n\nline three\n### not a heading",
    );
    assert_eq!(parse_one(&render_note(&note)), note);
}

#[test]
fn renders_the_layout_given_in_the_spec() {
    let note = note("01J8Z3K6Q9X2", "14:32", "Buy a USB-C hub");
    assert_eq!(
        render_note(&note),
        "<!-- sn:note id=01J8Z3K6Q9X2 time=14:32 -->\nBuy a USB-C hub\n<!-- sn:end -->\n"
    );
}

#[test]
fn preserves_user_text_between_blocks() {
    let mut doc = String::from("# 2026-09-22\n\nmy own notes here\n\n");
    doc.push_str(&render_note(&note("01AAA", "08:00", "first")));
    doc.push_str("\nstray prose the user typed\n\n");
    doc.push_str(&render_note(&note("01BBB", "09:00", "second")));

    let notes = parse_notes(&doc, DATE, FILE);
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].body, "first");
    assert_eq!(notes[1].body, "second");

    // Appending must leave every earlier byte alone.
    let appended = append_note(&doc, &note("01CCC", "10:00", "third"), DATE);
    assert!(appended.starts_with(&doc));
    assert!(appended.contains("stray prose the user typed"));
}

#[test]
fn a_body_starting_with_a_heading_or_a_quoted_hashtag_is_kept_whole() {
    for body in ["### My heading\ntext", "> #1 priority\n> ship it"] {
        let note = note("01DDD", "11:00", body);
        assert_eq!(parse_one(&render_note(&note)), note);
    }
}

#[test]
fn skips_a_block_with_no_end_marker() {
    let doc = format!(
        "# 2026-09-22\n\n<!-- sn:note id=01EEE time=08:00 -->\ndangling\n\n{}",
        render_note(&note("01FFF", "09:00", "intact"))
    );
    let notes = parse_notes(&doc, DATE, FILE);
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "01FFF");
}

#[test]
fn skips_a_block_missing_required_attributes() {
    let doc = "<!-- sn:note time=08:00 -->\nno id\n<!-- sn:end -->\n";
    assert!(parse_notes(doc, DATE, FILE).is_empty());
}

#[test]
fn a_labelled_block_is_read_without_its_labels() {
    let note = parse_one(LABELLED);
    assert_eq!(note.subject, None);
    assert_eq!(
        note.body,
        "Talked with the team, the auto-sync broke staging again.\nWe pin the chart version and roll back."
    );
    assert_eq!(note.hash, body_hash(&note.body));
    // The day the model wrote is not taken: this text names none.
    assert_eq!(note.on, None);
}

#[test]
fn a_note_labelled_with_a_summary_and_tags_loses_them_all() {
    let doc = "<!-- sn:note id=01AAA time=08:00 status=done hash=dead -->\n### s\n> A summary.\n> #infra #argocd #staging\n\nbody\n<!-- sn:end -->\n";
    assert_eq!(parse_one(doc).body, "body");
    let bare = "<!-- sn:note id=01AAA time=08:00 status=manual hash=dead -->\n### s\n>   \n> #infra\n\nbody\n<!-- sn:end -->\n";
    assert_eq!(parse_one(bare).body, "body");
}

#[test]
fn dropping_labels_rewrites_only_the_labelled_blocks() {
    let mut doc = String::from("# 2026-09-22\n\nmy prose\n\n");
    doc.push_str(&render_note(&note("01AAA", "08:00", "### kept heading")));
    doc.push('\n');
    doc.push_str(LABELLED);
    doc.push_str("\ntrailing prose\n");

    let out = drop_labels(&doc).expect("one block is labelled");
    assert!(out.starts_with(
        "# 2026-09-22\n\nmy prose\n\n<!-- sn:note id=01AAA time=08:00 -->\n### kept heading\n"
    ));
    assert!(out.contains("<!-- sn:note id=01J8Z3K6Q9X2 time=14:32 -->\nTalked with the team"));
    assert!(out.ends_with("\ntrailing prose\n"));
    assert!(!out.contains("status=") && !out.contains("> #infrastructure"));
    assert_eq!(parse_notes(&out, DATE, FILE), parse_notes(&doc, DATE, FILE));
    assert_eq!(drop_labels(&out), None, "nothing left to drop");

    let crlf = drop_labels(&doc.replace('\n', "\r\n")).unwrap();
    assert!(!crlf.replace("\r\n", "").contains('\n'));
}

#[test]
fn the_day_ahead_is_read_off_the_text_unless_cleared() {
    let doc = append_note("", &note("01AAA", "08:00", "Dentist on Friday"), DATE);
    assert_eq!(parse_one(&doc).on.as_deref(), Some("2026-09-25"));

    let out = clear_day_ahead(&doc, "01AAA").unwrap();
    assert!(out.contains("time=08:00 ahead=off -->"), "{out}");
    let cleared = parse_one(&out);
    assert_eq!(cleared.on, None);
    assert!(cleared.ahead_off);
    // Still off once the text changes.
    let edited = replace_body(&out, "01AAA", "Dentist on Monday").unwrap();
    assert_eq!(parse_one(&edited).on, None);
    assert!(clear_day_ahead(&doc, "01ZZZ").is_none());
}

#[test]
fn parses_an_empty_file_as_no_notes() {
    assert!(parse_notes("", DATE, FILE).is_empty());
    assert!(parse_notes("# 2026-09-22\n", DATE, FILE).is_empty());
}

/// Three notes with the user's own prose wrapped around them.
fn day_with_prose() -> String {
    let mut doc = String::from("# 2026-09-22\n\nmy own notes here\n\n");
    doc.push_str(&render_note(&note("01AAA", "08:00", "first")));
    doc.push('\n');
    doc.push_str(&render_note(&note("01BBB", "09:00", "second")));
    doc.push_str("\nstray prose the user typed\n\n");
    doc.push_str(&render_note(&note("01CCC", "10:00", "third")));
    doc
}

#[test]
fn removes_only_the_named_block() {
    let out = remove_note(&day_with_prose(), "01BBB").expect("id is present");
    let left = parse_notes(&out, DATE, FILE);
    assert_eq!(
        left.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
        ["01AAA", "01CCC"]
    );
    assert!(!out.contains("01BBB"));
}

#[test]
fn removing_a_block_keeps_the_user_prose_around_it() {
    let out = remove_note(&day_with_prose(), "01BBB").expect("id is present");
    assert!(out.starts_with("# 2026-09-22\n\nmy own notes here\n"));
    assert!(out.contains("stray prose the user typed"));
}

#[test]
fn removing_a_block_does_not_leave_a_growing_gap() {
    let out = remove_note(&day_with_prose(), "01BBB").expect("id is present");
    assert!(!out.contains("\n\n\n"), "blank lines piled up:\n{out}");
}

#[test]
fn removes_the_last_block_without_stranding_a_blank_line() {
    let out = remove_note(&day_with_prose(), "01CCC").expect("id is present");
    assert!(out.ends_with("stray prose the user typed\n"), "got:\n{out}");
}

#[test]
fn removes_the_only_block() {
    let doc = append_note("", &note("01AAA", "08:00", "alone"), DATE);
    let out = remove_note(&doc, "01AAA").expect("id is present");
    assert_eq!(out, "# 2026-09-22\n");
    assert!(parse_notes(&out, DATE, FILE).is_empty());
}

#[test]
fn keeps_crlf_when_the_file_uses_it() {
    let doc = day_with_prose().replace('\n', "\r\n");
    let out = remove_note(&doc, "01BBB").expect("id is present");
    assert!(out.contains("\r\n"));
    assert!(
        !out.replace("\r\n", "").contains('\n'),
        "a bare LF survived in a CRLF file"
    );
    assert_eq!(parse_notes(&out, DATE, FILE).len(), 2);
}

#[test]
fn returns_none_for_an_id_that_is_not_there() {
    assert!(remove_note(&day_with_prose(), "01ZZZ").is_none());
}

#[test]
fn does_not_remove_a_block_whose_end_marker_is_missing() {
    let doc = "<!-- sn:note id=01AAA time=08:00 status=pending hash=dead -->\n### (untitled)\nno end marker\n";
    assert!(remove_note(doc, "01AAA").is_none());
}

#[test]
fn replacing_the_body_rehashes_and_keeps_the_rest() {
    let before = Note {
        ahead_off: true,
        ..note("01BBB", "09:00", "the body")
    };
    let doc = format!("# {DATE}\n\nmy prose\n\n{}", render_note(&before));

    let out = replace_body(&doc, &before.id, "  a new body\n").expect("id is present");
    assert!(out.starts_with("# 2026-09-22\n\nmy prose\n"));

    let after = parse_one(&out);
    assert_eq!(after.body, "a new body");
    assert_eq!(after.hash, body_hash("a new body"));
    assert_eq!(after.time, before.time);
    assert!(after.ahead_off);
}

#[test]
fn replacing_the_body_of_an_unknown_id_changes_nothing() {
    assert!(replace_body(&day_with_prose(), "01ZZZ", "x").is_none());
}

#[test]
fn appends_a_day_header_to_a_new_file() {
    let out = append_note("", &note("01HHH", "08:00", "first"), DATE);
    assert!(out.starts_with("# 2026-09-22\n\n<!-- sn:note "));
}

fn stub(id: &str, title: &str) -> Stub {
    Stub {
        id: id.to_string(),
        time: "10:00".to_string(),
        title: title.to_string(),
        target: format!("../../pages/2026/2026-09-22 {title}.md"),
    }
}

/// Notes and prose with a stub between the second and third note.
fn day_with_stub() -> String {
    let mut doc = day_with_prose();
    let at = doc.find("<!-- sn:note id=01CCC").unwrap();
    doc.insert_str(
        at,
        &format!("{}\n", render_stub(&stub("01PPP", "Weekly sync"))),
    );
    doc
}

#[test]
fn renders_the_stub_given_in_the_spec() {
    assert_eq!(
        render_stub(&stub("01PPP", "Weekly sync, platform team")),
        concat!(
            "<!-- sn:page id=01PPP time=10:00 -->\n",
            "[Weekly sync, platform team](<../../pages/2026/2026-09-22 Weekly sync, platform team.md>)\n",
            "<!-- sn:end -->\n",
        )
    );
}

#[test]
fn stubs_round_trip_and_the_note_parser_skips_them() {
    let doc = day_with_stub();
    assert_eq!(parse_stubs(&doc), vec![stub("01PPP", "Weekly sync")]);
    let notes = parse_notes(&doc, DATE, FILE);
    assert_eq!(
        notes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
        ["01AAA", "01BBB", "01CCC"]
    );
    assert!(notes.iter().all(|n| !n.body.contains("sn:page")));
}

#[test]
fn brackets_in_a_title_survive_the_link() {
    let tricky = stub("01PPP", r"Q3 [draft] review \ notes");
    let doc = append_stub("", &tricky, DATE);
    assert!(doc.contains(r"[Q3 \[draft\] review \\ notes]"), "{doc}");
    assert_eq!(parse_stubs(&doc), vec![tricky]);
}

#[test]
fn a_stub_whose_link_was_edited_out_of_shape_is_still_a_stub() {
    let doc = "<!-- sn:page id=01PPP time=10:00 -->\nsomething else\n<!-- sn:end -->\n";
    let stubs = parse_stubs(doc);
    assert_eq!(stubs.len(), 1);
    assert_eq!(
        (stubs[0].title.as_str(), stubs[0].target.as_str()),
        ("", "")
    );
    // A link without angle brackets, as another editor may write it.
    let plain =
        "<!-- sn:page id=01PPP time=10:00 -->\n[Sync](../../pages/2026/a.md)\n<!-- sn:end -->\n";
    assert_eq!(parse_stubs(plain)[0].target, "../../pages/2026/a.md");
}

#[test]
fn replacing_and_removing_a_stub_leaves_the_notes_alone() {
    let doc = day_with_stub();
    let renamed = stub("01PPP", "Renamed");
    let out = replace_stub(&doc, &renamed).expect("stub is present");
    assert_eq!(parse_stubs(&out), vec![renamed]);
    assert_eq!(parse_notes(&out, DATE, FILE), parse_notes(&doc, DATE, FILE));

    let out = remove_stub(&doc, "01PPP").expect("stub is present");
    assert!(parse_stubs(&out).is_empty());
    assert_eq!(out, day_with_prose(), "the file is back as it was");
    assert!(remove_stub(&out, "01PPP").is_none());
}

#[test]
fn removing_a_note_does_not_touch_a_stub_with_its_id() {
    let doc = append_stub(&day_with_prose(), &stub("01AAA", "Clash"), DATE);
    let out = remove_note(&doc, "01AAA").unwrap();
    assert_eq!(parse_stubs(&out).len(), 1);
}

#[test]
fn a_note_turned_into_a_page_leaves_its_stub_in_its_place() {
    let doc = day_with_prose();
    let out = note_to_stub(&doc, "01BBB", &stub("01PPP", "Second")).unwrap();
    assert!(out.contains("stray prose the user typed"));
    let notes = parse_notes(&out, DATE, FILE);
    assert_eq!(
        notes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
        ["01AAA", "01CCC"]
    );
    let first = out.find("01AAA").unwrap();
    let page = out.find("01PPP").unwrap();
    let third = out.find("01CCC").unwrap();
    assert!(
        first < page && page < third,
        "the stub keeps the note's place"
    );
    assert!(note_to_stub(&out, "01BBB", &stub("01PPP", "x")).is_none());
}

#[test]
fn a_stub_appended_to_an_empty_day_starts_the_file() {
    let out = append_stub("", &stub("01PPP", "Sync"), DATE);
    assert!(out.starts_with("# 2026-09-22\n\n<!-- sn:page id=01PPP"));
}
