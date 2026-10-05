//! What the user pinned to the left edge of a space (SPEC 3.13), kept in
//! `space.db` beside the thread edits, and like them never derived.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::storage::space_db::{to_string, SpaceDb};

/// The most a space holds, so the edge never needs a scrollbar of its own.
pub const MAX_PINS: usize = 24;
/// The longest target or label kept, in characters.
const MAX_TEXT: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PinKind {
    /// A thread, by its id. Its title is read live, so none is kept.
    Thread,
    /// A plugin's page, by its type and query, such as `mentions?name=bob`.
    View,
}

impl PinKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Thread => "thread",
            Self::View => "view",
        }
    }

    fn parse(kind: &str) -> Option<Self> {
        match kind {
            "thread" => Some(Self::Thread),
            "view" => Some(Self::View),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pin {
    pub kind: PinKind,
    pub target: String,
    /// The title the view had when pinned, for a view whose title is not
    /// known before it opens.
    #[serde(default)]
    pub label: Option<String>,
}

/// The pins in their order. A kind a later version added is passed over,
/// and pins that cannot be read pin nothing.
pub fn load(db: &SpaceDb) -> Vec<Pin> {
    read(db.conn()).unwrap_or_else(|e| {
        log::warn!("could not read the pins: {e}");
        Vec::new()
    })
}

fn read(conn: &Connection) -> rusqlite::Result<Vec<Pin>> {
    let mut statement = conn.prepare("SELECT kind, target, label FROM pins ORDER BY position")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;
    let mut pins = Vec::new();
    for row in rows {
        let (kind, target, label) = row?;
        if let Some(kind) = PinKind::parse(&kind) {
            pins.push(Pin {
                kind,
                target,
                label,
            });
        }
    }
    Ok(pins)
}

/// `pins` tidied: blank ones dropped, text cut to length, each pinned once,
/// at most `MAX_PINS`.
pub fn tidy(pins: Vec<Pin>) -> Vec<Pin> {
    let cut = |text: &str| text.trim().chars().take(MAX_TEXT).collect::<String>();
    let mut kept: Vec<Pin> = Vec::new();
    for pin in pins {
        let target = cut(&pin.target);
        if target.is_empty()
            || kept
                .iter()
                .any(|other| other.kind == pin.kind && other.target == target)
        {
            continue;
        }
        let label = pin
            .label
            .as_deref()
            .map(cut)
            .filter(|label| !label.is_empty());
        kept.push(Pin {
            kind: pin.kind,
            target,
            label,
        });
        if kept.len() == MAX_PINS {
            break;
        }
    }
    kept
}

/// Save these in place of the pins saved before.
pub fn save(db: &mut SpaceDb, pins: &[Pin]) -> Result<(), String> {
    db.transaction(|tx| {
        tx.execute("DELETE FROM pins", []).map_err(to_string)?;
        let mut statement = tx
            .prepare_cached(
                "INSERT INTO pins (position, kind, target, label) VALUES (?1, ?2, ?3, ?4)",
            )
            .map_err(to_string)?;
        for (position, pin) in pins.iter().enumerate() {
            statement
                .execute(rusqlite::params![
                    position as i64,
                    pin.kind.as_str(),
                    pin.target,
                    pin.label
                ])
                .map_err(to_string)?;
        }
        Ok(())
    })
}

/// Point the pins of thread `from` at thread `into`, as merging the one
/// into the other does. Says whether any changed.
pub fn follow_merge(pins: &mut Vec<Pin>, from: &str, into: &str) -> bool {
    let at = |id: &str, pins: &[Pin]| {
        pins.iter()
            .position(|pin| pin.kind == PinKind::Thread && pin.target == id)
    };
    let Some(old) = at(from, pins) else {
        return false;
    };
    if at(into, pins).is_some() {
        pins.remove(old);
    } else {
        pins[old].target = into.to_string();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thread(id: &str) -> Pin {
        Pin {
            kind: PinKind::Thread,
            target: id.into(),
            label: None,
        }
    }

    fn view(target: &str, label: &str) -> Pin {
        Pin {
            kind: PinKind::View,
            target: target.into(),
            label: Some(label.into()),
        }
    }

    #[test]
    fn pins_come_back_in_their_order() {
        let mut db = SpaceDb::in_memory().unwrap();
        let pins = vec![
            view("mentions?name=bob", "@bob"),
            thread("01B"),
            thread("01A"),
        ];
        save(&mut db, &pins).unwrap();
        assert_eq!(load(&db), pins);
        save(&mut db, &pins[1..]).unwrap();
        assert_eq!(load(&db), pins[1..]);
    }

    #[test]
    fn a_kind_from_a_later_version_is_passed_over() {
        let mut db = SpaceDb::in_memory().unwrap();
        save(&mut db, &[thread("01A")]).unwrap();
        db.conn()
            .execute(
                "INSERT INTO pins (position, kind, target) VALUES (5, 'search', 'kitchen')",
                [],
            )
            .unwrap();
        assert_eq!(load(&db), [thread("01A")]);
    }

    #[test]
    fn tidying_drops_blanks_and_twins_and_keeps_the_first() {
        let pins = vec![
            thread("01A"),
            thread(" "),
            view(" mentions?name=bob ", "  "),
            thread("01A"),
            view("mentions?name=bob", "@bob"),
        ];
        assert_eq!(
            tidy(pins),
            [
                thread("01A"),
                Pin {
                    kind: PinKind::View,
                    target: "mentions?name=bob".into(),
                    label: None
                }
            ]
        );
        let many = (0..40).map(|n| thread(&format!("{n:03}"))).collect();
        assert_eq!(tidy(many).len(), MAX_PINS);
    }

    #[test]
    fn a_merged_thread_stays_pinned_as_the_one_it_went_into() {
        let mut pins = vec![thread("01A"), view("mentions", "Mentions")];
        assert!(follow_merge(&mut pins, "01A", "01B"));
        assert_eq!(pins, [thread("01B"), view("mentions", "Mentions")]);
        // Both pinned: the one merged away goes.
        let mut pins = vec![thread("01A"), thread("01B")];
        assert!(follow_merge(&mut pins, "01A", "01B"));
        assert_eq!(pins, [thread("01B")]);
        assert!(!follow_merge(&mut pins, "01C", "01B"));
    }
}
