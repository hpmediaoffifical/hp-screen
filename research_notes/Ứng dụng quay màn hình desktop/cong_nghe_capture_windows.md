# Công nghệ lõi để xây dựng trình quay màn hình trên Windows 10/11 (trạng thái cuối 2026)

> Ghi chú về phương pháp: mọi mục trong "Cited Findings" đều lấy từ trang đã được đọc trực tiếp trong phiên nghiên cứu này (11/2026). Những gì là kiến thức nền của người nghiên cứu nhưng CHƯA được kiểm chứng lại bằng nguồn trong phiên này được đặt trong "Inferences" với nhãn **[chưa kiểm chứng]** hoặc trong "Gaps". Hai trang không tải được: `ffmpeg.org/legal.html` (ECONNRESET, 2 lần) và `trac.ffmpeg.org/wiki/Encode/H.264` (bị chặn bot).

---

## 1. Capture APIs: WGC, DXGI Desktop Duplication, GDI/BitBlt, PrintWindow, DWM thumbnails, game hooks

### Takeaway
Windows.Graphics.Capture (WGC) là API duy nhất được Microsoft thiết kế để lấy nội dung riêng của một cửa sổ (kể cả khi bị cửa sổ khác che) dưới dạng texture D3D11, và cũng capture được cả monitor; đây nên là đường capture chính của HP Screen. DXGI Desktop Duplication chỉ capture theo từng output (monitor) nên thấy mọi thứ đang hiển thị (kể cả cửa sổ che), phù hợp làm fallback cho fullscreen/region. Các tính năng "dễ chịu" của WGC (tắt viền vàng, tắt con trỏ, giới hạn tần suất) có yêu cầu build khác nhau, nên cần kiểm tra runtime bằng `ApiInformation.IsPropertyPresent`.

### Cited Findings

**Windows.Graphics.Capture (WGC)**
- `GraphicsCaptureSession` có từ Windows 10 version 1803 (build 10.0.17134, UniversalApiContract v6.0). — [Microsoft Learn: GraphicsCaptureSession](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession)
- Bảng version history chính thức chỉ ghi một dòng: `IsCursorCaptureEnabled` được thêm ở Windows 10 version 2004 (SDK 19041). — [Microsoft Learn: GraphicsCaptureSession](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession)
- Các property hiện có của session: `IsBorderRequired`, `IsCursorCaptureEnabled`, `MinUpdateInterval`, `DirtyRegionMode`, `IncludeSecondaryWindows`, `ConfigurationIteration`; phương thức `IsSupported()`, `StartCapture()`, `Close()`. Trang lớp KHÔNG ghi build tối thiểu cho `MinUpdateInterval`, `DirtyRegionMode`, `IncludeSecondaryWindows`. — [Microsoft Learn: GraphicsCaptureSession](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession)
- `IsBorderRequired`: "Windows 10, version 2104 (introduced in 10.0.20348.0)", UniversalApiContract v12.0. Trước khi hệ thống tắt viền, app phải xin đồng ý của người dùng bằng `GraphicsCaptureAccess.RequestAccessAsync(GraphicsCaptureAccessKind.Borderless)`; để gọi được phải khai báo capability `graphicsCaptureWithoutBorder` trong package manifest. Nếu người dùng từ chối, gán `false` vẫn "thành công" nhưng bị bỏ qua và viền vẫn hiện. Nếu một app khác đang capture cùng cửa sổ/màn hình với `IsBorderRequired = true` thì viền vẫn hiện. — [Microsoft Learn: IsBorderRequired](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession.isborderrequired)
- Mặc định hệ thống vẽ viền thông báo màu vàng quanh item đang bị capture; nhiều session đồng thời thì mỗi item một viền. — [Microsoft Learn: Screen capture](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture)
- `MinUpdateInterval` gắn với SDK 26100 một cách không chính thức: issue trên repo mẫu của robmikh nói gần như không có tài liệu cho các API mới của 26100 nhưng sample dùng `MinUpdateInterval` để giới hạn tần suất capture. — [robmikh/Win32CaptureSample issue #82](https://github.com/robmikh/Win32CaptureSample/issues/82)
- Thư viện Rust `wgc` kiểm tra từng property bằng `ApiInformation::IsPropertyPresent` trước khi dùng (cách làm nên bắt chước). — [docs.rs: wgc capabilities.rs](https://docs.rs/crate/wgc/latest/source/src/capabilities.rs)
- Tạo capture item không cần picker: dùng `IGraphicsCaptureItemInterop` (`CreateForWindow` với HWND, `CreateForMonitor` với HMONITOR). Sample chạy được từ build 17134; build sample cần Windows 11 SDK 10.0.26100. — [robmikh/Win32CaptureSample](https://github.com/robmikh/Win32CaptureSample)
- `Direct3D11CaptureFramePool.Create` cần `DispatcherQueue` trên thread hiện tại và thread phải pump message (`FrameArrived` bắn trên thread tạo); `CreateFreeThreaded` không cần `DispatcherQueue`, `FrameArrived` bắn trên thread nội bộ. Chờ đồng bộ một snapshot thì phải dùng `CreateFreeThreaded` kẻo deadlock. — [robmikh/Win32CaptureSample](https://github.com/robmikh/Win32CaptureSample)
- Pixel format mặc định `DXGI_FORMAT_B8G8R8A8_UNORM`. Khi bật Windows HD Color (HDR), nội dung có thể không phải BGRA8; để tránh ảnh bị "washed out"/clipping nên dùng `DXGI_FORMAT_R16G16B16A16_FLOAT` xuyên suốt pipeline, sau đó tự tone-map HDR sang SDR hoặc lưu định dạng HDR. — [Microsoft Learn: Screen capture](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture)
- Mỗi frame (`Direct3D11CaptureFrame`) có `ContentSize`, `Surface`, `SystemRelativeTime`; `SystemRelativeTime` là thời gian QPC dùng để đồng bộ với media khác. — [Microsoft Learn: Screen capture](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture)
- Surface luôn có kích thước đã khai báo khi tạo frame pool; nội dung lớn hơn thì bị cắt, nhỏ hơn thì phần dư là dữ liệu không xác định, nên phải copy sub-rect theo `ContentSize`. Khi cửa sổ đổi kích thước hoặc mất device, gọi `Direct3D11CaptureFramePool.Recreate` (mọi frame đang có bị huỷ). — [Microsoft Learn: Screen capture](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture)
- Một `GraphicsCaptureItem` có thể gắn với nhiều capture session. — [Microsoft Learn: Screen capture](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture)
- OBS gọi phương thức WGC cho Window Capture là "Windows 10 (1903 and up)". — [OBS forum: prefer "Windows 10" capture method](https://obsproject.com/forum/threads/please-prefer-using-windows-10-capture-method-reather-than-bitblt-when-using-1903-or-newer-windows-version.186837)
- PR thêm WGC vào OBS ghi rằng OBS xử lý êm việc không nạp được module WinRT trên Windows 7 và Windows 10 1809. — [obsproject/obs-studio PR #2208](https://github.com/obsproject/obs-studio/pull/2208)
- Báo cáo người dùng (không phải tài liệu chính thức): BitBlt không capture được cửa sổ minimized; với WGC cửa sổ minimized đôi khi giữ frame vẽ cuối nhưng không đáng tin cậy; WGC vẫn lấy được nội dung cửa sổ bị che, BitBlt đôi khi hỏng với cửa sổ bị che. — [obs-versions.com: window capture guide (bên thứ ba)](https://obs-versions.com/blog/obs-window-capture-guide); [OBS forum](https://obsproject.com/forum/threads/for-capture-method-whats-the-difference-between-bitblt-and-windows-graphics-capture.127687/)
- Chế độ "Automatic" của OBS không luôn chọn đúng: issue #13829 (OBS 32.2.2) báo Automatic chọn BitBlt cho một game và chỉ ra 1 frame duy nhất trong 25 lần capture, còn WGC cập nhật bình thường; issue #13359 báo BitBlt không tương thích với app WebView2/Chromium. — [OBS issue #13829 (mirror)](https://obs-versions.com/community/issues/13829); [OBS issue #13359 (mirror)](https://obs-versions.com/community/issues/13359)
- Có báo cáo WGC window capture ra hình trống trên một số máy Windows 11 trong khi BitBlt chạy được, tức WGC không phải thuốc chữa bách bệnh. — [OBS forum: WGC not working on Windows 11](https://obsproject.com/forum/threads/window-capture-not-working-using-wgc-method-on-windows-11.193528/latest)

**DXGI Desktop Duplication (DDA)**
- Có từ Windows 8, thay thế mirror driver XDDM; trả ảnh desktop dạng DXGI surface để xử lý trên GPU. — [Microsoft Learn: Desktop Duplication API](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api)
- `IDXGIOutputDuplication::AcquireNextFrame` luôn trả `DXGI_FORMAT_B8G8R8A8_UNORM` bất kể display mode. — [Microsoft Learn: Desktop Duplication API](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api)
- Cung cấp dirty rects (`GetFrameDirtyRects`) và move rects (`GetFrameMoveRects`); phải xử lý move trước, dirty sau. — [Microsoft Learn: Desktop Duplication API](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api)
- Màn hình xoay: surface luôn ở hướng chưa xoay (ví dụ desktop 768x1024 xoay 90 độ trả surface 1024x768), app phải tự xoay; đa màn hình thì mỗi monitor xoay độc lập. — [Microsoft Learn: Desktop Duplication API](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api)
- Con trỏ chuột: có thể đã vẽ sẵn vào ảnh hoặc tách rời (overlay phần cứng); khi tách rời app phải tự vẽ, dùng `PointerPosition` trong `DXGI_OUTDUPL_FRAME_INFO` và `GetFramePointerShape` (chỉ gọi lại khi hình dạng thay đổi). Điều này cho phép tự do bật/tắt con trỏ hoặc vẽ con trỏ tuỳ biến. — [Microsoft Learn: Desktop Duplication API](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api)
- Lỗi cần xử lý trong vòng lặp: `DXGI_ERROR_ACCESS_LOST` (phải tạo lại duplication) và `DXGI_ERROR_WAIT_TIMEOUT` (không có frame mới). — [Microsoft Learn: Desktop Duplication API](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api)
- Thư viện `windows-capture` cảnh báo vài frame đầu của DDA có thể rỗng (kiểm tra `LastPresentTime`). — [NiiightmareXD/windows-capture](https://github.com/NiiightmareXD/windows-capture)

**GDI / BitBlt**
- Cách cổ điển: `GetDC(NULL)` lấy DC màn hình, `CreateCompatibleDC` + `CreateCompatibleBitmap`, rồi `BitBlt`/`StretchBlt` copy sang memory DC, `GetDIBits` lấy pixel. Hoàn toàn trên CPU/GDI. — [Microsoft Learn: Capturing an Image](https://learn.microsoft.com/en-us/windows/win32/gdi/capturing-an-image)

**Loại trừ cửa sổ khỏi capture / nội dung được bảo vệ**
- `SetWindowDisplayAffinity`: `WDA_MONITOR` làm cửa sổ hiện ra không có nội dung ở mọi nơi ngoài monitor; `WDA_EXCLUDEFROMCAPTURE` (từ Windows 10 version 2004) làm cửa sổ hoàn toàn không xuất hiện trong capture; Microsoft nêu đúng use case "cửa sổ điều khiển quay video để nó không lọt vào bản ghi". Trên Windows cũ hơn 2004, giá trị này hành xử như `WDA_MONITOR`. Chỉ áp dụng cho top-level window thuộc process hiện tại, và chỉ khi DWM đang compose. — [Microsoft Learn: SetWindowDisplayAffinity](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity)
- Microsoft nhấn mạnh display affinity không phải là tính năng bảo mật hay DRM. — [Microsoft Learn: SetWindowDisplayAffinity](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity)

### Inferences
- **Trả lời câu hỏi chính:** để quay một cửa sổ "sạch" (không dính cửa sổ khác che lên) dùng WGC với `CreateForWindow(HWND)`. DDA và BitBlt từ DC màn hình đều chụp kết quả compose cuối cùng của monitor nên sẽ dính cửa sổ che.
- `IsBorderRequired` ghi build 20348; build này trên client tương ứng với Windows 11 trở lên (Windows 10 client dừng ở 1904x), nên trên Windows 10 viền vàng nhiều khả năng không tắt được **[chưa kiểm chứng trong phiên này]**. HP Screen nên thiết kế chấp nhận viền trên Windows 10 hoặc dùng DDA cho fullscreen/region trên Windows 10 (DDA không có viền).
- Tài liệu nói phải khai báo `graphicsCaptureWithoutBorder` trong package manifest. Hành vi với app Win32 không đóng gói (unpackaged) không được tài liệu nêu; kinh nghiệm cộng đồng là unpackaged app chỉ cần gán `IsBorderRequired = false` trên Windows 11 **[chưa kiểm chứng]**. Cần thử nghiệm thực tế sớm.
- Dùng `WDA_EXCLUDEFROMCAPTURE` cho thanh điều khiển/overlay đếm ngược của HP Screen để chúng không lọt vào video (yêu cầu Windows 10 2004+).
- `SystemRelativeTime` (QPC) của WGC + timestamp QPC của WASAPI và của input hook cho phép dùng chung một đồng hồ cho video, audio và sự kiện click.
- HDR: nếu người dùng bật HDR, pipeline BGRA8 có thể cho màu sai; tối thiểu cần phát hiện HDR và capture `R16G16B16A16_FLOAT` rồi tone-map về SDR. DDA cơ bản (`DuplicateOutput`) luôn trả BGRA8; biến thể `DuplicateOutput1` (IDXGIOutput5) cho phép nhận format HDR **[chưa kiểm chứng trong phiên này]**.
- PrintWindow (`PW_RENDERFULLCONTENT`), DWM thumbnails (`DwmRegisterThumbnail`) và game hook (inject DLL hook Present như OBS Game Capture): không tìm được nguồn trong phiên này. Kiến thức nền **[chưa kiểm chứng]**: PrintWindow với cờ `PW_RENDERFULLCONTENT` (Windows 8.1+) chụp được cửa sổ bị che, phù hợp screenshot tĩnh nhưng quá chậm cho video; DWM thumbnail chỉ vẽ được vào cửa sổ của chính mình, không trả pixel; game hook cho hiệu năng tốt nhất với game exclusive fullscreen nhưng bị anti-cheat soi và rất tốn công bảo trì. Với một recorder mới, không nên tự làm game hook.

### Gaps
- Build tối thiểu chính thức của `MinUpdateInterval`, `DirtyRegionMode`, `IncludeSecondaryWindows`: không có trong trang lớp; cần xem trang từng member hoặc header SDK, hoặc kiểm tra runtime.
- Không tìm được nguồn chính thức về: hành vi WGC với cửa sổ minimized (chỉ có báo cáo người dùng), với nội dung DRM (thường ra vùng đen), và với game exclusive fullscreen.
- Giới hạn về per-monitor DPI awareness đối với DDA/WGC không có trong các trang đã đọc (xem thêm mục Screenshots).
- Không đọc được blog/ghi chú của robmikh ngoài README của Win32CaptureSample.

---

## 2. Region capture và kích thước frame chẵn cho H.264

### Takeaway
Không có API nào capture trực tiếp "một vùng tuỳ ý"; cách chuẩn là capture cả monitor (WGC `CreateForMonitor` hoặc DDA) rồi cắt sub-rect trên GPU trước khi đưa vào encoder. Kích thước vùng nên được làm tròn về số chẵn trước khi encode 4:2:0.

### Cited Findings
- WGC yêu cầu app tự copy sub-rect từ surface (ít nhất theo `ContentSize`), tức việc cắt vùng trên texture là thao tác thông thường trong pipeline. — [Microsoft Learn: Screen capture](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture)
- DDA trả surface của nguyên một output, kèm toạ độ desktop của output (`OutputDesc.DesktopCoordinates`) để quy đổi toạ độ. — [Microsoft Learn: Desktop Duplication API](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api)
- Một pipeline Rust thực tế (crate `win-native-media`, chưa kiểm chứng kỹ vì build docs.rs lỗi) mô tả: WGC capture, chuyển màu trên GPU bằng D3D11 Video Processor, encode H.264 bằng Media Foundation MFT phần cứng có fallback phần mềm, WASAPI + AAC. — [docs.rs: win-native-media](https://docs.rs/win-native-media)
- YouTube khuyến nghị H.264 High Profile, 4:2:0, progressive cho video upload, tức 4:2:0 là đích phổ biến nhất. — [YouTube Help: Recommended upload encoding settings](https://support.google.com/youtube/answer/1722171?hl=en)

### Inferences
- Cách làm cụ thể **[kiến thức nền, chưa kiểm chứng bằng nguồn]**: dùng `ID3D11DeviceContext::CopySubresourceRegion` với `D3D11_BOX` để cắt vùng sang texture đích, rồi chuyển BGRA sang NV12 bằng D3D11 Video Processor hoặc shader; không readback về CPU.
- Vì 4:2:0 lấy mẫu chroma 2x2, chiều rộng và cao phải chẵn: làm tròn xuống `w &= ~1; h &= ~1` (mất tối đa 1 pixel ở mép) hoặc pad thêm 1 pixel. Một số encoder phần cứng còn yêu cầu căn 16 nội bộ nhưng tự xử lý bằng cropping metadata **[chưa kiểm chứng]**.
- Vùng chọn trải qua hai monitor cần ghép từ hai nguồn capture (hai texture) vào một canvas; đơn giản nhất cho phiên bản đầu là giới hạn vùng trong một monitor.
- Toạ độ vùng chọn phải ở pixel vật lý (process per-monitor DPI aware v2) để khớp với texture capture.
- Quay region bằng WGC monitor trên Windows 11 có thể tắt viền; trên Windows 10 viền vàng nằm ở mép monitor nên thường nằm ngoài vùng cắt, trừ khi vùng chạm mép.

### Gaps
- Không tìm được tài liệu chính thức nào mô tả "region capture" như một quy trình; phần trên là thực hành chung của ngành.
- Không kiểm chứng được yêu cầu căn chỉnh kích thước cụ thể của từng encoder MFT (NVIDIA/Intel/AMD).

---

## 3. Encoding: Media Foundation vs FFmpeg vs libobs; cấu hình cho nội dung màn hình; an toàn khi crash; định dạng; giấy phép

### Takeaway
MP4 thường (moov ghi ở cuối) sẽ hỏng toàn bộ nếu app crash; OBS khuyến nghị MKV rồi remux, và từ OBS 30.2 có "Hybrid MP4" (ghi dạng fragmented rồi "soft-remux" khi kết thúc). HP Screen muốn mặc định MP4 thì nên ghi fragmented MP4 (hoặc MKV tạm) rồi finalize/remux khi dừng. Về chất lượng, dùng rate control theo chất lượng (CRF/CQP) thay vì bitrate cố định; số liệu bitrate tham chiếu công khai duy nhất tìm được là bảng của YouTube.

### Cited Findings

**Container và an toàn khi crash**
- OBS khuyến nghị MKV làm định dạng ghi vì dừng đột ngột không làm hỏng toàn bộ file; muốn MP4 thì dùng File > Remux Recordings hoặc bật "Automatically remux to mp4". — [OBS KB: Standard Recording Output Guide](https://obsproject.com/kb/standard-recording-output-guide)
- Với MP4 thường, moov atom chỉ được ghi sau khi dừng quay; nếu phần mềm không kết thúc đúng cách thì thiếu moov và toàn bộ file không dùng được. — [Elgato: OBS Studio 30.2 update](https://www.elgato.com/kr/en/explorer/news/streaming/obs-studio-30-2-update/)
- Hybrid MP4 ra mắt ở OBS 30.2, Hybrid MOV ở OBS 32.0. File được ghi ở dạng fragmented, sau đó finalize bằng "soft-remux" để phần mềm đọc như MP4 thường; file vẫn khôi phục được nếu việc ghi bị cắt ngang (crash, mất điện). Chapter marker chỉ ghi lúc finalize nên mất nếu crash. — [OBS KB: Hybrid MP4](https://obsproject.com/kb/hybrid-mp4)
- Codec trong Hybrid MP4: video H.264, H.265, AV1 (AV1 chỉ MP4; ProRes chỉ MOV); audio AAC, ALAC, Opus, FLAC (Opus/FLAC chỉ MP4); PCM trong MP4 là bổ sung mới của chuẩn và không phải ứng dụng nào cũng đọc được. — [OBS KB: Hybrid MP4](https://obsproject.com/kb/hybrid-mp4)
- Một trang bên thứ ba báo kết quả thử crash với OBS 30.2.4: MKV 14/14, Hybrid MP4 13/14, MP4 thường 0/14 (không kiểm chứng độc lập được). — [obs-versions.com: MKV vs MP4 vs Hybrid MP4 (bên thứ ba)](https://obs-versions.com/blog/obs-mkv-vs-mp4-hybrid-mp4)
- Các nguồn bên thứ ba nói Hybrid MP4 là mặc định cho profile mới từ OBS 32.0; trang KB chính thức của OBS không xác nhận trạng thái mặc định. — [obs-versions.com (bên thứ ba)](https://obs-versions.com/blog/obs-mkv-vs-mp4-hybrid-mp4); không được xác nhận bởi [OBS KB: Hybrid MP4](https://obsproject.com/kb/hybrid-mp4)

**Rate control và bitrate**
- OBS: dùng encoder riêng cho recording cho phép đặt bitrate, preset chất lượng, keyframe interval; nên thử các encoder theo phần cứng (ví dụ của trang dùng NVIDIA NVENC). Trang KB này không đưa con số CQP/CRF cụ thể. — [OBS KB: Standard Recording Output Guide](https://obsproject.com/kb/standard-recording-output-guide)
- CQP được dùng cho ghi hình "không phân biệt được" hoặc lossless; hỗ trợ 4:2:0/4:2:2/4:4:4 tuỳ phần cứng. — [StreamFX wiki: Encoder FFmpeg NVENC](https://github.com/Vhonowslend/StreamFX-Public/wiki/Encoder-FFmpeg-NVENC)
- Khuyến nghị của YouTube cho upload SDR (dùng làm mốc bitrate tham chiếu): 1080p 8 Mbps (24-30 fps) / 12 Mbps (48-60 fps); 1440p 16 / 24 Mbps; 2160p 35-45 / 53-68 Mbps. Container MP4 có moov ở đầu file (fast start), không edit list; H.264 High Profile, progressive, 2 B-frame liên tiếp, closed GOP dài bằng nửa frame rate, CABAC, 4:2:0, VBR. Audio AAC-LC hoặc Opus, 48 kHz, stereo 384 kbps. — [YouTube Help: Recommended upload encoding settings](https://support.google.com/youtube/answer/1722171?hl=en)

**Thư viện encode sẵn có**
- ScreenRecorderLib (.NET) dùng Media Foundation gốc của Microsoft để encode realtime ra H.264 hoặc ảnh PNG; yêu cầu Windows 8+, Visual C++ Redistributable và Media Foundation (Windows N/KN cần "Media Feature Pack"). — [sskodje/ScreenRecorderLib](https://github.com/sskodje/ScreenRecorderLib)
- `windows-capture` (Rust) có `VideoEncoder` tăng tốc phần cứng với "stable audio timing", ghi ra file hoặc stream trong bộ nhớ. — [NiiightmareXD/windows-capture](https://github.com/NiiightmareXD/windows-capture)

### Inferences
- **Lựa chọn encoder [kiến thức nền, chưa kiểm chứng bằng nguồn trong phiên này]:**
  - *Media Foundation Sink Writer*: có sẵn trong Windows, không tăng kích thước installer, không vướng giấy phép FFmpeg, dùng được MFT phần cứng (NVENC/QSV/AMF) và nhận trực tiếp texture D3D11. Nhược: ít tuỳ chọn tinh chỉnh, chất lượng encoder phần mềm của Microsoft kém x264, MP4 sink ghi moov ở cuối (không an toàn khi crash trừ khi dùng fragmented MP4 sink), thiếu trên Windows N nếu chưa cài Media Feature Pack.
  - *FFmpeg (libav\* hoặc gọi ffmpeg.exe qua pipe)*: linh hoạt nhất (libx264, h264_nvenc, h264_qsv, h264_amf, hevc_*, av1_nvenc/av1_qsv/av1_amf, libsvtav1), hỗ trợ mọi container (MKV, fragmented MP4 bằng `-movflags +frag_keyframe+empty_moov`, `+faststart`, WebM, GIF, WebP). Nhược: tăng installer vài chục MB, ràng buộc giấy phép.
  - *libobs*: có sẵn mọi thứ (capture, mixer, encoder, muxer) nhưng là GPL v2+, kéo theo việc toàn bộ app phải GPL; nặng và khó nhúng.
- **Cấu hình khuyến nghị cho nội dung màn hình [kiến thức nền, chưa kiểm chứng]:** CFR (30 hoặc 60 fps) để tương thích trình dựng phim; WGC chỉ phát frame khi nội dung đổi nên phải tự lặp frame để giữ CFR. x264: CRF khoảng 18-23 (thấp hơn là nét hơn), preset veryfast-medium; NVENC/QSV/AMF: CQP/CQ khoảng 18-23; keyframe 2 giây. Các con số này cần thử nghiệm trên clip mẫu có chữ nhỏ.
- **4:2:0 vs 4:4:4 [kiến thức nền, chưa kiểm chứng]:** 4:2:0 làm nhoè viền chữ màu (đặc biệt chữ đỏ/xanh trên nền tối, ClearType); 4:4:4 nét hơn nhưng H.264 High 4:4:4 Predictive thường không được hardware decoder, trình duyệt và thiết bị di động hỗ trợ. Mặc định nên là 4:2:0 (yuv420p) cho MP4; có thể cung cấp tuỳ chọn "chất lượng tối đa" 4:4:4 có cảnh báo tương thích. Cách tăng độ nét an toàn hơn là giữ nguyên độ phân giải gốc (không scale) và hạ CRF/CQP.
- Dùng bảng YouTube làm mốc VBR/giới hạn trên: nội dung màn hình tĩnh thường cần ít bitrate hơn nhiều so với video quay thật ở cùng CRF, nên rate control theo chất lượng tiết kiệm dung lượng hơn bitrate cố định.
- **MP4 mặc định an toàn:** bắt chước Hybrid MP4: ghi fragmented MP4 trong lúc quay, khi dừng thì finalize thành MP4 thường (hoặc ghi MKV tạm rồi remux không re-encode sang MP4 có faststart). Khi khởi động lại sau crash, phát hiện file tạm và đề nghị khôi phục.
- **Định dạng nên có:** MP4 (mặc định), MKV (an toàn, đa track audio), WebM (VP9/AV1 + Opus cho web), GIF và animated WebP (xuất sau khi quay, cho clip ngắn), MOV (ít giá trị trên Windows, trừ khi cần ProRes/PCM cho dựng phim).

### Gaps
- **Giấy phép FFmpeg:** không tải được `ffmpeg.org/legal.html` trong phiên này. Kiến thức nền **[chưa kiểm chứng]**: FFmpeg mặc định là LGPL v2.1+; bật `--enable-gpl` (bắt buộc để dùng libx264, libx265) biến bản build thành GPL, khi đó app link trực tiếp phải theo GPL; `--enable-nonfree` tạo bản không được phân phối lại. Bản LGPL yêu cầu link động, kèm thông báo và cung cấp mã nguồn FFmpeg đã dùng. Bản LGPL vẫn dùng được h264_nvenc/h264_qsv/h264_amf và h264_mf (Media Foundation), nên có thể né GPL bằng encoder phần cứng + fallback h264_mf thay cho libx264. Gọi `ffmpeg.exe` như tiến trình riêng thường được xem là tách biệt hơn so với link, nhưng đây là vấn đề pháp lý cần tư vấn. Cần đọc lại trang legal trước khi quyết định.
- **Bằng sáng chế H.264:** không có nguồn trong phiên này. Kiến thức nền **[chưa kiểm chứng]**: giấy phép phần mềm (LGPL/GPL) không bao gồm quyền bằng sáng chế; H.264 thuộc pool của Via LA (trước là MPEG LA), có ngưỡng miễn phí theo số lượng; dùng encoder của hệ điều hành (Media Foundation) hoặc của phần cứng thường đẩy trách nhiệm cấp phép về phía nhà cung cấp đó. HEVC có nhiều pool hơn và rủi ro cao hơn; AV1 được định vị là miễn phí bản quyền.
- Không lấy được hướng dẫn x264 chính thức (trang FFmpeg wiki bị chặn) nên các giá trị CRF/preset/tune ở trên chưa có nguồn.
- Không tìm được con số CQP khuyến nghị chính thức từ NVIDIA/Intel/AMD cho nội dung màn hình, cũng không có nguồn xác nhận mức hỗ trợ decode 4:4:4 trên thiết bị phổ thông.
- Không kiểm chứng được yêu cầu GPU cho AV1 phần cứng (RTX 40, Arc, RX 7000 trở lên theo kiến thức nền).

---

## 4. Audio: WASAPI loopback, microphone, thiết bị mặc định, per-process loopback, mixing, đồng bộ A/V

### Takeaway
System audio lấy bằng WASAPI loopback trên render endpoint mặc định (shared mode, hỗ trợ event-driven từ Windows 10 1703); microphone là WASAPI capture thông thường. Per-application loopback dùng `ActivateAudioInterfaceAsync` với `AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS`, yêu cầu build 20348 theo tài liệu, và có ưu điểm lớn là không gắn với endpoint cụ thể.

### Cited Findings
- Mở loopback: lấy `IMMDevice` của render endpoint (`GetDefaultAudioEndpoint` với `eRender`), gọi `IAudioClient::Initialize` với cờ `AUDCLNT_STREAMFLAGS_LOOPBACK`, rồi lấy `IAudioCaptureClient`. — [Microsoft Learn: Loopback Recording](https://learn.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording)
- Loopback chỉ hoạt động với shared mode (`AUDCLNT_SHAREMODE_SHARED`); stream exclusive mode không loopback được. — [Microsoft Learn: Loopback Recording](https://learn.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording)
- Trước Windows 10 1703, loopback event-driven không nhận event (phải dùng mẹo mở thêm render stream event-driven); từ 1703 trở lên loopback event-driven được hỗ trợ trực tiếp. — [Microsoft Learn: Loopback Recording](https://learn.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording)
- WASAPI loopback hoạt động bất kể phần cứng có thiết bị loopback ("Stereo Mix", "What You Hear") hay không. — [Microsoft Learn: Loopback Recording](https://learn.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording)
- DRM: driver audio tin cậy không cho loopback capture stream số chứa nội dung được bảo vệ. — [Microsoft Learn: Loopback Recording](https://learn.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording)
- Loopback mặc định chứa mix của mọi audio đang phát trên endpoint, kể cả từ các session Terminal Services khác; Remote Desktop chuyển hướng audio qua thiết bị audio riêng của session đó. — [Microsoft Learn: Loopback Recording](https://learn.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording)
- Process loopback: `ActivateAudioInterfaceAsync` với cấu trúc khởi tạo mới cho phép chỉ capture audio do một process và các process con render; có cờ đảo để capture mọi thứ TRỪ cây process đó. — [Microsoft Learn: Application loopback audio capture sample](https://learn.microsoft.com/en-us/samples/microsoft/windows-classic-samples/applicationloopbackaudio-sample/)
- Process loopback không gắn với một audio endpoint cụ thể, nên không cần tạo `IAudioClient` riêng cho từng endpoint vật lý. Nếu process đích không có render stream nào thì nhận được silence. — [Microsoft Learn: Application loopback audio capture sample](https://learn.microsoft.com/en-us/samples/microsoft/windows-classic-samples/applicationloopbackaudio-sample/)
- `AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS` gồm `TargetProcessId` và `ProcessLoopbackMode`; "Minimum supported client: Windows 10 Build 20348"; header `audioclientactivationparams.h`. Sample cũng ghi yêu cầu build 20348 trở lên. — [Microsoft Learn: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS](https://learn.microsoft.com/en-us/windows/win32/api/audioclientactivationparams/ns-audioclientactivationparams-audioclient_process_loopback_params)
- Hybrid MP4 hỗ trợ nhiều codec audio (AAC, ALAC, Opus, FLAC), và OBS cho chọn nhiều audio track khi ghi (hữu ích để tách nhạc khỏi bản ghi). — [OBS KB: Hybrid MP4](https://obsproject.com/kb/hybrid-mp4); [OBS KB: Standard Recording Output Guide](https://obsproject.com/kb/standard-recording-output-guide)

### Inferences
- **Mẹo "exclude tree" rất hữu ích:** dùng process loopback ở chế độ loại trừ với PID của chính HP Screen sẽ thu được toàn bộ system audio mà không gắn với endpoint nào, tức tự động "đi theo" thiết bị mặc định khi người dùng đổi loa/tai nghe giữa chừng và không thu lại âm thanh do chính app phát. Trên máy không hỗ trợ (dưới build 20348) thì fallback về endpoint loopback cổ điển.
- Tài liệu ghi build 20348; cộng đồng báo API này chạy được trên Windows 10 2004 (19041) trở lên dù không được ghi nhận chính thức **[chưa kiểm chứng]**. Cần thử trên Windows 10 22H2 thật trước khi dựa vào.
- **Đi theo thiết bị mặc định [kiến thức nền, chưa kiểm chứng bằng nguồn]:** đăng ký `IMMNotificationClient::OnDefaultDeviceChanged` (và xử lý `AUDCLNT_E_DEVICE_INVALIDATED`); khi thiết bị đổi thì tạo lại `IAudioClient` trên endpoint mới, resample về định dạng chung (ví dụ 48 kHz float stereo) và chèn silence cho khoảng trống để không lệch timeline.
- **Khoảng lặng:** endpoint loopback không phát gói dữ liệu khi không có gì đang phát **[chưa kiểm chứng]**; phải tự chèn silence theo timestamp (hoặc phát một render stream im lặng) nếu không audio sẽ bị dồn và lệch so với video.
- **Đồng bộ A/V:** gắn timestamp cho mọi mẫu audio bằng QPC (`IAudioCaptureClient::GetBuffer` trả `qpcPosition`) và so với `SystemRelativeTime` (QPC) của frame WGC. Cạm bẫy thường gặp: clock drift giữa hai thiết bị audio (mic và loa chạy hai clock khác nhau), độ trễ khởi động khác nhau của từng stream, cờ `DATA_DISCONTINUITY`, và việc đếm mẫu thay vì dùng timestamp.
- **Mixing vs tách track:** tách track (mic riêng, system riêng) cho phép hậu kỳ chỉnh âm lượng/khử ồn riêng, nhưng nhiều trình phát chỉ phát track đầu. Hợp lý: mặc định mix thành một track AAC trong MP4, tuỳ chọn nâng cao ghi thêm các track riêng (MKV/MP4 đều hỗ trợ đa track).

### Gaps
- Không đọc được tài liệu `IMMNotificationClient` và tài liệu về hành vi silence của loopback trong phiên này.
- Không có nguồn về echo: khi dùng loa ngoài, mic sẽ thu lại system audio; cần AEC (Windows có Voice Capture DSP / `AudioCategory_Communications`) nếu muốn xử lý.
- Không kiểm chứng được NAudio hỗ trợ process loopback sẵn hay phải tự P/Invoke.

---

## 5. Input: global hotkeys và ghi sự kiện chuột/phím có timestamp

### Takeaway
Dùng `RegisterHotKey` cho hotkey start/stop (đơn giản, không bị AV soi, nhưng thất bại nếu tổ hợp đã bị app khác đăng ký). Để ghi click/scroll/phím cho hiệu ứng click và auto-zoom, Microsoft khuyến nghị Raw Input thay cho low-level hook trong đa số trường hợp; nếu dùng `WH_MOUSE_LL`/`WH_KEYBOARD_LL` thì phải chạy trên thread riêng có message loop và trả về thật nhanh vì hook bị gỡ âm thầm khi quá timeout.

### Cited Findings
- `RegisterHotKey(hWnd, id, fsModifiers, vk)`: hệ thống post `WM_HOTKEY` vào hàng đợi của cửa sổ, hoặc của thread gọi nếu `hWnd` là NULL (phải xử lý trong message loop). — [Microsoft Learn: RegisterHotKey](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey)
- `MOD_NOREPEAT` (0x4000) ngăn auto-repeat sinh nhiều thông báo. — [Microsoft Learn: RegisterHotKey](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey)
- Hàm thường thất bại nếu tổ hợp phím đã được đăng ký cho hotkey khác; một số hotkey mặc định của hệ điều hành (như PrintScreen mở Snipping Tool) có thể bị ghi đè bởi đăng ký khác khi cửa sổ của app đang ở foreground. Thất bại nếu gắn hotkey với cửa sổ do thread khác tạo. — [Microsoft Learn: RegisterHotKey](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey)
- F12 được dành cho debugger, không nên đăng ký; tổ hợp có phím Windows (`MOD_WIN`) được dành cho hệ điều hành. Id của ứng dụng nằm trong khoảng 0x0000-0xBFFF. — [Microsoft Learn: RegisterHotKey](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey)
- `WH_KEYBOARD_LL`: hook KHÔNG được inject vào process khác; hệ thống chuyển ngữ cảnh về process đã cài hook và gọi trên chính thread đã cài, bằng cách gửi message, nên thread đó phải có message loop. — [Microsoft Learn: LowLevelKeyboardProc](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc)
- Hook phải xử lý xong trong thời gian `LowLevelHooksTimeout` (`HKEY_CURRENT_USER\Control Panel\Desktop`, mili giây). Trên Windows 7 trở lên, hook quá timeout bị gỡ âm thầm và app không có cách nào biết. Từ Windows 10 1709, timeout tối đa hệ thống cho phép là 1000 ms. — [Microsoft Learn: LowLevelKeyboardProc](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc)
- Khuyến nghị của Microsoft: nếu buộc phải dùng low-level hook, chạy trên thread chuyên dụng, đẩy việc cho worker thread rồi return ngay; "trong hầu hết trường hợp" nên theo dõi Raw Input thay vì low-level hook vì Raw Input theo dõi bất đồng bộ hiệu quả hơn. — [Microsoft Learn: LowLevelKeyboardProc](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc)
- Nên luôn gọi `CallNextHookEx`; trả giá trị khác 0 sẽ chặn sự kiện không đến app đích. Trong callback không dùng được `GetAsyncKeyState` để đọc trạng thái phím vừa đổi. — [Microsoft Learn: LowLevelKeyboardProc](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc)
- DDA cung cấp vị trí con trỏ theo từng frame (`PointerPosition`, `LastMouseUpdateTime`) và hình dạng con trỏ, có thể dùng làm nguồn dữ liệu vị trí chuột khớp frame. — [Microsoft Learn: Desktop Duplication API](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api)

### Inferences
- **Hotkey:** `RegisterHotKey` + `MOD_NOREPEAT`, báo lỗi rõ ràng và cho người dùng đổi tổ hợp khi đăng ký thất bại; tránh F12 và tổ hợp phím Windows. Hạn chế **[kiến thức nền, chưa kiểm chứng]**: hotkey có thể không tới được khi cửa sổ foreground chạy với quyền cao hơn (UIPI) hoặc trong một số game exclusive fullscreen/đọc input thô; low-level hook cũng bị UIPI giới hạn tương tự.
- **Ghi sự kiện cho click effect/auto-zoom:** phương án ít rủi ro nhất là Raw Input (`RegisterRawInputDevices` với `RIDEV_INPUTSINK` để nhận khi không ở foreground) cho click/scroll/phím, kết hợp lấy vị trí con trỏ bằng `GetCursorPos`/`GetCursorInfo` theo nhịp frame; gắn timestamp bằng `QueryPerformanceCounter` ngay khi nhận để cùng hệ quy chiếu với `SystemRelativeTime` của WGC. Raw Input cho chuyển động tương đối chứ không cho toạ độ màn hình, nên vẫn phải hỏi vị trí con trỏ **[chưa kiểm chứng]**.
- Nếu chọn `WH_MOUSE_LL`: `MSLLHOOKSTRUCT` cho toạ độ màn hình và `time` (theo tick, độ phân giải thấp hơn QPC) **[chưa kiểm chứng]**; callback chỉ nên ghi sự kiện vào hàng đợi lock-free rồi return. Hook chuột chậm sẽ làm cả hệ thống giật con trỏ.
- **Antivirus/anti-cheat [kiến thức nền, không có nguồn trong phiên này]:** hook bàn phím toàn cục là hành vi đặc trưng của keylogger nên dễ bị heuristic của AV gắn cờ, nhất là với binary chưa ký số; game có anti-cheat kernel có thể chặn hoặc nghi ngờ process cài hook. Giảm rủi ro: ký số code (EV/OV), chỉ bật ghi phím khi người dùng bật tính năng hiển thị phím tắt, chỉ lưu tổ hợp phím tắt thay vì toàn bộ ký tự gõ, không ghi khi đang ở ô mật khẩu, và ưu tiên Raw Input cho chuột.
- Chỉ cần ghi dữ liệu sự kiện (JSON/sidecar) trong lúc quay; hiệu ứng click và auto-zoom áp dụng ở bước render/hậu kỳ, giống hướng tiếp cận của các recorder kiểu Screen Studio/Cap **[chưa kiểm chứng kiến trúc cụ thể của các sản phẩm này]**.

### Gaps
- Không đọc được tài liệu Raw Input và `LowLevelMouseProc` trong phiên này; các chi tiết về `RIDEV_INPUTSINK`, `MSLLHOOKSTRUCT` là kiến thức nền.
- Không tìm được nguồn đáng tin nào (bài viết kỹ thuật của hãng AV/anti-cheat) định lượng rủi ro bị gắn cờ do `SetWindowsHookEx`.
- Không tìm được bài kỹ thuật của Cap, Loom hay Screen Studio mô tả cách họ ghi sự kiện input trên Windows.

---

## 6. Screenshots: độ phân giải gốc, DPI awareness, định dạng, bóng đổ và góc bo trên Windows 11

### Takeaway
Ảnh chụp nét nhất đến từ việc lấy texture ở pixel vật lý (WGC/DDA trả đúng kích thước pixel của nguồn) và lưu PNG lossless; điều kiện tiên quyết là process phải khai báo Per-Monitor DPI Aware V2 để mọi toạ độ (vùng chọn, khung cửa sổ) cùng ở pixel vật lý. Khung cửa sổ "nhìn thấy" lấy bằng `DWMWA_EXTENDED_FRAME_BOUNDS`; góc bo/viền là thuộc tính DWM có từ Windows 11 build 22000.

### Cited Findings
- WGC có thể dùng để chụp một ảnh đơn: lấy frame, chuyển thành bitmap và lưu PNG (ví dụ chính thức dùng Win2D `CanvasBitmap.SaveAsync(..., CanvasBitmapFileFormat.Png, 1f)`). — [Microsoft Learn: Screen capture](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture)
- Khi chờ đồng bộ một snapshot WGC, dùng `CreateFreeThreaded` để tránh deadlock. — [robmikh/Win32CaptureSample](https://github.com/robmikh/Win32CaptureSample)
- Trên hệ thống bật HDR, capture BGRA8 có thể bị clipping/washed out; cần `R16G16B16A16_FLOAT` và tone-map hoặc lưu định dạng HDR. — [Microsoft Learn: Screen capture](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture)
- `DWMWA_EXTENDED_FRAME_BOUNDS` (dùng với `DwmGetWindowAttribute`) trả về hình chữ nhật extended frame bounds trong không gian màn hình, kiểu `RECT`. — [Microsoft Learn: DWMWINDOWATTRIBUTE](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute)
- `DWMWA_WINDOW_CORNER_PREFERENCE` (góc bo), `DWMWA_BORDER_COLOR` (đặt `DWMWA_COLOR_NONE` = 0xFFFFFFFE để bỏ vẽ viền), `DWMWA_VISIBLE_FRAME_BORDER_THICKNESS` (độ dày viền ngoài DWM vẽ, thay đổi theo DPI): đều hỗ trợ từ Windows 11 build 22000. Các thuộc tính set chỉ áp dụng cho cửa sổ của chính mình qua `DwmSetWindowAttribute`. — [Microsoft Learn: DWMWINDOWATTRIBUTE](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute)
- `DWMWA_CLOAKED` cho biết cửa sổ đang bị cloak (bởi app, Shell, hoặc kế thừa); hữu ích để lọc cửa sổ "ma" khi liệt kê cửa sổ cho người dùng chọn. — [Microsoft Learn: DWMWINDOWATTRIBUTE](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute)
- `DWMWA_CAPTION_BUTTON_BOUNDS` trả RECT không xác định khi cửa sổ minimized/không hiển thị; có thể dùng để phát hiện trạng thái đó. — [Microsoft Learn: DWMWINDOWATTRIBUTE](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute)
- GDI: ví dụ chính thức chụp desktop bằng `GetDC(NULL)` + `StretchBlt`/`BitBlt` với kích thước từ `GetSystemMetrics(SM_CXSCREEN/SM_CYSCREEN)` (các giá trị này phụ thuộc DPI awareness của process). — [Microsoft Learn: Capturing an Image](https://learn.microsoft.com/en-us/windows/win32/gdi/capturing-an-image)
- ScreenRecorderLib hỗ trợ xuất ảnh PNG; `windows-capture` có `frame.save_as_image()` với `ImageFormat::Png`. — [sskodje/ScreenRecorderLib](https://github.com/sskodje/ScreenRecorderLib); [NiiightmareXD/windows-capture](https://github.com/NiiightmareXD/windows-capture)

### Inferences
- **DPI [kiến thức nền, chưa kiểm chứng bằng nguồn trong phiên này]:** khai báo trong application manifest `<dpiAwareness>PerMonitorV2, PerMonitor</dpiAwareness>` kèm `<dpiAware>true/pm</dpiAware>` (hoặc gọi `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` trước khi tạo cửa sổ). Nếu process không DPI-aware, Windows ảo hoá toạ độ và GDI trả ảnh đã scale (mờ) trên màn hình 125%/150%. Texture từ WGC/DDA luôn ở pixel vật lý của nguồn, nên vấn đề chính là toạ độ vùng chọn và overlay chọn vùng phải khớp pixel vật lý.
- `GetWindowRect` trên Windows 10/11 bao gồm cả viền resize vô hình (khoảng 7 px mỗi bên ở 100%) và không phản ánh phần nhìn thấy; dùng `DWMWA_EXTENDED_FRAME_BOUNDS` để lấy khung nhìn thấy. Giá trị này không được điều chỉnh theo DPI ảo hoá, nên process phải per-monitor aware để các toạ độ khớp nhau **[chưa kiểm chứng; ghi chú này nằm ở trang `DwmGetWindowAttribute`, không đọc trong phiên này]**.
- **Chụp cửa sổ không bóng, góc bo:** WGC `CreateForWindow` trả nội dung cửa sổ không kèm bóng đổ; trên Windows 11 bốn góc ngoài vùng bo có alpha trong suốt/đen tuỳ format, nên lưu PNG có alpha để giữ góc bo đẹp **[chưa kiểm chứng]**. **Chụp có bóng:** bóng do DWM vẽ bên ngoài cửa sổ, WGC window capture không có; muốn có thì chụp vùng màn hình mở rộng quanh `EXTENDED_FRAME_BOUNDS` (sẽ dính nền phía sau) hoặc tự vẽ bóng tổng hợp lên nền trong suốt ở hậu kỳ, là cách cho kết quả sạch nhất.
- **Định dạng:** PNG là mặc định cho ảnh màn hình (lossless, chữ sắc nét, hỗ trợ alpha). JPG tạo artefact quanh chữ, chỉ nên là tuỳ chọn khi cần file nhỏ cho ảnh nhiều chi tiết ảnh chụp. WebP lossless nhỏ hơn PNG đáng kể và có alpha, nhưng mức hỗ trợ dán/đính kèm ở một số ứng dụng kém hơn PNG; nên là tuỳ chọn. Clipboard nên đưa cả bitmap lẫn PNG.
- Không scale ảnh ở bất kỳ bước nào; "sắc nét nhất" nghĩa là 1 pixel ảnh = 1 pixel vật lý. Với màn hình HDR, tone-map về sRGB trước khi lưu PNG 8-bit.

### Gaps
- Không đọc được tài liệu DPI awareness (manifest, `SetProcessDpiAwarenessContext`) và trang `DwmGetWindowAttribute` trong phiên này.
- Không có nguồn xác nhận chính xác alpha của góc bo trong frame WGC window capture trên Windows 11; cần thử nghiệm.
- Không có số liệu so sánh kích thước PNG với WebP lossless cho ảnh màn hình.

---

## 7. Framework cho app shell và mức hỗ trợ capture

### Takeaway
Ba hướng khả thi cho HP Screen: (a) C#/.NET (WPF hoặc WinUI 3) với ScreenRecorderLib hoặc tự gọi WGC + Media Foundation, phát triển nhanh nhất; (b) Rust + Tauri với crate `windows-capture`, installer nhỏ và hiệu năng native; (c) Electron, nhanh về UI nhưng pipeline capture của Chromium khó kiểm soát chất lượng/encoder. libobs mạnh nhất nhưng nặng và GPL.

### Cited Findings
- **ScreenRecorderLib (.NET):** thư viện quay màn hình cho .NET dùng Media Foundation gốc để encode realtime; ghi H.264 hoặc ảnh PNG; API dạng `Recorder.CreateRecorder()` / `Record(path)` / `Stop()` với các event trạng thái/hoàn tất/lỗi; yêu cầu Windows 8+, Visual C++ Redistributable (x64/x86 khớp nền tảng), Media Foundation (Windows N/KN cần Media Feature Pack); giấy phép MIT; có trên NuGet; 805 commit, 502 sao, 25 issue mở. Trang README không liệt kê chi tiết tính năng (window/region/audio/H.265), cần xem wiki. — [sskodje/ScreenRecorderLib](https://github.com/sskodje/ScreenRecorderLib)
- **windows-capture (Rust + Python):** tự mô tả là thư viện capture Windows nhanh nhất cho Rust và Python; MIT; phiên bản 2.0.1; dùng WGC và hỗ trợ cả DXGI Desktop Duplication; chỉ cập nhật frame khi cần; có `VideoEncoder` tăng tốc phần cứng với audio timing ổn định (ghi file hoặc stream bộ nhớ); `Settings` gồm `CursorCaptureSettings`, `DrawBorderSettings`, `SecondaryWindowSettings`, `MinimumUpdateIntervalSettings` (giới hạn tần suất nhưng không đảm bảo frame rate cố định), `DirtyRegionSettings`, `ColorFormat`; binding Python trong thư mục `windows-capture-python`; 528 sao, 14 issue mở. — [NiiightmareXD/windows-capture](https://github.com/NiiightmareXD/windows-capture)
- **scap (Rust, đa nền tảng):** trên Windows dùng WGC thông qua crate `windows-capture` (1.3.6); vẫn tự ghi nhãn work-in-progress, chưa phù hợp production; bản 0.0.8 build docs.rs lỗi. — [docs.rs: scap README](https://docs.rs/crate/scap/0.0.8/source/README.md); [helmerapp/scap](https://www.github.com/helmerapp/scap)
- **xcap (Rust):** có backend WGC trên Windows (file `windows/wgc.rs`). — [docs.rs: xcap wgc.rs](https://docs.rs/crate/xcap/0.9.8/source/src/windows/wgc.rs)
- **Electron:** main process gọi `desktopCapturer.getSources({ types: ['screen'] | ['window'] })`, đăng ký `session.setDisplayMediaRequestHandler()` và trả `callback({ video: source, audio: 'loopback' })`; renderer gọi `navigator.mediaDevices.getDisplayMedia()`. `getDisplayMedia` không hỗ trợ `deviceId` để chọn nguồn. Thumbnail mặc định 150x150. — [Electron docs: desktopCapturer](https://www.electronjs.org/docs/latest/api/desktop-capturer)
- Tuỳ chọn audio `loopback` / `loopbackWithMute` (thu system audio) của Electron hiện chỉ hỗ trợ trên Windows, theo tóm tắt kết quả tìm kiếm về tài liệu session API (không đọc trực tiếp trang session). — [Electron docs: desktopCapturer](https://www.electronjs.org/docs/latest/api/desktop-capturer)
- **libwebrtc/Chromium** (nền của Electron) có bộ capture WGC riêng (`wgc_capturer_win.cc`) và có kiểm tra `DirtyRegionMode` qua API contract. — [Firefox source: libwebrtc wgc_capturer_win.cc](https://arai.searchfox.org/firefox-main/source/third_party/libwebrtc/modules/desktop_capture/win/wgc_capturer_win.cc)
- **WinUI 3:** dùng được `Windows.Graphics.Capture` (namespace không đổi so với UWP); picker phải khởi tạo bằng HWND qua `InitializeWithWindow`; thao tác composition phải dispatch về UI thread. — [Microsoft Learn: Screen capture](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture)
- **OBS** có cả ba phương thức window capture (Automatic, BitBlt, WGC) và cho thấy ngay cả sản phẩm trưởng thành vẫn gặp lỗi chọn phương thức. — [OBS issue #13829 (mirror)](https://obs-versions.com/community/issues/13829)

### Inferences
Bảng so sánh dưới đây là đánh giá của người nghiên cứu **[phần lớn là kiến thức nền, chưa kiểm chứng bằng nguồn trong phiên này; kích thước installer là ước lượng]**:

| Hướng | Capture | Hiệu năng | Installer | Tốc độ phát triển | Ghi chú |
|---|---|---|---|---|---|
| C# / .NET 8-10 + WPF hoặc WinUI 3 | WGC qua CsWinRT; ScreenRecorderLib (MIT, Media Foundation); Vortice cho D3D11/DXGI/MF; NAudio cho WASAPI | Tốt nếu giữ frame trên GPU; GC cần lưu ý ở đường nóng | Nhỏ nếu framework-dependent; self-contained khoảng 60-150 MB, có thể trim | Nhanh nhất cho app Windows thuần | WinUI 3 hiện đại hơn nhưng nhiều ràng buộc đóng gói; WPF chín muồi, dễ làm overlay trong suốt |
| Rust + Tauri (WebView2) | `windows-capture` (WGC + DDA + encoder), crate `windows` cho WASAPI/MF/hook | Native, không GC | Rất nhỏ (vài MB đến khoảng 15 MB, dựa vào WebView2 có sẵn trên Windows 11) | Trung bình; UI web nhanh, phần native đòi hỏi kỹ năng Rust | Cùng kiểu kiến trúc mà Cap sử dụng **[chưa kiểm chứng]** |
| C++ + libobs | Đầy đủ nhất (WGC, DDA, game capture hook, mixer, encoder, muxer) | Tốt nhất, đã tối ưu | Lớn | Chậm; API phức tạp | GPL v2+ buộc app theo GPL |
| Electron | `desktopCapturer`/`getDisplayMedia` (WGC/DDA bên trong Chromium), audio `loopback` | Trung bình; `MediaRecorder` ít kiểm soát encoder, thường cho VFR và WebM/MP4 chất lượng khó tinh chỉnh | Lớn (khoảng 80-150 MB) | Nhanh nhất về UI | Muốn chất lượng cao thường phải kèm addon native hoặc ffmpeg, làm mất lợi thế đơn giản |
| Python | `windows-capture` binding, dxcam (DDA) | Đủ cho prototype; GIL và đóng gói là trở ngại | Lớn khi đóng gói (PyInstaller) | Nhanh cho thử nghiệm | Không khuyến nghị cho sản phẩm phát hành |

- Với mục tiêu của HP Screen (window capture sạch, region, MP4, mic + system audio theo thiết bị mặc định, hotkey, screenshot nét), cả hướng .NET và Rust đều đáp ứng được bằng API chính thức. Thư mục dự án nằm trong "PHAT TRIEN GAME" gợi ý nhóm có thể quen C#; nếu vậy .NET + WGC trực tiếp (ScreenRecorderLib để khởi động nhanh, hoặc tự viết bằng Vortice khi cần kiểm soát fragmented MP4/đa track) là đường ngắn nhất. Đây là suy đoán, cần hỏi lại nhóm.
- Rủi ro của việc phụ thuộc thư viện bọc sẵn: khó kiểm soát crash-safety của MP4 (Media Foundation Sink Writer ghi moov ở cuối) và các property WGC mới; nên kiểm tra xem thư viện có lộ ra `IsBorderRequired`, fragmented MP4 và đa track audio hay không trước khi cam kết.

### Gaps
- Không kiểm chứng được kiến trúc Windows thực tế của Cap, Loom, Screen Studio (không tìm được bài kỹ thuật; kết quả tìm kiếm chỉ ra các crate Rust liên quan).
- Không đọc được wiki của ScreenRecorderLib để xác nhận danh sách tính năng (ghi cửa sổ, vùng, nhiều nguồn, audio in/out, H.265, overlay click chuột) và ngày phát hành gần nhất.
- Không có nguồn về dxcam, NAudio, Vortice trong phiên này; không có số đo kích thước installer thực tế cho từng framework.
- Không kiểm chứng được Electron trên Windows chọn WGC hay DDA mặc định ở phiên bản 2026, và có cho tắt viền vàng hay không.
