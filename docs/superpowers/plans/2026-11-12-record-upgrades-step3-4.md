# Nâng cấp quay — Bước 3–4 (vẽ lên màn hình khi quay, webcam) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Trong lúc quay, người dùng vẽ bút / mũi tên / khung lên vùng đang quay, nét tự mờ; và bật webcam ở góc. Cả hai lọt vào video ở cả ba chế độ quay.

**Architecture:**
- Một "lớp trực tiếp" (`live_layer.rs`) giữ các nét vẽ và khung webcam mới nhất.
- `pipeline.rs` gọi `LiveLayer::apply` lên khung BGRA ngay trước `write_video`. Hàm này chỉ đưa vùng bao quanh nét và webcam lên Direct2D, vẽ, rồi đọc ngược về.
- Cửa sổ vẽ `draw` và bong bóng `cam` hiện trên màn hình nhưng bị loại khỏi capture, để không bị in hai lần vào video.
- Logic thuần (hình học nét, độ mờ, vị trí webcam, khung cắt camera) nằm trong `hp-core`.

**Tech Stack:** Rust; crate `windows` 0.62 (Direct2D, Media Foundation để đọc camera); Tauri 2; React. Thêm feature `jpeg` của crate `image` (MIT/Apache) để gửi ảnh nhỏ cho bong bóng.

**Spec:** `docs/superpowers/specs/2026-11-12-record-upgrades-design.md`, các mục 4.4 và 4.5, cùng các hàng tương ứng ở mục 5, 6, 9.

## Global Constraints

- Không giả lập chuột hay phím trên máy người dùng. Test chạm màn hình hoặc camera phải mang `#[ignore]`.
- Không sửa `SnipOverlay.tsx`, `snip.rs`. File dùng chung chỉ được thêm vào, không sắp xếp lại.
- Build ra `G:/hp-screen-build/record-upgrades` qua `.cargo/config.toml` (ổ F: đầy).
- Thông số vẽ, theo spec 4.4:
  - phím tắt mặc định `Ctrl+Alt+D` (trường cài đặt `hotkey_draw`);
  - nét giữ nguyên `draw_fade_secs` giây (chọn 0 / 2 / 3 / 5 / 10, mặc định 3; 0 nghĩa là không tự mờ), rồi mờ hết trong 500 ms;
  - 5 màu: đỏ `#EF4444`, vàng `#FACC15`, xanh lá `#22C55E`, xanh dương `#3B82F6`, trắng `#FFFFFF`;
  - 3 độ dày: 4, 8 và 14 px vật lý.
- Thông số webcam, theo spec 4.5:
  - cỡ 15 / 20 / 25% chiều cao video;
  - cách mép 2,5% chiều cao video;
  - mặc định góc dưới phải, lật gương;
  - viền trắng dày `max(2, 0,3% chiều cao)` px;
  - camera đọc ở độ phân giải gần nhất không quá 720p;
  - bong bóng nhận ảnh JPEG 240 px, 10 khung/giây.
- Lớp vẽ mất quá 8 ms cho một khung thì ghi một dòng vào `hp-screen.log` (tối đa một lần mỗi 10 giây).
- Commit kết thúc bằng dòng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Các lệnh kiểm phải đạt: `cargo test -p hp-core`, `cargo test -p hp-screen --lib`, `pnpm tsc --noEmit`, `pnpm test`.

## Review Focus

1. **Dừng quay khi đang ở chế độ vẽ, hoặc khi webcam đang bật.** Mong đợi: cửa sổ vẽ và bong bóng đóng, camera tắt (đèn camera tắt), nét vẽ bị xóa. Lần quay sau bắt đầu sạch. Test ở Task 4 và Task 10.
2. **Click chuột lúc đang vẽ.** Mong đợi: những click đó không thành vòng click khi xuất video. Test ở Task 4.
3. **Nét vẽ ra ngoài mép vùng quay, hoặc nét chỉ có một điểm.** Mong đợi: không lỗi, không ghi tràn bộ nhớ. Test ở Task 1 và Task 3.
4. **Camera bị app khác giữ, máy không có camera, hoặc Windows chặn quyền camera.** Mong đợi: báo lỗi tiếng Việt, việc quay vẫn tiếp tục. Test ở Task 8 và Task 10.
5. **Video nhỏ hơn bong bóng webcam** (vùng quay rất nhỏ). Mong đợi: webcam được thu nhỏ cho vừa và vẫn nằm trong khung. Test ở Task 7.

## File Structure

```
crates/hp-core/src/stroke.rs     (mới) công cụ, nét, đoạn thẳng để vẽ, độ mờ, vùng bao
crates/hp-core/src/webcam.rs     (mới) WebcamSettings, vị trí bong bóng, khung cắt camera
crates/hp-core/src/settings.rs   (sửa, thêm) hotkey_draw, draw_fade_secs, webcam
src-tauri/src/compositor.rs      (sửa) tách hàm tạo device context dùng chung
src-tauri/src/live_layer.rs      (mới) LiveLayer: nét, khung camera, vẽ lên canvas
src-tauri/src/camera.rs          (mới) liệt kê và mở camera bằng Media Foundation
src-tauri/src/live_cmds.rs       (mới) lệnh draw_* / cam_*, cửa sổ draw và cam
src-tauri/src/pipeline.rs        (sửa) PipelineConfig.live, gọi apply
src-tauri/src/recorder.rs        (sửa) giữ Arc<LiveLayer>, dọn khi dừng
src-tauri/src/input_hook.rs      (sửa) bỏ click khi đang vẽ
src-tauri/src/hotkeys.rs         (sửa, thêm) HotkeyAction::Draw
src-tauri/src/lib.rs             (sửa, thêm) mod và đăng ký lệnh
src-tauri/capabilities/default.json (sửa) thêm cửa sổ "draw", "cam"
src/liveApi.ts                   (mới)
src/windows/DrawOverlay.tsx      (mới)
src/windows/CamBubble.tsx        (mới)
src/windows/ControlBar.tsx, src/pages/RecordPage.tsx, src/pages/SettingsPage.tsx, src/main.tsx, src/api.ts, src/theme.css (sửa, thêm)
```

---

## BƯỚC 3 — Vẽ lên màn hình khi quay

### Task 1: `hp-core::stroke`

**Interfaces (Produces):**
- `#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "lowercase")] pub enum Tool { Pen, Arrow, Rect }`
- `#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)] pub struct Stroke`, gồm:
  - `pub id: u64`
  - `pub tool: Tool`
  - `pub color: [u8; 3]`
  - `pub width: f32`
  - `pub points: Vec<(f32, f32)>`: pixel vật lý, tương đối gốc vùng quay
  - `pub finished_ms: Option<i64>`
- Hằng số:
  - `pub const FADE_OUT_MS: i64 = 500;`
  - `pub const COLORS: [[u8; 3]; 5]`
  - `pub const WIDTHS: [f32; 3] = [4.0, 8.0, 14.0];`
- `pub fn segments(s: &Stroke) -> Vec<((f32, f32), (f32, f32))>`
  - Bút: các điểm liên tiếp.
  - Mũi tên: thân, cộng hai cạnh đầu mũi tên. Mỗi cạnh dài `max(12, 4*width)`, lệch 28° so với thân, tính từ điểm cuối.
  - Khung: 4 cạnh của hình chữ nhật từ điểm đầu đến điểm cuối.
  - Nét chỉ có một điểm cho ra một đoạn dài 0.
- `pub fn alpha_at(s: &Stroke, now_ms: i64, hold_ms: i64) -> f32`
  - Bằng 1 khi chưa vẽ xong, hoặc khi `hold_ms == 0`.
  - Bằng 1 cho tới `finished + hold`, sau đó giảm tuyến tính về 0 trong `FADE_OUT_MS`.
- `pub fn visible_bounds(strokes: &[Stroke], now_ms: i64, hold_ms: i64, w: u32, h: u32) -> Option<Rect>`
  - Hộp bao các nét còn `alpha > 0`, nới thêm `width/2 + 2`, kẹp trong `w×h`.
  - Trả `None` khi không còn gì để vẽ.

- [ ] **Step 1: Viết test hỏng.**
  - `pen_segments_join_points`: 3 điểm cho 2 đoạn.
  - `arrow_has_shaft_and_two_head_lines`: mũi tên từ (0,0) tới (100,0), độ dày 4. Ra 3 đoạn; hai đoạn đầu mũi tên xuất phát từ (100,0), có độ dài gần bằng 16, x < 100, y đối xứng qua trục.
  - `rect_has_four_edges`.
  - `single_point_gives_one_zero_length_segment`.
  - `alpha_holds_then_fades`: hold 3000, vẽ xong lúc 1000. Lúc 3999 là 1; lúc 4250 gần bằng 0,5; lúc 4500 là 0. Chưa vẽ xong thì lúc 99999 vẫn là 1. `hold_ms == 0` thì luôn là 1.
  - `bounds_cover_visible_strokes_and_clip`: một nét có điểm âm và điểm vượt `w`; hộp bao bị kẹp trong `[0, w)`. Nét đã mờ hết thì trả `None`.
- [ ] **Step 2:** `cargo test -p hp-core stroke`. Mong đợi: FAIL.
- [ ] **Step 3:** Cài đặt.
- [ ] **Step 4:** `cargo test -p hp-core`. Mong đợi: PASS.
- [ ] **Step 5:** Commit `feat(core): strokes for drawing while recording`.

### Task 2: Cài đặt vẽ (`settings.rs`)

**Interfaces:**
- Thêm hằng `pub const DEFAULT_HOTKEY_DRAW: &str = "Ctrl+Alt+D";`
- Thêm trường `pub hotkey_draw: String`, `pub draw_fade_secs: u32` (mặc định 3).
- `normalize`:
  - `draw_fade_secs` phải thuộc `{0, 2, 3, 5, 10}`, nếu không thì về 3.
  - `hotkey_draw` tham gia luật không trùng phím như các phím tắt khác.

- [ ] Test trước:
  - `draw_defaults`;
  - `bad_fade_falls_back_to_3`;
  - `draw_hotkey_cannot_duplicate_toggle`: `hotkey_draw` bằng `"Ctrl+Alt+R"` thì sau `normalize` phải khác `hotkey_toggle`.
- [ ] Chạy test, thấy FAIL; cài đặt; chạy lại, thấy PASS.
- [ ] Phía TypeScript: thêm hai trường vào `Settings` trong `api.ts`. `pnpm tsc` sạch.
- [ ] Commit.

### Task 3: `live_layer.rs` (nét vẽ)

**Interfaces:**
- Trong `compositor.rs`: `pub(crate) fn d2d_context(device: Device) -> Result<ID2D1DeviceContext, String>`, dùng lại trong `Compositor::new`.
- `pub struct LiveLayer`, các hàm:
  - `new() -> Arc<LiveLayer>`
  - `set_hold_ms(&self, ms: i64)`
  - `upsert_stroke(&self, s: Stroke)`: thay nét cùng `id`. Nếu `s.finished_ms` là `Some` thì Rust tự đặt lại bằng giờ hiện tại theo `clock`, nên giao diện chỉ cần gửi `Some(0)` để báo "đã xong".
  - `clear(&self)`
  - `set_drawing(&self, on: bool)`, `is_drawing(&self) -> bool`
  - `drawing_flag(&self) -> Arc<AtomicBool>`
  - `apply(&self, canvas: &mut [u8], w: u32, h: u32, now_ms: i64)`
- Cách `apply` làm việc:
  - Gom các nét còn hiện; không có thì trả về ngay, không gọi Direct2D.
  - Có thì lấy `visible_bounds`, chép vùng đó lên một bitmap Direct2D, vẽ từng đoạn với nét bo tròn (đường thẳng cộng một chấm tròn ở mỗi đầu), màu với độ mờ `alpha`, rồi đọc vùng đó ngược về `canvas`.
  - Bộ vẽ Direct2D tạo một lần và giữ lại; lỗi thì bỏ qua khung đó, có ghi nhật ký.
  - Sau Task 9, `apply` vẽ thêm webcam.

- [ ] **Test (WARP, không cần màn hình):**
  - `stroke_is_drawn_onto_the_frame`: canvas xám 200×200; nét bút đỏ, độ dày 8, từ (20,100) tới (180,100). Pixel (100,100) có R > 200, G < 80. Pixel (100,20) vẫn xám.
  - `nothing_visible_leaves_the_frame_untouched`: không có nét, hoặc nét đã mờ hết, thì canvas giữ nguyên từng byte.
  - `strokes_outside_the_frame_are_clipped`: nét có điểm âm và vượt ra ngoài; không panic, canvas vẫn đúng kích thước.
  - `apply_cost_on_a_big_frame` (`#[ignore]`, thiết bị phần cứng): canvas 2560×1600, nét dài 1000 px. In ra thời gian trung bình của 30 lần gọi; ghi số đo vào sổ.
- [ ] Chạy test, thấy FAIL; cài đặt; chạy lại, thấy PASS.
- [ ] Commit `feat: live layer draws strokes onto recorded frames`.

### Task 4: Nối lớp vẽ vào pipeline, recorder và input hook

**Interfaces:**
- `PipelineConfig.live: Option<Arc<LiveLayer>>`.
- `pace_loop` gọi `live.apply(&mut canvas, width, height, clock::now_100ns()/10_000)` sau `blit`, trước vòng `write_video`. Đo thời gian; quá 8 ms thì gửi `PipelineEvent::Tick` kèm cảnh báo (giới hạn tần suất) và ghi nhật ký.
- `Recorder` tạo một `Arc<LiveLayer>` trong `with_host`, có `pub fn live(&self) -> Arc<LiveLayer>`.
  - Khi bắt đầu quay: đặt hold theo `settings.draw_fade_secs`.
  - Khi dừng hoặc hủy: `clear()` và `set_drawing(false)`.
- `start_input_hook` nhận thêm `skip_clicks: Arc<AtomicBool>`. Khi cờ đang bật thì bỏ `Down`/`Up`, nhưng vẫn ghi `Move`, `Wheel`, `Key`.
  - Tách luật lọc thành hàm thuần `fn keep_event(e: &Event, drawing: bool) -> bool` để test.

- [ ] Test trước:
  - `clicks_are_dropped_while_drawing`, dùng `keep_event`;
  - `stopping_clears_the_live_layer`, dùng Recorder với `Host` giả như các test recorder hiện có.
- [ ] Test `#[ignore]` (cần màn hình, chỉ chạy khi người dùng cho phép): `recorded_frames_contain_strokes`. Quay màn hình chính 2 giây với một nét đỏ dày 14 ở giữa, rồi giải mã khung cuối: pixel giữa màn hình phải đỏ.
- [ ] Commit `feat: strokes are recorded into the video; clicks while drawing are not logged`.

### Task 5: Lệnh, phím tắt và cửa sổ vẽ (Rust)

**Interfaces (`live_cmds.rs`):**
- `draw_toggle(app)`, `draw_set(app, on: bool)`:
  - Chỉ dùng được khi đang quay hoặc tạm dừng; khi đang chờ thì báo "Chỉ vẽ được khi đang quay".
  - Lần đầu thì tạo cửa sổ `draw`: trong suốt, không viền, luôn trên cùng, không hiện ở thanh tác vụ. Đặt nó đúng `capture::target_rect(target)` theo pixel vật lý và loại khỏi capture.
  - `on` thì nhận chuột (`set_ignore_cursor_events(false)`) và lấy tiêu điểm; `off` thì để chuột đi xuyên qua (`true`).
  - Gửi sự kiện `draw-mode {on}`.
- `draw_stroke(stroke: Stroke)` gọi `upsert_stroke`.
- `draw_clear()`.
- `draw_context() -> DrawContext { on: bool, hold_ms: i64 }`.
- `HotkeyAction::Draw`, đăng ký `settings.hotkey_draw` và gọi `draw_toggle`.
- `shell::sync_windows`: khi về `Idle` thì đóng cửa sổ `draw` (và `cam` ở Task 10).
- `capabilities/default.json` thêm `"draw"`, `"cam"` và quyền `core:window:allow-set-ignore-cursor-events`.

- [ ] Test trước: `draw_window_rect_matches_the_target`, cho hàm thuần `fn overlay_rect(target_rect: Rect) -> Rect`, kiểm cả kích thước lẻ và tọa độ âm.
- [ ] `cargo build -p hp-screen` không cảnh báo mới.
- [ ] Commit.

### Task 6: Giao diện vẽ

**Files:** `src/liveApi.ts`, `src/windows/DrawOverlay.tsx`; sửa `main.tsx`, `ControlBar.tsx`, `SettingsPage.tsx`, `theme.css`.

- **DrawOverlay**
  - Một canvas phủ cả cửa sổ.
  - Thanh công cụ ở giữa cạnh trên: Bút, Mũi tên, Khung, 5 màu, 3 độ dày, Xóa hết, Thoát.
  - Kéo chuột thì tạo nét; điểm tính bằng `clientX * devicePixelRatio`.
  - Gửi `draw_stroke` tối đa 60 lần/giây trong lúc vẽ; khi nhả chuột gửi `finished_ms: 0`.
  - Tự vẽ lại các nét trên canvas, với cùng thời gian giữ và mờ (đồng hồ: `performance.now()` đặt mốc lúc nhả chuột).
  - Esc gọi `draw_set(false)`.
  - Khi `draw-mode` tắt: ẩn thanh công cụ, các nét vẫn mờ dần.
- **ControlBar:** nút ✎ "Vẽ (Ctrl+Alt+D)" khi đang quay hoặc tạm dừng; sáng lên khi đang bật.
- **SettingsPage:**
  - phím tắt "Vẽ lên màn hình khi quay";
  - lựa chọn "Nét tự mờ sau": 2 / 3 / 5 / 10 giây hoặc "Không tự mờ".
- **Hàm thuần dùng chung, có test:** `src/windows/strokeMath.ts` với `alphaAt` giống Rust, cộng `strokeMath.test.ts` chạy bằng `node --test`.

- [ ] Viết `strokeMath.test.ts` trước: FAIL rồi PASS bằng `pnpm test`.
- [ ] `pnpm tsc --noEmit` sạch.
- [ ] Commit `feat: draw on screen while recording`.

**Điểm dừng bước 3:** chạy ba lệnh kiểm. Ghi danh sách thử tay vào `docs/manual-test-checklist.md`:
- vẽ ở cả 3 chế độ quay;
- nét mờ dần, và cũng mờ trong video;
- Esc thoát chế độ vẽ;
- phím tắt;
- click lúc đang vẽ không thành vòng click khi xuất.

---

## BƯỚC 4 — Webcam

### Task 7: `hp-core::webcam` và cài đặt

**Interfaces:**
- `pub enum CamShape { Circle, Rounded }`
- `pub enum CamSize { Small, Medium, Large }`, tương ứng 0,15 / 0,20 / 0,25 chiều cao video.
- `pub enum Corner { TopLeft, TopRight, BottomLeft, BottomRight }`
- `#[serde(default)] pub struct WebcamSettings { pub enabled: bool, pub device: Option<String>, pub shape: CamShape, pub size: CamSize, pub corner: Corner, pub mirror: bool }`
  - Mặc định: tắt, `Circle`, `Medium`, `BottomRight`, có lật gương.
- `Settings.webcam: WebcamSettings`.
- `pub fn bubble_rect(frame_w: u32, frame_h: u32, ws: &WebcamSettings) -> Rect`
  - Cao bằng `size × frame_h`. Dạng tròn thì rộng bằng cao; dạng bo góc thì rộng bằng `4/3 × cao`.
  - Cách mép `0,025 × frame_h`, theo góc đã chọn.
  - Kẹp cho vừa khung: không rộng hơn `frame_w − 2 × lề`.
- `pub fn camera_crop(cam_w: u32, cam_h: u32, shape: CamShape) -> Rect`: cắt giữa ảnh camera theo tỉ lệ 1:1 hoặc 4:3.

- [ ] Test trước:
  - `medium_circle_bottom_right`: khung 1920×1080 ra `{x: 1677, y: 837, w: 216, h: 216}`;
  - `rounded_is_four_by_three`;
  - `each_corner`;
  - `tiny_frames_shrink_the_bubble`: khung 100×400 cỡ Large thì bong bóng vẫn nằm trong khung;
  - `crop_is_centred`: camera 1280×720 dạng tròn ra `{280, 0, 720, 720}`;
  - `webcam_settings_default_and_survive_bad_values`.
- [ ] FAIL, cài đặt, PASS. Thêm các trường vào `api.ts`. Commit.

### Task 8: `camera.rs`

**Interfaces:**
- `#[derive(Serialize)] pub struct CameraInfo { pub id: String, pub name: String }`
- `pub fn list_cameras() -> Vec<CameraInfo>`: liệt kê bằng `MFEnumDeviceSources` với kiểu nguồn quay video; `id` là symbolic link.
- `pub struct CamFrame { pub width: u32, pub height: u32, pub bgra: Vec<u8> }`
- `pub struct Camera`, các hàm:
  - `open(id: Option<&str>) -> Result<Camera, String>`
  - `latest(&self) -> Arc<Mutex<Option<CamFrame>>>`
  - `is_alive(&self) -> bool`
  - Drop thì dừng luồng đọc và giải phóng thiết bị.
- Cách đọc:
  - Một luồng riêng dùng Source Reader có xử lý video, ra RGB32.
  - Chọn kiểu gốc có diện tích lớn nhất mà cao ≤ 720.
  - Mỗi buffer đọc qua kiểm biên `rows_in_bounds`, chuyển `pub(crate)` từ `decoder.rs`.
- Lỗi đổi sang tiếng Việt:
  - không có camera: "Không tìm thấy camera";
  - lỗi truy cập (`E_ACCESSDENIED`): "Windows đang chặn quyền dùng camera cho ứng dụng desktop: mở Cài đặt › Quyền riêng tư › Camera";
  - thiết bị đang bận: "Camera đang được ứng dụng khác dùng".

- [ ] Test trước:
  - `listing_cameras_never_fails` (không ignore): gọi được và trả một Vec, có thể rỗng;
  - `camera_errors_are_explained`: hàm thuần `fn camera_error(hr: HRESULT) -> String` với `E_ACCESSDENIED` và `MF_E_HW_MFT_FAILED_START_STREAMING`;
  - `#[ignore] first_camera_gives_frames`: cần camera và người dùng cho phép.
- [ ] FAIL, cài đặt, PASS. Commit.

### Task 9: Vẽ webcam trong `LiveLayer`

**Interfaces:**
- `LiveLayer::set_camera(&self, cam: Option<(Arc<Mutex<Option<CamFrame>>>, WebcamSettings)>)`
- `apply` vẽ webcam vào `bubble_rect`:
  - vùng vẽ là hợp của hộp bao nét vẽ và hộp bong bóng;
  - ảnh camera cắt theo `camera_crop`, lật ngang khi `mirror`;
  - đặt vào bong bóng bằng bitmap brush có biến đổi, tô bằng `FillEllipse` (tròn) hoặc `FillRoundedRectangle` với bán kính 12% (bo góc);
  - viền trắng.
- Camera mất kết nối: giữ khung cuối 1 giây, sau đó không vẽ webcam nữa (dựa vào thời điểm cập nhật khung trong `CamFrame`, thêm trường `at_ms`).

- [ ] **Test (WARP):**
  - `webcam_is_drawn_in_its_bubble`: khung xám 640×360, camera giả đỏ 320×240, tròn cỡ Medium góc dưới phải. Tâm bong bóng đỏ; góc hộp bao (ngoài hình tròn) vẫn xám.
  - `mirror_flips_the_picture`: camera giả nửa trái đỏ, nửa phải xanh. Khi lật gương, tâm nửa trái của bong bóng phải xanh.
  - `stale_camera_frame_is_dropped`: khung camera cũ hơn 1 giây thì không vẽ.
- [ ] FAIL, cài đặt, PASS. Commit `feat: webcam picture recorded into the video`.

### Task 10: Lệnh, bong bóng và giao diện webcam

**Interfaces:**
- Lệnh:
  - `cam_list() -> Vec<CameraInfo>`
  - `cam_set(app, on: bool) -> Result<(), String>`: mở hoặc đóng camera, gắn vào `LiveLayer`, tạo hoặc đóng cửa sổ `cam` (loại khỏi capture, đặt ở góc tương ứng của vùng quay, kích thước theo `bubble_rect` quy đổi ra màn hình), gửi sự kiện `cam-state {on, error}`.
  - Một luồng gửi `cam-frame {jpeg_base64}` 10 lần/giây: ảnh 240 px đã cắt và lật như trong video.
- Khi bắt đầu quay và `settings.webcam.enabled` đang bật thì tự `cam_set(true)`.
- Khi về `Idle` thì `cam_set(false)`.
- Lỗi camera: báo bằng `rec_error` (thanh gọn hiện thông báo); việc quay vẫn tiếp tục.
- Giao diện:
  - `CamBubble.tsx`: ảnh tròn hoặc bo góc;
  - `ControlBar`: nút 📷 khi đang quay;
  - `RecordPage`: công tắc "Webcam";
  - `SettingsPage`: mục Webcam gồm chọn camera, hình dạng, cỡ, góc, lật gương.
- `Cargo.toml`: `image` thêm feature `jpeg`.

- [ ] Test trước:
  - `camera_closes_when_recording_ends`: với `Host` giả, `Recorder` gọi hook dọn dẹp khi về Idle;
  - `bubble_window_rect_on_screen`: hàm thuần quy đổi `bubble_rect` sang tọa độ màn hình theo gốc vùng quay.
- [ ] FAIL, cài đặt, PASS. `pnpm tsc` sạch. Commit `feat: webcam toggle, bubble and settings`.

**Điểm dừng bước 4:**
- Chạy đủ bốn lệnh kiểm.
- Thêm vào danh sách thử tay: webcam ở cả 3 chế độ quay, đổi góc / cỡ / hình dạng, lật gương, rút camera giữa chừng, đèn camera tắt sau khi dừng.
- Review cả nhánh bằng một reviewer mới; sửa các lỗi Critical / Important kèm test.
- Mở bản dev cho người dùng thử (hỏi trước khi đóng bản đang chạy).
