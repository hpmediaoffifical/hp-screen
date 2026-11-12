//! The export plan: for each output frame, which source time to show and what to draw.
use crate::cut::{Cuts, MIN_OUTPUT_MS};
use crate::edit::EditDoc;
use crate::edit::EffectToggles;
use crate::effects::{Draw, Effects, draws_at};
use crate::layout::{Layout, layout, source_view};
use crate::subtitle::{BURN_BOTTOM, Cue, burned_height, cue_at};
use crate::zoom::{Camera, Spot, camera_at, spot_at, zoom_track};

/// Camera samples per second for zoom.
const CAMERA_STEP_MS: i64 = 1000 / 60;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Source {
    pub width: u32,
    pub height: u32,
    pub duration_ms: i64,
    pub fps: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FramePlan {
    pub out_ms: i64,
    pub src_ms: i64,
    pub draws: Vec<Draw>,
    /// Where the zoom camera looks in the source frame.
    pub camera: Camera,
    /// The part of the source shown (zoom and fill crop), in source pixels.
    pub view: (f32, f32, f32, f32),
    /// Views to render and average for motion blur while the camera moves; empty when still.
    pub blur_views: Vec<(f32, f32, f32, f32)>,
}

/// Motion blur: views taken across half a frame (a 180° shutter)...
const BLUR_SAMPLES: usize = 5;
/// ...used when a corner of the view moves more than this fraction of its size in that time.
const BLUR_MIN_MOVE: f32 = 0.0025;

#[derive(Clone, Debug)]
pub struct Plan {
    src: Source,
    cuts: Cuts,
    effects: Option<Effects>,
    toggles: EffectToggles,
    /// Zoom camera and spotlight over source time; `None` when zoom is off.
    track: Option<Vec<(i64, Camera, Spot)>>,
    /// The camera track for callers that want only the camera.
    cameras: Option<Vec<(i64, Camera)>>,
    motion_blur: bool,
    layout: Layout,
    /// Cues drawn into the video; `None` when not burned in.
    burned: Option<Vec<Cue>>,
}

impl Plan {
    /// `effects` is `None` for a video without an input log.
    pub fn new(src: Source, doc: &EditDoc, effects: Option<Effects>) -> Plan {
        let track = doc.zoom.enabled.then(|| {
            let (w, h) = (src.width.max(1) as f32, src.height.max(1) as f32);
            let cursor = |t: i64| effects.as_ref().and_then(|e| e.cursor_at(t)).map(|(x, y)| (x / w, y / h));
            zoom_track(&doc.zoom, &cursor, src.duration_ms, CAMERA_STEP_MS)
        });
        let cameras = track.as_ref().map(|t| t.iter().map(|s| (s.0, s.1)).collect());
        let layout = layout(src.width, src.height, &doc.frame);
        let burned = doc.subtitles.burn.then(|| doc.subtitles.cues.clone());
        Plan { src, cuts: doc.cuts(src.duration_ms), effects, toggles: doc.effects, track, cameras, motion_blur: doc.zoom.motion_blur, layout, burned }
    }

    /// The same edit as a GIF: `fps` frames per second, `width` pixels wide.
    pub fn for_gif(&self, fps: u32, width: u32) -> Plan {
        let mut p = self.clone();
        p.src.fps = fps.max(1);
        p.layout = self.layout.scaled(width);
        p
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// The zoom camera samples (for the preview).
    pub fn camera_track(&self) -> Option<&[(i64, Camera)]> {
        self.cameras.as_deref()
    }

    fn view_at(&self, src_ms: i64) -> (f32, f32, f32, f32) {
        let camera = self.cameras.as_deref().map_or(Camera::WHOLE, |t| camera_at(t, src_ms));
        source_view(camera, self.src.width, self.src.height, self.layout.fill_aspect)
    }

    /// Views across half a frame around output time `out_ms` when the camera moves; else empty.
    fn blur_views(&self, out_ms: i64, src_ms: i64) -> Vec<(f32, f32, f32, f32)> {
        if !self.motion_blur || self.cameras.is_none() {
            return Vec::new();
        }
        let frame_ms = 1000.0 / self.fps() as f64;
        // Samples from the other side of a cut would blend two different shots: none then.
        let views: Vec<_> = (0..BLUR_SAMPLES)
            .filter_map(|i| {
                let off = (i as f64 / (BLUR_SAMPLES - 1) as f64 - 0.5) * frame_ms / 2.0;
                self.cuts
                    .to_source((out_ms as f64 + off).round() as i64)
                    .filter(|s| ((s - src_ms) as f64).abs() <= frame_ms)
                    .map(|s| self.view_at(s))
            })
            .collect();
        let (first, last) = (views.first(), views.last());
        let moved = match (first, last) {
            (Some(a), Some(b)) => {
                let size = a.2.max(a.3).max(1.0);
                [(a.0 - b.0).abs(), (a.1 - b.1).abs(), ((a.0 + a.2) - (b.0 + b.2)).abs(), ((a.1 + a.3) - (b.1 + b.3)).abs()].into_iter().fold(0.0f32, f32::max) / size
            }
            _ => 0.0,
        };
        if views.len() == BLUR_SAMPLES && moved > BLUR_MIN_MOVE { views } else { Vec::new() }
    }

    pub fn frames(&self) -> u64 {
        (self.cuts.output_ms().max(0) as u64) * self.fps() as u64 / 1000
    }

    pub fn fps(&self) -> u32 {
        self.src.fps.max(1)
    }

    pub fn out_size(&self) -> (u32, u32) {
        (self.layout.out_w, self.layout.out_h)
    }

    pub fn frame(&self, n: u64) -> Option<FramePlan> {
        if n >= self.frames() {
            return None;
        }
        let out_ms = (n * 1000 / self.fps() as u64) as i64;
        let src_ms = self.cuts.to_source(out_ms)?;
        let mut draws = match &self.effects {
            Some(e) => draws_at(e, src_ms, &self.toggles, self.src.height),
            None => Vec::new(),
        };
        if let Some(cue) = self.burned.as_deref().and_then(|c| cue_at(c, src_ms)) {
            // Shortcut chips move up above the subtitle.
            let above = BURN_BOTTOM + burned_height(cue.text.lines().count()) + 0.02;
            for d in draws.iter_mut() {
                if let Draw::Chip { bottom, .. } = d {
                    *bottom = above;
                }
            }
            draws.push(Draw::Subtitle { text: cue.text.clone() });
        }
        let camera = self.cameras.as_deref().map_or(Camera::WHOLE, |t| camera_at(t, src_ms));
        let view = source_view(camera, self.src.width, self.src.height, self.layout.fill_aspect);
        // A spotlight darkens the video around its box, under the click marks.
        if let Some(track) = self.track.as_deref() {
            let spot = spot_at(track, src_ms);
            if spot.alpha > 0.01 {
                // The lit box is the frame the user placed, crop included.
                let (x, y, w, h) = source_view(Camera { cx: spot.cx, cy: spot.cy, scale: spot.scale }, self.src.width, self.src.height, self.layout.fill_aspect);
                draws.insert(0, Draw::Spot { x, y, w, h, alpha: spot.alpha });
            }
        }
        let blur_views = self.blur_views(out_ms, src_ms);
        Some(FramePlan { out_ms, src_ms, draws, camera, view, blur_views })
    }

    pub fn cuts(&self) -> &Cuts {
        &self.cuts
    }

    pub fn can_export(&self) -> bool {
        self.cuts.output_ms() >= MIN_OUTPUT_MS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cut::Span;
    use crate::edit::Trim;

    fn src(width: u32, height: u32) -> Source {
        Source { width, height, duration_ms: 10_000, fps: 30 }
    }

    fn doc(start_ms: i64, end_ms: i64, cuts: Vec<Span>) -> EditDoc {
        EditDoc { trim: Trim { start_ms, end_ms: Some(end_ms) }, cuts, ..EditDoc::default() }
    }

    fn sample() -> Plan {
        Plan::new(src(640, 480), &doc(1000, 9000, vec![Span { start_ms: 3000, end_ms: 5000 }]), None)
    }

    #[test]
    fn frame_count_follows_output_length() {
        assert_eq!(sample().frames(), 180);
    }

    #[test]
    fn frame_times_skip_the_removed_span() {
        let p = sample();
        assert_eq!(p.frame(0).unwrap().src_ms, 1000);
        assert_eq!(p.frame(59).unwrap().src_ms, 2966);
        assert_eq!(p.frame(60).unwrap().src_ms, 5000);
        assert_eq!(p.frame(60).unwrap().out_ms, 2000);
        assert_eq!(p.frame(180), None);
    }

    #[test]
    fn too_short_cannot_export() {
        assert!(!Plan::new(src(640, 480), &doc(0, 400, vec![]), None).can_export());
        assert!(Plan::new(src(640, 480), &doc(0, 500, vec![]), None).can_export());
    }

    #[test]
    fn plan_draws_effects_at_source_time() {
        use crate::input_log::{Button, Event, ParsedLog};
        let log = ParsedLog { header: None, events: vec![Event::Down { t: 6000, b: Button::Left, x: 10, y: 10 }] };
        let fx = crate::effects::build_effects(&log, 640, 480);
        let p = Plan::new(src(640, 480), &doc(1000, 9000, vec![Span { start_ms: 3000, end_ms: 5000 }]), Some(fx));
        // Output 3000 ms shows source 6000 ms, the click.
        let n = 3000 * 30 / 1000;
        let f = p.frame(n).unwrap();
        assert_eq!(f.src_ms, 6000);
        assert!(matches!(f.draws.as_slice(), [Draw::Ring { .. }]), "{:?}", f.draws);
        assert!(p.frame(0).unwrap().draws.is_empty());
    }

    #[test]
    fn plan_frames_carry_the_camera() {
        use crate::zoom::{ZoomDoc, ZoomSegment};
        let zoom = ZoomDoc { enabled: true, segments: vec![ZoomSegment { start_ms: 0, end_ms: 10_000, scale: 2.0, center: Some((0.25, 0.25)), manual: true, ..ZoomSegment::default() }], ..ZoomDoc::default() };
        let p = Plan::new(src(640, 480), &EditDoc { zoom, ..EditDoc::default() }, None);
        assert_eq!(p.frame(0).unwrap().camera.scale, 1.0, "eases in from the whole frame");
        let late = p.frame(250).unwrap().camera;
        assert!((late.scale - 2.0).abs() < 0.02 && (late.cx - 0.25).abs() < 0.01, "{late:?}");
        let off = Plan::new(src(640, 480), &EditDoc::default(), None);
        assert_eq!(off.frame(100).unwrap().camera, crate::zoom::Camera::WHOLE);
    }

    #[test]
    fn the_frame_layout_sets_the_size_and_view() {
        use crate::layout::{Aspect, Fit, FrameDoc};
        let doc = EditDoc { frame: FrameDoc { aspect: Aspect::Tall, fit: Fit::Fill, ..FrameDoc::default() }, ..EditDoc::default() };
        let p = Plan::new(src(1920, 1080), &doc, None);
        assert_eq!(p.out_size(), (608, 1080));
        let v = p.frame(0).unwrap().view;
        assert!((v.2 / v.3 - 608.0 / 1080.0).abs() < 1e-3, "{v:?}");
        let fit = Plan::new(src(1920, 1080), &EditDoc::default(), None);
        assert_eq!(fit.frame(0).unwrap().view, (0.0, 0.0, 1920.0, 1080.0));
    }

    #[test]
    fn a_gif_plan_has_its_own_rate_and_width() {
        let p = sample().for_gif(10, 320);
        assert_eq!(p.fps(), 10);
        assert_eq!(p.frames(), 60, "6 s kept at 10 fps");
        assert_eq!(p.out_size(), (320, 240));
    }

    #[test]
    fn burned_subtitle_is_in_the_frame_plan() {
        use crate::subtitle::{Cue, SubtitleDoc};
        let cues = vec![Cue { start_ms: 1000, end_ms: 2000, text: "Xin chào".into() }];
        let on = EditDoc { subtitles: SubtitleDoc { cues: cues.clone(), burn: true }, ..EditDoc::default() };
        let p = Plan::new(src(640, 480), &on, None);
        assert!(p.frame(45).unwrap().draws.contains(&Draw::Subtitle { text: "Xin chào".into() }), "1.5 s");
        assert!(!p.frame(0).unwrap().draws.iter().any(|d| matches!(d, Draw::Subtitle { .. })));
        let off = EditDoc { subtitles: SubtitleDoc { cues, burn: false }, ..EditDoc::default() };
        assert!(Plan::new(src(640, 480), &off, None).frame(45).unwrap().draws.is_empty());
    }

    #[test]
    fn chips_move_above_a_burned_subtitle() {
        use crate::input_log::{Event, ParsedLog};
        use crate::subtitle::{Cue, SubtitleDoc};
        let log = ParsedLog { header: None, events: vec![Event::Key { t: 1000, k: "Ctrl+S".into() }] };
        let fx = crate::effects::build_effects(&log, 640, 480);
        let doc = EditDoc { subtitles: SubtitleDoc { cues: vec![Cue { start_ms: 1000, end_ms: 2000, text: "Lưu".into() }], burn: true }, ..EditDoc::default() };
        let p = Plan::new(src(640, 480), &doc, Some(fx));
        let chip = p.frame(36).unwrap().draws.into_iter().find_map(|d| if let Draw::Chip { bottom, .. } = d { Some(bottom) } else { None });
        let top_of_subtitle = crate::subtitle::BURN_BOTTOM + crate::subtitle::burned_height(1);
        assert!(chip.unwrap() > top_of_subtitle, "the chip sits above the subtitle box: {chip:?}");
    }

    #[test]
    fn odd_sources_give_even_output() {
        assert_eq!(Plan::new(src(641, 481), &EditDoc::default(), None).out_size(), (640, 480));
    }

    fn zooming(motion_blur: bool, style: crate::zoom::Style) -> EditDoc {
        use crate::zoom::{ZoomDoc, ZoomSegment};
        let seg = ZoomSegment { start_ms: 1000, end_ms: 3000, scale: 2.0, center: Some((0.25, 0.25)), manual: true, style, ..ZoomSegment::default() };
        EditDoc { zoom: ZoomDoc { enabled: true, segments: vec![seg], motion_blur, ..ZoomDoc::default() }, ..EditDoc::default() }
    }

    #[test]
    fn a_spotlight_is_drawn_over_the_video() {
        let p = Plan::new(src(640, 480), &zooming(true, crate::zoom::Style::Spotlight), None);
        let f = p.frame(75).unwrap(); // 2.5 s
        let spot = f.draws.iter().find_map(|d| if let Draw::Spot { x, y, w, h, alpha } = d { Some((*x, *y, *w, *h, *alpha)) } else { None });
        let (x, y, w, h, alpha) = spot.expect("a spotlight at 2.5 s");
        assert!(x.abs() < 3.0 && y.abs() < 3.0 && (w - 320.0).abs() < 6.0 && (h - 240.0).abs() < 6.0 && alpha > 0.9, "{spot:?}");
        assert!((f.camera.scale - 1.0).abs() < 1e-3);
        assert!(!p.frame(0).unwrap().draws.iter().any(|d| matches!(d, Draw::Spot { .. })));
    }

    #[test]
    fn motion_blur_only_while_the_camera_moves() {
        let p = Plan::new(src(640, 480), &zooming(true, crate::zoom::Style::Zoom), None);
        assert_eq!(p.frame(34).unwrap().blur_views.len(), 5, "zooming in at 1.13 s");
        assert!(p.frame(0).unwrap().blur_views.is_empty(), "still camera");
        let off = Plan::new(src(640, 480), &zooming(false, crate::zoom::Style::Zoom), None);
        assert!(off.frame(34).unwrap().blur_views.is_empty(), "turned off");
    }

    #[test]
    fn with_fill_the_lit_box_has_the_shape_of_the_frame() {
        use crate::layout::{Aspect, Fit, FrameDoc};
        let mut doc = zooming(true, crate::zoom::Style::Spotlight);
        doc.frame = FrameDoc { aspect: Aspect::Tall, fit: Fit::Fill, ..FrameDoc::default() };
        let p = Plan::new(src(640, 360), &doc, None);
        let f = p.frame(75).unwrap();
        let (w, h) = f.draws.iter().find_map(|d| if let Draw::Spot { w, h, .. } = d { Some((*w, *h)) } else { None }).unwrap();
        assert!((w / h - 9.0 / 16.0).abs() < 0.01, "a 9:16 box like the dragged frame: {w}x{h}");
    }

    #[test]
    fn motion_blur_never_mixes_both_sides_of_a_cut() {
        let mut doc = zooming(true, crate::zoom::Style::Zoom);
        // Zoomed in until 3 s; cut 3000–4000: the frame at the join must not blend 2x and 1x views.
        doc.zoom.segments[0].end_ms = 3000;
        doc.zoom.segments[0].speed = crate::zoom::Speed::Instant;
        doc.cuts = vec![Span { start_ms: 3000, end_ms: 4000 }];
        let p = Plan::new(src(640, 480), &doc, None);
        let join = p.frame(90).unwrap(); // output 3000 ms = source 4000 ms
        assert!(join.blur_views.is_empty(), "{:?}", join.blur_views);
    }

    fn cursor_effects(drawn: bool) -> crate::effects::Effects {
        use crate::geometry::Rect;
        use crate::input_log::{Event, Header, ParsedLog};
        let header = Header { v: 1, mode: "monitor".into(), region: Rect { x: 0, y: 0, w: 640, h: 480 }, width: 640, height: 480, fps: 30, cursor: drawn };
        let events = vec![
            Event::Shape { id: 1, hx: 0, hy: 0, png: String::new() },
            Event::Cur { t: 0, id: 1 },
            Event::Move { t: 0, x: 100, y: 100 },
            Event::Move { t: 500, x: 200, y: 150 },
        ];
        crate::effects::build_effects(&ParsedLog { header: Some(header), events }, 640, 480)
    }

    #[test]
    fn the_cursor_is_drawn_only_for_recordings_without_the_windows_cursor() {
        let doc = EditDoc::default();
        let p = Plan::new(src(640, 480), &doc, Some(cursor_effects(true)));
        let cur = p.frame(30).unwrap().draws.into_iter().find_map(|d| if let Draw::Cursor { x, y, scale, alpha, shape } = d { Some((x, y, scale, alpha, shape)) } else { None });
        let (x, y, scale, alpha, shape) = cur.expect("a drawn cursor at 1 s");
        assert!((x - 200.0).abs() < 1.0 && (y - 150.0).abs() < 1.0 && alpha == 1.0 && shape == 1, "{cur:?}");
        assert!((scale - 1.5).abs() < 1e-6, "default size 1.5x");
        assert!(!Plan::new(src(640, 480), &doc, Some(cursor_effects(false))).frame(30).unwrap().draws.iter().any(|d| matches!(d, Draw::Cursor { .. })), "the video has its own cursor");
        let mut off = EditDoc::default();
        off.effects.cursor = false;
        assert!(!Plan::new(src(640, 480), &off, Some(cursor_effects(true))).frame(30).unwrap().draws.iter().any(|d| matches!(d, Draw::Cursor { .. })), "turned off");
        let d = cursor_effects(true).smooth;
        assert!(!d.is_empty() && cursor_effects(false).smooth.is_empty(), "the path is made only when needed");
    }

    #[test]
    fn old_edit_files_draw_the_cursor_at_one_and_a_half() {
        let doc = crate::edit::load_edit(r#"{"v":1,"effects":{"clicks":true,"keys":false}}"#).unwrap();
        assert!(doc.effects.cursor);
        assert!((doc.effects.cursor_size - 1.5).abs() < 1e-6);
    }
}
