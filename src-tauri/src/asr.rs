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

/// Installs the whisper.cpp package `zip` as `dir/bin`, replacing what was there only once the
/// whole package unpacked: a half-unpacked package never counts as installed.
pub fn install_cli(zip: &Path, dir: &Path) -> Result<(), String> {
    let tmp = dir.join("bin.tmp");
    let bin = dir.join("bin");
    let _ = std::fs::remove_dir_all(&tmp);
    let unpacked = extract_zip(zip, &tmp).and_then(|_| match find_cli(&tmp) {
        Some(_) => Ok(()),
        None => Err("Gói whisper.cpp không có whisper-cli.exe".to_string()),
    });
    if let Err(e) = unpacked {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    if bin.exists() {
        std::fs::remove_dir_all(&bin).map_err(|e| format!("Không thay được bộ nhận giọng cũ: {e}"))?;
    }
    std::fs::rename(&tmp, &bin).map_err(|e| format!("Không cài được bộ nhận giọng: {e}"))
}

/// A file's text; bytes that are not UTF-8 (whisper can split a Vietnamese letter between two
/// tokens) become U+FFFD instead of failing the whole read.
pub fn read_text_lossy(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Không đọc được kết quả nhận dạng: {e}"))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Sends each line of `r` to `tx`, whatever its encoding, until `r` ends. Reading never stops
/// early, so the child never blocks on a full pipe.
pub fn pump_lines(r: impl Read, tx: std::sync::mpsc::Sender<String>) {
    let mut r = BufReader::new(r);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match r.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n']).to_string();
                let _ = tx.send(line);
            }
        }
    }
}

/// `p` in 8.3 form when it has characters outside ASCII: whisper-cli reads its arguments in
/// the ANSI code page, which cannot hold every Vietnamese letter. Unchanged when it is plain
/// ASCII or has no short form.
pub fn short_path(p: &Path) -> PathBuf {
    use windows::Win32::Storage::FileSystem::GetShortPathNameW;
    use windows::core::PCWSTR;
    let s = p.as_os_str().to_string_lossy();
    if s.is_ascii() {
        return p.to_path_buf();
    }
    let wide: Vec<u16> = p.as_os_str().encode_wide_nul();
    let mut out = vec![0u16; 1024];
    // SAFETY: `wide` is NUL-terminated; `out` is a writable buffer of its length.
    let n = unsafe { GetShortPathNameW(PCWSTR(wide.as_ptr()), Some(&mut out)) } as usize;
    if n == 0 || n >= out.len() {
        return p.to_path_buf();
    }
    let short = PathBuf::from(String::from_utf16_lossy(&out[..n]));
    if short.as_os_str().to_string_lossy().is_ascii() { short } else { p.to_path_buf() }
}

trait EncodeWideNul {
    fn encode_wide_nul(&self) -> Vec<u16>;
}

impl EncodeWideNul for std::ffi::OsStr {
    fn encode_wide_nul(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        self.encode_wide().chain(std::iter::once(0)).collect()
    }
}

/// A Windows job object that ends its processes when it is closed, which also happens when
/// HP Screen exits for any reason: whisper-cli never keeps running on its own.
pub struct KillJob(windows::Win32::Foundation::HANDLE);

// SAFETY: a job handle may be used and closed from any thread.
unsafe impl Send for KillJob {}
unsafe impl Sync for KillJob {}

impl KillJob {
    pub fn new() -> Result<KillJob, String> {
        use windows::Win32::System::JobObjects::*;
        // SAFETY: plain Win32 calls; the info struct outlives the call that reads it.
        unsafe {
            let h = CreateJobObjectW(None, windows::core::PCWSTR::null()).map_err(|e| e.to_string())?;
            let job = KillJob(h);
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .map_err(|e| e.to_string())?;
            Ok(job)
        }
    }

    pub fn assign(&self, child: &std::process::Child) -> Result<(), String> {
        use std::os::windows::io::AsRawHandle;
        // SAFETY: both handles are valid for the duration of the call.
        unsafe {
            windows::Win32::System::JobObjects::AssignProcessToJobObject(self.0, windows::Win32::Foundation::HANDLE(child.as_raw_handle()))
                .map_err(|e| e.to_string())
        }
    }
}

impl Drop for KillJob {
    fn drop(&mut self) {
        // SAFETY: the handle is owned and closed once.
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

/// The job every whisper-cli run joins; it lives until the app exits.
fn app_job() -> Option<&'static KillJob> {
    static JOB: std::sync::OnceLock<Option<KillJob>> = std::sync::OnceLock::new();
    JOB.get_or_init(|| KillJob::new().ok()).as_ref()
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
    // whisper-cli reads paths in the ANSI code page: give it short forms of non-ASCII paths.
    let (model, wav) = (short_path(model), short_path(wav));
    let out_base = wav.with_extension("");
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 8);
    let mut child = std::process::Command::new(cli)
        .args(["-m"])
        .arg(&model)
        .args(["-f"])
        .arg(&wav)
        .args(["-l", "vi", "-t", &threads.to_string(), "-oj", "-ojf", "-pp", "-of"])
        .arg(&out_base)
        .current_dir(cli.parent().unwrap_or_else(|| Path::new(".")))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Không chạy được bộ nhận giọng: {e}"))?;
    if let Some(job) = app_job() {
        let _ = job.assign(&child);
    }
    let stderr = child.stderr.take().ok_or("Không đọc được tiến độ")?;
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let reader = std::thread::spawn(move || pump_lines(stderr, tx));
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
        let code = status.code().map_or_else(|| "?".to_string(), |c| format!("{c:#x}"));
        return Err(format!("Không tạo được phụ đề (mã lỗi {code}).\n{}", last.join("\n")));
    }
    let text = read_text_lossy(&json)?;
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

    #[test]
    fn a_broken_package_is_not_left_half_installed() {
        let zip_path = temp_path("asr-install", "cli.zip");
        {
            let mut z = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            z.start_file("Release/whisper-cli.exe", opts).unwrap();
            z.write_all(b"MZ fake").unwrap();
            z.start_file("Release/whisper.dll", opts).unwrap();
            z.write_all(b"DLL-CONTENT-THAT-GETS-CORRUPTED").unwrap();
            z.finish().unwrap();
        }
        let good = std::fs::read(&zip_path).unwrap();
        let dir = zip_path.parent().unwrap().to_path_buf();
        // Damage the second file's bytes: its checksum fails while unpacking.
        let mut bad = good.clone();
        let at = bad.windows(11).position(|w| w == b"DLL-CONTENT").unwrap();
        bad[at] ^= 0xFF;
        std::fs::write(&zip_path, &bad).unwrap();
        assert!(install_cli(&zip_path, &dir).is_err());
        assert_eq!(find_cli(&dir.join("bin")), None, "the program does not count as installed");
        std::fs::write(&zip_path, &good).unwrap();
        install_cli(&zip_path, &dir).unwrap();
        assert!(find_cli(&dir.join("bin")).is_some());
        assert!(dir.join("bin").join("Release").join("whisper.dll").exists());
    }

    #[test]
    fn output_with_split_characters_is_still_read() {
        let p = temp_path("asr-lossy", "out.json");
        // "ệ" is E1 BB 87: whisper can write a token with only its first two bytes.
        std::fs::write(&p, b"{\"t\":\"\xe1\xbb\"}").unwrap();
        let text = read_text_lossy(&p).unwrap();
        assert!(text.starts_with("{\"t\":\"") && text.contains('\u{FFFD}'));
    }

    #[test]
    fn progress_lines_after_a_non_utf8_line_are_read() {
        let input: &[u8] = b"model C:\\Users\\Ph\xf9ng\\x.bin\nwhisper_print_progress_callback: progress =  50%\n";
        let (tx, rx) = std::sync::mpsc::channel();
        pump_lines(input, tx);
        let lines: Vec<String> = rx.try_iter().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(progress_percent(&lines[1]), Some(50));
    }

    #[test]
    fn closing_the_job_ends_the_child() {
        let job = KillJob::new().unwrap();
        let mut child = std::process::Command::new("ping").args(["-n", "30", "127.0.0.1"]).stdout(std::process::Stdio::null()).spawn().unwrap();
        job.assign(&child).unwrap();
        drop(job);
        let t = std::time::Instant::now();
        while child.try_wait().unwrap().is_none() {
            assert!(t.elapsed().as_secs() < 5, "the child outlived the job");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    #[test]
    fn non_ascii_paths_are_given_in_short_form() {
        let dir = temp_path("asr-short", "x").parent().unwrap().join("Phụng");
        std::fs::create_dir_all(&dir).unwrap();
        let s = short_path(&dir);
        assert!(s.exists());
        assert_eq!(std::fs::canonicalize(&s).unwrap(), std::fs::canonicalize(&dir).unwrap());
        assert_eq!(short_path(Path::new("C:\\Windows")), PathBuf::from("C:\\Windows"), "plain paths are kept");
    }
}
