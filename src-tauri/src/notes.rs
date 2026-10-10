//! Notes, SPEC 3.1 to 3.3: what capturing, editing, deleting and moving a
//! note does to its day's file and to the index. The commands in
//! `commands::notes` check what they are given, call these, and tell the
//! windows.

use std::sync::Arc;

use ulid::Ulid;

use crate::commands::blocking;
use crate::spaces::Space;
use crate::state::{read_lock, AppState};
use crate::storage::daily_file::{self, Kind, Note, Stub};
use crate::storage::index::{self, IndexEntry};
use crate::storage::writer::Writer;
use crate::storage::{date_and_time, day_path, page_file, relative_day_path};
use crate::Result;

/// Append a note with `body`, trimmed and not empty, to a day's file,
/// today's unless `date` is given, at the current time either way.
pub async fn add(
    writer: &Writer,
    space: &Space,
    body: String,
    date: Option<String>,
) -> Result<Note> {
    let (today, time) = date_and_time();
    let date = date.unwrap_or(today);
    let note = Note {
        id: Ulid::generate().to_string(),
        time,
        file: relative_day_path(&date),
        subject: None,
        hash: daily_file::body_hash(&body),
        on: crate::ahead::day_ahead(&body, &date),
        ahead_off: false,
        body,
        date: date.clone(),
        kind: Kind::Note,
        missing: false,
    };
    writer
        .append_note(day_path(&space.root, &date), date, note.clone())
        .await?;
    space.note_added(&note)?;
    Ok(note)
}

/// The notes and pages of a day, by time. The notes are read straight from
/// the markdown, because the index deliberately carries no bodies and the
/// day view shows them. The day's pages come from the index, with their
/// text from search.db, and a stub whose page is gone comes back as a
/// missing page (SPEC 3.5).
pub async fn day(space: &Space, date: &str) -> Result<Vec<Note>> {
    let path = day_path(&space.root, date);
    let contents = match tokio::fs::read_to_string(&path).await {
        Ok(contents) => contents,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    let mut notes = daily_file::parse_notes(&contents, date, &relative_day_path(date));
    let mut pages: Vec<Note> = {
        let idx = read_lock(&space.index, "index")?;
        for stub in daily_file::parse_stubs(&contents) {
            if idx.page(&stub.id).is_none() {
                notes.push(missing_page(date, stub));
            }
        }
        idx.pages_on(date).map(IndexEntry::to_note).collect()
    };
    space.fill_bodies(&mut pages);
    notes.extend(pages);
    notes.sort_by(|a, b| a.time.cmp(&b.time));
    Ok(notes)
}

/// What the day view shows for a stub whose page file is gone.
fn missing_page(date: &str, stub: Stub) -> Note {
    Note {
        id: stub.id,
        date: date.to_string(),
        time: stub.time,
        file: stub.target,
        subject: Some(stub.title).filter(|t| !t.is_empty()),
        hash: String::new(),
        on: None,
        ahead_off: false,
        body: String::new(),
        kind: Kind::Page,
        missing: true,
    }
}

/// A note of a day as its file has it now.
pub async fn read_note(space: &Space, date: &str, id: &str) -> Result<Note> {
    let contents = tokio::fs::read_to_string(day_path(&space.root, date)).await?;
    daily_file::parse_notes(&contents, date, &relative_day_path(date))
        .into_iter()
        .find(|note| note.id == id)
        .ok_or_else(|| format!("no note {id} in {date}").into())
}

/// Remove a note from its day's file. A delete rewrites the day file, so
/// the day's entries are reparsed (SPEC 4.4).
pub async fn delete(writer: &Writer, space: &Space, date: &str, id: &str) -> Result<()> {
    let path = day_path(&space.root, date);
    if !writer.delete_note(path, id.to_string()).await? {
        return Err(format!("no note {id} in {date}").into());
    }
    reindex_day(space, date)
}

/// Swap a note's body for `body`, trimmed and not empty. Returns the note
/// as it is then, and whether it changed: a body the note already has is
/// not written again.
pub async fn update(
    writer: &Writer,
    space: &Space,
    date: &str,
    id: &str,
    body: String,
) -> Result<(Note, bool)> {
    let current = read_note(space, date, id).await?;
    if current.body == body {
        return Ok((current, false));
    }
    let path = day_path(&space.root, date);
    if !writer.replace_body(path, id.to_string(), body).await? {
        return Err(format!("no note {id} in {date}").into());
    }
    reindex_day(space, date)?;
    Ok((read_note(space, date, id).await?, true))
}

/// The space a note or a page of the open space moves to: any other.
pub fn move_target(state: &AppState, name: &str) -> Result<Arc<Space>> {
    let to = state.space_or_open(Some(name))?;
    if Arc::ptr_eq(&to, &state.space()?) {
        return Err(format!("it is already in {name}").into());
    }
    Ok(to)
}

/// The id a note or a page moving to `space` onto `date` takes there: its
/// own, unless that day already holds it, as a move cut short by a crash
/// leaves it in both spaces.
pub async fn free_id(space: &Space, date: &str, id: &str) -> String {
    let contents = tokio::fs::read_to_string(day_path(&space.root, date))
        .await
        .unwrap_or_default();
    let taken = daily_file::parse_notes(&contents, date, "")
        .iter()
        .any(|note| note.id == id)
        || daily_file::parse_stubs(&contents)
            .iter()
            .any(|stub| stub.id == id);
    if taken {
        Ulid::generate().to_string()
    } else {
        id.to_string()
    }
}

/// Move a note from the space at `from` to the one at `to`, onto the same
/// day at the same time, with the files it links (SPEC 3.2). It is written
/// there before it leaves here, so a crash in between leaves it in both
/// spaces, never in neither.
pub async fn move_to(
    writer: &Writer,
    from: &Space,
    to: &Space,
    date: &str,
    id: &str,
) -> Result<()> {
    let note = read_note(from, date, id).await?;
    let (body, carried) = crate::attachments::carry_async(&from.root, &to.root, &note.body).await?;
    let moved = Note {
        id: free_id(to, date, id).await,
        hash: daily_file::body_hash(&body),
        body,
        ..note
    };
    writer
        .append_note(day_path(&to.root, date), date.to_string(), moved.clone())
        .await?;
    to.note_added(&moved)?;

    writer
        .delete_note(day_path(&from.root, date), id.to_string())
        .await?;
    reindex_day(from, date)?;
    crate::attachments::drop_carried(from, &carried);
    Ok(())
}

/// Forget the day ahead read in a note or a page (SPEC 5.3), for one read
/// wrong. No day is read in it again, even once its text changes.
pub async fn clear_day_ahead(writer: &Writer, space: &Space, date: &str, id: &str) -> Result<()> {
    if is_page(space, id) {
        return crate::pages::clear_day_ahead(writer, space, id).await;
    }
    let path = day_path(&space.root, date);
    let cleared = id.to_string();
    if !writer
        .rewrite(path, move |existing| {
            daily_file::clear_day_ahead(existing?, &cleared)
        })
        .await?
    {
        return Err(format!("no note {id} in {date}").into());
    }
    reindex_day(space, date)
}

fn is_page(space: &Space, id: &str) -> bool {
    space.index.read().is_ok_and(|idx| idx.page(id).is_some())
}

/// After rewriting a day file: reparse the day.
fn reindex_day(space: &Space, date: &str) -> Result<()> {
    space.day_changed(date)?;
    space.index_changed();
    Ok(())
}

/// Write the notes and pages a model labelled as they are written now, then
/// reparse every markdown file and replace the cache. Returns how many notes
/// the rebuilt index holds.
pub async fn rebuild(writer: &Writer, space: &Arc<Space>) -> Result<usize> {
    drop_labels(writer, space).await?;
    let rebuilding = space.clone();
    let count = blocking(move || rebuilding.rebuild()).await??;
    space.index_changed();
    Ok(count)
}

/// Write every block and page of the space that still carries the labels a
/// model wrote as it is written now, without them (SPEC 4.3). The rest of
/// each file is left as it is.
async fn drop_labels(writer: &Writer, space: &Space) -> Result<()> {
    for (_, path) in index::daily_files(&space.root) {
        writer
            .rewrite(path, |existing| daily_file::drop_labels(existing?))
            .await?;
    }
    for path in index::page_files(&space.root) {
        let Some(file) = index::relative(&space.root, &path) else {
            continue;
        };
        writer
            .rewrite(path, move |existing| {
                page_file::drop_labels(existing?, &file)
            })
            .await?;
    }
    Ok(())
}
