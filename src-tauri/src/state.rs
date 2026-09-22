use std::sync::RwLock;

use crate::settings::Settings;
use crate::storage::index::Index;
use crate::storage::writer::Writer;

pub struct AppState {
    pub settings: Settings,
    pub writer: Writer,
    /// The derived cache from SPEC 4.4. Guards are held only for the length of
    /// a read or a swap, never across an await.
    pub index: RwLock<Index>,
}
