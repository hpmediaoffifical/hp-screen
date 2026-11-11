# Nâng cấp quay — Bước 7 (phụ đề tiếng Việt offline) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Trong thẻ Phụ đề:
- tải bộ nhận giọng về khi người dùng đồng ý (một lần);
- tạo phụ đề tiếng Việt ngay trên máy;
- sửa từng dòng phụ đề;
- xuất file SRT, và/hoặc ghi cứng phụ đề vào video.

**Architecture:**
- **`hp-core::subtitle`** (thuần): đọc JSON của whisper.cpp, chia đoạn nhận dạng thành dòng phụ đề, quy đổi qua phần cắt, ghi SRT.
- **`asr.rs`:**
  - danh mục model là dữ liệu (tên, URL, kích thước, SHA-256);
  - tải bằng WinHTTP qua crate `windows`, không thêm thư viện HTTP;
  - kiểm SHA-256 bằng crate `sha2` (MIT/Apache), giải nén bằng crate `zip` (MIT, chỉ bật deflate);
  - tách tiếng ra WAV 16 kHz mono bằng `AudioDecoder` và bộ resampler sẵn có;
  - chạy `whisper-cli.exe` thành tiến trình riêng, đọc phần trăm tiến độ, hủy được.
- **Ghi cứng phụ đề:** `FramePlan.subtitle` cộng DirectWrite trong `Compositor`.

**Spec:** mục 4.7.

**Thay đổi so với spec:** spec yêu cầu đo hai model trên video thật của người dùng trước khi chốt. Bước đó cần người dùng cung cấp video và cho phép tải model (khoảng 0,6 GB). Người dùng đang ngủ và đã giao tự quyết, nhưng việc tải file luôn phải được đồng ý riêng. Vì vậy:
- Mặc định là Whisper large-v3-turbo q5_0. Lý do: báo cáo cho WER khoảng 9,5% trên FLEURS tiếng Việt, xử lý tốt câu trộn Việt–Anh, có sẵn dấu câu, giấy phép MIT.
- Có thêm lựa chọn "Nhẹ": `ggml-base`.
- PhoWhisper không có bản ggml công khai trong kho chính thức, nên để sau.
- Phép đo đưa vào danh sách việc người dùng làm khi thức dậy.

## Global Constraints

- **Không tải gì khi người dùng chưa bấm đồng ý trong hộp thoại.** Hộp thoại ghi rõ: dung lượng, nguồn (github.com/ggml-org/whisper.cpp và huggingface.co/ggerganov/whisper.cpp), nơi lưu `%LOCALAPPDATA%\HP Screen\asr\`, và câu "âm thanh không rời khỏi máy".
- Tải xong phải kiểm SHA-256; sai thì xóa file và báo lỗi. Các mã băm lấy từ API công khai ngày 2026-11-12:
  - `whisper-bin-x64.zip` (b5454): 8 928 640 B, `6ba69e3482d7826214f90a6a9c84ca07782aec1e1d0c6a7c30c994fd5d816ccb`
  - `ggml-large-v3-turbo-q5_0.bin`: 574 041 195 B, `394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2`
  - `ggml-base.bin`: 147 951 465 B, `60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe`
- **Chia dòng:** tối đa 2 dòng × 42 ký tự, dài 1–6 giây; ngắt ưu tiên ở dấu câu, sau đó ở khoảng trắng; có mốc thời gian từng token thì dùng, không có thì chia thời gian theo số ký tự.
- **SRT** tính theo thời gian xuất: dòng nằm hẳn trong phần cắt bị bỏ, dòng vắt qua chỗ cắt bị cắt ngắn.
- **Ghi cứng:**
  - chữ trắng Segoe UI Semibold, cao bằng 4,5% chiều cao khung;
  - nền đen mờ bo góc;
  - rộng tối đa 80% khung, cách mép dưới 6%;
  - khi có phụ đề, chip phím dời lên trên dòng phụ đề.
- Commit kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. Video không có tiếng → báo "Video này không có tiếng", không chạy nhận dạng.
2. Dòng phụ đề có ký tự đặc biệt hoặc xuống dòng → SRT vẫn hợp lệ.
3. Tải bị ngắt, hoặc sai SHA-256 → không còn file dở, có nút thử lại.
4. `whisper-cli` thoát với mã lỗi → báo lỗi, ghi 20 dòng cuối của stderr vào log.
5. Phụ đề vắt qua phần cắt → SRT đúng thời gian xuất.

---

### Task 1: `hp-core::subtitle`

- `Cue { start_ms, end_ms, text }`
- `SubtitleDoc { cues: Vec<Cue>, burn: bool }`, thêm thành `EditDoc.subtitles`.
- `AsrSegment { start_ms, end_ms, text, tokens: Vec<(i64, i64, String)> }`
- `parse_whisper_json(&str) -> Result<Vec<AsrSegment>, String>`, đọc định dạng `-ojf`: `transcription[].offsets.from/to`, `text`, `tokens[].offsets`, `tokens[].text`. Bỏ các token đặc biệt `[_...]`.
- `cues_from_segments(&[AsrSegment]) -> Vec<Cue>`
- `wrap_lines(text) -> Vec<String>`: tối đa 42 ký tự mỗi dòng.
- `to_srt(cues, cuts: &Cuts) -> String`
- `cue_at(cues, t_ms) -> Option<&Cue>`
- Test:
  - đọc một mẫu JSON;
  - đoạn dài bị chia, mỗi dòng phụ đề ≤ 6 giây và ≤ 2×42 ký tự, ưu tiên ngắt ở dấu câu;
  - không có token thì chia theo số ký tự;
  - định dạng thời gian SRT `00:01:02,345`;
  - dòng nằm trong phần cắt bị bỏ, dòng vắt qua bị cắt ngắn;
  - dòng rỗng bị bỏ;
  - dòng nhiều dòng giữ ngắt dòng;
  - file chỉnh sửa cũ vẫn đọc được.

### Task 2: Ghi cứng phụ đề trong bản xuất

- `FramePlan.subtitle: Option<String>`, có giá trị khi `burn` bật và có dòng tại `src_ms`.
- `Draw::Chip.bottom` tăng lên khi có phụ đề.
- `Compositor` vẽ phụ đề: tự xuống dòng trong 80% bề rộng, nền bo góc.
- Test WARP: `subtitle_is_drawn_near_the_bottom`.
- Test plan: `burned_subtitle_is_in_the_frame_plan`.

### Task 3: `asr.rs`

- `CATALOG`: chương trình và hai model.
- `asr_dir()`, `installed(model) -> bool`
- `verify_sha256(path, hex) -> bool`
- `extract_zip(zip, dir)`: tìm `whisper-cli.exe` ở bất kỳ thư mục con nào.
- `write_wav_16k_mono(source, wav)`
- `download(url, dest, expected_len, sha, progress, cancel)`: WinHTTP, ghi vào file `.part`, đổi tên khi xong, theo được chuyển hướng.
- `run_whisper(exe, model, wav, out_json, threads, progress, cancel) -> Result<String>`: đọc dòng `progress = N%` trên stderr; hủy thì kết thúc tiến trình.
- Test (không cần mạng):
  - `sha256_of_a_known_file`;
  - `wav_header_and_length`: 1 giây tiếng từ video mẫu ra 16 000 mẫu;
  - `progress_lines_are_read`;
  - `catalog_entries_are_complete`;
  - `zip_with_the_cli_in_a_subfolder` (zip tự tạo trong test);
  - `#[ignore] full_download_and_recognise` (cần mạng và model).

### Task 4: Lệnh và giao diện

- Lệnh:
  - `asr_status() -> { cli: bool, models: [{id, name, size_mb, installed}] }`
  - `asr_install(model)`, chạy nền; sự kiện `asr-progress {phase, done, total}`, `asr-done`, `asr-error`
  - `asr_cancel()`
  - `subtitles_generate(path, model)`, chạy nền; sự kiện `subs-progress {percent}`, `subs-done {cues}`, `subs-error`
  - `export_srt(path, doc) -> String`, ghi `<tên>_edit.srt` (không ghi đè file sẵn có)
- **Thẻ Phụ đề:**
  - chưa cài bộ nhận giọng thì hiện hộp đồng ý tải;
  - "Tạo phụ đề" kèm thanh tiến độ và nút Hủy;
  - danh sách dòng: thời gian, ô chữ, Gộp với dòng sau, Tách tại con trỏ, Xóa; bấm một dòng thì tua tới dòng đó;
  - ô "Ghi cứng phụ đề vào video";
  - nút "Xuất SRT".
- **Xem trước:** vẽ dòng phụ đề đang phát trên lớp cố định.

**Điểm dừng:** chạy bốn lệnh kiểm, thêm mục S1–S8 vào danh sách thử tay, review bằng reviewer mới, sửa lỗi Critical / Important.
