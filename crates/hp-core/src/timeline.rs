//! Maps system clock ticks (QPC) to video time, skipping paused spans.
//! Video, audio and the input log all go through one `Timeline` so they stay in sync.

#[derive(Clone, Debug)]
pub struct Timeline {
    start: i64,
    ticks_per_sec: i64,
    paused_total: i64,
    paused_since: Option<i64>,
}

impl Timeline {
    pub fn new(start_ticks: i64, ticks_per_sec: i64) -> Self {
        let _ = (start_ticks, ticks_per_sec);
        todo!()
    }

    /// Starts a pause at `now`. Ignored when already paused.
    pub fn pause(&mut self, now: i64) {
        let _ = now;
        todo!()
    }

    /// Ends the pause at `now`. Ignored when not paused.
    pub fn resume(&mut self, now: i64) {
        let _ = now;
        todo!()
    }

    pub fn is_paused(&self) -> bool {
        todo!()
    }

    /// Video time in 100 ns units (Media Foundation's unit); `None` while paused.
    pub fn video_100ns(&self, now: i64) -> Option<i64> {
        let _ = now;
        todo!()
    }

    /// Video time in milliseconds; `None` while paused.
    pub fn video_ms(&self, now: i64) -> Option<i64> {
        let _ = now;
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_time_counts_from_start() {
        let t = Timeline::new(1_000, 1_000);
        assert_eq!(t.video_ms(3_000), Some(2_000));
        assert_eq!(t.video_100ns(3_000), Some(20_000_000));
    }

    #[test]
    fn time_before_start_is_zero() {
        let t = Timeline::new(1_000, 1_000);
        assert_eq!(t.video_ms(500), Some(0));
    }

    #[test]
    fn paused_span_is_removed_from_video_time() {
        let mut t = Timeline::new(1_000, 1_000);
        t.pause(3_000);
        assert!(t.is_paused());
        assert_eq!(t.video_ms(4_000), None);
        t.resume(5_000);
        assert!(!t.is_paused());
        assert_eq!(t.video_ms(6_000), Some(3_000));
        assert_eq!(t.video_100ns(6_000), Some(30_000_000));
    }

    #[test]
    fn repeated_pause_and_resume_are_ignored() {
        let mut t = Timeline::new(1_000, 1_000);
        t.pause(3_000);
        t.pause(3_500);
        t.resume(5_000);
        t.resume(5_500);
        assert_eq!(t.video_ms(6_000), Some(3_000));
    }

    #[test]
    fn two_pauses_accumulate() {
        let mut t = Timeline::new(0, 1_000);
        t.pause(1_000);
        t.resume(2_000);
        t.pause(3_000);
        t.resume(5_000);
        assert_eq!(t.video_ms(6_000), Some(3_000));
    }

    #[test]
    fn real_qpc_frequency_does_not_overflow() {
        // 10 MHz QPC, ten hours in.
        let freq = 10_000_000;
        let t = Timeline::new(0, freq);
        assert_eq!(t.video_ms(freq * 36_000), Some(36_000_000));
        assert_eq!(t.video_100ns(freq * 36_000), Some(360_000_000_000));
    }
}
