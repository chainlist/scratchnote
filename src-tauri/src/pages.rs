//! Pages, SPEC 3.5 and 4.7: notes with a title and a markdown file of their
//! own, shown on their day through a stub in the day's file.
//!
//! The page file is the source of truth. The stub is written from it, and
//! rewritten from it whenever the app sees it go stale, never at launch;
//! nothing is ever read back from a stub but its id and time.
//!
//! A page open in the editor is held: the page view saves it every pause in
//! typing, and neither model runs on it until `finish_page` says the view
//! closed (SPEC 3.5). The page view's commands are in `commands::pages`.

use tauri::{AppHandle, Emitter, State};

use crate::commands::notes::enqueue;
use crate::spaces::Space;
use crate::state::AppState;
use crate::storage::daily_file::{self, Note, Status, Stub};
use crate::storage::day_path;
use crate::storage::index::{Index, IndexEntry};
use crate::storage::page_file;
use crate::storage::writer::Writer;

fn lock_poisoned<T>(_: T) -> String {
    "index lock poisoned".to_string()
}

/// The page's entry in the index, or why there is none.
pub fn entry(space: &Space, id: &str) -> Result<IndexEntry, String> {
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
pub fn reindex(space: &Space, file: &str) -> Result<Option<Note>, String> {
    let page = std::fs::read_to_string(space.root.join(file))
        .ok()
        .and_then(|contents| page_file::parse_page(&contents, file));
    if let Some(page) = &page {
        space.page_changed(page)?;
    }
    Ok(page)
}

/// A path for a page of `date` called `title` that no other file has taken.
/// `current` is the page's own file, which it may keep, in any case.
pub fn free_path(space: &Space, date: &str, title: &str, current: Option<&str>) -> String {
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

/// Write a new page's file, refusing to replace one that appeared meanwhile.
pub async fn write_new(writer: &Writer, space: &Space, page: &Note) -> Result<(), String> {
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

pub fn emit_updated(app: &AppHandle, id: &str) {
    let _ = app.emit("note-updated", serde_json::json!({ "id": id }));
}

/// Every page in the index, newest first: by date, then by time within the day.
pub fn newest_first(index: &Index) -> Vec<Note> {
    let mut pages: Vec<Note> = index.pages().map(IndexEntry::to_note).collect();
    pages.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.time.cmp(&a.time)));
    pages
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
    use crate::storage::daily_file::{body_hash, Kind};
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

    #[test]
    fn lists_every_page_newest_first_without_the_notes() {
        let at = |id: &str, date: &str, time: &str| Note {
            date: date.to_string(),
            time: time.to_string(),
            ..page(id, id)
        };
        let mut index = Index::default();
        index.push(IndexEntry::from(&at("01OLD", "2026-09-01", "18:00")));
        index.push(IndexEntry::from(&at("01MORNING", DATE, "09:00")));
        index.push(IndexEntry::from(&at("01EVENING", DATE, "17:30")));
        index.push(IndexEntry::from(&Note {
            kind: Kind::Note,
            ..at("01NOTE", DATE, "12:00")
        }));

        let ids: Vec<String> = newest_first(&index).into_iter().map(|p| p.id).collect();
        assert_eq!(ids, ["01EVENING", "01MORNING", "01OLD"]);
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
