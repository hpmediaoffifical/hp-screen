# HP Screen giai đoạn 1 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ứng dụng quay màn hình Windows 11 quay được toàn màn hình / một cửa sổ / vùng chọn ra MP4 có tiếng mic và âm máy, có phím tắt, thư viện, chụp ảnh và nhật ký thao tác chuột/phím.

**Architecture:** Cargo workspace gồm `crates/hp-core` (logic thuần, không gọi Windows, test tự động) và `src-tauri` (ứng dụng Tauri 2: các module gọi Windows + lệnh Tauri). Giao diện React + TypeScript chỉ gọi lệnh và nghe sự kiện; dữ liệu hình/tiếng không đi qua webview.

**Tech Stack:** Rust stable, Tauri 2, crate `windows-capture` 2.x (WGC), crate `windows` (WASAPI, Media Foundation, hook, hotkey), `serde`/`serde_json`, `image` (PNG), React 18 + TypeScript + Vite, pnpm.

**Spec:** `docs/superpowers/specs/2026-11-11-hp-screen-mvp-design.md`

## Global Constraints

- Chỉ Windows 11. Không đóng gói FFmpeg. Không dùng mã GPL/AGPL; chỉ phụ thuộc MIT/Apache/BSD.
- Giao diện tiếng Việt, nền tối, bố cục A (thanh trái: Quay / Thư viện / Cài đặt).
- Tên file mặc định `hp_HH-mm-ss_dd-MM-yyyy`, đuôi `.mp4` / `.png`; trùng tên thêm `_2`, `_3`.
- Thư mục lưu mặc định `Videos\HP Screen`; dữ liệu phụ trong thư mục con `.hpscreen`.
- Video H.264 + AAC 48 kHz stereo 192 kbps trong MP4, fps cố định 30 (mặc định) hoặc 60, kích thước chẵn.
- Phím tắt mặc định: `Ctrl+Alt+R` bật/tắt, `Ctrl+Alt+P` tạm dừng, `Ctrl+Alt+S` chụp ảnh. F12 không được làm mặc định.
- Nhật ký phím chỉ ghi tổ hợp có Ctrl/Alt/Win và phím chức năng/điều hướng; không ghi ký tự gõ thường.
- Hook chuột/phím chỉ được cài trong lúc đang quay.
- API của `windows-capture`, Tauri 2 và các plugin phải được tra trên docs.rs / tauri.app trước khi dùng; kế hoạch này không ghim tên hàm của thư viện ngoài.
- Mỗi task kết thúc bằng một commit; thông điệp commit kết thúc bằng dòng `Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>`.

## Review Focus

1. Hai lần chụp/quay trong cùng một giây → file thứ hai phải có hậu tố `_2`, không ghi đè (Task 2).
2. Người dùng sửa mẫu tên chứa ký tự cấm của Windows (`:`, `/`, `*`…) hoặc để trống → tên vẫn hợp lệ (Task 2).
3. Vùng chọn trên màn hình phụ nằm bên trái màn hình chính (tọa độ âm), hoặc tràn mép màn hình, hoặc quá nhỏ → cắt đúng, hoặc từ chối rõ ràng (Task 3).
4. Bấm phím tắt bật/tắt liên tiếp rất nhanh, kể cả lúc đang Hoàn tất → không tạo hai phiên quay, không treo (Task 4).
5. File cài đặt hỏng hoặc thiếu trường (bản cũ) → dùng mặc định cho phần thiếu, không sập (Task 5).

## File Structure

```
Cargo.toml                      workspace: crates/hp-core, src-tauri
crates/hp-core/src/
  lib.rs  naming.rs  geometry.rs  state.rs  timeline.rs  quality.rs
  settings.rs  input_log.rs  hotkey.rs  mix.rs  library.rs
src-tauri/
  tauri.conf.json  build.rs  app.manifest
  src/main.rs  lib.rs  commands.rs  events.rs  paths.rs
  src/capture.rs  encoder.rs  audio.rs  input_hook.rs  hotkeys.rs
  src/screenshot.rs  library_fs.rs  recorder.rs  tray.rs
src/                            React
  main.tsx  App.tsx  api.ts  theme.css
  pages/RecordPage.tsx  LibraryPage.tsx  SettingsPage.tsx
  windows/PickerOverlay.tsx  ControlBar.tsx
spikes/mf-fmp4/                 mã thử nghiệm, không dùng lại
docs/manual-test-checklist.md
```

---

### Task 1: Khung dự án

**Files:** tạo toàn bộ khung ở trên (file module để trống), `package.json`, `vite.config.ts`, `index.html`.

**Interfaces:**
- Produces: workspace build được; `cargo test -p hp-core` chạy; `pnpm tauri dev` mở cửa sổ "HP Screen".

- [ ] **Step 1:** Tạo dự án Tauri 2 + React + TypeScript bằng `pnpm create tauri-app` vào thư mục tạm rồi chuyển file vào gốc dự án (gốc đã có `docs/`, `reports/`). Tên sản phẩm `HP Screen`, identifier `vn.hpscreen.app`.
- [ ] **Step 2:** Tạo `Cargo.toml` ở gốc khai báo workspace với `crates/hp-core` và `src-tauri`; `hp-core` là lib phụ thuộc `serde`, `serde_json`; `src-tauri` phụ thuộc `hp-core` theo path.
- [ ] **Step 3:** Thêm `app.manifest` khai báo `PerMonitorV2` DPI awareness và nhúng qua `build.rs` (tra cách nhúng manifest của `tauri-build`).
- [ ] **Step 4:** Thêm plugin single-instance của Tauri: mở lần hai thì hiện và focus cửa sổ chính.
- [ ] **Step 5:** `App.tsx` dựng bố cục A với ba trang rỗng, tiêu đề tiếng Việt; `theme.css` nền tối.
- [ ] **Step 6:** Chạy `cargo test -p hp-core` (Expected: 0 tests, ok) và `pnpm tauri dev` (Expected: cửa sổ hiện, chuyển được ba mục).
- [ ] **Step 7:** Commit `chore: scaffold Tauri workspace`.

### Task 2: `hp-core::naming`

**Files:** `crates/hp-core/src/naming.rs` (test nằm trong `#[cfg(test)] mod tests` cùng file, áp dụng cho mọi task hp-core).

**Interfaces — Produces:**
```rust
pub const DEFAULT_PATTERN: &str = "hp_HH-mm-ss_dd-MM-yyyy";
#[derive(Clone, Copy)] pub struct LocalTime { pub year: u16, pub month: u8, pub day: u8, pub hour: u8, pub minute: u8, pub second: u8 }
pub fn render_pattern(pattern: &str, t: LocalTime) -> String;   // đã sanitize
pub fn sanitize(name: &str) -> String;
pub fn unique_path(dir: &Path, stem: &str, ext: &str, exists: &dyn Fn(&Path) -> bool) -> PathBuf;
```

- [ ] **Step 1: Viết test (fail)**
```rust
let t = LocalTime { year: 2026, month: 11, day: 11, hour: 14, minute: 30, second: 5 };
assert_eq!(render_pattern(DEFAULT_PATTERN, t), "hp_14-30-05_11-11-2026");
assert_eq!(render_pattern("yyyy-MM-dd_HH-mm-ss", t), "2026-11-11_14-30-05");
assert_eq!(render_pattern("a:b/c*?", t), "a-b-c--");          // ký tự cấm \ / : * ? " < > | → '-'
assert_eq!(render_pattern("", t), "hp");
assert_eq!(sanitize("ten. "), "ten");                          // bỏ dấu chấm/khoảng trắng cuối
// unique_path
let taken = |p: &Path| p.ends_with("hp_x.mp4") || p.ends_with("hp_x_2.mp4");
assert_eq!(unique_path(Path::new("d"), "hp_x", "mp4", &taken), Path::new("d").join("hp_x_3.mp4"));
assert_eq!(unique_path(Path::new("d"), "hp_y", "png", &|_| false), Path::new("d").join("hp_y.png"));
```
- [ ] **Step 2:** `cargo test -p hp-core naming` → FAIL.
- [ ] **Step 3:** Cài đặt. Token phân biệt hoa thường: `MM` tháng, `mm` phút; thay theo thứ tự `yyyy, MM, dd, HH, mm, ss`.
- [ ] **Step 4:** Test PASS. **Step 5:** Commit `feat(core): file naming`.

### Task 3: `hp-core::geometry`

**Interfaces — Produces:**
```rust
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct Rect { pub x: i32, pub y: i32, pub w: u32, pub h: u32 }
pub const MIN_SIZE: u32 = 32;
pub fn make_even(w: u32, h: u32) -> (u32, u32);                       // bớt 1 nếu lẻ
pub fn crop_in_monitor(region: Rect, monitor: Rect) -> Option<Rect>;  // kết quả tương đối gốc màn hình, đã kẹp và làm chẵn
pub fn monitor_containing(point: (i32, i32), monitors: &[Rect]) -> Option<usize>;
```

- [ ] **Step 1: Test**
```rust
assert_eq!(make_even(1921, 1080), (1920, 1080));
let main = Rect { x: 0, y: 0, w: 1920, h: 1080 };
let left = Rect { x: -1920, y: 0, w: 1920, h: 1080 };
assert_eq!(crop_in_monitor(Rect { x: 100, y: 50, w: 801, h: 601 }, main), Some(Rect { x: 100, y: 50, w: 800, h: 600 }));
assert_eq!(crop_in_monitor(Rect { x: -1820, y: 10, w: 400, h: 300 }, left), Some(Rect { x: 100, y: 10, w: 400, h: 300 }));
assert_eq!(crop_in_monitor(Rect { x: 1800, y: 1000, w: 400, h: 300 }, main), Some(Rect { x: 1800, y: 1000, w: 120, h: 80 })); // kẹp vào mép
assert_eq!(crop_in_monitor(Rect { x: 10, y: 10, w: 20, h: 500 }, main), None);       // nhỏ hơn MIN_SIZE
assert_eq!(crop_in_monitor(Rect { x: 5000, y: 0, w: 400, h: 300 }, main), None);     // ngoài màn hình
assert_eq!(monitor_containing((-10, 5), &[main, left]), Some(1));
assert_eq!(monitor_containing((9999, 5), &[main, left]), None);
```
- [ ] **Step 2–5:** FAIL → cài đặt → PASS → commit `feat(core): geometry`.

### Task 4: `hp-core::state` và `hp-core::timeline`

**Interfaces — Produces:**
```rust
#[derive(Clone, Copy, PartialEq, Debug, Serialize)]
pub enum RecState { Idle, Countdown, Recording, Paused, Finalizing }
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Command { Start, CountdownDone, Cancel, Pause, Resume, Stop, Finalized }
pub struct InvalidTransition { pub state: RecState, pub command: Command }
pub fn transition(s: RecState, c: Command) -> Result<RecState, InvalidTransition>;
pub fn toggle_command(s: RecState) -> Option<Command>;        // phím bật/tắt
pub fn pause_toggle_command(s: RecState) -> Option<Command>;  // phím tạm dừng

pub struct Timeline { /* private */ }
impl Timeline {
    pub fn new(start_ticks: i64, ticks_per_sec: i64) -> Self;
    pub fn pause(&mut self, now: i64);
    pub fn resume(&mut self, now: i64);
    pub fn is_paused(&self) -> bool;
    pub fn video_100ns(&self, now: i64) -> Option<i64>;   // None khi đang tạm dừng
    pub fn video_ms(&self, now: i64) -> Option<i64>;
}
```

- [ ] **Step 1: Test bảng chuyển trạng thái** — hợp lệ: `Idle+Start→Countdown`, `Countdown+CountdownDone→Recording`, `Countdown+Cancel→Idle`, `Recording+Pause→Paused`, `Paused+Resume→Recording`, `Recording+Stop→Finalizing`, `Paused+Stop→Finalizing`, `Finalizing+Finalized→Idle`. Mọi cặp khác trả `Err` (duyệt toàn bộ 5×7 cặp trong test). `toggle_command`: Idle→Start, Countdown→Cancel, Recording→Stop, Paused→Stop, Finalizing→None. `pause_toggle_command`: Recording→Pause, Paused→Resume, còn lại None.
- [ ] **Step 2: Test Timeline**
```rust
let mut t = Timeline::new(1_000, 1_000);         // 1000 tick/giây
assert_eq!(t.video_ms(3_000), Some(2_000));
t.pause(3_000);  assert_eq!(t.video_ms(4_000), None);
t.pause(3_500);                                   // pause lặp: bỏ qua
t.resume(5_000); assert_eq!(t.video_ms(6_000), Some(3_000));
t.resume(5_500);                                  // resume lặp: bỏ qua
assert_eq!(t.video_100ns(6_000), Some(30_000_000));
assert_eq!(t.video_ms(500), Some(0));             // trước mốc bắt đầu → 0
```
- [ ] **Step 3–5:** FAIL → cài đặt → PASS → commit `feat(core): record state machine and timeline`.

### Task 5: `hp-core::quality`, `settings`, `hotkey`

**Interfaces — Produces:**
```rust
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)] pub enum Quality { Light, Standard, High }
pub fn video_bitrate(w: u32, h: u32, fps: u32, q: Quality) -> u32;   // bit/giây, tối thiểu 1_000_000

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)] #[serde(default)]
pub struct Settings {
    pub version: u32,                 // 1
    pub output_dir: Option<PathBuf>,  // None = Videos\HP Screen
    pub name_pattern: String,         // DEFAULT_PATTERN
    pub fps: u32,                     // 30; chỉ 30|60
    pub quality: Quality,             // Standard
    pub countdown_secs: u32,          // 3; chỉ 0|3|5
    pub system_audio: bool,           // true
    pub mic: bool,                    // true
    pub hotkey_toggle: String,        // "Ctrl+Alt+R"
    pub hotkey_pause: String,         // "Ctrl+Alt+P"
    pub hotkey_screenshot: String,    // "Ctrl+Alt+S"
    pub input_log: bool,              // true
    pub autostart: bool,              // false
    pub last_region: Option<Rect>,
}
pub fn load_settings(json: &str) -> Settings;   // không bao giờ lỗi; đã normalize
pub fn normalize(s: Settings) -> Settings;       // fps/countdown ngoài tập cho phép → mặc định; hotkey không parse được → mặc định

#[derive(Clone, Copy, PartialEq, Debug)] pub struct Hotkey { pub ctrl: bool, pub alt: bool, pub shift: bool, pub win: bool, pub vk: u32 }
pub fn parse_hotkey(s: &str) -> Result<Hotkey, String>;
pub fn format_hotkey(h: &Hotkey) -> String;
```

- [ ] **Step 1: Test**
```rust
assert_eq!(video_bitrate(1920, 1080, 30, Quality::Standard), 8_000_000);
assert_eq!(video_bitrate(1920, 1080, 60, Quality::Standard), 12_000_000);
assert_eq!(video_bitrate(1920, 1080, 30, Quality::Light), 4_800_000);
assert_eq!(video_bitrate(1920, 1080, 30, Quality::High), 12_800_000);
assert_eq!(video_bitrate(3840, 2160, 30, Quality::Standard), 32_000_000);
assert_eq!(video_bitrate(64, 64, 30, Quality::Light), 1_000_000);

assert_eq!(load_settings("{{{ hỏng"), Settings::default());
assert_eq!(load_settings(r#"{"fps":60}"#).countdown_secs, 3);          // thiếu trường → mặc định
assert_eq!(load_settings(r#"{"fps":24,"countdown_secs":7}"#).fps, 30);  // giá trị lạ → mặc định
assert_eq!(load_settings(r#"{"hotkey_toggle":"abc"}"#).hotkey_toggle, "Ctrl+Alt+R");

let h = parse_hotkey("Ctrl+Alt+R").unwrap();
assert!(h.ctrl && h.alt && !h.shift && h.vk == 0x52);
assert_eq!(format_hotkey(&parse_hotkey("alt + ctrl + r").unwrap()), "Ctrl+Alt+R");
assert!(parse_hotkey("R").is_err());          // phím chữ phải có Ctrl, Alt hoặc Win
assert!(parse_hotkey("Shift+R").is_err());
assert!(parse_hotkey("F9").is_ok());          // phím F đứng một mình được
assert!(parse_hotkey("F12").is_err());        // Windows dành riêng
assert!(parse_hotkey("Ctrl+Alt").is_err());
```
- [ ] **Step 2–5:** FAIL → cài đặt (hỗ trợ A–Z, 0–9, F1–F11, F13–F24) → PASS → commit `feat(core): quality, settings, hotkey parsing`.

### Task 6: `hp-core::input_log`, `mix`, `library`

**Interfaces — Produces:**
```rust
// input_log
#[derive(Serialize)] pub struct Header { pub v: u32, pub mode: String, pub region: Rect, pub width: u32, pub height: u32, pub fps: u32 }
#[derive(Serialize)] #[serde(tag = "e", rename_all = "lowercase")]
pub enum Event {
    Move { t: i64, x: i32, y: i32 },
    Down { t: i64, b: Button, x: i32, y: i32 },
    Up { t: i64, b: Button, x: i32, y: i32 },
    Wheel { t: i64, dx: i32, dy: i32, x: i32, y: i32 },
    Key { t: i64, k: String },
}
#[derive(Serialize)] #[serde(rename_all = "lowercase")] pub enum Button { Left, Right, Middle }
pub fn to_line<T: Serialize>(v: &T) -> String;                 // một dòng JSON, không có '\n'
pub struct KeyStroke { pub ctrl: bool, pub alt: bool, pub shift: bool, pub win: bool, pub vk: u32 }
pub fn loggable_combo(k: &KeyStroke) -> Option<String>;
pub struct MoveSampler { /* private */ }
impl MoveSampler { pub fn new() -> Self; pub fn accept(&mut self, t_ms: i64) -> bool; }  // tối đa 60 mẫu/giây

// mix
pub fn mix_f32(a: &[f32], b: &[f32]) -> Vec<f32>;   // cộng mẫu, kẹp [-1, 1], dài bằng mảng dài hơn

// library
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)] pub enum Kind { Video, Image }
#[derive(Clone, PartialEq, Debug, Serialize)] pub struct Item { pub path: PathBuf, pub kind: Kind, pub created_ms: i64, pub size: u64 }
pub fn kind_of(path: &Path) -> Option<Kind>;                    // .mp4 → Video, .png → Image, không phân biệt hoa thường
pub fn sort_and_filter(items: Vec<Item>, filter: Option<Kind>) -> Vec<Item>;   // mới nhất trước
pub fn sidecar_path(media: &Path) -> PathBuf;                   // <dir>/.hpscreen/<stem>.input.jsonl
pub fn thumb_path(media: &Path) -> PathBuf;                     // <dir>/.hpscreen/<stem>.thumb.png
```

- [ ] **Step 1: Test**
```rust
assert_eq!(to_line(&Event::Down { t: 1200, b: Button::Left, x: 10, y: 20 }), r#"{"e":"down","t":1200,"b":"left","x":10,"y":20}"#);
let k = |ctrl, alt, shift, win, vk| KeyStroke { ctrl, alt, shift, win, vk };
assert_eq!(loggable_combo(&k(true, false, false, false, 0x43)), Some("Ctrl+C".into()));
assert_eq!(loggable_combo(&k(false, true, false, false, 0x09)), Some("Alt+Tab".into()));
assert_eq!(loggable_combo(&k(false, false, false, false, 0x74)), Some("F5".into()));
assert_eq!(loggable_combo(&k(false, false, false, false, 0x0D)), Some("Enter".into()));
assert_eq!(loggable_combo(&k(false, false, false, false, 0x41)), None);   // 'a'
assert_eq!(loggable_combo(&k(false, false, true, false, 0x41)), None);    // 'A'
assert_eq!(loggable_combo(&k(false, false, false, false, 0x31)), None);   // '1'
assert_eq!(loggable_combo(&k(true, false, false, false, 0x11)), None);    // chỉ bấm Ctrl
let mut s = MoveSampler::new();
assert!(s.accept(0)); assert!(!s.accept(10)); assert!(s.accept(17));
assert_eq!(mix_f32(&[0.5, 0.9], &[0.25, 0.9, -0.1]), vec![0.75, 1.0, -0.1]);
assert_eq!(sidecar_path(Path::new("d/hp_a.mp4")), Path::new("d/.hpscreen/hp_a.input.jsonl"));
assert_eq!(kind_of(Path::new("x.MP4")), Some(Kind::Video)); assert_eq!(kind_of(Path::new("x.txt")), None);
// sort_and_filter: 3 item created_ms 1,3,2 → thứ tự 3,2,1; filter Image chỉ còn ảnh
```
Phím được ghi khi không có modifier: F1–F24, Esc, Enter, Tab, Backspace, Delete, Insert, Home, End, PageUp, PageDown, bốn mũi tên, PrintScreen. Shift đứng một mình không tính là modifier đủ điều kiện.
- [ ] **Step 2–5:** FAIL → cài đặt → PASS → commit `feat(core): input log, audio mix, library model`.

### Task 7: Phép thử MP4 an toàn khi treo máy (spike)

**Files:** `spikes/mf-fmp4/` (crate riêng, ngoài workspace), sửa mục 4.3 và mục 8 của spec.

- [ ] **Step 1:** Viết chương trình nhỏ dùng Media Foundation Sink Writer ghi 20 giây khung hình màu đổi dần (H.264) vào MP4 với container phân mảnh (`MFTranscodeContainerType_FMPEG4`), in ra "writing" mỗi giây.
- [ ] **Step 2:** Chạy, đến giây thứ 10 thì `taskkill /F`. Mở file bằng trình phát mặc định của Windows và bằng trình duyệt.
- [ ] **Step 3:** Chạy lại đến hết để chắc file hoàn chỉnh phát được và tua được.
- [ ] **Step 4:** Ghi kết quả vào spec mục 4.3: dùng MP4 phân mảnh nếu file bị tắt nóng vẫn phát được ≥ 8 giây; nếu không, chọn phương án dự phòng (a) hoặc (b) và nêu lý do. Task 9 làm theo kết luận này.
- [ ] **Step 5:** Commit `spike: fragmented MP4 crash safety` (mã spike đánh dấu không dùng lại).

### Task 8: `capture` và `screenshot`

**Files:** `src-tauri/src/capture.rs`, `screenshot.rs`, `paths.rs`, `commands.rs`.

**Interfaces:**
- Consumes: `hp_core::geometry::{Rect, crop_in_monitor}`, `naming::*`, `settings::Settings`.
- Produces:
```rust
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(tag = "kind", rename_all = "lowercase")]
pub enum Target { Monitor { index: usize }, Window { hwnd: isize }, Region { monitor: usize, rect: Rect } }  // rect theo tọa độ màn hình ảo
#[derive(Serialize)] pub struct MonitorInfo { pub index: usize, pub name: String, pub rect: Rect, pub primary: bool }
#[derive(Serialize)] pub struct WindowInfo { pub hwnd: isize, pub title: String, pub process: String, pub rect: Rect }
pub fn list_monitors() -> Vec<MonitorInfo>;
pub fn list_windows() -> Vec<WindowInfo>;     // chỉ cửa sổ hiển thị, có tiêu đề, không phải của HP Screen
pub struct Frame<'a> { pub bgra: &'a [u8], pub width: u32, pub height: u32, pub stride: u32, pub qpc_ticks: i64 }
pub trait FrameSink: Send { fn on_frame(&mut self, f: Frame<'_>); fn on_closed(&mut self); }
pub struct CaptureSession;  // Drop = dừng
pub fn start_capture(target: &Target, sink: Box<dyn FrameSink>) -> Result<CaptureSession, String>;  // đã cắt vùng, kích thước chẵn, tắt viền, có con trỏ
pub fn output_size(target: &Target) -> Result<(u32, u32), String>;
pub fn grab_frame(target: &Target) -> Result<image::RgbaImage, String>;   // screenshot.rs: kích thước pixel vật lý, không làm chẵn
pub fn output_dir(s: &Settings) -> PathBuf;   // paths.rs: tạo thư mục và .hpscreen nếu chưa có
```
- Lệnh Tauri: `list_monitors`, `list_windows`, `take_screenshot(target: Target) -> String` (đường dẫn file; lưu PNG + copy clipboard), phát sự kiện `library-changed`.

- [ ] **Step 1:** Cài `capture.rs` trên `windows-capture`. Thử ngay việc tắt viền vàng; ghi kết quả vào spec mục 8.
- [ ] **Step 2:** Cài `screenshot.rs` + lệnh `take_screenshot`; tên file qua `render_pattern` + `unique_path` với đuôi `png`.
- [ ] **Step 3:** Nút tạm "Chụp ảnh" trên trang Quay gọi `take_screenshot` với `Monitor{0}`.
- [ ] **Step 4: Kiểm tra thủ công:** ảnh PNG xuất hiện trong `Videos\HP Screen`, kích thước bằng độ phân giải vật lý của màn hình (thử ở tỉ lệ 150%), dán được từ clipboard; chụp hai lần trong một giây ra hai file.
- [ ] **Step 5:** Commit `feat: capture sources and screenshots`.

### Task 9: `encoder` và `recorder` (chỉ hình)

**Files:** `src-tauri/src/encoder.rs`, `recorder.rs`, `events.rs`, `commands.rs`.

**Interfaces:**
- Consumes: Task 4 (`transition`, `Timeline`), Task 5 (`video_bitrate`, `Settings`), Task 8 (`start_capture`, `FrameSink`, `output_size`).
- Produces:
```rust
pub struct EncoderConfig { pub path: PathBuf, pub width: u32, pub height: u32, pub fps: u32, pub video_bitrate: u32, pub audio: bool }
pub struct Mp4Encoder;
impl Mp4Encoder {
    pub fn new(cfg: &EncoderConfig) -> Result<Self, String>;   // thử phần cứng, lỗi thì phần mềm
    pub fn write_video(&mut self, bgra: &[u8], stride: u32, time_100ns: i64) -> Result<(), String>;
    pub fn write_audio(&mut self, pcm_f32_stereo_48k: &[f32], time_100ns: i64) -> Result<(), String>;
    pub fn finish(self) -> Result<(), String>;
}
pub struct Recorder;   // một thể hiện trong trạng thái Tauri, bọc Mutex
impl Recorder {
    pub fn command(&self, c: Command, target: Option<Target>) -> Result<RecState, String>;
    pub fn state(&self) -> RecState;
}
```
- Lệnh Tauri: `rec_start(target)`, `rec_stop()`, `rec_toggle_pause()`, `rec_cancel()`, `rec_state()`.
- Sự kiện: `rec-state` `{ state, elapsed_ms, file, warning }` phát khi đổi trạng thái và mỗi 500 ms khi đang quay; `rec-error` `{ message }` (tiếng Việt).

- [ ] **Step 1:** Cài `Mp4Encoder` theo kết luận Task 7. Tốc độ khung hình cố định: `recorder` giữ khung hình mới nhất và một luồng nhịp `1/fps` ghi khung đó với thời gian từ `Timeline` (WGC chỉ phát khung khi màn hình đổi).
- [ ] **Step 2:** Cài `Recorder`: Start → đếm ngược `countdown_secs` (phát `rec-state` mỗi giây) → mở capture + encoder → Recording; Stop → `finish()`, tạo ảnh thu nhỏ từ khung đầu vào `thumb_path`, phát `library-changed`, về Idle. Lệnh sai trạng thái trả lỗi, không đổi gì.
- [ ] **Step 3:** Pause/Resume dùng `Timeline::pause/resume`; luồng nhịp không ghi khi `video_100ns` trả `None`.
- [ ] **Step 4:** Lỗi giữa chừng: cố `finish()`, về Idle, phát `rec-error`. Cửa sổ nguồn đóng (`on_closed`) → tự Stop.
- [ ] **Step 5: Kiểm tra thủ công:** quay màn hình 15 giây có tạm dừng 5 giây ở giữa → file MP4 dài ~10 giây, phát được, tua được, đúng fps; quay một cửa sổ bị cửa sổ khác che → video chỉ có cửa sổ đó.
- [ ] **Step 6:** Commit `feat: MP4 encoder and recording session (video)`.

### Task 10: `audio`

**Files:** `src-tauri/src/audio.rs`, sửa `recorder.rs`.

**Interfaces:**
- Consumes: `hp_core::mix::mix_f32`, `Mp4Encoder::write_audio`, `Timeline`.
- Produces:
```rust
pub struct AudioConfig { pub system: bool, pub mic: bool }
pub trait AudioSink: Send { fn on_audio(&mut self, pcm_f32_stereo_48k: &[f32], qpc_ticks: i64); }
pub struct AudioSession;  // Drop = dừng
pub fn start_audio(cfg: AudioConfig, sink: Box<dyn AudioSink>) -> Result<AudioSession, String>;
pub struct AudioStatus { pub system_available: bool, pub mic_available: bool }
pub fn audio_status() -> AudioStatus;
```

- [ ] **Step 1:** WASAPI loopback trên thiết bị phát mặc định và capture trên mic mặc định; chuyển mẫu về f32 stereo 48 kHz; trộn bằng `mix_f32`; gọi sink theo khối 10 ms kèm mốc QPC.
- [ ] **Step 2:** Theo dõi đổi thiết bị mặc định (`IMMNotificationClient`): mở lại luồng trên thiết bị mới, chèn im lặng cho quãng trống để thời gian âm thanh liên tục. Loopback không phát mẫu khi máy im lặng → cũng chèn im lặng theo đồng hồ.
- [ ] **Step 3:** Thiếu thiết bị → vẫn chạy với nguồn còn lại; `audio_status` báo cho giao diện. Lệnh Tauri `audio_status`.
- [ ] **Step 4:** Nối vào `Recorder`: thời gian âm thanh qua cùng `Timeline`; bỏ mẫu khi tạm dừng.
- [ ] **Step 5: Kiểm tra thủ công:** quay 10 phút video có đồng hồ đếm và tiếng bíp mỗi giây → cuối video hình và tiếng lệch dưới 100 ms; rút tai nghe giữa lúc quay → tiếng tiếp tục ở loa, video không hỏng; nói vào mic → nghe được trong video.
- [ ] **Step 6:** Commit `feat: system audio and microphone capture`.

### Task 11: `input_hook`, `hotkeys`, khay hệ thống, thanh điều khiển

**Files:** `src-tauri/src/input_hook.rs`, `hotkeys.rs`, `tray.rs`, `src/windows/ControlBar.tsx`, sửa `recorder.rs`, `tauri.conf.json`.

**Interfaces:**
- Consumes: `hp_core::input_log::*`, `hotkey::parse_hotkey`, `state::{toggle_command, pause_toggle_command}`, `library::sidecar_path`.
- Produces:
```rust
pub struct InputHook;  // Drop = gỡ hook
pub fn start_input_hook(log_path: &Path, header: Header, timeline: Arc<Mutex<Timeline>>) -> Result<InputHook, String>;
pub enum HotkeyAction { Toggle, Pause, Screenshot }
pub fn register_hotkeys(app: &AppHandle, s: &Settings) -> Vec<(HotkeyAction, Result<(), String>)>;  // đăng ký lại toàn bộ
```
- Lệnh Tauri: `hotkey_status() -> [{ action, ok, message }]`.

- [ ] **Step 1:** `input_hook`: luồng riêng có message loop, `WH_MOUSE_LL` + `WH_KEYBOARD_LL`; callback chỉ đẩy sự kiện thô vào channel, luồng khác lọc (`MoveSampler`, `loggable_combo`), đổi tọa độ, ghi từng dòng bằng `to_line`, flush mỗi giây. Bỏ sự kiện khi `Timeline` đang tạm dừng. Dòng đầu là `Header`.
- [ ] **Step 2:** `Recorder` cài hook khi vào Recording nếu `settings.input_log`, gỡ khi Stop/Cancel/lỗi.
- [ ] **Step 3:** `hotkeys` qua plugin global-shortcut của Tauri (hoặc `RegisterHotKey`); Toggle → `toggle_command(state)`, Pause → `pause_toggle_command(state)`, Screenshot → chụp theo target gần nhất. Đăng ký thất bại không làm sập ứng dụng.
- [ ] **Step 4:** Khay hệ thống: Bắt đầu/Dừng quay, Chụp ảnh, Mở HP Screen, Thoát. Đóng cửa sổ chính → ẩn xuống khay.
- [ ] **Step 5:** Cửa sổ `control-bar` (không viền, luôn trên cùng, không hiện ở taskbar): thời gian, Tạm dừng/Tiếp tục, Dừng, dòng cảnh báo. Đặt `WDA_EXCLUDEFROMCAPTURE` cho cửa sổ này. Hiện khi Countdown/Recording/Paused, ẩn khi Idle; cửa sổ chính tự thu nhỏ khi bắt đầu quay.
- [ ] **Step 6: Kiểm tra thủ công:** đang ở trình duyệt bấm `Ctrl+Alt+R` → đếm ngược rồi quay; bấm liên tiếp 5 lần thật nhanh → không có hai phiên, không treo; thanh điều khiển không có trong video; file `.input.jsonl` có `down`/`up`/`wheel`/`move` và `Ctrl+C`, không có ký tự gõ vào Notepad; sau khi dừng, hook đã gỡ.
- [ ] **Step 7:** Commit `feat: input log, hotkeys, tray, control bar`.

### Task 12: Trang Quay và lớp chọn vùng/cửa sổ

**Files:** `src/pages/RecordPage.tsx`, `src/windows/PickerOverlay.tsx`, `src/api.ts`, sửa `commands.rs`, `tauri.conf.json`.

**Interfaces:**
- Consumes: các lệnh và sự kiện của Task 8–11.
- Produces: lệnh `pick_region() -> Option<Target>` và `pick_window() -> Option<Target>` (mở cửa sổ `picker`, trả kết quả khi người dùng chọn hoặc `None` khi Esc); `api.ts` bọc mọi lệnh/sự kiện có kiểu TypeScript khớp với các struct Rust.

- [ ] **Step 1:** `RecordPage`: ba thẻ chế độ (Toàn màn hình có chọn màn hình; Cửa sổ; Vùng chọn có "Dùng vùng gần nhất"), chip Mic / Âm máy bật-tắt và hiện "không có thiết bị", chip `MP4 · 30 fps`, nút lớn "BẮT ĐẦU QUAY" kèm phím tắt, ghi chú giới hạn của chế độ Cửa sổ (menu chuột phải/popup), danh sách "Vừa quay" 3 mục.
- [ ] **Step 2:** `PickerOverlay`: cửa sổ trong suốt phủ một màn hình. Chế độ vùng: kéo chọn, hiện `rộng × cao` theo pixel vật lý, từ chối vùng nhỏ hơn 32 px. Chế độ cửa sổ: tô sáng cửa sổ dưới con trỏ (dùng `list_windows`), bấm để chọn. Esc hủy. Lưu `last_region` vào cài đặt.
- [ ] **Step 3:** Nút quay phản ánh `rec-state` (Bắt đầu / Đang đếm ngược – Hủy / Dừng); `rec-error` hiện thành thông báo.
- [ ] **Step 4: Kiểm tra thủ công:** ba chế độ quay từ giao diện; vùng chọn trên màn hình phụ đúng vị trí; kích thước video của vùng lẻ được làm chẵn.
- [ ] **Step 5:** Commit `feat: record page and region/window picker`.

### Task 13: Thư viện

**Files:** `src-tauri/src/library_fs.rs`, `src/pages/LibraryPage.tsx`, sửa `commands.rs`.

**Interfaces:**
- Consumes: `hp_core::library::*`, `naming::{sanitize, unique_path}`.
- Produces lệnh Tauri: `library_list(filter: Option<Kind>) -> Vec<LibraryEntry>` (`Item` + `thumb: Option<String>`, `duration_ms`, `width`, `height`), `library_rename(path, new_stem) -> String`, `library_delete(path)`, `library_open(path)`, `library_reveal(path)`.

- [ ] **Step 1:** `library_fs`: quét thư mục lưu (không đệ quy), đọc thời lượng/kích thước video qua thuộc tính file của Windows, lưu đệm vào `.hpscreen/index.json` theo đường dẫn + thời gian sửa; ảnh thu nhỏ cho ảnh PNG tạo khi quét.
- [ ] **Step 2:** Đổi tên: `sanitize`, từ chối nếu trùng, đổi kèm file nhật ký và ảnh thu nhỏ. Xóa: chuyển vào Thùng rác (file chính), xóa hẳn file phụ.
- [ ] **Step 3:** `LibraryPage`: lọc Tất cả / Video / Ảnh, lưới thẻ có ảnh thu nhỏ, tên, thời lượng, độ phân giải, dung lượng; nút Phát, Mở thư mục; F2 đổi tên tại chỗ; Delete xóa có xác nhận; tự làm mới khi nhận `library-changed`; trạng thái rỗng có hướng dẫn.
- [ ] **Step 4: Kiểm tra thủ công:** video mới quay hiện trên cùng; đổi tên thì file nhật ký đổi theo; xóa thì file vào Thùng rác.
- [ ] **Step 5:** Commit `feat: library`.

### Task 14: Cài đặt và xử lý lỗi

**Files:** `src/pages/SettingsPage.tsx`, sửa `commands.rs`, `recorder.rs`, `paths.rs`.

**Interfaces:**
- Consumes: `hp_core::settings::*`, `naming::render_pattern`, `register_hotkeys`.
- Produces lệnh Tauri: `settings_get() -> Settings`, `settings_set(s: Settings) -> Settings` (normalize, lưu vào thư mục cấu hình của ứng dụng, đăng ký lại phím tắt), `preview_name(pattern) -> String`, `pick_folder() -> Option<String>`.

- [ ] **Step 1:** `SettingsPage` theo spec mục 4.9: thư mục lưu, mẫu tên có dòng xem trước trực tiếp, fps, chất lượng, đếm ngược, ba ô phím tắt (bấm tổ hợp để ghi, hiện cảnh báo từ `hotkey_status`), bật/tắt nhật ký thao tác kèm giải thích riêng tư, khởi động cùng Windows (plugin autostart).
- [ ] **Step 2:** Trước khi quay: thư mục lưu phải ghi được, nếu không trả lỗi kèm gợi ý chọn thư mục khác. Trong khi quay: mỗi 5 giây kiểm tra dung lượng trống, dưới 500 MB thì tự Stop và báo.
- [ ] **Step 3:** Ghi nhật ký lỗi của lõi vào thư mục log của ứng dụng (plugin log của Tauri).
- [ ] **Step 4: Kiểm tra thủ công:** sửa file cài đặt thành rác rồi mở ứng dụng → chạy với mặc định; đổi phím tắt sang tổ hợp đang bị ứng dụng khác giữ → có cảnh báo; đổi thư mục lưu → file mới vào thư mục mới.
- [ ] **Step 5:** Commit `feat: settings and error handling`.

### Task 15: Bộ cài và danh sách kiểm thủ công

**Files:** `docs/manual-test-checklist.md`, `src-tauri/tauri.conf.json`, icon.

- [ ] **Step 1:** Cấu hình bundle NSIS, tên `HP Screen`, icon, phiên bản `0.1.0`, yêu cầu WebView2.
- [ ] **Step 2:** Viết `docs/manual-test-checklist.md` gom toàn bộ bước "Kiểm tra thủ công" của Task 8–14 cùng danh sách ở spec mục 6.
- [ ] **Step 3:** `pnpm tauri build` → cài bằng bộ cài vừa tạo trên máy phát triển, chạy hết danh sách kiểm; ghi kết quả từng mục vào file.
- [ ] **Step 4:** `cargo test -p hp-core` PASS toàn bộ; `cargo clippy --workspace` không có cảnh báo mới.
- [ ] **Step 5:** Commit `chore: installer and manual test checklist`.
