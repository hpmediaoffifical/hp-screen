//! Reading text in a picture with Windows' own OCR (`Windows.Media.Ocr`): offline, nothing to
//! install beyond Windows language packs. Vietnamese is preferred, then English, then the
//! user's own languages.
use hp_core::snip::{ocr_scale, pick_ocr_language};
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
    let lines: Vec<String> = result
        .Lines()
        .map_err(ocr_error)?
        .into_iter()
        .filter_map(|line| line.Text().ok().map(|t| t.to_string()))
        .collect();

    let tag = &tags[index];
    let display = language.DisplayName().map(|n| n.to_string()).unwrap_or_else(|_| tag.clone());
    let name = language_name(tag, &display);
    let vietnamese = tag.to_ascii_lowercase().starts_with("vi");
    let note = (!vietnamese).then(|| {
        format!("Máy chưa có bộ nhận chữ tiếng Việt nên HP Screen đọc bằng {name}; chữ có dấu có thể bị sai. {HOW_TO_ADD_VIETNAMESE}")
    });
    Ok(OcrOutcome { text: lines.join("\n"), language: name, vietnamese, note })
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
        assert!(!outcome.text.trim().is_empty());
    }
}
