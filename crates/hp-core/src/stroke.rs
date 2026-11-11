//! Strokes drawn on screen while recording: their geometry, how long they stay, and the area
//! they cover. Points are physical pixels relative to the captured area's origin.
use crate::geometry::Rect;
use serde::{Deserialize, Serialize};

/// A finished stroke fades out over this long once its hold time is over.
pub const FADE_OUT_MS: i64 = 500;
/// Red, yellow, green, blue, white.
pub const COLORS: [[u8; 3]; 5] = [[0xEF, 0x44, 0x44], [0xFA, 0xCC, 0x15], [0x22, 0xC5, 0x5E], [0x3B, 0x82, 0xF6], [0xFF, 0xFF, 0xFF]];
pub const WIDTHS: [f32; 3] = [4.0, 8.0, 14.0];
/// Arrow head: angle of each side to the shaft, and its length relative to the stroke width.
const HEAD_ANGLE_DEG: f32 = 28.0;
const HEAD_MIN: f32 = 12.0;
const HEAD_PER_WIDTH: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tool {
    Pen,
    Arrow,
    Rect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub id: u64,
    pub tool: Tool,
    pub color: [u8; 3],
    pub width: f32,
    pub points: Vec<(f32, f32)>,
    /// When the stroke was finished (ms on the recording clock); `None` while drawing.
    pub finished_ms: Option<i64>,
}

pub type Segment = ((f32, f32), (f32, f32));

/// The straight lines that draw `s`.
pub fn segments(s: &Stroke) -> Vec<Segment> {
    let (Some(&first), Some(&last)) = (s.points.first(), s.points.last()) else { return Vec::new() };
    if s.points.len() == 1 {
        return vec![(first, first)];
    }
    match s.tool {
        Tool::Pen => s.points.windows(2).map(|w| (w[0], w[1])).collect(),
        Tool::Arrow => {
            let mut out = vec![(first, last)];
            let (dx, dy) = (last.0 - first.0, last.1 - first.1);
            let len = (dx * dx + dy * dy).sqrt();
            if len > 0.0 {
                let (bx, by) = (-dx / len, -dy / len);
                let head = HEAD_MIN.max(HEAD_PER_WIDTH * s.width);
                let (sin, cos) = HEAD_ANGLE_DEG.to_radians().sin_cos();
                for sign in [1.0, -1.0] {
                    let (rx, ry) = (bx * cos - by * sin * sign, bx * sin * sign + by * cos);
                    out.push((last, (last.0 + rx * head, last.1 + ry * head)));
                }
            }
            out
        }
        Tool::Rect => {
            let (a, b) = ((first.0, last.1), (last.0, first.1));
            vec![(first, b), (b, last), (last, a), (a, first)]
        }
    }
}

/// Opacity of `s` at `now_ms`, when finished strokes stay for `hold_ms` (0: forever).
pub fn alpha_at(s: &Stroke, now_ms: i64, hold_ms: i64) -> f32 {
    let Some(done) = s.finished_ms else { return 1.0 };
    if hold_ms <= 0 {
        return 1.0;
    }
    let past = now_ms - done - hold_ms;
    if past <= 0 {
        return 1.0;
    }
    (1.0 - past as f32 / FADE_OUT_MS as f32).max(0.0)
}

/// The part of a `w`×`h` frame covered by strokes still visible at `now_ms`.
pub fn visible_bounds(strokes: &[Stroke], now_ms: i64, hold_ms: i64, w: u32, h: u32) -> Option<Rect> {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for s in strokes.iter().filter(|s| alpha_at(s, now_ms, hold_ms) > 0.0) {
        let pad = s.width / 2.0 + 2.0;
        for ((ax, ay), (bx, by)) in segments(s) {
            x0 = x0.min(ax.min(bx) - pad);
            y0 = y0.min(ay.min(by) - pad);
            x1 = x1.max(ax.max(bx) + pad);
            y1 = y1.max(ay.max(by) + pad);
        }
    }
    let left = x0.floor().max(0.0);
    let top = y0.floor().max(0.0);
    let right = x1.ceil().min(w as f32);
    let bottom = y1.ceil().min(h as f32);
    if !(left < right && top < bottom) {
        return None;
    }
    Some(Rect { x: left as i32, y: top as i32, w: (right - left) as u32, h: (bottom - top) as u32 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(tool: Tool, points: Vec<(f32, f32)>, width: f32) -> Stroke {
        Stroke { id: 1, tool, color: COLORS[0], width, points, finished_ms: None }
    }

    fn len(((x0, y0), (x1, y1)): Segment) -> f32 {
        ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt()
    }

    #[test]
    fn pen_segments_join_points() {
        let s = stroke(Tool::Pen, vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], 4.0);
        assert_eq!(segments(&s), vec![((0.0, 0.0), (10.0, 0.0)), ((10.0, 0.0), (10.0, 10.0))]);
    }

    #[test]
    fn arrow_has_shaft_and_two_head_lines() {
        let s = stroke(Tool::Arrow, vec![(0.0, 0.0), (50.0, 3.0), (100.0, 0.0)], 4.0);
        let seg = segments(&s);
        assert_eq!(seg.len(), 3);
        assert_eq!(seg[0], ((0.0, 0.0), (100.0, 0.0)), "shaft from first to last point");
        for head in &seg[1..] {
            assert_eq!(head.0, (100.0, 0.0));
            assert!((len(*head) - 16.0).abs() < 0.01, "{head:?}");
            assert!((head.1).0 < 100.0);
        }
        assert!(((seg[1].1).1 + (seg[2].1).1).abs() < 0.01, "head sides are symmetric");
    }

    #[test]
    fn rect_has_four_edges() {
        let s = stroke(Tool::Rect, vec![(10.0, 20.0), (15.0, 25.0), (30.0, 60.0)], 4.0);
        let seg = segments(&s);
        assert_eq!(seg.len(), 4);
        let corners: Vec<(f32, f32)> = seg.iter().map(|s| s.0).collect();
        assert_eq!(corners, vec![(10.0, 20.0), (30.0, 20.0), (30.0, 60.0), (10.0, 60.0)]);
    }

    #[test]
    fn single_point_gives_one_zero_length_segment() {
        for tool in [Tool::Pen, Tool::Arrow, Tool::Rect] {
            assert_eq!(segments(&stroke(tool, vec![(5.0, 6.0)], 4.0)), vec![((5.0, 6.0), (5.0, 6.0))], "{tool:?}");
        }
        assert!(segments(&stroke(Tool::Pen, vec![], 4.0)).is_empty());
    }

    #[test]
    fn alpha_holds_then_fades() {
        let mut s = stroke(Tool::Pen, vec![(0.0, 0.0)], 4.0);
        assert_eq!(alpha_at(&s, 99_999, 3000), 1.0, "still drawing");
        s.finished_ms = Some(1000);
        assert_eq!(alpha_at(&s, 3999, 3000), 1.0);
        assert!((alpha_at(&s, 4250, 3000) - 0.5).abs() < 0.01);
        assert_eq!(alpha_at(&s, 4500, 3000), 0.0);
        assert_eq!(alpha_at(&s, 99_999, 0), 1.0, "no fading");
    }

    #[test]
    fn bounds_cover_visible_strokes_and_clip() {
        let s = stroke(Tool::Pen, vec![(-20.0, 10.0), (50.0, 30.0)], 8.0);
        let r = visible_bounds(&[s.clone()], 0, 3000, 40, 100).unwrap();
        assert_eq!((r.x, r.y), (0, 4), "left edge clipped, top = 10 - 4 - 2");
        assert_eq!(r.w, 40, "right edge clipped to the frame");
        assert_eq!(r.y + r.h as i32, 36, "bottom = 30 + 4 + 2");
        let mut gone = s;
        gone.finished_ms = Some(0);
        assert_eq!(visible_bounds(&[gone], 10_000, 3000, 40, 100), None);
        let outside = stroke(Tool::Pen, vec![(500.0, 500.0), (600.0, 600.0)], 4.0);
        assert_eq!(visible_bounds(&[outside], 0, 3000, 40, 100), None);
    }
}
