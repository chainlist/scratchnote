use super::*;
use crate::storage::daily_file::{append_note, body_hash};
use std::time::SystemTime;

fn db() -> SearchDb {
    SearchDb::in_memory().unwrap()
}

fn note(id: &str, date: &str, time: &str, body: &str) -> Note {
    Note {
        id: id.to_string(),
        date: date.to_string(),
        time: time.to_string(),
        file: relative_day_path(date),
        subject: None,
        hash: body_hash(body),
        ahead_off: false,
        body: body.to_string(),
        kind: Kind::Note,
        on: None,
        missing: false,
    }
}

fn page(id: &str, date: &str, title: &str, body: &str) -> Note {
    Note {
        file: page_file::relative_path(date, &page_file::file_name(date, title, 1)),
        subject: Some(title.to_string()),
        kind: Kind::Page,
        ..note(id, date, "10:00", body)
    }
}

fn write_page(root: &Path, page: &Note) {
    let path = root.join(&page.file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, page_file::render_page(page)).unwrap();
}

fn scratch_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("scratchnote-index-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn the_day_ahead_survives_the_cache() {
    let mut note = note("01A", "2026-09-22", "09:00", "dentist friday");
    note.on = Some("2026-09-25".to_string());
    let mut index = Index::default();
    index.push(IndexEntry::from(&note));
    let jsonl = index.to_jsonl();
    assert!(jsonl.contains(r#""on":"2026-09-25""#), "{jsonl}");
    let back = Index::from_jsonl(&jsonl);
    let entry = back.entries().next().unwrap();
    assert_eq!(entry.on.as_deref(), Some("2026-09-25"));
    assert_eq!(entry.to_note().on.as_deref(), Some("2026-09-25"));

    // An entry without a day writes none, as before.
    note.on = None;
    assert!(!serde_json::to_string(&IndexEntry::from(&note))
        .unwrap()
        .contains("\"on\""));
}

/// Writes a day file the same way the writer would.
fn write_day(root: &Path, date: &str, notes: &[Note]) {
    let path = super::super::day_path(root, date);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut contents = String::new();
    for note in notes {
        contents = append_note(&contents, note, date);
    }
    std::fs::write(path, contents).unwrap();
}

#[test]
fn jsonl_round_trips() {
    let mut index = Index::default();
    index.push(IndexEntry::from(&note("01AAA", "2026-09-22", "08:00", "a")));
    index.push(IndexEntry::from(&note("01BBB", "2026-09-23", "09:00", "b")));

    let back = Index::from_jsonl(&index.to_jsonl());
    assert_eq!(back.len(), 2);
    assert_eq!(
        back.days(),
        vec![("2026-09-23".into(), 1), ("2026-09-22".into(), 1)]
    );
}

#[test]
fn a_line_matches_the_shape_in_the_spec() {
    let mut index = Index::default();
    index.push(IndexEntry::from(&note(
        "01AAA",
        "2026-09-22",
        "14:32",
        "body",
    )));
    let line = index.to_jsonl();
    let value: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

    for key in ["id", "date", "time", "file", "subject", "hash"] {
        assert!(value.get(key).is_some(), "missing {key} in {line}");
    }
    assert!(
        value.get("body").is_none(),
        "the index must not carry bodies"
    );
    assert_eq!(value["file"], "notes/2026/2026-09-22.md");
}

#[test]
fn a_day_as_last_read_is_not_read_again() {
    let root = scratch_root("fresh-cache");
    write_day(
        &root,
        "2026-09-22",
        &[note("01AAA", "2026-09-22", "08:00", "the body")],
    );
    let mut db = db();
    let cache = index_path(&root);
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    std::fs::write(&cache, rebuild(&root, &mut db).to_jsonl()).unwrap();

    // Changed to the same length, its time put back: read again, the day
    // would say otherwise.
    let day = super::super::day_path(&root, "2026-09-22");
    let modified = std::fs::metadata(&day).unwrap().modified().unwrap();
    let text = std::fs::read_to_string(&day).unwrap();
    std::fs::write(&day, text.replace("the body", "THE BODY")).unwrap();
    filetime::set_file_mtime(&day, filetime::FileTime::from_system_time(modified)).unwrap();

    let (index, changed) = load(&root, &mut db);
    assert!(!changed, "a fresh cache needs no write back");
    assert_eq!(index.len(), 1);
    assert_eq!(db.bodies(["01AAA"]).unwrap()["01AAA"], "the body");

    // With search.db gone, the day is read into a new one, and what it now
    // says reaches the cache too.
    let mut fresh = self::db();
    let (index, changed) = load(&root, &mut fresh);
    assert!(changed);
    assert_eq!(fresh.bodies(["01AAA"]).unwrap()["01AAA"], "THE BODY");
    assert_eq!(index.entries().next().unwrap().hash, body_hash("THE BODY"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_cache_written_with_labels_is_rebuilt_from_the_markdown() {
    let root = scratch_root("labels-cache");
    write_day(
        &root,
        "2026-09-22",
        &[note("01AAA", "2026-09-22", "08:00", "Dentist on Friday")],
    );
    let cache = index_path(&root);
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    std::fs::write(
        &cache,
        r#"{"id":"01AAA","date":"2026-09-22","time":"08:00","file":"notes/2026/2026-09-22.md","subject":"Dentist","category":"health","status":"done","hash":"x","on":"2026-10-02"}"#,
    )
    .unwrap();

    let (index, changed) = load(&root, &mut db());
    assert!(changed);
    let entry = index.entries().next().unwrap();
    assert_eq!(entry.subject, None);
    assert_eq!(entry.on.as_deref(), Some("2026-09-25"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn one_bad_line_costs_only_that_entry() {
    let mut index = Index::default();
    index.push(IndexEntry::from(&note("01AAA", "2026-09-22", "08:00", "a")));
    let raw = format!("not json at all\n{}", index.to_jsonl());
    assert_eq!(Index::from_jsonl(&raw).len(), 1);
}

#[test]
fn days_are_newest_first_with_counts() {
    let mut index = Index::default();
    index.push(IndexEntry::from(&note("01AAA", "2026-09-22", "08:00", "a")));
    index.push(IndexEntry::from(&note("01BBB", "2026-09-22", "09:00", "b")));
    index.push(IndexEntry::from(&note("01CCC", "2026-09-24", "09:00", "c")));

    assert_eq!(
        index.days(),
        vec![("2026-09-24".into(), 1), ("2026-09-22".into(), 2)]
    );
}

#[test]
fn words_leave_out_pages_and_markdown_marks() {
    let mut index = Index::default();
    index.push(IndexEntry::from(&note(
        "01AAA",
        "2026-09-22",
        "08:00",
        "# Groceries\n- [ ] buy milk\n- [x] call Anna, 2 times",
    )));
    index.push(IndexEntry::from(&note(
        "01BBB",
        "2026-09-22",
        "09:00",
        "> fine",
    )));
    index.push(IndexEntry::from(&page(
        "01CCC",
        "2026-09-22",
        "Plan",
        "long page text",
    )));

    assert_eq!(index.words_on("2026-09-22"), 8);
    assert_eq!(index.words_on("2026-09-23"), 0);
}

#[test]
fn words_are_counted_once_and_kept_without_the_body() {
    let mut index = Index::default();
    index.push(IndexEntry::from(&note(
        "01AAA",
        "2026-09-22",
        "08:00",
        "three small words",
    )));
    assert_eq!(index.words_on("2026-09-22"), 3);

    let back = Index::from_jsonl(&index.to_jsonl());
    assert_eq!(back.words_on("2026-09-22"), 3);
}

#[test]
fn a_cache_from_before_words_were_counted_has_its_days_counted() {
    let root = scratch_root("uncounted-cache");
    write_day(
        &root,
        "2026-09-22",
        &[note("01AAA", "2026-09-22", "08:00", "four words in here")],
    );
    // Written after the day file, and search.db read it, so only the
    // missing count makes it stale.
    let mut db = db();
    let old: String = rebuild(&root, &mut db)
        .to_jsonl()
        .lines()
        .map(|line| {
            let mut value: serde_json::Value = serde_json::from_str(line).unwrap();
            value.as_object_mut().unwrap().remove("words");
            format!("{value}\n")
        })
        .collect();
    let cache = index_path(&root);
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    std::fs::write(&cache, old).unwrap();

    let (index, changed) = load(&root, &mut db);
    assert!(changed, "the counts have to be written back");
    assert_eq!(index.words_on("2026-09-22"), 4);

    let _ = std::fs::remove_dir_all(&root);
}

/// What a heavy writer's space costs to load, for the memory plan: five
/// years at 40 notes a day. Not a gate, a measure:
/// `cargo test --release heavy_writer -- --ignored --nocapture`.
#[test]
#[ignore]
fn a_heavy_writers_space_loads() {
    const DAYS: usize = 5 * 365;
    const NOTES_A_DAY: usize = 40;
    let root = scratch_root("heavy-writer");
    let start = chrono::NaiveDate::from_ymd_opt(2021, 1, 1).unwrap();
    let sentence = "Talked with the team about the deployment pipeline and the \
                    staging cluster, then wrote down what to try next week. ";
    for day in 0..DAYS {
        let date = (start + chrono::Days::new(day as u64))
            .format("%Y-%m-%d")
            .to_string();
        let notes: Vec<Note> = (0..NOTES_A_DAY)
            .map(|n| {
                let body = format!("Note {n} of {date}. {}", sentence.repeat(1 + n % 8));
                let time = format!("{:02}:{:02}", 8 + n / 4, (n % 4) * 15);
                note(&format!("{day:05}{n:02}"), &date, &time, &body)
            })
            .collect();
        write_day(&root, &date, &notes);
    }
    let cache = index_path(&root);
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    let started = std::time::Instant::now();
    let mut db = SearchDb::open(&root).unwrap();
    std::fs::write(&cache, rebuild(&root, &mut db).to_jsonl()).unwrap();
    let built = started.elapsed();
    drop(db);

    let started = std::time::Instant::now();
    let mut db = SearchDb::open(&root).unwrap();
    let (index, changed) = load(&root, &mut db);
    let took = started.elapsed();
    assert!(!changed);

    let timed = |query: &str| {
        let started = std::time::Instant::now();
        let found = crate::search::search(&index, &db, query, 0, 50).unwrap();
        (found.total, started.elapsed())
    };
    let (broad_hits, broad) = timed("deployment");
    let (one_day_hits, one_day) = timed("2023-05-14");
    let (short_hits, short) = timed("of");
    drop(db);
    let on_disk = std::fs::metadata(super::super::search_db::search_db_path(&root))
        .unwrap()
        .len();
    eprintln!(
        "{} notes: search.db built in {built:?}, {:.1} MB; fresh caches loaded in {took:?}; \
         'deployment' found {broad_hits} in {broad:?}, '2023-05-14' {one_day_hits} in {one_day:?}, \
         'of' {short_hits} in {short:?}",
        index.len(),
        on_disk as f64 / 1_000_000.0
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn rebuild_reads_every_daily_file() {
    let root = scratch_root("rebuild");
    write_day(
        &root,
        "2026-09-22",
        &[
            note("01AAA", "2026-09-22", "08:00", "first"),
            note("01BBB", "2026-09-22", "09:00", "second"),
        ],
    );
    write_day(
        &root,
        "2026-09-23",
        &[note("01CCC", "2026-09-23", "10:00", "third")],
    );

    let index = rebuild(&root, &mut db());
    assert_eq!(index.len(), 3);
    assert_eq!(
        index.days(),
        vec![("2026-09-23".into(), 1), ("2026-09-22".into(), 2)]
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// SPEC 11: deleting `.scratchnote/` and relaunching restores the index.
#[test]
fn load_rebuilds_from_markdown_when_the_cache_is_missing() {
    let root = scratch_root("missing-cache");
    write_day(
        &root,
        "2026-09-22",
        &[note("01AAA", "2026-09-22", "08:00", "first")],
    );

    let (index, changed) = load(&root, &mut db());
    assert!(changed, "a missing cache has to be written back");
    assert_eq!(index.len(), 1);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn load_reparses_a_day_whose_file_is_newer_than_the_cache() {
    let root = scratch_root("stale-cache");
    write_day(
        &root,
        "2026-09-22",
        &[note("01AAA", "2026-09-22", "08:00", "first")],
    );

    // A cache written before the file it describes, and search.db up to
    // date, so only the cache is stale.
    let mut db = db();
    rebuild(&root, &mut db);
    let cache = index_path(&root);
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    std::fs::write(&cache, "").unwrap();
    let old = SystemTime::now() - std::time::Duration::from_secs(60);
    filetime::set_file_mtime(&cache, filetime::FileTime::from_system_time(old)).unwrap();

    let (index, changed) = load(&root, &mut db);
    assert!(changed);
    assert_eq!(
        index.len(),
        1,
        "the newer day file should have been reparsed"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn pages_are_indexed_from_their_files_and_counted_on_their_day() {
    let root = scratch_root("pages");
    write_day(
        &root,
        "2026-09-22",
        &[note("01AAA", "2026-09-22", "08:00", "a note")],
    );
    write_page(
        &root,
        &page("01PPP", "2026-09-22", "Weekly sync", "the meeting"),
    );
    write_page(&root, &page("01QQQ", "2026-09-24", "Retro", "went well"));
    // Not a page: no marker.
    std::fs::write(root.join("pages/2026/loose.md"), "# loose\n").unwrap();

    let mut db = db();
    let index = rebuild(&root, &mut db);
    assert_eq!(index.len(), 3);
    assert_eq!(
        index.days(),
        vec![("2026-09-24".into(), 1), ("2026-09-22".into(), 2)]
    );
    let sync = index.page("01PPP").unwrap();
    assert_eq!(sync.file, "pages/2026/2026-09-22 Weekly sync.md");
    assert_eq!(sync.subject.as_deref(), Some("Weekly sync"));
    let words = ["weekly".to_string(), "meeting".to_string()];
    assert!(
        db.matching(&words).unwrap().contains("01PPP"),
        "the title and the text"
    );
    assert_eq!(
        index.page_at(&sync.file).map(|p| p.id.as_str()),
        Some("01PPP")
    );
    assert_eq!(index.pages_on("2026-09-24").count(), 1);

    // A day's reparse is about its notes and leaves its pages alone.
    let mut index = index;
    index.replace_day("2026-09-22", Vec::new());
    assert!(index.page("01PPP").is_some());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_kind_survives_the_cache_and_notes_leave_it_out() {
    let mut index = Index::default();
    index.push(IndexEntry::from(&note("01AAA", "2026-09-22", "08:00", "a")));
    index.push(IndexEntry::from(&page("01PPP", "2026-09-22", "Sync", "b")));
    let jsonl = index.to_jsonl();
    let lines: Vec<&str> = jsonl.lines().collect();
    assert!(!lines[0].contains("kind"), "{}", lines[0]);
    assert!(lines[1].contains(r#""kind":"page""#), "{}", lines[1]);

    let back = Index::from_jsonl(&jsonl);
    assert_eq!(back.page("01PPP").map(|p| p.kind), Some(Kind::Page));
    assert_eq!(back.days(), vec![("2026-09-22".into(), 2)]);
}

#[test]
fn load_follows_a_page_renamed_by_hand() {
    let root = scratch_root("renamed-page");
    let sync = page("01PPP", "2026-09-22", "Weekly sync", "the meeting");
    write_page(&root, &sync);
    let cache = index_path(&root);
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    let mut db = db();
    std::fs::write(&cache, rebuild(&root, &mut db).to_jsonl()).unwrap();

    let from = root.join(&sync.file);
    let to = root.join("pages/2026/sync.md");
    std::fs::rename(&from, &to).unwrap();
    // A rename keeps the mtime, so the cache still looks fresh.
    let old = SystemTime::now() - std::time::Duration::from_secs(60);
    filetime::set_file_mtime(&to, filetime::FileTime::from_system_time(old)).unwrap();

    let (index, changed) = load(&root, &mut db);
    assert!(changed, "the cache has to be written back");
    let moved = index.page("01PPP").unwrap();
    assert_eq!(moved.file, "pages/2026/sync.md");
    assert_eq!(db.bodies(["01PPP"]).unwrap()["01PPP"], "the meeting");
    assert_eq!(
        db.page_files().unwrap().keys().collect::<Vec<_>>(),
        ["pages/2026/sync.md"],
        "the old name is forgotten"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_page_as_last_read_is_not_read_again() {
    let root = scratch_root("fresh-page");
    let sync = page("01PPP", "2026-09-22", "Weekly sync", "the meeting");
    write_page(&root, &sync);
    // Not a page, and recorded as read all the same.
    std::fs::write(root.join("pages/2026/loose.md"), "# loose\n").unwrap();
    let mut db = db();
    let cache = index_path(&root);
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    std::fs::write(&cache, rebuild(&root, &mut db).to_jsonl()).unwrap();
    assert!(db.page_files().unwrap().contains_key("pages/2026/loose.md"));

    // Changed to the same length, its time put back: read again, the page
    // would say otherwise.
    let path = root.join(&sync.file);
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("the meeting", "THE MEETING")).unwrap();
    filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(modified)).unwrap();

    let (index, changed) = load(&root, &mut db);
    assert!(!changed, "a fresh cache needs no write back");
    assert!(index.page("01PPP").is_some());
    assert_eq!(db.bodies(["01PPP"]).unwrap()["01PPP"], "the meeting");

    // With search.db gone, the page is read into a new one.
    let mut fresh = self::db();
    let (index, _) = load(&root, &mut fresh);
    assert_eq!(fresh.bodies(["01PPP"]).unwrap()["01PPP"], "THE MEETING");
    assert_eq!(index.page("01PPP").unwrap().hash, body_hash("THE MEETING"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn load_forgets_a_page_whose_file_is_gone() {
    let root = scratch_root("vanished-page");
    let sync = page("01PPP", "2026-09-22", "Weekly sync", "the meeting");
    write_page(&root, &sync);
    write_page(&root, &page("01QQQ", "2026-09-24", "Retro", "went well"));
    let mut db = db();
    let cache = index_path(&root);
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    std::fs::write(&cache, rebuild(&root, &mut db).to_jsonl()).unwrap();

    std::fs::remove_file(root.join(&sync.file)).unwrap();
    let (index, changed) = load(&root, &mut db);
    assert!(changed);
    assert!(index.page("01PPP").is_none());
    assert!(index.page("01QQQ").is_some());
    assert!(db.bodies(["01PPP"]).unwrap().is_empty());
    assert!(!db.page_files().unwrap().contains_key(&sync.file));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_copy_of_a_page_is_taken_once_the_page_is_gone() {
    let root = scratch_root("copied-page");
    let sync = page("01PPP", "2026-09-22", "Weekly sync", "the meeting");
    write_page(&root, &sync);
    let mut db = db();
    let cache = index_path(&root);
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    std::fs::write(&cache, rebuild(&root, &mut db).to_jsonl()).unwrap();

    let copy = "pages/2026/copy.md";
    std::fs::copy(root.join(&sync.file), root.join(copy)).unwrap();
    let (index, _) = load(&root, &mut db);
    assert_eq!(index.page("01PPP").unwrap().file, sync.file);
    assert!(
        !db.page_files().unwrap().contains_key(copy),
        "a copy left out is read again"
    );

    std::fs::remove_file(root.join(&sync.file)).unwrap();
    let (index, changed) = load(&root, &mut db);
    assert!(changed);
    assert_eq!(index.page("01PPP").unwrap().file, copy);
    assert_eq!(db.bodies(["01PPP"]).unwrap()["01PPP"], "the meeting");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn load_forgets_a_day_whose_file_is_gone() {
    let root = scratch_root("vanished-day");
    std::fs::create_dir_all(root.join("notes")).unwrap();

    let mut stale = Index::default();
    stale.push(IndexEntry::from(&note(
        "01AAA",
        "2026-09-22",
        "08:00",
        "gone",
    )));
    let cache = index_path(&root);
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    std::fs::write(&cache, stale.to_jsonl()).unwrap();

    let (index, changed) = load(&root, &mut db());
    assert!(changed);
    assert_eq!(index.len(), 0);

    let _ = std::fs::remove_dir_all(&root);
}
