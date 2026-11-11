//! Builds export frames with Direct2D: the source picture scaled into the output, then the
//! effects drawn over it, read back as top-down BGRA for the encoder.
use hp_core::effects::Draw;
use std::mem::ManuallyDrop;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::core::Interface;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Device {
    /// The GPU, falling back to WARP when no hardware device can be made.
    Hardware,
    /// Software rendering (tests).
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
}

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

impl Compositor {
    pub fn new(out_w: u32, out_h: u32, device: Device) -> Result<Compositor, String> {
        if out_w == 0 || out_h == 0 {
            return Err(format!("Kích thước khung xuất không hợp lệ: {out_w}x{out_h}"));
        }
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
            let ctx = d2d.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE).map_err(err("Không khởi động được Direct2D"))?;
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
            Ok(Compositor { ctx, target, readback, source: None, out_w, out_h, out: vec![0; (out_w * out_h * 4) as usize] })
        }
    }

    /// Draws `src` (top-down BGRA, `src_w * 4` per row) over the whole output, then `draws`,
    /// and returns the output as top-down BGRA, `out_w * 4` per row.
    pub fn render(&mut self, src: &[u8], src_w: u32, src_h: u32, draws: &[Draw]) -> Result<&[u8], String> {
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
            self.ctx.DrawBitmap(&source, Some(&dest), 1.0, D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC, None, None);
            self.draw_effects(draws, src_w, src_h)?;
            self.ctx.EndDraw(None, None).map_err(err("Vẽ khung hình thất bại"))?;
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

    unsafe fn draw_effects(&mut self, draws: &[Draw], _src_w: u32, _src_h: u32) -> Result<(), String> {
        match draws.first() {
            None => Ok(()),
            Some(d) => match *d {},
        }
    }
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
        let out = c.render(&src, 64, 48, &[]).unwrap();
        assert_eq!(out.len(), 64 * 48 * 4);
        assert!(close(rgb_at(out, 64, 10, 24), [255, 0, 0]), "{:?}", rgb_at(out, 64, 10, 24));
        assert!(close(rgb_at(out, 64, 54, 24), [0, 0, 255]), "{:?}", rgb_at(out, 64, 54, 24));
    }

    #[test]
    fn render_scales_to_the_output_size() {
        let src = solid(64, 48, |_, _| [0, 255, 0]);
        let mut c = Compositor::new(128, 96, Device::Warp).unwrap();
        let out = c.render(&src, 64, 48, &[]).unwrap();
        assert_eq!(out.len(), 128 * 96 * 4);
        for (x, y) in [(0, 0), (127, 0), (64, 48), (0, 95), (127, 95)] {
            assert!(close(rgb_at(out, 128, x, y), [0, 255, 0]), "({x},{y}) {:?}", rgb_at(out, 128, x, y));
        }
    }

    #[test]
    fn source_size_may_change_between_frames() {
        let mut c = Compositor::new(64, 48, Device::Warp).unwrap();
        c.render(&solid(32, 24, |_, _| [255, 0, 0]), 32, 24, &[]).unwrap();
        let out = c.render(&solid(64, 48, |_, _| [0, 0, 255]), 64, 48, &[]).unwrap();
        assert!(close(rgb_at(out, 64, 32, 24), [0, 0, 255]));
    }
}
