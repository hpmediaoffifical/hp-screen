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
}

impl Default for ZoomSegment {
    fn default() -> Self {
        ZoomSegment { start_ms: 0, end_ms: 0, scale: DEFAULT_SCALE, center: None, manual: false }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ZoomDoc {
    pub enabled: bool,
    pub segments: Vec<ZoomSegment>,
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
            manual: false,
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

/// Valid segments only, in order: no empty ones, scale and centre within range, no overlaps
/// (the later segment wins the overlapping part).
pub fn normalized(segments: &[ZoomSegment], duration_ms: i64) -> Vec<ZoomSegment> {
    let mut v: Vec<ZoomSegment> = segments
        .iter()
        .map(|s| ZoomSegment {
            start_ms: s.start_ms.max(0),
            end_ms: s.end_ms.min(duration_ms),
            scale: if s.scale.is_finite() { s.scale.clamp(MIN_SCALE, MAX_SCALE) } else { DEFAULT_SCALE },
            center: s.center.map(|(x, y)| (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0))),
            manual: s.manual,
        })
        .filter(|s| s.end_ms > s.start_ms)
        .collect();
    v.sort_by_key(|s| s.start_ms);
    // Where segments overlap, the one that starts later wins; a segment inside another splits it.
    let mut cuts: Vec<i64> = v.iter().flat_map(|s| [s.start_ms, s.end_ms]).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut out: Vec<(usize, ZoomSegment)> = Vec::new();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let winner = v.iter().enumerate().filter(|(_, s)| s.start_ms <= a && s.end_ms >= b).max_by_key(|(i, s)| (s.start_ms, *i));
        let Some((i, s)) = winner else { continue };
        match out.last_mut() {
            Some((last_i, last)) if *last_i == i && last.end_ms == a => last.end_ms = b,
            _ => out.push((i, ZoomSegment { start_ms: a, end_ms: b, ..s.clone() })),
        }
    }
    out.into_iter().map(|(_, s)| s).collect()
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
                let aim = match s.center {
                    Some(c) => c,
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
                (s.scale, aim)
            }
        };
        if t > 0 {
            CAMERA.step(&mut scale, target_scale, 0.001);
            CAMERA.step(&mut cx, target.0, 0.001);
            CAMERA.step(&mut cy, target.1, 0.001);
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
        ZoomSegment { start_ms, end_ms, scale, center, manual: true }
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
        ZoomDoc { enabled: true, segments }
    }

    #[test]
    fn disabled_zoom_is_identity() {
        let z = ZoomDoc { enabled: false, segments: vec![manual(0, 5000, 3.0, None)] };
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
}
