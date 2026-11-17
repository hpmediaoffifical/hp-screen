//! User settings. Loading never fails: bad or missing values fall back to defaults.
use crate::geometry::Rect;
use crate::hotkey::{format_hotkey, parse_hotkey};
use crate::naming::DEFAULT_PATTERN;
use crate::quality::Quality;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 2: the screenshot hotkeys moved off Ctrl+Alt+S / Ctrl+Alt+A, which Zalo holds.
pub const SETTINGS_VERSION: u32 = 2;
pub const DEFAULT_HOTKEY_TOGGLE: &str = "Ctrl+Alt+R";
pub const DEFAULT_HOTKEY_PAUSE: &str = "Ctrl+Alt+P";
/// Capture an area and draw notes on it ("C" for chụp).
pub const DEFAULT_HOTKEY_SCREENSHOT: &str = "Ctrl+Alt+C";
/// Instant screenshot of the selected source, no editing ("N" for nhanh).
pub const DEFAULT_HOTKEY_QUICKSHOT: &str = "Ctrl+Alt+N";
/// Draw on screen while recording.
pub const DEFAULT_HOTKEY_DRAW: &str = "Ctrl+Alt+D";
/// Seconds a finished stroke stays before fading; 0 keeps strokes until cleared.
pub const DRAW_FADE_CHOICES: [u32; 5] = [0, 2, 3, 5, 10];
/// Defaults before version 2. Zalo holds both for its own screenshots, so settings still on
/// them move to the new defaults; a key the user picked themselves is never changed.
const OLD_DEFAULTS: [(&str, &str); 2] = [("hotkey_screenshot", "Ctrl+Alt+S"), ("hotkey_quickshot", "Ctrl+Alt+A")];

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    /// `None` means `Videos\HP Screen`.
    pub output_dir: Option<PathBuf>,
    pub name_pattern: String,
    /// 30 or 60.
    pub fps: u32,
    pub quality: Quality,
    /// 0, 3 or 5.
    pub countdown_secs: u32,
    pub system_audio: bool,
    pub mic: bool,
    pub hotkey_toggle: String,
    pub hotkey_pause: String,
    pub hotkey_screenshot: String,
    pub hotkey_quickshot: String,
    pub input_log: bool,
    pub autostart: bool,
    pub last_region: Option<Rect>,
    pub hotkey_draw: String,
    /// One of `DRAW_FADE_CHOICES`.
    pub draw_fade_secs: u32,
    pub webcam: crate::webcam::WebcamSettings,
    /// Record without the Windows cursor and draw a smooth one when editing (needs the input log).
    pub smooth_cursor: bool,
    /// Look for a new release on start and hourly, and install it without asking (never
    /// while a recording is running). Off: releases are only offered for a manual install.
    pub auto_update: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            output_dir: None,
            name_pattern: DEFAULT_PATTERN.to_string(),
            fps: 30,
            quality: Quality::Standard,
            countdown_secs: 3,
            system_audio: true,
            mic: true,
            hotkey_toggle: DEFAULT_HOTKEY_TOGGLE.to_string(),
            hotkey_pause: DEFAULT_HOTKEY_PAUSE.to_string(),
            hotkey_screenshot: DEFAULT_HOTKEY_SCREENSHOT.to_string(),
            hotkey_quickshot: DEFAULT_HOTKEY_QUICKSHOT.to_string(),
            input_log: true,
            autostart: false,
            last_region: None,
            hotkey_draw: DEFAULT_HOTKEY_DRAW.to_string(),
            draw_fade_secs: 3,
            webcam: crate::webcam::WebcamSettings::default(),
            smooth_cursor: false,
            auto_update: true,
        }
    }
}

/// Parses settings JSON. Unreadable JSON gives the defaults; missing fields are filled in.
pub fn load_settings(json: &str) -> Settings {
    use serde_json::Value;
    let Ok(Value::Object(mut input)) = serde_json::from_str::<Value>(json) else {
        return Settings::default();
    };
    let version = input.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version < 2 {
        for (field, old) in OLD_DEFAULTS {
            let on_old_default = input
                .get(field)
                .and_then(Value::as_str)
                .and_then(|k| parse_hotkey(k).ok())
                .is_some_and(|h| format_hotkey(&h) == old);
            if on_old_default {
                input.remove(field); // filled in with the new default below
            }
        }
    }
    let Ok(Value::Object(mut merged)) = serde_json::to_value(Settings::default()) else {
        return Settings::default();
    };
    // Accept fields one at a time so a single wrongly typed value cannot discard the rest.
    for (key, value) in input {
        if !merged.contains_key(&key) {
            continue;
        }
        let mut candidate = merged.clone();
        candidate.insert(key, value);
        if serde_json::from_value::<Settings>(Value::Object(candidate.clone())).is_ok() {
            merged = candidate;
        }
    }
    normalize(serde_json::from_value(Value::Object(merged)).unwrap_or_default())
}

/// Replaces out-of-range values with defaults and puts hotkeys in canonical form.
pub fn normalize(mut s: Settings) -> Settings {
    let defaults = Settings::default();
    s.version = SETTINGS_VERSION;
    if !matches!(s.fps, 30 | 60) {
        s.fps = defaults.fps;
    }
    if !matches!(s.countdown_secs, 0 | 3 | 5) {
        s.countdown_secs = defaults.countdown_secs;
    }
    if s.name_pattern.trim().is_empty() {
        s.name_pattern = defaults.name_pattern;
    }
    if !DRAW_FADE_CHOICES.contains(&s.draw_fade_secs) {
        s.draw_fade_secs = defaults.draw_fade_secs;
    }

    const SPARE_HOTKEYS: [&str; 8] = [
        DEFAULT_HOTKEY_TOGGLE,
        DEFAULT_HOTKEY_PAUSE,
        DEFAULT_HOTKEY_SCREENSHOT,
        DEFAULT_HOTKEY_QUICKSHOT,
        DEFAULT_HOTKEY_DRAW,
        "Ctrl+Alt+F9",
        "Ctrl+Alt+F10",
        "Ctrl+Alt+F11",
    ];
    let mut taken: Vec<String> = Vec::new();
    for (slot, default) in [
        (&mut s.hotkey_toggle, DEFAULT_HOTKEY_TOGGLE),
        (&mut s.hotkey_pause, DEFAULT_HOTKEY_PAUSE),
        (&mut s.hotkey_screenshot, DEFAULT_HOTKEY_SCREENSHOT),
        (&mut s.hotkey_quickshot, DEFAULT_HOTKEY_QUICKSHOT),
        (&mut s.hotkey_draw, DEFAULT_HOTKEY_DRAW),
    ] {
        let mut value = match parse_hotkey(slot) {
            Ok(h) => format_hotkey(&h),
            Err(_) => default.to_string(),
        };
        if taken.contains(&value) {
            value = std::iter::once(default)
                .chain(SPARE_HOTKEYS)
                .find(|k| !taken.iter().any(|t| t == k))
                .unwrap_or(default)
                .to_string();
        }
        taken.push(value.clone());
        *slot = value;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_defaults() {
        let s = Settings::default();
        assert_eq!(s.hotkey_draw, "Ctrl+Alt+D");
        assert_eq!(s.draw_fade_secs, 3);
    }

    #[test]
    fn bad_fade_falls_back_to_3() {
        assert_eq!(load_settings(r#"{"draw_fade_secs":7}"#).draw_fade_secs, 3);
        assert_eq!(load_settings(r#"{"draw_fade_secs":0}"#).draw_fade_secs, 0);
        assert_eq!(load_settings(r#"{"draw_fade_secs":10}"#).draw_fade_secs, 10);
    }

    #[test]
    fn draw_hotkey_cannot_duplicate_toggle() {
        let s = load_settings(r#"{"hotkey_draw":"Ctrl+Alt+R"}"#);
        assert_eq!(s.hotkey_toggle, "Ctrl+Alt+R");
        assert_ne!(s.hotkey_draw, s.hotkey_toggle);
    }

    #[test]
    fn corrupted_file_gives_defaults() {
        assert_eq!(load_settings("{{{ hỏng"), Settings::default());
        assert_eq!(load_settings(""), Settings::default());
        assert_eq!(load_settings("[1,2]"), Settings::default());
    }

    #[test]
    fn missing_fields_are_filled_with_defaults() {
        let s = load_settings(r#"{"fps":60}"#);
        assert_eq!(s.fps, 60);
        assert_eq!(s.countdown_secs, 3);
        assert_eq!(s.hotkey_toggle, "Ctrl+Alt+R");
        assert!(s.input_log);
    }

    #[test]
    fn unsupported_values_fall_back_to_defaults() {
        let s = load_settings(r#"{"fps":24,"countdown_secs":7,"name_pattern":"  "}"#);
        assert_eq!(s.fps, 30);
        assert_eq!(s.countdown_secs, 3);
        assert_eq!(s.name_pattern, DEFAULT_PATTERN);
    }

    #[test]
    fn a_wrongly_typed_field_does_not_discard_the_rest() {
        // One bad field must not wipe the user's other choices.
        let s = load_settings(r#"{"fps":"nhanh","countdown_secs":5,"mic":false}"#);
        assert_eq!(s.fps, 30);
        assert_eq!(s.countdown_secs, 5);
        assert!(!s.mic);
    }

    #[test]
    fn invalid_hotkeys_fall_back_and_valid_ones_are_canonicalized() {
        let s = load_settings(r#"{"hotkey_toggle":"abc","hotkey_pause":"alt+ctrl+k","hotkey_screenshot":"F12"}"#);
        assert_eq!(s.hotkey_toggle, "Ctrl+Alt+R");
        assert_eq!(s.hotkey_pause, "Ctrl+Alt+K");
        assert_eq!(s.hotkey_screenshot, DEFAULT_HOTKEY_SCREENSHOT);
    }

    #[test]
    fn two_actions_cannot_share_one_hotkey() {
        let s = load_settings(r#"{"hotkey_toggle":"Ctrl+Alt+P"}"#);
        // toggle keeps the user's choice; the colliding pause hotkey is cleared back to a free default
        assert_eq!(s.hotkey_toggle, "Ctrl+Alt+P");
        assert_ne!(s.hotkey_pause, s.hotkey_toggle);
        assert_ne!(s.hotkey_screenshot, s.hotkey_toggle);
        assert_ne!(s.hotkey_screenshot, s.hotkey_pause);
    }

    #[test]
    fn settings_round_trip_through_json() {
        let mut s = Settings::default();
        s.fps = 60;
        s.output_dir = Some(PathBuf::from(r"D:\Quay phim"));
        s.last_region = Some(Rect { x: -10, y: 20, w: 640, h: 480 });
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(load_settings(&json), s);
    }
}

#[cfg(test)]
mod quickshot_tests {
    use super::*;

    /// "Capture and annotate" and the instant screenshot each have their own key.
    #[test]
    fn instant_screenshot_has_its_own_hotkey() {
        let s = Settings::default();
        assert_ne!(s.hotkey_screenshot, s.hotkey_quickshot);
        // Files written before this setting existed get the default.
        assert_eq!(load_settings(r#"{"version":2,"hotkey_screenshot":"Ctrl+Alt+X"}"#).hotkey_quickshot, DEFAULT_HOTKEY_QUICKSHOT);
    }

    #[test]
    fn the_quickshot_hotkey_cannot_collide_with_the_other_three() {
        let s = load_settings(r#"{"hotkey_quickshot":"Ctrl+Alt+R"}"#);
        let keys = [&s.hotkey_toggle, &s.hotkey_pause, &s.hotkey_screenshot, &s.hotkey_quickshot];
        for (i, a) in keys.iter().enumerate() {
            for b in &keys[i + 1..] {
                assert_ne!(a, b, "two actions share {a}");
            }
        }
        assert_eq!(load_settings(r#"{"hotkey_quickshot":"bad"}"#).hotkey_quickshot, DEFAULT_HOTKEY_QUICKSHOT);
    }
}

#[cfg(test)]
mod zalo_hotkey_tests {
    //! Zalo, used on most Vietnamese PCs, holds Ctrl+Alt+S and Ctrl+Alt+A for its own
    //! screenshots, so HP Screen's screenshot keys moved off them.
    use super::*;

    #[test]
    fn screenshot_defaults_avoid_the_keys_zalo_holds() {
        let s = Settings::default();
        for zalo in ["Ctrl+Alt+S", "Ctrl+Alt+A"] {
            assert!([&s.hotkey_toggle, &s.hotkey_pause, &s.hotkey_screenshot, &s.hotkey_quickshot].iter().all(|k| *k != zalo));
        }
        assert_eq!(s.hotkey_screenshot, "Ctrl+Alt+C");
        assert_eq!(s.hotkey_quickshot, "Ctrl+Alt+N");
        assert_eq!(s.version, 2);
    }

    #[test]
    fn older_settings_still_on_the_old_defaults_move_to_the_new_ones() {
        let s = load_settings(r#"{"version":1,"hotkey_screenshot":"Ctrl+Alt+S","hotkey_quickshot":"Ctrl+Alt+A","fps":60}"#);
        assert_eq!((s.hotkey_screenshot.as_str(), s.hotkey_quickshot.as_str()), ("Ctrl+Alt+C", "Ctrl+Alt+N"));
        assert_eq!(s.fps, 60, "other choices are kept");
        // Files from before settings had a version count as old too.
        assert_eq!(load_settings(r#"{"hotkey_screenshot":"Ctrl+Alt+S"}"#).hotkey_screenshot, "Ctrl+Alt+C");
    }

    #[test]
    fn hotkeys_the_user_chose_are_never_moved() {
        let s = load_settings(r#"{"version":1,"hotkey_screenshot":"Ctrl+Alt+F9","hotkey_quickshot":"Ctrl+Alt+A"}"#);
        assert_eq!(s.hotkey_screenshot, "Ctrl+Alt+F9");
        assert_eq!(s.hotkey_quickshot, "Ctrl+Alt+N");
        // Picked again after the move: it stays.
        let s = load_settings(r#"{"version":2,"hotkey_screenshot":"Ctrl+Alt+S"}"#);
        assert_eq!(s.hotkey_screenshot, "Ctrl+Alt+S");
    }
}

#[cfg(test)]
mod auto_update_tests {
    use super::*;

    #[test]
    fn auto_update_is_on_by_default_and_for_files_written_before_it_existed() {
        assert!(Settings::default().auto_update);
        assert!(load_settings(r#"{"fps":60,"mic":false}"#).auto_update);
        assert!(load_settings("{{ hỏng").auto_update);
    }

    #[test]
    fn turning_auto_update_off_is_kept() {
        let s = load_settings(r#"{"auto_update":false}"#);
        assert!(!s.auto_update);
        assert!(!normalize(s.clone()).auto_update);
        assert_eq!(load_settings(&serde_json::to_string(&s).unwrap()), s);
    }

    #[test]
    fn a_wrongly_typed_auto_update_falls_back_to_on_without_losing_other_fields() {
        let s = load_settings(r#"{"auto_update":"có","countdown_secs":5}"#);
        assert!(s.auto_update);
        assert_eq!(s.countdown_secs, 5);
    }
}
