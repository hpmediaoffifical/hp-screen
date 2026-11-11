//! Click and key effects drawn over exported video (filled in by the effects task).
use serde::Serialize;

/// One thing to draw over a frame.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Draw {}
