//! Pages, SPEC 3.5 and 4.7: notes with a title and a markdown file of their
//! own, shown on their day through a stub in the day's file.
//!
//! The page file is the source of truth. The stub is written from it, and
//! rewritten from it whenever it goes stale; nothing is ever read back from
//! a stub but its id and time.
//!
//! A page open in the editor is held: the page view saves it every pause in
//! typing, and neither model runs on it until `finish_page` says the view
//! closed (SPEC 3.5).

use std::collections::HashMap;

use chrono::Local;
use tauri::{AppHandle, Emitter, State};
use ulid::Ulid;

use crate::commands::notes::{enqueue, persist_queue, read_note};
use crate::spaces::Space;
use crate::state::AppState;
use crate::storage::daily_file::{self, body_hash, Kind, Note, Status, Stub};
use crate::storage::index::{self, IndexEntry};
use crate::storage::page_file;
use crate::storage::writer::Writer;
use crate::storage::{check_date, day_path};

const NO_TITLE: &str = "a page needs a title";

fn lock_poisoned<T>(_: T) -> String {
    "index lock poisoned".to_string()
}

/// The page's entry in the index, or why there is none.
fn entry(space: &Space, id: &str) -> Result<IndexEntry, String> {
    space
        .index
        .read()
        .map_err(lock_poisoned)?
        .page(id)
        .cloned()
        .ok_or_else(|| format!("no page {id}"))
}

/// A page read afresh from its file.
pub async fn read_page(space: &Space, id: &str) -> Result<Note, String> {
    let entry = entry(space, id)?;
    let contents = tokio::fs::read_to_string(space.root.join(&entry.file))
        .await
        .map_err(|e| format!("could not read {}: {e}", entry.file))?;
    page_file::parse_page(&contents, &entry.file)
        .filter(|page| page.id == id)
        .ok_or_else(|| format!("{} no longer holds page {id}", entry.file))
}

/// Reparse a page's file into the index. `None` when it is gone or no
/// longer a page, which leaves the index as it was.
fn reindex(space: &Space, file: &str) -> Result<Option<Note>, String> {
    let page = std::fs::read_to_string(space.root.join(file))
        .ok()
        .and_then(|contents| page_file::parse_page(&contents, file));
    if let Some(page) = &page {
        space
            .index
            .write()
            .map_err(lock_poisoned)?
            .replace_page(IndexEntry::from(page));
    }
    Ok(page)
}

/// A path for a page of `date` called `title` that no other file has taken.
/// `current` is the page's own file, which it may keep, in any case.
fn free_path(space: &Space, date: &str, title: &str, current: Option<&str>) -> String {
    let current = current.map(str::to_lowercase);
    (1..)
        .map(|n| page_file::relative_path(date, &page_file::file_name(date, title, n)))
        .find(|rel| {
            current.as_deref() == Some(rel.to_lowercase().as_str())
                || !space.root.join(rel).exists()
        })
        .expect("some numbered name is free")
}

/// Give the page's day the stub it should have: added when there is none,
/// rewritten when it is stale, left alone when it is right.
pub async fn sync_stub(writer: &Writer, space: &Space, page: &Note) -> Result<(), String> {
    if space.is_retired() {
        return Ok(());
    }
    let wanted = Stub::for_page(page);
    let date = page.date.clone();
    writer
        .rewrite(day_path(&space.root, &page.date), move |existing| {
            let content = existing.unwrap_or("");
            match daily_file::parse_stubs(content)
                .into_iter()
                .find(|stub| stub.id == wanted.id)
            {
                Some(stub) if stub == wanted => None,
                Some(_) => daily_file::replace_stub(content, &wanted),
                None => Some(daily_file::append_stub(content, &wanted, &date)),
            }
        })
        .await?;
    Ok(())
}

/// Take a page's stub out of a day's file. False when it was not there.
pub async fn drop_stub(
    writer: &Writer,
    space: &Space,
    date: &str,
    id: &str,
) -> Result<bool, String> {
    if space.is_retired() {
        return Ok(false);
    }
    let id = id.to_string();
    writer
        .rewrite(day_path(&space.root, date), move |existing| {
            daily_file::remove_stub(existing?, &id)
        })
        .await
}

/// SPEC 4.7, at startup: every page gets its stub, a stale one is rewritten
/// and one on another day goes. A stub whose page is gone is kept, since a
/// sync tool may not have brought the file yet.
pub async fn repair_stubs(writer: &Writer, space: &Space) {
    let pages: Vec<Note> = match space.index.read() {
        Ok(idx) => idx.pages().map(IndexEntry::to_note).collect(),
        Err(_) => return,
    };
    if pages.is_empty() {
        return;
    }
    let root = space.root.clone();
    let stubs = tauri::async_runtime::spawn_blocking(move || {
        let mut days: HashMap<String, Vec<String>> = HashMap::new();
        for (date, path) in index::daily_files(&root) {
            let Ok(contents) = std::fs::read_to_string(&path) else {
                continue;
            };
            for stub in daily_file::parse_stubs(&contents) {
                days.entry(stub.id).or_default().push(date.clone());
            }
        }
        days
    })
    .await
    .unwrap_or_default();

    for page in &pages {
        if space.is_retired() {
            return;
        }
        for date in stubs.get(&page.id).into_iter().flatten() {
            if *date != page.date {
                if let Err(e) = drop_stub(writer, space, date, &page.id).await {
                    log::warn!("could not take page {} off {date}: {e}", page.id);
                }
            }
        }
        if let Err(e) = sync_stub(writer, space, page).await {
            log::warn!("could not write the stub of page {}: {e}", page.id);
        }
    }
}

/// Write a new page's file, refusing to replace one that appeared meanwhile.
async fn write_new(writer: &Writer, space: &Space, page: &Note) -> Result<(), String> {
    let rendered = page_file::render_page(page);
    let written = writer
        .rewrite(space.root.join(&page.file), move |existing| {
            existing.is_none().then_some(rendered)
        })
        .await?;
    if !written {
        return Err(format!("{} is already there", page.file));
    }
    Ok(())
}

fn emit_updated(app: &AppHandle, id: &str) {
    let _ = app.emit("note-updated", serde_json::json!({ "id": id }));
}

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
) -> Result<Note, String> {
    let title = page_file::clean_title(&title).ok_or(NO_TITLE)?;
    if let Some(date) = &date {
        check_date(date)?;
    }
    let space = state.space()?;
    let now = Local::now();
    let date = date.unwrap_or_else(|| now.format("%Y-%m-%d").to_string());
    let body = body.trim().to_string();
    let page = Note {
        id: Ulid::generate().to_string(),
        file: free_path(&space, &date, &title, None),
        date,
        time: now.format("%H:%M").to_string(),
        subject: Some(title),
        category: None,
        status: Status::Pending,
        hash: body_hash(&body),
        lang: None,
        body,
        kind: Kind::Page,
        missing: false,
    };

    space.hold(&page.id);
    write_new(&state.writer, &space, &page).await?;
    space
        .index
        .write()
        .map_err(lock_poisoned)?
        .replace_page(IndexEntry::from(&page));
    space.persist_index(&state.writer).await?;
    sync_stub(&state.writer, &space, &page).await?;

    emit_updated(&app, &page.id);
    Ok(page)
}

#[tauri::command]
pub async fn get_page(state: State<'_, AppState>, id: String) -> Result<Note, String> {
    let space = state.space()?;
    read_page(&space, &id).await
}

/// Replace a page's text, as the page view autosaves it. A changed text
/// marks the page pending, unless the user set the category by hand, but it
/// only goes to the model once the view closes (`finish_page`). Pending in
/// the file, it is queued at the next launch should the app quit first. An
/// empty text is allowed: the title is still there.
#[tauri::command]
pub async fn update_page(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    body: String,
) -> Result<Note, String> {
    let space = state.space()?;
    space.hold(&id);
    let body = body.trim().to_string();
    let current = read_page(&space, &id).await?;
    if current.body == body {
        return Ok(current);
    }

    let relabel = current.status != Status::Manual;
    let file = current.file.clone();
    let written = {
        let file = file.clone();
        state
            .writer
            .rewrite(space.root.join(&file), move |existing| {
                page_file::replace_body(existing?, &file, &body, relabel.then_some(Status::Pending))
            })
            .await?
    };
    if !written {
        return Err(format!("{file} no longer holds page {id}"));
    }
    let page = reindex(&space, &file)?.ok_or_else(|| format!("{file} is gone"))?;
    space.persist_index(&state.writer).await?;

    emit_updated(&app, &id);
    Ok(page)
}

/// The page view closed: the page is released, and goes to the model when
/// its text changed while it was open. The embed task is woken for it too.
/// A page only read, or re-run by hand since its last change, was not held
/// and needs nothing. A page deleted meanwhile is only released.
#[tauri::command]
pub async fn finish_page(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let space = state.space()?;
    if !space.release(&id) {
        return Ok(());
    }
    let pending = entry(&space, &id)
        .ok()
        .filter(|page| page.status == Status::Pending);
    if let Some(page) = pending {
        enqueue(&state, &space, id, page.date).await;
    }
    space.index_changed();
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
) -> Result<Note, String> {
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

    emit_updated(&app, &id);
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
) -> Result<(), String> {
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
        space.index.write().map_err(lock_poisoned)?.remove_page(&id);
        space.persist_index(&state.writer).await?;
        if let Ok(mut queue) = space.queue.lock() {
            queue.remove(&id);
        }
        persist_queue(&state, &space).await;
    }

    emit_updated(&app, &id);
    Ok(())
}

/// Turn a note into a page with the same day, time, text and labels. The
/// page gets a new id and is written first; then the note's block becomes
/// its stub, so a crash between the two leaves the text twice rather than
/// nowhere. The page view opens on it next, so it is held, and a note still
/// waiting on the model goes to it when the view closes.
#[tauri::command]
pub async fn note_to_page(
    app: AppHandle,
    state: State<'_, AppState>,
    date: String,
    id: String,
    title: String,
) -> Result<Note, String> {
    let title = page_file::clean_title(&title).ok_or(NO_TITLE)?;
    check_date(&date)?;
    let space = state.space()?;
    let note = read_note(&space, &date, &id).await?;

    let page = Note {
        id: Ulid::generate().to_string(),
        file: free_path(&space, &note.date, &title, None),
        subject: Some(title),
        // A failed note gets another go as a page.
        status: match note.status {
            Status::Done => Status::Done,
            Status::Manual => Status::Manual,
            Status::Pending | Status::Failed => Status::Pending,
        },
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
    {
        let mut idx = space.index.write().map_err(lock_poisoned)?;
        idx.replace_day(
            &date,
            index::parse_day(&day_path(&space.root, &date), &date),
        );
        idx.replace_page(IndexEntry::from(&page));
    }
    space.persist_index(&state.writer).await?;
    if let Ok(mut queue) = space.queue.lock() {
        queue.remove(&id);
    }
    persist_queue(&state, &space).await;

    emit_updated(&app, &id);
    emit_updated(&app, &page.id);
    Ok(page)
}

/// Put a page back in the queue by hand (SPEC 5.6). One whose category the
/// user set is left alone, as a manual note is. Asked for from the page view
/// too, so the page is released: the model runs now. Typing again holds it
/// again, and the job is then dropped for the one `finish_page` queues.
pub async fn retry(
    app: &AppHandle,
    state: &State<'_, AppState>,
    space: &Space,
    id: &str,
) -> Result<(), String> {
    let page = read_page(space, id).await?;
    if page.status == Status::Manual {
        return Ok(());
    }
    {
        let (file, category, lang) = (page.file.clone(), page.category.clone(), page.lang.clone());
        state
            .writer
            .rewrite(space.root.join(&page.file), move |existing| {
                page_file::update_meta(existing?, &file, category, Status::Pending, lang)
            })
            .await?;
    }
    reindex(space, &page.file)?;
    space.release(id);
    space.persist_index(&state.writer).await?;
    enqueue(state, space, id.to_string(), page.date.clone()).await;
    emit_updated(app, id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    const DATE: &str = "2026-09-22";

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("scratchnote-pages-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("notes")).unwrap();
        root
    }

    fn page(id: &str, title: &str) -> Note {
        let body = "the meeting";
        Note {
            id: id.to_string(),
            date: DATE.to_string(),
            time: "10:00".to_string(),
            file: page_file::relative_path(DATE, &page_file::file_name(DATE, title, 1)),
            subject: Some(title.to_string()),
            category: None,
            status: Status::Done,
            hash: body_hash(body),
            lang: None,
            body: body.to_string(),
            kind: Kind::Page,
            missing: false,
        }
    }

    fn write(root: &Path, rel: &str, contents: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn open(root: &Path) -> Space {
        Space::open(
            "Test",
            root.to_path_buf(),
            Arc::new(tokio::sync::Notify::new()),
        )
        .0
    }

    fn stubs(root: &Path, date: &str) -> Vec<Stub> {
        std::fs::read_to_string(day_path(root, date))
            .map(|c| daily_file::parse_stubs(&c))
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn repair_gives_each_page_its_stub_and_nothing_else() {
        let root = scratch("repair");
        let sync = page("01PPP", "Weekly sync");
        write(&root, &sync.file, &page_file::render_page(&sync));
        let retro = page("01QQQ", "Retro");
        write(&root, &retro.file, &page_file::render_page(&retro));
        // Retro's stub is stale, a stub for Sync sits on another day, and
        // one stub has lost its page.
        let stale = Stub {
            title: "Old title".into(),
            ..Stub::for_page(&retro)
        };
        let lost = Stub {
            id: "01GONE".into(),
            ..Stub::for_page(&retro)
        };
        let day = daily_file::append_stub(&daily_file::append_stub("", &stale, DATE), &lost, DATE);
        write(&root, &crate::storage::relative_day_path(DATE), &day);
        let elsewhere = daily_file::append_stub("", &Stub::for_page(&sync), "2026-09-21");
        write(
            &root,
            &crate::storage::relative_day_path("2026-09-21"),
            &elsewhere,
        );

        let space = open(&root);
        let writer = Writer::spawn();
        repair_stubs(&writer, &space).await;

        let mut here = stubs(&root, DATE);
        here.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(
            here,
            vec![lost.clone(), Stub::for_page(&sync), Stub::for_page(&retro)],
            "the lost stub stays, Sync gets one, Retro's is rewritten"
        );
        assert!(stubs(&root, "2026-09-21").is_empty());
        assert_eq!(
            stubs(&root, DATE)
                .iter()
                .find(|s| s.id == "01PPP")
                .map(|s| s.target.as_str()),
            Some("../../pages/2026/2026-09-22 Weekly sync.md")
        );

        // Nothing left to do: a second pass writes nothing.
        let before = std::fs::read_to_string(day_path(&root, DATE)).unwrap();
        repair_stubs(&writer, &space).await;
        assert_eq!(
            std::fs::read_to_string(day_path(&root, DATE)).unwrap(),
            before
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_taken_file_name_gets_a_number_and_a_page_keeps_its_own() {
        let root = scratch("free-path");
        let sync = page("01PPP", "Weekly sync");
        write(&root, &sync.file, &page_file::render_page(&sync));
        let space = open(&root);

        assert_eq!(
            free_path(&space, DATE, "Weekly sync", None),
            "pages/2026/2026-09-22 Weekly sync 2.md"
        );
        // Its own name, whatever the case, is free for the page itself.
        assert_eq!(
            free_path(&space, DATE, "weekly SYNC", Some(&sync.file)),
            "pages/2026/2026-09-22 weekly SYNC.md"
        );
        assert_eq!(
            free_path(&space, DATE, "Retro", Some(&sync.file)),
            "pages/2026/2026-09-22 Retro.md"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
