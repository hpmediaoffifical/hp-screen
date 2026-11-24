/*! © 2026 HP Media (hpvn.media) — HP Screen. Bảo lưu mọi quyền. HP không có đại lý khác, không hợp tác với đơn vị nào, mọi sao chép đều là vi phạm. */
//! Reading text in a picture, offline. The built-in Vietnamese engine (`tess`) does the work;
//! Windows' own OCR (`Windows.Media.Ocr`) stands in if that engine cannot run. Windows' OCR
//! alone was not enough: most machines have no Vietnamese for it, and it then reads accented
//! letters wrong.
use hp_core::snip::{OcrBox, TableText, ocr_scale, pick_ocr_language, table_text, tsv_text};
use image::RgbaImage;
use serde::Serialize;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};

#[derive(Clone, Debug, Serialize)]
pub struct OcrOutcome {
    /// One line of text per line found, top to bottom.
    pub text: String,
    /// Name of the language it was read in, in Vietnamese ("tiếng Việt", "tiếng Anh"…).
    pub language: String,
    pub vietnamese: bool,
    /// Shown above the text when it was not read in Vietnamese: why, and how to fix it.
    pub note: Option<String>,
    /// Every word read, with its box in the *input* picture's pixels (unscaled). Words follow
    /// line order, top to bottom, left to right within a line.
    pub words: Vec<OcrWord>,
    /// The same text with its columns kept (tabs between cells), when it stood in columns.
    pub table: Option<TableText>,
}

/// One word and where it sits in the picture.
#[derive(Clone, Debug, Serialize)]
pub struct OcrWord {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

const HOW_TO_ADD_VIETNAMESE: &str = "Cách thêm: mở Cài đặt Windows → Thời gian và ngôn ngữ → Ngôn ngữ và vùng → Thêm ngôn ngữ → Tiếng Việt, \
     để Windows tải các gói đi kèm, rồi mở lại công cụ chụp. Một số bản Windows không có bộ nhận chữ tiếng Việt; khi đó HP Screen \
     vẫn đọc được chữ nhưng có thể mất dấu.";

/// Vietnamese name of a language, from its tag ("en-US" → "tiếng Anh").
fn language_name(tag: &str, display: &str) -> String {
    let name = match tag.split('-').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "vi" => "tiếng Việt",
        "en" => "tiếng Anh",
        "fr" => "tiếng Pháp",
        "de" => "tiếng Đức",
        "es" => "tiếng Tây Ban Nha",
        "ja" => "tiếng Nhật",
        "ko" => "tiếng Hàn",
        "zh" => "tiếng Trung",
        "ru" => "tiếng Nga",
        _ => return display.to_string(),
    };
    name.to_string()
}

fn ocr_error(e: windows::core::Error) -> String {
    format!("Không nhận được chữ ({})", e.message())
}

/// Reads the text in `picture`. Blocking: call it off the main thread.
pub fn recognize(picture: &RgbaImage) -> Result<OcrOutcome, String> {
    match crate::tess::words(picture) {
        Ok(found) => {
            let words: Vec<OcrWord> =
                found.iter().map(|w| OcrWord { text: w.text.clone(), x: w.x, y: w.y, w: w.w, h: w.h }).collect();
            let boxes: Vec<OcrBox> = words.iter().map(|w| OcrBox { text: &w.text, x: w.x, y: w.y, w: w.w, h: w.h }).collect();
            let table = table_text(&boxes);
            Ok(OcrOutcome { text: tsv_text(&found), language: "tiếng Việt".to_string(), vietnamese: true, note: None, words, table })
        }
        // The built-in engine could not run: Windows' own is better than nothing.
        Err(reason) => recognize_with_windows(picture).map_err(|e| format!("{e} (bộ nhận chữ tiếng Việt: {reason})")),
    }
}

/// Reads the text in `picture` with Windows' own OCR. Blocking.
fn recognize_with_windows(picture: &RgbaImage) -> Result<OcrOutcome, String> {
    // WinRT needs the calling thread in an apartment; "already initialised" is fine too.
    // SAFETY: plain COM initialisation of the current thread.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };

    let languages: Vec<Language> = OcrEngine::AvailableRecognizerLanguages().map_err(ocr_error)?.into_iter().collect();
    let tags: Vec<String> =
        languages.iter().map(|l| l.LanguageTag().map(|t| t.to_string()).unwrap_or_default()).collect();
    let user: Vec<String> = OcrEngine::TryCreateFromUserProfileLanguages()
        .and_then(|e| e.RecognizerLanguage())
        .and_then(|l| l.LanguageTag())
        .map(|t| vec![t.to_string()])
        .unwrap_or_default();
    let index = pick_ocr_language(&tags, &user).ok_or(
        "Windows trên máy này chưa có ngôn ngữ nhận chữ nào. Hãy thêm một ngôn ngữ trong Cài đặt → Thời gian và ngôn ngữ → Ngôn ngữ và vùng.",
    )?;
    let language = &languages[index];
    let engine = OcrEngine::TryCreateFromLanguage(language).map_err(ocr_error)?;

    // Screen text is small: the engine reads it better enlarged (within its size limit).
    let max = OcrEngine::MaxImageDimension().unwrap_or(2600);
    let scale = ocr_scale(picture.width(), picture.height(), max);
    let (w, h) = (
        ((picture.width() as f64 * scale).round() as u32).max(1),
        ((picture.height() as f64 * scale).round() as u32).max(1),
    );
    let scaled = if (w, h) == picture.dimensions() {
        picture.clone()
    } else {
        image::imageops::resize(picture, w, h, image::imageops::FilterType::CatmullRom)
    };
    let mut bgra = scaled.into_raw();
    for px in bgra.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    let writer = DataWriter::new().map_err(ocr_error)?;
    writer.WriteBytes(&bgra).map_err(ocr_error)?;
    let buffer = writer.DetachBuffer().map_err(ocr_error)?;
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, w as i32, h as i32).map_err(ocr_error)?;
    let result = engine.RecognizeAsync(&bitmap).map_err(ocr_error)?.join().map_err(ocr_error)?;
    let mut lines: Vec<String> = Vec::new();
    let mut words: Vec<OcrWord> = Vec::new();
    for line in result.Lines().map_err(ocr_error)?.into_iter() {
        match line.Text() {
            Ok(t) => lines.push(t.to_string()),
            Err(_) => lines.push(String::new()),
        }
        // The boxes come back in the *scaled* bitmap's pixels; undo the enlargement so they
        // point into the picture the caller handed in.
        for word in line.Words().map_err(ocr_error)?.into_iter() {
            let (text, r) = (word.Text().unwrap_or_default(), word.BoundingRect().unwrap_or_default());
            words.push(OcrWord {
                text: text.to_string(),
                x: r.X as f64 / scale,
                y: r.Y as f64 / scale,
                w: r.Width as f64 / scale,
                h: r.Height as f64 / scale,
            });
        }
    }

    let tag = &tags[index];
    let display = language.DisplayName().map(|n| n.to_string()).unwrap_or_else(|_| tag.clone());
    let name = language_name(tag, &display);
    let vietnamese = tag.to_ascii_lowercase().starts_with("vi");
    let note = (!vietnamese).then(|| {
        format!("Máy chưa có bộ nhận chữ tiếng Việt nên HP Screen đọc bằng {name}; chữ có dấu có thể bị sai. {HOW_TO_ADD_VIETNAMESE}")
    });
    let boxes: Vec<OcrBox> = words.iter().map(|w| OcrBox { text: &w.text, x: w.x, y: w.y, w: w.w, h: w.h }).collect();
    let table = table_text(&boxes);
    Ok(OcrOutcome { text: lines.join("\n"), language: name, vietnamese, note, words, table })
}

/// Puts text on the clipboard, retrying while another program holds it.
pub fn copy_text(text: &str) -> Result<(), String> {
    let mut last_error = String::new();
    for attempt in 0..20 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        match arboard::Clipboard::new().and_then(|mut c| c.set_text(text.to_string())) {
            Ok(()) => return Ok(()),
            Err(e) => last_error = e.to_string(),
        }
    }
    Err(format!("Không sao chép được chữ vào clipboard ({last_error}). Hãy thử lại."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_are_named_in_vietnamese() {
        assert_eq!(language_name("vi-VN", "Vietnamese"), "tiếng Việt");
        assert_eq!(language_name("en-US", "English (United States)"), "tiếng Anh");
        assert_eq!(language_name("pl-PL", "Polski"), "Polski");
    }
}

#[cfg(test)]
mod desktop_tests {
    //! Uses the real Windows OCR engine on a picture of text you provide.
    use super::*;

    /// Reads a picture of real text: `HP_OCR_SAMPLE=path.png cargo test -p hp-screen --lib -- --ignored ocr_reads_a_sample`
    #[test]
    #[ignore = "uses the Windows OCR engine and a sample picture"]
    fn ocr_reads_a_sample_file() {
        let Some(path) = std::env::var_os("HP_OCR_SAMPLE") else { return };
        let picture = image::open(path).expect("sample").to_rgba8();
        let outcome = recognize(&picture).expect("ocr");
        println!("{outcome:?}");
        println!("--- Windows' own:\n{:?}", recognize_with_windows(&picture).map(|o| o.text));
        assert!(!outcome.text.trim().is_empty());
    }
}
