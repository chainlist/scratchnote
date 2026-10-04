//! The map of the space by meaning, SPEC 6.5.

use std::collections::HashMap;

use serde::Serialize;
use tauri::State;

use crate::state::AppState;
use crate::storage::daily_file::Kind;

/// A note's place on the map, with what opening it takes.
#[derive(Debug, Serialize)]
pub struct MapNote {
    pub id: String,
    pub date: String,
    pub kind: Kind,
    pub x: f32,
    pub y: f32,
}

/// Every note of the open space the embed task has placed on its map, none
/// until it has. A note deleted since its last pass is left out.
#[tauri::command]
pub async fn note_map(state: State<'_, AppState>) -> Result<Vec<MapNote>, String> {
    let space = state.space()?;
    let places: HashMap<String, [f32; 2]> = {
        let map = space.map.lock().map_err(|_| "map lock poisoned")?;
        match map.as_ref() {
            Some(map) => map.places().map(|(id, at)| (id.to_string(), at)).collect(),
            None => return Ok(Vec::new()),
        }
    };
    let index = space.index.read().map_err(|_| "index lock poisoned")?;
    Ok(index
        .entries()
        .filter_map(|entry| {
            let [x, y] = *places.get(&entry.id)?;
            Some(MapNote {
                id: entry.id.clone(),
                date: entry.date.clone(),
                kind: entry.kind,
                x,
                y,
            })
        })
        .collect())
}
