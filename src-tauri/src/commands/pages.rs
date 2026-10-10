//! Pages, SPEC 3.5 and 4.7: the page view and the list of pages. How a
//! page's file and stub are kept is `crate::pages`.

use tauri::{AppHandle, State};
use ulid::Ulid;

use super::notes::{free_id, move_target, read_note};
use crate::events;
use crate::pages::{
    drop_stub, entry, free_path, newest_first, read_page, reindex, sync_stub, write_new,
};
use crate::state::{read_lock, AppState};
use crate::storage::daily_file::{self, body_hash, Kind, Note, Stub};
use crate::storage::page_file;
use crate::storage::{check_date, date_and_time, day_path};
use crate::Result;

const NO_TITLE: &str = "a page needs a title";

/// Start a page on a day, today's unless another is given, at the current
/// time. The file is written first, then the stub (SPEC 4.7). It is made in
/// the page view, so it is held until `finish_page`.
#[tauri::command]
pub async fn create_page(
    app: AppHandle,
    state: State<'_, AppState>,
    title: String,
    body: String,
    date: Option<String>,
) -> Result<Note> {
    let title = page_file::clean_title(&title).ok_or(NO_TITLE)?;
    if let Some(date) = &date {
        check_date(date)?;
    }
    let space = state.space()?;
    let (today, time) = date_and_time();
    let date = date.unwrap_or(today);
    let body = body.trim().to_string();
    let page = Note {
        id: Ulid::generate().to_string(),
        file: free_path(&space, &date, &title, None),
        time,
        hash: body_hash(&body),
        on: page_file::day_ahead(&title, &body, &date),
        ahead_off: false,
        date,
        subject: Some(title),
        body,
        kind: Kind::Page,
        missing: false,
    };

    space.hold(&page.id);
    write_new(&state.writer, &space, &page).await?;
    space.page_changed(&page)?;
    space.persist_index(&state.writer).await?;
    sync_stub(&state.writer, &space, &page).await?;

    events::note_updated(&app, &page.id);
    Ok(page)
}

/// Every page of the open space, for the list of pages (SPEC 3.5).
#[tauri::command]
pub async fn list_pages(state: State<'_, AppState>) -> Result<Vec<Note>> {
    let space = state.space()?;
    let mut pages = {
        let index = read_lock(&space.index, "index")?;
        newest_first(&index)
    };
    space.fill_bodies(&mut pages);
    Ok(pages)
}

#[tauri::command]
pub async fn get_page(state: State<'_, AppState>, id: String) -> Result<Note> {
    let space = state.space()?;
    read_page(&space, &id).await
}

/// Replace a page's text, as the page view autosaves it. It is embedded
/// once the view closes (`finish_page`). An empty text is allowed: the title
/// is still there.
#[tauri::command]
pub async fn update_page(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    body: String,
) -> Result<Note> {
    let space = state.space()?;
    space.hold(&id);
    let body = body.trim().to_string();
    let current = read_page(&space, &id).await?;
    if current.body == body {
        return Ok(current);
    }

    let file = current.file.clone();
    let written = {
        let file = file.clone();
        state
            .writer
            .rewrite(space.root.join(&file), move |existing| {
                page_file::replace_body(existing?, &file, &body)
            })
            .await?
    };
    if !written {
        return Err(format!("{file} no longer holds page {id}").into());
    }
    let page = reindex(&space, &file)?.ok_or_else(|| format!("{file} is gone"))?;
    space.persist_index(&state.writer).await?;

    events::note_updated(&app, &id);
    Ok(page)
}

/// The page view closed: the page is released, and the embed task is woken
/// for it. A page only read was not held and needs nothing.
#[tauri::command]
pub async fn finish_page(state: State<'_, AppState>, id: String) -> Result<()> {
    let space = state.space()?;
    if space.release(&id) {
        space.index_changed();
    }
    Ok(())
}

/// Retitle a page, which renames its file and rewrites its stub. It happens
/// in the page view, so the page is held.
#[tauri::command]
pub async fn rename_page(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> Result<Note> {
    let title = page_file::clean_title(&title).ok_or(NO_TITLE)?;
    let space = state.space()?;
    space.hold(&id);
    let current = read_page(&space, &id).await?;
    if current.subject.as_deref() == Some(title.as_str()) {
        return Ok(current);
    }

    let file = free_path(&space, &current.date, &title, Some(&current.file));
    if file != current.file {
        state
            .writer
            .rename(space.root.join(&current.file), space.root.join(&file))
            .await?;
    }
    {
        let (file, title) = (file.clone(), title.clone());
        state
            .writer
            .rewrite(space.root.join(&file), move |existing| {
                page_file::set_title(existing?, &file, &title)
            })
            .await?;
    }
    let page = reindex(&space, &file)?.ok_or_else(|| format!("{file} is gone"))?;
    space.persist_index(&state.writer).await?;
    sync_stub(&state.writer, &space, &page).await?;

    events::note_updated(&app, &id);
    Ok(page)
}

/// Delete a page: its stub first, then its file, so a crash between the two
/// leaves a page the next launch gives a stub again rather than a stub whose
/// text is gone. `date` is the day showing it, which is how a stub whose
/// page is missing is removed too.
#[tauri::command]
pub async fn delete_page(
    app: AppHandle,
    state: State<'_, AppState>,
    date: String,
    id: String,
) -> Result<()> {
    check_date(&date)?;
    let space = state.space()?;
    space.release(&id);
    let page = entry(&space, &id).ok();

    drop_stub(&state.writer, &space, &date, &id).await?;
    if let Some(page) = page {
        if page.date != date {
            drop_stub(&state.writer, &space, &page.date, &id).await?;
        }
        state.writer.remove(space.root.join(&page.file)).await?;
        space.page_removed(&id)?;
        space.persist_index(&state.writer).await?;
    }

    events::note_updated(&app, &id);
    Ok(())
}

/// Move a page of the open space to another, onto the same day at the same
/// time, with its title and the files it links (SPEC 3.5). Its file and stub
/// are written there before they leave here, stub first as for a delete, so
/// a crash in between leaves it in both spaces, never in neither. `date` is
/// the day showing it, as for `delete_page`. The page view closes on it, so
/// it is released.
#[tauri::command]
pub async fn move_page(
    app: AppHandle,
    state: State<'_, AppState>,
    date: String,
    id: String,
    space: String,
) -> Result<()> {
    check_date(&date)?;
    let from = state.space()?;
    let to = move_target(&state, &space)?;
    let page = read_page(&from, &id).await?;
    let title = page.subject.clone().unwrap_or_default();

    let (body, carried) = crate::attachments::carry_async(&from.root, &to.root, &page.body).await?;
    let moved = Note {
        id: free_id(&to, &page.date, &id).await,
        file: free_path(&to, &page.date, &title, None),
        hash: body_hash(&body),
        body,
        ..page.clone()
    };
    write_new(&state.writer, &to, &moved).await?;
    to.page_changed(&moved)?;
    to.persist_index(&state.writer).await?;
    sync_stub(&state.writer, &to, &moved).await?;

    drop_stub(&state.writer, &from, &date, &id).await?;
    if page.date != date {
        drop_stub(&state.writer, &from, &page.date, &id).await?;
    }
    state.writer.remove(from.root.join(&page.file)).await?;
    from.page_removed(&id)?;
    from.persist_index(&state.writer).await?;
    from.release(&id);
    crate::attachments::drop_carried(&from, &carried);

    events::note_updated(&app, &id);
    Ok(())
}

/// Turn a note into a page with the same day, time and text. The page gets a
/// new id and is written first; then the note's block becomes its stub, so a
/// crash between the two leaves the text twice rather than nowhere. The page
/// view opens on it next, so it is held.
#[tauri::command]
pub async fn note_to_page(
    app: AppHandle,
    state: State<'_, AppState>,
    date: String,
    id: String,
    title: String,
) -> Result<Note> {
    let title = page_file::clean_title(&title).ok_or(NO_TITLE)?;
    check_date(&date)?;
    let space = state.space()?;
    let note = read_note(&space, &date, &id).await?;

    let page = Note {
        id: Ulid::generate().to_string(),
        file: free_path(&space, &note.date, &title, None),
        on: (!note.ahead_off)
            .then(|| page_file::day_ahead(&title, &note.body, &note.date))
            .flatten(),
        subject: Some(title),
        kind: Kind::Page,
        missing: false,
        ..note
    };
    space.hold(&page.id);
    write_new(&state.writer, &space, &page).await?;

    let stub = Stub::for_page(&page);
    let note_id = id.clone();
    let replaced = state
        .writer
        .rewrite(day_path(&space.root, &date), move |existing| {
            daily_file::note_to_stub(existing?, &note_id, &stub)
        })
        .await?;
    if !replaced {
        // The note went meanwhile. The page stays, with a stub of its own.
        sync_stub(&state.writer, &space, &page).await?;
    }
    space.day_changed(&date)?;
    space.page_changed(&page)?;
    space.persist_index(&state.writer).await?;

    events::note_updated(&app, &id);
    events::note_updated(&app, &page.id);
    Ok(page)
}
