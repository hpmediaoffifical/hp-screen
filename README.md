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

## Phát hành và tự cập nhật

HP Screen tự kiểm tra bản mới trên [GitHub Releases](https://github.com/hpmediaoffifical/hp-screen/releases) và hỏi người dùng trước khi cài. Cách phát hành một bản mới:

1. Tăng `version` trong `src-tauri/tauri.conf.json` (và cho khớp trong `package.json`, `src-tauri/Cargo.toml`).
2. Commit, rồi đẩy tag cùng số phiên bản, ví dụ `v0.2.0`.
3. Workflow `.github/workflows/release.yml` build bộ cài Windows, ký và đăng Release kèm `latest.json`.

Bản cài được ký bằng khóa riêng của HP Media, lưu trong GitHub Secrets `TAURI_SIGNING_PRIVATE_KEY` và `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Khóa riêng không bao giờ nằm trong repo. Mất khóa riêng thì các bản đã cài sẽ không nhận được cập nhật nữa, nên hãy giữ một bản sao an toàn.

Build bộ cài có ký trên máy (cần hai biến môi trường trên):

```bash
pnpm tauri build --config src-tauri/tauri.release.conf.json
```

---

© 2026 HP Media — Nguyễn Hoàng Phụng
