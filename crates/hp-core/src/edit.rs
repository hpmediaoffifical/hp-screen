//! The non-destructive edit of one video, stored as `.hpscreen/<stem>.edit.json`.
use crate::cut::{Cuts, Span};
use serde::{Deserialize, Serialize};

pub const EDIT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Trim {
    pub start_ms: i64,
    /// `None` keeps everything up to the end.
    pub end_ms: Option<i64>,
}

impl Default for Trim {
    fn default() -> Self {
        Trim { start_ms: 0, end_ms: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EffectToggles {
    pub clicks: bool,
    pub keys: bool,
    /// Draw the smooth cursor (recordings made without the Windows cursor).
    pub cursor: bool,
    /// Its size, 1–3 times the Windows cursor.
    pub cursor_size: f32,
}

impl Default for EffectToggles {
    fn default() -> Self {
        EffectToggles { clicks: true, keys: true, cursor: true, cursor_size: 1.5 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditDoc {
    pub v: u32,
    pub trim: Trim,
    /// Removed spans in source time.
    pub cuts: Vec<Span>,
    pub effects: EffectToggles,
    pub zoom: crate::zoom::ZoomDoc,
    pub frame: crate::layout::FrameDoc,
    pub subtitles: crate::subtitle::SubtitleDoc,
}

impl Default for EditDoc {
    fn default() -> Self {
        EditDoc { v: EDIT_VERSION, trim: Trim::default(), cuts: Vec::new(), effects: EffectToggles::default(), zoom: crate::zoom::ZoomDoc::default(), frame: crate::layout::FrameDoc::default(), subtitles: crate::subtitle::SubtitleDoc::default() }
    }
}

impl EditDoc {
    pub fn cuts(&self, duration_ms: i64) -> Cuts {
        Cuts::new(duration_ms, self.trim.start_ms, self.trim.end_ms, &self.cuts)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoadError {
    /// Written by a newer HP Screen; must not be opened or overwritten.
    Newer(u32),
    Invalid,
}

/// Reads an edit file. Missing fields take their defaults; a file from a newer version is
/// refused before anything else is read, so its unknown parts are never dropped by a save.
pub fn load_edit(json: &str) -> Result<EditDoc, LoadError> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|_| LoadError::Invalid)?;
    let v = value.get("v").and_then(|v| v.as_u64()).unwrap_or(EDIT_VERSION as u64);
    if v > EDIT_VERSION as u64 {
        return Err(LoadError::Newer(v.min(u32::MAX as u64) as u32));
    }
    let mut doc: EditDoc = serde_json::from_value(value).map_err(|_| LoadError::Invalid)?;
    doc.v = EDIT_VERSION;
    Ok(doc)
}

pub fn save_edit(doc: &EditDoc) -> String {
    serde_json::to_string_pretty(doc).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_take_defaults() {
        let doc = load_edit("{\"v\":1}").unwrap();
        assert_eq!(doc.trim, Trim { start_ms: 0, end_ms: None });
        assert!(doc.cuts.is_empty());
        assert!(doc.effects.clicks && doc.effects.keys);
    }

    #[test]
    fn round_trip_keeps_everything() {
        let doc = EditDoc {
            trim: Trim { start_ms: 200, end_ms: Some(9000) },
            cuts: vec![Span { start_ms: 1000, end_ms: 2000 }, Span { start_ms: 4000, end_ms: 4500 }],
            effects: EffectToggles { clicks: true, keys: false, cursor: true, cursor_size: 1.5 },
            ..EditDoc::default()
        };
        assert_eq!(load_edit(&save_edit(&doc)), Ok(doc));
    }

    #[test]
    fn newer_version_is_refused() {
        assert_eq!(load_edit("{\"v\":2}"), Err(LoadError::Newer(2)));
    }

    #[test]
    fn garbage_is_invalid() {
        assert_eq!(load_edit("not json"), Err(LoadError::Invalid));
        assert_eq!(load_edit("{\"v\":1,\"cuts\":\"oops\"}"), Err(LoadError::Invalid));
    }

    #[test]
    fn doc_cuts_use_trim_and_removed_spans() {
        let doc = EditDoc {
            trim: Trim { start_ms: 1000, end_ms: Some(9000) },
            cuts: vec![Span { start_ms: 3000, end_ms: 5000 }],
            ..EditDoc::default()
        };
        assert_eq!(doc.cuts(10_000).output_ms(), 6000);
    }
}
