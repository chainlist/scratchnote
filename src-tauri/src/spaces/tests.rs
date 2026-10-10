use super::*;
use crate::storage::daily_file::Note;
use crate::storage::day_path;
use crate::storage::pins::{self, Pin};

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("scratchnote-spaces-{name}"));
    let _ = std::fs::remove_dir_all(&root);
    root
}

/// A day of one note, written as the writer would.
fn write_note(root: &Path, id: &str, date: &str) -> Note {
    use crate::storage::daily_file::{append_note, body_hash, Kind};
    let note = Note {
        id: id.to_string(),
        date: date.to_string(),
        time: "08:00".to_string(),
        file: crate::storage::relative_day_path(date),
        subject: None,
        hash: body_hash("a note"),
        ahead_off: false,
        body: "a note".to_string(),
        kind: Kind::Note,
        on: None,
        missing: false,
    };
    let path = day_path(root, date);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, append_note("", &note, date)).unwrap();
    note
}

fn wake() -> Wake {
    std::sync::Arc::new(tokio::sync::Notify::new())
}

#[test]
fn a_change_to_threads_is_undone_once_and_never_over_a_later_one() {
    let root = scratch("undo-threads");
    write_note(&root, "01AAA", "2026-09-22");
    let space = Space::new("Test", root.clone(), wake());
    space.load();
    assert_eq!(space.undo_threads().unwrap(), None, "nothing to undo yet");

    // A merge-like change: the edits and the pins both move.
    let before = space.decided();
    space
        .change_edits(|edits| edits.alone.insert("01AAA".into()))
        .unwrap();
    space
        .change_pins(|pins| {
            pins.push(Pin {
                kind: pins::PinKind::Thread,
                target: "01AAA".into(),
                label: None,
            });
            true
        })
        .unwrap();
    space.remember_change(before.clone());
    assert_eq!(space.undo_threads().unwrap(), Some(true));
    assert_eq!(space.decided(), before);
    assert_eq!(space.undo_threads().unwrap(), None, "only once");

    // Anything changed after it keeps the older change from coming undone.
    let before = space.decided();
    space
        .change_edits(|edits| {
            edits.titles.insert("01AAA".into(), "Kitchen".into());
            true
        })
        .unwrap();
    space.remember_change(before);
    space
        .change_edits(|edits| edits.alone.insert("01AAA".into()))
        .unwrap();
    assert_eq!(space.undo_threads().unwrap(), None);
    assert!(space.edits().titles.contains_key("01AAA"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_space_is_read_only_while_it_is_open() {
    let root = scratch("open-close");
    write_note(&root, "01AAA", "2026-09-22");

    let space = Space::new("Test", root.clone(), wake());
    assert!(!space.is_open());
    assert_eq!(space.note_count(), None);

    space.load();
    assert!(space.is_open());
    assert_eq!(space.note_count(), Some(1));

    assert_eq!(space.unload(), Some(1));
    assert!(!space.is_open());
    assert_eq!(space.note_count(), None);
    assert_eq!(space.unload(), None, "no count to record twice");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_closed_space_drops_changes_and_reads_them_when_opened() {
    let root = scratch("closed-writes");
    write_note(&root, "01AAA", "2026-09-22");
    let space = Space::open("Test", root.clone(), wake());

    // What a job or a late capture does once the space is left.
    space.unload();
    write_note(&root, "01BBB", "2026-09-23");
    space.day_changed("2026-09-23").unwrap();
    let late = write_note(&root, "01CCC", "2026-09-24");
    space.note_added(&late).unwrap();
    assert_eq!(space.note_count(), None);

    // Opened again, the days written meanwhile are read from their files.
    space.load();
    assert_eq!(space.note_count(), Some(3));
    drop(space);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn opening_removes_the_index_file_of_earlier_versions() {
    let root = scratch("old-index");
    write_note(&root, "01AAA", "2026-09-22");
    let old = paths::meta_dir(&root).join("index.jsonl");
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    std::fs::write(&old, "{}\n").unwrap();

    let space = Space::open("Test", root.clone(), wake());
    assert!(!old.exists());
    assert_eq!(space.note_count(), Some(1));
    drop(space);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_rebuild_drops_the_vectors_to_embed_every_note_again() {
    let root = scratch("rebuild-vectors");
    write_note(&root, "01AAA", "2026-09-22");
    let space = Space::open("Test", root.clone(), wake());
    let mut vectors = Vectors::new("m", 2);
    vectors
        .insert("01AAA".into(), "h".into(), vec![1.0, 0.0])
        .unwrap();
    vectors
        .save(space.db.lock().unwrap().as_mut().unwrap())
        .unwrap();
    *space.vectors.lock().unwrap() = Some(vectors);

    assert_eq!(space.rebuild().unwrap(), 1);
    assert!(space.vectors.lock().unwrap().is_none());
    let saved = Vectors::load(space.db.lock().unwrap().as_ref().unwrap(), "m", 2);
    assert_eq!(saved.len(), 0);
    drop(space);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_missing_file_opens_the_first_space() {
    let registry = Registry::load(&scratch("missing"));
    assert_eq!(registry.active, FIRST_NAME);
}

#[test]
fn the_space_at_the_root_moves_into_its_own_folder() {
    let root = scratch("migrate");
    std::fs::create_dir_all(root.join("notes").join("2026")).unwrap();
    std::fs::write(root.join("notes/2026/2026-09-22.md"), "x").unwrap();
    std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
    std::fs::write(
        registry_path(&root),
        r#"{"active":"Home","defaultName":"Home"}"#,
    )
    .unwrap();
    // Already taken, ignoring case, so the moved space gets another name.
    std::fs::create_dir_all(spaces_dir(&root).join("home")).unwrap();

    migrate_root(&root);

    let to = spaces_dir(&root).join("Home 2");
    assert!(to.join("notes/2026/2026-09-22.md").exists());
    assert!(!root.join("notes").exists());
    assert_eq!(Registry::load(&root).active, "Home 2");

    // Nothing left at the root, so a second launch changes nothing.
    migrate_root(&root);
    assert!(to.join("notes/2026/2026-09-22.md").exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn names_are_tidied_and_must_make_a_folder() {
    assert_eq!(check_name("  Work   stuff ").unwrap(), "Work stuff");
    assert_eq!(check_name("Journal 2026").unwrap(), "Journal 2026");
    assert_eq!(check_name("日記").unwrap(), "日記");
    for bad in [
        "",
        "   ",
        "a/b",
        "a\\b",
        "what?",
        "C:",
        "..",
        ".hidden",
        "trailing.",
        "CON",
        "com1",
        "nul.txt",
    ] {
        assert!(check_name(bad).is_err(), "{bad:?} should be refused");
    }
    assert!(check_name("COM").is_ok());
    assert!(check_name("Console").is_ok());
    assert!(check_name(&"x".repeat(MAX_NAME + 1)).is_err());
}

#[test]
fn every_usable_folder_under_spaces_is_a_space() {
    let root = scratch("discover");
    for dir in ["work", "Books", "two  spaces", ".git"] {
        std::fs::create_dir_all(spaces_dir(&root).join(dir)).unwrap();
    }
    std::fs::write(spaces_dir(&root).join("stray.md"), "x").unwrap();
    let names: Vec<String> = discover(&root).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, vec!["Books", "work"]);
    let _ = std::fs::remove_dir_all(&root);
}
