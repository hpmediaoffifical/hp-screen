//! Events the core sends to the UI.
use crate::recorder::RecStatus;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const LIBRARY_CHANGED: &str = "library-changed";
pub const REC_STATE: &str = "rec-state";
pub const REC_ERROR: &str = "rec-error";

#[derive(Clone, Serialize)]
struct ErrorPayload<'a> {
    message: &'a str,
}

/// A recording or screenshot was added, renamed or removed.
pub fn library_changed(app: &AppHandle) {
    let _ = app.emit(LIBRARY_CHANGED, ());
}

pub fn rec_state(app: &AppHandle, status: &RecStatus) {
    crate::shell::sync_windows(app, status.state);
    crate::live_cmds::on_rec_state(app, status.state);
    let _ = app.emit(REC_STATE, status);
}

/// Something went wrong while recording; `message` is shown to the user as is.
pub fn rec_error(app: &AppHandle, message: &str) {
    crate::paths::log_error(app, message);
    #[cfg(debug_assertions)]
    eprintln!("[hp] rec-error: {message}");
    let _ = app.emit(REC_ERROR, ErrorPayload { message });
}
