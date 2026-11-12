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

## Zoom (giai đoạn 2, bước 5)

| # | Việc cần thử | Máy | Người |
|---|---|---|---|
| Z1 | Thẻ Zoom → "Bật zoom" trên một video có click: rãnh Zoom hiện các đoạn tự tạo quanh chỗ click | Đạt (`clicks_close_in_time_and_place_make_one_segment`) | |
| Z2 | Phát xem trước: khung phóng to mượt vào chỗ click rồi thu về, không giật, không nảy | Đạt (`zoom_eases_in_and_out`) | |
| Z3 | Kéo trên chỗ trống của rãnh Zoom để thêm một đoạn; kéo mép để đổi thời gian; kéo giữa để dời | — | |
| Z4 | Chọn một đoạn: đổi mức phóng bằng thanh trượt (trên 2× có cảnh báo); "Chọn tâm trên video" rồi bấm lên video | — | |
| Z5 | Delete xóa đoạn zoom đang chọn; Ctrl+Z khôi phục; "Tự tạo lại theo click" giữ các đoạn bạn đã sửa | Đạt (`regenerate_keeps_manual_segments`) | |
| Z6 | Xuất MP4: chỗ zoom giống hệt xem trước; vòng click phóng theo, chip phím thì không | Đạt cho khung hình (`zoomed_export_shows_the_zoomed_area`) | |
| Z7 | Zoom sát mép hoặc góc màn hình: không lộ phần ngoài video | Đạt (`corner_center_is_kept_inside_the_video`) | |
| Z8 | Video không có nhật ký thao tác: vẫn thêm zoom bằng tay được | — | |
| Z9 | Bấm một đoạn trên rãnh Zoom: video dừng, hiện toàn bộ hình với khung zoom; kéo giữa khung để dời, bấm Phát thì thấy zoom đúng vùng đó | Đạt cho phép tính khung (`zoomMath.test.ts`) | |
| Z10 | Kéo góc khung: phóng to/nhỏ trong 1–4×, góc đối diện đứng yên, khung không ra ngoài hình; ở 9:16 "Lấp đầy" khung vẫn đúng | Đạt (`resize keeps the opposite corner`, `frameRoundTripsInFillMode`) | |
| Z11 | Trong một đoạn, tua tới giữa đoạn, bấm "◆ Thêm điểm" (hoặc K), kéo khung sang chỗ khác: camera lia từ khung đầu sang khung mới; thêm điểm thứ ba lia tiếp | Đạt cho bản xuất (`keyframes_pan_the_export`) | |
| Z12 | Hình thoi trên rãnh Zoom: kéo đổi giờ, bấm để tua tới, Delete xóa điểm (Delete lần nữa mới xóa cả đoạn) | — | |
| Z13 | Tốc độ Chậm / Vừa / Nhanh / Tức thì: thấy khác rõ; Tức thì là cắt thẳng | Đạt (`slow_is_slower_and_fast_is_faster`, `instant_jumps_to_the_target`) | |
| Z14 | Bấm Z tại một chỗ: thêm đoạn 3 giây ở vị trí con trỏ chuột lúc đó và được chọn ngay; gần cuối video thì báo không đủ chỗ | Đạt (`zAtTheEndAddsNothing`) | |
| Z15 | Tự động Ít / Vừa / Nhiều: số đoạn ít dần ở mức Ít; không còn zoom vào thanh tác vụ khi quay toàn màn hình; "Xóa các đoạn tự động" giữ đoạn vàng | Đạt (`low_level_needs_more_than_a_lone_click`, `edge_clicks_are_ignored_when_guarded`) | |
| Z16 | Quay mới: click vào ô rồi gõ chữ một lúc: zoom giữ trong lúc gõ và trở ra khoảng 1 giây sau; rê chuột đi xa thì trở ra sớm | Đạt (`typing_keeps_the_zoom`, `leaving_the_view_ends_early`, `plain_keys_leave_a_type_mark_without_the_key`) | |
| Z17 | Bật zoom: rãnh Zoom trống, không tự phủ cả video. Kéo nút "+ Zoom tay" xuống rãnh ở giây bất kỳ: có đoạn vàng 3 giây ở đó với khung để chỉnh | Đạt trên trang thử || Đạt (người dùng xác nhận 2026-11-12) |
| Z18 | Kéo nút "+ Tự zoom theo click" xuống rãnh: có vùng nét đứt "Tự động" 10 giây, bên trong tự có các đoạn xanh theo click; ngoài vùng giữ toàn màn hình | Đạt (`auto_zoom_only_inside_regions`, `automatic_segments_stay_inside_their_region`) || Đạt (người dùng xác nhận 2026-11-12) |
| Z19 | Kéo dời / kéo mép vùng tự động: đoạn xanh tính lại; chọn vùng rồi Delete: vùng và đoạn xanh biến mất, đoạn vàng giữ nguyên | Đạt trên trang thử || Đạt (người dùng xác nhận 2026-11-12) |
| Z20 | Quay mới: bấm vài nút nhỏ (ví dụ nút Lưu, mục menu). Trong vùng "Tự zoom theo click", khung zoom ôm vừa nút được bấm (nút nhỏ phóng tới 2,5×), không còn luôn 2× quanh chuột | Đạt (`the_zoom_fits_the_clicked_element`) || Đạt (người dùng xác nhận 2026-11-12) |
| Z21 | Bấm vào một hộp thoại lớn: zoom vừa cả hộp thoại, không phóng sát vào một góc | Đạt (`the_zoom_fits_the_clicked_element`) || Đạt (người dùng xác nhận 2026-11-12) |
| Z22 | Quay mới: rê chuột ra chỗ khác rồi gõ chữ vào một ô: zoom tới đúng ô đang gõ | Đạt (`typing_zooms_on_the_field_being_typed_in`) || Đạt (người dùng xác nhận 2026-11-12) |
| Z23 | Quay mới: chuyển sang cửa sổ khác (Alt+Tab hoặc bấm thanh tác vụ) giữa hai lần thao tác: camera lùi ra toàn cảnh rồi mới zoom vào chỗ mới | Đạt (`a_window_switch_zooms_out_between_groups`, `window_switches_are_logged_relative_to_the_capture_with_no_title`) || Đạt (người dùng xác nhận 2026-11-12) |
| Z24 | Chọn một đoạn zoom → Kiểu "Làm tối xung quanh": video không phóng to; vùng trong khung sáng (góc bo, viền mảnh), phần còn lại tối đi; rãnh ghi "Spot". Kéo khung hoặc thêm điểm ◆: vùng sáng di chuyển theo | Đạt (`a_spotlight_lights_the_frame_without_zooming`, `a_spotlight_darkens_only_outside_its_box`), xem trước đã kiểm trên trang thử || Đạt (người dùng xác nhận 2026-11-12) |
| Z25 | Xuất MP4 có đoạn spotlight: vùng tối hiện dần khi vào đoạn, tắt dần khi ra | Đạt (`a_spotlight_is_drawn_over_the_video`) || Đạt (người dùng xác nhận 2026-11-12) |
| Z26 | Ô "Nhòe chuyển động khi zoom" (mặc định bật): bản xuất nhòe nhẹ lúc phóng to/thu nhỏ/lia, khung đứng yên vẫn nét; tắt ô thì không nhòe. Khi phát xem trước cũng hơi nhòe lúc camera chạy | Đạt (`motion_blur_smears_the_zoom_in_and_not_still_frames`, `motion_blur_only_while_the_camera_moves`) || Đạt (người dùng xác nhận 2026-11-12) |
| Z27 | Cài đặt → bật "Con trỏ mượt" (cần nhật ký thao tác). Quay một đoạn có rê chuột, bấm, gõ: file MP4 gốc không có chuột | — || Đạt (người dùng xác nhận 2026-11-12) |
| Z28 | Mở video đó ở trang Chỉnh sửa: xem trước và bản xuất có con trỏ đúng hình (mũi tên, chữ I, bàn tay), đi mượt, không rung | Đạt cho đường mượt và vẽ (`hand_jitter_is_filtered`, `a_recording_without_the_windows_cursor_exports_with_the_drawn_one`) || Đạt (người dùng xác nhận 2026-11-12) |
| Z29 | Chuột đứng yên khoảng 1,5 giây thì con trỏ mờ dần rồi ẩn, nhúc nhích thì hiện ngay; giữ nút thì con trỏ nhỏ lại một chút | Đạt (`an_idle_cursor_fades_out_and_comes_back_when_moved`, `a_held_button_keeps_it_shown_and_presses_it`) || Đạt (người dùng xác nhận 2026-11-12) |
| Z30 | Thẻ Hiệu ứng: "Con trỏ mượt" bật/tắt, thanh "Cỡ con trỏ" 1–3×; con trỏ phóng to theo zoom. Tắt "Con trỏ mượt" ở Cài đặt thì quay lại như cũ (file gốc có chuột) | Đạt cho cỡ mặc định (`the_cursor_is_drawn_only_for_recordings_without_the_windows_cursor`) || Đạt (người dùng xác nhận 2026-11-12) |

## Khung hình và GIF (giai đoạn 2, bước 6)

| # | Việc cần thử | Máy | Người |
|---|---|---|---|
| F1 | Thẻ Khung → 9:16, "Vừa khung": xem trước có nền trên và dưới, video ở giữa | Đạt (`tall_export_has_the_frame_size_and_background`) | |
| F2 | 9:16, "Lấp đầy": video kín khung; bật zoom thì vùng cắt đi theo zoom | Đạt (`fill_export_crops`) | |
| F3 | Đổi nền (6 mẫu), khoảng đệm, bo góc, bóng đổ: xem trước và bản xuất giống nhau | Đạt cho bản xuất (test vẽ của compositor) | |
| F4 | Xuất MP4 ở 1:1 và 9:16: file có đúng tỉ lệ | Đạt (`tall_export_has_the_frame_size_and_background`) | |
| F5 | Thẻ Xuất → GIF, 15 fps, 720 px: file `_edit.gif` mở được trong trình duyệt, chạy lặp, đúng tốc độ | Đạt (`gif_has_the_frames_and_size`, `frames_size_timing_and_colour_survive`) | |
| F6 | GIF hiện trong Thư viện (loại Ảnh) | Đạt cho cách xếp loại (`gif_names_do_not_overwrite`) | |
| F7 | Quay một vùng chọn có kích thước bất kỳ (ví dụ 301×201), xuất ra: hình không bị lệch hay xéo | Đạt (`widths_that_are_not_multiples_of_16_are_not_skewed`) | |

## Phụ đề tiếng Việt (giai đoạn 2, bước 7)

Cần mạng cho lần đầu (tải khoảng 584 MB với model "Chính xác", 157 MB với model "Nhẹ"). Sau đó không cần mạng.

| # | Việc cần thử | Máy | Người |
|---|---|---|---|
| S1 | Thẻ Phụ đề → "Tạo phụ đề" lần đầu: hiện hộp hỏi, ghi rõ dung lượng, nguồn tải (github.com, huggingface.co) và thư mục lưu. "Để sau" thì không tải gì | — | |
| S2 | Đồng ý tải: thanh tiến độ chạy theo MB, bấm Hủy thì dừng; tải lại thì xong và tự tạo phụ đề | Đạt cho kiểm mã SHA-256 và giải nén (`sha256_of_a_known_file`, `zip_with_the_cli_in_a_subfolder`) | |
| S3 | Video nói tiếng Việt 1–3 phút: phụ đề đúng phần lớn lời, mỗi dòng không quá 2 hàng, khớp thời gian | Đạt cho cách chia dòng (`long_segments_are_split_into_readable_cues`) | |
| S4 | Sửa chữ một dòng, Gộp, Tách tại đây, Xóa, Đầu / Cuối; Ctrl+Z lấy lại được | Đạt cho phép tính (`subsMath.test.ts`) | |
| S5 | Bấm vào giờ của một dòng: video tua tới đó; xem trước hiện phụ đề đang phát | — | |
| S6 | Bật "Ghi cứng phụ đề vào video", xuất MP4: chữ nằm gần cạnh dưới, nền tối; phím tắt (nếu có) nằm trên phụ đề | Đạt (`subtitle_is_drawn_near_the_bottom`, `chips_move_above_a_burned_subtitle`) | |
| S7 | "Xuất SRT": file `_edit.srt` cạnh video, mở trong CapCut hoặc VLC thấy đúng dấu tiếng Việt và đúng giờ (cả khi đã cắt bớt đoạn giữa) | Đạt (`srt_is_written_next_to_the_video_without_overwriting`, `srt_uses_output_time`) | |
| S8 | Video không có tiếng: nút Tạo phụ đề tắt, có lời giải thích; vẫn "Thêm dòng tại đây" bằng tay được | Đạt cho phát hiện không có tiếng (`a_silent_video_gives_no_wav`) | |
