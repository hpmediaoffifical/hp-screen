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
