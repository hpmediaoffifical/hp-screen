# HP Screen — Spotlight và nhòe chuyển động (bước 5d)

Ngày: 2026-11-12 · Trạng thái: người dùng chọn "làm tiếp Spotlight và nhòe chuyển động"; người dùng đã dặn tự quyết chi tiết.

## Spotlight

- Mỗi đoạn zoom có thêm `style`: `"zoom"` (mặc định, như cũ) hoặc `"spotlight"`.
- **Đoạn spotlight không phóng to.**
  - Camera giữ toàn khung.
  - Vùng mà đoạn đó lẽ ra phóng tới (khung chính, các điểm ◆, hoặc theo con trỏ) được giữ sáng; phần còn lại của video tối đi (đen 55%).
  - Vùng sáng có góc bo bằng 2% cạnh ngắn và viền trắng mảnh (1,5 px, độ đục 50%).
- **Chuyển động:**
  - Vùng sáng di chuyển và đổi kích thước theo cùng lò xo và tốc độ với camera.
  - Độ tối hiện dần khi vào đoạn và tắt dần khi ra (cùng lò xo); với tốc độ Tức thì thì bật/tắt ngay.
- **Đường tính:** lõi tính `Spot { cx, cy, scale, alpha }` cùng lúc với camera (`zoom_track`).
  - Plan thêm `Draw::Spot { x, y, w, h, alpha }` (pixel nguồn), vẽ trước vòng click.
  - Xem trước vẽ cùng phép tính trên lớp canvas đi theo video.
- **Giao diện:**
  - Thẻ Zoom, đoạn đang chọn: hàng nút "Phóng to / Làm tối xung quanh".
  - Khung kéo thả và các điểm ◆ giữ nguyên, nay chúng xác định vùng sáng.
  - Trên rãnh, đoạn spotlight ghi "Spot".
- Đoạn tự động vẫn là phóng to.

## Nhòe chuyển động

- `ZoomDoc.motion_blur` (mặc định bật); thẻ Zoom có ô "Nhòe chuyển động khi zoom".
- **Bản xuất (MP4 và GIF):**
  - Với mỗi khung, plan lấy 5 khung nhìn (view) của camera, trải đều trong nửa khung hình quanh thời điểm khung đó (màn trập 180°).
  - Nếu một góc của khung nhìn dịch quá 0,25% cạnh của nó, khung được vẽ 5 lần theo 5 khung nhìn đó rồi lấy trung bình điểm ảnh.
  - Camera đứng yên thì vẽ 1 lần như cũ, nên xuất chỉ chậm hơn ở các lúc đang zoom hoặc lia.
- **Xem trước** (gần đúng):
  - Khi đang phát và camera đang chuyển động, phần video được làm mờ nhẹ bằng `filter: blur()`, tỉ lệ theo tốc độ, tối đa 3 px.
  - Khi dừng thì không mờ.

## Tương thích

`style` và `motion_blur` có giá trị mặc định khi đọc file cũ: file cũ cho ra đúng như trước, chỉ thêm nhòe khi zoom.

## Kiểm thử

- **`hp-core`:**
  - file cũ đọc ra đúng mặc định;
  - đoạn spotlight: camera giữ toàn khung, `alpha` lên gần 1 và vùng sáng trùng khung của đoạn, ra khỏi đoạn thì `alpha` về 0;
  - Plan có `Draw::Spot`;
  - `blur_views`: 5 khung nhìn khi đang zoom, rỗng khi camera đứng yên hoặc tắt nhòe.
- **`hp-screen`:**
  - compositor: `Draw::Spot` làm tối ngoài vùng, giữ nguyên trong vùng;
  - `average_into` lấy trung bình đúng;
  - xuất có nhòe: số khung đúng, và một khung lúc đang zoom khác với bản không nhòe.
- **TS:** `spotAt` nội suy; `drawSpot` không lỗi khi không có spot.
- **Thử tay:** thêm Z24–Z26.
