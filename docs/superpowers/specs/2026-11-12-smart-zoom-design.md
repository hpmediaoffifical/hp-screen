# HP Screen — Zoom thông minh (giai đoạn 2, bước 5b)

Ngày: 2026-11-12 · Trạng thái: hai phần thiết kế đã duyệt qua trao đổi, chờ duyệt bản viết

## 1. Mục tiêu

Người dùng quyết định được đoạn nào zoom, đoạn nào không, và zoom vào đâu bằng cách kéo một khung trên video. Zoom tự động cho ra các đoạn hợp lý hơn, để người dùng ít phải sửa. Người dùng dựng tiếp bằng CapCut, nên mọi thứ phải đơn giản và làm bằng tay được.

### Tiêu chí thành công

- Chọn một đoạn zoom: xem trước hiện toàn bộ hình, có khung zoom để kéo dời và kéo góc. Bản xuất zoom đúng vào vùng trong khung (sai số không quá 1% kích thước khung hình).
- Trong một đoạn có ít nhất hai điểm ◆, camera lia lần lượt qua từng điểm. Tại thời điểm của mỗi điểm, camera ở đúng khung của điểm đó (sai số không quá 2%, khi tốc độ không phải "Chậm").
- Chọn được tốc độ Chậm / Vừa / Nhanh / Tức thì cho từng đoạn.
- Phím Z thêm một đoạn tại vị trí đang phát; phím K thêm một điểm ◆.
- Tự động có ba mức Ít / Vừa / Nhiều. Ở mức Vừa, trên cùng một nhật ký, số đoạn bằng hoặc ít hơn bản hiện tại, và không có đoạn nào bắt đầu từ click sát mép màn hình.
- File chỉnh sửa cũ (chưa có điểm ◆ và tốc độ) vẫn mở được và cho ra cùng kết quả như trước.
- Xem trước và bản xuất dùng chung một đường tính camera (`zoom_track`), như hiện nay.

## 2. Mô hình dữ liệu (`hp-core::zoom`)

`ZoomSegment` có thêm hai trường, cả hai đều có giá trị mặc định khi đọc file cũ:

| Trường | Kiểu | Ý nghĩa |
|---|---|---|
| `keys` | `Vec<ZoomKey>` | Các điểm ◆ sau đầu đoạn. Mặc định là danh sách rỗng |
| `speed` | `"slow" \| "normal" \| "fast" \| "instant"` | Tốc độ vào, ra và lia. Mặc định `"normal"` |

`ZoomKey { t_ms: i64, cx: f32, cy: f32, scale: f32 }`:

- `t_ms` là thời gian nguồn, nằm hẳn trong đoạn (`start_ms < t_ms < end_ms`).
- `cx`, `cy` là tâm (0–1 của khung).
- `scale` nằm trong khoảng 1–4.

**Khung chính** là `center` và `scale` sẵn có của đoạn: đó là điểm ở đầu đoạn.

- Đoạn có điểm ◆ thì luôn có tâm cố định. Nếu `center` là `None` (đang đi theo con trỏ) mà `keys` không rỗng, `normalized` đặt `center` bằng tâm của điểm đầu tiên.
- Thêm điểm ◆ vào một đoạn đang đi theo con trỏ thì giao diện chuyển đoạn đó sang tâm cố định, tại vị trí camera lúc đó.

`normalized` thêm các việc sau:

- Sắp `keys` theo thời gian.
- Bỏ điểm nằm ngoài khoảng (start, end) và điểm cách điểm trước không quá 50 ms.
- Giới hạn tâm trong 0–1 và mức phóng trong 1–4.
- Khi một đoạn bị tách, vì đoạn sau đè lên, mỗi mảnh chỉ giữ những điểm nằm trong nó. Mảnh sau bắt đầu từ khung mà camera nhắm tới tại điểm cắt.

### Đích của camera trong một đoạn

1. Không có điểm ◆: giữ nguyên như hiện nay, tức tâm cố định hoặc đi theo con trỏ.
2. Có điểm ◆, tức chuỗi P0 (khung chính, tại `start_ms`), P1, …, Pn:
   - Giữa Pi và Pi+1, đích là nội suy từ Pi sang Pi+1, dùng đường cong `smoothstep`. Điều này áp dụng cho cả tâm và mức phóng; mức phóng nội suy theo log để tốc độ phóng đều mắt.
   - Sau Pn, đích giữ nguyên ở Pn cho tới hết đoạn.
3. Ngoài mọi đoạn: toàn khung (1×, tâm 0,5; 0,5), như hiện nay.

### Tốc độ

Camera vẫn đi theo đích bằng lò xo quá giảm chấn sẵn có (tension 300, mass 6,75, friction 120). Tốc độ co giãn thời gian của lò xo theo hệ số f: tension × f², friction × f, mass giữ nguyên. Như vậy độ nảy (bằng 0) không đổi.

| Tốc độ | f | Cảm nhận |
|---|---|---|
| Chậm | 0,6 | Vào zoom mất khoảng gấp 1,7 lần mức Vừa |
| Vừa | 1 | Như hiện nay |
| Nhanh | 2 | Nhanh gấp đôi |
| Tức thì | — | Không có lò xo: camera nhảy thẳng tới đích, kể cả khi vào, ra và tại mỗi điểm ◆ (với Tức thì, đích giữa hai điểm là Pi, không nội suy) |

Dùng tốc độ của đoạn đang nhắm tới. Khi trở ra ngoài đoạn, dùng tốc độ của đoạn vừa kết thúc.

## 3. Giao diện zoom bằng tay

### Chế độ chỉnh khung

Bật khi đủ cả bốn điều kiện: thẻ Zoom đang mở, zoom đang bật, có một đoạn đang được chọn, và video đang dừng.

- Xem trước hiện **toàn bộ hình**, không zoom. Có thêm một lớp phủ: phần ngoài khung tối đi (đen 45%), khung có viền trắng 2 px, bốn tay nắm vuông ở bốn góc, và nhãn "1.8×" ở góc trên trái.
- Khung là vùng nguồn mà camera nhìn thấy tại vị trí đang phát. Nó được tính bằng `sourceView` sẵn có, nên đúng cả ở chế độ "Lấp đầy".
- **Kéo giữa khung** để dời tâm. Khung không ra ngoài hình.
- **Kéo một góc** để đổi kích thước, giữ nguyên tỉ lệ. Góc đối diện đứng yên khi có thể; nếu không thì giữ tâm. Mức phóng giới hạn trong 1–4×.
- Một lần kéo là một bước hoàn tác.
- Khung đang kéo là **điểm đang hoạt động**: điểm ◆ gần nhất ở trước hoặc đúng vị trí đang phát. Nếu không có điểm ◆ nào như vậy thì là khung chính.
- Kéo khung của một đoạn đang đi theo con trỏ thì đoạn đó chuyển sang tâm cố định.
- Bấm Phát thì lớp phủ ẩn và xem trước hiện đúng như bản xuất.
- Chọn một đoạn trên rãnh Zoom thì video tua tới đầu đoạn. Riêng khi vị trí đang phát đã nằm trong đoạn đó thì giữ nguyên.
- Nút "Chọn tâm trên video" bị bỏ, vì kéo khung đã thay nó.

### Điểm ◆

- Nút **"◆ Thêm điểm"** (phím **K**) thêm một điểm tại vị trí đang phát, với khung bằng camera lúc đó. Nút chỉ bật khi vị trí đang phát nằm trong đoạn đang chọn và cách các điểm có sẵn hơn 50 ms.
- Trên rãnh Zoom, điểm hiện là hình thoi bên trong khối của đoạn:
  - kéo hình thoi để đổi giờ (không ra khỏi đoạn);
  - bấm vào hình thoi để chọn điểm và tua tới nó;
  - **Delete** xóa điểm đang chọn; nếu không có điểm nào được chọn thì xóa đoạn, như hiện nay.
- Thẻ Zoom ghi "Đoạn có N điểm lia", và có nút "Xóa điểm này" khi đang chọn một điểm.

### Tốc độ và thêm nhanh

- Thẻ Zoom có hàng nút **Chậm / Vừa / Nhanh / Tức thì** cho đoạn đang chọn.
- **Z** hoặc nút **"+ Zoom tại đây"** thêm một đoạn bắt đầu tại vị trí đang phát:
  - dài 3 giây, ngắn lại nếu chạm đoạn sau hoặc chạm cuối video; không thêm nếu còn chưa tới 0,5 giây;
  - mức phóng 1,5×;
  - tâm ở vị trí con trỏ chuột lúc đó (có nhật ký), không thì ở giữa;
  - đánh dấu là đoạn làm tay, và được chọn ngay.
- Phím Z và K không chạy khi đang gõ trong một ô nhập, giống các phím tắt khác của trang.

## 4. Tự động khôn hơn

Vẫn là quy tắc chạy trên máy, nằm ở `hp-core::zoom::auto_segments`. Hàm này nhận các sự kiện của nhật ký thay vì chỉ click:

```
auto_segments(input: &AutoInput, level: Level, duration_ms) -> Vec<ZoomSegment>
AutoInput { clicks: [(ms, x, y)], wheels: [ms], typing: [ms], cursor: Fn(ms) -> Option<(x, y)>, edge_guard: bool }
```

Tọa độ tính theo 0–1 của khung. `edge_guard` bật khi nhật ký là quay toàn màn hình (`mode == "monitor"`).

### Mức độ

| Thông số | Ít | Vừa (mặc định) | Nhiều |
|---|---|---|---|
| Nhóm cần ít nhất | 2 click, hoặc 1 click mà trong 2 s sau đó có gõ phím hay cuộn | 1 click | 1 click |
| Giữ sau lần thao tác cuối | 1,8 s | 1,8 s | 1,2 s |
| Bỏ click sát mép (khi `edge_guard`) | có | có | không |

Mức độ lưu trong `ZoomDoc.level`, mặc định `"normal"`. Đổi mức thì tạo lại các đoạn tự động ngay, vẫn giữ các đoạn làm tay.

### Quy tắc

1. **Click sát mép:** một click cách mép khung không quá 4% (theo chiều tương ứng) thì không tạo và không nối dài đoạn nào. Đó thường là thanh tác vụ hoặc nút đóng, thu nhỏ cửa sổ.
2. **Gom nhóm:** như hiện nay. Click cách nhau dưới 2,5 s và dưới 0,15 thì vào cùng nhóm. Đoạn bắt đầu 0,4 s trước click đầu tiên.
3. **Mức phóng tự chọn:** gọi m là cạnh lớn hơn của hình chữ nhật bao quanh các click trong nhóm. Mức phóng là `clamp(min(2.0, 1 / (m + 0.2)), 1.25, 2.0)`, tức click đơn được 2×, và khung luôn chứa mọi click kèm lề 0,1 mỗi bên khi có thể.
4. **Lia theo click:** các nhóm cách nhau dưới 0,8 s được nối thành một đoạn, như hiện nay.
   - Nếu các nhóm có tâm cách nhau từ 0,15 trở lên, đoạn không đi theo con trỏ nữa.
   - Khung chính là tâm và mức phóng của nhóm đầu. Mỗi nhóm sau thêm một điểm ◆ tại 0,4 s trước click đầu của nhóm đó, với tâm và mức phóng của nhóm.
5. **Giữ khi gõ hoặc cuộn:** sau click cuối, mỗi lần gõ phím, hoặc mỗi lần cuộn mà con trỏ còn trong khung, đẩy cuối đoạn tới 1 s sau thao tác đó. Không quá 20 s sau click cuối.
6. **Trở ra sớm:** sau click cuối, nếu con trỏ ra ngoài khung zoom và ở ngoài liên tục 1 s mà không click, thì đoạn kết thúc tại lúc đó (thời điểm ra ngoài cộng 1 s). Đoạn không ngắn quá 1 s.
7. Đoạn ngắn hơn 1 s thì bỏ, như hiện nay.

### Dấu "đang gõ" trong nhật ký

- Bộ quay ghi thêm dòng `{"e":"type","t":<ms>}` khi có phím bị bấm mà không thuộc tổ hợp được ghi. Tối đa một dòng mỗi 300 ms.
- **Không ghi phím nào**, chỉ ghi thời điểm. Bộ lọc riêng tư hiện có không đổi.
- Không ghi khi đang ở chế độ vẽ, giống các sự kiện phím khác.
- Bản đọc nhật ký cũ bỏ qua dòng lạ, nên `LOG_VERSION` giữ là 1.
- Video quay trước bản này không có dấu đó: quy tắc 5 khi ấy chỉ dùng cuộn chuột.

### Đoạn tự động và đoạn làm tay

- Trên rãnh Zoom, đoạn tự động có màu nhạt hơn (cùng màu nhấn, độ đậm 55%). Đoạn làm tay có màu đầy đủ.
- Sửa bất kỳ thứ gì của một đoạn tự động (giờ, khung, điểm, tốc độ) thì nó thành đoạn làm tay, như hiện nay.
- Thẻ Zoom có nút "Xóa các đoạn tự động": một bước hoàn tác, giữ nguyên đoạn làm tay.

## 5. Lệnh và giao diện

- `zoom_auto(path, doc, frame)` dùng `doc.zoom.level` và các sự kiện cuộn, gõ trong nhật ký.
- `zoom_track` không đổi chữ ký. Kết quả đã có điểm ◆ và tốc độ.
- Các kiểu trong `editApi.ts` thêm `ZoomKey`, `ZoomSegment.keys`, `ZoomSegment.speed`, `ZoomDoc.level`.
- Phép tính khung trong giao diện nằm ở `src/editor/zoomMath.ts`, có test bằng `node --test`:
  - khung từ camera;
  - camera từ khung, khi dời và khi kéo góc, có giới hạn;
  - tìm điểm đang hoạt động;
  - thêm điểm.

## 6. Kiểm thử

**`hp-core`**, viết test trước:

- File cũ không có `keys`/`speed`/`level` đọc ra mặc định; camera như trước.
- Hai điểm ◆: tại thời điểm của mỗi điểm, camera cách khung của điểm đó không quá 2% (tốc độ Nhanh).
- Tức thì: camera bằng đích ngay sau khi vào đoạn và ngay sau mỗi điểm.
- Chậm vào zoom chậm hơn Vừa, Nhanh nhanh hơn Vừa (so thời gian tới 90% mức phóng).
- `normalized` sắp xếp, cắt và bỏ điểm sai; tách đoạn thì chia điểm đúng mảnh.
- Mức phóng tự chọn: click đơn được 2×; nhóm rộng 0,6 được 1,25×.
- Lia theo click: hai nhóm xa nhau thành một đoạn có một điểm ◆.
- Click sát mép bị bỏ khi `edge_guard`, và được giữ ở mức Nhiều.
- Mức Ít bỏ click lẻ nhưng giữ click có gõ phím ngay sau.
- Gõ phím kéo dài đoạn; quá 20 s thì dừng.
- Con trỏ ra ngoài 1 s thì trở ra sớm.
- Đọc dòng `type` của nhật ký; bản đọc bỏ qua dòng lạ.

**`hp-screen`:**

- Bộ quay ghi `type` cho phím thường, không ghi khi đang vẽ, tối đa một dòng mỗi 300 ms.
- Bản xuất của một đoạn có điểm ◆ zoom đúng vùng tại thời điểm của điểm đó (test xuất có sẵn với nguồn bốn góc màu).

**Thử tay:** thêm mục Z9–Z16 vào `docs/manual-test-checklist.md`.

## 7. Ngoài phạm vi

- Zoom có tâm đi theo con trỏ cộng với điểm ◆ trong cùng một đoạn.
- Đường lia cong, hoặc chỉnh đường cong cho từng điểm.
- Gợi ý zoom bằng AI hay nhận dạng nội dung màn hình.
- Phóng to vùng gõ chữ theo vị trí con trỏ nhập liệu: hệ thống không cho biết vị trí này một cách tin cậy.
