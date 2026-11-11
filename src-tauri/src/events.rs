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
    let _ = app.emit(REC_STATE, status);
}

/// Something went wrong while recording; `message` is shown to the user as is.
pub fn rec_error(app: &AppHandle, message: &str) {
    let _ = app.emit(REC_ERROR, ErrorPayload { message });
}
