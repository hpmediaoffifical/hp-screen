//! Trimming and removed spans: which parts of the source video are kept, and how output
//! time maps to source time and back. All times are milliseconds.
use serde::{Deserialize, Serialize};

/// Length of the fade-out / fade-in on each side of a join between two kept spans.
pub const JOIN_FADE_MS: f64 = 10.0;
/// Shortest output that may be exported.
pub const MIN_OUTPUT_MS: i64 = 500;

/// A half-open time span `[start_ms, end_ms)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start_ms: i64,
    pub end_ms: i64,
}

/// The kept parts of a video, in source order.
#[derive(Clone, Debug, PartialEq)]
pub struct Cuts {
    keep: Vec<Span>,
}

impl Cuts {
    pub fn new(duration_ms: i64, trim_start_ms: i64, trim_end_ms: Option<i64>, removed: &[Span]) -> Cuts {
        let lo = trim_start_ms.max(0);
        let hi = trim_end_ms.unwrap_or(duration_ms).min(duration_ms);
        if lo >= hi {
            return Cuts { keep: Vec::new() };
        }
        let mut gone: Vec<Span> = removed
            .iter()
            .map(|r| Span { start_ms: r.start_ms.max(lo), end_ms: r.end_ms.min(hi) })
            .filter(|r| r.start_ms < r.end_ms)
            .collect();
        gone.sort_by_key(|r| r.start_ms);
        let mut keep = Vec::new();
        let mut at = lo;
        for r in gone {
            if r.start_ms > at {
                keep.push(Span { start_ms: at, end_ms: r.start_ms });
            }
            at = at.max(r.end_ms);
        }
        if at < hi {
            keep.push(Span { start_ms: at, end_ms: hi });
        }
        Cuts { keep }
    }

    pub fn keep(&self) -> &[Span] {
        &self.keep
    }

    pub fn output_ms(&self) -> i64 {
        self.keep.iter().map(|k| k.end_ms - k.start_ms).sum()
    }

    /// Source time shown at output time `out_ms`, or `None` past the end.
    pub fn to_source(&self, out_ms: i64) -> Option<i64> {
        if out_ms < 0 {
            return None;
        }
        let mut before = 0;
        for k in &self.keep {
            let len = k.end_ms - k.start_ms;
            if out_ms < before + len {
                return Some(k.start_ms + out_ms - before);
            }
            before += len;
        }
        None
    }

    /// Output time at which source time `src_ms` is shown, or `None` when it was cut.
    pub fn to_output(&self, src_ms: i64) -> Option<i64> {
        let mut before = 0;
        for k in &self.keep {
            if src_ms >= k.start_ms && src_ms < k.end_ms {
                return Some(before + src_ms - k.start_ms);
            }
            before += k.end_ms - k.start_ms;
        }
        None
    }

    /// Audio gain at source time `src_ms`: 1 inside kept spans, ramping to 0 over
    /// `JOIN_FADE_MS` towards each join between two kept spans; 0 where cut.
    pub fn join_gain(&self, src_ms: f64) -> f32 {
        let last = self.keep.len().saturating_sub(1);
        for (i, k) in self.keep.iter().enumerate() {
            if src_ms >= k.start_ms as f64 && src_ms < k.end_ms as f64 {
                let mut d = f64::INFINITY;
                if i > 0 {
                    d = d.min(src_ms - k.start_ms as f64);
                }
                if i < last {
                    d = d.min(k.end_ms as f64 - src_ms);
                }
                return (d / JOIN_FADE_MS).min(1.0) as f32;
            }
        }
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start_ms: i64, end_ms: i64) -> Span {
        Span { start_ms, end_ms }
    }

    fn sample() -> Cuts {
        Cuts::new(10_000, 1_000, Some(9_000), &[span(3_000, 5_000)])
    }

    #[test]
    fn keep_spans_are_trim_minus_removed() {
        let c = sample();
        assert_eq!(c.keep(), &[span(1000, 3000), span(5000, 9000)]);
        assert_eq!(c.output_ms(), 6000);
    }

    #[test]
    fn output_time_maps_to_source() {
        let c = sample();
        assert_eq!(c.to_source(0), Some(1000));
        assert_eq!(c.to_source(1999), Some(2999));
        assert_eq!(c.to_source(2000), Some(5000));
        assert_eq!(c.to_source(5999), Some(8999));
        assert_eq!(c.to_source(6000), None);
        assert_eq!(c.to_source(-1), None);
    }

    #[test]
    fn source_time_maps_to_output() {
        let c = sample();
        assert_eq!(c.to_output(4000), None);
        assert_eq!(c.to_output(5000), Some(2000));
        assert_eq!(c.to_output(500), None);
        assert_eq!(c.to_output(9000), None);
        assert_eq!(c.to_output(1000), Some(0));
    }

    #[test]
    fn overlapping_and_outside_removals_are_merged_and_clipped() {
        let c = Cuts::new(10_000, 0, None, &[span(2000, 4000), span(3000, 6000), span(9500, 20000), span(-50, 100)]);
        assert_eq!(c.keep(), &[span(100, 2000), span(6000, 9500)]);
    }

    #[test]
    fn reversed_or_empty_spans_are_ignored() {
        let c = Cuts::new(10_000, 0, None, &[span(5000, 5000), span(6000, 4000)]);
        assert_eq!(c.keep(), &[span(0, 10000)]);
    }

    #[test]
    fn trim_end_past_the_video_is_clamped() {
        assert_eq!(Cuts::new(4000, 0, Some(99_999), &[]).output_ms(), 4000);
    }

    #[test]
    fn cutting_everything_leaves_nothing() {
        let c = Cuts::new(4000, 3000, Some(1000), &[]);
        assert!(c.keep().is_empty());
        assert_eq!(c.output_ms(), 0);
        assert_eq!(c.to_source(0), None);
    }

    #[test]
    fn audio_fades_only_around_joins() {
        let c = sample();
        assert_eq!(c.join_gain(2000.0), 1.0);
        assert!((c.join_gain(2995.0) - 0.5).abs() < 0.01);
        assert_eq!(c.join_gain(5000.0), 0.0);
        assert!((c.join_gain(5005.0) - 0.5).abs() < 0.01);
        assert_eq!(c.join_gain(1000.0), 1.0);
        assert_eq!(c.join_gain(8999.0), 1.0);
    }
}
