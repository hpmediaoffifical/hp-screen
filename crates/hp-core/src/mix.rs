//! Mixing of microphone and system audio.

/// Adds two sample buffers, clamping to [-1, 1]. The result is as long as the longer input;
/// the shorter one is treated as silence past its end.
pub fn mix_f32(a: &[f32], b: &[f32]) -> Vec<f32> {
    (0..a.len().max(b.len()))
        .map(|i| {
            let sum = a.get(i).copied().unwrap_or(0.0) + b.get(i).copied().unwrap_or(0.0);
            sum.clamp(-1.0, 1.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_samples_and_clamps_to_full_scale() {
        assert_eq!(mix_f32(&[0.5, 0.9], &[0.25, 0.9, -0.1]), vec![0.75, 1.0, -0.1]);
        assert_eq!(mix_f32(&[-0.8], &[-0.8]), vec![-1.0]);
    }

    #[test]
    fn one_empty_source_passes_the_other_through() {
        assert_eq!(mix_f32(&[], &[0.1, -0.2]), vec![0.1, -0.2]);
        assert_eq!(mix_f32(&[0.3], &[]), vec![0.3]);
        assert!(mix_f32(&[], &[]).is_empty());
    }
}

/// Keeps the first two channels of interleaved audio; mono is copied to both sides.
pub fn to_stereo(input: &[f32], channels: usize) -> Vec<f32> {
    match channels {
        0 => Vec::new(),
        1 => input.iter().flat_map(|s| [*s, *s]).collect(),
        _ => input.chunks_exact(channels).flat_map(|frame| [frame[0], frame[1]]).collect(),
    }
}

/// Linear resampler for interleaved stereo that can be fed in chunks of any size.
///
/// Positions are tracked as integers so the output does not depend on how the input is split.
#[derive(Clone, Debug)]
pub struct Resampler {
    from_rate: u64,
    to_rate: u64,
    /// Position of the next output frame, in units of 1/`to_rate` input frames, where frame 0
    /// is `prev` (the last frame of the previous chunk) and frame 1 starts the current chunk.
    phase: u64,
    prev: [f32; 2],
}

impl Resampler {
    pub fn new(from_rate: u32, to_rate: u32) -> Self {
        let to_rate = to_rate.max(1) as u64;
        Self { from_rate: from_rate.max(1) as u64, to_rate, phase: to_rate, prev: [0.0; 2] }
    }

    /// Converts the next chunk of interleaved stereo samples.
    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if self.from_rate == self.to_rate {
            return input[..input.len() & !1].to_vec();
        }
        let frames = (input.len() / 2) as u64;
        if frames == 0 {
            return Vec::new();
        }
        let frame = |index: u64| -> [f32; 2] {
            match index {
                0 => self.prev,
                i => {
                    let at = (i as usize - 1) * 2;
                    [input[at], input[at + 1]]
                }
            }
        };
        let mut out = Vec::with_capacity((frames * self.to_rate / self.from_rate + 2) as usize * 2);
        let mut phase = self.phase;
        // Each output needs the frame at its position and the one after it.
        while phase / self.to_rate < frames {
            let index = phase / self.to_rate;
            let t = (phase % self.to_rate) as f32 / self.to_rate as f32;
            let (a, b) = (frame(index), frame(index + 1));
            out.push(a[0] + (b[0] - a[0]) * t);
            out.push(a[1] + (b[1] - a[1]) * t);
            phase += self.from_rate;
        }
        let last = frame(frames);
        self.phase = phase - frames * self.to_rate;
        self.prev = last;
        out
    }
}

#[cfg(test)]
mod convert_tests {
    use super::*;

    #[test]
    fn mono_is_copied_to_both_channels() {
        assert_eq!(to_stereo(&[0.1, 0.2], 1), vec![0.1, 0.1, 0.2, 0.2]);
    }

    #[test]
    fn stereo_passes_through_and_surround_keeps_front_pair() {
        assert_eq!(to_stereo(&[0.1, 0.2, 0.3, 0.4], 2), vec![0.1, 0.2, 0.3, 0.4]);
        assert_eq!(to_stereo(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0], 6), vec![1.0, 2.0, 7.0, 8.0]);
    }

    #[test]
    fn zero_channels_or_partial_frames_do_not_panic() {
        assert!(to_stereo(&[0.5, 0.5], 0).is_empty());
        assert_eq!(to_stereo(&[1.0, 2.0, 3.0], 2), vec![1.0, 2.0]);
    }

    fn stereo(values: &[f32]) -> Vec<f32> {
        values.iter().flat_map(|v| [*v, -*v]).collect()
    }

    #[test]
    fn same_rate_is_a_passthrough() {
        let mut r = Resampler::new(48_000, 48_000);
        let input = stereo(&[0.1, 0.2, 0.3]);
        assert_eq!(r.process(&input), input);
    }

    #[test]
    fn doubling_the_rate_interpolates_between_frames() {
        let mut r = Resampler::new(24_000, 48_000);
        assert_eq!(r.process(&stereo(&[0.0, 1.0, 2.0, 3.0])), stereo(&[0.0, 0.5, 1.0, 1.5, 2.0, 2.5]));
        // The next chunk continues from the last frame of the previous one.
        assert_eq!(r.process(&stereo(&[4.0])), stereo(&[3.0, 3.5]));
    }

    #[test]
    fn chunking_does_not_change_the_output() {
        let input = stereo(&(0..441).map(|i| (i as f32 * 0.05).sin()).collect::<Vec<_>>());
        let whole = Resampler::new(44_100, 48_000).process(&input);
        let mut chunked = Resampler::new(44_100, 48_000);
        let mut pieces = Vec::new();
        for chunk in input.chunks(2 * 37) {
            pieces.extend(chunked.process(chunk));
        }
        assert_eq!(pieces, whole);
        // 441 frames at 44.1 kHz are 10 ms: about 480 frames at 48 kHz.
        assert!((478..=480).contains(&(whole.len() / 2)), "{} frames", whole.len() / 2);
    }

    #[test]
    fn halving_the_rate_drops_every_other_frame() {
        let mut r = Resampler::new(96_000, 48_000);
        assert_eq!(r.process(&stereo(&[0.0, 1.0, 2.0, 3.0, 4.0])), stereo(&[0.0, 2.0]));
    }
}
