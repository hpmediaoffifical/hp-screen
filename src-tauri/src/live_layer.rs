//! What is drawn live onto recorded frames: strokes the user draws on screen while recording
//! (and, later, the webcam picture). The pacer thread calls `apply` on each frame right before
//! it is encoded; only the area that has something on it goes through Direct2D.
use crate::clock;
use crate::compositor::{Device, bgra_props, d2d_context};
use hp_core::geometry::Rect;
use crate::camera::{CamFrame, SharedFrame};
use hp_core::stroke::{Stroke, alpha_at, segments, visible_bounds};
use hp_core::webcam::{CamShape, ROUNDED_RADIUS, WebcamSettings, bubble_rect, camera_crop};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows_numerics::{Matrix3x2, Vector2};

/// A camera frame older than this is no longer drawn (the camera stopped or was unplugged).
const CAMERA_STALE_MS: i64 = 1000;

pub struct LiveLayer {
    strokes: Mutex<Vec<Stroke>>,
    hold_ms: AtomicI64,
    drawing: Arc<AtomicBool>,
    device: Device,
    /// Direct2D objects, made on first use by the thread that calls `apply`.
    painter: Mutex<Option<Painter>>,
    camera: Mutex<Option<(SharedFrame, WebcamSettings)>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl LiveLayer {
    pub fn new() -> Arc<LiveLayer> {
        Self::with_device(Device::Hardware)
    }

    pub fn with_device(device: Device) -> Arc<LiveLayer> {
        Arc::new(LiveLayer {
            strokes: Mutex::new(Vec::new()),
            hold_ms: AtomicI64::new(3000),
            drawing: Arc::new(AtomicBool::new(false)),
            device,
            painter: Mutex::new(None),
            camera: Mutex::new(None),
        })
    }

    /// How long finished strokes stay before fading (0: until cleared).
    pub fn set_hold_ms(&self, ms: i64) {
        self.hold_ms.store(ms, Ordering::Relaxed);
    }

    /// Adds or replaces the stroke with this id. A `finished_ms` of `Some(_)` marks the stroke
    /// done now, on the recording clock.
    pub fn upsert_stroke(&self, mut s: Stroke) {
        let mut strokes = lock(&self.strokes);
        let previous = strokes.iter().position(|x| x.id == s.id);
        if s.finished_ms.is_some() {
            // Keep the first finish time if the UI repeats the final update.
            let earlier = previous.and_then(|i| strokes[i].finished_ms);
            s.finished_ms = Some(earlier.unwrap_or_else(now_ms));
        }
        match previous {
            Some(i) => strokes[i] = s,
            None => strokes.push(s),
        }
        // Forget strokes that have faded out for good.
        let (now, hold) = (now_ms(), self.hold_ms.load(Ordering::Relaxed));
        strokes.retain(|x| alpha_at(x, now, hold) > 0.0);
    }

    pub fn clear(&self) {
        lock(&self.strokes).clear();
    }

    /// The camera picture to draw, and how; `None` removes the webcam.
    pub fn set_camera(&self, cam: Option<(SharedFrame, WebcamSettings)>) {
        *lock(&self.camera) = cam;
    }

    #[cfg(test)]
    pub fn has_camera(&self) -> bool {
        lock(&self.camera).is_some()
    }

    #[cfg(test)]
    pub fn stroke_count(&self) -> usize {
        lock(&self.strokes).len()
    }

    pub fn set_drawing(&self, on: bool) {
        self.drawing.store(on, Ordering::Relaxed);
    }

    pub fn is_drawing(&self) -> bool {
        self.drawing.load(Ordering::Relaxed)
    }

    /// Shared with the input hook, which skips clicks while the user draws.
    pub fn drawing_flag(&self) -> Arc<AtomicBool> {
        self.drawing.clone()
    }

    /// Draws what is visible at `now_ms` onto `canvas` (top-down BGRA, `w * 4` per row).
    pub fn apply(&self, canvas: &mut [u8], w: u32, h: u32, now_ms: i64) {
        if w == 0 || h == 0 || canvas.len() < (w * h * 4) as usize {
            return;
        }
        let hold = self.hold_ms.load(Ordering::Relaxed);
        let visible: Vec<(Stroke, f32)> = lock(&self.strokes)
            .iter()
            .map(|s| (s.clone(), alpha_at(s, now_ms, hold)))
            .filter(|(_, a)| *a > 0.0)
            .collect();
        let strokes: Vec<Stroke> = visible.iter().map(|(s, _)| s.clone()).collect();
        let stroke_area = visible_bounds(&strokes, now_ms, hold, w, h);
        let camera = lock(&self.camera).clone();
        let frame_guard = camera.as_ref().map(|(f, _)| lock(f));
        let cam = match (&camera, &frame_guard) {
            (Some((_, ws)), Some(guard)) => guard
                .as_ref()
                .filter(|f| {
                    now_ms - f.at_ms <= CAMERA_STALE_MS && f.width > 0 && f.height > 0 && f.bgra.len() >= (f.width * f.height * 4) as usize
                })
                .map(|f| (f, ws, bubble_rect(w, h, ws))),
            _ => None,
        };
        let area = match (stroke_area, cam.as_ref().map(|c| c.2)) {
            (Some(a), Some(b)) => Some(union(a, b)),
            (a, b) => a.or(b),
        };
        let Some(area) = area else { return };
        let mut painter = lock(&self.painter);
        if painter.as_ref().is_none_or(|p| (p.w, p.h) != (w, h)) {
            *painter = Painter::new(self.device, w, h).ok();
        }
        if let Some(p) = painter.as_mut()
            && p.paint(canvas, area, &visible, cam).is_err()
        {
            // A lost device is made again on the next frame.
            *painter = None;
        }
    }
}

fn union(a: Rect, b: Rect) -> Rect {
    let (x0, y0) = (a.x.min(b.x), a.y.min(b.y));
    let x1 = (a.x + a.w as i32).max(b.x + b.w as i32);
    let y1 = (a.y + a.h as i32).max(b.y + b.h as i32);
    Rect { x: x0, y: y0, w: (x1 - x0) as u32, h: (y1 - y0) as u32 }
}

type CamDraw<'a> = (&'a CamFrame, &'a WebcamSettings, Rect);

fn now_ms() -> i64 {
    clock::now_100ns() / 10_000
}

/// Direct2D target the size of the frame; only the drawn area is uploaded and read back.
struct Painter {
    ctx: ID2D1DeviceContext,
    factory: ID2D1Factory,
    round: ID2D1StrokeStyle,
    target: ID2D1Bitmap1,
    readback: ID2D1Bitmap1,
    w: u32,
    h: u32,
    /// Camera picture (its cropped part), remade when the crop size changes.
    cam_bitmap: Option<(ID2D1Bitmap1, u32, u32)>,
}

// SAFETY: the painter is only used under the layer's mutex, by one thread at a time, and its
// single-threaded Direct2D objects are never used concurrently.
unsafe impl Send for Painter {}

impl Painter {
    fn new(device: Device, w: u32, h: u32) -> Result<Painter, String> {
        let ctx = d2d_context(device)?;
        // SAFETY: plain Direct2D object creation with valid arguments.
        unsafe {
            let factory = ctx.GetFactory().map_err(|e| e.to_string())?;
            let props = D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_ROUND,
                endCap: D2D1_CAP_STYLE_ROUND,
                dashCap: D2D1_CAP_STYLE_ROUND,
                lineJoin: D2D1_LINE_JOIN_ROUND,
                miterLimit: 10.0,
                dashStyle: D2D1_DASH_STYLE_SOLID,
                dashOffset: 0.0,
            };
            let round = factory.CreateStrokeStyle(&props, None).map_err(|e| e.to_string())?;
            let size = D2D_SIZE_U { width: w, height: h };
            let target = ctx
                .CreateBitmap(size, None, 0, &bgra_props(D2D1_BITMAP_OPTIONS_TARGET, D2D1_ALPHA_MODE_PREMULTIPLIED))
                .map_err(|e| e.to_string())?;
            let readback = ctx
                .CreateBitmap(size, None, 0, &bgra_props(D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_ALPHA_MODE_PREMULTIPLIED))
                .map_err(|e| e.to_string())?;
            Ok(Painter { ctx, factory, round, target, readback, w, h, cam_bitmap: None })
        }
    }

    fn paint(&mut self, canvas: &mut [u8], area: Rect, strokes: &[(Stroke, f32)], cam: Option<CamDraw>) -> windows::core::Result<()> {
        let row = self.w as usize * 4;
        let (ax, ay, aw, ah) = (area.x as u32, area.y as u32, area.w, area.h);
        let rect_u = D2D_RECT_U { left: ax, top: ay, right: ax + aw, bottom: ay + ah };
        // SAFETY: `area` lies inside the frame (visible_bounds clips it), so every pointer
        // offset below stays within `canvas`, whose size was checked by `apply`; the mapped
        // bitmap is only read while mapped, within its pitch and height.
        unsafe {
            let start = canvas.as_ptr().add(ay as usize * row + ax as usize * 4);
            self.target.CopyFromMemory(Some(&rect_u), start as *const _, row as u32)?;
            self.ctx.SetTarget(&self.target);
            self.ctx.BeginDraw();
            let clip = D2D_RECT_F { left: ax as f32, top: ay as f32, right: (ax + aw) as f32, bottom: (ay + ah) as f32 };
            self.ctx.PushAxisAlignedClip(&clip, D2D1_ANTIALIAS_MODE_ALIASED);
            let drawn = match cam {
                Some(c) => self.draw_camera(c),
                None => Ok(()),
            }
            .and_then(|_| self.draw_strokes(strokes));
            self.ctx.PopAxisAlignedClip();
            self.ctx.EndDraw(None, None)?;
            drawn?;
            let at = D2D_POINT_2U { x: ax, y: ay };
            self.readback.CopyFromBitmap(Some(&at), &self.target, Some(&rect_u))?;
            let mapped = self.readback.Map(D2D1_MAP_OPTIONS_READ)?;
            for y in ay..ay + ah {
                let src = mapped.bits.add(y as usize * mapped.pitch as usize + ax as usize * 4);
                let dst = canvas.as_mut_ptr().add(y as usize * row + ax as usize * 4);
                std::ptr::copy_nonoverlapping(src, dst, aw as usize * 4);
            }
            self.readback.Unmap()?;
        }
        Ok(())
    }

    /// The camera picture in its bubble: cropped, scaled, mirrored, masked to the shape, with a
    /// thin white border. Drawn under the strokes.
    unsafe fn draw_camera(&mut self, (f, ws, bubble): CamDraw) -> windows::core::Result<()> {
        let crop = camera_crop(f.width, f.height, ws.shape);
        // SAFETY: the crop lies inside the frame (camera_crop), whose buffer holds
        // width * height * 4 bytes (checked by `apply`); drawing calls are between BeginDraw
        // and EndDraw.
        unsafe {
            if self.cam_bitmap.as_ref().is_none_or(|(_, w, h)| (*w, *h) != (crop.w, crop.h)) {
                let bmp = self.ctx.CreateBitmap(
                    D2D_SIZE_U { width: crop.w, height: crop.h },
                    None,
                    0,
                    &bgra_props(D2D1_BITMAP_OPTIONS_NONE, D2D1_ALPHA_MODE_IGNORE),
                )?;
                self.cam_bitmap = Some((bmp, crop.w, crop.h));
            }
            let (bmp, _, _) = self.cam_bitmap.as_ref().expect("just made");
            let start = f.bgra.as_ptr().add((crop.y as usize * f.width as usize + crop.x as usize) * 4);
            bmp.CopyFromMemory(None, start as *const _, f.width * 4)?;
            let props = D2D1_BITMAP_BRUSH_PROPERTIES1 {
                extendModeX: D2D1_EXTEND_MODE_CLAMP,
                extendModeY: D2D1_EXTEND_MODE_CLAMP,
                interpolationMode: D2D1_INTERPOLATION_MODE_LINEAR,
            };
            let brush = self.ctx.CreateBitmapBrush(bmp, Some(&props), None)?;
            let (sx, sy) = (bubble.w as f32 / crop.w as f32, bubble.h as f32 / crop.h as f32);
            let (bx, by, bw, bh) = (bubble.x as f32, bubble.y as f32, bubble.w as f32, bubble.h as f32);
            let m = if ws.mirror {
                Matrix3x2 { M11: -sx, M12: 0.0, M21: 0.0, M22: sy, M31: bx + bw, M32: by }
            } else {
                Matrix3x2 { M11: sx, M12: 0.0, M21: 0.0, M22: sy, M31: bx, M32: by }
            };
            brush.SetTransform(&m);
            let border = (0.003 * self.h as f32).max(2.0);
            let white = self.ctx.CreateSolidColorBrush(&D2D1_COLOR_F { r: 1.0, g: 1.0, b: 1.0, a: 1.0 }, None)?;
            match ws.shape {
                CamShape::Circle => {
                    let e = D2D1_ELLIPSE { point: Vector2 { X: bx + bw / 2.0, Y: by + bh / 2.0 }, radiusX: bw / 2.0, radiusY: bh / 2.0 };
                    self.ctx.FillEllipse(&e, &brush);
                    let inner = D2D1_ELLIPSE { radiusX: e.radiusX - border / 2.0, radiusY: e.radiusY - border / 2.0, ..e };
                    self.ctx.DrawEllipse(&inner, &white, border, None::<&ID2D1StrokeStyle>);
                }
                CamShape::Rounded => {
                    let r = ROUNDED_RADIUS * bh;
                    let rect = D2D_RECT_F { left: bx, top: by, right: bx + bw, bottom: by + bh };
                    self.ctx.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect, radiusX: r, radiusY: r }, &brush);
                    let half = border / 2.0;
                    let inner = D2D_RECT_F { left: bx + half, top: by + half, right: bx + bw - half, bottom: by + bh - half };
                    self.ctx.DrawRoundedRectangle(&D2D1_ROUNDED_RECT { rect: inner, radiusX: r, radiusY: r }, &white, border, None::<&ID2D1StrokeStyle>);
                }
            }
        }
        Ok(())
    }

    /// Each stroke is one geometry, so overlapping parts of a fading stroke are not darker.
    unsafe fn draw_strokes(&self, strokes: &[(Stroke, f32)]) -> windows::core::Result<()> {
        // SAFETY: drawing calls between BeginDraw and EndDraw on valid objects.
        unsafe {
            for (s, alpha) in strokes {
                let [r, g, b] = s.color;
                let colour = D2D1_COLOR_F { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: *alpha };
                let brush = self.ctx.CreateSolidColorBrush(&colour, None)?;
                let path = self.factory.CreatePathGeometry()?;
                let sink = path.Open()?;
                for ((x0, y0), (x1, y1)) in segments(s) {
                    sink.BeginFigure(Vector2 { X: x0, Y: y0 }, D2D1_FIGURE_BEGIN_HOLLOW);
                    sink.AddLine(Vector2 { X: x1, Y: y1 });
                    sink.EndFigure(D2D1_FIGURE_END_OPEN);
                }
                sink.Close()?;
                self.ctx.DrawGeometry(&path, &brush, s.width, &self.round);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hp_core::stroke::{COLORS, Tool};

    fn grey(w: u32, h: u32) -> Vec<u8> {
        [128u8, 128, 128, 255].repeat((w * h) as usize)
    }

    fn rgb(c: &[u8], w: u32, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * w + x) * 4) as usize;
        [c[i + 2], c[i + 1], c[i]]
    }

    fn red_line(points: Vec<(f32, f32)>, width: f32) -> Stroke {
        Stroke { id: 1, tool: Tool::Pen, color: COLORS[0], width, points, finished_ms: None }
    }

    #[test]
    fn stroke_is_drawn_onto_the_frame() {
        let live = LiveLayer::with_device(Device::Warp);
        live.upsert_stroke(red_line(vec![(20.0, 100.0), (180.0, 100.0)], 8.0));
        let mut c = grey(200, 200);
        live.apply(&mut c, 200, 200, 0);
        let [r, g, _] = rgb(&c, 200, 100, 100);
        assert!(r > 200 && g < 80, "stroke pixel {:?}", rgb(&c, 200, 100, 100));
        assert_eq!(rgb(&c, 200, 100, 20), [128, 128, 128]);
    }

    #[test]
    fn nothing_visible_leaves_the_frame_untouched() {
        let live = LiveLayer::with_device(Device::Warp);
        let mut c = grey(64, 64);
        live.apply(&mut c, 64, 64, 0);
        assert_eq!(c, grey(64, 64));
        live.set_hold_ms(1000);
        live.upsert_stroke(Stroke { finished_ms: Some(0), ..red_line(vec![(5.0, 5.0), (50.0, 50.0)], 4.0) });
        live.apply(&mut c, 64, 64, i64::MAX / 2);
        assert_eq!(c, grey(64, 64), "a faded stroke draws nothing");
    }

    #[test]
    fn clear_removes_strokes_and_upsert_replaces_by_id() {
        let live = LiveLayer::with_device(Device::Warp);
        live.upsert_stroke(red_line(vec![(0.0, 32.0), (10.0, 32.0)], 6.0));
        live.upsert_stroke(red_line(vec![(40.0, 32.0), (60.0, 32.0)], 6.0));
        let mut c = grey(64, 64);
        live.apply(&mut c, 64, 64, 0);
        assert_eq!(rgb(&c, 64, 5, 32), [128, 128, 128], "first version of stroke 1 was replaced");
        assert!(rgb(&c, 64, 50, 32)[0] > 200);
        live.clear();
        let mut c = grey(64, 64);
        live.apply(&mut c, 64, 64, 0);
        assert_eq!(c, grey(64, 64));
    }

    #[test]
    fn strokes_outside_the_frame_are_clipped() {
        let live = LiveLayer::with_device(Device::Warp);
        live.upsert_stroke(red_line(vec![(-500.0, -10.0), (30.0, 30.0), (900.0, 900.0)], 14.0));
        let mut c = grey(64, 48);
        live.apply(&mut c, 64, 48, 0);
        assert_eq!(c.len(), 64 * 48 * 4);
        live.upsert_stroke(red_line(vec![(-500.0, -500.0), (-400.0, -400.0)], 14.0));
        let mut c = grey(64, 48);
        live.apply(&mut c, 64, 48, 0);
        assert_eq!(c, grey(64, 48), "a stroke fully outside draws nothing");
    }

    fn cam(frame: impl Fn(u32, u32) -> [u8; 3], w: u32, h: u32, at_ms: i64) -> crate::camera::SharedFrame {
        let mut bgra = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let [r, g, b] = frame(x, y);
                bgra.extend_from_slice(&[b, g, r, 255]);
            }
        }
        Arc::new(Mutex::new(Some(crate::camera::CamFrame { width: w, height: h, bgra, at_ms })))
    }

    #[test]
    fn webcam_is_drawn_in_its_bubble() {
        use hp_core::webcam::{WebcamSettings, bubble_rect};
        let live = LiveLayer::with_device(Device::Warp);
        let ws = WebcamSettings::default();
        live.set_camera(Some((cam(|_, _| [255, 0, 0], 320, 240, 1000), ws.clone())));
        let mut c = grey(640, 360);
        live.apply(&mut c, 640, 360, 1000);
        let b = bubble_rect(640, 360, &ws);
        let (cx, cy) = (b.x as u32 + b.w / 2, b.y as u32 + b.h / 2);
        let [r, g, _] = rgb(&c, 640, cx, cy);
        assert!(r > 200 && g < 60, "bubble centre {:?}", rgb(&c, 640, cx, cy));
        assert_eq!(rgb(&c, 640, b.x as u32 + 1, b.y as u32 + 1), [128, 128, 128], "outside the circle");
        assert_eq!(rgb(&c, 640, 10, 10), [128, 128, 128]);
    }

    #[test]
    fn mirror_flips_the_picture() {
        use hp_core::webcam::{WebcamSettings, bubble_rect};
        let live = LiveLayer::with_device(Device::Warp);
        let half = |x: u32, _| if x < 160 { [255, 0, 0] } else { [0, 0, 255] };
        let left_of = |mirror: bool| {
            let ws = WebcamSettings { mirror, ..WebcamSettings::default() };
            live.set_camera(Some((cam(half, 320, 240, 0), ws.clone())));
            let mut c = grey(640, 360);
            live.apply(&mut c, 640, 360, 0);
            let b = bubble_rect(640, 360, &ws);
            rgb(&c, 640, b.x as u32 + b.w / 4, b.y as u32 + b.h / 2)
        };
        assert!(left_of(false)[0] > 200, "unmirrored: left half is red");
        assert!(left_of(true)[2] > 200, "mirrored: left half is blue");
    }

    #[test]
    fn stale_camera_frame_is_dropped() {
        use hp_core::webcam::WebcamSettings;
        let live = LiveLayer::with_device(Device::Warp);
        live.set_camera(Some((cam(|_, _| [255, 0, 0], 320, 240, 0), WebcamSettings::default())));
        let mut c = grey(640, 360);
        live.apply(&mut c, 640, 360, 1500);
        assert_eq!(c, grey(640, 360));
        live.set_camera(None);
    }

    #[test]
    #[ignore]
    fn apply_cost_on_a_big_frame() {
        let live = LiveLayer::with_device(Device::Hardware);
        live.upsert_stroke(red_line((0..100).map(|i| (300.0 + i as f32 * 10.0, 800.0 + (i % 7) as f32 * 20.0)).collect(), 8.0));
        let mut c = grey(2560, 1600);
        live.apply(&mut c, 2560, 1600, 0);
        let start = std::time::Instant::now();
        for _ in 0..30 {
            live.apply(&mut c, 2560, 1600, 0);
        }
        println!("live layer apply on 2560x1600: {:.2} ms per frame", start.elapsed().as_secs_f64() * 1000.0 / 30.0);
    }
}
