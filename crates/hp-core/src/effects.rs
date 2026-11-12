//! Click and key effects drawn over exported video, built from the input log.
//!
//! Positions of rings, dots and wheel marks are in source-video pixels (they zoom with the
//! picture); chips are placed in the output frame.
use crate::edit::EffectToggles;
use crate::input_log::{Button, Event, ParsedLog};
use serde::Serialize;

/// How long a click ripple grows and fades.
pub const RIPPLE_MS: i64 = 480;
/// Final ripple radius as a fraction of the source video height.
pub const RIPPLE_RADIUS: f32 = 0.025;
/// How long the wheel mark stays after the last wheel tick.
pub const WHEEL_HOLD_MS: i64 = 400;
/// Wheel marks fade out over this long before they end.
pub const WHEEL_FADE_MS: i64 = 150;
/// How long a shortcut chip shows.
pub const CHIP_MS: i64 = 1500;
/// Chip fade-in and fade-out.
pub const CHIP_FADE_MS: i64 = 150;
/// Chip bottom edge distance from the bottom of the output, as a fraction of its height.
pub const CHIP_BOTTOM: f32 = 0.08;
/// Ring start size and hold-dot size, as fractions of the full ripple radius.
const RING_START: f32 = 0.3;
const DOT_RADIUS: f32 = 0.35;
const DOT_ALPHA: f32 = 0.55;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Ripple {
    pub t_ms: i64,
    pub x: f32,
    pub y: f32,
    pub button: Button,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Hold {
    pub start_ms: i64,
    pub end_ms: i64,
    pub button: Button,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Wheel {
    pub start_ms: i64,
    pub end_ms: i64,
    pub up: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Chip {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// Everything the log says to draw, with times in source milliseconds.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Effects {
    pub ripples: Vec<Ripple>,
    pub holds: Vec<Hold>,
    pub wheels: Vec<Wheel>,
    pub chips: Vec<Chip>,
    /// Cursor positions over time: (ms, x, y).
    pub cursor: Vec<(i64, f32, f32)>,
    /// Moments of typing (ms), for automatic zoom.
    pub typing: Vec<i64>,
    /// Wheel events (ms, x, y), for automatic zoom.
    pub scrolls: Vec<(i64, f32, f32)>,
    /// A full-screen recording: clicks at the very edge are taskbar or window buttons.
    pub edge_guard: bool,
    /// UI element boxes logged at clicks and while typing, for automatic zoom.
    pub elements: Vec<UiBox>,
    /// When another window came to the front, and its box.
    pub windows: Vec<(i64, [f32; 4])>,
    /// The video has no Windows cursor: it is drawn from `smooth` with `shapes`.
    pub drawn_cursor: bool,
    pub shapes: Vec<crate::cursor::CursorShape>,
    /// (ms, shape id) as the cursor changed shape; 0 is hidden.
    pub cursor_ids: Vec<(i64, u32)>,
    /// The smooth, self-hiding cursor path (video pixels); empty until `with_cursor_path`.
    pub smooth: Vec<crate::cursor::CursorSample>,
    /// Time of the last logged event.
    #[serde(skip)]
    pub cursor_end: i64,
}

/// The box of a UI element (video pixels): under a click (`kind` "click") or receiving keys ("focus").
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UiBox {
    pub t_ms: i64,
    pub kind: String,
    pub rect: [f32; 4],
}

/// One thing to draw over a frame.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Draw {
    Ring { x: f32, y: f32, radius: f32, alpha: f32, button: Button },
    Dot { x: f32, y: f32, radius: f32, alpha: f32, button: Button },
    Wheel { x: f32, y: f32, up: bool, alpha: f32 },
    Chip { text: String, alpha: f32, bottom: f32 },
    /// A burned-in subtitle cue, placed in the output frame.
    Subtitle { text: String },
    /// A spotlight: the video outside this box (source pixels) is darkened by `alpha`.
    Spot { x: f32, y: f32, w: f32, h: f32, alpha: f32 },
    /// The drawn cursor: tip at (x, y) in source pixels, `scale` times the shape's size.
    Cursor { x: f32, y: f32, scale: f32, alpha: f32, shape: u32 },
}

pub fn build_effects(log: &ParsedLog, video_w: u32, video_h: u32) -> Effects {
    let (sx, sy) = match &log.header {
        Some(h) if h.width > 0 && h.height > 0 => (video_w as f32 / h.width as f32, video_h as f32 / h.height as f32),
        _ => (1.0, 1.0),
    };
    let at = |x: i32, y: i32| (x as f32 * sx, y as f32 * sy);
    let mut fx = Effects {
        edge_guard: log.header.as_ref().is_some_and(|h| h.mode == "monitor"),
        drawn_cursor: log.header.as_ref().is_some_and(|h| h.cursor),
        ..Effects::default()
    };
    let mut open_holds: Vec<(i64, Button)> = Vec::new();
    let mut chip_key: Option<(String, u32)> = None;
    for e in &log.events {
        match e {
            Event::Move { t, x, y } => fx.cursor.push((*t, at(*x, *y).0, at(*x, *y).1)),
            Event::Down { t, b, x, y } => {
                let (px, py) = at(*x, *y);
                fx.cursor.push((*t, px, py));
                fx.ripples.push(Ripple { t_ms: *t, x: px, y: py, button: *b });
                open_holds.retain(|(_, ob)| ob != b);
                open_holds.push((*t, *b));
            }
            Event::Up { t, b, x, y } => {
                let (px, py) = at(*x, *y);
                fx.cursor.push((*t, px, py));
                if let Some(i) = open_holds.iter().position(|(_, ob)| ob == b) {
                    let (start, button) = open_holds.remove(i);
                    fx.holds.push(Hold { start_ms: start, end_ms: *t, button });
                }
            }
            Event::Wheel { t, dy, x, y, .. } => {
                let (px, py) = at(*x, *y);
                fx.cursor.push((*t, px, py));
                fx.scrolls.push((*t, px, py));
                if *dy == 0 {
                    continue;
                }
                let up = *dy > 0;
                match fx.wheels.last_mut() {
                    Some(w) if w.end_ms > *t && w.up == up => w.end_ms = t + WHEEL_HOLD_MS,
                    last => {
                        if let Some(w) = last
                            && w.end_ms > *t
                        {
                            w.end_ms = *t;
                        }
                        fx.wheels.push(Wheel { start_ms: *t, end_ms: t + WHEEL_HOLD_MS, up });
                    }
                }
            }
            Event::Type { t } => fx.typing.push(*t),
            Event::El { t, k, r } => {
                let (x, y) = at(r[0], r[1]);
                fx.elements.push(UiBox { t_ms: *t, kind: k.clone(), rect: [x, y, r[2] as f32 * sx, r[3] as f32 * sy] });
            }
            Event::Shape { id, hx, hy, png } => {
                // Shapes are in screen pixels; the hot spot scales with the video like positions.
                fx.shapes.push(crate::cursor::CursorShape { id: *id, hx: (*hx as f32 * sx).round() as i32, hy: (*hy as f32 * sy).round() as i32, png: png.clone() });
            }
            Event::Cur { t, id } => fx.cursor_ids.push((*t, *id)),
            Event::Win { t, r } => {
                let (x, y) = at(r[0], r[1]);
                fx.windows.push((*t, [x, y, r[2] as f32 * sx, r[3] as f32 * sy]));
            }
            Event::Key { t, k } => {
                let showing = fx.chips.last().is_some_and(|c| c.end_ms > *t);
                match (&mut chip_key, fx.chips.last_mut()) {
                    (Some((key, count)), Some(chip)) if showing && key == k => {
                        *count += 1;
                        chip.end_ms = t + CHIP_MS;
                        chip.text = format!("{} ×{count}", chip_text(k));
                    }
                    (_, last) => {
                        if let Some(chip) = last
                            && showing
                        {
                            chip.end_ms = *t;
                        }
                        fx.chips.push(Chip { start_ms: *t, end_ms: t + CHIP_MS, text: chip_text(k) });
                        chip_key = Some((k.clone(), 1));
                    }
                }
            }
        }
    }
    let last_t = log.events.last().map_or(0, Event::t);
    for (start, button) in open_holds {
        fx.holds.push(Hold { start_ms: start, end_ms: last_t, button });
    }
    fx.holds.sort_by_key(|h| h.start_ms);
    fx.cursor_end = last_t;
    fx
}

impl Effects {
    /// Makes the drawn cursor's smooth path (only for recordings without the Windows cursor).
    /// Costly on long videos: done only where the cursor is drawn (edit page, export).
    pub fn with_cursor_path(mut self) -> Self {
        if self.drawn_cursor && self.smooth.is_empty() {
            // Long enough past the last event for an idle cursor to fade out.
            let end = self.cursor_end + crate::cursor::IDLE_MS + crate::cursor::FADE_MS + 200;
            let holds: Vec<(i64, i64)> = self.holds.iter().map(|h| (h.start_ms, h.end_ms)).collect();
            self.smooth = crate::cursor::smooth_cursor(&|t| self.cursor_at(t), &holds, end);
        }
        self
    }
}

/// "Ctrl+Shift+S" shown as "Ctrl + Shift + S".
fn chip_text(k: &str) -> String {
    k.split('+').collect::<Vec<_>>().join(" + ")
}

impl Effects {
    /// Cursor position at `t_ms`, interpolated between logged positions.
    pub fn cursor_at(&self, t_ms: i64) -> Option<(f32, f32)> {
        let first = self.cursor.first()?;
        if t_ms <= first.0 {
            return Some((first.1, first.2));
        }
        let i = self.cursor.partition_point(|c| c.0 <= t_ms);
        let a = self.cursor[i - 1];
        let Some(b) = self.cursor.get(i) else { return Some((a.1, a.2)) };
        let f = if b.0 > a.0 { (t_ms - a.0) as f32 / (b.0 - a.0) as f32 } else { 0.0 };
        Some((a.1 + (b.1 - a.1) * f, a.2 + (b.2 - a.2) * f))
    }
}

fn fade(t: i64, start: i64, end: i64, fade_in: i64, fade_out: i64) -> f32 {
    let mut a: f32 = 1.0;
    if fade_in > 0 {
        a = a.min((t - start) as f32 / fade_in as f32);
    }
    if fade_out > 0 {
        a = a.min((end - t) as f32 / fade_out as f32);
    }
    a.clamp(0.0, 1.0)
}

/// What to draw at source time `t_ms`.
pub fn draws_at(e: &Effects, t_ms: i64, on: &EffectToggles, video_h: u32) -> Vec<Draw> {
    let r = RIPPLE_RADIUS * video_h as f32;
    let mut out = Vec::new();
    if on.clicks {
        for h in e.holds.iter().filter(|h| h.start_ms <= t_ms && t_ms < h.end_ms) {
            if let Some((x, y)) = e.cursor_at(t_ms) {
                out.push(Draw::Dot { x, y, radius: DOT_RADIUS * r, alpha: DOT_ALPHA, button: h.button });
            }
        }
        for rp in e.ripples.iter().filter(|rp| rp.t_ms <= t_ms && t_ms < rp.t_ms + RIPPLE_MS) {
            let p = (t_ms - rp.t_ms) as f32 / RIPPLE_MS as f32;
            let grow = 1.0 - (1.0 - p) * (1.0 - p);
            out.push(Draw::Ring { x: rp.x, y: rp.y, radius: r * (RING_START + (1.0 - RING_START) * grow), alpha: 1.0 - p, button: rp.button });
        }
        for w in e.wheels.iter().filter(|w| w.start_ms <= t_ms && t_ms < w.end_ms) {
            if let Some((x, y)) = e.cursor_at(t_ms) {
                out.push(Draw::Wheel { x, y, up: w.up, alpha: fade(t_ms, w.start_ms, w.end_ms, 0, WHEEL_FADE_MS) });
            }
        }
    }
    if e.drawn_cursor
        && on.cursor
        && let Some(c) = crate::cursor::cursor_at(&e.smooth, t_ms)
    {
        let shape = e.cursor_ids.iter().rev().find(|(t, _)| *t <= t_ms).map_or_else(|| e.shapes.first().map_or(0, |s| s.id), |(_, id)| *id);
        if c.alpha > 0.0 && shape != 0 {
            out.push(Draw::Cursor { x: c.x, y: c.y, scale: on.cursor_size.clamp(1.0, 3.0) * c.press, alpha: c.alpha, shape });
        }
    }
    if on.keys
        && let Some(c) = e.chips.iter().find(|c| c.start_ms <= t_ms && t_ms < c.end_ms)
    {
        out.push(Draw::Chip { text: c.text.clone(), alpha: fade(t_ms, c.start_ms, c.end_ms, CHIP_FADE_MS, CHIP_FADE_MS), bottom: CHIP_BOTTOM });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;
    use crate::input_log::Header;

    const ALL: EffectToggles = EffectToggles { clicks: true, keys: true, cursor: true, cursor_size: 1.5 };

    fn log(events: Vec<Event>) -> ParsedLog {
        ParsedLog { header: None, events }
    }

    #[test]
    fn element_boxes_and_window_switches_are_in_video_pixels() {
        use crate::geometry::Rect;
        use crate::input_log::Header;
        let header = Header { v: 1, mode: "monitor".into(), region: Rect { x: 0, y: 0, w: 200, h: 100 }, width: 200, height: 100, fps: 30, cursor: false };
        let events = vec![Event::El { t: 10, k: "click".into(), r: [10, 20, 30, 40] }, Event::El { t: 20, k: "focus".into(), r: [0, 0, 50, 10] }, Event::Win { t: 30, r: [0, 0, 100, 50] }];
        let fx = build_effects(&ParsedLog { header: Some(header), events }, 400, 200);
        assert_eq!(fx.elements, vec![UiBox { t_ms: 10, kind: "click".into(), rect: [20.0, 40.0, 60.0, 80.0] }, UiBox { t_ms: 20, kind: "focus".into(), rect: [0.0, 0.0, 100.0, 20.0] }]);
        assert_eq!(fx.windows, vec![(30, [0.0, 0.0, 200.0, 100.0])]);
    }

    #[test]
    fn typing_scrolls_and_edge_guard_come_from_the_log() {
        use crate::geometry::Rect;
        use crate::input_log::Header;
        let header = |mode: &str| Header { v: 1, mode: mode.into(), region: Rect { x: 0, y: 0, w: 200, h: 100 }, width: 200, height: 100, fps: 30, cursor: false };
        let events = vec![Event::Type { t: 100 }, Event::Wheel { t: 200, dx: 0, dy: 120, x: 50, y: 40 }, Event::Type { t: 300 }];
        let fx = build_effects(&ParsedLog { header: Some(header("monitor")), events: events.clone() }, 400, 200);
        assert!(fx.edge_guard);
        assert_eq!(fx.typing, vec![100, 300]);
        assert_eq!(fx.scrolls, vec![(200, 100.0, 80.0)], "in video pixels");
        assert!(!build_effects(&ParsedLog { header: Some(header("region")), events: events.clone() }, 400, 200).edge_guard);
        assert!(!build_effects(&log(events), 400, 200).edge_guard, "no header");
    }

    fn rings(d: &[Draw]) -> Vec<(f32, f32, Button)> {
        d.iter().filter_map(|d| if let Draw::Ring { radius, alpha, button, .. } = d { Some((*radius, *alpha, *button)) } else { None }).collect()
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn left_click_makes_a_ring_that_grows_and_fades() {
        let e = build_effects(&log(vec![Event::Down { t: 1000, b: Button::Left, x: 100, y: 50 }, Event::Up { t: 1050, b: Button::Left, x: 100, y: 50 }]), 400, 400);
        let r = rings(&draws_at(&e, 1000, &ALL, 400));
        assert_eq!(r.len(), 1);
        assert!(close(r[0].0, 3.0) && close(r[0].1, 1.0), "{r:?}");
        let r = rings(&draws_at(&e, 1240, &ALL, 400));
        assert!(close(r[0].0, 8.25) && close(r[0].1, 0.5), "{r:?}");
        assert!(rings(&draws_at(&e, 1480, &ALL, 400)).is_empty());
        assert!(rings(&draws_at(&e, 999, &ALL, 400)).is_empty());
    }

    #[test]
    fn ring_colours_follow_the_button() {
        let e = build_effects(&log(vec![Event::Down { t: 0, b: Button::Right, x: 1, y: 1 }]), 100, 100);
        assert_eq!(rings(&draws_at(&e, 10, &ALL, 100))[0].2, Button::Right);
        let e = build_effects(&log(vec![Event::Down { t: 0, b: Button::Middle, x: 1, y: 1 }]), 100, 100);
        assert_eq!(rings(&draws_at(&e, 10, &ALL, 100))[0].2, Button::Middle);
    }

    #[test]
    fn holding_draws_a_dot_that_follows_the_cursor() {
        let e = build_effects(
            &log(vec![
                Event::Down { t: 0, b: Button::Left, x: 0, y: 0 },
                Event::Move { t: 100, x: 100, y: 0 },
                Event::Up { t: 200, b: Button::Left, x: 100, y: 0 },
            ]),
            400,
            400,
        );
        let dot = draws_at(&e, 50, &ALL, 400).into_iter().find_map(|d| if let Draw::Dot { x, radius, alpha, .. } = d { Some((x, radius, alpha)) } else { None });
        let (x, radius, alpha) = dot.expect("a dot while held");
        assert!(close(x, 50.0) && close(radius, 3.5) && close(alpha, 0.55));
        assert!(!draws_at(&e, 200, &ALL, 400).iter().any(|d| matches!(d, Draw::Dot { .. })));
    }

    #[test]
    fn a_press_without_release_holds_until_the_last_event() {
        let e = build_effects(&log(vec![Event::Down { t: 0, b: Button::Left, x: 0, y: 0 }, Event::Move { t: 300, x: 5, y: 5 }]), 100, 100);
        assert_eq!(e.holds, vec![Hold { start_ms: 0, end_ms: 300, button: Button::Left }]);
    }

    #[test]
    fn wheel_shows_for_400ms_after_the_last_tick() {
        let e = build_effects(
            &log(vec![Event::Wheel { t: 0, dx: 0, dy: 120, x: 5, y: 5 }, Event::Wheel { t: 300, dx: 0, dy: 120, x: 5, y: 5 }]),
            100,
            100,
        );
        assert_eq!(e.wheels, vec![Wheel { start_ms: 0, end_ms: 700, up: true }]);
    }

    #[test]
    fn changing_wheel_direction_starts_a_new_mark() {
        let e = build_effects(
            &log(vec![Event::Wheel { t: 0, dx: 0, dy: 120, x: 5, y: 5 }, Event::Wheel { t: 100, dx: 0, dy: -120, x: 5, y: 5 }]),
            100,
            100,
        );
        assert_eq!(e.wheels, vec![Wheel { start_ms: 0, end_ms: 100, up: true }, Wheel { start_ms: 100, end_ms: 500, up: false }]);
    }

    #[test]
    fn wheel_mark_fades_at_the_end() {
        let e = build_effects(&log(vec![Event::Wheel { t: 0, dx: 0, dy: -120, x: 5, y: 5 }]), 100, 100);
        let alpha = |t| draws_at(&e, t, &ALL, 100).into_iter().find_map(|d| if let Draw::Wheel { alpha, up, .. } = d { assert!(!up); Some(alpha) } else { None });
        assert_eq!(alpha(100), Some(1.0));
        assert!(close(alpha(325).unwrap(), 0.5));
        assert_eq!(alpha(400), None);
    }

    #[test]
    fn repeated_shortcut_counts_up() {
        let e = build_effects(
            &log(vec![
                Event::Key { t: 0, k: "Ctrl+S".into() },
                Event::Key { t: 1000, k: "Ctrl+S".into() },
                Event::Key { t: 3000, k: "Ctrl+Shift+S".into() },
            ]),
            100,
            100,
        );
        assert_eq!(e.chips[0], Chip { start_ms: 0, end_ms: 2500, text: "Ctrl + S ×2".into() });
        assert_eq!(e.chips[1], Chip { start_ms: 3000, end_ms: 4500, text: "Ctrl + Shift + S".into() });
    }

    #[test]
    fn a_different_shortcut_replaces_the_chip() {
        let e = build_effects(&log(vec![Event::Key { t: 0, k: "Ctrl+S".into() }, Event::Key { t: 1200, k: "Ctrl+Z".into() }]), 100, 100);
        assert_eq!(e.chips[0].end_ms, 1200);
        assert_eq!(e.chips[1], Chip { start_ms: 1200, end_ms: 2700, text: "Ctrl + Z".into() });
    }

    #[test]
    fn chip_fades_in_and_out() {
        let e = build_effects(&log(vec![Event::Key { t: 0, k: "Ctrl+C".into() }]), 100, 100);
        let alpha = |t| draws_at(&e, t, &ALL, 100).into_iter().find_map(|d| if let Draw::Chip { alpha, bottom, .. } = d { assert_eq!(bottom, CHIP_BOTTOM); Some(alpha) } else { None });
        assert_eq!(alpha(0), Some(0.0));
        assert_eq!(alpha(150), Some(1.0));
        assert!(close(alpha(1425).unwrap(), 0.5));
        assert_eq!(alpha(1500), None);
    }

    #[test]
    fn toggles_hide_effects() {
        let e = build_effects(
            &log(vec![
                Event::Down { t: 0, b: Button::Left, x: 1, y: 1 },
                Event::Wheel { t: 0, dx: 0, dy: 120, x: 1, y: 1 },
                Event::Key { t: 0, k: "Ctrl+C".into() },
            ]),
            100,
            100,
        );
        let no_clicks = draws_at(&e, 100, &EffectToggles { clicks: false, keys: true, cursor: true, cursor_size: 1.5 }, 100);
        assert!(no_clicks.iter().all(|d| matches!(d, Draw::Chip { .. })) && !no_clicks.is_empty());
        let no_keys = draws_at(&e, 100, &EffectToggles { clicks: true, keys: false, cursor: true, cursor_size: 1.5 }, 100);
        assert!(!no_keys.iter().any(|d| matches!(d, Draw::Chip { .. })) && !no_keys.is_empty());
    }

    #[test]
    fn coordinates_are_scaled_to_the_video() {
        let header = Header { v: 1, mode: "region".into(), region: Rect { x: 0, y: 0, w: 200, h: 100 }, width: 200, height: 100, fps: 30, cursor: false };
        let e = build_effects(&ParsedLog { header: Some(header), events: vec![Event::Down { t: 0, b: Button::Left, x: 50, y: 50 }] }, 400, 200);
        assert_eq!((e.ripples[0].x, e.ripples[0].y), (100.0, 100.0));
    }

    #[test]
    fn cursor_is_interpolated_and_held_at_the_ends() {
        let e = build_effects(&log(vec![Event::Move { t: 100, x: 0, y: 0 }, Event::Move { t: 200, x: 10, y: 20 }]), 100, 100);
        assert_eq!(e.cursor_at(150), Some((5.0, 10.0)));
        assert_eq!(e.cursor_at(0), Some((0.0, 0.0)));
        assert_eq!(e.cursor_at(999), Some((10.0, 20.0)));
        assert_eq!(build_effects(&log(vec![]), 100, 100).cursor_at(5), None);
    }
}
