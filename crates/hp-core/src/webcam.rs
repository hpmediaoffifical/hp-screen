//! Webcam in a corner of the recording: its settings, where the bubble sits in the frame, and
//! which part of the camera picture fills it.
use crate::geometry::Rect;
use serde::{Deserialize, Serialize};

/// Distance from the frame edges, as a fraction of the frame height.
pub const MARGIN: f32 = 0.025;
/// Corner radius of the rounded shape, as a fraction of the bubble height.
pub const ROUNDED_RADIUS: f32 = 0.12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CamShape {
    Circle,
    Rounded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CamSize {
    Small,
    Medium,
    Large,
}

impl CamSize {
    /// Bubble height as a fraction of the frame height.
    pub fn fraction(self) -> f32 {
        match self {
            CamSize::Small => 0.15,
            CamSize::Medium => 0.20,
            CamSize::Large => 0.25,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WebcamSettings {
    /// Turn the webcam on when a recording starts.
    pub enabled: bool,
    /// Camera id (symbolic link); `None` is the first camera.
    pub device: Option<String>,
    pub shape: CamShape,
    pub size: CamSize,
    pub corner: Corner,
    pub mirror: bool,
}

impl Default for WebcamSettings {
    fn default() -> Self {
        WebcamSettings { enabled: false, device: None, shape: CamShape::Circle, size: CamSize::Medium, corner: Corner::BottomRight, mirror: true }
    }
}

/// Where the webcam bubble sits in a `frame_w`×`frame_h` frame.
pub fn bubble_rect(frame_w: u32, frame_h: u32, ws: &WebcamSettings) -> Rect {
    let margin = (MARGIN * frame_h as f32).round() as u32;
    let max_w = frame_w.saturating_sub(2 * margin).max(4);
    let max_h = frame_h.saturating_sub(2 * margin).max(3);
    let mut h = ((ws.size.fraction() * frame_h as f32).round() as u32).max(3);
    let (mut w, ratio) = match ws.shape {
        CamShape::Circle => (h, (1, 1)),
        CamShape::Rounded => ((h as f32 * 4.0 / 3.0).round() as u32, (4, 3)),
    };
    if w > max_w || h > max_h {
        // Shrink to fit, keeping the exact shape.
        let units = (max_w / ratio.0).min(max_h / ratio.1).max(1);
        (w, h) = (units * ratio.0, units * ratio.1);
    }
    let x = match ws.corner {
        Corner::TopLeft | Corner::BottomLeft => margin as i32,
        Corner::TopRight | Corner::BottomRight => frame_w as i32 - margin as i32 - w as i32,
    };
    let y = match ws.corner {
        Corner::TopLeft | Corner::TopRight => margin as i32,
        Corner::BottomLeft | Corner::BottomRight => frame_h as i32 - margin as i32 - h as i32,
    };
    Rect { x: x.max(0), y: y.max(0), w, h }
}

/// The centred part of a `cam_w`×`cam_h` camera picture that fills the bubble.
pub fn camera_crop(cam_w: u32, cam_h: u32, shape: CamShape) -> Rect {
    let aspect = match shape {
        CamShape::Circle => 1.0,
        CamShape::Rounded => 4.0 / 3.0,
    };
    if cam_w as f32 > cam_h as f32 * aspect {
        let w = ((cam_h as f32 * aspect).round() as u32).min(cam_w);
        Rect { x: ((cam_w - w) / 2) as i32, y: 0, w, h: cam_h }
    } else {
        let h = ((cam_w as f32 / aspect).round() as u32).min(cam_h);
        Rect { x: 0, y: ((cam_h - h) / 2) as i32, w: cam_w, h }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(shape: CamShape, size: CamSize, corner: Corner) -> WebcamSettings {
        WebcamSettings { shape, size, corner, ..WebcamSettings::default() }
    }

    #[test]
    fn medium_circle_bottom_right() {
        assert_eq!(bubble_rect(1920, 1080, &WebcamSettings::default()), Rect { x: 1677, y: 837, w: 216, h: 216 });
    }

    #[test]
    fn rounded_is_four_by_three() {
        let r = bubble_rect(1920, 1080, &ws(CamShape::Rounded, CamSize::Medium, Corner::BottomRight));
        assert_eq!((r.w, r.h), (288, 216));
        assert_eq!(r.x, 1920 - 27 - 288);
    }

    #[test]
    fn each_corner() {
        let at = |c| {
            let r = bubble_rect(1000, 1000, &ws(CamShape::Circle, CamSize::Small, c));
            (r.x, r.y)
        };
        assert_eq!(at(Corner::TopLeft), (25, 25));
        assert_eq!(at(Corner::TopRight), (825, 25));
        assert_eq!(at(Corner::BottomLeft), (25, 825));
        assert_eq!(at(Corner::BottomRight), (825, 825));
    }

    #[test]
    fn tiny_frames_shrink_the_bubble() {
        let r = bubble_rect(100, 400, &ws(CamShape::Rounded, CamSize::Large, Corner::BottomRight));
        assert!(r.x >= 0 && r.x as u32 + r.w <= 100, "{r:?}");
        assert!(r.y >= 0 && r.y as u32 + r.h <= 400, "{r:?}");
        assert!(r.w > 0 && r.h > 0);
        assert_eq!(r.w * 3, r.h * 4, "keeps 4:3");
    }

    #[test]
    fn crop_is_centred() {
        assert_eq!(camera_crop(1280, 720, CamShape::Circle), Rect { x: 280, y: 0, w: 720, h: 720 });
        assert_eq!(camera_crop(1280, 720, CamShape::Rounded), Rect { x: 160, y: 0, w: 960, h: 720 });
        assert_eq!(camera_crop(640, 480, CamShape::Rounded), Rect { x: 0, y: 0, w: 640, h: 480 });
        assert_eq!(camera_crop(480, 640, CamShape::Circle), Rect { x: 0, y: 80, w: 480, h: 480 });
    }

    #[test]
    fn webcam_settings_default_and_survive_bad_values() {
        let s: WebcamSettings = serde_json::from_str(r#"{"size":"large"}"#).unwrap();
        assert_eq!(s.size, CamSize::Large);
        assert!(s.mirror && !s.enabled);
        assert_eq!(s.corner, Corner::BottomRight);
        let all = crate::settings::load_settings(r#"{"webcam":{"size":"huge"},"fps":60}"#);
        assert_eq!(all.webcam, WebcamSettings::default());
        assert_eq!(all.fps, 60);
    }
}
