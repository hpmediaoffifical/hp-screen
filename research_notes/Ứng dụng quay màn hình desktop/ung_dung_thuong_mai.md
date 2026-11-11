# Ứng dụng quay màn hình desktop thương mại / closed-source (trạng thái tháng 11/2026, trọng tâm Windows)

Ghi chú chung về độ tin cậy:
- Phần Bandicam lấy trực tiếp từ trang hướng dẫn chính thức bandicam.com (phiên bản hiện hành là **Bandicam 2027**). Đây là nguồn mạnh nhất trong ghi chú.
- Các app khác lấy từ trang sản phẩm/bảng giá chính thức (một lần đọc mỗi trang). Giá Camtasia, Snagit, Icecream lấy từ trang tổng hợp bên thứ ba vì trang chính thức không đọc được (404) hoặc không ghi giá, nên được gắn nhãn **chưa xác minh từ nguồn gốc**.
- Không tìm được thread Reddit nào qua công cụ tìm kiếm; phần "khen/chê" dựa trên Capterra/G2/blog review, trong đó có blog của đối thủ (thiên lệch).
- Mọi thứ nằm ở mục "Gaps" là chưa xác minh, không nên coi là sự thật.

---

## 1. Bandicam có những chế độ quay nào, mỗi chế độ hoạt động ra sao, phơi ra những thiết lập gì, và giới hạn bản chưa đăng ký là gì?

### Takeaway
Bandicam 2027 có 4 nhóm chế độ (Screen: Rectangle area / Fullscreen / Specific window / Around mouse; Game; Webcam-HDMI device; Audio only), xuất MP4/AVI với codec AV1/HEVC/H.264/Xvid/MPEG-1/MJPEG/YV12/RGB24, FPS 1–1000, hotkey mặc định F12 (quay) và F11 (chụp ảnh). Bản miễn phí 2027 đã đổi luật: **giới hạn 5 phút mỗi lần quay, KHÔNG còn watermark trên video quay gốc** (chỉ watermark khi export video đã chỉnh sửa/gắn phụ đề); bản 2024/2025 cũ là 10 phút + watermark.

### Cited Findings

**Chế độ quay**
- Trang hỗ trợ liệt kê các chế độ: Screen (màn hình và vùng chọn), Game (game 2D/3D), Webcam/HDMI (webcam và capture device), Audio only (loa + micro), và trường hợp dùng cho họp online (Teams/Zoom), video web, VLC, PowerPoint — [Bandicam Support](https://www.bandicam.com/support/)
- Screen Recording có 4 chế độ con: Rectangle area, Fullscreen, Specific window, Around mouse; truy cập qua Home > Get Started hoặc mũi tên drop-down góc trên trái — [Record selected area](https://www.bandicam.com/guide/record-selected-area/)
- Rectangle area: kéo cạnh/góc để đổi kích thước, chọn lại vùng, chọn preset hoặc custom size, di chuyển vị trí; nút Blur, webcam overlay, bật/tắt loa và micro nằm ngay trên khung quay, dùng được trước và trong khi quay; có thể vẽ mũi tên, hộp, số thứ tự, text, highlight trong lúc quay — [Record selected area](https://www.bandicam.com/guide/record-selected-area/)
- Fullscreen: chọn display; "Fullscreen > All display" chụp tất cả màn hình vào một ảnh (đối với screenshot) — [Screen capture](https://www.bandicam.com/guide/screen-capture/)
- Game Recording: dành cho game DirectX/OpenGL/Vulkan; chọn icon gamepad rồi mở game; số FPS **màu xanh lá** xuất hiện khi Bandicam nhận diện được game, chuyển **màu đỏ** khi đang quay; có thể cần chấp nhận UAC (quyền admin); muốn quay app Microsoft Store phải bật "Windows Apps (UWP)" trong General > Advanced; nếu không hiện FPS xanh thì dùng Specific window thay thế; trang nói Game mode cho chất lượng tốt hơn Screen mode — [Record game](https://www.bandicam.com/guide/record-game/)
- Thiết lập Hooking: "Hooking target" (chọn công nghệ đồ họa mà Game Recording nhận diện), "Disable DX10/11 hooking on IE9(or above)/Firefox", "Block DirectX video overlay", "Blocked process list" (ví dụ chrome.exe, firefox.exe) — [Settings General](https://www.bandicam.com/guide/settings-general/)
- Hotkey riêng Game mode: FPS Show/Hide F9, FPS Position Shift+F9, FPS Limit F10 — [Hotkeys](https://www.bandicam.com/guide/hotkeys/)
- Quay liên tục hơn 24 giờ; tối đa 4K (3840x2160); trang tuyên bố file nhỏ hơn 20–95% so với phần mềm khác nhờ nén trong lúc quay — [Bandicam Support](https://www.bandicam.com/support/)

**Định dạng, codec, chất lượng**
- Container: "AVI" và "MP4" — [Settings Video](https://www.bandicam.com/guide/settings-video/)
- Video codec: AV1, HEVC (H.265), H.264, XVID, MPEG-1, Motion JPEG, YV12 (uncompressed), RGB24 (uncompressed), và External Codec (VFW) — [Settings Video](https://www.bandicam.com/guide/settings-video/)
- Video size: "Full Size", "Half Size", "Fit Width", "Fit Height", "320*240", "Custom" — [Settings Video](https://www.bandicam.com/guide/settings-video/)
- FPS: 1 đến 1.000; có tùy chọn VFR/CFR — [Settings Video](https://www.bandicam.com/guide/settings-video/)
- Chất lượng theo "Quality (%)"; bitrate cố định khả dụng khi bỏ chọn VBR — [Settings Video](https://www.bandicam.com/guide/settings-video/)
- Audio codec: AAC (48–448 kbps), MP2, MP3 (32–320 kbps), PCM (có "Save as original format" cho đa kênh/hi-def); kênh Stereo/Mono; tần số 22.050/32.000/44.100/48.000 Hz — [Settings Video](https://www.bandicam.com/guide/settings-video/)
- Có "Presets" để chọn nhanh tổ hợp video + audio — [Settings Video](https://www.bandicam.com/guide/settings-video/)

**Âm thanh**
- "Record sound"; "Save audio tracks while recording" (lưu file WAV hoặc MP3 riêng); Speakers + Microphone (có volume micro); "Filter" (chuyển mono hoặc noise suppression); Secondary Sound Device; "Two Sound Mixing" (trộn 2 thiết bị thành một track); Push-To-Talk / Push-To-Mute (mặc định phím Space, phải bật mới có hiệu lực) — [Settings Video](https://www.bandicam.com/guide/settings-video/), [Hotkeys](https://www.bandicam.com/guide/hotkeys/)
- Specific window có "Capture audio exclusively": chỉ ghi âm thanh của cửa sổ được chọn — [Record window](https://www.bandicam.com/guide/record-window/)
- Hotkey Mute Speakers Ctrl+Alt+S, Mute Microphone Ctrl+Alt+M — [Hotkeys](https://www.bandicam.com/guide/hotkeys/)

**Hiệu ứng chuột, phím, logo, text**
- "Show mouse cursor" (chỉnh kích thước); "Add mouse click effect" (kích thước, màu riêng cho click trái/phải); "Add mouse highlight effect" (kích thước, màu, opacity); "Play mouse click sounds while recording" — [Settings Video](https://www.bandicam.com/guide/settings-video/)
- "Add keystroke to video" (chỉ Screen Recording mode), chỉnh font size và vị trí — [Settings Video](https://www.bandicam.com/guide/settings-video/)
- "Add logo overlay to video" (ảnh, opacity, vị trí, margin); "Add text overlay to video" (danh sách text, thêm/sửa/xóa/preview) — [Settings Video](https://www.bandicam.com/guide/settings-video/)

**Webcam overlay**
- "Add webcam overlay to video", "Save webcam recording as MP4" (file riêng), cài đặt Device/format, Flip/Rotate, Border (màu, độ dày, faded border), Background (blur, remove, virtual background, ảnh nền tùy chỉnh), Shape, kích thước và vị trí; trong Screen mode có thể kéo thả, resize, phóng to webcam (Shift+Tab); trong Game mode có preview bật/tắt bằng Tab — [Overlay webcam](https://www.bandicam.com/guide/overlay-webcam/), [Hotkeys](https://www.bandicam.com/guide/hotkeys/)
- Bản free chỉ có blur nền webcam; background removal, virtual/custom background và mọi shape cần bản full — [Overlay webcam](https://www.bandicam.com/guide/overlay-webcam/)

**Vẽ khi đang quay (Drawing mode)**
- Công cụ: Cursor mode, Pen, Highlighter, Line, Arrow, Box, Ellipse, Numbering, Text, Eraser, Whiteboard, Undo/Redo, Clear all; hotkey Ctrl+Alt+D bật/tắt, Ctrl+Alt+1..9 chọn công cụ, Ctrl+Alt+Q whiteboard, giữ Shift để ràng buộc đường thẳng/hình vuông, Esc xóa và thoát — [Hotkeys](https://www.bandicam.com/guide/hotkeys/)
- Vẽ real-time KHÔNG dùng được trong Specific window mode — [Record window](https://www.bandicam.com/guide/record-window/)

**Blur**
- Bandicam 2027 có multi-area blur (tối đa 3 vùng) — [Bandicam Support](https://www.bandicam.com/support/)

**Hotkey mặc định (đầy đủ)**
- Start/Stop Recording **F12**; Pause **Shift+F12**; Cancel Recording **Ctrl+Shift+F12**; Capture Image **F11**; Show/Hide Rectangle Ctrl+Alt+H; Select target area Ctrl+Shift+F — [Hotkeys](https://www.bandicam.com/guide/hotkeys/)
- Khung quay: phím mũi tên để dịch chuyển; Shift+mũi tên để đổi rộng/cao; Ctrl+F chọn vùng; Ctrl+Enter fullscreen; Ctrl+S kích thước dùng gần đây; Ctrl+U custom size; Ctrl+1..9 preset 320x240, 480x268, 480x360, 640x360, 640x480, 848x480, 1024x768, 1280x720, 1920x1080; Shift+Right-click mở Size Preset / Add a Preset — [Hotkeys](https://www.bandicam.com/guide/hotkeys/)
- Quản lý file trong app: F2 Rename, Del Delete, Ctrl+A Select all — [Hotkeys](https://www.bandicam.com/guide/hotkeys/)
- Mọi hotkey có thể bật/tắt/đổi; nút "R" khôi phục mặc định từng phím — [Settings General](https://www.bandicam.com/guide/settings-general/)

**Thiết lập chung, lịch quay, tự kết thúc**
- General: Output folder (chọn/mở), "Bandicam window always on top", "Start Bandicam minimized to tray", "Run Bandicam on Windows startup", "Show FPS in captured video/image", "Turn off tray balloon notifications", "Start full-screen recording on Bandicam startup", "Set timer for Auto Start Recording" — [Settings General](https://www.bandicam.com/guide/settings-general/)
- Output: "Enable write caching on the disk"; "Autocreate output subfolder with name" gồm "Same as recording target name" (ví dụ thư mục "WOW" cho WOW.exe), "'Videos'/'Images'/'Audios'", "Date format (YYYY-MM-DD)" — [Settings General](https://www.bandicam.com/guide/settings-general/)
- Scheduled Recording: nút Add tạo lịch bắt đầu/kết thúc; Bandicam phải đang chạy thì lịch mới kích hoạt — [Settings General](https://www.bandicam.com/guide/settings-general/)
- Auto Complete Recording: dừng theo thời gian, dung lượng file, hoặc khoảng im lặng (silent duration); "After Recording": không làm gì / bắt đầu quay mới / thoát Bandicam / shutdown / hibernate — [Settings General](https://www.bandicam.com/guide/settings-general/)
- Options tab: "Recording priority" (Normal/High/Highest); "Play sound effects when the recording starts or stops"; "Open video player after recording"; "Use enhanced capture method" với "Auto select method (default)", "Method 1" (Windows 8+), "Method 2" (Windows 10+); "Exclude Bandicam windows from recording"; "Hide main window during recording"; "Show countdown timer before recording begins" 1–10 giây, mặc định 3; "Auto-Pause Recording" (sau một khoảng không hoạt động); "Skip recording while frame is not updated" (Game mode) — [Settings Video](https://www.bandicam.com/guide/settings-video/)
- Quyền chạy: "Run Bandicam as standard user (non-admin)", "Run Bandicam as administrator", "Run as administrator only on 'Game Recording' mode" — [Settings General](https://www.bandicam.com/guide/settings-general/)
- Dữ liệu quay lưu cục bộ, không upload lên server Bandicam — [Settings General](https://www.bandicam.com/guide/settings-general/)

**Tính năng mới trong Bandicam 2027 (chỉnh sửa và AI)**
- Có trình phát tích hợp mở sau khi dừng quay, nút Edit để trim và Export — [Smart zoom](https://www.bandicam.com/guide/smart-zoom-in-out/)
- Smart Zoom: mô tả nguyên văn "Smart Zoom shows a black cursor and automatically zooms in and out based on your mouse clicks and movements"; áp dụng **ở bước chỉnh sửa sau khi quay**, không phải lúc quay; xuất MP4 mới với hậu tố "_exported", giữ file gốc; chỉ dùng được cho video quay bằng Rectangle area hoặc Fullscreen (không dùng cho Specific window, Game) — [Smart zoom](https://www.bandicam.com/guide/smart-zoom-in-out/)
- Transcription (AI): chọn model Tiny / Base / Small / Medium / Large Turbo (mặc định Base); Auto Detect ngôn ngữ hoặc chọn tay; copy clipboard, lưu TXT/SRT, export video có phụ đề burned-in (hậu tố "_subtitle"); tìm kiếm trong transcript; có công cụ AI dịch phụ đề, tóm tắt, tạo quiz, auto-edit, tạo highlights — [Video to text](https://www.bandicam.com/guide/video-to-text/)
- Upload trực tiếp YouTube, Google Drive, Vimeo; có Bandifix (khôi phục video hỏng) — [Bandicam Support](https://www.bandicam.com/support/)

**Giới hạn bản free / unregistered**
- Bandicam 2027 free: video và audio-only giới hạn **5 phút mỗi lần quay**; "Recorded videos have no watermark"; video đã edit/gắn phụ đề khi export có watermark; tối đa 3 dòng phụ đề burned-in; không có virtual background/background removal webcam; AI Transcription chỉ xem được một phần, không copy/lưu TXT/SRT; scheduled recording chỉ cho bản quay dưới 5 phút, không có quay liên tục 24/365 — [Free vs Full](https://www.bandicam.com/faqs/free-full-version/)
- Bản cũ Bandicam 2024/2025: tối đa 10 phút mỗi lần quay và có watermark trên bản quay — [Free vs Full](https://www.bandicam.com/faqs/free-full-version/)

**Giá / giấy phép (trang Buy chính thức, có khuyến mãi đang chạy)**
- Personal (phi thương mại, không hóa đơn): Annual 1-PC $33.26/năm; Bandicam + Bandicut 1 năm $49.97; Perpetual 1-PC $53.90 (giá gốc $77.00), áp dụng cho Bandicam 2027 kèm 1 năm hỗ trợ kỹ thuật — [Bandicam Buy](https://www.bandicam.com/buy/)
- Business & Education (có hóa đơn): Annual 1-PC $49.46 (gốc $65.95); Annual 2-PC $95.92; bundle với Bandicut $79.33 — [Bandicam Buy](https://www.bandicam.com/buy/)
- Dùng cho công việc/thương mại bắt buộc Business License kể cả khi thử bản free — [Free vs Full](https://www.bandicam.com/faqs/free-full-version/)

### Inferences
- Bandicam đã chuyển từ "recorder thuần" sang "recorder + editor nhẹ + AI" trong bản 2027 (trim, Smart Zoom, transcription Whisper-style, tóm tắt). Tên model "Tiny/Base/Small/Medium/Large Turbo" trùng với các kích cỡ model OpenAI Whisper, nên nhiều khả năng là Whisper chạy cục bộ — nhưng trang không nêu tên engine.
- Điểm yếu thiết kế có thể khai thác: Smart Zoom không có tham số điều chỉnh được ghi trong tài liệu, không hoạt động với Specific window/Game; drawing không dùng được ở Specific window; phụ đề/transcript bị khóa mạnh ở bản free.
- Việc giới hạn free chuyển từ "10 phút + watermark" sang "5 phút, không watermark" có nghĩa watermark không còn là rào cản cạnh tranh chính; giới hạn thời lượng mới là rào cản.
- Hotkey mặc định F12 xung đột với DevTools trình duyệt (chính Bandicam thừa nhận và gợi ý đổi sang F9) — HP Screen nên chọn mặc định ít xung đột hơn hoặc cảnh báo xung đột.

### Gaps
- **NVENC / Intel Quick Sync / AMD AMF**: trang Settings Video được đọc không liệt kê tên các hardware encoder. Bandicam trước đây có quảng cáo H.264/HEVC/AV1 qua NVENC, QSV, AMF nhưng tôi không xác minh được trong lần nghiên cứu này — chưa xác minh.
- **Chroma key**: không thấy trên trang overlay webcam hiện hành (thay vào đó là AI background removal/virtual background). Không rõ tùy chọn chroma key cũ còn tồn tại hay không — chưa xác minh.
- **Around mouse**: chỉ có tên chế độ, không có mô tả cách hoạt động (kích thước khung bám theo con trỏ) trên các trang đã đọc.
- **Device/HDMI mode và Audio-only mode**: chưa đọc trang hướng dẫn riêng (/guide/record-webcam/, /guide/record-audio-only/); định dạng file audio-only chưa xác minh.
- Giá trị mặc định của FPS, Quality, codec, bitrate không được nêu trên trang.
- Chính sách hoàn tiền và giá Business perpetual không có trên trang Buy.

---

## 2. Chế độ "Specific window" của Bandicam quay một cửa sổ mà không dính các cửa sổ đè lên bằng cách nào?

### Takeaway
Bandicam xác nhận Specific window quay đúng một cửa sổ kể cả khi bị cửa sổ khác che, chỉ hoạt động trên Windows 10/11 và không quay được khi cửa sổ bị minimize; tài liệu không nêu tên API, nhưng hành vi và giới hạn khớp với mô hình capture theo từng cửa sổ của Windows (kiểu Windows Graphics Capture).

### Cited Findings
- Chế độ này ghi một cửa sổ chương trình được chọn; cửa sổ khác có thể che lên mà bản quay vẫn chỉ chứa cửa sổ đích — [Record window](https://www.bandicam.com/guide/record-window/)
- Có thể quay toàn bộ cửa sổ hoặc một phần cửa sổ bằng "Select a recording area on specific window"; vùng đó vẫn được quay khi bị che — [Record window](https://www.bandicam.com/guide/record-window/)
- Windows 10/11: được hỗ trợ. Windows 7/8: không quay được cửa sổ riêng, thay vào đó ghi mọi thứ trên màn hình — [Record window](https://www.bandicam.com/guide/record-window/)
- Không capture được nếu cửa sổ bị minimize — [Record window](https://www.bandicam.com/guide/record-window/)
- Chỉ cửa sổ được chỉ định được ghi: menu chuột phải (context menu) và popup phụ (ví dụ của Photoshop) KHÔNG được ghi; vẽ real-time không khả dụng — [Record window](https://www.bandicam.com/guide/record-window/)
- Tab mới mở trong cùng trình duyệt đang được quay sẽ bị ghi; Bandicam khuyên dùng trình duyệt khác cho việc khác — [Record window](https://www.bandicam.com/guide/record-window/)
- Zoom: bản quay dừng khi share screen hoặc chuyển Zoom sang full screen; khuyên dùng Rectangle/Fullscreen cho họp — [Record window](https://www.bandicam.com/guide/record-window/)
- Lỗi video đen/trắng khi một cửa sổ full-screen che cửa sổ video; Bandicam gợi ý thêm cờ `--disable-backgrounding-occluded-windows` vào shortcut Chrome/Edge và tắt Startup Boost của Edge — [Record window](https://www.bandicam.com/guide/record-window/)
- "Capture audio exclusively" ghi riêng âm thanh của cửa sổ đó — [Record window](https://www.bandicam.com/guide/record-window/)
- Options có "Use enhanced capture method": "Method 1" (Windows 8+), "Method 2" (Windows 10+), "Auto select method (default)" — [Settings Video](https://www.bandicam.com/guide/settings-video/)
- Smart Zoom không dùng được cho video quay bằng Specific window — [Smart zoom](https://www.bandicam.com/guide/smart-zoom-in-out/)
- Đối chiếu: Action! có "Selected Application mode" ghi chỉ một app và **bôi đen phần còn lại của desktop** — một cách tiếp cận khác (che thay vì capture theo cửa sổ) — [Mirillis Action!](https://mirillis.com/en/products/action.html)

### Inferences
- Bộ đặc điểm (yêu cầu Windows 10+, capture được khi bị che, không capture khi minimize, không thấy popup/context menu là cửa sổ top-level riêng, nội dung đen khi trình duyệt ngừng render vì bị che) trùng với hành vi của capture theo HWND của Windows Graphics Capture (WGC). "Method 1 (Windows 8+)" nhiều khả năng là DXGI Desktop Duplication và "Method 2 (Windows 10+)" là WGC. Đây là suy luận, Bandicam không xác nhận.
- Lỗi đen/trắng với Chrome/Edge không phải do recorder mà do trình duyệt dừng vẽ khi bị che (occlusion backgrounding). HP Screen sẽ gặp đúng vấn đề này nếu dùng capture theo cửa sổ; nên phát hiện frame không đổi/đen và cảnh báo người dùng, hoặc có hướng dẫn tự động.
- "Capture audio exclusively" tương ứng với khả năng loopback theo process của Windows (process loopback, Windows 10 2004+) — suy luận, không được Bandicam nêu.
- Cơ hội vượt Bandicam: (a) gộp popup/context menu/dialog con của cùng process vào bản quay cửa sổ; (b) cho phép vẽ và auto-zoom trong chế độ cửa sổ; (c) tự chuyển đổi/giữ hình khi cửa sổ bị minimize thay vì hỏng bản quay.

### Gaps
- Bandicam không công bố API capture; không xác minh được có viền vàng (yellow border của WGC) hay không, cũng như cách xử lý nội dung DRM.
- Không tìm thấy tài liệu về cách Bandicam xử lý khi cửa sổ đổi kích thước giữa lúc quay (scale, letterbox hay đổi độ phân giải).

---

## 3. Người dùng khen nhất và chê nhất điều gì ở các app này (pain point mà app mới có thể giải quyết)?

### Takeaway
Bandicam được khen vì nhẹ, khởi động nhanh, giao diện đơn giản; bị chê vì giới hạn bản free, giá so với lựa chọn miễn phí (OBS), giấy phép 1 PC, và thiếu trình chỉnh sửa thực thụ. Ở nhóm còn lại, pain point rõ nhất là mô hình subscription (Camtasia, Snagit, Screen Studio đều bỏ bán vĩnh viễn cho khách mới) và giới hạn gắt của gói free (Loom 5 phút/25 video, ScreenPal 15 phút + watermark, Icecream 5 phút + watermark + chỉ WEBM).

### Cited Findings
- Khen Bandicam: khởi động rất nhanh, chạy mượt khi quay; giao diện "really simple" — vừa là ưu vừa là nhược điểm — [Capterra Bandicam reviews](https://www.capterra.com/p/188396/Bandicam-Screen-Recorder/reviews/)
- Chê Bandicam: watermark lớn và giới hạn 10 phút ở bản free (phản ánh bản cũ, trước 2027) — [Capterra Bandicam reviews](https://www.capterra.com/p/188396/Bandicam-Screen-Recorder/reviews/)
- Chê Bandicam: giá bị coi là cao khi có lựa chọn miễn phí mạnh hơn; người dùng muốn một license dùng cho 2 PC — [Capterra Bandicam reviews](https://www.capterra.com/p/188396/Bandicam-Screen-Recorder/reviews/)
- Chê Bandicam: file có thể chiếm nhiều dung lượng tùy độ dài video — [Capterra Bandicam reviews](https://www.capterra.com/p/188396/Bandicam-Screen-Recorder/reviews/); mâu thuẫn với tuyên bố file nhỏ hơn 20–95% của chính hãng — [Bandicam Support](https://www.bandicam.com/support/)
- Chê Bandicam: không được thiết kế để chỉnh sửa, phải import sang công cụ khác (nguồn là blog của đối thủ ActivePresenter, cần thận trọng) — [Atomi review](https://atomisystems.com/screencasting/bandicam-screen-recorder-review-2026-real-pros-cons-who-its-actually-for/)
- OBS Studio là lựa chọn thay thế được nhắc nhiều nhất; một số người thấy Snipping Tool có sẵn là đủ cho nhu cầu cơ bản — [Capterra Bandicam reviews](https://www.capterra.com/p/188396/Bandicam-Screen-Recorder/reviews/)
- Camtasia: chỉ còn subscription thanh toán theo năm, không còn perpetual và không có gói tháng — [Capterra Loom vs Camtasia](https://www.capterra.com/compare/191187-203013/Loom-vs-Camtasia)
- Snagit: người dùng thích công cụ sửa cơ bản nhưng thường phải tìm công cụ nâng cao ở nơi khác — [G2 Loom vs Snagit](https://www.g2.com/compare/atlassian-loom-vs-snagit)
- Loom: bị nhận xét "limited functionality" dù dễ dùng — [Capterra Loom reviews](https://capterra.com/p/191187/Loom/reviews/?page=5)
- Screen Studio: không còn bán license một lần cho khách mới; giá $29/tháng hoặc $9/tháng trả theo năm — [Screen Studio](https://screen.studio/)
- Xbox Game Bar: không quay được desktop Windows hoặc File Explorer; nút record bị vô hiệu khi desktop/Explorer đang focus — [Appuals](https://appuals.com/gaming-features-arent-available/), [The Windows Club](https://thewindowsclub.com/gaming-features-arent-available-for-windows-desktop-or-file-explorer)
- Bandicam Specific window: không ghi popup/context menu, dừng khi Zoom đổi chế độ, video đen khi bị cửa sổ full-screen che — [Record window](https://www.bandicam.com/guide/record-window/)
- Movavi Screen Recorder: bản thử 7 ngày có watermark; không dành cho game (hãng đẩy sang sản phẩm Gecata) — [Movavi Screen Recorder](https://www.movavi.com/screen-recorder/)

### Inferences
- Các pain point có thể giải bằng thiết kế cho HP Screen: (1) không giới hạn thời lượng, không watermark; (2) license không khóa cứng 1 PC; (3) trim/cắt nhanh ngay trong app mà không re-encode; (4) quay được mọi thứ (desktop, Explorer, cửa sổ, game) trong một app — khác Game Bar; (5) chế độ cửa sổ ghi được cả popup/menu; (6) auto-zoom/cursor effect chỉnh được tham số và hoạt động ở mọi chế độ; (7) hotkey mặc định không đụng F12.
- "Nhẹ, nhanh, đơn giản" là lý do chính người dùng trung thành với Bandicam; app mới không nên đánh đổi điều này để lấy tính năng editor nặng.

### Gaps
- Không truy xuất được thread Reddit/forum nào (công cụ tìm kiếm không trả kết quả Reddit); phần phàn nàn dựa trên Capterra/G2 và blog vendor nên có thể thiên lệch và thiếu chi tiết kỹ thuật (lệch tiếng, drop frame, lỗi encoder...).
- Không có dữ liệu khen/chê riêng cho FocuSee, Tella, Action!, CleanShot X, NVIDIA App trong lần nghiên cứu này.

---

## 4. App nào có auto zoom theo click, cursor smoothing, click highlight/ripple, hiển thị keystroke, auto caption/subtitle, AI summary/title/chapter, cắt im lặng/filler word? Mỗi app mô tả nó thế nào?

### Takeaway
Auto zoom + cursor smoothing + motion blur là "bộ ba" của nhóm Screen Studio / FocuSee / CleanShot X / Tella và nay Bandicam 2027 cũng có Smart Zoom (không có smoothing được ghi nhận). Click effect và keystroke overlay gần như phổ cập (Bandicam, Snagit, Movavi, CleanShot X, Screen Studio, FocuSee). AI title/summary/chapter và filler/silence removal chủ yếu nằm ở gói trả phí cao của Loom, Tella, ScreenPal, Screen Studio, FocuSee.

### Cited Findings

**Ma trận tính năng (chỉ ghi những gì nguồn xác nhận)**

| App | Auto zoom | Cursor smoothing / motion blur | Click effect | Keystroke | Caption / transcript | AI title/summary/chapter | Filler / silence removal |
|---|---|---|---|---|---|---|---|
| Bandicam 2027 | Có (Smart Zoom, hậu kỳ) | Không thấy ghi | Có (màu trái/phải, highlight, âm click) | Có (Screen mode) | Có (model Tiny–Large Turbo, TXT/SRT, burned-in) | Có tóm tắt, quiz, highlights | Không thấy ghi (có "auto-edit") |
| Screen Studio (macOS) | Có | Có + motion blur | Có (circle/ripple, âm click) | Có | Có (cục bộ) | Không thấy ghi | Có |
| FocuSee (Win/Mac) | Có | Motion blur, 3D Motion | Có (animation + âm) | Có | Có (50+ ngôn ngữ) | Không thấy ghi | Có |
| CleanShot X (macOS) | Có (trong video editor) | Có + motion blur | Có | Có | Transcription qua Cloud | Không thấy ghi | Không thấy ghi |
| Tella | Có | Không thấy ghi | Không thấy ghi | Không thấy ghi | Có (106 ngôn ngữ) | Có | Có |
| Loom | Không thấy ghi | Không thấy ghi | Không thấy ghi | Không thấy ghi | Có (50+ ngôn ngữ) | Có (Business + AI) | Có (Business + AI) |
| ScreenPal | Zoom in/out (không ghi là tự động) | Không thấy ghi | Không thấy ghi | Không thấy ghi | Có (từ Deluxe) | Có (Max) | Không thấy ghi |
| Snagit | Không thấy ghi | Không thấy ghi | Có (cursor highlight, click ripple) | Không thấy ghi | Có (PRO, cho video chia sẻ) | Chapter summaries (PRO) | Không thấy ghi |
| Movavi | Không thấy ghi | Không thấy ghi | Có (highlight cursor, click) | Có | Không thấy ghi | Không thấy ghi | Không thấy ghi |
| Icecream | Không thấy ghi | Không thấy ghi | Không thấy ghi | Hiển thị hotkey đã dùng | Không thấy ghi | Không thấy ghi | Không thấy ghi |
| Snipping Tool | Không | Không | Không | Không | Qua Clipchamp | Không | Không |

**Cách mỗi app mô tả**
- Bandicam: "Smart Zoom shows a black cursor and automatically zooms in and out based on your mouse clicks and movements"; bật trong bước Edit sau khi quay, export MP4 — [Smart zoom](https://www.bandicam.com/guide/smart-zoom-in-out/)
- Bandicam click/highlight/keystroke: "Add mouse click effect", "Add mouse highlight effect", "Play mouse click sounds while recording", "Add keystroke to video" — [Settings Video](https://www.bandicam.com/guide/settings-video/)
- Screen Studio auto zoom: zoom được thêm tự động theo hành động đã ghi (click); "Every animation is created automatically"; có zoom thủ công; hai kiểu Classic zoom và Loupe zoom; "6.5× upscale" on-device — [Screen Studio](https://screen.studio/)
- Screen Studio cursor: "Shaky movement becomes a steady glide, without looking artificial"; "Change the cursor's size and style after you record"; con trỏ hệ thống được thay bằng bản độ phân giải cao khi phóng to; ẩn khi đứng yên: "When the cursor rests, it steps out of view until you move it again"; motion blur "optics-based" — [Screen Studio](https://screen.studio/)
- Screen Studio click/keystroke: "A quiet circle or ripple, so every click is easy to follow"; âm click tùy chọn; "The keys you press appear on screen, so viewers can follow along" — [Screen Studio](https://screen.studio/)
- Screen Studio caption/AI: caption tạo "right on your Mac"; transcription cục bộ (Whisper hoặc Apple speech recognition) hoặc cloud tùy chọn; edit-by-transcript; bỏ filler word và khoảng dừng; export .srt; AI voiceover; noise reduction — [Screen Studio](https://screen.studio/)
- Screen Studio webcam: cutout nền xử lý trên máy, layout overlay/fullscreen/split, ẩn khi im lặng, face tracking, smart framing để webcam không che con trỏ — [Screen Studio](https://screen.studio/)
- FocuSee: auto-zoom thông minh bám theo click và gõ phím, không cần keyframe thủ công; spotlight; privacy blur; con trỏ "thông minh" hiện khi cần và mờ đi khi tập trung; nhiều style con trỏ; 3D Motion (nghiêng/phối cảnh theo con trỏ); motion blur; click animation + âm thanh; keystroke tự hiển thị — [FocuSee](https://focusee.ai/)
- FocuSee AI: speech recognition 50+ ngôn ngữ, tuyên bố độ chính xác trên 98% (số của hãng); sửa phụ đề (style, text, timing, tách/gộp, import SRT); AI voice enhancement, noise reduction, tự bỏ filler word và khoảng lặng; AI background removal; teleprompter; camera beautification — [FocuSee](https://focusee.ai/)
- CleanShot X khi quay: "Capture clicks" (đổi màu, kích thước, style, bật/tắt animation); "Capture keystrokes" (vị trí, kích thước, style, chọn tất cả phím hoặc chỉ phím lệnh); tự bật Do Not Disturb; ẩn desktop icon — [CleanShot X features](https://cleanshot.com/features)
- CleanShot X video editor: smart zoom bám con trỏ, làm mượt chuyển động con trỏ, đổi kích thước/style con trỏ và click effect, nền tùy chỉnh, motion blur chỉnh cường độ, export ngang/vuông/dọc — [CleanShot X features](https://cleanshot.com/features)
- Tella: Auto Layouts, Auto Zoom, zoom effects, blur, highlight, background, border; AI: Tella Agent (beta, tính credit), "AI mistakes finder", filler word removal, silence removal, buffer removal; phụ đề chỉnh style và export; AI title/description/chapter tự sinh và sửa tay được; transcription 106 ngôn ngữ — [Tella pricing](https://www.tella.com/pricing)
- Loom AI (gói Business + AI): auto titles, auto CTA, auto summaries, auto chapters, auto tasks, filler word removal, silence removal, edit by transcript, auto-video enhancement, auto-meeting notes và recap email — [Loom pricing](https://www.loom.com/pricing)
- Loom Starter đã có transcription 50+ ngôn ngữ và closed caption; drawing tool, blur, Do Not Disturb, custom recording dimensions thuộc Business — [Loom pricing](https://www.loom.com/pricing)
- ScreenPal: AI speech-to-text caption từ gói Deluxe; AI titles/summaries/chapters, AI text-to-speech, AI video generator ở gói Max; drawing tools và zoom in/out có ở mọi gói — [ScreenPal plans](https://screenpal.com/plans)
- Snagit: cursor highlight, click ripple animation, vẽ trên màn hình khi quay (mũi tên, hình, số bước); AI video captions và chapter summaries cho video chia sẻ (gói PRO); Step recorder dựng hướng dẫn từng bước từ click — [Snagit features](https://www.techsmith.com/snagit/features/)
- Movavi: highlight con trỏ, click và keystroke; vẽ real-time; noise reduction khi quay — [Movavi Screen Recorder](https://www.movavi.com/screen-recorder/)
- Icecream: hiển thị tổ hợp hotkey đã dùng trong lúc quay (Windows); vẽ, text, mũi tên, bullet — [Icecream Screen Recorder](https://icecreamapps.com/Screen-Recorder/)
- Snipping Tool: caption tự sinh thông qua Clipchamp (transcribe media) — [Microsoft Support](https://support.microsoft.com/en-us/windows/use-snipping-tool-to-capture-screenshots-00246869-1843-655f-f220-97299b865f6b)
- Camtasia: một nguồn bên thứ ba liệt kê Editor có "Cursor animation" và "Zoom and pan"; AI script generation, voiceover generation, transcript generation ở tier Create; avatar, translation, dubbing ở tier Pro (nguồn tổng hợp, không phải TechSmith) — [Guidde Camtasia pricing guide](https://www.guidde.com/knowledge-hub/camtasia-pricing-guide-2026)

### Inferences
- Cách làm chuẩn của nhóm "auto zoom": ghi metadata chuột/phím (vị trí, click, keystroke) song song với video rồi áp hiệu ứng ở hậu kỳ không phá hủy (Screen Studio, CleanShot X, Bandicam 2027 đều áp dụng sau khi quay và cho đổi style con trỏ về sau). HP Screen nên ghi một track sự kiện input riêng để có thể sửa/tắt zoom sau khi quay.
- Trên Windows, chỉ FocuSee (và Tella có app Windows) cạnh tranh trực tiếp mảng auto-zoom đẹp; Bandicam mới có Smart Zoom nhưng hạn chế. Đây là khoảng trống rõ nhất cho một recorder Windows nhẹ.
- Transcription cục bộ (Whisper) đã thành chuẩn (Bandicam, Screen Studio); AI summary/title thường bị gắn với cloud + subscription. Làm cục bộ, miễn phí là lợi thế khác biệt.

### Gaps
- Camtasia: không đọc được trang chính thức (404); cursor smoothing, filler word removal (Audiate) của Camtasia chưa xác minh trong lần nghiên cứu này.
- Không xác minh được Tella/Loom có click effect hay keystroke overlay.
- Không xác minh được Action!, NVIDIA App, Xbox Game Bar có bất kỳ hiệu ứng con trỏ nào (các trang đã đọc không nhắc tới).
- "Không thấy ghi" trong bảng nghĩa là trang đã đọc không đề cập, không phải khẳng định app không có.

---

## 5. Có những tính năng chụp ảnh tĩnh (screenshot) nào: định dạng, native-resolution/DPI, chụp liên tục, scrolling capture, copy clipboard, annotation?

### Takeaway
Bandicam chụp BMP/PNG/JPG (không có WebP), có chụp lặp 0,1–9999 giây và copy clipboard, nhưng không có scrolling capture hay OCR. Snagit và CleanShot X là chuẩn mực về screenshot (scrolling, OCR, redact, annotation, nhiều định dạng gồm WebP); Movavi và ScreenPal cũng có scrolling screenshot.

### Cited Findings
- Bandicam: định dạng BMP, PNG, JPG; JPG có Normal / High quality / Custom 20–100%; hotkey mặc định F11 (đổi được); chụp vùng chữ nhật hoặc toàn display; "Fullscreen > All display" gộp mọi màn hình vào một ảnh — [Screen capture](https://www.bandicam.com/guide/screen-capture/)
- Bandicam "Repeat screen capture": khoảng lặp 0,1 đến 9999 giây; nhấn F11 để bắt đầu và nhấn lại để dừng — [Screen capture](https://www.bandicam.com/guide/screen-capture/)
- Bandicam: Ctrl+C trong khung quay để copy ảnh vào clipboard; thêm mũi tên/text/hộp/số hoặc blur trước khi chụp; "Add logo overlay" cho ảnh (opacity, vị trí); xem ảnh tại Home > Images — [Screen capture](https://www.bandicam.com/guide/screen-capture/)
- Bandicam KHÔNG hỗ trợ scrolling capture và OCR; trang chỉ sang sản phẩm khác là ZunShot — [Screen capture](https://www.bandicam.com/guide/screen-capture/)
- Bandicam "Show FPS in captured video/image" ở Game mode — [Settings General](https://www.bandicam.com/guide/settings-general/)
- Snagit capture: Scrolling capture, Time delay, preset capture kèm hotkey riêng, kích thước chính xác/snap vào cửa sổ, menu và object capture (object capture chỉ Windows), Time-lapse capture theo chu kỳ (Windows), Multiple-area capture (Windows), Printer capture (Windows), Step recorder — [Snagit features](https://www.techsmith.com/snagit/features/)
- Snagit editor: arrows, callouts, shapes, Step tool, stamps, magnify, capture info (OS, ngày, app), templates, Smart redact, blur, text recognition (sửa/copy text trong ảnh), Smart move, Simplify, sửa/xóa con trỏ trong ảnh tĩnh, cut out, borders, background remover, batch processing — [Snagit features](https://www.techsmith.com/snagit/features/)
- Snagit export: SNAGX, PNG, JPG, HEIF, TIFF, GIF, BMP, WebP, PDF, MP4; lấy frame bất kỳ của video làm ảnh — [Snagit features](https://www.techsmith.com/snagit/features/)
- CleanShot X: chụp area/window/fullscreen; scrolling capture hai chiều; self-timer; All-In-One (một hotkey, kích thước cố định, khóa tỉ lệ, nhớ vùng chọn trước); crosshair + magnifier; screen freeze; chụp cửa sổ với nền/padding/trong suốt, bóng đổ bật/tắt — [CleanShot X features](https://cleanshot.com/features)
- CleanShot X sau khi chụp: Quick Access Overlay (copy, lưu, annotate, kéo thả vào app khác); annotation gồm crop, pixelate, arrow 4 kiểu, blur, spotlight, counter, pencil, highlighter, rectangle, line, ellipse, text 7 style; ghép nhiều ảnh; Background tool (20 nền, padding, tỉ lệ, preset); pin ảnh nổi trên mọi cửa sổ (opacity, Lock Mode); OCR on-device 30+ ngôn ngữ và đọc QR; capture history lưu tối đa một tháng — [CleanShot X features](https://cleanshot.com/features)
- Snipping Tool: Rectangle, Window, Full screen, Freeform; delay 1–5 giây; Text actions (OCR cục bộ, copy text, Quick redact email/số điện thoại); pen, highlighter, shapes, emoji, crop; tự lưu vào thư mục Screenshots; Perfect Screenshot và Color picker chỉ có trên Copilot+ PC — [Microsoft Support](https://support.microsoft.com/en-us/windows/use-snipping-tool-to-capture-screenshots-00246869-1843-655f-f220-97299b865f6b)
- Snipping Tool hotkey: Win+Shift+S (ảnh), Win+Shift+R (video), PrtSc (toàn màn hình vào clipboard) — [Microsoft Support](https://support.microsoft.com/en-us/windows/use-snipping-tool-to-capture-screenshots-00246869-1843-655f-f220-97299b865f6b)
- Movavi Screen Recorder: scrolling screenshot cho nội dung dài hơn một màn hình — [Movavi Screen Recorder](https://www.movavi.com/screen-recorder/)
- Icecream (Windows): lưu JPG hoặc PNG, copy clipboard, hoặc upload server lấy link — [Icecream Screen Recorder](https://icecreamapps.com/Screen-Recorder/)
- ScreenPal: scrolling screenshot từ gói Deluxe trở lên — [ScreenPal plans](https://screenpal.com/plans)
- Loom: "unlimited screenshots" ngay ở gói Starter — [Loom pricing](https://www.loom.com/pricing)
- Action!: chụp nhiều screenshot bằng hotkey, lưu thẳng ra đĩa — [Mirillis Action!](https://mirillis.com/en/products/action.html)

### Inferences
- Bandicam yếu rõ ở screenshot: không WebP, không scrolling, không OCR, annotation chỉ trước khi chụp. Một recorder Windows mới có thể vượt dễ dàng bằng: PNG/JPG/WebP (+ BMP), copy clipboard tự động, annotate sau chụp, overlay thao tác nhanh kiểu Quick Access Overlay, pin ảnh, OCR cục bộ.
- Chụp lặp (0,1–9999 s) của Bandicam và Time-lapse của Snagit là tính năng "continuous capture" đáng sao chép.

### Gaps
- Không nguồn nào đã đọc nêu rõ hành vi **DPI-aware / native-resolution** (chụp theo pixel vật lý trên màn hình scale 125–200%) cho bất kỳ app nào; cần kiểm thử thực tế.
- Không xác minh được Bandicam có tùy chọn "show mouse cursor" trong ảnh chụp và âm shutter (trang /guide/settings-image/ chưa đọc).
- Định dạng screenshot của Action!, NVIDIA App (có hỗ trợ HDR/JXR hay không) chưa xác minh.

---

## 6. Mẫu đặt tên file mặc định là gì, có cho phép template tùy chỉnh với token ngày/giờ không?

### Takeaway
Bandicam không có template tự do kiểu token; nó cho chọn phần gốc (Target Name / Custom Prefix / None) cộng hậu tố Date/Time hoặc Sequence Number, kèm tự tạo thư mục con theo tên target, theo loại hoặc theo ngày YYYY-MM-DD. Snagit đặt tên theo ngữ cảnh (tiêu đề trang web/tên file được chụp) hoặc ngày giờ. Không tìm thấy app nào trong nhóm này công bố hệ token tự do đầy đủ.

### Cited Findings
- Bandicam "File naming settings": chọn gốc tên là Target Name, Custom Prefix, hoặc None; sau đó thêm Date/Time hoặc Sequence Number; áp dụng cho video, ảnh và audio — [Settings General](https://www.bandicam.com/guide/settings-general/)
- Bandicam "Autocreate output subfolder with name": "Same as recording target name" (ví dụ thư mục "WOW" cho WOW.exe), "'Videos'/'Images'/'Audios'", hoặc "Date format (YYYY-MM-DD)" — [Settings General](https://www.bandicam.com/guide/settings-general/)
- Bandicam hậu tố file phái sinh: "_exported" cho video đã edit/Smart Zoom, "_subtitle" cho video gắn phụ đề; luôn giữ file gốc — [Smart zoom](https://www.bandicam.com/guide/smart-zoom-in-out/), [Video to text](https://www.bandicam.com/guide/video-to-text/)
- Bandicam có đổi tên (F2), xóa (Del), chọn tất cả (Ctrl+A) ngay trong danh sách file của app; Home > Images / Videos liệt kê file đã quay — [Hotkeys](https://www.bandicam.com/guide/hotkeys/), [Screen capture](https://www.bandicam.com/guide/screen-capture/)
- Snagit (Windows): Capture Preferences có tùy chọn loại bỏ nguồn capture khỏi tên file; khi đó tên dùng ngày giờ chụp hoặc tên tùy chỉnh nếu đã đổi tên; mặc định Snagit đặt tên capture chia sẻ theo ngữ cảnh như tiêu đề trang web hoặc tên file được chụp — [Snagit Preferences (Windows)](https://www.techsmith.com/learn/tutorials/snagit/preferences/)
- Snagit Library: tìm theo metadata, lọc/sắp theo ngày, ứng dụng, trang web; tag tùy chỉnh; mặc định lưu định dạng riêng SNAGX (có hướng dẫn đổi mặc định sang định dạng khác) — [Snagit features](https://www.techsmith.com/snagit/features/), [TechSmith Support](https://support.techsmith.com/hc/en-us/articles/38927872972941-How-to-Default-to-a-File-Type-Other-Than-SNAGX)
- Xbox Game Bar: lưu MP4 vào thư mục Videos\Captures của người dùng; mở qua "See my captures" trong Capture widget — [Dell KB](https://www.dell.com/support/kbdoc/en-us/000103410/how-to-use-the-windows-10-game-bar-to-capture-video-from-applications)
- Snipping Tool: snip tự lưu vào thư mục Screenshots, đổi được trong Settings — [Microsoft Support](https://support.microsoft.com/en-us/windows/use-snipping-tool-to-capture-screenshots-00246869-1843-655f-f220-97299b865f6b)
- Icecream: video mặc định lưu ra Desktop, đổi tại Settings > Save To; có thể đặt dung lượng file tối đa — [Icecream Screen Recorder](https://icecreamapps.com/Screen-Recorder/)
- CleanShot X: capture history lọc theo loại, lưu tối đa một tháng — [CleanShot X features](https://cleanshot.com/features)
- Action!: có recording manager tích hợp để duyệt, xóa, export — [Mirillis Action!](https://mirillis.com/en/products/action.html)

### Inferences
- Vì Bandicam chỉ có tổ hợp cố định (gốc + Date/Time hoặc Sequence), một hệ template token tự do (ví dụ `{app}_{window}_{yyyy}-{MM}-{dd}_{HH}-{mm}-{ss}_{seq}`), có xem trước trực tiếp và kiểm tra ký tự không hợp lệ, là điểm vượt trội dễ làm.
- Ba kiểu thư mục con tự động của Bandicam (theo target, theo loại, theo ngày) là đủ cho đa số người dùng; nên giữ và cho kết hợp nhiều cấp.
- Mẫu "giữ file gốc + hậu tố cho bản phái sinh" là thói quen tốt cần sao chép.

### Gaps
- Chuỗi tên file mặc định chính xác của Bandicam (theo trí nhớ là dạng `bandicam YYYY-MM-DD HH-MM-SS-mmm.mp4`) **không xuất hiện trên trang đã đọc** — chưa xác minh.
- Tên file mặc định của Xbox Game Bar (thường là tiêu đề cửa sổ + ngày giờ) và của Snipping Tool recorder (thường là "Screen Recording" + ngày giờ) không được nguồn nào trong lần tìm kiếm này xác nhận — chưa xác minh.
- Không tìm thấy tài liệu file naming của Camtasia, Loom, Screen Studio, FocuSee, Movavi, Action!, NVIDIA App, Tella, ScreenPal, CleanShot X (trang features của CleanShot X không nhắc tới).

---

## 7. Bảng kiểm kê theo từng app còn lại: chế độ quay, định dạng/codec, âm thanh, chỉnh sửa, giá/giấy phép, giới hạn bản free

### Takeaway
Thị trường chia ba nhóm: recorder hiệu năng cho Windows (Bandicam, Action!, NVIDIA App, Game Bar) xuất MP4/AVI với hardware encoder; công cụ tài liệu/screenshot (Snagit, CleanShot X, Snipping Tool); và nhóm "video đẹp tự động" + cloud/AI (Screen Studio, FocuSee, Tella, Loom, ScreenPal, Camtasia). Hầu hết đã chuyển sang subscription; Bandicam và Action! là hai app Windows còn bán license vĩnh viễn.

### Cited Findings

**Camtasia (TechSmith, Windows/macOS)**
- Chỉ còn subscription trả theo năm, không perpetual, không gói tháng; tier Essentials $179.88, Create $249.00, Pro $499.00 mỗi user/năm; có free tier nhưng export bị watermark — [Capterra Loom vs Camtasia](https://www.capterra.com/compare/191187-203013/Loom-vs-Camtasia)
- Nguồn khác ghi Pro $599/năm, Starter $39/năm (có watermark), Business $198/user/năm — mâu thuẫn về giá tier Pro — [Guidde Camtasia pricing guide](https://www.guidde.com/knowledge-hub/camtasia-pricing-guide-2026); [Capterra Camtasia](https://www.capterra.com/p/203013/Camtasia/) ghi Pro 499
- Camtasia 2026.1.4 (tháng 7/2026) thêm import bản ghi Zoom, import/export caption VTT, collaborative recording — [Trainn Camtasia pricing](https://trainn.co/blog/camtasia-pricing/)
- TechSmith thông báo tăng giá license Legacy của Snagit và Camtasia từ 11/06/2026 — [TechSmith Support](https://support.techsmith.com/hc/en-us/articles/45828582740493-An-Update-on-Your-TechSmith-Legacy-License-Subscription)

**Snagit (TechSmith, Windows/macOS)**
- Quay video với các lớp screen, camera, audio độc lập; webcam picture-in-picture (đổi kích thước/hình dạng, chuyển giữa screen và webcam toàn màn hình); micro + system audio; gộp clip, trim; tạo GIF từ video; tạo video từ ảnh kèm lời thuyết minh; khử nhiễu nền audio — [Snagit features](https://www.techsmith.com/snagit/features/)
- Chia sẻ: Outlook, Teams, OneDrive, PowerPoint, Word, Dropbox, Google Drive, link Screencast (bình luận, reaction, collection) — [Snagit features](https://www.techsmith.com/snagit/features/)
- Giá (nguồn bên thứ ba): Individual $39/năm, Business $48/năm, chỉ trả theo năm; Snagit 2026 chuyển sang subscription, không còn bán perpetual — [Trupeer Snagit pricing](https://www.trupeer.ai/tools-comparison/snagit-vs-trupeer-pricing), [Software-Express](https://www.software-express.de/hersteller/techsmith/snagit/)

**Loom (Atlassian, cloud + desktop app)**
- Starter $0: 25 video/người, 5 phút mỗi video, tối đa 720p; screen + cam bubble, system audio, recording canvas, camera frames, virtual background, noise suppression, speaker notes; không liệt kê download video, trim & stitch, AI — [Loom pricing](https://www.loom.com/pricing)
- Business $18/user/tháng: không giới hạn video và thời lượng, tới 4K, trim & stitch, upload/download, bỏ branding Loom, drawing tool, blur, custom recording dimensions, password — [Loom pricing](https://www.loom.com/pricing)
- Business + AI $24/user/tháng: toàn bộ Loom AI; Enterprise: SSO/SCIM, SLA 99,95% — [Loom pricing](https://www.loom.com/pricing)

**Screen Studio (chỉ macOS — tham khảo ý tưởng)**
- Export MP4 720p/1080p/4K tới 60 fps; GIF lặp tới 1080p; tỉ lệ wide/square/vertical, Reel mode preview; share link phát 1080p60 cho video tới 30 phút, có bình luận gắn timestamp — [Screen Studio](https://screen.studio/)
- Quay iPhone/iPad qua USB với device mockup; pause/resume, speaker notes, countdown, **crash recovery**, resize cửa sổ, ẩn desktop icon; cắt, trim, tăng tốc, crop, tăng tốc đoạn gõ phím, mask dữ liệu nhạy cảm; preset giao diện (background, shadow, inset) — [Screen Studio](https://screen.studio/)
- Giá: $29/tháng hoặc $9/tháng trả theo năm; mọi tính năng ở mọi gói; giảm 40% cho học sinh/giáo viên; không còn bán one-time license cho khách mới — [Screen Studio](https://screen.studio/)

**FocuSee (iMobie, Windows + macOS)**
- Quay full screen, cửa sổ hoặc vùng tùy chọn, kèm webcam và system audio trong một lần; quay màn hình iOS/Android qua USB; import video có sẵn; lưu preset — [FocuSee](https://focusee.ai/)
- Export tới 4K 60fps hoặc GIF; đổi tỉ lệ cho TikTok/YouTube/Instagram/Facebook/Twitter; share link có mật khẩu và phân quyền — [FocuSee](https://focusee.ai/)
- Free trial: dùng tính năng chuẩn và export **một** video tới 4K miễn phí; AI nâng cao cần subscription và credit; hoàn tiền 60 ngày — [FocuSee](https://focusee.ai/)

**Movavi Screen Recorder (Windows/macOS)**
- Quay full screen hoặc vùng tùy chọn; screen + system audio + micro + webcam cùng lúc hoặc tách riêng; 4K; lịch quay với ngày giờ bắt đầu/kết thúc; vẽ real-time — [Movavi Screen Recorder](https://www.movavi.com/screen-recorder/)
- Trial 7 ngày, watermark trên video; không hỗ trợ nội dung có bảo vệ bản quyền; không dành cho game (dùng Gecata) — [Movavi Screen Recorder](https://www.movavi.com/screen-recorder/)

**Icecream Screen Recorder (Windows/macOS/Android)**
- Full screen (chọn display), vùng chọn, đa màn hình (không quay hai màn hình cùng lúc); lịch quay; system audio + micro; webcam overlay đổi hình dạng/kích thước; watermark/logo riêng; trim, đổi tốc độ, mute, đổi độ phân giải/định dạng; upload lên server Icecream lấy link và theo dõi lượt xem — [Icecream Screen Recorder](https://icecreamapps.com/Screen-Recorder/)
- Bản free (nguồn review bên thứ ba): giới hạn 5 phút (một nguồn ghi 10 phút), chỉ xuất WEBM, có watermark; PRO thêm MP4/MKV, lịch quay, dùng thương mại; giá được ghi $29.95/năm hoặc $59.95 trọn đời (các nguồn không thống nhất) — [AnyRec review](https://www.anyrec.io/?p=1777), [Wondershare DemoCreator review](https://democreator.wondershare.com/screen-recorder/icecream-screen-recorder.html)

**Action! (Mirillis, Windows)**
- Chế độ: game DirectX 8–12/OpenGL/Vulkan; desktop và ứng dụng; "Active screen" (ghi liên tục vào một file khi chuyển giữa desktop và game fullscreen); "Selected Application" (chỉ ghi một app, bôi đen phần còn lại); device (console, PC khác, webcam, capture card USB/HDMI); vùng màn hình; audio-only — [Mirillis Action!](https://mirillis.com/en/products/action.html)
- Codec/định dạng: codec riêng FICV (AVI) và MP4 H.264/AVC với NVIDIA NVENC, AMD APP, Intel Quick Sync; 2K/3K/4K một màn hình; trên 8K cho đa màn hình (chỉ AVI); audio M4A; 120 fps cho slow motion — [Mirillis Action!](https://mirillis.com/en/products/action.html)
- Time-Shift (lưu đoạn gameplay vừa qua bằng một hotkey); Greenscreen mode cho webcam; live streaming Twitch/YouTube/Facebook...; HUD FPS + benchmark; file manager và player tích hợp; app Android Action! RCU điều khiển từ xa — [Mirillis Action!](https://mirillis.com/en/products/action.html)
- Giấy phép trọn đời theo serial; dùng thử 30 ngày — [Mirillis Action!](https://mirillis.com/en/products/action.html)

**Xbox Game Bar / Windows Snipping Tool (có sẵn, miễn phí)**
- Game Bar: xuất MP4 vào Videos\Captures; không quay được desktop/File Explorer, chỉ hoạt động khi game hoặc cửa sổ app được hỗ trợ đang focus — [Dell KB](https://www.dell.com/support/kbdoc/en-us/000103410/how-to-use-the-windows-10-game-bar-to-capture-video-from-applications), [Appuals](https://appuals.com/gaming-features-arent-available/)
- Snipping Tool: quay vùng chữ nhật bằng Win+Shift+R; lưu nguyên trạng hoặc mở Clipchamp để sửa và thêm caption tự động — [Microsoft Support](https://support.microsoft.com/en-us/windows/use-snipping-tool-to-capture-screenshots-00246869-1843-655f-f220-97299b865f6b)

**NVIDIA App / ShadowPlay (miễn phí, yêu cầu GPU NVIDIA)**
- Instant Replay kiểu DVR, độ dài buffer chỉnh được ("Instant Replay length"); hỗ trợ encode AV1 SDR và HDR; NVIDIA nói AV1 hiệu quả hơn H.264 40% trên RTX 40 Series — [NVIDIA App AV1 update](https://www.nvidia.com/en-eu/geforce/news/nvidia-app-beta-update-av1-performance-tuning/)
- Giới hạn độ phân giải: một trang NVIDIA ghi tới 8K HDR 30 fps hoặc 4K HDR 120 fps — [NVIDIA ShadowPlay](https://www.nvidia.com/ShadowPlay); bản tin khác ghi tới 4K 120 FPS hoặc 8K 60 FPS với AV1 — [NVIDIA App AV1 update](https://www.nvidia.com/en-eu/geforce/news/nvidia-app-beta-update-av1-performance-tuning/) (hai nguồn không khớp)
- Filter RTX HDR và RTX Dynamic Vibrance, hoạt động trên hơn 1200 game — [NVIDIA App release](https://www.nvidia.com/en-eu/geforce/news/nvidia-app-download-and-features/)

**CleanShot X (chỉ macOS — tham khảo ý tưởng)**
- Quay cửa sổ, vùng hoặc fullscreen ra MP4 (H.264) hoặc GIF; chỉnh quality, FPS, độ phân giải; micro + computer audio; webcam (vị trí, kích thước, hình dạng, fullscreen); hiện thời gian quay trên menu bar — [CleanShot X features](https://cleanshot.com/features)
- Cloud: upload một click, video 4K, transcription, bình luận nhóm, mật khẩu, link hết hạn/tự hủy; hoàn tiền 30 ngày — [CleanShot X features](https://cleanshot.com/features)

**Tella (web + app macOS/Windows)**
- Pro $13/user/tháng (trả năm): video không giới hạn, export 4K, 106 ngôn ngữ transcription; Premium $19/user/tháng: bỏ branding Tella, export 60 FPS, custom domain; trial 7 ngày; có native Windows recorder — [Tella pricing](https://www.tella.com/pricing)

**ScreenPal (trước là Screencast-O-Matic)**
- Free: 15 phút mỗi lần quay, có watermark, không quay system audio, 720p, 10 video lưu trữ; Deluxe $4/tháng (trả năm): không giới hạn thời lượng, full editor, caption speech-to-text, scrolling screenshot; Max $10/tháng: 4K, AI titles/summaries/chapters, AI text-to-speech; Team Business $8/user/tháng (3+ user) — [ScreenPal plans](https://screenpal.com/plans)

### Inferences
- Bảng định vị cho HP Screen: nếu là recorder Windows nhẹ, bán một lần, không giới hạn thời lượng, có auto-zoom + cursor effect + transcription cục bộ thì sẽ chiếm khoảng trống giữa Bandicam (nhẹ, hiệu năng, hiệu ứng hạn chế) và FocuSee/Screen Studio (đẹp nhưng subscription/credit, nặng hậu kỳ).
- Các tính năng "tiện ích" đáng sao chép vì ít đối thủ Windows có: crash recovery (Screen Studio), Time-Shift/Instant Replay cho desktop (Action!, NVIDIA), Active screen ghi liên tục khi chuyển desktop/game (Action!), Auto-Pause khi không hoạt động và Auto Complete theo im lặng/dung lượng (Bandicam), tự bật Do Not Disturb và ẩn desktop icon (CleanShot X, Screen Studio), ghi âm riêng theo cửa sổ (Bandicam), lưu track audio riêng (Bandicam).
- Hầu hết app AI tính tiền theo gói cao hoặc credit (Tella, FocuSee, Loom, ScreenPal); AI chạy cục bộ không tính phí là khác biệt bán hàng.

### Gaps
- Camtasia: không lấy được danh sách tính năng chính thức (trang pricing trả 404); chi tiết recorder (Camtasia Rev, cursor path editing, định dạng .trec) chưa xác minh trong lần này; giá tier Pro mâu thuẫn ($499 so với $599).
- Giá chính thức của Snagit, Movavi, Icecream, Action!, FocuSee, CleanShot X không lấy được từ trang gốc (trang không hiển thị giá hoặc không đọc được).
- Xbox Game Bar: các chi tiết như ghi nền "Record what happened", giới hạn thời lượng tối đa, codec/bitrate, tách track audio chưa xác minh. Snipping Tool: nguồn âm thanh (micro/system audio), export GIF, trim tích hợp không có trên trang Microsoft đã đọc — chưa xác minh.
- NVIDIA App: cơ chế Highlights, yêu cầu GPU tối thiểu cho AV1 (một bài bên thứ ba ghi RTX 30 Series, mâu thuẫn với cách NVIDIA nhắc RTX 40 Series), khả năng quay desktop và hotkey mặc định chưa xác minh đầy đủ.
- Action!: hỗ trợ HEVC/HDR không được nhắc trên trang đã đọc — chưa xác minh; hạn chế của bản trial ngoài 30 ngày (watermark) chưa xác minh.
- Định dạng đầu ra của Movavi, FocuSee (ngoài MP4/GIF suy ra), Loom download chưa xác minh.
