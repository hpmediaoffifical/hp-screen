# Nâng cấp quay — Bước 1–2 (nền xuất, cắt nhanh, hiệu ứng click và phím) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Từ Thư viện mở một video vào trang Chỉnh sửa, cắt đầu/đuôi/đoạn giữa, bật vòng click và chip phím tắt, xem trước, rồi xuất ra `<tên>_edit.mp4` mà không đụng file gốc.

**Architecture:** Logic thuần nằm trong `hp-core`: đọc nhật ký, `cut`, `edit`, `effects`, `plan`. Phía `src-tauri` có `decoder` (Source Reader), `compositor` (Direct2D/DirectWrite) và `export` (luồng nền dùng lại `Mp4Encoder`). Giao diện là trang `EditPage`: thẻ `<video>` cộng một canvas vẽ hiệu ứng, lấy dữ liệu từ lệnh `edit_*`.

**Tech Stack:** Rust 2024, crate `windows` 0.62 (Media Foundation, Direct3D11, Direct2D, DirectWrite), Tauri 2, React 19 + TypeScript.

**Spec:** `docs/superpowers/specs/2026-11-12-record-upgrades-design.md`. Kế hoạch này chỉ phủ hàng 1 và 2 của mục 7 trong spec. Các bước 3–7 có kế hoạch riêng, viết sau khi người dùng thử xong bước 2.

## Global Constraints

- Không bao giờ sửa, ghi đè hay xóa file gốc. Bản xuất là `<tên gốc>_edit.mp4`; trùng thì `_edit_2`, `_edit_3` (dùng `naming::unique_path`).
- Không đóng gói FFmpeg. Phụ thuộc mới chỉ được là MIT/Apache/BSD. Bước 1–2 không thêm crate mới, chỉ thêm feature của `windows`.
- Không sửa `src/windows/SnipOverlay.tsx`, `src-tauri/src/snip.rs`. Các file dùng chung (`commands.rs`, `lib.rs`, `api.ts`, `theme.css`, `settings.rs`, `Cargo.toml`, `App.tsx`, `main.tsx`, `LibraryPage.tsx`) chỉ được thêm dòng, không sắp xếp lại; feature mới của `windows` thêm vào cuối danh sách.
- Không giả lập chuột/phím trên máy người dùng. Test chạm màn hình hay camera phải mang `#[ignore]`.
- Giao diện tiếng Việt, nền tối, dùng biến màu trong `theme.css`.
- File xuất: H.264 + AAC 48 kHz stereo, giữ fps nguồn, bitrate `quality::video_bitrate(w, h, fps, settings.quality)`.
- Thông số hiệu ứng cố định (spec 4.3):
  - vòng gợn 480 ms, bán kính cuối bằng 2,5% chiều cao video nguồn;
  - màu: trái `#FF5A36`, phải `#3B82F6`, giữa `#22C55E`;
  - biểu tượng cuộn chuột giữ 400 ms sau lần cuộn cuối;
  - chip phím hiện 1500 ms, hiện ra và mờ đi mỗi bên 150 ms, cách mép dưới khung xuất 8%.
- Chỗ nối giữa hai đoạn giữ lại: tiếng làm nhỏ dần rồi to dần trong 10 ms. Phần giữ lại ngắn hơn 500 ms thì không cho xuất.
- Mọi API Media Foundation và Direct2D phải được tra trên learn.microsoft.com và docs.rs (`windows` 0.62) trước khi dùng. Kế hoạch này không ghim tên hàm của thư viện ngoài.
- Mỗi task kết thúc bằng một commit, thông điệp kết thúc bằng dòng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Lệnh kiểm phải đạt ở cuối mỗi task có chạm tới phần đó: `cargo test -p hp-core`, `cargo test -p hp-screen --lib`, `pnpm tsc --noEmit`.

## Review Focus

1. **Video không do HP Screen quay**: không có nhật ký, không có tiếng, tiếng 44,1 kHz, hoặc fps 29,97. Mong đợi: vẫn mở và xuất được; tắt hiệu ứng; tiếng được đổi tần số mẫu. Kích thước lẻ được làm chẵn. Test ở Task 3, Task 5 và Task 6.
2. **Nhật ký có header khác kích thước video**, hoặc có dòng hỏng. Mong đợi: tọa độ quy đổi theo tỉ lệ; dòng hỏng bị bỏ qua. Test ở Task 10 và Task 11.
3. **Hủy xuất giữa chừng, lỗi giữa chừng, hoặc tên xuất đã tồn tại.** Mong đợi: không để lại file dở; không ghi đè; tên mới là `_edit_2`. Test ở Task 6.
4. **Tên file có dấu tiếng Việt** (ví dụ `bài giảng.mp4`). Mong đợi: mở, xuất, đọc nhật ký đều chạy. Test ở Task 6.
5. **Đổi tên hoặc xóa một video đã có file chỉnh sửa.** Mong đợi: `.edit.json` và `.preview.mp4` đi theo video. Test ở Task 2.

## File Structure

```
crates/hp-core/src/
  cut.rs        (mới)  các đoạn giữ lại, quy đổi thời gian, độ lớn tiếng ở chỗ nối
  edit.rs       (mới)  EditDoc, đọc/ghi .edit.json
  effects.rs    (mới)  nhật ký → vòng click / giữ nút / cuộn / chip; trạng thái tại thời điểm t
  plan.rs       (mới)  khung xuất thứ n → thời điểm nguồn + danh sách thứ cần vẽ
  input_log.rs  (sửa)  thêm parse_log
  library.rs    (sửa)  edit_path, preview_path, companion_paths có 5 phần tử
  lib.rs        (sửa)  khai báo module mới
src-tauri/src/
  decoder.rs    (mới)  Source Reader: khung BGRA, PCM float, tua
  compositor.rs (mới)  Direct2D: dán khung, vẽ Draw, đọc ngược BGRA
  export.rs     (mới)  việc xuất: luồng, tiến độ, hủy, file .part
  edit_cmds.rs  (mới)  lệnh Tauri edit_* và export_*
  commands.rs   (sửa)  library_item thành pub(crate)
  pipeline.rs   (sửa)  save_thumbnail thành pub(crate)
  lib.rs        (sửa)  mod + đăng ký lệnh + manage(ExportState)
src/
  editApi.ts                (mới)  kiểu dữ liệu + hàm gọi lệnh edit_*/export_*
  pages/EditPage.tsx        (mới)  trang Chỉnh sửa: trạng thái, lưu tự động, hoàn tác, phím tắt
  editor/Timeline.tsx       (mới)  rãnh Video: tay cắt, đoạn bỏ, vùng chọn, đầu phát
  editor/Preview.tsx        (mới)  <video> + canvas, nhảy qua đoạn bỏ
  editor/drawEffects.ts     (mới)  vẽ hiệu ứng lên canvas
  editor/ExportPanel.tsx    (mới)  nút Xuất, tiến độ, Hủy, kết quả
  App.tsx, pages/LibraryPage.tsx, theme.css (sửa, chỉ thêm)
```

---

## BƯỚC 1 — Nền xuất và cắt nhanh

### Task 1: `hp-core::cut`

**Files:** Create `crates/hp-core/src/cut.rs` (có test trong file); Modify `crates/hp-core/src/lib.rs`.

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)] pub struct Span { pub start_ms: i64, pub end_ms: i64 }` (nửa mở `[start, end)`)
  - `pub struct Cuts`, có các hàm:
    - `pub fn new(duration_ms: i64, trim_start_ms: i64, trim_end_ms: Option<i64>, removed: &[Span]) -> Cuts`
    - `pub fn keep(&self) -> &[Span]`
    - `pub fn output_ms(&self) -> i64`
    - `pub fn to_source(&self, out_ms: i64) -> Option<i64>`
    - `pub fn to_output(&self, src_ms: i64) -> Option<i64>`
    - `pub fn join_gain(&self, src_ms: f64) -> f32`
  - `pub const JOIN_FADE_MS: f64 = 10.0;`
  - `pub const MIN_OUTPUT_MS: i64 = 500;`

- [ ] **Step 1: Viết test hỏng.** Dùng `let c = Cuts::new(10_000, 1_000, Some(9_000), &[Span{start_ms:3_000,end_ms:5_000}]);`.
  - `keep_spans_are_trim_minus_removed`: `c.keep() == [Span{1000,3000}, Span{5000,9000}]`, `c.output_ms() == 6000`.
  - `output_time_maps_to_source`: `to_source(0)==Some(1000)`, `to_source(1999)==Some(2999)`, `to_source(2000)==Some(5000)`, `to_source(5999)==Some(8999)`, `to_source(6000)==None`, `to_source(-1)==None`.
  - `source_time_maps_to_output`: `to_output(4000)==None`, `to_output(5000)==Some(2000)`, `to_output(500)==None`, `to_output(9000)==None`.
  - `overlapping_and_outside_removals_are_merged_and_clipped`: `Cuts::new(10_000, 0, None, &[{2000,4000},{3000,6000},{9500,20000},{-50,100}])`, kết quả `keep == [{100,2000},{6000,9500}]`.
  - `reversed_or_empty_spans_are_ignored`: bỏ `{5000,5000}` và `{6000,4000}` thì `keep == [{0,10000}]`.
  - `trim_end_past_the_video_is_clamped`: `Cuts::new(4000, 0, Some(99_999), &[]).output_ms() == 4000`.
  - `cutting_everything_leaves_nothing`: `Cuts::new(4000, 3000, Some(1000), &[]).keep().is_empty()` và `output_ms()==0`.
  - `audio_fades_only_around_joins`:
    - `join_gain(2000.0)==1.0`;
    - `join_gain(2995.0)` gần bằng `0.5` (±0.01);
    - `join_gain(5000.0)==0.0`;
    - `join_gain(5005.0)` gần bằng `0.5`;
    - `join_gain(1000.0)==1.0` và `join_gain(8999.0)==1.0`: điểm đầu đoạn giữ đầu tiên và điểm cuối đoạn giữ cuối cùng không mờ, vì đó không phải chỗ nối.
- [ ] **Step 2:** `cargo test -p hp-core cut`. Mong đợi: FAIL (module chưa có).
- [ ] **Step 3: Cài đặt.**
  - Chuẩn hóa: kẹp mọi đoạn vào `[trim_start, min(trim_end, duration))`, sắp theo `start`, gộp các đoạn chồng nhau, rồi lấy phần bù.
  - `join_gain`: tuyến tính, `= min(1, khoảng cách tới mép chỗ nối gần nhất / JOIN_FADE_MS)`.
- [ ] **Step 4:** `cargo test -p hp-core`. Mong đợi: PASS.
- [ ] **Step 5:** Commit `feat(core): cut spans and time mapping for trimming`.

### Task 2: `hp-core::edit` và file đi kèm trong `library`

**Files:** Create `crates/hp-core/src/edit.rs`; Modify `crates/hp-core/src/library.rs`, `crates/hp-core/src/lib.rs`, `src-tauri/src/library_fs.rs` (chỉ phần test, nếu kiểu mảng đổi).

**Interfaces:**
- Consumes: `cut::{Span, Cuts}`.
- Produces:
  - `pub const EDIT_VERSION: u32 = 1;`
  - `#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)] #[serde(default)] pub struct EditDoc { pub v: u32, pub trim: Trim, pub cuts: Vec<Span>, pub effects: EffectToggles }`
  - `pub struct Trim { pub start_ms: i64, pub end_ms: Option<i64> }`
  - `pub struct EffectToggles { pub clicks: bool, pub keys: bool }`, mặc định `true, true`.
  - `impl EditDoc { pub fn cuts(&self, duration_ms: i64) -> Cuts }`
  - `pub enum LoadError { Newer(u32), Invalid }`
  - `pub fn load_edit(json: &str) -> Result<EditDoc, LoadError>`
  - `pub fn save_edit(doc: &EditDoc) -> String` (JSON có thụt lề)
  - Trong `library`: `pub fn edit_path(media: &Path) -> PathBuf` (đuôi `edit.json`), `pub fn preview_path(media: &Path) -> PathBuf` (đuôi `preview.mp4`). `companion_paths` trả `[PathBuf; 5]`, theo thứ tự: input, thumb, meta, edit, preview.

- [ ] **Step 1: Viết test hỏng.**
  - `missing_fields_take_defaults`: `load_edit("{\"v\":1}")` cho `trim.start_ms==0`, `trim.end_ms==None`, `cuts` rỗng, `effects.clicks && effects.keys`.
  - `round_trip_keeps_everything`: `load_edit(&save_edit(&doc)) == Ok(doc)`, với doc có 2 cuts và `keys=false`.
  - `newer_version_is_refused`: `load_edit("{\"v\":2}") == Err(LoadError::Newer(2))`.
  - `garbage_is_invalid`: `load_edit("not json") == Err(LoadError::Invalid)`.
  - `doc_cuts_use_trim_and_removed_spans`: `EditDoc{trim:{1000,Some(9000)}, cuts:[{3000,5000}], ..}.cuts(10_000).output_ms()==6000`.
  - Trong `library`: `edit_and_preview_files_are_companions`. `companion_paths("d/hp_a.mp4")[3]` là `d/.hpscreen/hp_a.edit.json`, `[4]` là `d/.hpscreen/hp_a.preview.mp4`.
- [ ] **Step 2:** `cargo test -p hp-core`. Mong đợi: FAIL.
- [ ] **Step 3: Cài đặt.**
  - `load_edit`: đọc `v` trước; `v > EDIT_VERSION` thì trả `Newer`; sau đó mới đọc cả struct.
- [ ] **Step 4:** `cargo test -p hp-core` và `cargo test -p hp-screen --lib library_fs`. Mong đợi: PASS. Test đổi tên/xóa hiện có lặp qua `companion_paths`, nên đã phủ luôn 5 file.
- [ ] **Step 5:** Commit `feat(core): edit document and its companion files`.

### Task 3: `decoder.rs`

**Files:** Create `src-tauri/src/decoder.rs`; Modify `src-tauri/src/lib.rs` (`mod decoder;`), `src-tauri/Cargo.toml` (feature `windows` nếu cần thêm).

**Interfaces:**
- Consumes: `encoder::{init_thread, Mp4Encoder, EncoderConfig, AUDIO_SAMPLE_RATE, AUDIO_CHANNELS}` (encoder dùng để tạo video mẫu trong test); `hp_core::mix::{Resampler, to_stereo}` (tên đúng theo `mix.rs`).
- Produces:
  - `#[derive(Clone, Debug, Serialize)] pub struct VideoInfo { pub width: u32, pub height: u32, pub duration_ms: i64, pub fps: u32, pub has_audio: bool }`
  - `pub struct VideoDecoder`:
    - `pub fn open(path: &Path) -> Result<(VideoDecoder, VideoInfo), String>`
    - `pub fn frame_at(&mut self, src_ms: i64) -> Result<&[u8], String>`: BGRA top-down, stride `width*4`. Trả khung cuối cùng có mốc thời gian ≤ `src_ms`. Nếu `src_ms` nhỏ hơn khung hiện tại, hoặc lớn hơn khung hiện tại quá 1000 ms, thì tua.
  - `pub struct AudioDecoder`:
    - `pub fn open(path: &Path) -> Result<Option<AudioDecoder>, String>`: `None` nếu không có tiếng.
    - `pub fn next_chunk(&mut self) -> Result<Option<(f64, Vec<f32>)>, String>`: `(ms nguồn của mẫu đầu, PCM float stereo 48 kHz xen kẽ)`, đã đổi tần số và số kênh khi nguồn khác.

- [ ] **Step 1: Viết test hỏng** (không `#[ignore]`: chỉ cần Media Foundation, không cần màn hình).
  - Helper `make_sample(path, secs, audio)`: ghi video 320×240, 30 fps bằng `Mp4Encoder`.
    - Khung thứ n tô kín một màu: đỏ nếu `n/30 == 0`, xanh lá nếu `== 1`, xanh dương nếu `== 2`.
    - Có `audio` thì ghi sóng sin 440 Hz.
  - `opens_a_recording_and_reports_its_size`: `VideoInfo{width:320,height:240,fps:30,has_audio:true}`, `duration_ms` trong `2900..=3100`.
  - `frame_at_returns_the_frame_for_that_time`: pixel giữa ảnh của `frame_at(500)` có kênh R > 180 và G, B < 80. Tương tự `frame_at(1500)` cho xanh lá, `frame_at(2500)` cho xanh dương, rồi `frame_at(500)` lần nữa (tua ngược) lại cho đỏ.
  - `audio_decodes_to_48k_stereo`: tổng số mẫu của mọi chunk chia 2 nằm trong khoảng 48 000 × 3 ± 4 800.
  - `video_without_audio_has_no_audio_decoder`: `AudioDecoder::open` trả `Ok(None)`.
  - `vietnamese_file_names_open`: file `bài giảng.mp4` mở được.
  - Hai hàm thuần, test trực tiếp không cần file:
    - `fn fps_from_ratio(num: u32, den: u32) -> u32`. Test: `(30000,1001)` ra 30; `(60,1)` ra 60; `(0,0)` ra 1.
    - `fn to_48k_stereo(pcm: &[f32], rate: u32, channels: usize, rs: &mut Resampler) -> Vec<f32>`. Test: 1 s mono 44 100 Hz ra 96 000 ± 200 số float.
- [ ] **Step 2:** `cargo test -p hp-screen --lib decoder`. Mong đợi: FAIL.
- [ ] **Step 3: Cài đặt.**
  - Video: Source Reader, bật `MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING`, kiểu ra `MFVideoFormat_RGB32`.
  - Đọc buffer qua `IMF2DBuffer::Lock2D` và chép từng hàng theo pitch. Pitch âm nghĩa là ảnh lưu từ dưới lên.
  - fps lấy từ `MF_MT_FRAME_RATE`, làm tròn về số nguyên gần nhất, tối thiểu 1.
  - Thời lượng lấy từ `MF_PD_DURATION`.
  - Âm thanh: kiểu ra `MFAudioFormat_Float`. Nếu tần số khác 48 kHz hoặc số kênh khác 2 thì đưa qua bộ resampler và `to_stereo` của `hp_core::mix`.
- [ ] **Step 4:** `cargo test -p hp-screen --lib decoder`. Mong đợi: PASS.
- [ ] **Step 5:** Commit `feat: video and audio decoder on Media Foundation`.

### Task 4: `compositor.rs` (dán khung, chưa vẽ hiệu ứng)

**Files:** Create `src-tauri/src/compositor.rs`, `crates/hp-core/src/effects.rs` (tạm chỉ chứa `pub enum Draw {}`; khai báo trong `hp-core/src/lib.rs`); Modify `src-tauri/Cargo.toml`: thêm vào cuối danh sách feature của `windows` các mục `Win32_Graphics_Direct3D`, `Win32_Graphics_Direct3D11`, `Win32_Graphics_Dxgi`, `Win32_Graphics_Dxgi_Common`, `Win32_Graphics_Direct2D`, `Win32_Graphics_Direct2D_Common`, `Win32_Graphics_DirectWrite`, cùng các mục docs.rs báo còn thiếu. Modify `src-tauri/src/lib.rs`.

**Interfaces:**
- Produces:
  - `pub struct Compositor`:
    - `pub fn new(out_w: u32, out_h: u32, device: Device) -> Result<Compositor, String>`
    - `pub fn render(&mut self, src: &[u8], src_w: u32, src_h: u32, draws: &[hp_core::effects::Draw]) -> Result<&[u8], String>`: trả BGRA top-down, stride `out_w*4`.
  - `pub enum Device { Hardware, Warp }`. `Hardware` tự rơi về WARP nếu không tạo được thiết bị phần cứng.
- Ở task này `draws` luôn rỗng. Kiểu `Draw` do Task 10 định nghĩa; trong lúc chờ, tạo `pub enum Draw {}` rỗng trong `hp-core::effects` để code biên dịch được.

- [ ] **Step 1: Viết test hỏng** (WARP, không `#[ignore]`).
  - `same_size_render_keeps_the_picture`: nguồn 64×48 nửa trái đỏ, nửa phải xanh dương; ra 64×48. Pixel (10,24) đỏ, pixel (54,24) xanh dương, sai số ±2.
  - `render_scales_to_the_output_size`: nguồn 64×48 một màu xanh lá, ra 128×96. Mọi pixel kiểm tại 5 điểm đều xanh lá.
- [ ] **Step 2:** `cargo test -p hp-screen --lib compositor`. Mong đợi: FAIL.
- [ ] **Step 3: Cài đặt.**
  - D3D11 device có cờ BGRA support, rồi D2D device context.
  - Bitmap đích `out_w×out_h` (target), và một bitmap `CPU_READ | CANNOT_DRAW` để đọc ngược.
  - Bitmap nguồn tạo lại khi kích thước nguồn đổi; nạp bằng `CopyFromMemory`.
  - Vẽ nguồn phủ kín đích bằng nội suy high-quality cubic.
  - `CopyFromBitmap` sang bitmap đọc, `Map`, rồi chép từng hàng.
- [ ] **Step 4:** `cargo test -p hp-screen --lib compositor`. Mong đợi: PASS.
- [ ] **Step 5:** Commit `feat: Direct2D compositor for export frames`.

### Task 5: `hp-core::plan` (chưa có hiệu ứng)

**Files:** Create `crates/hp-core/src/plan.rs`; Modify `crates/hp-core/src/lib.rs`.

**Interfaces:**
- Consumes: `edit::EditDoc`, `cut::Cuts`.
- Produces:
  - `pub struct Source { pub width: u32, pub height: u32, pub duration_ms: i64, pub fps: u32 }`
  - `pub struct Plan`:
    - `pub fn new(src: Source, doc: &EditDoc) -> Plan`
    - `pub fn frames(&self) -> u64`, bằng `floor(output_ms * fps / 1000)`
    - `pub fn out_size(&self) -> (u32, u32)`: ở bước này bằng kích thước nguồn sau `geometry::make_even`.
    - `pub fn frame(&self, n: u64) -> Option<FramePlan>`
    - `pub fn cuts(&self) -> &Cuts`
  - `pub struct FramePlan { pub out_ms: i64, pub src_ms: i64, pub draws: Vec<effects::Draw> }`
  - `pub fn can_export(&self) -> bool`, bằng `output_ms >= MIN_OUTPUT_MS`.

- [ ] **Step 1: Viết test hỏng.** Nguồn 10 000 ms, 30 fps, doc như Task 1.
  - `frame_count_follows_output_length`: `frames() == 180`.
  - `frame_times_skip_the_removed_span`: `frame(0).src_ms == 1000`; `frame(59).src_ms == 2966`; `frame(60).src_ms == 5000`; `frame(180) == None`.
  - `too_short_cannot_export`: trim 0..400 thì `can_export() == false`; trim 0..500 thì `true`.
  - `odd_sources_give_even_output`: nguồn 641×481 thì `out_size() == (640, 480)`.
- [ ] **Step 2:** `cargo test -p hp-core plan`. Mong đợi: FAIL.
- [ ] **Step 3: Cài đặt.** `out_ms = n * 1000 / fps` (làm tròn xuống), `src_ms = cuts.to_source(out_ms)`.
- [ ] **Step 4:** `cargo test -p hp-core`. Mong đợi: PASS.
- [ ] **Step 5:** Commit `feat(core): export plan maps output frames to source times`.

### Task 6: `export.rs`

**Files:** Create `src-tauri/src/export.rs`; Modify `src-tauri/src/pipeline.rs` (`save_thumbnail` thành `pub(crate)`), `src-tauri/src/lib.rs`.

**Interfaces:**
- Consumes:
  - `decoder::{VideoDecoder, AudioDecoder, VideoInfo}`, `compositor::{Compositor, Device}`, `plan::Plan`;
  - `encoder::{Mp4Encoder, EncoderConfig}`;
  - `library_fs::write_meta`, `hp_core::library::{MediaMeta, DATA_DIR}`, `hp_core::naming::unique_path`;
  - `pipeline::save_thumbnail`.
- Produces:
  - `pub fn export_path(source: &Path) -> PathBuf`: `<dir>/<stem>_edit.mp4`, hoặc tên kế tiếp còn trống.
  - `pub fn run_export(source: &Path, out: &Path, plan: &Plan, bitrate: u32, progress: &mut dyn FnMut(u64, u64), cancel: &AtomicBool) -> Result<(), String>`
- Cách ghi:
  - Ghi vào `<dir>/.hpscreen/<tên xuất>.part`.
  - Thành công thì đổi tên sang `out`, rồi ghi thumbnail và meta.
  - Lỗi hoặc bị hủy thì xóa `.part` và trả `Err("Đã hủy")` hoặc thông điệp lỗi.
  - Khi `plan.frame(n).draws` rỗng và kích thước ra bằng kích thước nguồn thì ghi thẳng khung đã giải mã, bỏ qua compositor.

- [ ] **Step 1: Viết test hỏng** (dùng `make_sample` của Task 3, đặt `pub(crate)` trong `decoder::tests`, hoặc chuyển sang một module test dùng chung).
  - `export_drops_the_removed_span`:
    - Mẫu 3 s có tiếng; doc bỏ đoạn `{1000,2000}`.
    - `encoder::probe(out)`: số khung trong `59..=61`; khung cuối < 2,0 s; số mẫu tiếng > 0.
    - `VideoDecoder` đọc lại file xuất: `frame_at(500)` đỏ, `frame_at(1500)` xanh dương; không còn khung xanh lá nào.
  - `export_never_touches_the_source`: kích thước và thời điểm sửa của nguồn không đổi.
  - `export_name_does_not_overwrite`: đã có `a_edit.mp4` thì `export_path("a.mp4")` ra `a_edit_2.mp4`.
  - `cancelled_export_leaves_no_file`: `cancel` đặt `true` từ đầu thì trả `Err`, `out` không tồn tại, thư mục `.hpscreen` không có `.part`.
  - `video_without_audio_exports`: mẫu không tiếng xuất được, `probe` cho 0 mẫu tiếng.
  - `vietnamese_names_export`: `bài giảng.mp4` xuất ra `bài giảng_edit.mp4`.
- [ ] **Step 2:** `cargo test -p hp-screen --lib export`. Mong đợi: FAIL.
- [ ] **Step 3: Cài đặt.**
  - Hình: với n từ 0 tới `frames-1`, lấy `frame_at(src_ms)`, đưa qua compositor nếu cần, rồi `write_video(.., n * 10_000_000 / fps)`.
  - Tiếng: đọc tuần tự từng chunk. Với mỗi mẫu tại ms nguồn `s`, `to_output(s)` trả `None` thì bỏ mẫu; ngược lại nhân với `join_gain(s)`. Gom các mẫu liên tiếp thành từng đoạn liền để `write_audio` với thời điểm xuất tương ứng.
  - Gọi `progress(n+1, frames)` mỗi 10 khung và ở khung cuối.
- [ ] **Step 4:** `cargo test -p hp-screen --lib`. Mong đợi: PASS.
- [ ] **Step 5:** Commit `feat: export trimmed video to a new file`.

### Task 7: Lệnh Tauri `edit_*` / `export_*`

**Files:** Create `src-tauri/src/edit_cmds.rs`; Modify `src-tauri/src/commands.rs` (`fn library_item` thành `pub(crate) fn`), `src-tauri/src/lib.rs` (thêm `mod edit_cmds;`, `app.manage(edit_cmds::ExportState::default())`, và đăng ký các lệnh ở cuối `generate_handler!`), `src-tauri/src/events.rs` (thêm hằng sự kiện ở cuối).

**Interfaces:**
- Lệnh:
  - `edit_open(path: String) -> EditSession`, với `#[derive(Serialize)] EditSession { info: VideoInfo, doc: EditDoc, has_log: bool, warning: Option<String> }`.
    - `.edit.json` hỏng thì dùng doc mặc định và đặt `warning = "File chỉnh sửa bị hỏng, bắt đầu lại từ đầu"`.
    - Gặp `Newer` thì trả `Err("File chỉnh sửa do bản HP Screen mới hơn tạo")`.
  - `edit_save(path: String, doc: EditDoc) -> ()`: ghi `edit.json.tmp` rồi đổi tên đè lên.
  - `edit_keep(path: String, doc: EditDoc) -> KeepInfo`, với `KeepInfo { keep: Vec<Span>, output_ms: i64, can_export: bool }`.
  - `export_start(path: String, doc: EditDoc) -> String`: trả đường dẫn sẽ ghi.
    - Từ chối nếu recorder không ở trạng thái `Idle`: "Đang quay, hãy xuất sau khi dừng".
    - Từ chối nếu đang có một việc xuất khác: "Đang xuất một video khác".
  - `export_cancel() -> ()`.
- Sự kiện:
  - `export-progress {done: u64, total: u64}`;
  - `export-done {path: String}` (kèm `library_changed`);
  - `export-error {message: String}`.
- `ExportState { running: Mutex<Option<Arc<AtomicBool>>> }`.
- Mọi `path` đi qua `commands::library_item`. Chỉ nhận video: ảnh thì trả "Chỉ chỉnh sửa được video".
- `has_log` = `sidecar_path(path)` có tồn tại.
- `edit_open` mở `VideoDecoder` không được thì trả `Err("Không đọc được video này")` và không tạo `.edit.json`.
- `export_start` xóa các file `*.part` còn sót trong `.hpscreen`, do lần thoát app trước để lại.
- Khi app thoát (`RunEvent::Exit`), đặt cờ hủy của việc xuất đang chạy.

- [ ] **Step 1: Viết test hỏng** cho phần thuần trong `edit_cmds`:
  - `fn load_doc(media: &Path) -> Result<(EditDoc, Option<String>), String>`
  - `fn save_doc(media: &Path, doc: &EditDoc) -> Result<(), String>`
  - Test `save_then_load_round_trips`, `corrupt_file_gives_defaults_with_warning`, `newer_file_is_refused`, `save_leaves_no_tmp_file`. Chạy trong thư mục tạm.
- [ ] **Step 2:** `cargo test -p hp-screen --lib edit_cmds`. Mong đợi: FAIL.
- [ ] **Step 3:** Cài đặt hai hàm và các lệnh. Việc xuất chạy trên `std::thread::spawn`; `bitrate` lấy từ `quality::video_bitrate(out_w, out_h, fps, settings.quality)`.
- [ ] **Step 4:** `cargo test -p hp-screen --lib`. Mong đợi: PASS. Thêm `cargo build -p hp-screen` biên dịch được.
- [ ] **Step 5:** Commit `feat: edit and export commands`.

### Task 8: Trang Chỉnh sửa và cắt nhanh

**Files:** Create `src/editApi.ts`, `src/pages/EditPage.tsx`, `src/editor/Timeline.tsx`, `src/editor/Preview.tsx`, `src/editor/ExportPanel.tsx`; Modify `src/App.tsx`, `src/pages/LibraryPage.tsx`, `src/theme.css`. Mọi chỗ sửa đều là thêm.

**Interfaces:**
- `editApi.ts`:
  - Kiểu khớp với Rust: `VideoInfo`, `Span`, `EditDoc`, `EditSession`, `KeepInfo`.
  - Hàm gọi lệnh: `editOpen`, `editSave`, `editKeep`, `exportStart`, `exportCancel`.
  - Hàm nghe sự kiện: `onExportProgress`, `onExportDone`, `onExportError`.
- `App.tsx`:
  - Thêm state `editing: string | null`. Khác `null` thì `<main>` hiện `<EditPage path={editing} onClose={() => setEditing(null)} />`.
  - `LibraryPage` nhận prop tùy chọn `onEdit?: (path: string) => void`.
- `LibraryPage`: thẻ video có thêm nút "Chỉnh sửa", đặt trước nút "Thư mục".
- `Preview`:
  - `<video src={convertFileSrc(path)}>`.
  - Khi phát, mỗi khung (`requestVideoFrameCallback`) kiểm: `currentTime` nằm trong đoạn bỏ hoặc trước điểm cắt đầu thì nhảy tới đầu đoạn giữ kế tiếp; qua điểm cắt cuối thì dừng.
  - Video lỗi khi nạp (`error`) thì báo lên cha. Task 9 xử lý tiếp.
- `Timeline`:
  - Thanh ngang thể hiện toàn bộ video gốc, có hai tay kéo cắt đầu/cuối và đầu phát.
  - Đoạn bỏ tô sọc; kéo chuột trên thanh để tạo vùng chọn.
  - Bấm vào một đoạn bỏ thì hiện nút "Giữ lại".
- Trang:
  - Thẻ Cắt có nút "Bỏ đoạn này" (bật khi có vùng chọn).
  - `ExportPanel`: nút Xuất (khóa khi `!can_export`), thanh tiến độ, nút Hủy, dòng "Đã lưu <tên>" kèm nút Phát / Thư mục (dùng `libraryOpen` / `libraryReveal`).
  - Phím tắt theo spec 4.1, trừ Delete cho đoạn zoom.
  - Hoàn tác / làm lại tối đa 100 bước.
  - Lưu tự động bằng `editSave`, 1 giây sau thay đổi cuối; lưu ngay khi bấm "← Thư viện".
  - Sau mỗi thay đổi gọi `editKeep` để cập nhật `keep` và nút Xuất.

- [ ] **Step 1:** `pnpm tsc --noEmit`. Mong đợi: PASS, sạch.
- [ ] **Step 2:** Kiểm trên bản dev đang chạy (hỏi người dùng trước khi khởi động lại `pnpm tauri dev`; không giả lập chuột/phím): người dùng mở Thư viện, bấm "Chỉnh sửa", và trang hiện ra. Ghi vào sổ ghi điều đã kiểm được và điều chưa kiểm được.
- [ ] **Step 3:** Commit `feat: edit page with quick trim and export`.

### Task 9: WebView có phát được MP4 phân mảnh không, và đường dự phòng

**Files:** Modify `src-tauri/src/edit_cmds.rs`, `src/editor/Preview.tsx`, `src/editApi.ts`; có thể thêm `src-tauri/src/remux.rs`.

- [ ] **Step 1:** Nhờ người dùng mở trang Chỉnh sửa với một bản quay thật, phát, rồi tua vào giữa. Ghi kết quả vào `.superpowers/sdd/2026-11-12-record-upgrades/progress.md`.
- [ ] **Step 2:** Nếu phát và tua đều được: ghi `Ruling: không cần preview.mp4` và bỏ qua Step 3–5.
- [ ] **Step 3: Viết test hỏng.** `remux_writes_a_playable_copy`: `remux(make_sample(3s, audio), out)`, rồi `probe(out)` cho cùng số khung và cùng số mẫu tiếng với nguồn.
- [ ] **Step 4: Cài đặt.**
  - `pub fn remux(src: &Path, dst: &Path) -> Result<(), String>`: Source Reader ở chế độ không giải mã (để kiểu ra là kiểu nén gốc), Sink Writer với container `MFTranscodeContainerType_MPEG4` và cùng media type, chép nguyên mẫu.
  - Lệnh `edit_make_preview(path) -> String` trả đường dẫn `preview_path`; nếu file đã có thì dùng lại.
  - `Preview` gặp `error` thì gọi lệnh này một lần rồi nạp lại.
- [ ] **Step 5:** Chạy lại các lệnh kiểm. Commit `fix: plain MP4 preview copy when the WebView cannot play fragmented files`.

**Điểm dừng bước 1:**
- Chạy `cargo test -p hp-core`, `cargo test -p hp-screen --lib`, `pnpm tsc --noEmit`.
- Gửi người dùng danh sách thử:
  - mở trang Chỉnh sửa;
  - kéo tay cắt đầu/đuôi;
  - bỏ một đoạn giữa, rồi bấm Giữ lại;
  - hoàn tác;
  - xuất ra file `_edit`, phát thử và nghe chỗ nối;
  - file gốc còn nguyên;
  - hủy xuất giữa chừng;
  - đổi tên video đã chỉnh sửa rồi mở lại trang Chỉnh sửa.

---

## BƯỚC 2 — Hiệu ứng click và chip phím

### Task 10: Đọc nhật ký (`input_log::parse_log`)

**Files:** Modify `crates/hp-core/src/input_log.rs`: thêm `Deserialize` cho `Header`, `Button`, `Event`, và thêm hàm.

**Interfaces:**
- Produces:
  - `pub struct ParsedLog { pub header: Option<Header>, pub events: Vec<Event> }`
  - `pub fn parse_log(text: &str) -> ParsedLog`

- [ ] **Step 1: Viết test hỏng.**
  - `log_round_trips`: header cộng 4 sự kiện, ghi bằng `to_line` từng dòng, `parse_log` trả lại đúng như vậy.
  - `bad_and_unknown_lines_are_skipped`: chen một dòng `{oops`, một dòng `{"e":"teleport","t":5}` và một dòng trống thì vẫn còn đủ 4 sự kiện.
  - `newer_log_version_gives_nothing`: header `"v":2` thì `header==None` và `events` rỗng.
  - `events_are_sorted_by_time`: các dòng không theo thứ tự vẫn ra danh sách tăng dần theo `t` (sắp ổn định).
- [ ] **Step 2:** `cargo test -p hp-core input_log`. Mong đợi: FAIL.
- [ ] **Step 3:** Cài đặt. Dòng đầu là header nếu đọc được thành `Header`; các dòng sau đọc thành `Event`.
- [ ] **Step 4:** `cargo test -p hp-core`. Mong đợi: PASS.
- [ ] **Step 5:** Commit `feat(core): read input logs back`.

### Task 11: `hp-core::effects` và gắn vào `plan`

**Files:** Modify `crates/hp-core/src/effects.rs` (thay `enum Draw {}` rỗng của Task 4), `crates/hp-core/src/plan.rs`.

**Interfaces:**
- Consumes: `input_log::{ParsedLog, Event, Button}`, `edit::EffectToggles`.
- Produces:
  - Hằng số:
    - `pub const RIPPLE_MS: i64 = 480;`
    - `pub const RIPPLE_RADIUS: f32 = 0.025;` (phần của chiều cao nguồn)
    - `pub const WHEEL_HOLD_MS: i64 = 400;`
    - `pub const CHIP_MS: i64 = 1500;`
    - `pub const CHIP_FADE_MS: i64 = 150;`
    - `pub const CHIP_BOTTOM: f32 = 0.08;`
  - `#[derive(Clone, Debug, PartialEq, Serialize)] pub struct Effects`, gồm:
    - `pub ripples: Vec<Ripple { t_ms, x: f32, y: f32, button }>`
    - `pub holds: Vec<Hold { start_ms, end_ms, button }>`
    - `pub wheels: Vec<Wheel { start_ms, end_ms, up: bool }>`
    - `pub chips: Vec<Chip { start_ms, end_ms, text: String }>`
    - `pub cursor: Vec<(i64, f32, f32)>`
    - Tọa độ đã đổi sang pixel video nguồn.
  - `pub fn build_effects(log: &ParsedLog, video_w: u32, video_h: u32) -> Effects`: hệ số quy đổi là `video_w / header.width`; không có header thì là 1.
  - `impl Effects { pub fn cursor_at(&self, t_ms: i64) -> Option<(f32, f32)> }`: nội suy tuyến tính.
  - `pub fn draws_at(e: &Effects, t_ms: i64, on: &EffectToggles, video_h: u32) -> Vec<Draw>`
  - `#[derive(Clone, Debug, PartialEq, Serialize)] pub enum Draw`, gồm:
    - `Ring { x, y, radius: f32, alpha: f32, button }`
    - `Dot { x, y, radius, alpha, button }`
    - `Wheel { x, y, up: bool, alpha }`
    - `Chip { text: String, alpha: f32, bottom: f32 }`
    - Tọa độ của `Ring`, `Dot`, `Wheel` là pixel nguồn; `Chip` đặt theo khung xuất.
  - `plan`: `Plan::new` nhận thêm `effects: Option<Effects>`. `FramePlan.draws = draws_at(.., src_ms, ..)`.
- Quy tắc:
  - **Ring**: `p = (t - t0) / 480`, chỉ trong `[0, 1)`; `radius = R*(0.3 + 0.7*(1-(1-p)^2))`, với `R = 0.025*video_h`; `alpha = 1 - p`.
  - **Dot**: suốt quãng `[down, up)`, `radius = 0.35R`, `alpha = 0.55`, vị trí `cursor_at(t)`. Không có `Up` thì kết thúc ở sự kiện cuối cùng.
  - **Wheel**: mỗi sự kiện cuộn mở hoặc kéo dài một mốc tới `t + 400`; cùng chiều thì gộp, đổi chiều thì mở mốc mới; `alpha` giảm tuyến tính trong 150 ms cuối; `up = dy > 0`.
  - **Chip**:
    - `text` là `k` với mỗi `+` thay bằng ` + `.
    - Cùng `k` trong lúc chip còn hiện thì kéo dài `end` tới `t + 1500` và thêm ` ×N`.
    - `k` khác thì cắt chip cũ tại `t` và mở chip mới.
    - `alpha` tăng trong 150 ms đầu, giảm trong 150 ms cuối.

- [ ] **Step 1: Viết test hỏng.**
  - `left_click_makes_a_ring_that_grows_and_fades`: Down trái tại (100,50), t=1000, video cao 400 (R=10).
    - `draws_at(1000)` có `Ring{radius:3.0, alpha:1.0}`.
    - `draws_at(1240)` có `radius` gần bằng 8,25 và `alpha` gần bằng 0,5.
    - `draws_at(1480)` không còn `Ring`.
  - `ring_colours_follow_the_button`: chuột phải cho `button == Right`.
  - `holding_draws_a_dot_that_follows_the_cursor`: Down tại (0,0) t=0, Move tại (100,0) t=100, Up tại t=200. `draws_at(50)` có `Dot` ở x=50; `draws_at(200)` không còn `Dot`.
  - `wheel_shows_for_400ms_after_the_last_tick`: Wheel dy=+120 tại t=0 và t=300, thì `wheels == [{0,700,up:true}]`.
  - `repeated_shortcut_counts_up`: Key "Ctrl+S" tại t=0 và t=1000, thì `chips == [{0,2500,"Ctrl + S ×2"}]`. Key "Ctrl+Z" tại t=1200 thì chip trước kết thúc ở 1200.
  - `chip_fades_in_and_out`: `alpha` bằng 0 tại t=0, bằng 1 tại t=150, gần 0,5 tại t=1425.
  - `toggles_hide_effects`: `clicks=false` thì không có Ring/Dot/Wheel; `keys=false` thì không có Chip.
  - `coordinates_are_scaled_to_the_video`: header 200×100, video 400×200, click tại (50,50) thì `ripples[0]` ở (100,100).
  - `plan_draws_effects_at_source_time`: có cut trước click; khung xuất ứng với `src_ms == t0` phải có Ring.
- [ ] **Step 2:** `cargo test -p hp-core`. Mong đợi: FAIL.
- [ ] **Step 3:** Cài đặt theo quy tắc trên.
- [ ] **Step 4:** `cargo test -p hp-core`. Mong đợi: PASS.
- [ ] **Step 5:** Commit `feat(core): click ripples, hold dots, wheel marks and shortcut chips`.

### Task 12: Vẽ hiệu ứng trong `compositor` và đưa vào bản xuất

**Files:** Modify `src-tauri/src/compositor.rs`, `src-tauri/src/export.rs`, `src-tauri/src/edit_cmds.rs` (`export_start` đọc nhật ký, `build_effects`, truyền vào `Plan`).

**Interfaces:**
- Màu:
  - `Left = #FF5A36`, `Right = #3B82F6`, `Middle = #22C55E`.
  - Ring: đường viền dày `max(2, 0.12R)`.
  - Wheel: hình con chuột bo góc rộng 0,9R, cao 1,3R, đặt tại (x+R, y+R), có mũi tên lên/xuống, màu trắng, viền đen mảnh.
  - Chip: Segoe UI Semibold cao bằng 3,2% `out_h`, chữ trắng, nền `rgba(20,20,24,0.85)` bo góc bằng nửa chiều cao, padding ngang 0,8em; đặt giữa theo chiều ngang, mép dưới ở `out_h*(1-0.08)`.
- Tọa độ `Ring`, `Dot`, `Wheel` đổi từ pixel nguồn sang khung xuất bằng `out/src`.

- [ ] **Step 1: Viết test hỏng** (WARP).
  - `ring_is_drawn_in_its_colour`: nguồn 200×200 màu xám `#808080`; `Ring{x:100,y:100,radius:40,alpha:1,Left}`. Pixel (140,100) có R > 200 và B < 120; pixel (100,100) vẫn xám.
  - `chip_is_drawn_at_the_bottom_centre`: `Chip{"Ctrl + S",1,0.08}` trên nguồn trắng 400×300. Ít nhất một pixel trong dải y 262..276, x 180..220 tối (< 60).
  - `alpha_zero_draws_nothing`: ảnh ra giống hệt ảnh nguồn.
- [ ] **Step 2:** `cargo test -p hp-screen --lib compositor`. Mong đợi: FAIL.
- [ ] **Step 3:** Cài đặt. Bộ định dạng chữ DirectWrite tạo một lần trong `Compositor::new`.
- [ ] **Step 4:** `cargo test -p hp-screen --lib`. Mong đợi: PASS.
- [ ] **Step 5:** Commit `feat: draw click and key effects into exports`.

### Task 13: Hiệu ứng trong xem trước và thẻ Hiệu ứng

**Files:** Create `src/editor/drawEffects.ts`; Modify `src-tauri/src/edit_cmds.rs` (`EditSession` thêm `effects: Option<Effects>`), `src/editApi.ts`, `src/editor/Preview.tsx`, `src/pages/EditPage.tsx`.

**Interfaces:**
- `drawEffects(ctx: CanvasRenderingContext2D, fx: Effects, tMs: number, on: EffectToggles, videoH: number, scale: number): void`
  - Vẽ cùng quy tắc và hằng số như Task 11–12, với `scale` = kích thước canvas / kích thước nguồn.
  - Các hằng số chép sang TS phải ghi chú `// giống hp-core::effects`.
- Thẻ Hiệu ứng có hai công tắc "Vòng click" và "Phím tắt", ghi vào `doc.effects`.
- `has_log == false` thì thẻ hiện "Video này không có nhật ký thao tác" và khóa hai công tắc.

- [ ] **Step 1:** `pnpm tsc --noEmit` và `cargo test -p hp-screen --lib`. Mong đợi: PASS.
- [ ] **Step 2:** Commit `feat: preview click and key effects in the editor`.

**Điểm dừng bước 2:**
- Chạy đủ ba lệnh kiểm.
- Ghi sổ `progress.md` (các dòng Ruling, các điều chưa kiểm được).
- Gửi người dùng danh sách thử:
  - quay một đoạn có click trái/phải/giữa, kéo thả, cuộn, `Ctrl+C`/`Ctrl+V` lặp lại;
  - xem trước, rồi xuất; vòng click đúng chỗ, đúng lúc;
  - chip "×2";
  - tắt từng công tắc;
  - xem trước khớp bản xuất;
  - một video cũ không có nhật ký vẫn cắt được.
