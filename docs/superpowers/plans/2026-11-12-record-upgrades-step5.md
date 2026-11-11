# Nâng cấp quay — Bước 5 (auto-zoom và zoom tự chọn) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Trong trang Chỉnh sửa có thể:
- tự tạo các đoạn phóng to từ click;
- thêm, dời, đổi mức zoom, đặt tâm cho từng đoạn bằng tay;
- xem trước đúng như bản xuất;
- xuất ra video có zoom mượt.

**Architecture:**
- `hp-core::zoom` có ba phần:
  - gom click thành đoạn zoom;
  - tính "đường đi camera" (tâm và mức zoom theo thời gian, đi theo lò xo của `hp-core::spring`);
  - lấy mẫu đường đi đó 60 lần mỗi giây.
- `Plan` gắn khung camera vào từng khung xuất. `Compositor` vẽ đúng phần nguồn nằm trong khung camera, và đổi tọa độ hiệu ứng theo khung đó.
- Phần xem trước nhận đường đi camera từ lệnh `zoom_track` rồi áp `transform` CSS lên video và canvas. JS không tự tính zoom.

**Spec:** `docs/superpowers/specs/2026-11-12-record-upgrades-design.md` mục 4.6.

Người dùng đã nói rõ: họ dựng video trong CapCut, nên trong HP Screen chỉ cần zoom tự chọn đơn giản và gọn. Zoom bằng tay vì thế phải dễ dùng, quan trọng ngang zoom tự động.

## Global Constraints

- Thông số mặc định, theo spec 4.6:

  | Thông số | Giá trị |
  |---|---|
  | Mức zoom | 1,5× |
  | Gom click cách nhau dưới | 2500 ms |
  | Gom click cách nhau dưới | 0,15 chiều rộng video |
  | Bắt đầu zoom trước click đầu | 400 ms |
  | Giữ zoom sau click cuối | 1800 ms |
  | Gộp hai đoạn hở nhau dưới | 800 ms |
  | Bỏ đoạn ngắn hơn | 1000 ms |
  | Lò xo camera | tension 300, mass 6,75, friction 120, bước 1 ms |
  | Vùng chết khi bám con trỏ | 70% khung |
  | Nhìn trước | 1000 ms |
  | Mức zoom cho phép | 1–4×; trên 2× cảnh báo |

  Mỗi giá trị là một hằng có tên trong `zoom.rs`.
- File chỉnh sửa vẫn là `v: 1`. Trường `zoom` mới có giá trị mặc định nên file cũ vẫn đọc được.
- Không sửa `SnipOverlay.tsx`, `snip.rs`. File dùng chung chỉ được thêm vào.
- Commit kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Các lệnh kiểm phải đạt: `cargo test -p hp-core`, `cargo test -p hp-screen --lib`, `pnpm tsc --noEmit`, `pnpm test`.

## Review Focus

1. **Đoạn zoom sát mép video** (click ở góc). Mong đợi: khung camera không lộ ra ngoài video. Test ở Task 3.
2. **Video không có nhật ký thao tác.** Mong đợi: vẫn thêm được zoom tay, tâm cố định (mặc định ở giữa). Test ở Task 3 và Task 5.
3. **Đoạn zoom nằm trong phần đã cắt, hoặc vắt qua chỗ cắt.** Mong đợi: xuất không lỗi; chuyển động tính theo thời gian nguồn. Test ở Task 4.
4. **Hai đoạn tay chồng nhau, hoặc đoạn dài 0.** Mong đợi: không panic; đoạn dài 0 bị bỏ. Test ở Task 2.
5. **Zoom 4× và 1×.** Mong đợi: ở 4× vẫn đúng tỉ lệ; ở 1× giống hệt không zoom. Test ở Task 3 và Task 4.

---

### Task 1: `hp-core::spring`

- `pub struct Spring { pub tension: f32, pub mass: f32, pub friction: f32 }`
- `pub struct SpringState { pub value: f32, pub velocity: f32 }`
- `pub fn step(&self, s: &mut SpringState, target: f32, dt_s: f32)`: Euler bán ẩn, gọi với bước 1 ms.
- Test:
  - `settles_on_the_target_without_overshoot`: với thông số camera, đi từ 0 tới 1 trong 3 giây; giá trị không bao giờ vượt quá 1 và cuối cùng cách 1 dưới 0,01.
  - `moves_monotonically`: giá trị không bao giờ giảm.
  - `stays_put_at_the_target`.

### Task 2: Mô hình zoom và tạo đoạn tự động

- `#[serde(default)] pub struct ZoomSegment { pub start_ms: i64, pub end_ms: i64, pub scale: f32, pub center: Option<(f32, f32)>, pub manual: bool }`
  - `center` là tọa độ chuẩn hóa 0–1; `None` nghĩa là bám con trỏ (khi có nhật ký) hoặc ở giữa (khi không có).
- `#[serde(default)] pub struct ZoomDoc { pub enabled: bool, pub segments: Vec<ZoomSegment> }`, thêm thành `EditDoc.zoom`.
- `pub fn auto_segments(clicks: &[(i64, f32, f32)], duration_ms: i64) -> Vec<ZoomSegment>`
  - Đầu vào là click đã chuẩn hóa tọa độ.
  - Gom click, bắt đầu sớm 400 ms, giữ thêm 1800 ms, gộp đoạn hở dưới 800 ms, bỏ đoạn dưới 1000 ms, kẹp trong `[0, duration)`.
  - Tâm là trọng tâm các click; `manual = false`.
- `pub fn regenerate(existing: &[ZoomSegment], auto: Vec<ZoomSegment>) -> Vec<ZoomSegment>`: giữ đoạn tay, bỏ đoạn tự động chồng lên đoạn tay, sắp theo thời gian.
- `pub fn normalized(segments: &[ZoomSegment], duration_ms: i64) -> Vec<ZoomSegment>`
  - Bỏ đoạn dài 0 hoặc ngược chiều; kẹp `scale` trong 1–4; kẹp `center` trong 0–1.
  - Đoạn chồng nhau thì đoạn bắt đầu sau thắng phần chồng.
- Test:
  - `clicks_close_in_time_and_place_make_one_segment`: click ở 1000 và 2000 ms cùng chỗ → đoạn `[600, 3800)`, tâm là trọng tâm.
  - `far_apart_clicks_split`;
  - `nearby_segments_merge`: hở dưới 800 ms thì gộp;
  - `short_segments_are_dropped`;
  - `segments_are_clamped_to_the_video`;
  - `regenerate_keeps_manual_segments`;
  - `normalized_drops_empty_and_clamps`;
  - `old_edit_files_read_without_zoom`: `load_edit("{\"v\":1}")` cho `zoom.enabled == false`, không có đoạn nào.

### Task 3: Đường đi camera

- `#[derive(Clone, Copy, Debug, PartialEq, Serialize)] pub struct Camera { pub cx: f32, pub cy: f32, pub scale: f32 }`: tâm chuẩn hóa, cộng mức zoom.
- `pub fn camera_track(z: &ZoomDoc, cursor: &dyn Fn(i64) -> Option<(f32, f32)>, duration_ms: i64, step_ms: i64) -> Vec<(i64, Camera)>`
  - Mô phỏng từ 0 tới hết video với bước 1 ms, ghi một mẫu mỗi `step_ms`.
  - **Đích trong một đoạn:** mức zoom bằng `scale` của đoạn.
    - Có `center` thì tâm cố định ở đó.
    - Không có `center` nhưng có con trỏ: khi con trỏ ra khỏi 70% vùng giữa của khung hiện tại, và vẫn ở ngoài 1000 ms sau đó, thì đích dời tới vị trí con trỏ. Tâm ban đầu lấy vị trí con trỏ lúc đoạn bắt đầu.
    - Không có cả hai thì tâm ở (0,5; 0,5).
  - **Ngoài đoạn:** mức zoom 1, tâm (0,5; 0,5).
  - `z.enabled == false` thì mọi mẫu đều là mức 1, tâm giữa.
  - Sau mỗi bước, kẹp tâm vào `[0,5/scale; 1 − 0,5/scale]`.
- `pub fn camera_at(track: &[(i64, Camera)], t_ms: i64) -> Camera`: nội suy tuyến tính giữa hai mẫu.
- `impl Camera { pub fn view(&self, w: u32, h: u32) -> (f32, f32, f32, f32) }`: vùng nguồn tính bằng pixel `(x, y, vw, vh)`.
- Test:
  - `disabled_zoom_is_identity`;
  - `zoom_eases_in_and_out`: đoạn `[1000, 3000)` mức 2. Mức ở 0 là 1; ở 2900 gần bằng 2; ở 6000 gần bằng 1; không có lúc nào vượt quá 2.
  - `corner_center_is_kept_inside_the_video`: tâm (0; 0), mức 2 → sau khi kẹp, tâm là (0,25; 0,25).
  - `cursor_inside_the_dead_zone_does_not_move_the_camera`;
  - `cursor_leaving_and_staying_out_moves_the_camera`;
  - `cursor_that_comes_back_within_a_second_does_not`;
  - `scale_1_view_is_the_whole_frame`;
  - `view_at_4x_is_a_quarter`.

### Task 4: Đưa zoom vào bản xuất

- `Plan::new(src, doc, effects)` tự tính `camera_track` (bước 1000/60 ms) khi `doc.zoom.enabled`.
- `FramePlan` thêm `camera: Camera`.
- `Compositor::render(src, src_w, src_h, view: Option<(f32, f32, f32, f32)>, draws)`: vẽ phần nguồn nằm trong `view`.
  - Hiệu ứng Ring / Dot / Wheel đổi tọa độ: `x_out = (x − vx) · out_w / vw`; bán kính nhân với `out_w / vw`.
  - Chip giữ nguyên trong khung xuất.
- `export`: chỉ ghi thẳng khung (bỏ qua compositor) khi không có gì để vẽ **và** mức zoom bằng 1.
- Test:
  - `plan_frames_carry_the_camera`;
  - compositor (WARP) `view_crops_the_source`: nguồn nửa trên đỏ, nửa dưới xanh; `view` là nửa trên → mọi pixel ra đều đỏ.
  - compositor (WARP) `ring_moves_with_the_view`;
  - export `zoomed_export_shows_the_zoomed_area`: mẫu 1 giây, đoạn zoom mức 2 tâm (0,25; 0,25) trên nguồn bốn màu theo góc → tâm khung xuất mang màu của góc trên trái.
  - `zoom_inside_a_cut_exports`.

### Task 5: Lệnh Tauri

- `zoom_auto(path, doc) -> Vec<ZoomSegment>`: lấy click từ nhật ký (`effects.ripples`, chuẩn hóa theo kích thước nguồn), gọi `regenerate`. Không có nhật ký thì trả lại các đoạn hiện có.
- `zoom_track(path, doc) -> Vec<(i64, Camera)>`: mẫu 60 lần/giây cho phần xem trước. Con trỏ lấy từ `effects.cursor`.
- Thêm vào `editApi.ts`: `ZoomSegment`, `ZoomDoc`, `Camera`, `zoomAuto`, `zoomTrack`.
- Test: `zoom_auto_without_log_keeps_segments`, gọi qua một hàm thuần trong `edit_cmds`.

### Task 6: Giao diện zoom

- **Thẻ Zoom**
  - Công tắc "Bật zoom". Bật lần đầu mà chưa có đoạn nào thì gọi `zoomAuto`.
  - Nút "Tự tạo lại".
  - Khi chọn một đoạn: thanh trượt mức zoom 1–4×; trên 2× hiện "video có thể bị mờ"; nút "Bấm lên video để đặt tâm"; nút "Bám con trỏ"; nút "Xóa đoạn".
- **Dòng thời gian:** thêm rãnh Zoom dưới rãnh Video.
  - Bấm vào một đoạn để chọn.
  - Kéo mép để đổi thời gian; kéo giữa đoạn để dời.
  - Kéo trên chỗ trống để thêm đoạn tay: mức 1,5, `center = None`, `manual = true`.
  - Sửa bất cứ gì của một đoạn thì đoạn đó thành `manual`.
  - Phím Delete xóa đoạn đang chọn (ưu tiên hơn việc bỏ vùng chọn của rãnh Video).
- **Xem trước:** `Preview` nhận `track` và tính `camera_at` (bản TS có test trong `zoomMath.ts`, `pnpm test`), rồi áp lên lớp chứa video và canvas:
  `transform: scale(s) translate(−(cx − 0.5/s)·W, −(cy − 0.5/s)·H)`, với `transform-origin: 0 0` và khung ngoài `overflow: hidden`.
- **Đặt tâm:** đang ở chế độ đặt tâm mà bấm lên khung xem trước thì quy vị trí bấm ra tọa độ nguồn, tính cả zoom hiện tại.
- Lấy lại `zoomTrack` mỗi khi `doc.zoom` thay đổi (chờ 150 ms sau thay đổi cuối).

**Điểm dừng:**
- Chạy đủ bốn lệnh kiểm.
- Thêm danh sách thử tay Z1–Z8 vào `docs/manual-test-checklist.md`.
- Review cả nhánh bằng reviewer mới; sửa lỗi Critical / Important kèm test.
- Gửi người dùng danh sách thử.
