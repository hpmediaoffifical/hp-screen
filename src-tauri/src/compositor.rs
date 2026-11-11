//! Builds export frames with Direct2D: the source picture scaled into the output, then the
//! effects drawn over it, read back as top-down BGRA for the encoder.
use hp_core::effects::{Draw, RIPPLE_RADIUS};
use hp_core::input_log::Button;
use std::mem::ManuallyDrop;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::core::{HSTRING, Interface};
use windows_numerics::Vector2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Device {
    /// The GPU, falling back to WARP when no hardware device can be made.
    Hardware,
    /// Software rendering (tests).
    #[cfg_attr(not(test), allow(dead_code))]
    Warp,
}

pub struct Compositor {
    ctx: ID2D1DeviceContext,
    target: ID2D1Bitmap1,
    readback: ID2D1Bitmap1,
    source: Option<(ID2D1Bitmap1, u32, u32)>,
    out_w: u32,
    out_h: u32,
    out: Vec<u8>,
    text: IDWriteFactory,
    chip_format: IDWriteTextFormat,
    /// Chip font size in pixels.
    chip_size: f32,
}

/// Chip text height as a fraction of the output height.
const CHIP_FONT: f32 = 0.032;

// SAFETY: a Compositor is created and used on one export thread at a time; Direct2D objects
// from a single-threaded factory are never touched by two threads at once.
unsafe impl Send for Compositor {}

fn err(context: &str) -> impl Fn(windows::core::Error) -> String + '_ {
    move |e| format!("{context}: {e}")
}

fn bitmap_props(options: D2D1_BITMAP_OPTIONS, alpha: D2D1_ALPHA_MODE) -> D2D1_BITMAP_PROPERTIES1 {
    D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: alpha },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: options,
        colorContext: ManuallyDrop::new(None),
    }
}

unsafe fn d3d_device(driver: D3D_DRIVER_TYPE) -> windows::core::Result<ID3D11Device> {
    unsafe {
        let mut device = None;
        D3D11CreateDevice(
            None,
            driver,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )?;
        device.ok_or_else(windows::core::Error::empty)
    }
}

/// A Direct2D device context on a new Direct3D device (single-threaded: use it on one thread).
pub(crate) fn d2d_context(device: Device) -> Result<ID2D1DeviceContext, String> {
    // SAFETY: plain Direct3D/Direct2D object creation; all pointers are valid.
    unsafe {
        let d3d = match device {
            Device::Hardware => d3d_device(D3D_DRIVER_TYPE_HARDWARE).or_else(|_| d3d_device(D3D_DRIVER_TYPE_WARP)),
            Device::Warp => d3d_device(D3D_DRIVER_TYPE_WARP),
        }
        .map_err(err("Không tạo được thiết bị đồ họa"))?;
        let dxgi: IDXGIDevice = d3d.cast().map_err(err("Không tạo được thiết bị đồ họa"))?;
        let factory: ID2D1Factory1 =
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).map_err(err("Không khởi động được Direct2D"))?;
        let d2d = factory.CreateDevice(&dxgi).map_err(err("Không khởi động được Direct2D"))?;
        d2d.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE).map_err(err("Không khởi động được Direct2D"))
    }
}

/// Bitmap properties for 32-bit BGRA bitmaps.
pub(crate) fn bgra_props(options: D2D1_BITMAP_OPTIONS, alpha: D2D1_ALPHA_MODE) -> D2D1_BITMAP_PROPERTIES1 {
    bitmap_props(options, alpha)
}

impl Compositor {
    pub fn new(out_w: u32, out_h: u32, device: Device) -> Result<Compositor, String> {
        if out_w == 0 || out_h == 0 {
            return Err(format!("Kích thước khung xuất không hợp lệ: {out_w}x{out_h}"));
        }
        // SAFETY: plain Direct2D object creation; all pointers are valid.
        unsafe {
            let ctx = d2d_context(device)?;
            let size = D2D_SIZE_U { width: out_w, height: out_h };
            let target = ctx
                .CreateBitmap(size, None, 0, &bitmap_props(D2D1_BITMAP_OPTIONS_TARGET, D2D1_ALPHA_MODE_PREMULTIPLIED))
                .map_err(err("Thiếu bộ nhớ đồ họa"))?;
            let readback = ctx
                .CreateBitmap(
                    size,
                    None,
                    0,
                    &bitmap_props(D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_ALPHA_MODE_PREMULTIPLIED),
                )
                .map_err(err("Thiếu bộ nhớ đồ họa"))?;
            let text: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).map_err(err("Không khởi động được DirectWrite"))?;
            let chip_size = (CHIP_FONT * out_h as f32).max(9.0);
            let chip_format = text
                .CreateTextFormat(
                    &HSTRING::from("Segoe UI"),
                    None,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    chip_size,
                    &HSTRING::from("vi-VN"),
                )
                .map_err(err("Không tạo được phông chữ"))?;
            Ok(Compositor {
                ctx,
                target,
                readback,
                source: None,
                out_w,
                out_h,
                out: vec![0; (out_w * out_h * 4) as usize],
                text,
                chip_format,
                chip_size,
            })
        }
    }

    /// Draws `src` (top-down BGRA, `src_w * 4` per row) over the whole output, then `draws`,
    /// and returns the output as top-down BGRA, `out_w * 4` per row.
    /// `view` is the part of the source shown (x, y, w, h in source pixels), `None` for all of it.
    pub fn render(&mut self, src: &[u8], src_w: u32, src_h: u32, view: Option<(f32, f32, f32, f32)>, draws: &[Draw]) -> Result<&[u8], String> {
        if src_w == 0 || src_h == 0 || src.len() < (src_w * src_h * 4) as usize {
            return Err("Khung hình nguồn không hợp lệ".to_string());
        }
        // SAFETY: `src` holds at least `src_h` rows of `src_w * 4` bytes (checked above); the
        // mapped read-back bitmap is only read while mapped and within its pitch and height.
        unsafe {
            let source = self.source_bitmap(src_w, src_h)?;
            source.CopyFromMemory(None, src.as_ptr() as *const _, src_w * 4).map_err(err("Không nạp được khung hình"))?;
            self.ctx.SetTarget(&self.target);
            self.ctx.BeginDraw();
            self.ctx.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }));
            let dest = D2D_RECT_F { left: 0.0, top: 0.0, right: self.out_w as f32, bottom: self.out_h as f32 };
            let view = view.unwrap_or((0.0, 0.0, src_w as f32, src_h as f32));
            let from = D2D_RECT_F { left: view.0, top: view.1, right: view.0 + view.2, bottom: view.1 + view.3 };
            self.ctx.DrawBitmap(&source, Some(&dest), 1.0, D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC, Some(&from), None);
            // EndDraw must follow BeginDraw even when an effect failed.
            let drawn = self.draw_effects(draws, view);
            self.ctx.EndDraw(None, None).map_err(err("Vẽ khung hình thất bại"))?;
            drawn?;
            self.readback.CopyFromBitmap(None, &self.target, None).map_err(err("Không đọc được khung hình"))?;
            let mapped = self.readback.Map(D2D1_MAP_OPTIONS_READ).map_err(err("Không đọc được khung hình"))?;
            let row = self.out_w as usize * 4;
            for (y, dst) in self.out.chunks_exact_mut(row).enumerate() {
                let line = mapped.bits.add(y * mapped.pitch as usize);
                std::ptr::copy_nonoverlapping(line, dst.as_mut_ptr(), row);
            }
            let _ = self.readback.Unmap();
        }
        Ok(&self.out)
    }

    unsafe fn source_bitmap(&mut self, w: u32, h: u32) -> Result<ID2D1Bitmap1, String> {
        if let Some((bmp, bw, bh)) = &self.source
            && (*bw, *bh) == (w, h)
        {
            return Ok(bmp.clone());
        }
        // SAFETY: plain bitmap creation on a valid device context.
        let bmp = unsafe {
            self.ctx
                .CreateBitmap(D2D_SIZE_U { width: w, height: h }, None, 0, &bitmap_props(D2D1_BITMAP_OPTIONS_NONE, D2D1_ALPHA_MODE_IGNORE))
                .map_err(err("Thiếu bộ nhớ đồ họa"))?
        };
        self.source = Some((bmp.clone(), w, h));
        Ok(bmp)
    }

    /// Draws the effects of one frame. Positions of rings, dots and wheel marks are in source
    /// pixels and scale with the picture; chips are placed in the output frame.
    unsafe fn draw_effects(&mut self, draws: &[Draw], view: (f32, f32, f32, f32)) -> Result<(), String> {
        let (vx, vy, vw, vh) = view;
        let (sx, sy) = (self.out_w as f32 / vw.max(1.0), self.out_h as f32 / vh.max(1.0));
        let at = |x: f32, y: f32| ((x - vx) * sx, (y - vy) * sy);
        let full_ring = RIPPLE_RADIUS * self.out_h as f32;
        let stroke = (0.12 * full_ring).max(2.0);
        // SAFETY: drawing calls on the device context between BeginDraw and EndDraw.
        unsafe {
            for d in draws {
                match d {
                    Draw::Ring { x, y, radius, alpha, button } if *alpha > 0.0 => {
                        let brush = self.brush(button_colour(*button), *alpha)?;
                        let (px, py) = at(*x, *y);
                        let e = ellipse(px, py, radius * sx);
                        self.ctx.DrawEllipse(&e, &brush, stroke, None::<&ID2D1StrokeStyle>);
                    }
                    Draw::Dot { x, y, radius, alpha, button } if *alpha > 0.0 => {
                        let brush = self.brush(button_colour(*button), *alpha)?;
                        let (px, py) = at(*x, *y);
                        self.ctx.FillEllipse(&ellipse(px, py, radius * sx), &brush);
                    }
                    Draw::Wheel { x, y, up, alpha } if *alpha > 0.0 => {
                        let (px, py) = at(*x, *y);
                        self.draw_wheel(px, py, full_ring, *up, *alpha)?
                    }
                    Draw::Chip { text, alpha, bottom } if *alpha > 0.0 => self.draw_chip(text, *alpha, *bottom)?,
                    _ => {}
                }
            }
        }
        Ok(())
    }

    unsafe fn brush(&self, (r, g, b): (f32, f32, f32), alpha: f32) -> Result<ID2D1SolidColorBrush, String> {
        // SAFETY: plain brush creation on a valid device context.
        unsafe { self.ctx.CreateSolidColorBrush(&D2D1_COLOR_F { r, g, b, a: alpha.clamp(0.0, 1.0) }, None) }
            .map_err(err("Không tạo được màu vẽ"))
    }

    /// A small mouse beside the cursor with an arrow showing the scroll direction.
    unsafe fn draw_wheel(&self, x: f32, y: f32, r: f32, up: bool, alpha: f32) -> Result<(), String> {
        let (w, h) = (0.9 * r, 1.3 * r);
        let (left, top) = (x + r, y + r);
        let line = (r * 0.08).max(1.0);
        // SAFETY: drawing calls between BeginDraw and EndDraw.
        unsafe {
            let white = self.brush((1.0, 1.0, 1.0), alpha)?;
            let black = self.brush((0.0, 0.0, 0.0), alpha * 0.8)?;
            let body = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F { left, top, right: left + w, bottom: top + h },
                radiusX: w / 2.0,
                radiusY: w / 2.0,
            };
            self.ctx.FillRoundedRectangle(&body, &white);
            self.ctx.DrawRoundedRectangle(&body, &black, line, None::<&ID2D1StrokeStyle>);
            let cx = left + w / 2.0;
            self.ctx.DrawLine(point(cx, top + h * 0.15), point(cx, top + h * 0.4), &black, line * 1.5, None::<&ID2D1StrokeStyle>);
            // Chevron above (up) or below (down) the mouse.
            let (tip, base) = if up { (top - r * 0.55, top - r * 0.2) } else { (top + h + r * 0.55, top + h + r * 0.2) };
            let half = w * 0.45;
            for side in [-half, half] {
                self.ctx.DrawLine(point(cx + side, base), point(cx, tip), &white, line * 2.5, None::<&ID2D1StrokeStyle>);
                self.ctx.DrawLine(point(cx + side, base), point(cx, tip), &black, line, None::<&ID2D1StrokeStyle>);
            }
        }
        Ok(())
    }

    /// A rounded chip with the shortcut, centred, its bottom edge `bottom` of the height up.
    unsafe fn draw_chip(&self, text: &str, alpha: f32, bottom: f32) -> Result<(), String> {
        let wide: Vec<u16> = text.encode_utf16().collect();
        // SAFETY: DirectWrite and drawing calls with valid pointers, between BeginDraw and EndDraw.
        unsafe {
            let layout = self
                .text
                .CreateTextLayout(&wide, &self.chip_format, self.out_w as f32, self.out_h as f32)
                .map_err(err("Không dựng được chữ"))?;
            let mut m = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut m).map_err(err("Không dựng được chữ"))?;
            let pad_x = self.chip_size * 0.8;
            let pad_y = self.chip_size * 0.3;
            let (w, h) = (m.width + 2.0 * pad_x, m.height + 2.0 * pad_y);
            let bottom_y = self.out_h as f32 * (1.0 - bottom);
            let left = (self.out_w as f32 - w) / 2.0;
            let rect = D2D_RECT_F { left, top: bottom_y - h, right: left + w, bottom: bottom_y };
            let bg = D2D1_ROUNDED_RECT { rect, radiusX: h / 2.0, radiusY: h / 2.0 };
            self.ctx.FillRoundedRectangle(&bg, &self.brush((20.0 / 255.0, 20.0 / 255.0, 24.0 / 255.0), 0.85 * alpha)?);
            let white = self.brush((1.0, 1.0, 1.0), alpha)?;
            self.ctx.DrawTextLayout(point(left + pad_x - m.left, rect.top + pad_y - m.top), &layout, &white, D2D1_DRAW_TEXT_OPTIONS_NONE);
        }
        Ok(())
    }
}

fn button_colour(b: Button) -> (f32, f32, f32) {
    let hex = match b {
        Button::Left => 0xFF5A36,
        Button::Right => 0x3B82F6,
        Button::Middle => 0x22C55E,
    };
    (((hex >> 16) & 0xFF) as f32 / 255.0, ((hex >> 8) & 0xFF) as f32 / 255.0, (hex & 0xFF) as f32 / 255.0)
}

fn point(x: f32, y: f32) -> Vector2 {
    Vector2 { X: x, Y: y }
}

fn ellipse(x: f32, y: f32, r: f32) -> D2D1_ELLIPSE {
    D2D1_ELLIPSE { point: point(x, y), radiusX: r, radiusY: r }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let [r, g, b] = f(x, y);
                v.extend_from_slice(&[b, g, r, 255]);
            }
        }
        v
    }

    fn rgb_at(img: &[u8], w: u32, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * w + x) * 4) as usize;
        [img[i + 2], img[i + 1], img[i]]
    }

    fn close(a: [u8; 3], b: [u8; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (*x as i32 - y as i32).abs() <= 2)
    }

    #[test]
    fn same_size_render_keeps_the_picture() {
        let src = solid(64, 48, |x, _| if x < 32 { [255, 0, 0] } else { [0, 0, 255] });
        let mut c = Compositor::new(64, 48, Device::Warp).unwrap();
        let out = c.render(&src, 64, 48, None, &[]).unwrap();
        assert_eq!(out.len(), 64 * 48 * 4);
        assert!(close(rgb_at(out, 64, 10, 24), [255, 0, 0]), "{:?}", rgb_at(out, 64, 10, 24));
        assert!(close(rgb_at(out, 64, 54, 24), [0, 0, 255]), "{:?}", rgb_at(out, 64, 54, 24));
    }

    #[test]
    fn render_scales_to_the_output_size() {
        let src = solid(64, 48, |_, _| [0, 255, 0]);
        let mut c = Compositor::new(128, 96, Device::Warp).unwrap();
        let out = c.render(&src, 64, 48, None, &[]).unwrap();
        assert_eq!(out.len(), 128 * 96 * 4);
        for (x, y) in [(0, 0), (127, 0), (64, 48), (0, 95), (127, 95)] {
            assert!(close(rgb_at(out, 128, x, y), [0, 255, 0]), "({x},{y}) {:?}", rgb_at(out, 128, x, y));
        }
    }

    #[test]
    fn view_crops_the_source() {
        let src = solid(64, 64, |_, y| if y < 32 { [255, 0, 0] } else { [0, 0, 255] });
        let mut c = Compositor::new(64, 64, Device::Warp).unwrap();
        let out = c.render(&src, 64, 64, Some((0.0, 0.0, 64.0, 32.0)), &[]).unwrap();
        for (x, y) in [(0, 0), (32, 32), (63, 60), (5, 50)] {
            assert!(close(rgb_at(out, 64, x, y), [255, 0, 0]), "({x},{y}) {:?}", rgb_at(out, 64, x, y));
        }
    }

    #[test]
    fn ring_moves_with_the_view() {
        let src = solid(200, 200, |_, _| [128, 128, 128]);
        let mut c = Compositor::new(200, 200, Device::Warp).unwrap();
        // View the top-left quarter at 2x: a ring at (50, 50) radius 20 lands at (100, 100) radius 40.
        let ring = Draw::Ring { x: 50.0, y: 50.0, radius: 20.0, alpha: 1.0, button: Button::Left };
        let out = c.render(&src, 200, 200, Some((0.0, 0.0, 100.0, 100.0)), &[ring]).unwrap();
        let [r, _, b] = rgb_at(out, 200, 140, 100);
        assert!(r > 200 && b < 120, "ring pixel {:?}", rgb_at(out, 200, 140, 100));
    }

    #[test]
    fn ring_is_drawn_in_its_colour() {
        let src = solid(200, 200, |_, _| [128, 128, 128]);
        let mut c = Compositor::new(200, 200, Device::Warp).unwrap();
        let ring = Draw::Ring { x: 100.0, y: 100.0, radius: 40.0, alpha: 1.0, button: Button::Left };
        let out = c.render(&src, 200, 200, None, &[ring]).unwrap();
        let [r, _, b] = rgb_at(out, 200, 140, 100);
        assert!(r > 200 && b < 120, "ring pixel {:?}", rgb_at(out, 200, 140, 100));
        assert!(close(rgb_at(out, 200, 100, 100), [128, 128, 128]), "centre stays grey");
    }

    #[test]
    fn ring_position_scales_with_the_output() {
        let src = solid(100, 100, |_, _| [128, 128, 128]);
        let mut c = Compositor::new(200, 200, Device::Warp).unwrap();
        let ring = Draw::Ring { x: 50.0, y: 50.0, radius: 20.0, alpha: 1.0, button: Button::Right };
        let out = c.render(&src, 100, 100, None, &[ring]).unwrap();
        let [r, _, b] = rgb_at(out, 200, 140, 100);
        assert!(b > 200 && r < 120, "right-click ring is blue at the scaled spot: {:?}", rgb_at(out, 200, 140, 100));
    }

    #[test]
    fn chip_is_drawn_at_the_bottom_centre() {
        let src = solid(400, 300, |_, _| [255, 255, 255]);
        let mut c = Compositor::new(400, 300, Device::Warp).unwrap();
        let chip = Draw::Chip { text: "Ctrl + S".into(), alpha: 1.0, bottom: 0.08 };
        let out = c.render(&src, 400, 300, None, &[chip]).unwrap();
        let dark = (262..276).any(|y| (180..220).any(|x| rgb_at(out, 400, x, y)[0] < 60));
        assert!(dark, "chip background under the bottom centre");
        assert!(close(rgb_at(out, 400, 200, 100), [255, 255, 255]), "rest untouched");
    }

    #[test]
    fn wheel_mark_and_dot_draw_something() {
        let src = solid(200, 200, |_, _| [128, 128, 128]);
        let mut c = Compositor::new(200, 200, Device::Warp).unwrap();
        let before = c.render(&src, 200, 200, None, &[]).unwrap().to_vec();
        let draws = [
            Draw::Wheel { x: 50.0, y: 50.0, up: true, alpha: 1.0 },
            Draw::Dot { x: 150.0, y: 150.0, radius: 6.0, alpha: 0.55, button: Button::Middle },
        ];
        let out = c.render(&src, 200, 200, None, &draws).unwrap();
        assert_ne!(before, out);
        assert!(rgb_at(out, 200, 150, 150)[1] > 140, "middle-button dot is green: {:?}", rgb_at(out, 200, 150, 150));
    }

    #[test]
    fn alpha_zero_draws_nothing() {
        let src = solid(200, 200, |_, _| [128, 128, 128]);
        let mut c = Compositor::new(200, 200, Device::Warp).unwrap();
        let before = c.render(&src, 200, 200, None, &[]).unwrap().to_vec();
        let draws = [
            Draw::Ring { x: 100.0, y: 100.0, radius: 40.0, alpha: 0.0, button: Button::Left },
            Draw::Chip { text: "Ctrl + S".into(), alpha: 0.0, bottom: 0.08 },
        ];
        assert_eq!(before, c.render(&src, 200, 200, None, &draws).unwrap());
    }

    #[test]
    fn source_size_may_change_between_frames() {
        let mut c = Compositor::new(64, 48, Device::Warp).unwrap();
        c.render(&solid(32, 24, |_, _| [255, 0, 0]), 32, 24, None, &[]).unwrap();
        let out = c.render(&solid(64, 48, |_, _| [0, 0, 255]), 64, 48, None, &[]).unwrap();
        assert!(close(rgb_at(out, 64, 32, 24), [0, 0, 255]));
    }
}
