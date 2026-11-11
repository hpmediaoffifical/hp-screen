//! The output frame: its aspect, where the video sits in it, the decoration around it, and
//! which part of the source is shown (zoom, plus the crop for "fill").
use crate::geometry::make_even;
use crate::zoom::Camera;
use serde::{Deserialize, Serialize};

pub const MAX_PADDING: f32 = 0.2;
pub const MAX_RADIUS: f32 = 40.0;
/// Shadow offset as a fraction of the short side.
pub const SHADOW_OFFSET: f32 = 0.015;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Aspect {
    #[serde(rename = "source")]
    Source,
    #[serde(rename = "16:9")]
    Wide,
    #[serde(rename = "9:16")]
    Tall,
    #[serde(rename = "1:1")]
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    Fit,
    Fill,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FrameDoc {
    pub aspect: Aspect,
    pub fit: Fit,
    pub background: String,
    /// Fraction of the short side, each side.
    pub padding: f32,
    /// Corner radius of the video, in output pixels.
    pub radius: f32,
    pub shadow: bool,
}

impl Default for FrameDoc {
    fn default() -> Self {
        FrameDoc { aspect: Aspect::Source, fit: Fit::Fit, background: "slate".to_string(), padding: 0.0, radius: 0.0, shadow: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Background {
    Solid { color: [u8; 3] },
    Gradient { from: [u8; 3], to: [u8; 3] },
}

/// A background preset by id; unknown ids give `slate`.
pub fn background(id: &str) -> Background {
    match id {
        "black" => Background::Solid { color: [0x00, 0x00, 0x00] },
        "white" => Background::Solid { color: [0xff, 0xff, 0xff] },
        "ocean" => Background::Gradient { from: [0x0e, 0xa5, 0xe9], to: [0x63, 0x66, 0xf1] },
        "sunset" => Background::Gradient { from: [0xf9, 0x73, 0x16], to: [0xec, 0x48, 0x99] },
        "forest" => Background::Gradient { from: [0x10, 0xb9, 0x81], to: [0x0f, 0x76, 0x6e] },
        _ => Background::Solid { color: [0x1e, 0x29, 0x3b] },
    }
}

/// Width over height of the output for `aspect`, or `None` to keep the source's.
fn ratio(aspect: Aspect) -> Option<f32> {
    match aspect {
        Aspect::Source => None,
        Aspect::Wide => Some(16.0 / 9.0),
        Aspect::Tall => Some(9.0 / 16.0),
        Aspect::Square => Some(1.0),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Layout {
    pub out_w: u32,
    pub out_h: u32,
    /// Where the video sits in the output: (x, y, w, h) in output pixels.
    pub video: (f32, f32, f32, f32),
    pub radius: f32,
    pub shadow: bool,
    pub background: Background,
    /// For "fill": the aspect the source is cropped to.
    pub fill_aspect: Option<f32>,
    /// Anything visible besides the video itself (background, rounded corners, shadow).
    pub decorated: bool,
}

pub fn layout(src_w: u32, src_h: u32, f: &FrameDoc) -> Layout {
    let (sw, sh) = make_even(src_w.max(2), src_h.max(2));
    let (out_w, out_h) = match ratio(f.aspect) {
        None => (sw, sh),
        Some(r) => make_even(((sh as f32 * r).round() as u32).max(2), sh),
    };
    let (ow, oh) = (out_w as f32, out_h as f32);
    let padding = if f.padding.is_finite() { f.padding.clamp(0.0, MAX_PADDING) } else { 0.0 };
    let pad = padding * ow.min(oh);
    let (cx, cy, cw, ch) = (pad, pad, ow - 2.0 * pad, oh - 2.0 * pad);
    let (video, fill_aspect) = match f.fit {
        Fit::Fill => ((cx, cy, cw, ch), Some(cw / ch)),
        Fit::Fit => {
            let src_aspect = sw as f32 / sh as f32;
            let (w, h) = if cw / ch > src_aspect { (ch * src_aspect, ch) } else { (cw, cw / src_aspect) };
            ((cx + (cw - w) / 2.0, cy + (ch - h) / 2.0, w, h), None)
        }
    };
    let radius = if f.radius.is_finite() { f.radius.clamp(0.0, MAX_RADIUS) } else { 0.0 };
    let covers = (video.0, video.1, video.2, video.3) == (0.0, 0.0, ow, oh);
    let decorated = !covers || radius > 0.0 || f.shadow;
    Layout { out_w, out_h, video, radius, shadow: f.shadow, background: background(&f.background), fill_aspect, decorated }
}

/// The part of the source shown, in source pixels: the zoom camera's view, cropped to
/// `fill_aspect` around the camera centre when filling, always inside the source.
pub fn source_view(cam: Camera, src_w: u32, src_h: u32, fill_aspect: Option<f32>) -> (f32, f32, f32, f32) {
    let (x, y, w, h) = cam.view(src_w, src_h);
    let (sw, sh) = (src_w as f32, src_h as f32);
    let Some(a) = fill_aspect.filter(|a| a.is_finite() && *a > 0.0) else { return (x, y, w, h) };
    if w / h > a {
        let nw = h * a;
        let nx = (cam.cx * sw - nw / 2.0).clamp(0.0, (sw - nw).max(0.0));
        (nx, y, nw, h)
    } else {
        let nh = w / a;
        let ny = (cam.cy * sh - nh / 2.0).clamp(0.0, (sh - nh).max(0.0));
        (x, ny, w, nh)
    }
}

impl Layout {
    /// The same layout shrunk to `width` pixels wide (GIF).
    pub fn scaled(&self, width: u32) -> Layout {
        let k = width.max(1) as f32 / self.out_w.max(1) as f32;
        let (x, y, w, h) = self.video;
        Layout {
            out_w: width.max(1),
            out_h: ((self.out_h as f32 * k).round() as u32).max(1),
            video: (x * k, y * k, w * k, h * k),
            radius: self.radius * k,
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(aspect: Aspect, fit: Fit, padding: f32) -> FrameDoc {
        FrameDoc { aspect, fit, padding, ..FrameDoc::default() }
    }

    fn close(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
        (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01 && (a.2 - b.2).abs() < 0.01 && (a.3 - b.3).abs() < 0.01
    }

    #[test]
    fn source_aspect_is_the_identity() {
        let l = layout(1920, 1080, &FrameDoc::default());
        assert_eq!((l.out_w, l.out_h), (1920, 1080));
        assert_eq!(l.video, (0.0, 0.0, 1920.0, 1080.0));
        assert!(!l.decorated && l.fill_aspect.is_none());
    }

    #[test]
    fn tall_frame_from_a_wide_source_fits_with_bars() {
        let l = layout(1920, 1080, &frame(Aspect::Tall, Fit::Fit, 0.0));
        assert_eq!((l.out_w, l.out_h), (608, 1080));
        assert!(close(l.video, (0.0, 369.0, 608.0, 342.0)), "{:?}", l.video);
        assert!(l.decorated);
    }

    #[test]
    fn square_frame() {
        let l = layout(1920, 1080, &frame(Aspect::Square, Fit::Fit, 0.0));
        assert_eq!((l.out_w, l.out_h), (1080, 1080));
        assert!(close(l.video, (0.0, 236.25, 1080.0, 607.5)), "{:?}", l.video);
    }

    #[test]
    fn padding_shrinks_the_video() {
        let l = layout(1920, 1080, &frame(Aspect::Source, Fit::Fit, 0.2));
        assert_eq!((l.out_w, l.out_h), (1920, 1080));
        assert!(close(l.video, (384.0, 216.0, 1152.0, 648.0)), "{:?}", l.video);
        assert!(l.decorated);
        let too_much = layout(1920, 1080, &frame(Aspect::Source, Fit::Fit, 0.9));
        assert!(close(too_much.video, l.video), "padding is capped at 20%");
    }

    #[test]
    fn fill_covers_the_frame() {
        let l = layout(1920, 1080, &frame(Aspect::Tall, Fit::Fill, 0.0));
        assert_eq!(l.video, (0.0, 0.0, 608.0, 1080.0));
        assert!((l.fill_aspect.unwrap() - 608.0 / 1080.0).abs() < 1e-4);
        assert!(!l.decorated);
    }

    #[test]
    fn odd_sources_give_even_frames() {
        let l = layout(641, 481, &FrameDoc::default());
        assert_eq!((l.out_w, l.out_h), (640, 480));
        let t = layout(641, 481, &frame(Aspect::Tall, Fit::Fit, 0.0));
        assert!(t.out_w % 2 == 0 && t.out_h % 2 == 0, "{}x{}", t.out_w, t.out_h);
    }

    #[test]
    fn fill_view_has_the_frame_aspect_and_stays_inside() {
        let a = 608.0 / 1080.0;
        let v = source_view(Camera::WHOLE, 1920, 1080, Some(a));
        assert!(close(v, (656.0, 0.0, 608.0, 1080.0)), "608/1080 of the height, centred: {v:?}");
        let corner = source_view(Camera { cx: 0.25, cy: 0.25, scale: 2.0 }, 1920, 1080, Some(a));
        assert!((corner.2 / corner.3 - a).abs() < 1e-3, "{corner:?}");
        assert!(corner.0 >= 0.0 && corner.0 + corner.2 <= 1920.0 + 1e-3);
        let edge = source_view(Camera { cx: 0.0, cy: 0.5, scale: 1.0 }, 1920, 1080, Some(a));
        assert_eq!(edge.0, 0.0);
        assert_eq!(source_view(Camera::WHOLE, 1920, 1080, None), (0.0, 0.0, 1920.0, 1080.0));
    }

    #[test]
    fn unknown_background_is_slate() {
        assert_eq!(background("nope"), background("slate"));
        assert_eq!(background("slate"), Background::Solid { color: [0x1e, 0x29, 0x3b] });
        assert_eq!(background("ocean"), Background::Gradient { from: [0x0e, 0xa5, 0xe9], to: [0x63, 0x66, 0xf1] });
    }

    #[test]
    fn scaled_layout_for_gif() {
        let l = layout(1920, 1080, &FrameDoc { radius: 20.0, ..frame(Aspect::Tall, Fit::Fit, 0.0) });
        let g = l.scaled(304);
        assert_eq!((g.out_w, g.out_h), (304, 540));
        assert!(close(g.video, (0.0, 184.5, 304.0, 171.0)), "{:?}", g.video);
        assert!((g.radius - 10.0).abs() < 1e-4);
    }

    #[test]
    fn old_edit_files_read_with_the_default_frame() {
        let doc = crate::edit::load_edit("{\"v\":1}").unwrap();
        assert_eq!(doc.frame, FrameDoc::default());
    }
}
