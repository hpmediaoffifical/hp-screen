//! The Windows cursor's current shape, as an RGBA image with its hot spot, for recordings
//! that draw a smooth cursor instead of the Windows one.
use std::collections::HashMap;

/// A cursor image: RGBA (straight alpha), size, hot spot.
pub struct CursorImage {
    pub rgba: Vec<u8>,
    pub w: u32,
    pub h: u32,
    pub hx: i32,
    pub hy: i32,
}

/// The cursor handle Windows shows now, or `None` while it is hidden.
pub fn current() -> Option<isize> {
    use windows::Win32::UI::WindowsAndMessaging::{CURSOR_SHOWING, CURSORINFO, GetCursorInfo};
    let mut ci = CURSORINFO { cbSize: std::mem::size_of::<CURSORINFO>() as u32, ..Default::default() };
    // SAFETY: `ci` is a CURSORINFO with its size set.
    unsafe { GetCursorInfo(&mut ci) }.ok()?;
    (ci.flags.0 & CURSOR_SHOWING.0 != 0 && !ci.hCursor.is_invalid()).then_some(ci.hCursor.0 as isize)
}

/// The image of cursor `handle`. Drawn once on black and once on white: the difference gives
/// the transparency, which also works for black-and-white cursors (the text I-beam).
pub fn image(handle: isize) -> Option<CursorImage> {
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::{DI_NORMAL, DrawIconEx, GetIconInfo, HICON, ICONINFO};
    // SAFETY: GDI calls on objects created and released here; the DIB pixels are read only
    // while the bitmap is alive.
    unsafe {
        let hicon = HICON(handle as *mut _);
        let mut info = ICONINFO::default();
        GetIconInfo(hicon, &mut info).ok()?;
        let mut bm = BITMAP::default();
        let probe = if !info.hbmColor.is_invalid() { info.hbmColor } else { info.hbmMask };
        GetObjectW(probe.into(), std::mem::size_of::<BITMAP>() as i32, Some(&mut bm as *mut BITMAP as *mut _));
        let (w, mut h) = (bm.bmWidth.max(1) as u32, bm.bmHeight.max(1) as u32);
        if info.hbmColor.is_invalid() {
            h /= 2; // a monochrome cursor's mask holds the AND and XOR halves
        }
        for b in [info.hbmColor, info.hbmMask] {
            if !b.is_invalid() {
                let _ = DeleteObject(b.into());
            }
        }
        let (hx, hy) = (info.xHotspot as i32, info.yHotspot as i32);
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let draw_on = |bg: u8| -> Option<Vec<u8>> {
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w as i32,
                    biHeight: -(h as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let dib = CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
            let old = SelectObject(dc, dib.into());
            let px = std::slice::from_raw_parts_mut(bits as *mut u8, (w * h * 4) as usize);
            px.fill(bg);
            let drawn = DrawIconEx(dc, 0, 0, hicon, w as i32, h as i32, 0, None, DI_NORMAL).is_ok();
            let _ = GdiFlush();
            let copy = px.to_vec();
            SelectObject(dc, old);
            let _ = DeleteObject(dib.into());
            drawn.then_some(copy)
        };
        let (black, white) = (draw_on(0), draw_on(255));
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
        let (black, white) = (black?, white?);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        // Inverting pixels (black-and-white cursors such as the cross): bright over black, dark
        // over white. They are drawn black with a white edge, visible on any background.
        let inverting: Vec<bool> = (0..(w * h) as usize).map(|i| (0..3).any(|k| black[i * 4 + k] > white[i * 4 + k])).collect();
        for i in 0..(w * h) as usize {
            let (b, wh) = (&black[i * 4..i * 4 + 3], &white[i * 4..i * 4 + 3]);
            if inverting[i] {
                rgba[i * 4 + 3] = 255;
                continue;
            }
            // Over black a pixel shows c·a, over white c·a + (1 − a): the gap is 1 − a.
            let a = 255 - (0..3).map(|k| wh[k].saturating_sub(b[k])).max().unwrap_or(0);
            let o = i * 4;
            if a > 0 {
                for k in 0..3 {
                    rgba[o + 2 - k] = ((b[k] as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
                }
            }
            rgba[o + 3] = a;
        }
        let (wi, hi) = (w as i64, h as i64);
        for y in 0..hi {
            for x in 0..wi {
                let i = (y * wi + x) as usize;
                let next_to_inverting = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx >= 0 && ny >= 0 && nx < wi && ny < hi && inverting[(ny * wi + nx) as usize]
                });
                if !inverting[i] && rgba[i * 4 + 3] == 0 && next_to_inverting {
                    rgba[i * 4..i * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
        Some(CursorImage { rgba, w, h, hx, hy })
    }
}

/// The image as base64 PNG.
pub fn png_base64(img: &CursorImage) -> Option<String> {
    let buf = image::RgbaImage::from_raw(img.w, img.h, img.rgba.clone())?;
    let mut png = std::io::Cursor::new(Vec::new());
    buf.write_to(&mut png, image::ImageFormat::Png).ok()?;
    Some(hp_core::cursor::base64_encode(png.get_ref()))
}

/// Follows the cursor's shape during a recording: each new shape gets an id (and is logged
/// once), each change is logged.
#[derive(Default)]
pub struct ShapeTracker {
    ids: HashMap<isize, u32>,
    last: Option<u32>,
}

/// What the tracker saw: a shape to log first (id, image), then the current id (0: hidden).
pub struct Seen {
    pub new_shape: Option<(u32, CursorImage)>,
    pub id: u32,
}

impl ShapeTracker {
    /// Looks at the cursor now; `None` when nothing changed.
    pub fn poll(&mut self) -> Option<Seen> {
        self.see(current())
    }

    fn see(&mut self, handle: Option<isize>) -> Option<Seen> {
        let (id, new_shape) = match handle {
            None => (0, None),
            Some(h) => match self.ids.get(&h) {
                Some(&id) => (id, None),
                None => {
                    // A shape that cannot be read keeps an id (drawn as the plain arrow) and is
                    // not read again.
                    let id = self.ids.len() as u32 + 1;
                    self.ids.insert(h, id);
                    (id, image(h).map(|img| (id, img)))
                }
            },
        };
        if self.last == Some(id) {
            return None;
        }
        self.last = Some(id);
        Some(Seen { new_shape, id })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::{IDC_ARROW, IDC_IBEAM, LoadCursorW};

    fn system(id: windows::core::PCWSTR) -> isize {
        // SAFETY: loading a shared system cursor.
        unsafe { LoadCursorW(None, id) }.unwrap().0 as isize
    }

    #[test]
    fn the_arrow_has_a_transparent_background_and_a_solid_tip() {
        let img = image(system(IDC_ARROW)).expect("arrow image");
        assert!(img.w >= 16 && img.h >= 16);
        let alpha = |x: u32, y: u32| img.rgba[((y * img.w + x) * 4 + 3) as usize];
        assert_eq!(alpha(img.w - 1, img.h - 1), 0, "corner far from the arrow is transparent");
        assert!(img.rgba.chunks(4).any(|p| p[3] == 255), "the arrow itself is solid");
        assert!(png_base64(&img).unwrap().len() > 20);
    }

    #[test]
    fn the_text_cursor_is_read_too() {
        let img = image(system(IDC_IBEAM)).expect("I-beam image");
        assert!(img.rgba.chunks(4).any(|p| p[3] > 200), "{}x{}", img.w, img.h);
    }

    #[test]
    fn inverting_cursors_are_drawn_dark_with_a_light_edge() {
        use windows::Win32::UI::WindowsAndMessaging::IDC_CROSS;
        let img = image(system(IDC_CROSS)).expect("cross image");
        let solid: Vec<&[u8]> = img.rgba.chunks(4).filter(|p| p[3] == 255).collect();
        assert!(solid.iter().any(|p| p[0] < 64), "visible on white pages");
        assert!(solid.iter().any(|p| p[0] > 192), "and on dark ones");
    }

    #[test]
    fn a_shape_that_cannot_be_read_still_gets_an_id_once() {
        let mut t = ShapeTracker::default();
        let bad = t.see(Some(12345)).expect("reported");
        assert!(bad.id != 0 && bad.new_shape.is_none(), "drawn as the plain arrow");
        assert!(t.see(Some(12345)).is_none(), "not retried");
    }

    #[test]
    fn shapes_get_ids_once_and_changes_are_reported() {
        let mut t = ShapeTracker::default();
        let (arrow, beam) = (system(IDC_ARROW), system(IDC_IBEAM));
        let a = t.see(Some(arrow)).unwrap();
        assert_eq!((a.id, a.new_shape.is_some()), (1, true));
        assert!(t.see(Some(arrow)).is_none(), "no change");
        assert_eq!(t.see(Some(beam)).unwrap().id, 2);
        let back = t.see(Some(arrow)).unwrap();
        assert_eq!((back.id, back.new_shape.is_some()), (1, false), "known shape: not logged again");
        assert_eq!(t.see(None).unwrap().id, 0, "hidden");
    }
}
