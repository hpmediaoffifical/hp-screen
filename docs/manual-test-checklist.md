# HP Screen 0.1.0 — danh sách kiểm thủ công

Cột "Máy" ghi những gì đã được kiểm bằng lệnh hoặc bằng thao tác chuột/phím giả lập trên máy phát triển (Windows 11 build 26200, màn hình 2560×1600 tỉ lệ 150%, RTX 4060 + Intel Arc) ngày 2026-11-11. Cột "Người" để trống cho đến khi có người thử bằng mắt và tai.

Chạy kiểm thử tự động:

```bash
cargo test -p hp-core
```

```bash
cargo test -p hp-screen --lib
```

Kiểm thử cần màn hình thật (quay và chụp thật, di chuyển chuột vài pixel):

```bash
cargo test -p hp-screen --lib -- --ignored --test-threads=1
```

## Quay video

| # | Việc cần thử | Máy | Người |
|---|---|---|---|
| 1 | Quay toàn màn hình 15 giây, tạm dừng 5 giây ở giữa: video dài khoảng 10 giây, phát được | Đạt (3 giây + tạm dừng 2 giây: 98 khung hình, 3,2 giây) | |
| 2 | Video phát và tua tới/lui được trong trình phát quen dùng | — | Đạt (người dùng xác nhận 2026-11-11, bản chưa có tiếng) |
| 3 | Quay một cửa sổ đang bị cửa sổ khác che: video chỉ có cửa sổ đó | Chưa thử | |
| 4 | Quay cửa sổ rồi thu nhỏ nó: thanh điều khiển hiện cảnh báo, video giữ khung hình cuối | Chưa thử | |
| 5 | Quay cửa sổ rồi đóng nó: phiên quay tự dừng, file được lưu | Chưa thử | |
| 6 | Quay cửa sổ, mở menu chuột phải: ghi lại xem menu có vào video không | Chưa thử | |
| 7 | Quay vùng chọn: kích thước video khớp vùng (số lẻ bị bớt 1 pixel) | Đạt (vùng 699×450 → 698×450; 641×481 → 640×480) | |
| 8 | Vùng chọn trên màn hình phụ nằm đúng vị trí | Chưa thử (máy chỉ có một màn hình) | |
| 9 | Vùng nhỏ hơn 64 px bị từ chối ngay trên lớp chọn vùng | Một phần (lõi từ chối; chưa thử trên giao diện) | |
| 10 | Không có viền vàng quanh màn hình/cửa sổ khi quay | Chưa thử (cần nhìn bằng mắt) | |
| 11 | Thanh điều khiển hiện ở đáy màn hình khi quay và không có trong video | Chưa thử (thanh bị ẩn khỏi mọi ảnh chụp nên máy không tự thấy) | |
| 12 | Tắt nóng HP Screen giữa lúc quay (Task Manager): file vẫn mở được | Một phần (đạt với video không tiếng trong phép thử riêng; chưa thử bản có tiếng) | |

## Âm thanh

| # | Việc cần thử | Máy | Người |
|---|---|---|---|
| 13 | Quay khi đang phát nhạc và nói vào mic: nghe được cả hai trong video | Một phần (file có đường tiếng dài bằng video; chưa nghe) | |
| 14 | Tắt Mic hoặc Âm thanh máy ở trang Quay: nguồn đó không còn trong video | Chưa thử | |
| 15 | Đổi thiết bị phát mặc định giữa lúc quay (rút tai nghe): tiếng tiếp tục sau tối đa khoảng 1 giây, video không hỏng | Chưa thử | |
| 16 | Quay 10 phút video có đồng hồ và tiếng bíp mỗi giây: cuối video hình và tiếng lệch dưới 0,1 giây | Chưa thử | |

## Phím tắt, khay hệ thống

| # | Việc cần thử | Máy | Người |
|---|---|---|---|
| 17 | Đang ở ứng dụng khác, bấm Ctrl+Alt+R: đếm ngược rồi quay; bấm lần nữa: dừng | Đạt phần lớn. **Có 2 trong khoảng 8 lần chạy bằng phím giả lập, lệnh dừng không tới ứng dụng; chưa tìm ra nguyên nhân, chưa lặp lại được theo ý muốn.** Cần người bấm phím thật nhiều lần | |
| 18 | Ctrl+Alt+P tạm dừng và tiếp tục | Đạt | |
| 19 | Bấm Ctrl+Alt+R thật nhanh 5 lần: không có hai phiên quay, không treo | Một phần (logic trạng thái có kiểm thử; chưa thử bấm thật) | |
| 20 | Ctrl+Alt+S chụp ảnh | Không đạt trên máy phát triển: một ứng dụng khác đang giữ tổ hợp này; trang Cài đặt báo đúng. Đổi sang tổ hợp khác rồi thử lại | |
| 21 | Đổi phím tắt trong Cài đặt sang tổ hợp đang bị ứng dụng khác giữ: hiện cảnh báo | Một phần (cảnh báo sinh ra đúng; chưa nhìn trên giao diện) | |
| 22 | Đóng cửa sổ chính: ứng dụng còn trong khay hệ thống, phím tắt vẫn chạy | Chưa thử | |
| 23 | Menu khay: Bắt đầu/Dừng quay, Chụp ảnh, Mở HP Screen, Thoát (thoát khi đang quay vẫn lưu file) | Chưa thử | |
| 24 | Mở HP Screen lần hai: cửa sổ đang chạy hiện lên, không có bản thứ hai | Chưa thử | |

## Nhật ký thao tác

| # | Việc cần thử | Máy | Người |
|---|---|---|---|
| 25 | Sau khi quay có file `.hpscreen\<tên>.input.jsonl` với di chuyển, click, cuộn chuột | Đạt (di chuyển, cuộn; click chỉ có kiểm thử định dạng) | |
| 26 | Gõ một đoạn văn vào Notepad khi quay: nhật ký không có chữ đã gõ, chỉ có tổ hợp như Ctrl+C | Một phần (bộ lọc có kiểm thử; chưa gõ thật) | |
| 27 | Sự kiện trong lúc tạm dừng không được ghi | Đạt | |
| 28 | Tắt "Ghi nhật ký thao tác" trong Cài đặt: không còn file nhật ký mới | Chưa thử | |

## Chụp ảnh

| # | Việc cần thử | Máy | Người |
|---|---|---|---|
| 29 | Ảnh toàn màn hình có đúng số pixel vật lý trên màn hình tỉ lệ 150% | Đạt (2560×1600) | |
| 30 | Ảnh vùng chọn giữ nguyên kích thước lẻ | Đạt (301×201) | |
| 31 | Dán được ảnh vừa chụp từ clipboard vào Paint | Chưa thử | |
| 32 | Chụp hai lần trong cùng một giây: hai file, file sau có đuôi `_2` | Một phần (đặt tên có kiểm thử) | |

## Thư viện và Cài đặt

| # | Việc cần thử | Máy | Người |
|---|---|---|---|
| 33 | Video/ảnh mới hiện trên cùng, có ảnh thu nhỏ, thời lượng, độ phân giải, dung lượng | Đạt một phần (lưới và ảnh thu nhỏ đã thấy qua ảnh chụp; video quay trước khi có tính năng này không hiện thời lượng) | |
| 34 | Phát, Thư mục, F2 đổi tên (file nhật ký đổi theo), Delete xóa vào Thùng rác | Một phần (đổi tên có kiểm thử; chưa bấm trên giao diện; xóa chưa thử) | |
| 35 | Lọc Tất cả / Video / Ảnh | Chưa thử trên giao diện | |
| 36 | Đổi thư mục lưu: file mới vào thư mục mới | Chưa thử | |
| 37 | Sửa mẫu tên file: dòng ví dụ đổi theo, file mới dùng tên mới | Một phần (trang hiển thị đúng) | |
| 38 | Sửa file cài đặt thành nội dung rác rồi mở ứng dụng: chạy với mặc định | Một phần (kiểm thử phần đọc cài đặt) | |
| 39 | Bật "Khởi động cùng Windows", đăng xuất rồi đăng nhập lại | Chưa thử | |
| 40 | Ổ đĩa còn dưới 500 MB: không cho bắt đầu quay; đang quay thì tự dừng và lưu | Chưa thử (chưa mô phỏng ổ đầy) | |

## Bộ cài

| # | Việc cần thử | Máy | Người |
|---|---|---|---|
| 41 | `pnpm tauri build` tạo được bộ cài NSIS | Xem kết quả ở cuối file | |
| 42 | Cài trên một máy Windows 11 khác, chạy các mục 1, 13, 17 | Chưa thử. Bộ cài chưa ký số nên SmartScreen sẽ cảnh báo | |

## Chỉnh sửa và xuất (giai đoạn 2, bước 1–2)

Cột "Máy" ghi những gì kiểm thử tự động đã phủ (video mẫu do chính bộ mã hóa của app tạo, không cần màn hình). Cột "Người" để trống cho đến khi có người thử.

| # | Việc cần thử | Máy | Người |
|---|---|---|---|
| E1 | Thư viện → thẻ video → "Chỉnh sửa": trang Chỉnh sửa mở ra, có hình video | — | Đạt (người dùng xác nhận 2026-11-12) |
| E2 | Bấm ▶ hoặc Space: video phát; tua bằng cách bấm lên dòng thời gian, bấm ←/→ (Shift + ←/→ là 1 giây) | — | Đạt (người dùng xác nhận 2026-11-12) |
| E3 | Kéo hai tay vàng ở đầu và cuối dòng thời gian: khi phát, video bắt đầu và dừng đúng chỗ đó | Đạt (`frame_times_skip_the_removed_span`) | Đạt (người dùng xác nhận 2026-11-12) |
| E4 | Kéo chọn một đoạn giữa (hoặc I rồi O) → "Bỏ đoạn này": đoạn thành sọc, khi phát nhảy qua | Đạt (`export_drops_the_removed_span`) | Đạt (người dùng xác nhận 2026-11-12) |
| E5 | Bấm vào đoạn sọc → "Giữ lại": đoạn được giữ lại | — | Đạt (người dùng xác nhận 2026-11-12) |
| E6 | Ctrl+Z / Ctrl+Y hoàn tác và làm lại; một lần kéo tay cắt là một bước hoàn tác | — | Đạt (người dùng xác nhận 2026-11-12) |
| E7 | Thoát trang rồi mở lại: các chỉnh sửa vẫn còn | Đạt (`save_then_load_round_trips`) | Đạt (người dùng xác nhận 2026-11-12) |
| E8 | Thẻ Xuất → "Xuất MP4": thanh tiến độ chạy, xong hiện "Đã lưu …_edit.mp4"; thẻ mới có trong Thư viện | Đạt (`export_writes_thumbnail_and_meta`) | Đạt (người dùng xác nhận 2026-11-12) |
| E9 | Phát file `_edit.mp4`: đúng phần giữ lại, hình và tiếng khớp, chỗ nối không có tiếng "bụp" | Đạt một phần (số khung, không còn khung của đoạn bỏ); tai người chưa nghe | Đạt (người dùng xác nhận 2026-11-12) |
| E10 | File gốc vẫn còn nguyên, phát được như cũ | Đạt (`export_never_touches_the_source`) | Đạt (người dùng xác nhận 2026-11-12) |
| E11 | Bấm "Hủy" giữa lúc xuất: không có file `_edit` dở trong thư mục | Đạt (`cancelled_export_leaves_no_file`) | Đạt (người dùng xác nhận 2026-11-12) |
| E12 | Xuất lần hai: file mới tên `_edit_2.mp4`, file cũ không bị ghi đè | Đạt (`export_name_does_not_overwrite`) | Đạt (người dùng xác nhận 2026-11-12) |
| E13 | Quay một đoạn có click trái, phải, giữa, kéo thả, cuộn chuột, bấm Ctrl+C hai lần: xem trước có vòng màu đúng nút, chấm khi giữ nút, biểu tượng cuộn, chip "Ctrl + C ×2" | Đạt cho bản xuất (`export_draws_click_effects`, test vẽ của compositor) | Đạt (người dùng xác nhận 2026-11-12) |
| E14 | Thẻ Hiệu ứng: tắt "Vòng click" / "Phím tắt" thì xem trước và bản xuất đều không còn hiệu ứng đó | Đạt cho bản xuất (`toggles_hide_effects`) | Đạt (người dùng xác nhận 2026-11-12) |
| E15 | Hiệu ứng trong xem trước trông giống bản xuất (vị trí, màu, cỡ) | — | Đạt (người dùng xác nhận 2026-11-12) |
| E16 | Mở một video cũ (quay trước khi có nhật ký) hoặc một file `_edit.mp4`: vẫn cắt và xuất được; thẻ Hiệu ứng báo "không có nhật ký thao tác" | — | Đạt (người dùng xác nhận 2026-11-12) |
| E17 | Đổi tên video đã chỉnh sửa rồi mở lại trang Chỉnh sửa: chỉnh sửa đi theo | Đạt (`rename_moves_companions_and_cleans_the_name`) | |
| E18 | Đang quay thì bấm Xuất: báo "Đang quay, hãy xuất sau khi dừng" | — | |

## Vẽ khi quay và webcam (giai đoạn 2, bước 3–4)

| # | Việc cần thử | Máy | Người |
|---|---|---|---|
| D1 | Đang quay, bấm "✎ Vẽ" trên thanh gọn (hoặc Ctrl+Alt+D): thanh công cụ vẽ hiện ở cạnh trên vùng quay | — | Đạt (người dùng xác nhận 2026-11-12) |
| D2 | Vẽ bằng bút, mũi tên, khung với vài màu và độ dày: nét hiện ngay trên màn hình | — | Đạt (người dùng xác nhận 2026-11-12) |
| D3 | Thả chuột: nét giữ 3 giây rồi mờ dần; video quay ra cũng có nét và cũng mờ y như vậy | Đạt cho khung hình (`stroke_is_drawn_onto_the_frame`); chưa thử với video quay thật | Đạt (người dùng xác nhận 2026-11-12) |
| D4 | Esc hoặc "Thoát": chuột lại bấm được vào ứng dụng bên dưới; nét cũ vẫn mờ dần | — | Đạt (người dùng xác nhận 2026-11-12) |
| D5 | "Xóa hết": mọi nét biến mất cả trên màn hình lẫn trong video | Đạt (`clear_removes_strokes_and_upsert_replaces_by_id`) | |
| D6 | Thử ở cả ba chế độ quay: toàn màn hình, cửa sổ, vùng | — | Đạt (người dùng xác nhận 2026-11-12) |
| D7 | Xuất video vừa quay có vẽ: những click lúc vẽ không thành vòng click | Đạt (`clicks_are_dropped_while_drawing`) | |
| D8 | Cài đặt → "Nét vẽ khi quay tự mờ sau": 10 giây và "Không tự mờ" đều đúng | Đạt (`alpha_holds_then_fades`) | |
| D9 | Dừng quay khi đang vẽ: lớp vẽ đóng; lần quay sau không còn nét cũ | Đạt (`cancelling_clears_the_live_layer`) | |
| W1 | Đang quay, bấm "📷 Webcam": đèn camera sáng, bong bóng hiện ở góc dưới phải, thấy mặt mình (lật gương) | — | Đạt (người dùng xác nhận 2026-11-12) |
| W2 | Video quay ra có webcam đúng góc đó, tròn, viền trắng mảnh | Đạt cho khung hình (`webcam_is_drawn_in_its_bubble`) | Đạt (người dùng xác nhận 2026-11-12) |
| W3 | Cài đặt → Webcam: đổi sang bo góc, cỡ lớn, góc trên trái, tắt lật gương; lần quay sau đúng như vậy | Đạt (`bubble_rect`, `mirror_flips_the_picture`) | Đạt (người dùng xác nhận 2026-11-12) |
| W4 | Trang Quay bật "Webcam": bấm quay thì webcam tự bật | Đạt (`webcam_starts_once_per_recording_when_enabled`) | |
| W5 | Dừng quay: bong bóng đóng, đèn camera tắt | — | Đạt (người dùng xác nhận 2026-11-12) |
| W6 | Rút dây camera giữa lúc quay: thanh gọn báo lỗi, việc quay vẫn tiếp tục | — | |
| W7 | Mở camera bằng app khác (Camera của Windows) rồi bật webcam trong HP Screen: báo "Camera đang được ứng dụng khác dùng" | Đạt cho thông báo (`camera_errors_are_explained`) | |
| R1 | Quay một **vùng chọn**: quanh vùng có viền trắng nét đứt (từ lúc đếm ngược tới khi dừng); chuột bấm xuyên qua viền; video quay ra không có viền | Đạt cho vị trí (`the_border_sits_just_outside_the_region`) | |
