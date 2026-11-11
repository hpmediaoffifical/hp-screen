# HP Screen — Thiết kế nâng cấp quay video (giai đoạn 2)

Ngày: 2026-11-12 · Trạng thái: hướng đi đã duyệt qua trao đổi, chờ rà lại bản viết

## 1. Mục tiêu

Biến video quay bằng HP Screen thành video hướng dẫn dùng được ngay, mà không cần phần mềm dựng khác. Bảy tính năng, theo thứ tự ưu tiên của người dùng:

1. Hiệu ứng click và hiện phím tắt.
2. Cắt nhanh sau khi quay.
3. Vẽ lên màn hình trong lúc quay.
4. Webcam ở góc video.
5. Tự phóng to theo click (auto-zoom) khi xuất.
6. Phụ đề tiếng Việt tự động, chạy offline.
7. Xuất GIF; đổi tỉ lệ 9:16 / 1:1 có nền và padding.

### Tiêu chí thành công

- Từ Thư viện mở được một video vào trang Chỉnh sửa, xem trước, và xuất ra file mới. File gốc không bao giờ bị sửa hay xóa.
- Bản xuất có vòng click đúng chỗ và đúng lúc (lệch không quá 1 khung hình so với nhật ký), có chip phím tắt.
- Cắt đầu, đuôi và đoạn giữa cho ra video dài đúng bằng phần giữ lại (sai số không quá 1 khung hình). Hình và tiếng vẫn khớp.
- Nét vẽ khi quay và hình webcam có trong video ở cả ba chế độ quay: toàn màn hình, cửa sổ, vùng.
- Auto-zoom tạo các đoạn zoom từ click; người dùng sửa, xóa, thêm được từng đoạn.
- Phụ đề tiếng Việt được tạo hoàn toàn trên máy, sửa tay được, xuất được SRT và ghi cứng vào video.
- Xuất được GIF và video tỉ lệ 9:16, 1:1.

## 2. Quyết định đã chốt

| Hạng mục | Quyết định |
|---|---|
| Hiệu ứng, cắt, zoom, phụ đề, tỉ lệ, GIF | **Hậu kỳ không phá hủy.** Chỉnh sửa lưu ở `.hpscreen/<tên>.edit.json`; chỉ áp khi xuất ra file mới |
| Nét vẽ và webcam | **Ghi thẳng vào video khi quay.** Pipeline vẽ lên từng khung hình trước khi mã hóa, nên chạy được ở chế độ quay cửa sổ. Không gỡ được sau khi quay |
| Bộ dựng hình | Direct2D + DirectWrite (có sẵn trong Windows, gọi qua crate `windows`). Dùng chung cho xuất video, xuất GIF và lớp vẽ khi quay |
| Giải mã | Media Foundation Source Reader. Không đóng gói FFmpeg |
| Mã hóa | `Mp4Encoder` hiện có (H.264 + AAC); GIF dùng crate `gif` (MIT/Apache) |
| Cắt | Luôn mã hóa lại qua cùng đường xuất, cắt đúng từng khung hình |
| Xem trước | Thẻ `<video>` trong WebView, thêm lớp canvas, zoom bằng transform CSS. Cả hai đọc chung một "kế hoạch khung hình" do Rust tính sẵn. Bản xuất mới là chuẩn |
| Giao diện chỉnh sửa | Trang mới "Chỉnh sửa" trong cửa sổ chính, mở bằng nút "Chỉnh sửa" trên thẻ video ở Thư viện |
| Nhận giọng nói | whisper.cpp (MIT) chạy thành chương trình riêng; chương trình và model chỉ tải về khi bật lần đầu, có kiểm SHA-256. Model chốt sau phép đo ở mục 4.7 |
| Tên file xuất | `<tên gốc>_edit.mp4`, `.gif`, `.srt`; nếu trùng thì thêm `_2`, `_3` theo `naming::unique_path` |
| Giấy phép | Chỉ MIT / Apache / BSD. Cap và Recordly chỉ để tham khảo ý tưởng |

## 3. Kiến trúc

### 3.1 Hai loại hiệu ứng

```
Khi quay:  WGC ─▶ pipeline ─▶ [lớp trực tiếp: nét vẽ + webcam] ─▶ Mp4Encoder ─▶ <tên>.mp4
                       └─▶ input_hook ─▶ <tên>.input.jsonl

Khi xuất:  <tên>.mp4 ─▶ decoder ─▶ compositor (Direct2D) ─▶ Mp4Encoder / GIF ─▶ <tên>_edit.*
              ▲                       ▲
  <tên>.edit.json + <tên>.input.jsonl ─┴─▶ hp-core: kế hoạch khung hình
```

### 3.2 Hai dòng thời gian

- **Thời gian nguồn** là ms trong video gốc. Nhật ký thao tác, đoạn zoom và phụ đề đều lưu theo thời gian nguồn.
- **Thời gian xuất** là ms trong file xuất, sau khi đã bỏ các đoạn cắt.
- Module `cut` quy đổi hai chiều giữa hai loại thời gian. Khung xuất thứ n ứng với thời gian xuất n/fps. Bộ giải mã lấy khung nguồn có mốc thời gian gần nhất mà không vượt quá thời điểm nguồn tương ứng.

### 3.3 Module mới trong `crates/hp-core` (logic thuần, viết kiểm thử trước)

| Module | Nhiệm vụ |
|---|---|
| `input_log` (bổ sung) | Đọc lại file `.input.jsonl`. Dòng hỏng hoặc phiên bản lạ thì bỏ qua dòng đó, không làm hỏng cả file |
| `edit` | Mô hình `EditDoc` (mục 3.6): giá trị mặc định, kiểm hợp lệ, đọc/ghi JSON, số phiên bản |
| `cut` | Từ khoảng cắt đầu/đuôi và các đoạn bỏ, tính các đoạn giữ lại và tổng thời lượng; quy đổi thời gian xuất ↔ nguồn |
| `effects` | Từ nhật ký sinh ra danh sách vòng click, quầng khi giữ nút, biểu tượng cuộn chuột, chip phím; trả trạng thái các hiệu ứng tại thời điểm t |
| `spring` | Lò xo giảm chấn, tích phân bước cố định 1 ms nên kết quả không phụ thuộc fps |
| `zoom` | Gom click thành đoạn zoom; tính khung camera (tâm, mức zoom) tại thời điểm t; giữ khung camera nằm trong video |
| `layout` | Kích thước khung xuất theo tỉ lệ; vị trí video trong khung (padding, bo góc); khung cắt cho chế độ "Lấp đầy"; vị trí webcam |
| `subtitle` | Chia đoạn nhận dạng thành dòng phụ đề, quy đổi qua `cut`, ghi SRT |
| `stroke` | Hình học nét vẽ (đầu mũi tên, khung) và độ mờ theo thời gian |
| `plan` | Gộp các module trên thành kế hoạch khung hình: tại thời điểm xuất t thì lấy khung nguồn nào, camera ở đâu, vẽ gì |
| `library` (bổ sung) | Thêm `.edit.json` và `.preview.mp4` (mục 5) vào danh sách file đi kèm, để đổi tên hoặc xóa video thì file này cũng đi theo |
| `settings` (bổ sung) | Thêm trường mới, đều có giá trị mặc định (mục 4.3, 4.4) |

### 3.4 Module mới trong `src-tauri` (gọi Windows, giữ mỏng)

| Module | Nhiệm vụ |
|---|---|
| `decoder.rs` | Source Reader: khung BGRA top-down, PCM float 48 kHz stereo, tua tới một thời điểm |
| `compositor.rs` | Direct2D/DirectWrite: vẽ một khung theo kế hoạch (nền, video đã zoom/cắt, bo góc, bóng đổ, hiệu ứng, chip, phụ đề, webcam, nét vẽ) và đọc ngược ra BGRA |
| `export.rs` | Mỗi lần chỉ một việc xuất, chạy trên luồng riêng; báo tiến độ, hủy được, ghi thumbnail và meta cho file xuất |
| `gif_out.rs` | Giảm số màu và ghi GIF |
| `live_layer.rs` | Giữ danh sách nét vẽ và khung webcam mới nhất; vẽ chúng lên khung hình trong `pipeline.rs` |
| `camera.rs` | Liệt kê camera; mở camera bằng Media Foundation; giữ khung mới nhất |
| `asr.rs` | Tải whisper.cpp và model; tách tiếng ra WAV 16 kHz; chạy nhận dạng và đọc kết quả |
| `edit_cmds.rs` | Các lệnh Tauri của phần chỉnh sửa. Đặt trong file riêng; `lib.rs` chỉ thêm dòng đăng ký |

Phía giao diện:

- File mới: `src/pages/EditPage.tsx`, `src/editor/*`, `src/editApi.ts`, `src/windows/DrawOverlay.tsx`, `src/windows/CamBubble.tsx`.
- File dùng chung `ControlBar.tsx`, `LibraryPage.tsx`, `App.tsx`, `main.tsx`, `theme.css`, `api.ts` chỉ được thêm vào, không sắp xếp lại.
- Không đụng `SnipOverlay.tsx` và `snip.rs`.

### 3.5 Kế hoạch khung hình

- Lệnh `edit_plan(video, doc)` trả về cho giao diện:
  - kích thước khung xuất và vị trí video trong khung;
  - khung camera lấy mẫu 60 lần mỗi giây, theo thời gian nguồn;
  - danh sách hiệu ứng có thời điểm bắt đầu và kết thúc;
  - danh sách dòng phụ đề.
- Phần xem trước chỉ vẽ lại những dữ liệu này, không tự tính zoom hay gom click.
- Khi xuất, `export.rs` gọi trực tiếp `hp_core::plan` cho từng khung.

### 3.6 File chỉnh sửa `.hpscreen/<tên>.edit.json`

```json
{
  "v": 1,
  "trim": { "start_ms": 0, "end_ms": null },
  "cuts": [ { "start_ms": 12000, "end_ms": 15500 } ],
  "effects": { "clicks": true, "keys": true },
  "zoom": { "enabled": false, "segments": [
    { "start_ms": 3000, "end_ms": 7200, "scale": 1.5, "cx": 0.42, "cy": 0.31, "manual": false } ] },
  "frame": { "aspect": "source", "fit": "fit", "background": "slate", "padding": 0.0, "radius": 0, "shadow": false },
  "subtitles": { "cues": [ { "start_ms": 800, "end_ms": 3100, "text": "Bấm nút Xuất" } ], "burn": false }
}
```

- Thiếu trường thì lấy giá trị mặc định.
- Gặp phiên bản lớn hơn mức app hiểu thì không mở và không ghi đè; báo "File chỉnh sửa do bản HP Screen mới hơn tạo".
- Lưu tự động 1 giây sau thay đổi cuối. Ghi ra file tạm rồi đổi tên, để mất điện giữa chừng không làm hỏng file.

## 4. Hành vi chi tiết

### 4.1 Trang Chỉnh sửa và việc xuất

**Mở trang**
- Nút "Chỉnh sửa" chỉ có trên thẻ video. Trang thay nội dung cửa sổ chính, có nút "← Thư viện".
- Video không có nhật ký thao tác (quay trước khi có nhật ký, không do HP Screen quay, hoặc là bản đã xuất) vẫn cắt, đổi khung và làm phụ đề được. Thẻ Hiệu ứng và Zoom khi đó báo "Video này không có nhật ký thao tác".

**Bố cục**
- Giữa: khung xem trước.
- Dưới: nút phát/dừng, thời gian, dòng thời gian gồm các rãnh Video (tay cắt đầu/đuôi, đoạn bỏ tô sọc), Zoom, Phụ đề.
- Bên phải: các thẻ Cắt / Hiệu ứng / Zoom / Khung hình / Phụ đề / Xuất.

**Phím tắt trong trang**
- Space: phát/dừng.
- ←/→: lùi/tiến 1 khung hình; thêm Shift: 1 giây.
- I / O: đặt điểm đầu / cuối vùng chọn.
- Delete: bỏ vùng chọn hoặc xóa đoạn zoom đang chọn.
- Ctrl+Z / Ctrl+Y: hoàn tác / làm lại trong phiên chỉnh sửa.

**Xuất**
- Thẻ Xuất có: định dạng (MP4 / GIF), ô "Ghi cứng phụ đề", nút "Xuất SRT riêng", và nút Xuất.
- Trong lúc xuất: thanh tiến độ và nút Hủy. Hủy thì xóa file đang ghi dở.
- Xuất xong: báo "Đã lưu <tên file>", có nút Phát và Thư mục; thẻ mới hiện trong Thư viện.
- Đang quay thì không cho bắt đầu xuất. Nếu bắt đầu quay khi đang xuất thì việc xuất vẫn chạy tiếp, có thể chậm hơn.
- File xuất giữ fps của video gốc; bitrate tính bằng `quality` theo kích thước khung xuất; tiếng giải mã rồi mã hóa lại AAC.

### 4.2 Cắt nhanh (tính năng 2)

- Kéo tay cắt ở hai đầu rãnh Video để bỏ phần đầu và phần đuôi.
- Bỏ đoạn giữa: kéo chọn trên rãnh hoặc dùng I/O, rồi bấm "Bỏ đoạn này". Bỏ được nhiều đoạn. Bấm vào một đoạn đã bỏ thì có nút "Giữ lại".
- Khi xem trước, đầu phát tự nhảy qua các đoạn bỏ.
- Ở chỗ nối giữa hai đoạn giữ lại, tiếng được làm nhỏ dần rồi to dần trong 10 ms để không có tiếng "bụp".
- Phần giữ lại ngắn hơn 0,5 giây thì nút Xuất bị khóa.

### 4.3 Hiệu ứng click và chip phím (tính năng 1)

**Vòng click**
- Mỗi lần nhấn nút chuột vẽ một vòng gợn lan ra trong 480 ms. Bán kính cuối bằng 2,5% chiều cao video gốc. Đường viền dày, mờ dần khi lan ra.
- Màu: trái đỏ cam `#FF5A36`, phải xanh dương `#3B82F6`, giữa xanh lá `#22C55E`.
- Vòng vẽ theo tọa độ video gốc, nên phóng to cùng video khi zoom.

**Giữ nút và kéo**
- Từ lúc nhấn đến lúc nhả, một chấm quầng cùng màu bám theo con trỏ. Vị trí con trỏ lấy bằng nội suy giữa các sự kiện di chuyển.

**Cuộn chuột**
- Cạnh con trỏ hiện biểu tượng con chuột nhỏ với mũi tên lên hoặc xuống.
- Biểu tượng giữ 400 ms sau lần cuộn cuối rồi mờ đi.

**Chip phím**
- Chỉ hiện các phím đã được ghi vào nhật ký, tức tổ hợp phím tắt theo chính sách riêng tư hiện có. Chữ gõ thường không bao giờ hiện.
- Dạng chip bo góc như "Ctrl + Shift + S", đặt giữa, cách mép dưới khung xuất 8%.
- Hiện 1,5 giây, hiện ra và mờ đi mỗi bên 150 ms.
- Bấm lại đúng tổ hợp đó trong lúc chip còn hiện thì chip thêm "×2", "×3". Tổ hợp khác thì thay chip cũ.
- Chip vẽ theo tọa độ khung xuất, không phóng theo zoom. Nếu ô "Ghi cứng phụ đề" đang bật thì chip dời lên trên dòng phụ đề.

**Bật/tắt:** thẻ Hiệu ứng có hai công tắc "Vòng click" và "Phím tắt", mặc định bật cả hai.

### 4.4 Vẽ lên màn hình khi quay (tính năng 3)

**Bật/tắt**
- Nút bút trên thanh gọn, hoặc phím tắt `Ctrl+Alt+D` (đổi được trong Cài đặt; trường mới `hotkey_draw`).
- Chỉ dùng được khi đang quay hoặc tạm dừng.

**Khi bật**
- Một cửa sổ trong suốt (`draw`) phủ đúng vùng đang quay và nhận chuột.
- Có thanh công cụ nhỏ: Bút, Mũi tên, Khung; 5 màu (đỏ, vàng, xanh lá, xanh dương, trắng); 3 độ dày; Xóa hết; Thoát.
- Esc hoặc bấm phím tắt lần nữa thì tắt chế độ vẽ. Cửa sổ chuyển sang để chuột đi xuyên qua, nhưng vẫn hiện các nét đang mờ dần.

**Tự mờ**
- Mỗi nét giữ nguyên 3 giây sau khi vẽ xong rồi mờ hết trong 0,5 giây.
- Thời gian giữ chỉnh được trong Cài đặt: 2 / 3 / 5 / 10 giây hoặc "Không tự mờ" (trường mới `draw_fade_secs`, 0 nghĩa là không mờ).
- Thời gian tính theo đồng hồ thật, đúng như người quay nhìn thấy.

**Đưa vào video**
- Giao diện gửi từng nét (mã nét, công cụ, màu, độ dày, danh sách điểm theo pixel vật lý tương đối gốc vùng quay) xuống `live_layer`, nhiều nhất 60 lần mỗi giây.
- `pipeline.rs` vẽ các nét còn hiện lên khung hình ngay trước `write_video`, kể cả nét đang vẽ dở.
- Cửa sổ `draw` bị loại khỏi capture, nên nét không bị in hai lần.

**Hiệu năng**
- Chỉ vùng chữ nhật bao quanh các nét đang hiện mới được đưa lên Direct2D, vẽ, rồi đọc ngược về.
- Khi không có nét nào thì không tốn thêm gì.

**Giới hạn đã biết:** ở chế độ quay cửa sổ, nếu cửa sổ bị kéo đi chỗ khác thì cửa sổ vẽ không đi theo. Nét trong video vẫn bám vào nội dung cửa sổ.

### 4.5 Webcam (tính năng 4)

**Bật/tắt**
- Nút camera trên thanh gọn, và công tắc "Webcam" ở trang Quay trước khi bắt đầu.
- Camera chỉ mở khi bật; đóng khi tắt hoặc khi quay xong.

**Cài đặt** (trường mới `webcam`)
- Camera nào (danh sách lấy từ Media Foundation; mặc định là camera đầu tiên).
- Hình dạng: tròn hoặc bo góc.
- Cỡ: nhỏ / vừa / lớn, bằng 15 / 20 / 25% chiều cao video.
- Góc: một trong bốn góc, mặc định góc dưới phải; cách mép 2,5%.
- Lật gương: mặc định bật.

**Vẽ vào video**
- Lấy khung camera ở độ phân giải gần nhất không quá 720p.
- Cắt giữa ảnh thành hình vuông (dạng tròn) hoặc tỉ lệ 4:3 (dạng bo góc), viền trắng mảnh.
- `live_layer` vẽ khung camera mới nhất lên mỗi khung hình quay, cùng chỗ với nét vẽ.

**Bong bóng trên màn hình**
- Cửa sổ `cam`, bị loại khỏi capture, đặt ở cùng góc của vùng quay.
- Nhận ảnh nhỏ (khoảng 240 px, 10 khung/giây) để người quay tự thấy mình.
- Không kéo đi được; muốn đổi góc thì vào Cài đặt.

**Lỗi**
- Camera đang bị app khác dùng, không tìm thấy camera, hoặc Windows chặn quyền camera cho ứng dụng desktop: báo trên thanh gọn bằng tiếng Việt, kèm cách sửa.
- Việc quay vẫn tiếp tục, chỉ không có webcam.

### 4.6 Auto-zoom (tính năng 5)

**Tạo đoạn tự động.** Thông số mặc định theo PR #1663 của Cap, ghi trong báo cáo:
- Gom các click cách nhau dưới 2,5 giây và cách nhau dưới 0,15 chiều rộng video vào một nhóm.
- Đoạn zoom bắt đầu 0,4 giây trước click đầu và kéo dài đến 1,8 giây sau click cuối.
- Gộp hai đoạn nếu khoảng hở giữa chúng dưới 0,8 giây; bỏ đoạn ngắn hơn 1 giây.
- Mức zoom mặc định 1,5×.
- Tâm zoom là trọng tâm các click trong nhóm.

**Chuyển động**
- Mức zoom và tâm khung camera đi theo lò xo quá giảm chấn (tension 300, mass 6,75, friction 120), nên không bị nảy.
- Trong một đoạn, khung chỉ trượt theo con trỏ khi con trỏ ra khỏi 70% vùng giữa khung. Nếu trong 1 giây sau đó con trỏ quay lại vùng giữa thì khung đứng yên.
- Khung camera luôn nằm trong video.

**Người dùng sửa**
- Rãnh Zoom: kéo hai mép để đổi thời gian; kéo cả đoạn để dời; Delete để xóa; kéo trên chỗ trống để thêm đoạn.
- Đoạn đang chọn có thanh trượt mức zoom 1–4×. Ở mức trên 2× hiện cảnh báo "video có thể bị mờ".
- Bấm lên khung xem trước để đặt tâm cố định.
- Đoạn người dùng thêm hoặc đã sửa được đánh dấu `manual`. Nút "Tự tạo lại" chỉ thay các đoạn tự động, giữ nguyên đoạn `manual`.

**Mặc định:** mới mở thì zoom tắt. Bật công tắc lần đầu thì app tự tạo đoạn.

**Quan hệ với các tính năng khác:** zoom áp lên nội dung video bên trong khung. Vòng click phóng theo zoom; chip phím và phụ đề thì không.

### 4.7 Phụ đề tiếng Việt (tính năng 6)

**Phép đo trước khi tích hợp**
- Hai ứng viên: Whisper large-v3-turbo (MIT) và PhoWhisper-medium (BSD-3), cả hai chạy qua whisper.cpp.
- Đo trên 2–3 video hướng dẫn tiếng Việt thật của người dùng, ngay trên máy người dùng:
  - thời gian nhận dạng so với độ dài video, trên CPU;
  - số lỗi trên một đoạn mẫu có chép tay;
  - cách xử lý từ tiếng Anh;
  - dấu câu.
- Ghi kết quả và model được chọn vào sổ ghi quyết định.
- Danh sách model là dữ liệu (tên, URL, kích thước, SHA-256), nên đổi model mặc định không phải sửa logic.
- Nếu PhoWhisper không có bản ggml tải công khai với giấy phép rõ ràng thì loại.

**Lần đầu bật**
- Hộp thoại ghi rõ dung lượng tải, nguồn tải (GitHub release của whisper.cpp, Hugging Face), nơi lưu (`%LOCALAPPDATA%\HP Screen\asr\`), và "âm thanh không rời khỏi máy".
- Chỉ tải khi người dùng đồng ý.
- Tải xong kiểm SHA-256. Sai mã thì xóa file và báo lỗi.

**Tạo phụ đề**
- Tách tiếng của cả video gốc ra WAV 16 kHz mono vào thư mục tạm.
- Chạy `whisper-cli` với ngôn ngữ `vi`, xuất JSON có mốc thời gian, theo dõi phần trăm tiến độ. Hủy thì dừng tiến trình.
- Xong thì xóa file WAV tạm.

**Chia dòng (`subtitle`)**
- Mỗi dòng phụ đề tối đa 2 dòng × 42 ký tự, dài 1–6 giây.
- Ưu tiên ngắt ở dấu câu, sau đó ở khoảng trắng.
- Có mốc thời gian từng từ thì dùng; không có thì chia thời gian theo số ký tự.

**Sửa tay**
- Thẻ Phụ đề có danh sách các dòng: thời gian, ô chữ sửa được, các nút Gộp với dòng sau / Tách tại con trỏ / Xóa.
- Bấm một dòng thì đầu phát nhảy tới đó; dòng đang phát được tô sáng; phụ đề hiện trên khung xem trước.

**Xuất**
- SRT tính theo thời gian xuất: dòng nằm hẳn trong đoạn bỏ thì bị loại, dòng nằm vắt qua thì bị cắt ngắn.
- Ghi cứng: chữ trắng Segoe UI Semibold cao bằng 4,5% chiều cao khung xuất, nền đen mờ bo góc, rộng tối đa 80% khung, cách mép dưới 6%.
- Bỏ chọn "Ghi cứng phụ đề" thì file xuất không có chữ.

### 4.8 Khung hình và GIF (tính năng 7)

**Tỉ lệ:** Gốc / 16:9 / 9:16 / 1:1. Chiều cao khung xuất bằng chiều cao video gốc (làm chẵn); chiều rộng tính theo tỉ lệ.

**Cách đặt video** (khi tỉ lệ khác gốc)
- **Vừa khung:** cả video nằm trong khung, chỗ trống là nền.
- **Lấp đầy:** cắt video cho đúng tỉ lệ. Tâm khung cắt đi theo tâm camera của zoom; nếu không có đoạn zoom nào thì đi theo con trỏ, làm mượt bằng cùng lò xo.

**Trang trí**
- Nền: 6 màu và dải màu có sẵn.
- Padding 0–20% cạnh ngắn của khung.
- Bo góc video 0–40 px.
- Bóng đổ bật/tắt.
- Chỉ có tác dụng ở chế độ Vừa khung, hoặc khi padding lớn hơn 0.

**GIF**
- Chọn 10 hoặc 15 khung/giây (mặc định 15); chiều rộng 480 / 720 / 960 px (mặc định 720); lặp vô hạn; không có tiếng.
- Mỗi khung GIF có bảng màu riêng.
- Phần giữ lại dài hơn 60 giây thì cảnh báo "GIF sẽ rất nặng", nhưng vẫn cho xuất.

## 5. Xử lý lỗi

| Tình huống | Hành vi |
|---|---|
| Không giải mã được video (file hỏng, codec lạ) | Trang Chỉnh sửa báo "Không đọc được video này"; không tạo file chỉnh sửa |
| WebView không phát được MP4 phân mảnh | Lần đầu mở, sao chép luồng (không mã hóa lại) ra `.hpscreen/<tên>.preview.mp4` dạng MP4 thường và xem trước bằng file đó; xóa file này cùng video |
| Nhật ký thao tác thiếu, hỏng một phần, hoặc lệch kích thước | Bỏ qua dòng hỏng; nếu kích thước ghi trong header khác kích thước video thì quy đổi tỉ lệ tọa độ |
| Hết chỗ trống trên đĩa khi xuất | Dừng xuất, xóa file dở, báo lỗi |
| Đang xuất thì đóng cửa sổ chính | Cửa sổ thu về khay như cũ, việc xuất chạy tiếp; thoát app thì hủy xuất và xóa file dở |
| Phím tắt vẽ bị app khác chiếm | Báo xung đột như các phím tắt khác; nút trên thanh gọn vẫn dùng được |
| Lớp vẽ khi quay quá chậm (vẽ một khung mất trên 8 ms) | Ghi một dòng vào `hp-screen.log`; khung vẫn được ghi, không bỏ khung |
| Camera mất kết nối giữa lúc quay | Giữ khung camera cuối 1 giây, sau đó ẩn webcam khỏi video và báo trên thanh gọn |
| Tải model bị ngắt, hoặc sai SHA-256 | Xóa file dở, báo lỗi, có nút thử lại |
| `whisper-cli` thoát với mã lỗi | Hiện 20 dòng cuối của stderr trong log, báo "Không tạo được phụ đề" |

## 6. Kiểm thử

**Tự động, viết test trước (`cargo test -p hp-core`)**
- `input_log` (đọc), `edit`, `cut`, `effects`, `spring`, `zoom`, `layout`, `subtitle`, `stroke`, `plan`.
- Bổ sung cho `library` và `settings`.
- Các trường hợp biên phải có:
  - đoạn cắt chồng nhau hoặc nằm ngoài video;
  - cắt hết video;
  - click ngay mép video;
  - zoom ở mức 1× và 4×;
  - phụ đề vắt qua đoạn cắt;
  - nét vẽ chỉ có một điểm;
  - file chỉnh sửa có phiên bản lạ.

**Tự động (`cargo test -p hp-screen --lib`)**
- `compositor` vẽ lên thiết bị WARP (không cần GPU thật), kiểm màu từng pixel.
- `gif_out` ghi rồi đọc lại.
- Đọc ghi file chỉnh sửa trên đĩa.

**Desktop, nằm trong nhóm `--ignored`** (chỉ chạy khi người dùng không dùng máy, hoặc hỏi trước)
- Giải mã một bản quay thật.
- Xuất bản cắt rồi đếm khung và đo thời lượng bằng Source Reader.
- Xuất GIF.
- Pipeline có nét vẽ: pixel ở vị trí nét trong file ra có đúng màu nét.
- Mở camera (cần có camera).
- Nhận dạng một đoạn WAV mẫu (cần model đã tải).

**`pnpm tsc --noEmit`** phải sạch.

**Người dùng thử bằng tay, theo danh sách gửi kèm mỗi bước:**
- xem trước có khớp bản xuất không;
- kéo tay cắt;
- vẽ khi quay ở cả 3 chế độ;
- webcam;
- sửa đoạn zoom;
- phụ đề trên video có lẫn tiếng Anh;
- GIF mở được trong trình duyệt.

Không giả lập chuột hay phím trên máy người dùng trong bất kỳ kiểm thử nào.

## 7. Thứ tự giai đoạn

Mỗi bước xong thì commit trên nhánh phiên này và gửi người dùng danh sách cần thử.

| Bước | Nội dung | Ghi chú |
|---|---|---|
| 1 | Nền xuất (`decoder`, `compositor`, `export`, `edit`, `cut`, `plan`), trang Chỉnh sửa, **cắt nhanh** (TN 2) | Kiểm ngay WebView có phát được MP4 phân mảnh không |
| 2 | **Hiệu ứng click và chip phím** (TN 1) | `input_log` đọc, `effects` |
| 3 | **Vẽ khi quay** (TN 3) | `live_layer`, `stroke`, cửa sổ `draw`, đo thời gian vẽ mỗi khung |
| 4 | **Webcam** (TN 4) | `camera`, cửa sổ `cam`, dùng lại `live_layer` |
| 5 | **Auto-zoom** (TN 5) | `spring`, `zoom` |
| 6 | **Tỉ lệ, nền, padding, GIF** (TN 7) | `layout`, `gif_out`; đưa lên trước phụ đề vì rẻ khi đã có bộ dựng hình |
| 7 | **Phụ đề** (TN 6) | Đo model với video của người dùng trước, rồi mới tích hợp `asr`, `subtitle` |

## 8. Ngoài phạm vi đợt này

- Ẩn con trỏ hệ thống rồi vẽ lại con trỏ cho mượt.
- Cắt không mã hóa lại.
- Track phụ đề mềm trong MP4.
- Làm mờ vùng nhạy cảm; quầng sáng thường trực quanh con trỏ.
- Phụ đề hiện trực tiếp khi đang nói; dịch phụ đề.
- Kéo bong bóng webcam đi chỗ khác khi đang quay; webcam ghi thành file riêng.
- Nút "Xuất nhanh" ngay trên Thư viện (không mở trang Chỉnh sửa).
- Chạy nhận dạng giọng trên GPU (chỉ ghi nhận số đo nếu phép đo ở bước 7 có làm).

## 9. Rủi ro và điểm chưa kiểm chứng

| Điểm | Cách xử lý |
|---|---|
| WebView2 có phát và tua được MP4 phân mảnh không | Kiểm ở bước 1; nếu không được thì dùng file xem trước ở mục 5 |
| Đưa khung lên Direct2D rồi đọc ngược về có đủ nhanh khi đang quay ở 2560×1600, 60 fps không | Chỉ xử lý vùng bao quanh nét và webcam; đo ở bước 3, ghi số đo vào sổ ghi |
| Camera đã bị app khác giữ | Báo lỗi rõ, việc quay vẫn tiếp tục |
| Source Reader tua có chính xác tới từng khung không | Tua tới keyframe gần nhất phía trước rồi đọc tới đúng khung cần; kiểm bằng test đếm khung |
| whisper.cpp trên CPU phổ thông có đủ nhanh không | Đo ở bước 7; nếu chậm hơn 1× thời lượng video thì hiện ước lượng thời gian trước khi chạy |
| Giấy phép và nguồn tải của bản ggml PhoWhisper | Kiểm ở bước 7; không rõ thì loại |
| Thông số auto-zoom lấy từ một PR chưa được merge | Đặt hết thành hằng số có tên trong `zoom`, dễ chỉnh sau khi người dùng xem |
