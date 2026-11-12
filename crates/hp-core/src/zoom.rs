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

/// What automatic zoom looks at, with coordinates 0–1 of the frame.
pub struct AutoInput<'a> {
    pub clicks: &'a [(i64, f32, f32)],
    /// Wheel events (ms, x, y).
    pub wheels: &'a [(i64, f32, f32)],
    /// Moments of typing (ms).
    pub typing: &'a [i64],
    pub cursor: &'a dyn Fn(i64) -> Option<(f32, f32)>,
    /// Full-screen recording: clicks at the edge are ignored (taskbar, window buttons).
    pub edge_guard: bool,
}

/// Clicks within this distance of an edge (fraction of the frame) are ignored when guarded.
pub const EDGE_GUARD: f32 = 0.04;
/// At the Low level a lone click counts when typing or scrolling follows within this time.
pub const LOW_ACTIVITY_MS: i64 = 2000;
/// Typing or scrolling keeps the zoom this long after it.
pub const ACTIVITY_HOLD_MS: i64 = 1000;
/// ...but not longer than this after the last click.
pub const MAX_ACTIVITY_MS: i64 = 20_000;
/// The cursor out of the view this long ends the zoom early.
pub const LEAVE_MS: i64 = 1000;
/// Automatic zoom levels: at most `AUTO_MAX_SCALE`, at least `AUTO_MIN_SCALE`.
pub const AUTO_MAX_SCALE: f32 = 2.0;
pub const AUTO_MIN_SCALE: f32 = 1.25;
/// Margin kept around the clicks of a group, on each side.
pub const AUTO_MARGIN: f32 = 0.1;
/// Hold after the last click at the High level.
pub const HIGH_HOLD_MS: i64 = 1200;
/// An automatic pan between two groups takes this long, ending `LEAD_MS` before the next click.
pub const PAN_MS: i64 = 600;

/// Centre of the box around `clicks`: every click is as far inside the view as it can be.
fn box_center(clicks: &[(i64, f32, f32)]) -> (f32, f32) {
    let mid = |f: fn(&(i64, f32, f32)) -> f32| {
        let (lo, hi) = clicks.iter().map(f).fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
        (lo + hi) / 2.0
    };
    (mid(|c| c.1), mid(|c| c.2))
}

struct LevelParams {
    min_clicks: usize,
    hold_ms: i64,
    guard_edges: bool,
}

fn level_params(l: Level) -> LevelParams {
    match l {
        Level::Low => LevelParams { min_clicks: 2, hold_ms: HOLD_MS, guard_edges: true },
        Level::Normal => LevelParams { min_clicks: 1, hold_ms: HOLD_MS, guard_edges: true },
        Level::High => LevelParams { min_clicks: 1, hold_ms: HIGH_HOLD_MS, guard_edges: false },
    }
}

/// Zoom that shows every click of a group with a margin: a single click gets 2x.
pub fn auto_scale(clicks: &[(i64, f32, f32)]) -> f32 {
    let span = |f: fn(&(i64, f32, f32)) -> f32| {
        let (lo, hi) = clicks.iter().map(f).fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
        (hi - lo).max(0.0)
    };
    let m = span(|c| c.1).max(span(|c| c.2));
    (1.0 / (m + 2.0 * AUTO_MARGIN)).min(AUTO_MAX_SCALE).clamp(AUTO_MIN_SCALE, AUTO_MAX_SCALE)
}

/// One group of clicks close in time and place, with the zoom it gets.
struct Group {
    clicks: Vec<(i64, f32, f32)>,
    center: (f32, f32),
    scale: f32,
    start: i64,
    end: i64,
}

impl Group {
    fn first(&self) -> i64 {
        self.clicks[0].0
    }
    fn last(&self) -> i64 {
        self.clicks[self.clicks.len() - 1].0
    }
    /// Whether `p` is in what the camera shows for this group.
    fn shows(&self, p: (f32, f32)) -> bool {
        let half = 0.5 / self.scale;
        let c = (self.center.0.clamp(half, 1.0 - half), self.center.1.clamp(half, 1.0 - half));
        (p.0 - c.0).abs() <= half && (p.1 - c.1).abs() <= half
    }
}

/// Automatic zoom segments (spec "Zoom thông minh" 4): groups of clicks, zoom chosen from their
/// spread, kept while typing or scrolling, ended early when the cursor leaves, panning with
/// keyframes between groups far apart.
pub fn auto_segments(input: &AutoInput, level: Level, duration_ms: i64) -> Vec<ZoomSegment> {
    let p = level_params(level);
    let at_edge = |x: f32, y: f32| x <= EDGE_GUARD || y <= EDGE_GUARD || x >= 1.0 - EDGE_GUARD || y >= 1.0 - EDGE_GUARD;
    let mut clicks: Vec<(i64, f32, f32)> = input.clicks.iter().copied().filter(|c| !(input.edge_guard && p.guard_edges && at_edge(c.1, c.2))).collect();
    clicks.sort_by_key(|c| c.0);
    let mut typing = input.typing.to_vec();
    typing.sort_unstable();
    let mut wheels = input.wheels.to_vec();
    wheels.sort_by_key(|w| w.0);

    // Groups of clicks close in time and place.
    let mut raw: Vec<Vec<(i64, f32, f32)>> = Vec::new();
    for c in clicks {
        let joins = raw.last().and_then(|g| g.last()).is_some_and(|last| {
            let d = ((c.1 - last.1).powi(2) + (c.2 - last.2).powi(2)).sqrt();
            c.0 - last.0 < GROUP_GAP_MS && d < GROUP_DISTANCE
        });
        match raw.last_mut() {
            Some(g) if joins => g.push(c),
            _ => raw.push(vec![c]),
        }
    }
    let active_after = |from: i64, within: i64| {
        typing.iter().any(|&t| t > from && t <= from + within) || wheels.iter().any(|w| w.0 > from && w.0 <= from + within)
    };
    let mut groups: Vec<Group> = Vec::new();
    for g in raw {
        let last = g[g.len() - 1].0;
        if g.len() < p.min_clicks && !active_after(last, LOW_ACTIVITY_MS) {
            continue;
        }
        let center = box_center(&g);
        let mut group = Group { scale: auto_scale(&g), center, start: (g[0].0 - LEAD_MS).max(0), end: last + p.hold_ms, clicks: g };
        // Typing, or scrolling in view, keeps the zoom a moment longer; never past the cap.
        let cap = last + MAX_ACTIVITY_MS;
        let mut activity: Vec<i64> = typing.iter().copied().filter(|&t| t > last).collect();
        activity.extend(wheels.iter().filter(|w| w.0 > last && group.shows((w.1, w.2))).map(|w| w.0));
        activity.sort_unstable();
        for t in activity {
            if t > group.end || t > cap {
                break;
            }
            group.end = group.end.max((t + ACTIVITY_HOLD_MS).min(cap));
        }
        // The cursor out of view for a while ends the zoom early.
        // ...unless the user is typing (the mouse is often moved aside to type).
        let typed_lately = |t: i64| {
            let i = typing.partition_point(|&x| x <= t);
            i > 0 && typing[i - 1] > t - LEAVE_MS
        };
        let mut out_since: Option<i64> = None;
        let mut t = last;
        while t <= group.end {
            match (input.cursor)(t) {
                Some(c) if !group.shows(c) && !typed_lately(t) => {
                    let since = *out_since.get_or_insert(t);
                    if t - since >= LEAVE_MS {
                        group.end = group.end.min(since + LEAVE_MS);
                        break;
                    }
                }
                _ => out_since = None,
            }
            t += 50;
        }
        group.end = group.end.max(group.start + MIN_SEGMENT_MS).min(duration_ms);
        groups.push(group);
    }

    // Groups that nearly touch make one segment; groups far apart in it are keyframes.
    let mut spans: Vec<Vec<Group>> = Vec::new();
    for g in groups {
        match spans.last_mut() {
            Some(span) if g.start - span.iter().map(|s| s.end).max().unwrap_or(0) < MERGE_GAP_MS => span.push(g),
            _ => spans.push(vec![g]),
        }
    }
    spans
        .into_iter()
        .filter_map(|span| {
            let start = span[0].start;
            let end = span.iter().map(|g| g.end).max().unwrap_or(start).min(duration_ms);
            if end - start < MIN_SEGMENT_MS {
                return None;
            }
            let dist = |a: (f32, f32), b: (f32, f32)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
            let spread = span.windows(2).any(|w| dist(w[0].center, w[1].center) >= GROUP_DISTANCE);
            let seg = if spread {
                // Each later group: hold on the previous one, then pan to it in PAN_MS.
                let mut keys: Vec<ZoomKey> = Vec::new();
                for w in span.windows(2) {
                    let (prev, g) = (&w[0], &w[1]);
                    let pan_at = (g.first() - LEAD_MS).max(start + KEY_MIN_GAP_MS + 1);
                    let hold_at = prev.last().max(g.first() - LEAD_MS - PAN_MS);
                    let after = keys.last().map_or(start, |k| k.t_ms);
                    if hold_at - after > KEY_MIN_GAP_MS && pan_at - hold_at > KEY_MIN_GAP_MS {
                        keys.push(ZoomKey { t_ms: hold_at, cx: prev.center.0, cy: prev.center.1, scale: prev.scale });
                    }
                    keys.push(ZoomKey { t_ms: pan_at, cx: g.center.0, cy: g.center.1, scale: g.scale });
                }
                ZoomSegment { start_ms: start, end_ms: end, scale: span[0].scale, center: Some(span[0].center), keys, ..ZoomSegment::default() }
            } else {
                let all: Vec<(i64, f32, f32)> = span.iter().flat_map(|g| g.clicks.iter().copied()).collect();
                let center = box_center(&all);
                ZoomSegment { start_ms: start, end_ms: end, scale: auto_scale(&all), center: Some(center), ..ZoomSegment::default() }
            };
            Some(seg)
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

    fn auto(clicks: &[(i64, f32, f32)], duration_ms: i64) -> Vec<ZoomSegment> {
        auto_segments(&AutoInput { clicks, wheels: &[], typing: &[], cursor: NO_CURSOR, edge_guard: false }, Level::Normal, duration_ms)
    }

    fn input<'a>(clicks: &'a [(i64, f32, f32)], wheels: &'a [(i64, f32, f32)], typing: &'a [i64], cursor: &'a dyn Fn(i64) -> Option<(f32, f32)>, edge_guard: bool) -> AutoInput<'a> {
        AutoInput { clicks, wheels, typing, cursor, edge_guard }
    }

    fn close(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn clicks_close_in_time_and_place_make_one_segment() {
        let s = auto(&[(1000, 0.2, 0.4), (2000, 0.3, 0.4)], 60_000);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].start_ms, s[0].end_ms), (600, 3800));
        let (cx, cy) = s[0].center.unwrap();
        assert!(close(cx, 0.25, 1e-4) && close(cy, 0.4, 1e-4));
        assert_eq!(s[0].scale, auto_scale(&[(1000, 0.2, 0.4), (2000, 0.3, 0.4)]), "zoom chosen from the spread");
        assert!(!s[0].manual);
    }

    #[test]
    fn far_apart_clicks_split() {
        assert_eq!(auto(&[(1000, 0.2, 0.2), (9000, 0.2, 0.2)], 60_000).len(), 2, "far in time");
        assert_eq!(auto(&[(1000, 0.1, 0.1), (1500, 0.9, 0.9)], 60_000).len(), 1, "far in place but merged: gap < 800 ms");
        let s = auto(&[(1000, 0.1, 0.1), (4000, 0.9, 0.9)], 60_000);
        assert_eq!(s.len(), 2, "[600, 2800) and [3600, 5800): a gap of exactly 800 ms is not merged");
        let s = auto(&[(1000, 0.1, 0.1), (6000, 0.9, 0.9)], 60_000);
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn merged_clicks_far_apart_pan_between_them() {
        // Two clicks 500 ms apart at opposite corners: one span; no single centre shows both, so
        // the camera pans from the first to the second instead of zooming on the empty middle.
        let s = auto(&[(1000, 0.1, 0.1), (1500, 0.9, 0.9)], 60_000);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].center, Some((0.1, 0.1)));
        assert_eq!(s[0].keys.iter().map(|k| (k.t_ms, k.cx)).collect::<Vec<_>>(), vec![(1000, 0.1), (1100, 0.9)]);
        let near = auto(&[(1000, 0.2, 0.2), (1500, 0.25, 0.2)], 60_000);
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
        let s = auto(&[(1000, 0.1, 0.1), (3600, 0.8, 0.8)], 60_000);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].start_ms, s[0].end_ms), (600, 5400));
    }

    #[test]
    fn short_segments_are_dropped() {
        assert!(auto(&[(100, 0.5, 0.5)], 900).is_empty(), "clamped to [0, 900): too short");
        assert_eq!(auto(&[(100, 0.5, 0.5)], 60_000).len(), 1);
    }

    #[test]
    fn segments_are_clamped_to_the_video() {
        let s = auto(&[(100, 0.5, 0.5), (8500, 0.5, 0.5)], 10_000);
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

    #[test]
    fn auto_scale_follows_the_spread() {
        assert_eq!(auto(&[(1000, 0.5, 0.5)], 60_000)[0].scale, 2.0, "a single click");
        let row: Vec<(i64, f32, f32)> = (0..6).map(|i| (1000 + i * 100, 0.2 + 0.12 * i as f32, 0.5)).collect();
        let s = auto(&row, 60_000);
        assert_eq!(s.len(), 1);
        assert!(close(s[0].scale, 1.25, 1e-4), "{}", s[0].scale);
    }

    #[test]
    fn far_groups_pan_with_keys() {
        let s = auto(&[(1000, 0.2, 0.2), (2500, 0.8, 0.8)], 60_000);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].center, Some((0.2, 0.2)));
        assert_eq!(s[0].scale, 2.0);
        assert_eq!(s[0].keys, vec![ZoomKey { t_ms: 1500, cx: 0.2, cy: 0.2, scale: 2.0 }, ZoomKey { t_ms: 2100, cx: 0.8, cy: 0.8, scale: 2.0 }], "holds on A, then pans to B");
    }

    #[test]
    fn edge_clicks_are_ignored_when_guarded() {
        let clicks = [(1000, 0.5, 0.99)];
        assert!(auto_segments(&input(&clicks, &[], &[], NO_CURSOR, true), Level::Normal, 60_000).is_empty());
        assert_eq!(auto_segments(&input(&clicks, &[], &[], NO_CURSOR, false), Level::Normal, 60_000).len(), 1, "not a full-screen recording");
        assert_eq!(auto_segments(&input(&clicks, &[], &[], NO_CURSOR, true), Level::High, 60_000).len(), 1);
    }

    #[test]
    fn low_level_needs_more_than_a_lone_click() {
        let lone = [(1000, 0.5, 0.5)];
        assert!(auto_segments(&input(&lone, &[], &[], NO_CURSOR, false), Level::Low, 60_000).is_empty());
        assert_eq!(auto_segments(&input(&lone, &[], &[1800], NO_CURSOR, false), Level::Low, 60_000).len(), 1, "typing right after");
        let two = [(1000, 0.5, 0.5), (1500, 0.52, 0.5)];
        assert_eq!(auto_segments(&input(&two, &[], &[], NO_CURSOR, false), Level::Low, 60_000).len(), 1);
    }

    #[test]
    fn typing_keeps_the_zoom() {
        let clicks = [(1000, 0.5, 0.5)];
        let typing: Vec<i64> = (3..=12).map(|i| i * 500).collect();
        let s = auto_segments(&input(&clicks, &[], &typing, NO_CURSOR, false), Level::Normal, 60_000);
        assert_eq!(s[0].end_ms, 7000);
        let long: Vec<i64> = (3..=60).map(|i| i * 500).collect();
        let s = auto_segments(&input(&clicks, &[], &long, NO_CURSOR, false), Level::Normal, 60_000);
        assert_eq!(s[0].end_ms, 21_000, "at most 20 s after the last click");
    }

    #[test]
    fn scrolling_inside_the_view_keeps_the_zoom() {
        let clicks = [(1000, 0.5, 0.5)];
        let inside = auto_segments(&input(&clicks, &[(2500, 0.5, 0.5)], &[], NO_CURSOR, false), Level::Normal, 60_000);
        assert_eq!(inside[0].end_ms, 3500);
        let outside = auto_segments(&input(&clicks, &[(2500, 0.9, 0.9)], &[], NO_CURSOR, false), Level::Normal, 60_000);
        assert_eq!(outside[0].end_ms, 2800);
    }

    #[test]
    fn leaving_the_view_ends_early() {
        let clicks = [(1000, 0.5, 0.5)];
        let cursor = |t: i64| Some(if t < 1300 { (0.5, 0.5) } else { (0.95, 0.95) });
        let s = auto_segments(&input(&clicks, &[], &[], &cursor, false), Level::Normal, 60_000);
        assert_eq!(s[0].end_ms, 2300);
        let stays = |_t: i64| Some((0.5, 0.5));
        assert_eq!(auto_segments(&input(&clicks, &[], &[], &stays, false), Level::Normal, 60_000)[0].end_ms, 2800);
    }

    #[test]
    fn high_level_holds_shorter() {
        let clicks = [(1000, 0.5, 0.5)];
        assert_eq!(auto_segments(&input(&clicks, &[], &[], NO_CURSOR, false), Level::High, 60_000)[0].end_ms, 2200);
        assert_eq!(auto_segments(&input(&clicks, &[], &[], NO_CURSOR, false), Level::Normal, 60_000)[0].end_ms, 2800);
    }

    #[test]
    fn the_pan_waits_for_the_first_group() {
        let clicks = [(1000, 0.2, 0.2), (2000, 0.21, 0.2), (3000, 0.2, 0.21), (4500, 0.8, 0.8)];
        let s = auto(&clicks, 60_000);
        assert_eq!(s.len(), 1);
        let at3 = target_at(&s[0], 3000).unwrap();
        assert!(close(at3.cx, 0.2, 0.02), "still on A at its last click: {at3:?}");
        let at_b = target_at(&s[0], 4100).unwrap();
        assert!(close(at_b.cx, 0.8, 1e-4), "on B before its click: {at_b:?}");
    }

    #[test]
    fn every_click_of_a_group_is_in_view() {
        let mut clicks: Vec<(i64, f32, f32)> = (0..8).map(|i| (1000 + i * 100, 0.5, 0.5)).collect();
        clicks.extend([(1800, 0.6, 0.5), (1900, 0.7, 0.5), (2000, 0.8, 0.5), (2100, 0.9, 0.5)]);
        let s = auto(&clicks, 60_000);
        assert_eq!(s.len(), 1);
        let (cx, _) = s[0].center.unwrap();
        let half = 0.5 / s[0].scale;
        assert!(clicks.iter().all(|c| (c.1 - cx).abs() <= half), "centre {cx}, half {half}");
    }

    #[test]
    fn typing_keeps_the_zoom_even_with_the_mouse_away() {
        let clicks = [(1000, 0.5, 0.5)];
        let typing: Vec<i64> = (4..=13).map(|i| i * 300).collect();
        let away = |t: i64| Some(if t < 1300 { (0.5, 0.5) } else { (0.95, 0.95) });
        let s = auto_segments(&input(&clicks, &[], &typing, &away, false), Level::Normal, 60_000);
        assert_eq!(s[0].end_ms, 4900, "typing until 3.9 s holds to 4.9 s");
    }
}
