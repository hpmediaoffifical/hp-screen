//! The drawn cursor of recordings made without the Windows cursor: its shapes, and a smooth,
//! self-hiding path built from the logged mouse positions.
use crate::spring::{Spring, SpringState};
use serde::Serialize;

/// The cursor follows the logged position on this fast, critically damped spring: hand
/// jitter is filtered, the cursor still keeps up.
pub const CURSOR_SPRING: Spring = Spring { tension: 600.0, mass: 1.0, friction: 49.0 };
/// One sample of the smooth path every this many ms (it is interpolated between samples).
pub const SAMPLE_MS: i64 = 1000 / 30;
/// The cursor fades out after this long without moving (unless a button is held)...
pub const IDLE_MS: i64 = 1500;
/// ...over this long; it comes back at once when it moves.
pub const FADE_MS: i64 = 300;
/// Size while a button is held.
pub const PRESS_SCALE: f32 = 0.85;
/// Movement smaller than this (pixels per ms) is not "moving".
const MOVE_EPS: f32 = 0.5;
/// Faster than this (pixels per ms) is no hand movement: a jump across a pause, which the
/// cursor follows at once instead of sliding across the cut.
const SNAP_SPEED: f32 = 20.0;

/// One sample of the drawn cursor: position (video pixels), opacity, and size factor (pressed).
/// Sent to the edit page as a short array `[t, x, y, alpha, press]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CursorSample {
    pub t_ms: i64,
    pub x: f32,
    pub y: f32,
    pub alpha: f32,
    pub press: f32,
}

impl Serialize for CursorSample {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let r = |v: f32| (v * 10.0).round() / 10.0;
        let r2 = |v: f32| (v * 100.0).round() / 100.0;
        (self.t_ms, r(self.x), r(self.y), r2(self.alpha), r2(self.press)).serialize(s)
    }
}

/// A cursor shape: hot spot (pixels into the image) and the image as base64 PNG.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CursorShape {
    pub id: u32,
    pub hx: i32,
    pub hy: i32,
    pub png: String,
}

/// The smooth path of the cursor from 0 to `duration_ms`. `at(t)` is the logged position;
/// `holds` are the spans `[start, end)` while a button is held, sorted by start.
pub fn smooth_cursor(at: &dyn Fn(i64) -> Option<(f32, f32)>, holds: &[(i64, i64)], duration_ms: i64) -> Vec<CursorSample> {
    let Some(start) = at(0) else { return Vec::new() };
    let (mut x, mut y) = (SpringState { value: start.0, velocity: 0.0 }, SpringState { value: start.1, velocity: 0.0 });
    let mut last_move = 0i64;
    let mut prev = start;
    let mut alpha = 1.0f32;
    let mut target_prev = start;
    let mut hold_i = 0usize;
    let mut out = Vec::with_capacity((duration_ms.max(0) / SAMPLE_MS + 2) as usize);
    for t in 0..=duration_ms.max(0) {
        let target = at(t).unwrap_or(prev);
        if (target.0 - prev.0).abs() > MOVE_EPS || (target.1 - prev.1).abs() > MOVE_EPS {
            last_move = t;
            prev = target;
        }
        while hold_i < holds.len() && holds[hold_i].1 <= t {
            hold_i += 1;
        }
        let down = holds.get(hold_i).is_some_and(|h| h.0 <= t);
        let jump = (target.0 - target_prev.0).abs().max((target.1 - target_prev.1).abs());
        target_prev = target;
        if jump > SNAP_SPEED {
            (x, y) = (SpringState { value: target.0, velocity: 0.0 }, SpringState { value: target.1, velocity: 0.0 });
        } else if t > 0 {
            CURSOR_SPRING.step(&mut x, target.0, 0.001);
            CURSOR_SPRING.step(&mut y, target.1, 0.001);
        }
        alpha = if down || t - last_move < IDLE_MS { 1.0 } else { (alpha - 1.0 / FADE_MS as f32).max(0.0) };
        if t % SAMPLE_MS == 0 {
            out.push(CursorSample { t_ms: t, x: x.value, y: y.value, alpha, press: if down { PRESS_SCALE } else { 1.0 } });
        }
    }
    out
}

/// The drawn cursor at `t_ms`, interpolated between samples.
pub fn cursor_at(samples: &[CursorSample], t_ms: i64) -> Option<CursorSample> {
    let (first, last) = (samples.first()?, samples.last()?);
    if t_ms <= first.t_ms {
        return Some(*first);
    }
    if t_ms >= last.t_ms {
        return Some(*last);
    }
    let i = samples.partition_point(|s| s.t_ms <= t_ms);
    let (a, b) = (samples[i - 1], samples[i]);
    let f = (t_ms - a.t_ms) as f32 / (b.t_ms - a.t_ms).max(1) as f32;
    let mix = |p: f32, q: f32| p + (q - p) * f;
    Some(CursorSample { t_ms, x: mix(a.x, b.x), y: mix(a.y, b.y), alpha: mix(a.alpha, b.alpha), press: if f < 0.5 { a.press } else { b.press } })
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding.
pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Decodes standard base64 (padding optional); `None` on any other character.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.bytes().filter(|&c| c != b'=') {
        let v = B64.iter().position(|&b| b == c)? as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits & 0xff) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips() {
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"M"), "TQ==");
        let data: Vec<u8> = (0..=255).collect();
        assert_eq!(base64_decode(&base64_encode(&data)).unwrap(), data);
        assert_eq!(base64_decode("TWE="), Some(b"Ma".to_vec()));
        assert_eq!(base64_decode("T!E="), None);
    }

    #[test]
    fn the_path_keeps_up_with_the_mouse() {
        // A quick move of 100 px over 16 ms (logged positions are interpolated between samples).
        let at = |t: i64| Some(((t - 1000).clamp(0, 16) as f32 * 100.0 / 16.0, 0.0));
        let s = smooth_cursor(&at, &[], 2000);
        let x = |ms| cursor_at(&s, ms).unwrap().x;
        assert!(x(1050) > 20.0 && x(1050) < 100.0, "eases, does not jump: {}", x(1050));
        assert!(x(1400) > 99.0, "arrives quickly: {}", x(1400));
    }

    #[test]
    fn hand_jitter_is_filtered() {
        let at = |t: i64| Some((if (t / 16) % 2 == 0 { 3.0 } else { -3.0 }, 0.0));
        let s = smooth_cursor(&at, &[], 2000);
        let worst = s.iter().filter(|c| c.t_ms > 500).map(|c| c.x.abs()).fold(0.0f32, f32::max);
        assert!(worst < 1.5, "a 6 px shake moves the drawn cursor less than 1.5 px: {worst}");
    }

    #[test]
    fn an_idle_cursor_fades_out_and_comes_back_when_moved() {
        let at = |t: i64| Some(if t < 3000 { (10.0, 10.0) } else { (40.0, 10.0) });
        let s = smooth_cursor(&at, &[], 4000);
        let a = |ms| cursor_at(&s, ms).unwrap().alpha;
        assert_eq!(a(1000), 1.0);
        assert!(a(1650) > 0.0 && a(1650) < 1.0, "fading: {}", a(1650));
        assert_eq!(a(2000), 0.0, "gone after 1.5 s + 0.3 s still");
        assert_eq!(a(3020), 1.0, "back as soon as it moves");
    }

    #[test]
    fn a_held_button_keeps_it_shown_and_presses_it() {
        let at = |_| Some((10.0, 10.0));
        let s = smooth_cursor(&at, &[(2500, 3500)], 4000);
        let c = cursor_at(&s, 3000).unwrap();
        assert_eq!((c.alpha, c.press), (1.0, PRESS_SCALE));
        assert_eq!(cursor_at(&s, 1000).unwrap().press, 1.0);
    }

    #[test]
    fn a_jump_across_a_pause_does_not_glide() {
        // Paused with the mouse on the left, resumed with it on the right: no slide across the cut.
        let at = |t: i64| Some(if t < 1000 { (0.0, 0.0) } else if t < 1010 { ((t - 1000) as f32 * 60.0, 0.0) } else { (600.0, 0.0) });
        let s = smooth_cursor(&at, &[], 2000);
        assert!(cursor_at(&s, 1030).unwrap().x > 590.0, "at the next sample: {:?}", cursor_at(&s, 1030));
    }

    #[test]
    fn samples_are_sent_as_short_arrays() {
        let c = CursorSample { t_ms: 33, x: 10.04, y: 20.0, alpha: 0.5, press: 1.0 };
        assert_eq!(serde_json::to_string(&c).unwrap(), "[33,10.0,20.0,0.5,1.0]");
    }

    #[test]
    fn no_positions_no_path() {
        assert!(smooth_cursor(&|_| None, &[], 1000).is_empty());
        assert_eq!(cursor_at(&[], 5), None);
    }
}
