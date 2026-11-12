# HP Screen

Ứng dụng quay màn hình và chụp ảnh cho Windows 11. Nhẹ, nhanh, miễn phí, không cần key và không giới hạn thời lượng quay.

- **Thuộc:** HP Media
- **Tác giả:** Nguyễn Hoàng Phụng
- **Mã nguồn:** https://github.com/hpmediaoffifical/hp-screen
- **Giấy phép:** MIT (xem [LICENSE](LICENSE))

## Tính năng

- Quay toàn màn hình, một cửa sổ (không dính cửa sổ khác đè lên) hoặc một vùng tùy chọn.
- File MP4 (H.264 + AAC) ghi dạng phân mảnh: máy treo hay tắt nóng vẫn giữ được video.
- Thu âm mic và âm thanh máy theo thiết bị mặc định của Windows.
- Phím tắt toàn cục để bắt đầu/dừng, tạm dừng và chụp ảnh; thanh điều khiển gọn không lọt vào video.
- Chụp ảnh đúng pixel vật lý, vẽ chú thích, copy vào clipboard.
- Thư viện video và ảnh đã quay: xem, phát, đổi tên, xóa.
- Ghi kèm nhật ký chuột/phím cho mỗi video, dùng cho hiệu ứng click và hiện phím tắt khi xuất.

## Công nghệ

- [Tauri 2](https://tauri.app/) + Rust: lõi gọi thẳng Windows API.
  - Windows Graphics Capture để ghi hình.
  - Media Foundation để mã hóa video.
  - WASAPI qua `cpal` để thu âm.
- React + TypeScript cho giao diện tiếng Việt.
- `crates/hp-core`: phần logic thuần, có kiểm thử tự động.

## Chạy từ mã nguồn

Cần Windows 11, [Rust](https://rustup.rs/), Node.js và [pnpm](https://pnpm.io/).

```bash
pnpm install
```

```bash
pnpm tauri dev
```

Kiểm thử:

```bash
cargo test -p hp-core
```

```bash
cargo test -p hp-screen --lib
```

```bash
pnpm tsc --noEmit
```

---

© 2026 HP Media — Nguyễn Hoàng Phụng
