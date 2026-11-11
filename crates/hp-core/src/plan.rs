//! The export plan: for each output frame, which source time to show and what to draw.
use crate::cut::{Cuts, MIN_OUTPUT_MS};
use crate::edit::EditDoc;
use crate::edit::EffectToggles;
use crate::effects::{Draw, Effects, draws_at};
use crate::geometry::make_even;

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
}

#[derive(Clone, Debug)]
pub struct Plan {
    src: Source,
    cuts: Cuts,
    effects: Option<Effects>,
    toggles: EffectToggles,
}

impl Plan {
    /// `effects` is `None` for a video without an input log.
    pub fn new(src: Source, doc: &EditDoc, effects: Option<Effects>) -> Plan {
        Plan { src, cuts: doc.cuts(src.duration_ms), effects, toggles: doc.effects }
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
        Some(FramePlan { out_ms, src_ms, draws })
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
    fn odd_sources_give_even_output() {
        assert_eq!(Plan::new(src(641, 481), &EditDoc::default(), None).out_size(), (640, 480));
    }
}
