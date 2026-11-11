//! Format of the per-recording input log (`<stem>.input.jsonl`) and the privacy filter that
//! decides which key presses may be written to it.
use crate::geometry::Rect;
use serde::Serialize;

pub const LOG_VERSION: u32 = 1;

/// First line of every log.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Header {
    pub v: u32,
    /// "monitor", "window" or "region".
    pub mode: String,
    /// Captured area in virtual-screen pixels; event coordinates are relative to its origin.
    pub region: Rect,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Button {
    Left,
    Right,
    Middle,
}

/// One log line. `t` is video time in milliseconds (pauses removed).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "e", rename_all = "lowercase")]
pub enum Event {
    Move { t: i64, x: i32, y: i32 },
    Down { t: i64, b: Button, x: i32, y: i32 },
    Up { t: i64, b: Button, x: i32, y: i32 },
    Wheel { t: i64, dx: i32, dy: i32, x: i32, y: i32 },
    Key { t: i64, k: String },
}

/// Serializes a header or event as one JSON line without the trailing newline.
pub fn to_line<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// A key-down as seen by the keyboard hook.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyStroke {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    /// Windows virtual-key code.
    pub vk: u32,
}

/// Text to log for a key press, or `None` when it must not be logged.
///
/// Logged: combinations with Ctrl, Alt or Win, and function/navigation keys on their own.
/// Never logged: letters, digits and punctuation typed plainly or with Shift, so passwords
/// and other typed text cannot end up in the log.
pub fn loggable_combo(k: &KeyStroke) -> Option<String> {
    let has_command_modifier = k.ctrl || k.alt || k.win;
    let name = match special_key_name(k.vk) {
        Some(name) => name,
        None if has_command_modifier => character_key_name(k.vk)?,
        None => return None,
    };
    let mut out = String::new();
    for (on, prefix) in [(k.ctrl, "Ctrl+"), (k.alt, "Alt+"), (k.shift, "Shift+"), (k.win, "Win+")] {
        if on {
            out.push_str(prefix);
        }
    }
    out.push_str(&name);
    Some(out)
}

/// Keys that produce no text: safe to log even without a modifier.
fn special_key_name(vk: u32) -> Option<String> {
    let name = match vk {
        0x70..=0x87 => return Some(format!("F{}", vk - 0x70 + 1)),
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x1B => "Esc",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "Left",
        0x26 => "Up",
        0x27 => "Right",
        0x28 => "Down",
        0x2C => "PrintScreen",
        0x2D => "Insert",
        0x2E => "Delete",
        _ => return None,
    };
    Some(name.to_string())
}

/// Keys that type text: only logged as part of a Ctrl/Alt/Win shortcut.
fn character_key_name(vk: u32) -> Option<String> {
    match vk {
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(vk).map(String::from),
        0x20 => Some("Space".to_string()),
        _ => None,
    }
}

/// Limits mouse-move events to 60 per second.
#[derive(Clone, Debug, Default)]
pub struct MoveSampler {
    last_ms: Option<i64>,
}

impl MoveSampler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a move at `t_ms` should be written.
    pub fn accept(&mut self, t_ms: i64) -> bool {
        const MIN_GAP_MS: i64 = 16;
        match self.last_ms {
            Some(last) if t_ms - last < MIN_GAP_MS => false,
            _ => {
                self.last_ms = Some(t_ms);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(ctrl: bool, alt: bool, shift: bool, win: bool, vk: u32) -> KeyStroke {
        KeyStroke { ctrl, alt, shift, win, vk }
    }

    #[test]
    fn events_serialize_as_compact_tagged_json() {
        assert_eq!(
            to_line(&Event::Down { t: 1200, b: Button::Left, x: 10, y: 20 }),
            r#"{"e":"down","t":1200,"b":"left","x":10,"y":20}"#
        );
        assert_eq!(to_line(&Event::Move { t: 5, x: -3, y: 4 }), r#"{"e":"move","t":5,"x":-3,"y":4}"#);
        assert_eq!(
            to_line(&Event::Wheel { t: 9, dx: 0, dy: -120, x: 1, y: 2 }),
            r#"{"e":"wheel","t":9,"dx":0,"dy":-120,"x":1,"y":2}"#
        );
        assert_eq!(to_line(&Event::Key { t: 7, k: "Ctrl+C".into() }), r#"{"e":"key","t":7,"k":"Ctrl+C"}"#);
    }

    #[test]
    fn header_serializes_on_one_line() {
        let h = Header {
            v: LOG_VERSION,
            mode: "region".into(),
            region: Rect { x: -100, y: 0, w: 800, h: 600 },
            width: 800,
            height: 600,
            fps: 30,
        };
        let line = to_line(&h);
        assert_eq!(
            line,
            r#"{"v":1,"mode":"region","region":{"x":-100,"y":0,"w":800,"h":600},"width":800,"height":600,"fps":30}"#
        );
        assert!(!line.contains('\n'));
    }

    #[test]
    fn shortcuts_with_ctrl_alt_or_win_are_logged() {
        assert_eq!(loggable_combo(&key(true, false, false, false, 0x43)), Some("Ctrl+C".into()));
        assert_eq!(loggable_combo(&key(false, true, false, false, 0x09)), Some("Alt+Tab".into()));
        assert_eq!(loggable_combo(&key(true, false, true, false, 0x53)), Some("Ctrl+Shift+S".into()));
        assert_eq!(loggable_combo(&key(false, false, false, true, 0x44)), Some("Win+D".into()));
        assert_eq!(loggable_combo(&key(true, true, false, false, 0x2E)), Some("Ctrl+Alt+Delete".into()));
    }

    #[test]
    fn function_and_navigation_keys_are_logged_alone() {
        assert_eq!(loggable_combo(&key(false, false, false, false, 0x74)), Some("F5".into()));
        assert_eq!(loggable_combo(&key(false, false, false, false, 0x0D)), Some("Enter".into()));
        assert_eq!(loggable_combo(&key(false, false, false, false, 0x1B)), Some("Esc".into()));
        assert_eq!(loggable_combo(&key(false, false, true, false, 0x09)), Some("Shift+Tab".into()));
        assert_eq!(loggable_combo(&key(false, false, false, false, 0x25)), Some("Left".into()));
    }

    #[test]
    fn typed_characters_are_never_logged() {
        assert_eq!(loggable_combo(&key(false, false, false, false, 0x41)), None); // a
        assert_eq!(loggable_combo(&key(false, false, true, false, 0x41)), None); // A
        assert_eq!(loggable_combo(&key(false, false, false, false, 0x31)), None); // 1
        assert_eq!(loggable_combo(&key(false, false, true, false, 0x31)), None); // !
        assert_eq!(loggable_combo(&key(false, false, false, false, 0x20)), None); // space
        assert_eq!(loggable_combo(&key(false, false, false, false, 0xBA)), None); // ;
        assert_eq!(loggable_combo(&key(false, false, false, false, 0x60)), None); // numpad 0
    }

    #[test]
    fn modifier_keys_alone_are_not_logged() {
        assert_eq!(loggable_combo(&key(true, false, false, false, 0x11)), None); // Ctrl
        assert_eq!(loggable_combo(&key(true, false, false, false, 0xA2)), None); // left Ctrl
        assert_eq!(loggable_combo(&key(false, true, false, false, 0x12)), None); // Alt
        assert_eq!(loggable_combo(&key(false, false, false, true, 0x5B)), None); // Win
        assert_eq!(loggable_combo(&key(false, false, true, false, 0x10)), None); // Shift
    }

    #[test]
    fn move_sampler_allows_at_most_sixty_per_second() {
        let mut s = MoveSampler::new();
        assert!(s.accept(0));
        assert!(!s.accept(10));
        assert!(s.accept(17));
        assert!(!s.accept(30));
        assert!(s.accept(34));
    }
}
