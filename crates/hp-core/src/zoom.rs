//! Zoom: segments of the video shown closer, made from clicks or by hand, and the camera path
//! (centre and zoom level over time) that moves smoothly between them.
use crate::spring::{CAMERA, SpringState};
use serde::{Deserialize, Serialize};

pub const DEFAULT_SCALE: f32 = 1.5;
pub const MIN_SCALE: f32 = 1.0;
pub const MAX_SCALE: f32 = 4.0;
/// Above this the picture visibly softens.
pub const SOFT_SCALE: f32 = 2.0;
/// Clicks closer than this in time and place make one segment.
pub const GROUP_GAP_MS: i64 = 2500;
pub const GROUP_DISTANCE: f32 = 0.15;
/// Zoom starts this long before the first click of a group and holds after the last.
pub const LEAD_MS: i64 = 400;
pub const HOLD_MS: i64 = 1800;
/// Segments closer than this are merged; shorter ones are dropped.
pub const MERGE_GAP_MS: i64 = 800;
pub const MIN_SEGMENT_MS: i64 = 1000;
/// While zoomed, the camera follows the cursor only when it leaves this central part of the view.
pub const DEAD_ZONE: f32 = 0.7;
/// ...and stays out this long (a quick excursion is ignored).
pub const LOOK_AHEAD_MS: i64 = 1000;
/// Keyframes closer than this to the previous one are dropped.
pub const KEY_MIN_GAP_MS: i64 = 50;
/// Time factor of the camera spring for each speed.
pub const SLOW_FACTOR: f32 = 0.6;
pub const FAST_FACTOR: f32 = 2.0;

/// A keyframe inside a zoom segment: from here the camera aims at this frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZoomKey {
    /// Source time, strictly inside the segment.
    pub t_ms: i64,
    pub cx: f32,
    pub cy: f32,
    pub scale: f32,
}

/// How fast the camera zooms in, out and pans in a segment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speed {
    Slow,
    #[default]
    Normal,
    Fast,
    /// No motion: the camera cuts to each frame.
    Instant,
}

impl Speed {
    /// Time factor of the camera spring; `None` for a cut.
    pub fn factor(self) -> Option<f32> {
        match self {
            Speed::Slow => Some(SLOW_FACTOR),
            Speed::Normal => Some(1.0),
            Speed::Fast => Some(FAST_FACTOR),
            Speed::Instant => None,
        }
    }
}

/// How eagerly automatic zoom zooms.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Low,
    #[default]
    Normal,
    High,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ZoomSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub scale: f32,
    /// Fixed centre (0–1 of the frame); `None` follows the cursor, or stays centred without one.
    pub center: Option<(f32, f32)>,
    /// Added or changed by the user; "Tự tạo lại" keeps it.
    pub manual: bool,
    /// Keyframes after the start (the start is `center` and `scale`).
    pub keys: Vec<ZoomKey>,
    pub speed: Speed,
}

impl Default for ZoomSegment {
    fn default() -> Self {
        ZoomSegment { start_ms: 0, end_ms: 0, scale: DEFAULT_SCALE, center: None, manual: false, keys: Vec::new(), speed: Speed::Normal }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ZoomDoc {
    pub enabled: bool,
    pub segments: Vec<ZoomSegment>,
    /// Level used for automatic segments.
    pub level: Level,
}

/// Where the camera looks: centre (0–1 of the frame) and zoom level.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Camera {
    pub cx: f32,
    pub cy: f32,
    pub scale: f32,
}

impl Camera {
    pub const WHOLE: Camera = Camera { cx: 0.5, cy: 0.5, scale: 1.0 };

    /// The part of a `w`×`h` frame in view: (x, y, width, height) in pixels.
    pub fn view(&self, w: u32, h: u32) -> (f32, f32, f32, f32) {
        let s = self.scale.max(MIN_SCALE);
        let (vw, vh) = (w as f32 / s, h as f32 / s);
        (self.cx * w as f32 - vw / 2.0, self.cy * h as f32 - vh / 2.0, vw, vh)
    }
}

/// Segments made from clicks `(ms, x, y)` with coordinates 0–1 of the frame.
pub fn auto_segments(clicks: &[(i64, f32, f32)], duration_ms: i64) -> Vec<ZoomSegment> {
    let mut sorted = clicks.to_vec();
    sorted.sort_by_key(|c| c.0);
    // Groups of clicks close in time and place.
    let mut groups: Vec<Vec<(i64, f32, f32)>> = Vec::new();
    for c in sorted {
        let joins = groups.last().and_then(|g| g.last()).is_some_and(|last| {
            let d = ((c.1 - last.1).powi(2) + (c.2 - last.2).powi(2)).sqrt();
            c.0 - last.0 < GROUP_GAP_MS && d < GROUP_DISTANCE
        });
        match groups.last_mut() {
            Some(g) if joins => g.push(c),
            _ => groups.push(vec![c]),
        }
    }
    let centroid = |g: &[(i64, f32, f32)]| {
        let n = g.len().max(1) as f32;
        (g.iter().map(|c| c.1).sum::<f32>() / n, g.iter().map(|c| c.2).sum::<f32>() / n)
    };
    // Each group's time span, merged with the next when they nearly touch. A span made of
    // groups far apart has no single good centre: it follows the cursor instead.
    struct Span {
        start: i64,
        end: i64,
        clicks: Vec<(i64, f32, f32)>,
        spread: bool,
    }
    let mut spans: Vec<Span> = Vec::new();
    for g in groups {
        let start = (g[0].0 - LEAD_MS).max(0);
        let end = (g[g.len() - 1].0 + HOLD_MS).min(duration_ms);
        match spans.last_mut() {
            Some(last) if start - last.end < MERGE_GAP_MS => {
                let (a, b) = (centroid(&last.clicks), centroid(&g));
                last.spread |= ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt() >= GROUP_DISTANCE;
                last.end = last.end.max(end);
                last.clicks.extend(g);
            }
            _ => spans.push(Span { start, end, clicks: g, spread: false }),
        }
    }
    spans
        .into_iter()
        .filter(|s| s.end - s.start >= MIN_SEGMENT_MS)
        .map(|s| ZoomSegment {
            start_ms: s.start,
            end_ms: s.end,
            scale: DEFAULT_SCALE,
            center: (!s.spread).then(|| centroid(&s.clicks)),
            ..ZoomSegment::default()
        })
        .collect()
}

/// New automatic segments, keeping every manual one (automatic ones overlapping them are dropped).
pub fn regenerate(existing: &[ZoomSegment], auto: Vec<ZoomSegment>) -> Vec<ZoomSegment> {
    let manual: Vec<ZoomSegment> = existing.iter().filter(|s| s.manual).cloned().collect();
    let overlaps = |a: &ZoomSegment| manual.iter().any(|m| a.start_ms < m.end_ms && m.start_ms < a.end_ms);
    let mut out: Vec<ZoomSegment> = auto.into_iter().filter(|a| !overlaps(a)).collect();
    out.extend(manual.iter().cloned());
    out.sort_by_key(|s| s.start_ms);
    out
}

fn clamp_scale(s: f32) -> f32 {
    if s.is_finite() { s.clamp(MIN_SCALE, MAX_SCALE) } else { DEFAULT_SCALE }
}

fn clamp01(v: f32) -> f32 {
    if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.5 }
}

/// One segment made valid: inside the video, values in range, keyframes sorted, inside the
/// segment and apart; a segment with keyframes always has a fixed centre.
fn cleaned(s: &ZoomSegment, duration_ms: i64) -> ZoomSegment {
    let (start_ms, end_ms) = (s.start_ms.max(0), s.end_ms.min(duration_ms));
    let mut keys: Vec<ZoomKey> = s
        .keys
        .iter()
        .filter(|k| k.t_ms > start_ms && k.t_ms < end_ms)
        .map(|k| ZoomKey { t_ms: k.t_ms, cx: clamp01(k.cx), cy: clamp01(k.cy), scale: clamp_scale(k.scale) })
        .collect();
    keys.sort_by_key(|k| k.t_ms);
    let mut last = start_ms;
    keys.retain(|k| {
        let keep = k.t_ms - last > KEY_MIN_GAP_MS;
        if keep {
            last = k.t_ms;
        }
        keep
    });
    let center = s.center.map(|(x, y)| (clamp01(x), clamp01(y))).or_else(|| keys.first().map(|k| (k.cx, k.cy)));
    ZoomSegment { start_ms, end_ms, scale: clamp_scale(s.scale), center, manual: s.manual, keys, speed: s.speed }
}

/// The part `[a, b)` of `s`: it starts from the frame the camera aims at at `a`.
fn part(s: &ZoomSegment, a: i64, b: i64) -> ZoomSegment {
    let mut p = ZoomSegment { start_ms: a, end_ms: b, keys: s.keys.iter().filter(|k| k.t_ms > a && k.t_ms < b).copied().collect(), ..s.clone() };
    if a > s.start_ms
        && let Some(c) = target_at(s, a)
    {
        p.center = Some((c.cx, c.cy));
        p.scale = c.scale;
    }
    p
}

/// Valid segments only, in order: no empty ones, scale and centre within range, no overlaps
/// (the later segment wins the overlapping part).
pub fn normalized(segments: &[ZoomSegment], duration_ms: i64) -> Vec<ZoomSegment> {
    let mut v: Vec<ZoomSegment> = segments.iter().map(|s| cleaned(s, duration_ms)).filter(|s| s.end_ms > s.start_ms).collect();
    v.sort_by_key(|s| s.start_ms);
    // Where segments overlap, the one that starts later wins; a segment inside another splits it.
    let mut cuts: Vec<i64> = v.iter().flat_map(|s| [s.start_ms, s.end_ms]).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut pieces: Vec<(usize, i64, i64)> = Vec::new();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let winner = v.iter().enumerate().filter(|(_, s)| s.start_ms <= a && s.end_ms >= b).max_by_key(|(i, s)| (s.start_ms, *i));
        let Some((i, _)) = winner else { continue };
        match pieces.last_mut() {
            Some((last_i, _, end)) if *last_i == i && *end == a => *end = b,
            _ => pieces.push((i, a, b)),
        }
    }
    pieces.into_iter().map(|(i, a, b)| part(&v[i], a, b)).collect()
}

/// Where the camera aims in `seg` at `t_ms` from its frames: the start frame and keyframes, eased
/// between them (`smoothstep`, zoom in log space), held after the last. `None` when the segment
/// follows the cursor (no fixed centre, no keyframes).
pub fn target_at(seg: &ZoomSegment, t_ms: i64) -> Option<Camera> {
    let (cx, cy) = match seg.center {
        Some(c) => c,
        None => seg.keys.first().map(|k| (k.cx, k.cy))?,
    };
    let mut prev = (seg.start_ms, Camera { cx, cy, scale: seg.scale });
    for k in &seg.keys {
        let next = (k.t_ms, Camera { cx: k.cx, cy: k.cy, scale: k.scale });
        if t_ms < k.t_ms {
            if seg.speed == Speed::Instant {
                return Some(prev.1);
            }
            let u = ((t_ms - prev.0) as f32 / (next.0 - prev.0).max(1) as f32).clamp(0.0, 1.0);
            let u = u * u * (3.0 - 2.0 * u);
            let (a, b) = (prev.1, next.1);
            let ln = |s: f32| s.max(MIN_SCALE).ln();
            return Some(Camera {
                cx: a.cx + (b.cx - a.cx) * u,
                cy: a.cy + (b.cy - a.cy) * u,
                scale: (ln(a.scale) + (ln(b.scale) - ln(a.scale)) * u).exp(),
            });
        }
        prev = next;
    }
    Some(prev.1)
}

/// The camera every `step_ms` from 0 to `duration_ms`, moved by the camera spring in 1 ms steps.
pub fn camera_track(z: &ZoomDoc, cursor: &dyn Fn(i64) -> Option<(f32, f32)>, duration_ms: i64, step_ms: i64) -> Vec<(i64, Camera)> {
    let step_ms = step_ms.max(1);
    let duration_ms = duration_ms.max(0);
    if !z.enabled {
        return (0..=duration_ms / step_ms).map(|i| (i * step_ms, Camera::WHOLE)).collect();
    }
    let segments = normalized(&z.segments, duration_ms);
    let (mut scale, mut cx, mut cy) = (
        SpringState { value: 1.0, velocity: 0.0 },
        SpringState { value: 0.5, velocity: 0.0 },
        SpringState { value: 0.5, velocity: 0.0 },
    );
    let mut out = Vec::with_capacity((duration_ms / step_ms + 2) as usize);
    let mut seg_i = 0usize;
    // The speed of the segment the camera is in, or of the one it just left.
    let mut speed = Speed::Normal;
    // Where the camera aims inside the current segment, when it follows the cursor.
    let mut follow: Option<(usize, (f32, f32))> = None;
    for t in 0..=duration_ms {
        while seg_i < segments.len() && segments[seg_i].end_ms <= t {
            seg_i += 1;
        }
        let current = segments.get(seg_i).filter(|s| s.start_ms <= t);
        let (target_scale, target) = match current {
            None => (1.0, (0.5, 0.5)),
            Some(s) => {
                speed = s.speed;
                let fixed = target_at(s, t);
                let aim = match fixed {
                    Some(c) => (c.cx, c.cy),
                    None => match cursor(t) {
                        None => (0.5, 0.5),
                        Some(now) => {
                            // What can be shown: the view never leaves the frame.
                            let half = 0.5 / s.scale;
                            let inside = |p: (f32, f32)| (p.0.clamp(half, 1.0 - half), p.1.clamp(half, 1.0 - half));
                            let aim = inside(match follow {
                                Some((i, aim)) if i == seg_i => aim,
                                _ => cursor(s.start_ms).unwrap_or(now),
                            });
                            // Move only when the cursor leaves the central part of the view
                            // and is still out a moment later.
                            let reach = DEAD_ZONE * half;
                            let outside = |p: (f32, f32)| (p.0 - aim.0).abs() > reach || (p.1 - aim.1).abs() > reach;
                            let aim = if outside(now) && cursor(t + LOOK_AHEAD_MS).is_none_or(outside) { inside(now) } else { aim };
                            follow = Some((seg_i, aim));
                            aim
                        }
                    },
                };
                (fixed.map_or(s.scale, |c| c.scale), aim)
            }
        };
        if t > 0 {
            match speed.factor() {
                Some(f) => {
                    let spring = if f == 1.0 { CAMERA } else { CAMERA.scaled(f) };
                    spring.step(&mut scale, target_scale, 0.001);
                    spring.step(&mut cx, target.0, 0.001);
                    spring.step(&mut cy, target.1, 0.001);
                }
                None => {
                    scale = SpringState { value: target_scale, velocity: 0.0 };
                    cx = SpringState { value: target.0, velocity: 0.0 };
                    cy = SpringState { value: target.1, velocity: 0.0 };
                }
            }
        }
        let s = scale.value.clamp(MIN_SCALE, MAX_SCALE);
        let half = 0.5 / s;
        cx.value = cx.value.clamp(half, 1.0 - half);
        cy.value = cy.value.clamp(half, 1.0 - half);
        if t % step_ms == 0 {
            out.push((t, Camera { cx: cx.value, cy: cy.value, scale: s }));
        }
    }
    out
}

/// The camera at `t_ms`, interpolated between samples.
pub fn camera_at(track: &[(i64, Camera)], t_ms: i64) -> Camera {
    let (Some(first), Some(last)) = (track.first(), track.last()) else { return Camera::WHOLE };
    if t_ms <= first.0 {
        return first.1;
    }
    if t_ms >= last.0 {
        return last.1;
    }
    let i = track.partition_point(|(t, _)| *t <= t_ms);
    let (t0, a) = track[i - 1];
    let (t1, b) = track[i];
    let f = (t_ms - t0) as f32 / (t1 - t0).max(1) as f32;
    let mix = |x: f32, y: f32| x + (y - x) * f;
    Camera { cx: mix(a.cx, b.cx), cy: mix(a.cy, b.cy), scale: mix(a.scale, b.scale) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start_ms: i64, end_ms: i64) -> ZoomSegment {
        ZoomSegment { start_ms, end_ms, ..ZoomSegment::default() }
    }

    fn manual(start_ms: i64, end_ms: i64, scale: f32, center: Option<(f32, f32)>) -> ZoomSegment {
        ZoomSegment { start_ms, end_ms, scale, center, manual: true, ..ZoomSegment::default() }
    }

    fn key(t_ms: i64, cx: f32, cy: f32, scale: f32) -> ZoomKey {
        ZoomKey { t_ms, cx, cy, scale }
    }

    fn panning(speed: Speed) -> ZoomSegment {
        ZoomSegment { keys: vec![key(3000, 0.75, 0.75, 2.0)], speed, ..manual(0, 6000, 2.0, Some((0.25, 0.25))) }
    }

    fn close(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn clicks_close_in_time_and_place_make_one_segment() {
        let s = auto_segments(&[(1000, 0.2, 0.4), (2000, 0.3, 0.4)], 60_000);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].start_ms, s[0].end_ms), (600, 3800));
        let (cx, cy) = s[0].center.unwrap();
        assert!(close(cx, 0.25, 1e-4) && close(cy, 0.4, 1e-4));
        assert_eq!(s[0].scale, DEFAULT_SCALE);
        assert!(!s[0].manual);
    }

    #[test]
    fn far_apart_clicks_split() {
        assert_eq!(auto_segments(&[(1000, 0.2, 0.2), (9000, 0.2, 0.2)], 60_000).len(), 2, "far in time");
        assert_eq!(auto_segments(&[(1000, 0.1, 0.1), (1500, 0.9, 0.9)], 60_000).len(), 1, "far in place but merged: gap < 800 ms");
        let s = auto_segments(&[(1000, 0.1, 0.1), (4000, 0.9, 0.9)], 60_000);
        assert_eq!(s.len(), 2, "[600, 2800) and [3600, 5800): a gap of exactly 800 ms is not merged");
        let s = auto_segments(&[(1000, 0.1, 0.1), (6000, 0.9, 0.9)], 60_000);
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn merged_clicks_far_apart_follow_the_cursor() {
        // Two clicks 500 ms apart at opposite corners: one span, but no single centre shows both.
        let s = auto_segments(&[(1000, 0.1, 0.1), (1500, 0.9, 0.9)], 60_000);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].center, None, "follows the cursor instead of zooming on the empty middle");
        let near = auto_segments(&[(1000, 0.2, 0.2), (1500, 0.25, 0.2)], 60_000);
        assert!(near[0].center.is_some(), "clicks close together keep their centre");
    }

    #[test]
    fn a_segment_inside_another_splits_it() {
        let out = normalized(&[manual(0, 10_000, 2.0, None), manual(2000, 3000, 3.0, None)], 20_000);
        let spans: Vec<(i64, i64, f32)> = out.iter().map(|s| (s.start_ms, s.end_ms, s.scale)).collect();
        assert_eq!(spans, vec![(0, 2000, 2.0), (2000, 3000, 3.0), (3000, 10_000, 2.0)]);
    }

    #[test]
    fn near_an_edge_the_dead_zone_is_measured_from_what_is_shown() {
        // Aiming at 0.98 at 2x shows 0.5..1.0 (centre 0.75). A cursor at 0.6 is inside the visible
        // central 70% (0.575..0.925), so the camera must not move away from the edge.
        let t = camera_track(&on(vec![manual(0, 10_000, 2.0, None)]), &|ms| Some(if ms < 3000 { (0.98, 0.5) } else { (0.6, 0.5) }), 10_000, 10);
        assert!(close(camera_at(&t, 2900).cx, 0.75, 0.01), "{:?}", camera_at(&t, 2900));
        assert!(close(camera_at(&t, 8000).cx, 0.75, 0.01), "stays: {:?}", camera_at(&t, 8000));
    }

    #[test]
    fn nearby_segments_merge() {
        // [600, 2800) and [3200, 5400): 400 ms apart.
        let s = auto_segments(&[(1000, 0.1, 0.1), (3600, 0.8, 0.8)], 60_000);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].start_ms, s[0].end_ms), (600, 5400));
    }

    #[test]
    fn short_segments_are_dropped() {
        assert!(auto_segments(&[(100, 0.5, 0.5)], 900).is_empty(), "clamped to [0, 900): too short");
        assert_eq!(auto_segments(&[(100, 0.5, 0.5)], 60_000).len(), 1);
    }

    #[test]
    fn segments_are_clamped_to_the_video() {
        let s = auto_segments(&[(100, 0.5, 0.5), (8500, 0.5, 0.5)], 10_000);
        assert_eq!(s.first().unwrap().start_ms, 0);
        assert_eq!(s.last().unwrap().end_ms, 10_000);
    }

    #[test]
    fn regenerate_keeps_manual_segments() {
        let existing = vec![seg(0, 2000), manual(5000, 7000, 2.0, Some((0.1, 0.1)))];
        let fresh = vec![seg(1000, 3000), seg(6000, 8000), seg(9000, 11000)];
        let out = regenerate(&existing, fresh);
        assert_eq!(out, vec![seg(1000, 3000), manual(5000, 7000, 2.0, Some((0.1, 0.1))), seg(9000, 11000)]);
    }

    #[test]
    fn normalized_drops_empty_and_clamps() {
        let out = normalized(
            &[
                ZoomSegment { scale: 9.0, center: Some((-1.0, 2.0)), ..seg(0, 1000) },
                seg(500, 500),
                seg(3000, 2000),
                ZoomSegment { scale: 0.2, ..seg(800, 1500) },
                seg(20_000, 30_000),
            ],
            10_000,
        );
        assert_eq!(out.len(), 2);
        assert_eq!((out[0].start_ms, out[0].end_ms, out[0].scale, out[0].center), (0, 800, MAX_SCALE, Some((0.0, 1.0))));
        assert_eq!((out[1].start_ms, out[1].end_ms, out[1].scale), (800, 1500, MIN_SCALE));
    }

    #[test]
    fn old_edit_files_read_without_zoom() {
        let doc = crate::edit::load_edit("{\"v\":1}").unwrap();
        assert!(!doc.zoom.enabled && doc.zoom.segments.is_empty());
    }

    const NO_CURSOR: &dyn Fn(i64) -> Option<(f32, f32)> = &|_| None;

    fn on(segments: Vec<ZoomSegment>) -> ZoomDoc {
        ZoomDoc { enabled: true, segments, ..ZoomDoc::default() }
    }

    #[test]
    fn disabled_zoom_is_identity() {
        let z = ZoomDoc { enabled: false, segments: vec![manual(0, 5000, 3.0, None)], ..ZoomDoc::default() };
        let t = camera_track(&z, NO_CURSOR, 5000, 100);
        assert_eq!(t.len(), 51);
        assert!(t.iter().all(|(_, c)| *c == Camera::WHOLE));
    }

    #[test]
    fn zoom_eases_in_and_out() {
        let t = camera_track(&on(vec![manual(1000, 3000, 2.0, Some((0.5, 0.5)))]), NO_CURSOR, 8000, 10);
        assert!(close(camera_at(&t, 0).scale, 1.0, 1e-4));
        assert!(close(camera_at(&t, 2900).scale, 2.0, 0.05), "{:?}", camera_at(&t, 2900));
        assert!(close(camera_at(&t, 7000).scale, 1.0, 0.05), "{:?}", camera_at(&t, 7000));
        assert!(t.iter().all(|(_, c)| c.scale <= 2.0 + 1e-4), "no overshoot");
        assert!(camera_at(&t, 1100).scale > 1.0 && camera_at(&t, 1100).scale < 1.5, "eases in, does not jump");
    }

    #[test]
    fn corner_center_is_kept_inside_the_video() {
        let t = camera_track(&on(vec![manual(0, 5000, 2.0, Some((0.0, 0.0)))]), NO_CURSOR, 5000, 10);
        for (_, c) in &t {
            assert!(c.cx >= 0.5 / c.scale - 1e-4 && c.cy >= 0.5 / c.scale - 1e-4, "{c:?}");
        }
        let late = camera_at(&t, 4900);
        assert!(close(late.cx, 0.25, 0.01) && close(late.cy, 0.25, 0.01), "{late:?}");
    }

    #[test]
    fn segments_without_a_centre_or_cursor_stay_centred() {
        let t = camera_track(&on(vec![manual(0, 5000, 2.0, None)]), NO_CURSOR, 5000, 10);
        let c = camera_at(&t, 4000);
        assert!(close(c.cx, 0.5, 1e-3) && close(c.cy, 0.5, 1e-3) && close(c.scale, 2.0, 0.01));
    }

    fn follow(cursor: &dyn Fn(i64) -> Option<(f32, f32)>) -> Vec<(i64, Camera)> {
        camera_track(&on(vec![manual(0, 10_000, 2.0, None)]), cursor, 10_000, 10)
    }

    #[test]
    fn cursor_inside_the_dead_zone_does_not_move_the_camera() {
        let t = follow(&|ms| Some(if ms < 2000 { (0.5, 0.5) } else { (0.6, 0.5) }));
        assert!(close(camera_at(&t, 6000).cx, 0.5, 0.01), "{:?}", camera_at(&t, 6000));
    }

    #[test]
    fn cursor_leaving_and_staying_out_moves_the_camera() {
        let t = follow(&|ms| Some(if ms < 2000 { (0.5, 0.5) } else { (0.9, 0.5) }));
        assert!(close(camera_at(&t, 7000).cx, 0.75, 0.02), "follows, kept inside: {:?}", camera_at(&t, 7000));
    }

    #[test]
    fn cursor_that_comes_back_within_a_second_does_not() {
        let t = follow(&|ms| Some(if (2000..2500).contains(&ms) { (0.9, 0.5) } else { (0.5, 0.5) }));
        assert!(close(camera_at(&t, 6000).cx, 0.5, 0.01), "{:?}", camera_at(&t, 6000));
    }

    #[test]
    fn scale_1_view_is_the_whole_frame() {
        assert_eq!(Camera::WHOLE.view(1920, 1080), (0.0, 0.0, 1920.0, 1080.0));
    }

    #[test]
    fn view_at_4x_is_a_quarter() {
        assert_eq!(Camera { cx: 0.5, cy: 0.5, scale: 4.0 }.view(1600, 800), (600.0, 300.0, 400.0, 200.0));
    }

    #[test]
    fn camera_at_interpolates_and_holds_at_the_ends() {
        let t = vec![(0, Camera { cx: 0.5, cy: 0.5, scale: 1.0 }), (100, Camera { cx: 0.5, cy: 0.5, scale: 2.0 })];
        assert!(close(camera_at(&t, 50).scale, 1.5, 1e-4));
        assert_eq!(camera_at(&t, -5).scale, 1.0);
        assert_eq!(camera_at(&t, 500).scale, 2.0);
        assert_eq!(camera_at(&[], 5), Camera::WHOLE);
    }

    #[test]
    fn old_segments_read_with_defaults() {
        let doc = crate::edit::load_edit(r#"{"v":1,"zoom":{"enabled":true,"segments":[{"start_ms":0,"end_ms":1000,"scale":2,"center":[0.5,0.5],"manual":true}]}}"#).unwrap();
        let s = &doc.zoom.segments[0];
        assert!(s.keys.is_empty());
        assert_eq!(s.speed, Speed::Normal);
        assert_eq!(doc.zoom.level, Level::Normal);
        let json = serde_json::to_value(&doc.zoom).unwrap();
        assert_eq!(json["level"], "normal");
        assert_eq!(json["segments"][0]["speed"], "normal");
    }

    #[test]
    fn keys_are_reached_at_their_time() {
        let t = camera_track(&on(vec![panning(Speed::Fast)]), NO_CURSOR, 6000, 1);
        let c = camera_at(&t, 3200);
        assert!(close(c.cx, 0.75, 0.02) && close(c.cy, 0.75, 0.02), "{c:?}");
        let early = camera_at(&t, 1000);
        assert!(early.cx < 0.5, "still near the main frame: {early:?}");
    }

    #[test]
    fn target_eases_between_keys() {
        let s = panning(Speed::Normal);
        let mid = target_at(&s, 1500).unwrap();
        assert!(close(mid.cx, 0.5, 1e-4), "{mid:?}");
        let quarter = target_at(&s, 750).unwrap();
        assert!(quarter.cx < 0.25 + 0.125, "smoothstep is slow at first: {quarter:?}");
        let after = target_at(&s, 5000).unwrap();
        assert!(close(after.cx, 0.75, 1e-6), "held after the last key");
        let zoom = ZoomSegment { keys: vec![key(2000, 0.5, 0.5, 4.0)], ..manual(0, 4000, 1.0, Some((0.5, 0.5))) };
        assert!(close(target_at(&zoom, 1000).unwrap().scale, 2.0, 1e-4), "scale eases in log space");
        assert_eq!(target_at(&manual(0, 1000, 2.0, None), 500), None, "follows the cursor");
    }

    #[test]
    fn instant_jumps_to_the_target() {
        let t = camera_track(&on(vec![panning(Speed::Instant)]), NO_CURSOR, 7000, 1);
        let at = |ms| camera_at(&t, ms);
        assert!(close(at(1).cx, 0.25, 1e-4) && close(at(1).scale, 2.0, 1e-4), "{:?}", at(1));
        assert!(close(at(2999).cx, 0.25, 1e-4), "no easing between keys: {:?}", at(2999));
        assert!(close(at(3001).cx, 0.75, 1e-4), "{:?}", at(3001));
        assert!(close(at(6001).scale, 1.0, 1e-4), "leaves at once: {:?}", at(6001));
    }

    #[test]
    fn slow_is_slower_and_fast_is_faster() {
        let reach = |speed| {
            let s = ZoomSegment { speed, ..manual(0, 10_000, 2.0, Some((0.5, 0.5))) };
            let t = camera_track(&on(vec![s]), NO_CURSOR, 10_000, 1);
            t.iter().find(|(_, c)| c.scale >= 1.9).map(|(ms, _)| *ms).unwrap()
        };
        let (slow, normal, fast) = (reach(Speed::Slow), reach(Speed::Normal), reach(Speed::Fast));
        assert!(fast < normal && normal < slow, "{fast} {normal} {slow}");
    }

    #[test]
    fn normalized_sorts_and_drops_bad_keys() {
        let s = ZoomSegment {
            keys: vec![key(3000, 0.5, 0.5, 9.0), key(1000, 2.0, 0.5, 2.0), key(1040, 0.3, 0.3, 2.0), key(5000, 0.5, 0.5, 2.0), key(0, 0.5, 0.5, 2.0)],
            ..manual(0, 4000, 2.0, None)
        };
        let out = normalized(&[s], 10_000);
        let k = &out[0].keys;
        assert_eq!(k.iter().map(|k| k.t_ms).collect::<Vec<_>>(), vec![1000, 3000], "outside and too-close keys dropped");
        assert_eq!((k[0].cx, k[1].scale), (1.0, MAX_SCALE), "clamped");
        assert_eq!(out[0].center, Some((1.0, 0.5)), "a segment with keys gets a fixed centre");
    }

    #[test]
    fn a_split_segment_keeps_keys_in_each_part() {
        let a = ZoomSegment { keys: vec![key(1000, 0.4, 0.4, 2.0), key(5000, 0.8, 0.8, 2.0)], ..manual(0, 6000, 2.0, Some((0.2, 0.2))) };
        let out = normalized(&[a.clone(), manual(2000, 3000, 3.0, None)], 10_000);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].keys.iter().map(|k| k.t_ms).collect::<Vec<_>>(), vec![1000]);
        assert!(out[1].keys.is_empty());
        assert_eq!(out[2].keys.iter().map(|k| k.t_ms).collect::<Vec<_>>(), vec![5000]);
        let (cx, _) = out[2].center.unwrap();
        assert!(close(cx, target_at(&a, 3000).unwrap().cx, 1e-5), "the later part starts where the camera was aiming");
        assert!(close(out[2].scale, 2.0, 1e-5));
    }
}
