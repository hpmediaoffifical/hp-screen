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
