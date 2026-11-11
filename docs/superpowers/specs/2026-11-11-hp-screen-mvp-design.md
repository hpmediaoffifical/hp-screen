# HP Screen — Thiết kế giai đoạn 1 (bản quay cơ bản)

Ngày: 2026-11-11 · Trạng thái: đã duyệt qua trao đổi, chờ rà lại bản viết

## 1. Mục tiêu

HP Screen là ứng dụng quay màn hình cho Windows 11, nhẹ và nhanh như Bandicam, phát hành miễn phí cho nhiều người dùng, không có key kích hoạt và không giới hạn thời lượng quay. Giai đoạn 1 phải đủ để thay Bandicam cho việc quay hằng ngày.

Toàn bộ dự án chia ba giai đoạn, mỗi giai đoạn có thiết kế và kế hoạch riêng:

1. **Bản quay cơ bản** (tài liệu này).
2. Hậu kỳ: auto-zoom theo click, hiệu ứng click, hiện phím, chỉnh sửa nhanh.
3. AI: phụ đề tiếng Việt, tóm tắt, phân tích.

Giai đoạn 1 đã ghi nhật ký thao tác chuột/phím kèm mỗi video, để video quay từ bây giờ dùng được cho giai đoạn 2.

### Tiêu chí thành công

- Quay được toàn màn hình, một cửa sổ (không dính cửa sổ khác đè lên), và một vùng tùy chọn.
- File ra là MP4 (H.264 + AAC) phát được bằng trình phát mặc định của Windows, hình và tiếng khớp nhau.
- Thu được mic và âm thanh máy theo thiết bị mặc định của Windows.
- Bật/tắt quay bằng phím tắt khi HP Screen không ở trên cùng.
- Tên file đúng mẫu `hp_HH-mm-ss_dd-MM-yyyy.mp4`.
- Ảnh chụp có kích thước đúng bằng số pixel vật lý của nguồn chụp.

## 2. Quyết định đã chốt

| Hạng mục | Quyết định |
|---|---|
| Nền tảng | Chỉ Windows 11 |
| Công nghệ | Tauri 2 + Rust (lõi), React + TypeScript + Vite (giao diện), pnpm |
| Ghi hình | Windows Graphics Capture qua crate `windows-capture` (MIT) |
| Mã hóa | Media Foundation có sẵn trong Windows, ưu tiên bộ mã hóa phần cứng; không đóng gói FFmpeg |
| Bố cục | Bố cục A: thanh điều hướng bên trái với ba mục Quay / Thư viện / Cài đặt |
| Ngôn ngữ giao diện | Tiếng Việt, nền tối |
| Phát hành | Miễn phí, không key; bộ cài NSIS do Tauri tạo |

## 3. Kiến trúc

Hai lớp, ranh giới rõ:

- **Lõi Rust** làm toàn bộ việc nặng. Dữ liệu hình và tiếng không bao giờ đi qua giao diện web.
- **Giao diện** chỉ hiển thị trạng thái và gửi lệnh xuống lõi qua lệnh Tauri; lõi báo ngược bằng sự kiện Tauri (trạng thái quay, thời gian đã quay, lỗi).

### 3.1 Các module của lõi

Mỗi module một nhiệm vụ, giao tiếp qua kiểu dữ liệu rõ ràng. Các module "thuần" (không gọi Windows) được kiểm thử tự động.

| Module | Nhiệm vụ | Phụ thuộc | Thuần |
|---|---|---|---|
| `naming` | Sinh tên file từ mẫu và thời điểm; xử lý trùng tên bằng hậu tố `_2`, `_3` | — | Có |
| `geometry` | Tính vùng cắt, làm chẵn kích thước, quy đổi tọa độ giữa các màn hình | — | Có |
| `state` | Máy trạng thái phiên quay | — | Có |
| `timeline` | Quy đổi mốc thời gian hệ thống sang thời gian video, trừ các quãng tạm dừng | — | Có |
| `quality` | Tính bitrate từ độ phân giải, fps và mức chất lượng | — | Có |
| `input_log` | Định dạng và ghi nhật ký thao tác; lọc phím theo chính sách riêng tư | `timeline` | Phần định dạng và lọc |
| `settings` | Đọc/ghi cài đặt, giá trị mặc định, nâng cấp phiên bản file | — | Có |
| `capture` | Lấy khung hình từ WGC cho ba loại nguồn | `geometry` | Không |
| `audio` | Thu âm máy (loopback) và mic, trộn, bám thiết bị mặc định | — | Phần trộn |
| `encoder` | Ghi MP4 bằng Media Foundation | `quality` | Không |
| `input_hook` | Hook chuột/phím cấp thấp, đẩy sự kiện vào `input_log` | `input_log` | Không |
| `hotkeys` | Đăng ký phím tắt toàn cục, báo xung đột | — | Phần phân tích chuỗi phím |
| `screenshot` | Lấy một khung hình, lưu PNG, copy clipboard | `capture`, `naming` | Không |
| `library` | Quét thư mục lưu, ảnh thu nhỏ, đổi tên, xóa | `naming` | Phần sắp xếp/lọc |
| `recorder` | Điều phối một phiên quay: nối `capture`, `audio`, `encoder`, `input_hook` theo `state` | tất cả ở trên | Không |

### 3.2 Máy trạng thái phiên quay

```
Chờ → Đếm ngược → Đang quay ⇄ Tạm dừng
                      ↓            ↓
                   Hoàn tất  ←─────┘
                      ↓
                     Chờ
```

- Hủy trong lúc Đếm ngược quay về Chờ, không tạo file.
- Hoàn tất là lúc đóng file MP4 và nhật ký, tạo ảnh thu nhỏ; xong mới về Chờ.
- Lệnh không hợp lệ với trạng thái hiện tại (ví dụ Tạm dừng khi đang Chờ) bị từ chối và không đổi trạng thái.

### 3.3 Các cửa sổ

| Cửa sổ | Mô tả |
|---|---|
| Cửa sổ chính | Bố cục A. Đóng cửa sổ thì thu về khay hệ thống, không thoát. |
| Lớp chọn vùng | Phủ mờ toàn màn hình. Kéo để chọn vùng (hiện kích thước theo pixel), hoặc bấm vào một cửa sổ để chọn cửa sổ đó. Esc để hủy. |
| Thanh điều khiển khi quay | Nhỏ, luôn trên cùng: thời gian đã quay, Tạm dừng/Tiếp tục, Dừng. Ẩn khỏi video bằng `WDA_EXCLUDEFROMCAPTURE`. |
| Khay hệ thống | Bắt đầu/Dừng quay, Chụp ảnh, Mở HP Screen, Thoát. |

Chỉ cho chạy một phiên bản ứng dụng; mở lần hai thì đưa cửa sổ đang chạy lên trước.

## 4. Hành vi chi tiết

### 4.1 Ba chế độ quay

- **Toàn màn hình:** chọn màn hình nếu máy có nhiều màn hình; mặc định là màn hình đang chứa cửa sổ HP Screen.
- **Cửa sổ:** quay nội dung của riêng cửa sổ đã chọn, kể cả khi bị cửa sổ khác che. Tắt viền vàng. Nếu cửa sổ bị thu nhỏ, video giữ khung hình cuối và thanh điều khiển hiện cảnh báo. Nếu cửa sổ bị đóng, phiên quay tự dừng và file được lưu. Menu chuột phải và popup là cửa sổ riêng nên có thể không được ghi; đây là giới hạn đã biết của chế độ này, ghi rõ trong giao diện.
- **Vùng chọn:** vùng nằm trong một màn hình. Lõi ghi cả màn hình đó rồi cắt. Nhớ vùng gần nhất để quay lại nhanh.

Kích thước video luôn được làm chẵn (bớt 1 pixel nếu lẻ) vì H.264 yêu cầu. Con trỏ chuột được ghi vào video.

### 4.2 Video và âm thanh

| Thông số | Mặc định | Lựa chọn |
|---|---|---|
| Định dạng | MP4 (H.264 + AAC) | Giai đoạn 1 chỉ có MP4 |
| Tốc độ khung hình | 30 fps, cố định | 30 / 60 |
| Chất lượng | Chuẩn | Nhẹ / Chuẩn / Cao |
| Âm thanh máy | Bật | Bật / Tắt |
| Mic | Bật nếu có thiết bị | Bật / Tắt |
| Âm thanh ra | AAC 48 kHz stereo 192 kbps, mic và âm máy trộn một đường | — |

Bitrate mức Chuẩn lấy mốc 8 Mbps cho 1080p30 và 12 Mbps cho 1080p60, tỉ lệ theo số pixel; Nhẹ bằng 0,6 lần, Cao bằng 1,6 lần.

Âm thanh luôn theo thiết bị mặc định của Windows. Khi người dùng đổi thiết bị mặc định giữa lúc quay, lõi chuyển sang thiết bị mới và chèn im lặng cho quãng chuyển, không làm lệch hình với tiếng.

Tạm dừng: ngừng nhận khung hình và âm thanh; khi tiếp tục, mốc thời gian được dịch lại để video liền mạch.

### 4.3 An toàn khi treo máy

Mục tiêu là video đã quay không mất nếu ứng dụng hoặc máy bị tắt đột ngột. Cách đạt mục tiêu này với Media Foundation **chưa được kiểm chứng**, nên việc đầu tiên của kế hoạch là một phép thử: ghi MP4 phân mảnh rồi tắt nóng tiến trình và kiểm tra file. Nếu MP4 phân mảnh không dùng được, phương án dự phòng theo thứ tự là: (a) ghi tạm ra định dạng an toàn rồi chuyển sang MP4 khi dừng; (b) chấp nhận MP4 thường và ghi rõ giới hạn. Kết quả phép thử được ghi lại vào tài liệu này.

### 4.4 Nhật ký thao tác

Mỗi video có một file nhật ký `<tên video không đuôi>.input.jsonl` (ví dụ `hp_14-30-05_11-11-2026.input.jsonl`) nằm trong thư mục con `.hpscreen` của thư mục lưu. Mỗi dòng là một sự kiện JSON với thời gian tính bằng mili giây theo thời gian video (đã trừ tạm dừng):

- Dòng đầu: thông tin phiên quay (phiên bản định dạng, chế độ, vùng ghi theo tọa độ màn hình, kích thước video, fps).
- `move`: vị trí chuột, lấy mẫu tối đa 60 lần/giây.
- `down` / `up`: nút trái, phải, giữa, kèm vị trí.
- `wheel`: hướng và độ cuộn.
- `key`: tổ hợp phím tắt.

**Chính sách riêng tư, mặc định:** chỉ ghi tổ hợp có Ctrl, Alt hoặc Win, và các phím chức năng/điều hướng (F1–F12, Esc, Enter, Tab, Delete, mũi tên…). Không ghi chữ, số và ký tự gõ thường, để nhật ký không chứa mật khẩu. Người dùng có thể tắt hẳn việc ghi nhật ký trong Cài đặt. Nhật ký chỉ nằm trên máy người dùng, không gửi đi đâu.

### 4.5 Phím tắt

| Lệnh | Mặc định |
|---|---|
| Bật/tắt quay | Ctrl+Alt+R |
| Tạm dừng/tiếp tục | Ctrl+Alt+P |
| Chụp ảnh | Ctrl+Alt+S |

Đổi được trong Cài đặt. Nếu đăng ký thất bại vì trùng với ứng dụng khác, ô phím tắt đó hiện cảnh báo và người dùng chọn tổ hợp khác. F12 không được dùng làm mặc định.

### 4.6 Lưu file

- Thư mục mặc định: `Videos\HP Screen` của người dùng; đổi được trong Cài đặt.
- Tên video: `hp_HH-mm-ss_dd-MM-yyyy.mp4`, ví dụ `hp_14-30-05_11-11-2026.mp4`. Ảnh chụp dùng cùng mẫu với đuôi `.png`.
- Trùng tên thì thêm hậu tố `_2`, `_3`…
- Mẫu tên sửa được trong Cài đặt, có dòng xem trước; các ký hiệu hỗ trợ: `HH`, `mm`, `ss`, `dd`, `MM`, `yyyy`.

### 4.7 Chụp ảnh

Chụp theo chế độ đang chọn (màn hình, cửa sổ, vùng). Ứng dụng khai báo Per-Monitor DPI V2 để ảnh có đúng số pixel vật lý, không bị phóng to hay làm mờ trên màn hình đặt tỉ lệ 125%/150%. Lưu PNG không mất dữ liệu và đồng thời copy vào clipboard.

### 4.8 Thư viện

- Liệt kê video và ảnh trong thư mục lưu, sắp theo thời điểm tạo, mới nhất trên cùng (không sắp theo tên, vì mẫu tên đặt giờ trước ngày).
- Lọc Video / Ảnh. Mỗi mục hiện ảnh thu nhỏ, tên, thời lượng, độ phân giải, dung lượng.
- Thao tác: Phát (mở bằng ứng dụng mặc định của Windows), Mở thư mục, Đổi tên (F2), Xóa (Delete, chuyển vào Thùng rác). Đổi tên và xóa áp dụng luôn cho file nhật ký đi kèm.
- Màn Quay hiện ba mục gần nhất.

### 4.9 Màn Cài đặt

Thư mục lưu; mẫu tên file; fps; chất lượng; đếm ngược (Tắt / 3 giây / 5 giây, mặc định 3); phím tắt; bật/tắt ghi nhật ký thao tác; khởi động cùng Windows (mặc định tắt).

## 5. Xử lý lỗi

| Tình huống | Hành vi |
|---|---|
| Không có mic | Vẫn quay, không có tiếng mic; nút Mic hiện trạng thái "không có thiết bị" |
| Không có thiết bị phát âm thanh | Vẫn quay, không có âm máy |
| Bộ mã hóa phần cứng không khởi tạo được | Tự chuyển sang bộ mã hóa phần mềm của Windows; ghi vào nhật ký lỗi |
| Ổ đĩa sắp đầy (dưới 500 MB) | Dừng quay, đóng file, báo cho người dùng |
| Thư mục lưu không ghi được | Không bắt đầu quay; báo lỗi kèm nút chọn thư mục khác |
| Cửa sổ đang quay bị đóng | Dừng quay, lưu file |
| Phím tắt trùng | Cảnh báo ở màn Cài đặt; ứng dụng vẫn chạy |
| Lỗi giữa chừng khi quay | Cố gắng đóng file đang ghi, về trạng thái Chờ, hiện thông báo lỗi bằng tiếng Việt |

Lõi ghi nhật ký lỗi vào thư mục dữ liệu của ứng dụng để người dùng gửi khi cần hỗ trợ.

## 6. Kiểm thử

- **Tự động (Rust, viết test trước):** `naming`, `geometry`, `state`, `timeline`, `quality`, phần định dạng và lọc phím của `input_log`, `settings`, phần phân tích chuỗi phím của `hotkeys`, phần trộn của `audio`, phần sắp xếp của `library`.
- **Thủ công theo danh sách kiểm:** ba chế độ quay; cửa sổ bị che / thu nhỏ / đóng; đổi thiết bị âm thanh giữa lúc quay; tạm dừng rồi tiếp tục; khớp hình–tiếng trên video 10 phút; ảnh chụp trên màn hình tỉ lệ 150%; phím tắt khi ứng dụng khác đang ở trên; tắt nóng tiến trình giữa lúc quay.
- Các module gọi Windows trực tiếp không có test tự động ở giai đoạn 1; chúng được giữ mỏng và được kiểm bằng danh sách thủ công.

## 7. Ngoài phạm vi giai đoạn 1

Webcam; vẽ khi quay; lịch quay và tự dừng; thu âm riêng từng ứng dụng; nhiều đường tiếng; định dạng ngoài MP4; GIF; chế độ game và "quanh con trỏ"; HDR; chỉnh sửa, auto-zoom, hiệu ứng click (giai đoạn 2); phụ đề và AI (giai đoạn 3); ký số bộ cài (bộ cài chưa ký sẽ bị SmartScreen cảnh báo); tự động cập nhật.

## 8. Rủi ro và điểm chưa kiểm chứng

| Điểm | Cách xử lý |
|---|---|
| MP4 an toàn khi treo máy với Media Foundation | Phép thử đầu kế hoạch (mục 4.3) |
| Tắt viền vàng WGC với ứng dụng không đóng gói MSIX | Thử trên máy phát triển ngay khi có module `capture` |
| `windows-capture` có cho điều khiển đủ sâu bộ mã hóa không | Nếu không, dùng crate cho phần ghi hình và tự viết `encoder` bằng crate `windows` |
| Lệch hình–tiếng khi quay dài | Dùng chung một đồng hồ (QPC) cho hình, tiếng và nhật ký; kiểm bằng video 10 phút |
| Hook chuột/phím bị phần mềm diệt virus nghi ngờ | Chỉ cài hook trong lúc đang quay; gỡ ngay khi dừng |
