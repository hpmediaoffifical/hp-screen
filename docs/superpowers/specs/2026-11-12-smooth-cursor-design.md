# HP Screen — Con trỏ mượt (bước 5e)

Ngày: 2026-11-12 · Trạng thái: người dùng chọn "Công tắc, mặc định tắt" và dặn tự quyết phần còn lại.

## Hành vi

- **Cài đặt:** thêm ô "Con trỏ mượt", mặc định tắt. Ô này chỉ có tác dụng khi "Nhật ký thao tác" đang bật, vì vị trí chuột lấy từ nhật ký.
- **Khi bật:**
  - HP Screen quay **không có con trỏ Windows** trong video, nên file MP4 gốc không có chuột.
  - Nhật ký ghi thêm hình dạng con trỏ (mũi tên, chữ I, bàn tay…) mỗi khi nó đổi.
  - Ở trang Chỉnh sửa, cả xem trước lẫn bản xuất vẽ lại con trỏ:
    - **Mượt:** vị trí đi theo một lò xo nhanh (tension 600, mass 1, friction 49), lọc bỏ rung tay mà vẫn bám sát.
    - **To hơn:** cỡ 1–3×, mặc định 1,5×.
    - **Tự ẩn:** chuột đứng yên 1,5 s (và không giữ nút) thì mờ dần trong 300 ms; nhúc nhích thì hiện lại ngay.
    - **Nhún khi bấm:** đang giữ nút thì con trỏ nhỏ còn 85%.
  - Con trỏ phóng to theo zoom, như vòng click.
- **Khi tắt:** quay như trước, file gốc có chuột của Windows.

## Nhật ký

- Header thêm `"cursor":true` khi video không có con trỏ Windows. Header cũ đọc ra `false`.
- `{"e":"shape","id":n,"hx":..,"hy":..,"png":"<base64>"}` được ghi một lần cho mỗi hình con trỏ mới:
  - ảnh RGBA có nền trong suốt;
  - con trỏ đen trắng (như chữ I) được tách nền bằng cách vẽ lên nền đen và nền trắng rồi so sánh.
- `{"e":"cur","t":ms,"id":n}` khi hình con trỏ đổi; `id` = 0 nghĩa là Windows đang ẩn con trỏ.
- Bản đọc cũ bỏ qua dòng lạ, và `LOG_VERSION` vẫn là 1.

## Lõi

- `Effects`:
  - `drawn_cursor` lấy từ header;
  - `shapes` (id → điểm nóng và PNG base64), `cursor_ids` theo thời gian;
  - `smooth`: các mẫu `(ms, x, y, alpha, press)` mỗi 1000/60 ms, chỉ có khi `drawn_cursor`.
- `EffectToggles` thêm `cursor` (mặc định bật) và `cursor_size` (mặc định 1,5, giới hạn 1–3).
- Plan thêm `Draw::Cursor { x, y, scale, alpha, shape }`, vẽ trên cùng.
- Xem trước dùng cùng mẫu `smooth` nên khớp với bản xuất.

## Kiểm thử

- **`hp-core`:**
  - base64 đi và về;
  - đọc và ghi `shape` / `cur`;
  - header cũ cho `cursor` = false;
  - đường mượt: theo kịp vị trí đứng yên, lọc nhảy giật;
  - ẩn khi đứng yên lâu và hiện lại khi di chuyển;
  - nhỏ đi khi giữ nút;
  - Plan có `Draw::Cursor` chỉ khi video vẽ con trỏ và ô Con trỏ đang bật.
- **`hp-screen`:**
  - ảnh con trỏ mũi tên chuẩn có điểm ảnh trong suốt và điểm ảnh đặc;
  - compositor vẽ con trỏ đúng chỗ (tính cả điểm nóng);
  - với một video không vẽ con trỏ thì bản xuất giữ nguyên như cũ.
- **Thử tay:** Z27–Z30.
