//! The activity log, SPEC 4.11: who did what to which note or page of a
//! space. One JSON line per event, in a file per day under the space's
//! `.scratchnote/activity/`, kept for 90 days.
//!
//! It records what happened, not the text, so a deleted note's body is
//! gone with it; only its subject stays here until its day's file goes.
//! Settings > Activity reads it back, newest first.

use std::collections::{BTreeMap, HashMap};
use std::io::{self, SeekFrom};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, NaiveDate, SecondsFormat};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use super::daily_file::{Kind, Note};
use super::index::IndexEntry;

/// How many days of the log are kept, today's included.
pub const KEEP_DAYS: i64 = 90;

pub fn dir(space_root: &Path) -> PathBuf {
    space_root.join(".scratchnote").join("activity")
}

/// `activity/2026/2026-10-01.jsonl` in the space's `.scratchnote/`.
pub fn day_path(space_root: &Path, date: NaiveDate) -> PathBuf {
    let date = date.format("%Y-%m-%d").to_string();
    dir(space_root)
        .join(&date[..4])
        .join(format!("{date}.jsonl"))
}

/// Who made a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Actor {
    /// The app's own windows.
    User,
    /// A plugin, through `app.notes`. Plugins are not sandboxed, so this is
    /// what the plugin API says, not a proof (SPEC 3.9).
    Plugin(String),
    /// The enrichment worker, writing a subject and a category.
    Model,
    /// Another editor, or a sync tool, as the watcher saw it.
    External,
}

impl Actor {
    /// Who a command from a window acts for: the plugin the plugin API
    /// names, or else the user. A window cannot speak for the model.
    pub fn from_plugin(plugin: Option<String>) -> Self {
        match plugin {
            Some(id) if crate::plugins::valid_id(&id) => Actor::Plugin(id),
            _ => Actor::User,
        }
    }
}

impl Serialize for Actor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Actor::User => serializer.serialize_str("user"),
            Actor::Plugin(id) => serializer.serialize_str(&format!("plugin:{id}")),
            Actor::Model => serializer.serialize_str("model"),
            Actor::External => serializer.serialize_str("external"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Create,
    Edit,
    /// An edit that only ticked or cleared task boxes (SPEC 3.4).
    Tick,
    /// A subject or a category set, by the model or by hand.
    Label,
    /// The model gave up on a note (SPEC 5.6).
    Fail,
    /// Sent back to the model by hand.
    Rerun,
    /// A page's title or file changed.
    Rename,
    /// A note's day or a page's file changed outside the app, its text the same.
    Move,
    Delete,
    /// A note turned into a page, which has an id of its own.
    ToPage,
    Attach,
    /// Every note of the space sent back to the model.
    Regenerate,
}

/// What an event is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    Note,
    Page,
    Attachment,
    Space,
}

impl From<Kind> for Target {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Note => Target::Note,
            Kind::Page => Target::Page,
        }
    }
}

/// What changed, each field as `[before, after]`.
pub type Changes = BTreeMap<&'static str, (Value, Value)>;

/// One line of the log.
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    #[serde(serialize_with = "rfc3339")]
    pub at: DateTime<Local>,
    pub actor: Actor,
    pub action: Action,
    pub kind: Target,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The note's or the page's day, which need not be the day of the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Its file, from the space's folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Its subject, a page's title, or an attachment's name, as the event
    /// left it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// How many notes it took in, for an event about the whole space.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub changes: Changes,
}

fn rfc3339<S: Serializer>(at: &DateTime<Local>, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&at.to_rfc3339_opts(SecondsFormat::Secs, false))
}

impl Event {
    pub fn new(actor: Actor, action: Action, kind: Target) -> Self {
        Self {
            at: Local::now(),
            actor,
            action,
            kind,
            id: None,
            date: None,
            file: None,
            subject: None,
            count: None,
            changes: Changes::new(),
        }
    }

    /// About a note or a page, as the event left it.
    pub fn of_entry(actor: Actor, action: Action, entry: &IndexEntry) -> Self {
        Self {
            id: Some(entry.id.clone()),
            date: Some(entry.date.clone()),
            file: Some(entry.file.clone()),
            subject: entry.subject.clone(),
            ..Self::new(actor, action, entry.kind.into())
        }
    }

    pub fn of_note(actor: Actor, action: Action, note: &Note) -> Self {
        Self {
            id: Some(note.id.clone()),
            date: Some(note.date.clone()),
            file: Some(note.file.clone()),
            subject: note.subject.clone(),
            ..Self::new(actor, action, note.kind.into())
        }
    }

    pub fn change(
        mut self,
        field: &'static str,
        before: impl Serialize,
        after: impl Serialize,
    ) -> Self {
        let value = |v: Result<Value, _>| v.unwrap_or(Value::Null);
        self.changes.insert(
            field,
            (
                value(serde_json::to_value(before)),
                value(serde_json::to_value(after)),
            ),
        );
        self
    }

    /// The words the text held before and after; `None` where there was no text.
    pub fn words(self, before: Option<usize>, after: Option<usize>) -> Self {
        self.change("words", before, after)
    }

    pub fn with_changes(mut self, changes: Changes) -> Self {
        self.changes.extend(changes);
        self
    }
}

/// What a reader would see change from `before` to `after` of one note or
/// page: the action that names it best, and every field that changed.
/// `None` when nothing did but its status.
pub fn compare(before: &IndexEntry, after: &IndexEntry) -> Option<(Action, Changes)> {
    let page = after.kind == Kind::Page;
    let mut changes = Changes::new();
    let mut differs = |field: &'static str, a: &Option<String>, b: &Option<String>| {
        if a != b {
            changes.insert(field, (a.clone().into(), b.clone().into()));
            true
        } else {
            false
        }
    };
    let text = before.hash != after.hash;
    let title = differs(
        if page { "title" } else { "subject" },
        &before.subject,
        &after.subject,
    );
    let category = differs("category", &before.category, &after.category);
    let date = differs(
        "date",
        &Some(before.date.clone()),
        &Some(after.date.clone()),
    );
    let file = differs(
        "file",
        &Some(before.file.clone()),
        &Some(after.file.clone()),
    );
    if text {
        changes.insert("words", (before.words.into(), after.words.into()));
    }
    let action = if text {
        Action::Edit
    } else if page && title {
        Action::Rename
    } else if date || file {
        Action::Move
    } else if title || category {
        Action::Label
    } else {
        return None;
    };
    Some((action, changes))
}

/// One note or page as an edit outside the app found it: before, after,
/// or both. `None` on one side is a note that came or went.
pub type Seen = (Option<IndexEntry>, Option<IndexEntry>);

/// The events of edits made outside the app, which the watcher sees file by
/// file. A note moved to another day, or a page to another file, shows as
/// gone from one and new in the other; both halves become one event.
pub fn external(seen: Vec<Seen>) -> Vec<Event> {
    let mut order: Vec<String> = Vec::new();
    let mut merged: HashMap<String, Seen> = HashMap::new();
    for (before, after) in seen {
        let Some(id) = before.as_ref().or(after.as_ref()).map(|e| e.id.clone()) else {
            continue;
        };
        let slot = merged.entry(id.clone()).or_insert_with(|| {
            order.push(id);
            (None, None)
        });
        if slot.0.is_none() {
            slot.0 = before;
        }
        if after.is_some() {
            slot.1 = after;
        }
    }
    order
        .into_iter()
        .filter_map(|id| match merged.remove(&id)? {
            (None, Some(new)) => {
                Some(Event::of_entry(Actor::External, Action::Create, &new).words(None, new.words))
            }
            (Some(old), None) => {
                Some(Event::of_entry(Actor::External, Action::Delete, &old).words(old.words, None))
            }
            (Some(old), Some(new)) => {
                let (action, changes) = compare(&old, &new)?;
                Some(Event::of_entry(Actor::External, action, &new).with_changes(changes))
            }
            (None, None) => None,
        })
        .collect()
}

/// Add `event` to its day's file. The first event of a day also removes
/// the days past keeping. Not synced to disk: a power cut costs the last
/// lines, which is cheaper than a sync on every save.
pub async fn append(space_root: &Path, event: &Event) -> io::Result<()> {
    let today = event.at.date_naive();
    let path = day_path(space_root, today);
    let mut line = serde_json::to_string(event)?;
    line.push('\n');
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(&path)
        .await?;
    let len = file.metadata().await?.len();
    // A line cut short by a crash would swallow this one.
    if len > 0 {
        file.seek(SeekFrom::End(-1)).await?;
        let mut last = [0u8];
        file.read_exact(&mut last).await?;
        if last[0] != b'\n' {
            line.insert(0, '\n');
        }
    }
    file.write_all(line.as_bytes()).await?;
    file.flush().await?;
    if len == 0 {
        prune(space_root, today).await;
    }
    Ok(())
}

/// Whether a day's file is past keeping on `today`.
fn expired(date: NaiveDate, today: NaiveDate) -> bool {
    (today - date).num_days() >= KEEP_DAYS
}

/// The day a file of the log is for, from its name: `2026-10-01.jsonl`.
fn file_date(name: &str) -> Option<NaiveDate> {
    let date = name.strip_suffix(".jsonl")?;
    NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

/// A line of the log as read back. Its fields are kept as written, so a
/// line from a newer version of the app still shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Logged {
    pub at: String,
    pub actor: String,
    pub action: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub changes: BTreeMap<String, (Value, Value)>,
}

/// Whose events to read, as Settings > Activity filters them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Whose {
    User,
    /// Any plugin.
    Plugin,
    Model,
    External,
}

impl Whose {
    fn made(self, actor: &str) -> bool {
        match self {
            Whose::User => actor == "user",
            Whose::Plugin => actor.starts_with("plugin:"),
            Whose::Model => actor == "model",
            Whose::External => actor == "external",
        }
    }
}

/// The log's files still kept on `today`, newest first.
fn kept_days(space_root: &Path, today: NaiveDate) -> Vec<PathBuf> {
    let mut days: Vec<(NaiveDate, PathBuf)> = std::fs::read_dir(dir(space_root))
        .into_iter()
        .flatten()
        .flatten()
        .flat_map(|year| {
            std::fs::read_dir(year.path())
                .into_iter()
                .flatten()
                .flatten()
        })
        .filter_map(|day| Some((file_date(day.file_name().to_str()?)?, day.path())))
        .filter(|(date, _)| !expired(*date, today))
        .collect();
    days.sort_by(|a, b| b.0.cmp(&a.0));
    days.into_iter().map(|(_, path)| path).collect()
}

/// The events of the log newest first, only `whose` when given: `limit` of
/// them past the first `skip`, and whether more are left. Files are read
/// only as far as that goes, and a line that does not read is passed over.
pub fn read(
    space_root: &Path,
    today: NaiveDate,
    whose: Option<Whose>,
    skip: usize,
    limit: usize,
) -> (Vec<Logged>, bool) {
    let mut events = Vec::new();
    let mut passed = 0;
    for path in kept_days(space_root, today) {
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in contents.lines().rev() {
            let Ok(event) = serde_json::from_str::<Logged>(line) else {
                continue;
            };
            if whose.is_some_and(|whose| !whose.made(&event.actor)) {
                continue;
            }
            if passed < skip {
                passed += 1;
                continue;
            }
            if events.len() == limit {
                return (events, true);
            }
            events.push(event);
        }
    }
    (events, false)
}

/// Remove the days past keeping, and the years they leave empty. Anything
/// else in the folder is left alone.
async fn prune(space_root: &Path, today: NaiveDate) {
    let Ok(mut years) = tokio::fs::read_dir(dir(space_root)).await else {
        return;
    };
    while let Ok(Some(year)) = years.next_entry().await {
        let Ok(mut days) = tokio::fs::read_dir(year.path()).await else {
            continue;
        };
        let mut left = false;
        while let Ok(Some(day)) = days.next_entry().await {
            match day.file_name().to_str().and_then(file_date) {
                Some(date) if expired(date, today) => {
                    if let Err(e) = tokio::fs::remove_file(day.path()).await {
                        log::warn!("could not remove {}: {e}", day.path().display());
                        left = true;
                    }
                }
                _ => left = true,
            }
        }
        if !left {
            let _ = tokio::fs::remove_dir(year.path()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::daily_file::Status;
    use chrono::TimeZone;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("scratchnote-activity-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn entry(id: &str, kind: Kind) -> IndexEntry {
        IndexEntry {
            id: id.to_string(),
            date: "2026-09-22".to_string(),
            time: "14:32".to_string(),
            file: "notes/2026/2026-09-22.md".to_string(),
            subject: Some("Rollback plan".to_string()),
            category: Some("infrastructure".to_string()),
            status: Status::Done,
            hash: "a1b2c3d4".to_string(),
            lang: None,
            kind,
            words: Some(42),
            body: String::new(),
        }
    }

    fn at(date: &str) -> DateTime<Local> {
        let day = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
        Local
            .from_local_datetime(&day.and_hms_opt(12, 0, 0).unwrap())
            .unwrap()
    }

    fn json(event: &Event) -> Value {
        serde_json::to_value(event).unwrap()
    }

    #[test]
    fn a_line_says_who_did_what_where() {
        let edited = Event::of_entry(
            Actor::Plugin("mentions".to_string()),
            Action::Edit,
            &entry("01AAA", Kind::Note),
        )
        .words(Some(42), Some(47));
        let line = json(&edited);
        assert_eq!(line["actor"], "plugin:mentions");
        assert_eq!(line["action"], "edit");
        assert_eq!(line["kind"], "note");
        assert_eq!(line["id"], "01AAA");
        assert_eq!(line["date"], "2026-09-22");
        assert_eq!(line["subject"], "Rollback plan");
        assert_eq!(line["changes"]["words"], serde_json::json!([42, 47]));
        assert!(line.get("count").is_none(), "empty fields are left out");

        let created = Event::of_entry(Actor::User, Action::Create, &entry("01AAA", Kind::Note))
            .words(None, Some(3));
        assert_eq!(
            json(&created)["changes"]["words"],
            serde_json::json!([null, 3])
        );
        let attached = Event::new(Actor::User, Action::Attach, Target::Attachment);
        assert!(json(&attached).get("changes").is_none());
        assert_eq!(
            json(&Event::new(Actor::Model, Action::Label, Target::Note))["actor"],
            "model"
        );
    }

    #[test]
    fn only_a_valid_plugin_id_speaks_for_a_plugin() {
        assert_eq!(
            Actor::from_plugin(Some("word-count".to_string())),
            Actor::Plugin("word-count".to_string())
        );
        assert_eq!(Actor::from_plugin(None), Actor::User);
        assert_eq!(
            Actor::from_plugin(Some("../model".to_string())),
            Actor::User
        );
    }

    #[test]
    fn compare_names_what_a_reader_would_see() {
        let before = entry("01AAA", Kind::Note);

        let edited = IndexEntry {
            hash: "e5f6a7b8".to_string(),
            words: Some(50),
            ..before.clone()
        };
        let (action, changes) = compare(&before, &edited).unwrap();
        assert_eq!(action, Action::Edit);
        assert_eq!(changes["words"], (Value::from(42), Value::from(50)));

        let labelled = IndexEntry {
            category: Some("home".to_string()),
            ..before.clone()
        };
        let (action, changes) = compare(&before, &labelled).unwrap();
        assert_eq!(action, Action::Label);
        assert_eq!(changes.keys().copied().collect::<Vec<_>>(), ["category"]);

        let moved = IndexEntry {
            date: "2026-09-23".to_string(),
            file: "notes/2026/2026-09-23.md".to_string(),
            ..before.clone()
        };
        assert_eq!(compare(&before, &moved).unwrap().0, Action::Move);

        let page = entry("01PPP", Kind::Page);
        let retitled = IndexEntry {
            subject: Some("Retro".to_string()),
            ..page.clone()
        };
        let (action, changes) = compare(&page, &retitled).unwrap();
        assert_eq!(action, Action::Rename);
        assert!(changes.contains_key("title"));

        let only_status = IndexEntry {
            status: Status::Pending,
            ..before.clone()
        };
        assert!(compare(&before, &only_status).is_none());
    }

    #[test]
    fn a_note_moved_between_days_outside_the_app_is_one_event() {
        let old = entry("01AAA", Kind::Note);
        let new = IndexEntry {
            date: "2026-09-23".to_string(),
            file: "notes/2026/2026-09-23.md".to_string(),
            ..old.clone()
        };
        let gone = entry("01BBB", Kind::Note);
        let added = entry("01CCC", Kind::Note);
        // The new day before the old one, as a batch of paths may come.
        let events = external(vec![
            (None, Some(new)),
            (Some(old), None),
            (Some(gone), None),
            (None, Some(added)),
        ]);
        let what: Vec<(String, Action)> = events
            .iter()
            .map(|e| (e.id.clone().unwrap(), e.action))
            .collect();
        assert_eq!(
            what,
            [
                ("01AAA".to_string(), Action::Move),
                ("01BBB".to_string(), Action::Delete),
                ("01CCC".to_string(), Action::Create),
            ]
        );
        assert!(events.iter().all(|e| e.actor == Actor::External));
        assert_eq!(events[0].date.as_deref(), Some("2026-09-23"));
    }

    #[tokio::test]
    async fn appends_a_line_per_event_in_the_file_of_its_day() {
        let root = scratch("append");
        for (date, action) in [
            ("2026-10-01", Action::Create),
            ("2026-10-01", Action::Edit),
            ("2026-10-02", Action::Delete),
        ] {
            let event = Event {
                at: at(date),
                ..Event::new(Actor::User, action, Target::Note)
            };
            append(&root, &event).await.unwrap();
        }
        let first = std::fs::read_to_string(dir(&root).join("2026/2026-10-01.jsonl")).unwrap();
        let actions: Vec<String> = first
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap()["action"].to_string())
            .collect();
        assert_eq!(actions, ["\"create\"", "\"edit\""]);
        assert!(dir(&root).join("2026/2026-10-02.jsonl").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_line_cut_short_does_not_swallow_the_next() {
        let root = scratch("cut");
        let path = day_path(&root, at("2026-10-01").date_naive());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{\"at\":\"2026-10-01T09:").unwrap();
        let event = Event {
            at: at("2026-10-01"),
            ..Event::new(Actor::User, Action::Create, Target::Note)
        };
        append(&root, &event).await.unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        let last = contents.lines().last().unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(last).unwrap()["action"],
            "create"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn reads_back_newest_first_a_page_at_a_time() {
        let root = scratch("read");
        let events = [
            ("2026-09-30", Actor::User, Action::Create),
            ("2026-10-01", Actor::Model, Action::Label),
            (
                "2026-10-01",
                Actor::Plugin("tasks".to_string()),
                Action::Tick,
            ),
            ("2026-10-01", Actor::User, Action::Edit),
        ];
        for (date, actor, action) in events {
            let event = Event {
                at: at(date),
                ..Event::new(actor, action, Target::Note)
            };
            append(&root, &event).await.unwrap();
        }
        // A line cut short, and a day past keeping, are passed over.
        let today = day_path(&root, at("2026-10-01").date_naive());
        let mut contents = std::fs::read_to_string(&today).unwrap();
        contents.push_str("{\"at\":\"2026-10-01T23:");
        std::fs::write(&today, contents).unwrap();
        let old = day_path(&root, at("2026-01-01").date_naive());
        std::fs::create_dir_all(old.parent().unwrap()).unwrap();
        std::fs::write(
            &old,
            serde_json::to_string(&Event::new(Actor::User, Action::Delete, Target::Note)).unwrap(),
        )
        .unwrap();

        let on = at("2026-10-01").date_naive();
        let actions = |page: &[Logged]| page.iter().map(|e| e.action.clone()).collect::<Vec<_>>();
        let (first, more) = read(&root, on, None, 0, 2);
        assert_eq!(actions(&first), ["edit", "tick"]);
        assert!(more);
        let (rest, more) = read(&root, on, None, 2, 2);
        assert_eq!(actions(&rest), ["label", "create"]);
        assert!(!more, "the old day is past keeping");
        assert_eq!(rest[1].at[..10], *"2026-09-30");

        let (plugins, _) = read(&root, on, Some(Whose::Plugin), 0, 10);
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].actor, "plugin:tasks");
        let (users, _) = read(&root, on, Some(Whose::User), 1, 10);
        assert_eq!(actions(&users), ["create"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn the_first_event_of_a_day_removes_the_days_past_keeping() {
        let root = scratch("prune");
        // On 2026-10-02, 2026-07-04 is 90 days back: the 91st day.
        let keep = ["2026-07-05", "2026-10-01"];
        let gone = ["2025-12-31", "2026-07-04"];
        for date in keep.iter().chain(&gone) {
            let path = day_path(&root, NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "{}\n").unwrap();
        }
        std::fs::write(dir(&root).join("2026/notes.txt"), "mine").unwrap();

        let event = Event {
            at: at("2026-10-01"),
            ..Event::new(Actor::User, Action::Create, Target::Note)
        };
        // Today's file is there already, so nothing is pruned yet.
        append(&root, &event).await.unwrap();
        assert!(dir(&root).join("2026/2026-07-04.jsonl").exists());

        let tomorrow = Event {
            at: at("2026-10-02"),
            ..event
        };
        append(&root, &tomorrow).await.unwrap();
        assert!(dir(&root).join("2026/2026-07-05.jsonl").exists());
        assert!(!dir(&root).join("2026/2026-07-04.jsonl").exists());
        assert!(!dir(&root).join("2025").exists(), "an emptied year goes");
        assert!(dir(&root).join("2026/notes.txt").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
