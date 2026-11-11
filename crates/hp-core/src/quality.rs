//! Video bitrate from resolution, frame rate and quality level.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    Light,
    #[default]
    Standard,
    High,
}

/// Lowest bitrate ever requested from the encoder.
pub const MIN_BITRATE: u32 = 1_000_000;

/// Bits per second. Standard is 8 Mbps at 1080p30 and 12 Mbps at 1080p60, scaled by pixel
/// count; Light is 0.6x and High is 1.6x.
pub fn video_bitrate(w: u32, h: u32, fps: u32, q: Quality) -> u32 {
    const REFERENCE_PIXELS: u128 = 1920 * 1080;
    let base: u128 = if fps > 30 { 12_000_000 } else { 8_000_000 };
    let tenths: u128 = match q {
        Quality::Light => 6,
        Quality::Standard => 10,
        Quality::High => 16,
    };
    let pixels = w as u128 * h as u128;
    let bitrate = base * pixels * tenths / (REFERENCE_PIXELS * 10);
    bitrate.clamp(MIN_BITRATE as u128, u32::MAX as u128) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_1080p_matches_reference_bitrates() {
        assert_eq!(video_bitrate(1920, 1080, 30, Quality::Standard), 8_000_000);
        assert_eq!(video_bitrate(1920, 1080, 60, Quality::Standard), 12_000_000);
    }

    #[test]
    fn light_and_high_scale_the_standard_bitrate() {
        assert_eq!(video_bitrate(1920, 1080, 30, Quality::Light), 4_800_000);
        assert_eq!(video_bitrate(1920, 1080, 30, Quality::High), 12_800_000);
    }

    #[test]
    fn bitrate_scales_with_pixel_count() {
        assert_eq!(video_bitrate(3840, 2160, 30, Quality::Standard), 32_000_000);
        assert_eq!(video_bitrate(2560, 1440, 60, Quality::Standard), 21_333_333);
    }

    #[test]
    fn tiny_regions_get_the_minimum_bitrate() {
        assert_eq!(video_bitrate(64, 64, 30, Quality::Light), MIN_BITRATE);
    }
}
