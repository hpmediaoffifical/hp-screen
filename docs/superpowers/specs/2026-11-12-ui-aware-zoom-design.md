# HP Screen — Zoom hiểu giao diện (bước 5c)

Ngày: 2026-11-12 · Trạng thái: người dùng đã duyệt gói đề xuất 1 + 2 + 3 ("Làm gói 1+2+3"). Người dùng dặn tự quyết và làm đến khi xong.

## Mục tiêu

Zoom tự động (bên trong các vùng tự động) dựa vào thông tin giao diện ghi lúc quay, thay vì chỉ dựa vào chỗ click:

1. **Zoom khớp phần tử được bấm.** Khung zoom ôm vừa nút, ô nhập, mục menu hay hộp thoại dưới chuột, kèm lề.
2. **Zoom vào ô đang gõ.** Khi gõ phím, khung zoom tới ô đang nhận chữ, kể cả khi chuột ở chỗ khác.
3. **Lùi ra khi đổi cửa sổ.** Khi một cửa sổ khác lên trước, camera trở về toàn cảnh rồi mới zoom vào chỗ mới.

Chỉ có tác dụng với video quay sau bản này. Video cũ vẫn chạy như trước.

## Nhật ký thao tác (thêm, `LOG_VERSION` vẫn là 1)

| Dòng | Ý nghĩa |
|---|---|
| `{"e":"el","t":ms,"k":"click","r":[x,y,w,h]}` | Khung phần tử dưới chuột lúc bấm chuột trái |
| `{"e":"el","t":ms,"k":"focus","r":[x,y,w,h]}` | Khung phần tử đang nhận phím lúc gõ (tối đa 1 lần mỗi giây) |
| `{"e":"win","t":ms,"r":[x,y,w,h]}` | Một cửa sổ khác (không phải của HP Screen) lên trước; khung của nó |

- Tọa độ theo pixel vùng quay, như các sự kiện chuột; khung bị cắt theo vùng quay; khung nằm ngoài vùng quay thì không ghi.
- **Riêng tư:** chỉ ghi khung chữ nhật. Không ghi tên, chữ, loại phần tử hay tiêu đề cửa sổ.
- Bản đọc cũ bỏ qua dòng lạ.

## Bộ quay

- **UI Automation** chạy trên một luồng riêng (COM MTA). Không bao giờ chạy trong hook, vì hook bị giới hạn thời gian.
  - Luồng ghi nhật ký gửi yêu cầu sang; kết quả gửi về và được ghi như các sự kiện khác.
  - Phần tử dưới chuột: lấy bằng `ElementFromPoint`. Nếu khung nhỏ hơn 16 × 12 px thì đi lên phần tử cha, tối đa 4 bước.
  - Ô đang gõ: lấy bằng `GetFocusedElement`.
  - Khi dừng quay, chờ các kết quả còn lại tối đa 1 giây. Ứng dụng khác bị treo cũng không làm việc dừng quay bị treo.
- **Đổi cửa sổ:** dùng `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` trên luồng hook sẵn có. Khung lấy theo DWM (khung nhìn thấy). Bỏ qua cửa sổ của chính HP Screen.
- Không truy vấn khi đang vẽ (cùng quy tắc với click).

## Lõi (`hp-core`)

- `Effects` thêm:
  - `elements: Vec<(ms, kind, [x,y,w,h])>` và `windows: Vec<(ms, [x,y,w,h])>`, theo pixel video;
  - `kind` là `"click"` hoặc `"focus"`.
- `AutoInput` thêm `boxes` (khung click), `focus` (khung ô gõ) và `switches` (thời điểm đổi cửa sổ), theo tọa độ 0–1.
- **1. Khớp phần tử**
  - Mỗi click lấy khung `click` sớm nhất trong vòng 300 ms sau nó, nếu khung đó chứa điểm click (dung sai 2%).
  - Khung bị bỏ khi một cạnh lớn hơn 60% khung hình, hoặc khi cả hai cạnh nhỏ hơn 0,5%.
  - Một nhóm có khung thì khung zoom là hình bao của các khung và các điểm click trong nhóm, cộng lề 0,06 mỗi bên.
  - Mức phóng là `clamp(1 / (cạnh lớn + 0,12), 1,25, 2,5)`; tâm là tâm của hình bao.
  - Nhóm không có khung giữ cách tính cũ.
- **2. Ô đang gõ**
  - Mỗi đợt gõ (các dấu `type` cách nhau dưới 1,5 s) có khung `focus` trong đợt được coi như một click ở tâm khung, tại lúc gõ đầu tiên, với khung đó.
  - Gõ ngay sau khi click vào ô thì cùng nhóm (gần nhau), không tạo lia thừa.
- **3. Đổi cửa sổ**
  - Hai nhóm cách nhau bởi một lần đổi cửa sổ thì không gộp chung một đoạn.
  - Đoạn trước kết thúc muộn nhất lúc đổi; đoạn sau bắt đầu sớm nhất 700 ms sau lúc đổi. Đoạn ngắn quá 1 s thì bỏ.
  - Nhờ vậy camera về toàn cảnh khi đổi cửa sổ.
- Mọi quy tắc cũ (mức độ, chặn mép, giữ khi gõ hay cuộn, trở ra sớm, vùng tự động) vẫn giữ.

## Kiểm thử

- **`hp-core`:**
  - đọc và ghi `el` / `win`;
  - `build_effects` đổi tọa độ;
  - khớp phần tử: nút nhỏ → khung ôm nút, mức tới 2,5×;
  - phần tử quá to thì bỏ qua;
  - click không nằm trong khung thì bỏ qua;
  - gõ vào ô ở xa chuột → zoom vào ô;
  - đổi cửa sổ tách đoạn và để lại khoảng trống.
- **`hp-screen`:**
  - `rel_rect` cắt khung theo vùng quay và trả `None` khi nằm ngoài;
  - truy vấn UIA chạy trên luồng riêng và trả kết quả; có một test cần màn hình thật (`--ignored`): `GetFocusedElement` trả về một khung.
- **Thử tay:** thêm Z20–Z23.

## Kế hoạch

1. Log format, `Effects`, lõi zoom: test trước, `cargo test -p hp-core`.
2. Bộ quay: luồng UIA, hook đổi cửa sổ, ghi sự kiện; `cargo test -p hp-screen --lib`.
3. `edit_cmds::auto_zoom` truyền khung và lần đổi cửa sổ vào lõi.
4. Danh sách thử tay, review độc lập, sửa, đẩy lên.
