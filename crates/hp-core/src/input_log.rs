//! Format of the per-recording input log (`<stem>.input.jsonl`) and the privacy filter that
//! decides which key presses may be written to it.
use crate::geometry::Rect;
use serde::{Deserialize, Serialize};

pub const LOG_VERSION: u32 = 1;

/// First line of every log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Button {
    Left,
    Right,
    Middle,
}

/// One log line. `t` is video time in milliseconds (pauses removed).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "e", rename_all = "lowercase")]
pub enum Event {
    Move { t: i64, x: i32, y: i32 },
    Down { t: i64, b: Button, x: i32, y: i32 },
    Up { t: i64, b: Button, x: i32, y: i32 },
    Wheel { t: i64, dx: i32, dy: i32, x: i32, y: i32 },
    Key { t: i64, k: String },
}

impl Event {
    /// Video time of the event in milliseconds.
    pub fn t(&self) -> i64 {
        match self {
            Event::Move { t, .. } | Event::Down { t, .. } | Event::Up { t, .. } | Event::Wheel { t, .. } | Event::Key { t, .. } => *t,
        }
    }
}

/// A log read back from disk.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParsedLog {
    pub header: Option<Header>,
    /// Sorted by time.
    pub events: Vec<Event>,
}

/// Reads a log. Lines that cannot be read are skipped; a log from a newer version gives
/// nothing, because its events may not mean what this version thinks.
pub fn parse_log(text: &str) -> ParsedLog {
    let mut log = ParsedLog::default();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Ok(event) = serde_json::from_str::<Event>(line) {
            log.events.push(event);
        } else if log.header.is_none() && log.events.is_empty()
            && let Ok(header) = serde_json::from_str::<Header>(line)
        {
            if header.v > LOG_VERSION {
                return ParsedLog::default();
            }
            log.header = Some(header);
        }
    }
    log.events.sort_by_key(Event::t);
    log
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
        // Ctrl+Alt is how AltGr arrives, and AltGr plus a character key types text.
        None if k.ctrl && k.alt && !k.win => return None,
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

#[cfg(test)]
mod altgr_tests {
    use super::*;

    fn key(ctrl: bool, alt: bool, shift: bool, win: bool, vk: u32) -> KeyStroke {
        KeyStroke { ctrl, alt, shift, win, vk }
    }

    /// AltGr reaches the hook as Ctrl+Alt. On the Windows Vietnamese layout digits are typed
    /// with AltGr, and other layouts type letters and symbols that way, so Ctrl+Alt plus a
    /// character key is text, not a shortcut.
    #[test]
    fn ctrl_alt_with_a_character_key_is_treated_as_typing() {
        assert_eq!(loggable_combo(&key(true, true, false, false, 0x31)), None); // AltGr+1
        assert_eq!(loggable_combo(&key(true, true, false, false, 0x45)), None); // AltGr+E
        assert_eq!(loggable_combo(&key(true, true, true, false, 0x51)), None); // AltGr+Shift+Q
        assert_eq!(loggable_combo(&key(true, true, false, false, 0x20)), None); // AltGr+Space
    }

    #[test]
    fn ctrl_alt_with_other_keys_is_still_a_shortcut() {
        assert_eq!(loggable_combo(&key(true, true, false, false, 0x2E)), Some("Ctrl+Alt+Delete".into()));
        assert_eq!(loggable_combo(&key(true, true, false, false, 0x74)), Some("Ctrl+Alt+F5".into()));
        assert_eq!(loggable_combo(&key(true, true, false, true, 0x44)), Some("Ctrl+Alt+Win+D".into()));
        assert_eq!(loggable_combo(&key(true, false, false, false, 0x31)), Some("Ctrl+1".into()));
        assert_eq!(loggable_combo(&key(false, true, false, false, 0x31)), Some("Alt+1".into()));
    }
}

#[cfg(test)]
mod parse_tests {
    use super::*;

    fn header(v: u32) -> Header {
        Header { v, mode: "region".into(), region: Rect { x: 10, y: 20, w: 200, h: 100 }, width: 200, height: 100, fps: 30 }
    }

    fn events() -> Vec<Event> {
        vec![
            Event::Move { t: 0, x: 1, y: 2 },
            Event::Down { t: 10, b: Button::Left, x: 1, y: 2 },
            Event::Up { t: 20, b: Button::Left, x: 1, y: 2 },
            Event::Key { t: 30, k: "Ctrl+S".into() },
        ]
    }

    fn text(lines: &[String]) -> String {
        lines.join("
") + "
"
    }

    #[test]
    fn log_round_trips() {
        let mut lines = vec![to_line(&header(LOG_VERSION))];
        lines.extend(events().iter().map(to_line));
        let log = parse_log(&text(&lines));
        assert_eq!(log.header, Some(header(LOG_VERSION)));
        assert_eq!(log.events, events());
    }

    #[test]
    fn bad_and_unknown_lines_are_skipped() {
        let mut lines = vec![to_line(&header(LOG_VERSION))];
        lines.extend(events().iter().map(to_line));
        lines.insert(2, "{oops".into());
        lines.insert(3, r#"{"e":"teleport","t":5}"#.into());
        lines.insert(4, String::new());
        assert_eq!(parse_log(&text(&lines)).events, events());
    }

    #[test]
    fn newer_log_version_gives_nothing() {
        let mut lines = vec![to_line(&header(LOG_VERSION + 1))];
        lines.extend(events().iter().map(to_line));
        assert_eq!(parse_log(&text(&lines)), ParsedLog::default());
    }

    #[test]
    fn events_are_sorted_by_time() {
        let mut lines = vec![to_line(&header(LOG_VERSION))];
        lines.extend(events().iter().rev().map(to_line));
        assert_eq!(parse_log(&text(&lines)).events, events());
    }

    #[test]
    fn a_log_without_header_still_gives_its_events() {
        let lines: Vec<String> = events().iter().map(to_line).collect();
        let log = parse_log(&text(&lines));
        assert_eq!(log.header, None);
        assert_eq!(log.events, events());
    }
}
