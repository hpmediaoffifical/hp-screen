//! The export plan: for each output frame, which source time to show and what to draw.
use crate::cut::{Cuts, MIN_OUTPUT_MS};
use crate::edit::EditDoc;
use crate::edit::EffectToggles;
use crate::effects::{Draw, Effects, draws_at};
use crate::geometry::make_even;
use crate::zoom::{Camera, camera_at, camera_track};

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
}

#[derive(Clone, Debug)]
pub struct Plan {
    src: Source,
    cuts: Cuts,
    effects: Option<Effects>,
    toggles: EffectToggles,
    /// Zoom camera over source time; `None` when zoom is off.
    track: Option<Vec<(i64, Camera)>>,
}

impl Plan {
    /// `effects` is `None` for a video without an input log.
    pub fn new(src: Source, doc: &EditDoc, effects: Option<Effects>) -> Plan {
        let track = doc.zoom.enabled.then(|| {
            let (w, h) = (src.width.max(1) as f32, src.height.max(1) as f32);
            let cursor = |t: i64| effects.as_ref().and_then(|e| e.cursor_at(t)).map(|(x, y)| (x / w, y / h));
            camera_track(&doc.zoom, &cursor, src.duration_ms, CAMERA_STEP_MS)
        });
        Plan { src, cuts: doc.cuts(src.duration_ms), effects, toggles: doc.effects, track }
    }

    /// The zoom camera samples (for the preview).
    pub fn camera_track(&self) -> Option<&[(i64, Camera)]> {
        self.track.as_deref()
    }

    pub fn frames(&self) -> u64 {
        (self.cuts.output_ms().max(0) as u64) * self.fps() as u64 / 1000
    }

    pub fn fps(&self) -> u32 {
        self.src.fps.max(1)
    }

    pub fn out_size(&self) -> (u32, u32) {
        make_even(self.src.width, self.src.height)
    }

    pub fn frame(&self, n: u64) -> Option<FramePlan> {
        if n >= self.frames() {
            return None;
        }
        let out_ms = (n * 1000 / self.fps() as u64) as i64;
        let src_ms = self.cuts.to_source(out_ms)?;
        let draws = match &self.effects {
            Some(e) => draws_at(e, src_ms, &self.toggles, self.src.height),
            None => Vec::new(),
        };
        let camera = self.track.as_deref().map_or(Camera::WHOLE, |t| camera_at(t, src_ms));
        Some(FramePlan { out_ms, src_ms, draws, camera })
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
        let zoom = ZoomDoc { enabled: true, segments: vec![ZoomSegment { start_ms: 0, end_ms: 10_000, scale: 2.0, center: Some((0.25, 0.25)), manual: true }] };
        let p = Plan::new(src(640, 480), &EditDoc { zoom, ..EditDoc::default() }, None);
        assert_eq!(p.frame(0).unwrap().camera.scale, 1.0, "eases in from the whole frame");
        let late = p.frame(250).unwrap().camera;
        assert!((late.scale - 2.0).abs() < 0.02 && (late.cx - 0.25).abs() < 0.01, "{late:?}");
        let off = Plan::new(src(640, 480), &EditDoc::default(), None);
        assert_eq!(off.frame(100).unwrap().camera, crate::zoom::Camera::WHOLE);
    }

    #[test]
    fn odd_sources_give_even_output() {
        assert_eq!(Plan::new(src(641, 481), &EditDoc::default(), None).out_size(), (640, 480));
    }
}
