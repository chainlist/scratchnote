use crate::settings::Settings;
use crate::storage::writer::Writer;

pub struct AppState {
    pub settings: Settings,
    pub writer: Writer,
}
