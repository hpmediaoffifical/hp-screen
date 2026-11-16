# Quy trình phát hành bản mới

HP Screen cập nhật qua **Tauri updater**: app tự kiểm tra GitHub Releases, tải file `.exe`
mới có chữ ký số (minisign) và tự cài. Tài liệu này là các bước để ra một bản đúng cách.

## 0. Chuẩn bị (chỉ làm một lần)

- Cặp khóa updater nằm ở `%USERPROFILE%\.tauri\` — private key (`.key`) và public key
  (`.key.pub`). Public key tương ứng đã nhúng trong `src-tauri/tauri.conf.json` →
  `plugins.updater.pubkey`.
- **Mất private key = không bao giờ phát hành bản update được nữa.** Hãy copy key ra nơi
  an toàn (ổ dự phòng, trình quản lý mật khẩu).
- Thêm secret cho GitHub repo (Settings → Secrets and variables → Actions):
  - `TAURI_SIGNING_PRIVATE_KEY` = toàn bộ nội dung file `.key`
  - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = mật khẩu key (để trống nếu key không mật khẩu)

## 1. Phát hành tự động (khuyên dùng)

1. Bump version ở 4 nơi: `package.json`, `src-tauri/Cargo.toml`, `crates/hp-core/Cargo.toml`,
   `src-tauri/tauri.conf.json` (giữ nhau cùng số).
2. Commit, đẩy lên GitHub, rồi đánh tag và đẩy tag:
   ```powershell
   git tag v0.2.2
   git push origin v0.2.2
   ```
3. GitHub Actions `.github/workflows/release.yml` tự: kiểm tra version khớp tag → chạy test
   → build installer NSIS + ký chữ ký updater (qua `tauri-apps/tauri-action`,
   `includeUpdaterJson: true`) → mở GitHub Release kèm `latest.json`.
4. Người dùng cũ thấy thông báo bản mới trong góc cửa sổ chính; cài một chạm, app tự khởi
   động lại.

Mỗi push bình thường chỉ chạy CI test (`.github/workflows/ci.yml`), không tốn bản build.

## 2. Phát hành thủ công (khi không dùng CI)

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content "$env:USERPROFILE\.tauri\hp-screen.key" -Raw)
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
pnpm tauri build
pwsh scripts\make-release.ps1
```

Sau đó upload lên GitHub Release (tag `v<phiên bản>`) 3 file:
- `HP Screen_<phiên bản>_x64-setup.exe`
- `HP Screen_<phiên bản>_x64-setup.exe.sig`
- `release/latest.json` (từ script)

`latest.json` phải nằm ở **asset gốc của release** để endpoint
`releases/latest/download/latest.json` tìm thấy.

## 3. Kiểm tra sau khi phát hành

- Tải `latest.json` từ release về, mở kiểm tra: `version`, `url`, `signature` khớp.
- Trên máy có bản cũ: mở app → chờ thông báo "bản mới" ở góc phải-dưới → Cài đặt → app tự
  khởi động lại đúng bản mới.
- File log nếu cần soi: `%LOCALAPPDATA%\hp-screen\logs\hp-screen.log` (dòng `[diag]` ghi lại
  từng lần bấm phím tắt — dùng truy vết khi phím "không tới lệnh").

## Lưu ý

- **Ký số bộ cài (code signing) thật** chưa có: Windows SmartScreen sẽ vẫn cảnh báo lần đầu
  cài. Khi có chứng chỉ (OV/EV), thêm bước `signtool sign` vào `release.yml`.
- Bộ cài chưa ký KHÔNG ảnh hưởng updater: updater chỉ tin chữ ký minisign của Tauri.
- Đừng đổi `pubkey` trong `tauri.conf.json` trừ khi sẵn sàng cắt đường cập nhật của toàn bộ
  bản đã phát hành. Nếu buộc phải xoay khóa (mất khóa cũ): sinh khóa mới
  (`pnpm tauri signer generate -w %USERPROFILE%\.tauri\hp-screen.key --password '""'`),
  cập nhật `pubkey`, cập nhật secret trên GitHub — và nhớ rằng các bản đã phát hành bằng
  khóa cũ sẽ không tự cập nhật lên bản mới nữa.
