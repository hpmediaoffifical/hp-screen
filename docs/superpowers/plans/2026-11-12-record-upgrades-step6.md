# Nâng cấp quay — Bước 6 (khung hình: tỉ lệ, nền, padding; xuất GIF) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Xuất video theo tỉ lệ Gốc / 16:9 / 9:16 / 1:1, với hai cách đặt video:
- "Vừa khung": video nằm trọn trong khung, có nền, padding, bo góc, bóng đổ;
- "Lấp đầy": cắt video cho kín khung, tâm cắt đi theo camera zoom.

Xuất được GIF (10/15 khung/giây; rộng 480/720/960 px; lặp vô hạn).

**Architecture:**
- `hp-core::layout` tính hai thứ:
  - bố cục khung xuất: kích thước, chỗ đặt video, thông số trang trí;
  - vùng nguồn được hiện tại mỗi thời điểm: vùng zoom, cộng thêm phần cắt theo tỉ lệ khi ở chế độ Lấp đầy.
- `Compositor` vẽ theo thứ tự: nền (màu hoặc dải màu), bóng, video trong khung bo góc (dùng bitmap brush), rồi hiệu ứng.
- `gif_out.rs` ghi GIF bằng crate `gif` (MIT/Apache), giảm màu bằng NeuQuant có sẵn trong crate.
- Phần xem trước dựng lại bố cục bằng CSS:
  - các hằng số bố cục lấy từ lệnh `edit_layout`;
  - vùng nguồn ở mỗi thời điểm tính bằng `sourceView`, một hàm TS có test với cùng giá trị như bản Rust.

**Spec:** mục 4.8.

## Global Constraints

- **Kích thước khung:** cao bằng chiều cao nguồn (làm chẵn); rộng tính theo tỉ lệ (làm chẵn).
- **Padding:** 0–20% cạnh ngắn, mỗi bên.
- **Bo góc:** 0–40 px theo khung xuất (khi xuất GIF thì nhân theo tỉ lệ thu nhỏ).
- **Bóng đổ:** dịch xuống 1,5% cạnh ngắn, mờ dần qua 8 lớp.
- **Nền có sẵn:**

  | Mã | Màu |
  |---|---|
  | `slate` | #1E293B |
  | `black` | #000000 |
  | `white` | #FFFFFF |
  | `ocean` | #0EA5E9 → #6366F1 |
  | `sunset` | #F97316 → #EC4899 |
  | `forest` | #10B981 → #0F766E |

  Dải màu chạy chéo từ góc trên trái xuống góc dưới phải.
- **Lấp đầy:** tâm cắt là tâm camera (zoom). Khi zoom tắt thì cắt ở giữa.
- **GIF:**
  - mặc định 15 khung/giây, rộng 720;
  - phần giữ lại dài hơn 60 giây thì cảnh báo nhưng vẫn cho xuất;
  - đặt tên `<tên>_edit.gif`;
  - GIF được Thư viện xếp vào loại Ảnh.
- File chỉnh sửa vẫn `v: 1`; trường `frame` mới có giá trị mặc định.
- Commit kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **9:16 từ nguồn 16:9 ở chế độ Vừa khung:** video nhỏ, nằm giữa, có nền phía trên và phía dưới. Test ở Task 1 và Task 3.
2. **Lấp đầy cộng zoom ở sát mép:** vùng cắt không ra ngoài nguồn. Test ở Task 1.
3. **Padding 20% cộng tỉ lệ Gốc:** video thu nhỏ đúng tỉ lệ. Test ở Task 1.
4. **GIF của video không tiếng, hoặc rất ngắn:** vẫn ra file hợp lệ và mở được. Test ở Task 4.
5. **Nguồn có kích thước lẻ:** khung xuất là số chẵn. Test ở Task 1.

---

### Task 1: `hp-core::layout`

- `pub enum Aspect { Source, Wide, Tall, Square }`, serde `"source" | "16:9" | "9:16" | "1:1"`.
- `pub enum Fit { Fit, Fill }`, serde `"fit" | "fill"`.
- `#[serde(default)] pub struct FrameDoc { aspect, fit, background: String, padding: f32, radius: f32, shadow: bool }`, mặc định `source / fit / "slate" / 0 / 0 / false`.
- `EditDoc.frame: FrameDoc`.
- `pub struct Layout`:
  - `out_w, out_h: u32`
  - `video: (f32, f32, f32, f32)`: vị trí video trong khung xuất, theo pixel
  - `radius: f32`, `shadow: bool`, `background: Background`
  - `fill_aspect: Option<f32>`
  - `decorated: bool`: có nền, padding, bo góc hoặc bóng không
- `pub enum Background { Solid([u8; 3]), Gradient([u8; 3], [u8; 3]) }`, cùng `pub fn background(id: &str) -> Background`. Mã lạ thì dùng `slate`.
- `pub fn layout(src_w: u32, src_h: u32, f: &FrameDoc) -> Layout`
- `pub fn source_view(cam: Camera, src_w: u32, src_h: u32, fill_aspect: Option<f32>) -> (f32, f32, f32, f32)`
- `impl Layout { pub fn scaled(&self, width: u32) -> Layout }`: thu nhỏ để xuất GIF.
- Test:
  - `source_aspect_is_the_identity`;
  - `tall_frame_from_a_wide_source_fits_with_bars`: 1920×1080, Tall, Fit → khung 608×1080; video `(0, 369, 608, 342)`;
  - `square_frame`;
  - `padding_shrinks_the_video`: tỉ lệ Gốc, padding 0,2 → video `(216, 216, 1488, 648)`, giữ đúng tỉ lệ khi vừa khung;
  - `fill_covers_the_frame`;
  - `odd_sources_give_even_frames`;
  - `fill_view_has_the_frame_aspect_and_stays_inside`, ở camera giữa và camera sát góc;
  - `unknown_background_is_slate`;
  - `scaled_layout_for_gif`.

### Task 2: `Compositor` vẽ bố cục

- `render(src, sw, sh, view, layout: Option<&Layout>, draws)`: `None` nghĩa là toàn khung, không trang trí (như hiện nay).
- Có `layout` thì:
  - xóa khung bằng nền;
  - nếu có bóng thì vẽ bóng;
  - tô hình chữ nhật bo góc ở `layout.video` bằng bitmap brush, có biến đổi đưa `view` vào `layout.video`;
  - hiệu ứng đổi tọa độ: `x_out = video.x + (x − vx) · video.w / vw`.
- Test WARP:
  - `background_fills_around_a_fitted_video`: khung 9:16 từ nguồn đỏ → pixel trên cùng là màu nền, pixel giữa là đỏ;
  - `rounded_corner_shows_the_background`;
  - `gradient_background_changes_across_the_frame`;
  - `effects_follow_the_video_rect`.

### Task 3: `Plan` và xuất video có bố cục

- `Plan` giữ `layout`; `out_size()` lấy từ `layout`.
- `FramePlan.view` là `source_view(camera, …, layout.fill_aspect)`.
- `export` ghi thẳng khung (bỏ qua compositor) chỉ khi: không có hiệu ứng, không zoom, và không có bố cục.
- Test export:
  - `tall_export_has_the_frame_size_and_background`: `probe` cho ra kích thước 9:16; pixel trên cùng là màu `slate`;
  - `fill_export_crops`.

### Task 4: GIF

- Thêm phụ thuộc `gif = "0.13"`.
- `gif_out.rs`: `pub struct GifWriter`, các hàm `new(path, w, h, fps) -> Result`, `write(&mut self, bgra: &[u8]) -> Result`, `finish(self) -> Result`.
- `export::run_export_gif(source, out, plan, gif: GifOptions { fps, width }, progress, cancel) -> Result<PathBuf>`:
  - dùng `Plan::for_gif(fps, width)` (đổi fps và kích thước khung);
  - ghi vào file `.part`;
  - không có tiếng.
- `export_path(source, ext)` thêm tham số đuôi file.
- `hp_core::library::kind_of`: `.gif` là Ảnh.
- Lệnh `export_start(path, doc, format: "mp4" | "gif", gif: Option<GifOptions>)`.
- Test:
  - `gif_has_the_frames_and_size`: đọc lại bằng crate `gif`, 1 giây ở 10 khung/giây ra 10 khung, rộng 160;
  - `gif_of_a_silent_video`;
  - `gif_names_do_not_overwrite`.

### Task 5: Giao diện

- **Thẻ "Khung hình":**
  - tỉ lệ: Gốc / 16:9 / 9:16 / 1:1;
  - cách đặt: Vừa khung / Lấp đầy;
  - 6 ô nền;
  - thanh trượt padding 0–20%, bo góc 0–40;
  - ô chọn bóng đổ.
- **Thẻ Xuất:** chọn định dạng MP4 / GIF; khi chọn GIF thì chọn 10/15 fps và rộng 480/720/960; phần giữ lại dài hơn 60 giây thì hiện cảnh báo.
- **Xem trước:**
  - lệnh `edit_layout(doc, frame) -> Layout` cho các hằng bố cục;
  - khung ngoài có tỉ lệ khung xuất và nền CSS;
  - một ô video đặt ở `layout.video` (phần trăm), có `border-radius` và bóng;
  - bên trong, video được đặt cỡ và dịch theo `sourceView` ở mỗi khung hình (hàm TS `sourceView` có test, cùng giá trị như Rust).

**Điểm dừng:**
- Chạy bốn lệnh kiểm.
- Thêm mục F1–F6 vào danh sách thử tay.
- Review bằng reviewer mới, sửa lỗi Critical / Important kèm test.
