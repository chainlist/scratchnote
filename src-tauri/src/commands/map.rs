//! The map of the space by meaning, SPEC 6.5.

use std::collections::HashMap;

use serde::Serialize;
use tauri::State;

use crate::embed::categories::Category;
use crate::state::AppState;
use crate::storage::daily_file::Kind;

/// A note's place on the map, with what opening it takes, and the smallest
/// category it is in, if any.
#[derive(Debug, Serialize)]
pub struct MapNote {
    pub id: String,
    pub date: String,
    pub kind: Kind,
    pub x: f32,
    pub y: f32,
    pub category: Option<i64>,
}

/// Every note of the open space the embed task has placed on its map, none
/// until it has. A note deleted since its last pass is left out.
#[tauri::command]
pub async fn note_map(state: State<'_, AppState>) -> Result<Vec<MapNote>, String> {
    let space = state.space()?;
    let places: HashMap<String, ([f32; 2], Option<i64>)> = {
        let map = space.map.lock().map_err(|_| "map lock poisoned")?;
        let Some(map) = map.as_ref() else {
            return Ok(Vec::new());
        };
        let categories = map.categories().map(|c| &c.notes);
        map.places()
            .map(|(id, at)| {
                let category = categories.and_then(|notes| notes.get(id).copied());
                (id.to_string(), (at, category))
            })
            .collect()
    };
    let index = space.index.read().map_err(|_| "index lock poisoned")?;
    Ok(index
        .entries()
        .filter_map(|entry| {
            let ([x, y], category) = *places.get(&entry.id)?;
            Some(MapNote {
                id: entry.id.clone(),
                date: entry.date.clone(),
                kind: entry.kind,
                x,
                y,
                category,
            })
        })
        .collect())
}

/// The closest notes each note is linked to in the graph view.
const GRAPH_LINKS: usize = 3;

/// The links of the graph view: the notes, and each link as two places in
/// `ids`, one after the other.
#[derive(Debug, Default, Serialize)]
pub struct MapLinks {
    pub ids: Vec<String>,
    pub links: Vec<u32>,
}

/// Every note of the open space's map linked to its closest notes, each
/// pair once, none until the map is laid out.
#[tauri::command]
pub async fn map_links(state: State<'_, AppState>) -> Result<MapLinks, String> {
    let space = state.space()?;
    let map = space.map.lock().map_err(|_| "map lock poisoned")?;
    let Some(map) = map.as_ref() else {
        return Ok(MapLinks::default());
    };
    let mut out = MapLinks::default();
    let mut place: HashMap<&str, u32> = HashMap::new();
    for (a, b) in map.links(GRAPH_LINKS) {
        for id in [a, b] {
            let at = *place.entry(id).or_insert_with(|| {
                out.ids.push(id.to_string());
                out.ids.len() as u32 - 1
            });
            out.links.push(at);
        }
    }
    Ok(out)
}

/// The categories of the map: how far apart notes are joined at level 0,
/// level k reaching √2^k times as far, and every category.
#[derive(Debug, Default, Serialize)]
pub struct MapCategories {
    pub base: f32,
    pub categories: Vec<Category>,
}

/// The categories of the open space's map, none until they are found.
#[tauri::command]
pub async fn map_categories(state: State<'_, AppState>) -> Result<MapCategories, String> {
    let space = state.space()?;
    let map = space.map.lock().map_err(|_| "map lock poisoned")?;
    Ok(map
        .as_ref()
        .and_then(|map| map.categories())
        .map_or_else(MapCategories::default, |c| MapCategories {
            base: c.base,
            categories: c.list.clone(),
        }))
}
