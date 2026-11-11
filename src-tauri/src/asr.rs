//! Offline speech recognition for subtitles: the whisper.cpp command-line program and a model,
//! downloaded once with the user's consent, checked against known SHA-256 sums, then run as a
//! separate process on a 16 kHz mono WAV made from the video's sound.
use crate::decoder::AudioDecoder;
use crate::encoder::AUDIO_SAMPLE_RATE;
use hp_core::mix::Resampler;
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// One file to download.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Download {
    pub url: &'static str,
    pub file: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Model {
    pub id: &'static str,
    pub name: &'static str,
    pub download: Download,
}

/// whisper.cpp for Windows x64 (CPU), MIT licence. Checked 2026-11-12 on the GitHub API.
pub const CLI: Download = Download {
    url: "https://github.com/ggml-org/whisper.cpp/releases/download/b5454/whisper-bin-x64.zip",
    file: "whisper-bin-x64.zip",
    bytes: 8_928_640,
    sha256: "6ba69e3482d7826214f90a6a9c84ca07782aec1e1d0c6a7c30c994fd5d816ccb",
};

/// Models (MIT licence, from huggingface.co/ggerganov/whisper.cpp). The first is the default.
pub const MODELS: [Model; 2] = [
    Model {
        id: "large-v3-turbo-q5",
        name: "Chính xác (Whisper large-v3-turbo)",
        download: Download {
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
            file: "ggml-large-v3-turbo-q5_0.bin",
            bytes: 574_041_195,
            sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
        },
    },
    Model {
        id: "base",
        name: "Nhẹ (Whisper base)",
        download: Download {
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
            file: "ggml-base.bin",
            bytes: 147_951_465,
            sha256: "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
        },
    },
];

pub fn model(id: &str) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.id == id)
}

/// `%LOCALAPPDATA%\HP Screen\asr`.
pub fn asr_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("HP Screen").join("asr")
}

/// The whisper command-line program inside `dir`, wherever the zip put it.
pub fn find_cli(dir: &Path) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        let mut subdirs = Vec::new();
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                subdirs.push(p);
            } else if p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("whisper-cli.exe")) {
                return Some(p);
            }
        }
        subdirs.sort();
        stack.extend(subdirs.into_iter().rev());
    }
    None
}

pub fn model_path(dir: &Path, m: &Model) -> PathBuf {
    dir.join(m.download.file)
}

/// SHA-256 of a file, lower-case hex.
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("Không đọc được {}: {e}", path.display()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Unpacks `zip` into `dir` (entries that would land outside `dir` are skipped).
pub fn extract_zip(zip: &Path, dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip).map_err(|e| format!("Không mở được file nén: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("File nén hỏng: {e}"))?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("File nén hỏng: {e}"))?;
        // enclosed_name refuses absolute paths and "..": nothing lands outside `dir`.
        let Some(rel) = entry.enclosed_name() else { continue };
        let target = dir.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&target).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Writes the sound of `video` as 16 kHz mono 16-bit WAV; `Ok(false)` when it has no sound.
pub fn write_wav_16k_mono(video: &Path, wav: &Path, cancel: &AtomicBool) -> Result<bool, String> {
    const RATE: u32 = 16_000;
    let Some(mut dec) = AudioDecoder::open(video)? else { return Ok(false) };
    let mut rs = Resampler::new(AUDIO_SAMPLE_RATE, RATE);
    let mut samples: Vec<i16> = Vec::new();
    while let Some((_, pcm)) = dec.next_chunk()? {
        if cancel.load(Ordering::Relaxed) {
            return Err("Đã hủy".to_string());
        }
        for frame in rs.process(&pcm).chunks_exact(2) {
            let mono = ((frame[0] + frame[1]) * 0.5).clamp(-1.0, 1.0);
            samples.push((mono * i16::MAX as f32) as i16);
        }
    }
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(wav, out).map_err(|e| format!("Không ghi được file âm thanh tạm: {e}"))?;
    Ok(true)
}

/// The percentage in a whisper.cpp progress line ("... progress = 42%").
pub fn progress_percent(line: &str) -> Option<u32> {
    let rest = &line[line.find("progress =")? + "progress =".len()..];
    rest.trim().trim_end_matches('%').trim().parse().ok()
}

/// Downloads `d` into `dir` (through a `.part` file), checks its size and SHA-256, and
/// returns the file. A wrong or partial file is removed.
pub fn download(d: &Download, dir: &Path, progress: &mut dyn FnMut(u64, u64), cancel: &AtomicBool) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Không tạo được thư mục {}: {e}", dir.display()))?;
    let dest = dir.join(d.file);
    let part = dir.join(format!("{}.part", d.file));
    let got = http_get(d.url, &part, d.bytes, progress, cancel);
    let checked = got.and_then(|_| {
        let sum = sha256_file(&part)?;
        if sum != d.sha256 {
            return Err(format!("File tải về bị sai ({}), hãy thử lại", d.file));
        }
        Ok(())
    });
    if let Err(e) = checked {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    let _ = std::fs::remove_file(&dest);
    std::fs::rename(&part, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}

/// HTTPS GET of `url` into `dest` with WinHTTP (system proxy, redirects followed).
fn http_get(url: &str, dest: &Path, expected: u64, progress: &mut dyn FnMut(u64, u64), cancel: &AtomicBool) -> Result<(), String> {
    use windows::Win32::Networking::WinHttp::*;
    use windows::core::{HSTRING, PCWSTR};
    let rest = url.strip_prefix("https://").ok_or("Chỉ tải qua https")?;
    let (host, path) = rest.split_once('/').map(|(h, p)| (h, format!("/{p}"))).unwrap_or((rest, "/".into()));
    let fail = |what: &str| format!("Không tải được ({what}). Kiểm tra kết nối mạng rồi thử lại.");
    struct Handle(*mut core::ffi::c_void);
    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: a handle returned by WinHttpOpen/Connect/OpenRequest, closed once.
                unsafe {
                    let _ = WinHttpCloseHandle(self.0);
                }
            }
        }
    }
    // SAFETY: WinHTTP calls on handles owned by the guards above; buffers outlive each call.
    unsafe {
        let session = Handle(WinHttpOpen(&HSTRING::from("HP Screen"), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0));
        if session.0.is_null() {
            return Err(fail("không mở được WinHTTP"));
        }
        let _ = WinHttpSetTimeouts(session.0, 30_000, 30_000, 30_000, 60_000);
        let conn = Handle(WinHttpConnect(session.0, &HSTRING::from(host), INTERNET_DEFAULT_HTTPS_PORT, 0));
        if conn.0.is_null() {
            return Err(fail("không kết nối được máy chủ"));
        }
        let req = Handle(WinHttpOpenRequest(conn.0, &HSTRING::from("GET"), &HSTRING::from(path.as_str()), PCWSTR::null(), PCWSTR::null(), std::ptr::null(), WINHTTP_FLAG_SECURE));
        if req.0.is_null() {
            return Err(fail("không tạo được yêu cầu"));
        }
        WinHttpSendRequest(req.0, None, None, 0, 0, 0).map_err(|e| fail(&e.to_string()))?;
        WinHttpReceiveResponse(req.0, std::ptr::null_mut()).map_err(|e| fail(&e.to_string()))?;
        let mut status = 0u32;
        let mut len = 4u32;
        let mut index = 0u32;
        WinHttpQueryHeaders(req.0, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER, PCWSTR::null(), Some(&mut status as *mut u32 as *mut _), &mut len, &mut index)
            .map_err(|e| fail(&e.to_string()))?;
        if status != 200 {
            return Err(fail(&format!("máy chủ trả mã {status}")));
        }
        let mut file = std::io::BufWriter::new(std::fs::File::create(dest).map_err(|e| e.to_string())?);
        let mut buf = vec![0u8; 256 * 1024];
        let mut done = 0u64;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("Đã hủy".to_string());
            }
            let mut read = 0u32;
            WinHttpReadData(req.0, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut read).map_err(|e| fail(&e.to_string()))?;
            if read == 0 {
                break;
            }
            file.write_all(&buf[..read as usize]).map_err(|e| e.to_string())?;
            done += read as u64;
            progress(done, expected.max(done));
        }
        file.flush().map_err(|e| e.to_string())?;
        if done != expected {
            return Err(fail("tải không trọn vẹn"));
        }
    }
    Ok(())
}

/// Runs whisper.cpp on `wav` (Vietnamese) and returns its JSON output. `progress(percent)` is
/// called as it goes; `cancel` stops the process.
pub fn run_whisper(cli: &Path, model: &Path, wav: &Path, progress: &mut dyn FnMut(u32), cancel: &AtomicBool) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out_base = wav.with_extension("");
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 8);
    let mut child = std::process::Command::new(cli)
        .args(["-m"])
        .arg(model)
        .args(["-f"])
        .arg(wav)
        .args(["-l", "vi", "-t", &threads.to_string(), "-oj", "-ojf", "-pp", "-of"])
        .arg(&out_base)
        .current_dir(cli.parent().unwrap_or_else(|| Path::new(".")))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Không chạy được bộ nhận giọng: {e}"))?;
    let stderr = child.stderr.take().ok_or("Không đọc được tiến độ")?;
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut tail: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err("Đã hủy".to_string());
        }
        while let Ok(line) = rx.try_recv() {
            if let Some(p) = progress_percent(&line) {
                progress(p.min(100));
            }
            tail.push_back(line);
            if tail.len() > 20 {
                tail.pop_front();
            }
        }
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(s) => break s,
            None => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    };
    let _ = reader.join();
    while let Ok(line) = rx.try_recv() {
        tail.push_back(line);
    }
    let json = out_base.with_extension("json");
    if !status.success() {
        let last: Vec<String> = tail.into_iter().rev().take(20).collect::<Vec<_>>().into_iter().rev().collect();
        return Err(format!("Không tạo được phụ đề (mã {:?}).\n{}", status.code(), last.join("\n")));
    }
    let text = std::fs::read_to_string(&json).map_err(|e| format!("Không đọc được kết quả nhận dạng: {e}"))?;
    let _ = std::fs::remove_file(&json);
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_media::{make_sample, temp_path};

    #[test]
    fn sha256_of_a_known_file() {
        let p = temp_path("asr-sha", "abc.txt");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(sha256_file(&p).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn catalog_entries_are_complete() {
        for d in std::iter::once(CLI).chain(MODELS.iter().map(|m| m.download)) {
            assert!(d.url.starts_with("https://") && d.url.ends_with(d.file), "{d:?}");
            assert_eq!(d.sha256.len(), 64);
            assert!(d.sha256.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
            assert!(d.bytes > 1_000_000);
        }
        assert_eq!(model("base").unwrap().download.file, "ggml-base.bin");
        assert!(model("nope").is_none());
    }

    #[test]
    fn progress_lines_are_read() {
        assert_eq!(progress_percent("whisper_print_progress_callback: progress =  42%"), Some(42));
        assert_eq!(progress_percent("progress = 100%"), Some(100));
        assert_eq!(progress_percent("whisper_init_from_file: loading model"), None);
    }

    #[test]
    fn wav_header_and_length() {
        let video = temp_path("asr-wav", "a.mp4");
        make_sample(&video, 1, 320, 240, true);
        let wav = video.with_extension("wav");
        assert!(write_wav_16k_mono(&video, &wav, &AtomicBool::new(false)).unwrap());
        let bytes = std::fs::read(&wav).unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..16], b"WAVEfmt ");
        assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 1, "mono");
        assert_eq!(u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]), 16_000);
        let samples = (bytes.len() - 44) / 2;
        assert!((samples as i64 - 16_000).abs() < 800, "{samples} samples for one second");
    }

    #[test]
    fn a_silent_video_gives_no_wav() {
        let video = temp_path("asr-silent", "a.mp4");
        make_sample(&video, 1, 320, 240, false);
        assert!(!write_wav_16k_mono(&video, &video.with_extension("wav"), &AtomicBool::new(false)).unwrap());
    }

    #[test]
    #[ignore = "downloads about 580 MB and runs speech recognition"]
    fn full_download_and_recognise() {
        let dir = asr_dir();
        let cancel = AtomicBool::new(false);
        let zip = download(&CLI, &dir, &mut |_, _| {}, &cancel).unwrap();
        extract_zip(&zip, &dir.join("bin")).unwrap();
        let cli = find_cli(&dir.join("bin")).expect("whisper-cli.exe in the zip");
        let m = &MODELS[0];
        let model = download(&m.download, &dir, &mut |_, _| {}, &cancel).unwrap();
        let video = temp_path("asr-full", "a.mp4");
        make_sample(&video, 3, 320, 240, true);
        let wav = video.with_extension("wav");
        write_wav_16k_mono(&video, &wav, &cancel).unwrap();
        let json = run_whisper(&cli, &model, &wav, &mut |p| println!("{p}%"), &cancel).unwrap();
        println!("{json}");
        assert!(hp_core::subtitle::parse_whisper_json(&json).is_ok());
    }

    #[test]
    fn zip_with_the_cli_in_a_subfolder() {
        let zip_path = temp_path("asr-zip", "cli.zip");
        {
            let mut z = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            z.start_file("Release/whisper-cli.exe", opts).unwrap();
            z.write_all(b"MZ fake").unwrap();
            z.start_file("../escape.txt", opts).unwrap();
            z.write_all(b"no").unwrap();
            z.finish().unwrap();
        }
        let dir = zip_path.parent().unwrap().join("bin");
        extract_zip(&zip_path, &dir).unwrap();
        assert_eq!(find_cli(&dir), Some(dir.join("Release").join("whisper-cli.exe")));
        assert!(!zip_path.parent().unwrap().join("escape.txt").exists(), "entries outside the folder are skipped");
    }
}
