/*! © 2026 HP Media (hpvn.media) — HP Screen. Bảo lưu mọi quyền. HP không có đại lý khác, không hợp tác với đơn vị nào, mọi sao chép đều là vi phạm. */
//! Reading Vietnamese text with Tesseract, built into the program.
//!
//! Windows' own OCR has no Vietnamese on most machines and then reads accented letters wrong
//! ("được" comes out as "duøc"). Tesseract with its Vietnamese data reads them right, works
//! offline and needs nothing installed: the library is linked statically (see `build.rs`) and
//! the language data is part of the program file.
//!
//! Tesseract and its Vietnamese data are the work of others (Apache License 2.0); see
//! `third-party/` for their notices. This file is only HP Screen's own calls into it.
use hp_core::snip::{TsvWord, tsv_words};
use image::{GrayImage, RgbaImage};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::Mutex;

/// `vie.traineddata` of tessdata_fast: the small, quick model. Measured on screen text it reads
/// as well as the large one (99 % of characters).
static VIETNAMESE: &[u8] = include_bytes!("../third-party/tessdata/vie.traineddata");

/// Largest side of the picture handed to Tesseract.
const MAX_SIDE: f64 = 8000.0;
/// Small pictures are enlarged up to this much: screen text is small for an engine made for
/// scanned pages, and 1.5× reads it better (measured: 99.3 % of characters against 97.9 %).
const MAX_ENLARGE: f64 = 1.5;
/// ...but only while the enlarged picture stays under this many pixels: reading time grows
/// faster than the picture, and a whole screen is big enough as it is.
const ENLARGE_BUDGET: f64 = 3_000_000.0;
/// "Assume a single uniform block of text": lines run across the whole picture, which keeps the
/// cells of a table row together on one line.
const PSM_SINGLE_BLOCK: c_int = 6;
/// The neural-network recogniser, the only one the fast data holds.
const OEM_LSTM_ONLY: c_int = 1;

#[repr(C)]
struct TessBaseApi {
    _opaque: [u8; 0],
}

// The few functions of Tesseract's C interface (`tesseract/capi.h`) that are used here.
unsafe extern "C" {
    fn TessBaseAPICreate() -> *mut TessBaseApi;
    fn TessBaseAPIDelete(handle: *mut TessBaseApi);
    #[allow(clippy::too_many_arguments)]
    fn TessBaseAPIInit5(
        handle: *mut TessBaseApi,
        data: *const c_char,
        data_size: c_int,
        language: *const c_char,
        mode: c_int,
        configs: *mut *mut c_char,
        configs_size: c_int,
        vars_vec: *mut *mut c_char,
        vars_values: *mut *mut c_char,
        vars_vec_size: usize,
        set_only_non_debug_params: c_int,
    ) -> c_int;
    fn TessBaseAPISetPageSegMode(handle: *mut TessBaseApi, mode: c_int);
    fn TessBaseAPISetImage(handle: *mut TessBaseApi, data: *const u8, width: c_int, height: c_int, bytes_per_pixel: c_int, bytes_per_line: c_int);
    fn TessBaseAPISetSourceResolution(handle: *mut TessBaseApi, ppi: c_int);
    fn TessBaseAPIRecognize(handle: *mut TessBaseApi, monitor: *mut c_void) -> c_int;
    fn TessBaseAPIGetTsvText(handle: *mut TessBaseApi, page_number: c_int) -> *mut c_char;
    fn TessBaseAPIClear(handle: *mut TessBaseApi);
    fn TessDeleteText(text: *const c_char);
}

/// A loaded engine. Loading takes a moment, so one is kept for the life of the program.
struct Engine(*mut TessBaseApi);
// SAFETY: the engine is only ever used under the `ENGINE` lock, by one thread at a time.
unsafe impl Send for Engine {}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: the handle came from TessBaseAPICreate and is not used after this.
        unsafe { TessBaseAPIDelete(self.0) };
    }
}

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

fn load() -> Result<Engine, String> {
    // SAFETY: plain calls into the library; the data pointer and length describe a static
    // slice, the language is a NUL-terminated literal and the optional lists are empty.
    unsafe {
        let handle = TessBaseAPICreate();
        if handle.is_null() {
            return Err("không tạo được bộ nhận chữ".to_string());
        }
        let engine = Engine(handle);
        let failed = TessBaseAPIInit5(
            handle,
            VIETNAMESE.as_ptr().cast(),
            VIETNAMESE.len() as c_int,
            c"vie".as_ptr(),
            OEM_LSTM_ONLY,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            0,
        );
        if failed != 0 {
            return Err("không nạp được dữ liệu tiếng Việt".to_string());
        }
        TessBaseAPISetPageSegMode(handle, PSM_SINGLE_BLOCK);
        Ok(engine)
    }
}

/// The picture as Tesseract reads it best: grey, enlarged when small (screen text is tiny for
/// an engine made for scanned pages) and dark-on-light. Returns it with the enlargement used.
/// How much to enlarge a `width` × `height` picture before reading it (below 1 shrinks it).
fn scale_for(width: u32, height: u32) -> f64 {
    let (w, h) = (width.max(1) as f64, height.max(1) as f64);
    let enlarge = (ENLARGE_BUDGET / (w * h)).sqrt().clamp(1.0, MAX_ENLARGE);
    enlarge.min(MAX_SIDE / w.max(h))
}

fn prepare(picture: &RgbaImage) -> (GrayImage, f64) {
    let scale = scale_for(picture.width(), picture.height());
    let mut grey = image::DynamicImage::ImageRgba8(picture.clone()).to_luma8();
    let (w, h) = (((grey.width() as f64 * scale).round() as u32).max(1), ((grey.height() as f64 * scale).round() as u32).max(1));
    if (w, h) != grey.dimensions() {
        grey = image::imageops::resize(&grey, w, h, image::imageops::FilterType::CatmullRom);
    }
    // A dark theme: light text on a dark ground. Tesseract expects the opposite.
    let sum: u64 = grey.as_raw().iter().map(|&v| v as u64).sum();
    if sum < 128 * grey.as_raw().len() as u64 {
        for v in grey.iter_mut() {
            *v = 255 - *v;
        }
    }
    (grey, scale)
}

/// Reads the words in `picture`, in reading order, with their boxes in its own pixels.
/// Blocking, and only one reading runs at a time.
pub fn words(picture: &RgbaImage) -> Result<Vec<TsvWord>, String> {
    let (grey, scale) = prepare(picture);
    let (w, h) = grey.dimensions();
    let mut guard = ENGINE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(load()?);
    }
    let handle = guard.as_ref().map(|e| e.0).ok_or("không có bộ nhận chữ")?;
    // SAFETY: `grey` holds w × h bytes, one per pixel, and outlives the calls that read it
    // (Clear drops Tesseract's reference before `grey` goes away). The text returned is copied
    // and then released with the library's own function.
    let tsv = unsafe {
        TessBaseAPISetImage(handle, grey.as_raw().as_ptr(), w as c_int, h as c_int, 1, w as c_int);
        // Screen pictures carry no resolution; without one Tesseract guesses and warns.
        TessBaseAPISetSourceResolution(handle, (96.0 * scale).round().max(70.0) as c_int);
        let failed = TessBaseAPIRecognize(handle, std::ptr::null_mut()) != 0;
        let text = if failed { std::ptr::null_mut() } else { TessBaseAPIGetTsvText(handle, 0) };
        let copied = (!text.is_null()).then(|| CStr::from_ptr(text).to_string_lossy().into_owned());
        if !text.is_null() {
            TessDeleteText(text);
        }
        TessBaseAPIClear(handle);
        copied
    };
    let tsv = tsv.ok_or("không đọc được ảnh")?;
    // Boxes come back in the enlarged picture's pixels.
    Ok(tsv_words(&tsv)
        .into_iter()
        .map(|word| TsvWord { x: word.x / scale, y: word.y / scale, w: word.w / scale, h: word.h / scale, ..word })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hp_core::snip::tsv_text;

    #[test]
    fn small_pictures_are_enlarged_and_big_ones_are_not() {
        assert_eq!(scale_for(600, 200), MAX_ENLARGE);
        assert_eq!(scale_for(2560, 1600), 1.0);
        let s = scale_for(1600, 1200);
        assert!(s > 1.0 && s < MAX_ENLARGE && (1600.0 * s) * (1200.0 * s) <= ENLARGE_BUDGET + 1.0);
        assert!((scale_for(20_000, 100) * 20_000.0 - MAX_SIDE).abs() < 1e-6);
    }

    /// White letters "II" as two bars on black: after `prepare` the ground is light.
    #[test]
    fn dark_pictures_are_turned_dark_on_light() {
        let mut picture = RgbaImage::from_pixel(200, 100, image::Rgba([10, 10, 10, 255]));
        for y in 30..70 {
            for x in 90..96 {
                picture.put_pixel(x, y, image::Rgba([240, 240, 240, 255]));
            }
        }
        let (grey, _) = prepare(&picture);
        assert!(grey.get_pixel(0, 0).0[0] > 200);
        let light = RgbaImage::from_pixel(200, 100, image::Rgba([250, 250, 250, 255]));
        assert!(prepare(&light).0.get_pixel(0, 0).0[0] > 200);
    }

    /// The engine loads from the data inside the program and reads an empty picture as no text.
    #[test]
    fn the_built_in_engine_loads_and_reads_nothing_from_a_blank_picture() {
        let blank = RgbaImage::from_pixel(320, 120, image::Rgba([255, 255, 255, 255]));
        assert_eq!(tsv_text(&words(&blank).expect("the engine works")), "");
        // And again: the engine is reused.
        assert!(words(&blank).expect("the engine works").is_empty());
    }

    /// Reads a picture of Vietnamese text: `HP_OCR_SAMPLE=path.png cargo test -p hp-screen --lib -- --ignored tess_reads`
    #[test]
    #[ignore = "needs a sample picture"]
    fn tess_reads_a_sample_file() {
        let Some(path) = std::env::var_os("HP_OCR_SAMPLE") else { return };
        let picture = image::open(path).expect("sample").to_rgba8();
        let started = std::time::Instant::now();
        let found = words(&picture).expect("ocr");
        println!("{} ms\n{}", started.elapsed().as_millis(), tsv_text(&found));
        assert!(!found.is_empty());
    }
}
