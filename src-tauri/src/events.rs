//! Events the core sends to the UI.
use tauri::{AppHandle, Emitter};

pub const LIBRARY_CHANGED: &str = "library-changed";

/// A recording or screenshot was added, renamed or removed.
pub fn library_changed(app: &AppHandle) {
    let _ = app.emit(LIBRARY_CHANGED, ());
}
