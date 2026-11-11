# Tính năng "thông minh" của trình quay màn hình: AI phân tích, phụ đề tự động, auto zoom theo click, hiệu ứng chuột/phím (cho HP Screen, Windows, ưu tiên tiếng Việt)

> Ngày kiểm tra nguồn và giá: 2026-11-11. Giá API thay đổi nhanh, cần kiểm tra lại trước khi chốt. Mục "Inferences" là suy luận của người nghiên cứu, không phải dữ kiện có nguồn. Những gì không tìm được nguồn được đưa vào "Gaps".

## 1. Phụ đề tự động / speech-to-text (ưu tiên tiếng Việt)

### Takeaway
Với tiếng Việt chạy local trên Windows, hai ứng viên có số liệu công bố tốt nhất là Zipformer-30M (qua sherpa-onnx, rất nhẹ, chạy CPU) và PhoWhisper (họ Whisper fine-tune, nặng hơn nhưng có sẵn dấu câu/timestamp kiểu Whisper); Whisper large-v3 / large-v3-turbo là lựa chọn đa ngôn ngữ (Việt + Anh) an toàn. Phía cloud, giá phổ biến khoảng 0,0025–0,006 USD/phút; ElevenLabs Scribe tự công bố WER tiếng Việt thấp nhất (số của chính hãng, chưa có kiểm chứng độc lập).

### Cited Findings

**Mô hình tiếng Việt chuyên biệt và số WER**
- PhoWhisper (VinAI) có 5 cỡ: tiny 39M, base 74M, small 244M, medium 769M, large 1,55B tham số; fine-tune từ Whisper đa ngôn ngữ trên bộ dữ liệu 844 giờ gồm nhiều giọng vùng miền; repo ghi licence BSD-3-Clause; ví dụ dùng qua `transformers` pipeline với audio 16 kHz; repo không hướng dẫn chuyển sang faster-whisper/whisper.cpp — [PhoWhisper GitHub](https://github.com/VinAIResearch/PhoWhisper)
- WER của PhoWhisper (CMV-Vi / VIVOS / VLSP 2020 Task-1 / VLSP 2020 Task-2): tiny 19,05 / 10,41 / 20,74 / 49,85; base 16,19 / 8,46 / 19,70 / 43,01; small 11,08 / 6,33 / 15,93 / 32,96; medium 8,27 / 4,97 / 14,12 / 26,85; large 8,14 / 4,67 / 13,75 / 26,68 — [PhoWhisper GitHub](https://github.com/VinAIResearch/PhoWhisper); bài báo khẳng định PhoWhisper-large đạt SOTA trên từng benchmark tại thời điểm công bố (ICLR 2024 Tiny Papers) — [arXiv 2406.02555](https://arxiv.org/pdf/2406.02555)
- Vietnamese Open ASR Leaderboard (VietAudio, Hugging Face Space) so 7 mô hình trên 9 bộ dữ liệu, hai nhóm cột "O-WER" và "N-WER" (trang không định nghĩa rõ; nhiều khả năng là WER gốc và WER sau chuẩn hoá văn bản — chưa kiểm chứng). Trung bình N-WER trên dữ liệu "Natural": Zipformer-30M-RNNT-6000h 6,77; PhoWhisper-medium 7,61; PhoWhisper-small 8,77; Qwen3-ASR-1.7B 9,84; Qwen3-ASR-0.6B 12,44; nvidia/parakeet-ctc-0.6b-Vietnamese 12,81; wav2vec2-base-vietnamese-250h 14,89. Trung bình O-WER "Natural": PhoWhisper-medium 16,73; Zipformer-30M 17,51; PhoWhisper-small 17,73; Qwen3-ASR-1.7B 18,76; Parakeet-vi 20,78; Qwen3-ASR-0.6B 20,92; wav2vec2 24,26. Leaderboard không có cột tốc độ/RTF hay kích thước — [Vietnamese Open ASR Leaderboard](https://vietaudio-team-vietnamese-asr-leaderboard.hf.space/)
- Zipformer-30M-RNNT-6000h: kiến trúc ZipFormer chỉ ~30 triệu tham số, huấn luyện trên ~6.000 giờ; model card tự báo WER 7,56% (GigaSpeech2-Test), 8,10% (VLSP2025 PrivateTest), 11,10% (VLSP2023 PrivateTest); có bản xuất ONNX cho sherpa-onnx — [model card (bản mirror)](https://hug-proxy.pages.dev/hynt/Zipformer-30M-RNNT-6000h)
- sherpa-onnx liệt kê 3 mô hình tiếng Việt, đều là offline (không streaming) zipformer transducer: `sherpa-onnx-zipformer-vi-30M-int8-2026-02-09`, `sherpa-onnx-zipformer-vi-2025-04-20`, `sherpa-onnx-zipformer-vi-int8-2025-04-20`; không có mô hình streaming tiếng Việt trong danh sách; ví dụ dùng kèm VAD để nhận dạng micro gần-thời-gian-thực; sherpa-onnx có API cho C, C#, Python, Java, JS, Go, Rust, Kotlin, Swift, WebAssembly, Tauri... — [sherpa-onnx pretrained models](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/index.html)
- Dự án phostt (server Rust dùng weights Zipformer-vi của sherpa-onnx) báo WER ~7,7% trên GigaSpeech2-vi (giọng sạch) nhưng 103,08% trên FLEURS tiếng Việt, tác giả giải thích do tên riêng nước ngoài, số, thuật ngữ tiếng Anh bị phiên âm sang tiếng Việt — [phostt trên docs.rs](https://docs.rs/crate/phostt/latest)
- Một model card khác (fine-tune Zipformer INT8, test giọng nói đời thường) báo zipformer-6000h gốc đạt 21,93% WER tổng trên bộ test của họ, checkpoint tốt nhất 8,22% — nguồn đơn lẻ, chưa kiểm chứng — [NghiASR](https://app.alphaneural.io/assets/NghiMe/NghiASR)
- Moonshine: có mô hình đơn ngữ tiếng Việt `moonshine-ai/moonshine-tiny-vi`; README báo Moonshine Tiny (27M) đạt 15,92% WER tiếng Việt so với 96,4% của Whisper Tiny; cột Base tiếng Việt để trống. Bài báo "Flavors of Moonshine" lại nêu Whisper Tiny tiếng Việt ~60% lỗi — hai con số mâu thuẫn — [Flavors of Moonshine (arXiv 2509.02523)](https://arxiv.org/html/2509.02523v1); [moonshine-tiny-vi](https://huggingface.co/moonshine-ai/moonshine-tiny-vi). Moonshine v2 là kiến trúc streaming độ trễ thấp nhưng số liệu streaming tìm được chỉ cho tiếng Anh — [Moonshine v2 (arXiv 2602.12241)](https://arxiv.org/html/2602.12241v1)
- Fine-tune cộng đồng PhoWhisper-large cho code-switching Việt–Anh tự báo WER-N 4,56% (baseline 20,95%) — tự công bố, chưa bình duyệt — [rinhoooo/phowhisper-large-vien-cs-asr](https://huggingface.co/rinhoooo/phowhisper-large-vien-cs-asr)
- EraX-WoW-Turbo (Whisper large-v3-turbo fine-tune, có bản CT2 cho faster-whisper) tự báo WER ~12% trung bình trên các ngôn ngữ chính kể cả phương ngữ Việt khó, và "30 giây audio trong ~350 ms" (không nêu phần cứng; tác giả gọi là sơ bộ) — [EraX-WoW-Turbo-V1.1-CT2](https://huggingface.co/erax-ai/EraX-WoW-Turbo-V1.1-CT2)

**Whisper đa ngôn ngữ trên tiếng Việt và tốc độ**
- Benchmark của Handy trên FLEURS tiếng Việt (whisper.cpp, lượng tử hoá Q8_0, chạy GPU tích hợp AMD Ryzen 4750U qua Vulkan): whisper-large-v3 ~8,7% WER, large-v3-turbo ~9,5%, medium ~13,7% — [Handy: best model for Vietnamese](https://models.handy.computer/languages/vi)
- Bảng của ElevenLabs trên FLEURS tiếng Việt: Scribe v1 3,5% (trang khác ghi 3,1%), Gemini Flash 2 4,2%, Whisper Large v3 10,7%, Deepgram Nova 2 11,6%; Common Voice 5,5% hoặc 9,7% tuỳ bản dịch trang — số do hãng tự công bố, không nhất quán giữa các bản trang, và là Scribe v1 chứ không phải v2 — [ElevenLabs Vietnamese STT](https://elevenlabs.io/speech-to-text/vietnamese)
- faster-whisper trên GPU (bảng benchmark LibriSpeech, beam 5): large-v3-turbo fp16 19,155 s, int8 19,591 s (WER 1,919); large-v3 fp16 52,023 s, int8 52,639 s → turbo nhanh hơn ~2,5–2,7 lần; turbo int8 dùng khoảng 1,5 GB VRAM — [HF discussion: speed benchmark](https://huggingface.co/deepdml/faster-whisper-large-v3-turbo-ct2/discussions/3); [faster-whisper issue #1030](https://github.com/SYSTRAN/faster-whisper/issues/1030)
- Một hướng dẫn bên thứ ba nêu large-v3 INT8 trên CPU 16 nhân x86 đạt 1,5–2,5× thời gian thực — chưa kiểm chứng với nguồn gốc — [localaimaster faster-whisper guide](https://localaimaster.com/blog/faster-whisper-guide)
- PhoWhisper ngoài miền (lời bài hát) kém hẳn: PhoWhisper-large 38,3 WER trên dữ liệu thô, trong khi Whisper-large-v2 fine-tune riêng đạt 20,1 — [VietLyrics (arXiv 2510.22295)](https://arxiv.org/pdf/2510.22295)

**Windows Live Captions**
- Live Captions trên Copilot+ PC (Windows 11 24H2 trở lên) có thể dịch từ hơn 40 ngôn ngữ (có tiếng Việt) sang tiếng Anh; đầu ra dịch là English (US) hoặc Chinese (Simplified); không tìm thấy nguồn nào nói Live Captions xuất phụ đề bằng tiếng Việt — [Microsoft Support: Use live captions](https://support.microsoft.com/accessibility/windows/use-live-captions-to-better-understand-audio)

**Cloud API và giá (kiểm tra 2026-11-11)**
- OpenAI (trang giá chính thức): gpt-4o-transcribe ~0,006 USD/phút (2,50/10,00 USD mỗi 1M token vào/ra); gpt-4o-mini-transcribe ~0,003 USD/phút; gpt-transcribe 0,0045 USD/phút; Whisper 0,006 USD/phút; gpt-4o-transcribe-diarize 0,006 USD/phút — [OpenAI API pricing](https://developers.openai.com/api/docs/pricing)
- Một nguồn bên thứ ba nói OpenAI đã deprecate whisper-1, gpt-4o-transcribe và gpt-4o-mini-transcribe ngày 26/08/2026, gỡ khỏi API ngày 26/02/2027; trang giá chính thức của OpenAI không ghi ngày này — CHƯA KIỂM CHỨNG, cần xem trang deprecations của OpenAI — [digitalapplied: gpt-transcribe comparison](https://www.digitalapplied.com/blog/openai-gpt-transcribe-stt-model-comparison-2026)
- AssemblyAI Universal-2 từ 0,15 USD/giờ audio (~0,0025 USD/phút) — [AssemblyAI pricing blog](https://assemblyai.com/blog/speech-to-text-api-pricing)
- Deepgram Nova-3: 0,0043 USD/phút (batch), 0,0077 USD/phút (streaming) theo tổng hợp bên thứ ba; nguồn khác nêu real-time gần 0,0092 USD/phút — số liệu không thống nhất — [soniox STT pricing compare](https://soniox.com/compare/stt/pricing); [smallest.ai pricing explainer](https://smallest.ai/blog/speech-to-text-api-pricing-models-explained-(2026))
- ElevenLabs Scribe v2 0,22 USD/giờ (nguồn khác ghi 0,27), Scribe v2 Realtime 0,39 USD/giờ — số bên thứ ba, có mâu thuẫn — [convertaudiototext pricing 2026](https://convertaudiototext.com/blog/speech-to-text-api-pricing-2026)
- Gemini có thể nhận audio trực tiếp: Gemini 3.5 Flash-Lite 0,30 USD/1M token vào (text/ảnh/video/audio), ra 2,50; Gemini 3.1 Flash-Lite audio 0,50 vào, ra 1,50; Gemini 2.5 Flash-Lite audio 0,30 vào, ra 0,40; các model Flash/Flash-Lite có free tier (nội dung có thể được dùng để cải thiện sản phẩm Google) — [Gemini API pricing](https://aistudio.google.com/docs/pricing)

### Inferences
- Kiến trúc đề xuất cho HP Screen: (a) mặc định local, xử lý sau khi quay: Zipformer-vi 30M int8 qua sherpa-onnx (có binding C#/C, chạy CPU, gói cài nhỏ) cho tiếng Việt thuần; (b) tuỳ chọn "chất lượng cao/đa ngôn ngữ": Whisper large-v3-turbo qua whisper.cpp (Vulkan/CUDA) hoặc faster-whisper, tải model theo yêu cầu; (c) tuỳ chọn cloud (người dùng nhập API key) cho máy yếu.
- Zipformer thắng về N-WER và kích thước, nhưng lỗi nặng với tên riêng/thuật ngữ tiếng Anh (FLEURS 103% trong báo cáo phostt) — mà video hướng dẫn phần mềm thường trộn Việt–Anh ("click vào nút Export", "mở file Excel"). Họ Whisper (kể cả PhoWhisper) nhiều khả năng xử lý code-switching tốt hơn. Cần tự benchmark trên mẫu thật của người dùng HP Screen trước khi chốt mặc định.
- Chênh lệch lớn giữa O-WER và N-WER trên leaderboard cho thấy khâu chuẩn hoá văn bản (số, viết hoa, dấu câu) ảnh hưởng mạnh; phụ đề cần dấu câu và viết hoa → mô hình transducer/CTC (Zipformer, Parakeet) thường cần thêm bước khôi phục dấu câu, còn Whisper xuất sẵn.
- Phụ đề "live trong khi quay" bằng tiếng Việt: sherpa-onnx không có model streaming tiếng Việt, nên cách khả thi là VAD + model offline theo từng đoạn nói (độ trễ khoảng 1–3 giây sau mỗi câu). Với trình quay màn hình, xử lý sau khi quay (post-processing) hợp lý hơn: chính xác hơn, không tranh CPU/GPU với encoder video, và cho phép sửa trước khi xuất.
- Timestamp mức từ: Whisper (whisper.cpp, faster-whisper `word_timestamps`) và WhisperX (forced alignment bằng wav2vec2) hỗ trợ; transducer của sherpa-onnx trả timestamp theo token. Đây là kiến thức nền, chưa đối chiếu nguồn trong lần nghiên cứu này.
- Xuất phụ đề: lưu SRT/VTT cạnh file; nhúng mềm vào MP4 bằng track `mov_text` (bật/tắt được, nhưng không phải trình phát/mạng xã hội nào cũng hiển thị); burn-in (render chữ vào khung hình) cần re-encode nhưng hiển thị chắc chắn ở mọi nơi và cho phép style kiểu "dynamic captions" (tô sáng từng từ). Nên hỗ trợ cả ba. Kiến thức nền về FFmpeg, chưa đối chiếu nguồn.
- Chi phí cloud: 1 giờ video ≈ 0,15 USD (AssemblyAI Universal-2) đến 0,36 USD (gpt-4o-transcribe/Whisper API); gpt-4o-mini-transcribe ≈ 0,18 USD/giờ.

### Gaps
- Không tìm được WER tiếng Việt chính thức cho: gpt-4o-transcribe/gpt-transcribe, Deepgram Nova-3, AssemblyAI Universal-2/3, Google Chirp, Azure Speech, Scribe v2, Gemini 3.x. Chỉ có số tự công bố của ElevenLabs cho Scribe v1.
- Không kiểm chứng được giá Google Cloud Speech-to-Text và Azure AI Speech trong lần này.
- Không có số RTF đáng tin cậy trên CPU cho large-v3-turbo, PhoWhisper, Zipformer-vi; không có benchmark whisper.cpp Vulkan trên Windows. Cần tự đo trên máy mục tiêu (CPU phổ thông, không GPU rời).
- NVIDIA Parakeet-TDT v3 / Canary: chưa xác minh danh sách ngôn ngữ có tiếng Việt hay không; chỉ xác nhận tồn tại `nvidia/parakeet-ctc-0.6b-Vietnamese` trên leaderboard. Licence và cách chạy trên Windows (ONNX) của model này chưa kiểm tra.
- distil-whisper (chủ yếu tiếng Anh), Vosk (có model tiếng Việt nhỏ), WhisperX: không tìm được số WER tiếng Việt; chưa kiểm chứng trong lần này.
- Licence của weights PhoWhisper (repo ghi BSD-3-Clause cho mã; không nêu riêng cho weights) và của Zipformer-30M-RNNT-6000h chưa xác nhận — cần đọc model card gốc trước khi dùng thương mại.
- Ngày deprecate các model transcribe cũ của OpenAI chỉ có nguồn bên thứ ba.

## 2. AI phân tích bản ghi

### Takeaway
Bộ tính năng AI đã thành "chuẩn" của nhóm Loom/Descript/Camtasia: tiêu đề, tóm tắt, chương tự động, xoá từ đệm và khoảng lặng, transcript tìm kiếm được, dịch/lồng tiếng; nhóm Scribe/Tango/Guidde tạo hướng dẫn từng bước từ log click + ảnh chụp. Tất cả đều dựng được bằng "transcript + log sự kiện + vài khung hình → LLM", với chi phí LLM ước tính dưới 0,01 USD/phút video nếu dùng model nhỏ.

### Cited Findings
- Loom AI: auto title (sinh từ transcript, có trên mọi gói kể cả Free), auto summary (điền vào ô mô tả sau khi quay), auto chapters (theo chủ đề), xoá filler words ("um", "ah", lặp từ) khỏi cả audio lẫn transcript, cắt khoảng lặng dài; phần lớn tính năng AI nằm ở gói Business + AI và Enterprise; summary/chapters chỉ áp dụng cho bản ghi sau khi mua, còn xoá filler/silence áp dụng được cho bản ghi cũ — [Loom AI features (Atlassian Support)](https://support.atlassian.com/loom/docs/loom-ai-features/)
- Giá Loom Business + AI: 24 USD/user/tháng (khoảng 20 USD nếu trả năm) theo một bài review; review cũng nhận xét summary hơi chung chung, tiêu đề chương máy móc, xoá filler đôi khi cắt nhầm mép từ thật — [beginnersinai Loom review 2026](https://beginnersinai.org/loom-review/)
- Camtasia/TechSmith: tính năng lõi của Audiate được gộp vào Camtasia Editor v25+; xoá "um/uh", khoảng dừng và từ lặp; Audiate add-on có AI script generation, AI voices, AI Smart Scenes (tự chia cảnh), AI avatars, AI translations, khử tạp âm nền; "Dynamic Captions" hiển thị từng từ khi được nói; TechSmith lưu ý độ chính xác thay đổi theo giọng/kiểu nói/người không bản ngữ — [Camtasia What's New](https://www.techsmith.com/camtasia/whats-new/?tier=1); [TechSmith: remove filler words](https://www.techsmith.com/blog/remove-filler-words-from-recorded-calls/); [Audiate add-on one sheet](https://www.techsmith.com/wp-content/uploads/2026/03/TechSmith-Audiate-Add-On-Sales-One-Sheet.pdf)
- Descript: bộ AI "Underlord" với hơn 30 công cụ; Studio Sound (làm sạch giọng), xoá filler words, AI dubbing (nguồn ghi 24+ hoặc 30+ ngôn ngữ — không thống nhất), dịch — [stylefactoryproductions Descript review 2026](https://www.stylefactoryproductions.com/blog/descript-review); [theneuron Descript](https://www.theneuron.ai/tools/descript/)
- Tella: quay màn hình + camera trên Mac/Windows/web, có AI agent chỉnh sửa chạy bằng credit hàng tháng; không có gói free, Pro 13 USD/user/tháng (trả năm) — [Tella: best AI video editors](https://www.tella.com/blog/best-ai-video-editors)
- FocuSee: một trang liệt kê bên thứ ba nói AI sinh phụ đề sau khi dừng quay; không tìm được trang chính thức mô tả chi tiết — [bestaitools FocuSee](https://www.bestaitools.com/tool/focusee)
- Scribe: extension/desktop app ghi lại click, phím gõ và thay đổi màn hình rồi dựng hướng dẫn gồm ảnh chụp đánh số + mô tả chữ; mô tả bước do AI sinh; che dữ liệu nhạy cảm là thủ công trừ gói cao — [Scribe vs Guidde (Scribe)](https://scribe.com/library/scribe-vs-guidde)
- Tango: extension Chrome/Edge ghi click, phím, điều hướng thành how-to guide; AI gợi ý tiêu đề và mô tả bước; mỗi click một ảnh chụp tự crop và chú thích (theo bài bên thứ ba) — [Tango vs Scribe](https://tango.us/compare/tango-vs-scribe); [dev.to so sánh công cụ workflow documentation](https://dev.to/pbinder_2e65897d5/scribe-vs-clicktrek-vs-tango-vs-guidde-vs-floik-workflow-documentation-tools-compared-2026-5077)
- Guidde: "Magic Capture" quay quy trình rồi AI dựng thành video hướng dẫn có voiceover + caption và tài liệu từng bước — nguồn là trang marketing của chính Guidde — [Guidde knowledge hub](https://www.guidde.com/knowledge-hub/scribe-vs-tango-feature-comparison-2026)
- Scribe, Tango, Guidde, Floik đều xử lý ảnh chụp/video trên server của họ (theo so sánh bên thứ ba) — [dev.to](https://dev.to/pbinder_2e65897d5/scribe-vs-clicktrek-vs-tango-vs-guidde-vs-floik-workflow-documentation-tools-compared-2026-5077)
- Các bài tổng hợp 2026 coi transcript tự động, thư viện video tìm kiếm được, chia chương thông minh và tóm tắt AI là tính năng "được kỳ vọng sẵn" chứ không còn là add-on cao cấp — [claap: best screen recorder guide 2026](https://claap.io/blog/best-screen-recorder)
- Giá LLM đa phương thức (USD / 1M token vào / ra, kiểm tra 2026-11-11):
  - Anthropic: Claude Haiku 5.5 0,10 / 0,50 (prompt ≤100k token; 0,50 / 2,50 nếu >100k); Claude Haiku 4.5 1 / 5; Claude Sonnet 5.5 2 / 10; Claude Opus 5.5 4 / 20; Claude Fable 5.2 8 / 40; Batch API giảm 50% — [Claude pricing](https://platform.claude.com/docs/en/about-claude/pricing)
  - OpenAI: gpt-5-nano 0,05 / 0,40; gpt-5-mini 0,25 / 2,00; gpt-5.4-nano 0,20 / 1,25; gpt-5.4-mini 0,75 / 4,50; gpt-5.6-luna 0,20 / 1,20; gpt-5.6-terra 2 / 12 — [OpenAI API pricing](https://developers.openai.com/api/docs/pricing)
  - Google: Gemini 2.5 Flash-Lite 0,10 / 0,40; Gemini 3.1 Flash-Lite 0,25 / 1,50; Gemini 3.5 Flash-Lite 0,30 / 2,50; Gemini 3.8 Flash 0,75 / 3,75 đến 31/12/2026 rồi tăng gấp đôi (1,50 / 7,50) từ 01/01/2027; Gemini 3.1 Pro Preview 2 / 12; Gemini 2.5 Pro/Flash tự chuyển sang model mới từ 01/12/2026 — [Gemini API pricing](https://aistudio.google.com/docs/pricing)

### Inferences
- Cách dựng bằng LLM API cho HP Screen:
  1. Tiêu đề / tóm tắt / chương: gửi transcript có timestamp (SRT) → yêu cầu JSON `{title, summary, chapters:[{start, title}]}`. Chỉ cần text, rất rẻ.
  2. Xoá filler và khoảng lặng: không cần LLM cho khoảng lặng (VAD/ngưỡng năng lượng từ audio); filler tiếng Việt ("ờ", "ừm", "à", "kiểu như", "thì là") dò bằng timestamp mức từ + danh sách từ, có thể nhờ LLM đánh dấu đoạn nói lặp/nói vấp. Luôn cho người dùng xem trước vì review Loom ghi nhận cắt nhầm.
  3. Hướng dẫn từng bước (kiểu Scribe/Tango): HP Screen đã có log click cho auto zoom → với mỗi click lấy 1 khung hình tại thời điểm đó, crop quanh điểm click, vẽ vòng đánh dấu, gửi (ảnh crop + câu transcript gần nhất + tên cửa sổ) cho model đa phương thức để viết "Bước N: Nhấn nút …". Xuất Markdown/HTML/PDF.
  4. Dịch phụ đề: dịch SRT theo lô, giữ nguyên timestamp. Lồng tiếng AI cần TTS tiếng Việt/Anh riêng (chưa nghiên cứu trong lần này).
  5. Khử ồn: làm local bằng mô hình khử nhiễu (ví dụ RNNoise/DeepFilterNet — kiến thức nền, chưa đối chiếu nguồn), không cần LLM.
- Ước tính chi phí (giả định, cần đo lại): video 10 phút tiếng Việt ≈ 4k token transcript; lấy 60 khung hình (1 khung/10 giây hoặc 1 khung/click), giả định ~1.300 token/ảnh (con số token mỗi ảnh CHƯA kiểm chứng, phụ thuộc độ phân giải và từng hãng) → ~82k token vào, ~1,5k token ra:
  - Claude Haiku 5.5: ≈ 0,009 USD/video ≈ 0,001 USD/phút
  - gpt-5-mini: ≈ 0,024 USD/video ≈ 0,0024 USD/phút
  - Gemini 3.5 Flash-Lite: ≈ 0,03 USD/video ≈ 0,003 USD/phút
  - Claude Sonnet 5.5: ≈ 0,18 USD/video ≈ 0,018 USD/phút
  - Chỉ transcript (không ảnh): dưới 0,03 USD/video ngay cả với Sonnet 5.5; dưới 0,002 USD với model nhỏ.
  - Cộng thêm STT cloud nếu dùng: 0,0025–0,006 USD/phút. Tức tổng "AI đầy đủ" bằng cloud khoảng 0,004–0,025 USD/phút tuỳ model.
- Giá Claude Haiku 5.5 (0,10/0,50) thấp hơn Haiku 4.5 mười lần — đúng theo trang giá chính thức nhưng nên xác nhận lại trước khi lập kế hoạch tài chính.
- Về quyền riêng tư: bản ghi màn hình thường chứa dữ liệu nhạy cảm; nên để AI cloud là tuỳ chọn opt-in, nêu rõ dữ liệu nào được gửi đi (transcript, khung hình), và tránh free tier của Gemini cho dữ liệu người dùng vì nội dung có thể được dùng để cải thiện sản phẩm.

### Gaps
- Không tìm được thông tin đáng tin về tính năng AI năm 2026 của ScreenPal, Clipchamp (chỉ có mô tả mơ hồ), Screen Studio và Cap (về phần AI/caption).
- Chưa kiểm chứng số token mỗi ảnh/mỗi giây video của Claude, OpenAI, Gemini → ước tính chi phí ở trên chỉ ở mức bậc độ lớn.
- Chưa nghiên cứu TTS/AI dubbing tiếng Việt (ElevenLabs, Azure, v.v.) và giá.
- Nguồn về Guidde/Scribe/Tango phần lớn là trang marketing so sánh lẫn nhau; cơ chế nội bộ không được công bố.

## 3. Auto zoom theo click

### Takeaway
Mọi sản phẩm (Screen Studio, FocuSee, Tella, Cap và các bản clone mã nguồn mở) dùng cùng một pipeline: quay video thô không zoom + log sự kiện chuột có timestamp, sau đó sinh các "zoom segment" quanh cụm click và render bằng hoạt ảnh spring; người dùng sửa segment trên timeline. Thông số cụ thể nhất được công bố nằm trong mã/PR của Cap và bản port "Screen Studio Effects".

### Cited Findings
- Screen Studio: zoom vào con trỏ ngay trước khi click xảy ra; nếu đoạn đó không có click, một trang hướng dẫn nói zoom sẽ bám theo chuyển động chuột, nhưng trang auto-zoom lại nói sẽ không zoom vào đâu và phải chuyển sang manual — hai trang chính thức mâu thuẫn — [Screen Studio: adding/editing zooms](https://screen.studio/guide/adding-editing-zooms); [Screen Studio: auto zoom](https://screen.studio/guide/auto-zoom)
- Screen Studio: mỗi zoom đặt được Auto hoặc Manual; Manual dùng khi muốn zoom vào vị trí khác điểm click hoặc đoạn không có click; tạo zoom bằng cách click vào vùng zoom trên timeline — [Screen Studio: manual zoom](https://screen.studio/guide/manual-zoom)
- Screen Studio: motion blur có thanh trượt cường độ và chỉnh riêng cho 3 loại: chuyển động con trỏ, zoom in/out, và màn hình di chuyển khi đang zoom; tốc độ hoạt ảnh con trỏ có 4 mức Smooth / Medium / Rapid / None; hoạt ảnh màn hình có 2 kiểu: "Focused" (ổn định nhanh, dễ đọc) và "Smooth" (mượt hơn, cho nội dung sáng tạo) — [Screen Studio: Animations](https://screen.studio/guide/animations)
- Screen Studio tự làm mượt chuyển động con trỏ để bỏ rung/giật và cho chỉnh cỡ con trỏ; review nêu Manual zoom cho chọn vùng, mức zoom và "glide" (độ trôi của camera ảo khi đang zoom) — [daveswift Screen Studio review](https://daveswift.com/screen-studio/)
- "Screen Studio Effects" (MIT, port thuật toán spring và nội suy zoom từ Cap) mô tả pipeline 5 bước: lọc rung (shake filter) → densify (lấp khoảng trống thời gian) → mô phỏng spring làm mượt quỹ đạo → dò auto-zoom → tính crop từng khung hình bằng viewport spring — [Screen-Studio-Effects](https://github.com/pythonlearner1025/Screen-Studio-Effects)
- Cùng nguồn, thông số spring (dùng nghiệm giải tích của dao động tắt dần, không dùng Euler/RK4 → ổn định và độc lập frame rate; ζ = friction / (2·√(tension·mass))):
  - Con trỏ "default": tension 170, mass 1,0, friction 20 (ζ≈0,77)
  - Con trỏ "snappy" (trong vòng 160 ms quanh click): tension 700, mass 1,0, friction 30 (ζ≈0,57)
  - Con trỏ "drag" (khi kéo chuột): tension 136, mass 1,2, friction 26 (ζ≈1,02)
  - Viewport "focused": tension 300, mass 6,75, friction 120 (ζ≈1,33)
  - Viewport "smooth": tension 240, mass 3,375, friction 80 (ζ≈1,40)
  — [Screen-Studio-Effects](https://github.com/pythonlearner1025/Screen-Studio-Effects)
- Cùng nguồn, bám con trỏ khi đang zoom: viewport khoá khi vào segment; chỉ pan khi con trỏ rời "vùng an toàn" 70% bên trong; nhìn trước 1 giây và bỏ qua pan nếu con trỏ sẽ quay lại vùng an toàn; mục tiêu pan là trung bình vị trí con trỏ trong 0,5 giây tương lai. Phát hiện "đứng yên": dịch chuyển dưới 2 px trong ít nhất 0,5 giây. Crop xuất ra dạng `{x, y, w, h}` chuẩn hoá 0–1 (zoom 2× ở giữa = `{0.25, 0.25, 0.5, 0.5}`); segment thủ công có `sourceStart`, `sourceEnd`, `amount`, `manualCenter` tuỳ chọn; ví dụ dùng amount 2,0 và 1,5 — [Screen-Studio-Effects](https://github.com/pythonlearner1025/Screen-Studio-Effects)
- Cap (mã nguồn mở, MIT theo tài liệu dự án): timeline có zoom segment thủ công hoặc tự sinh từ click — [Cap open source page](https://cap.so/open-source-screen-recorder)
- Cap PR #1663 "configurable auto-zoom settings" (mở 18/03/2026, ĐÓNG không merge 20/10/2026 vì app desktop đã chuyển sang bản native GPUI; PR chỉ sửa bản Tauri cũ) hé lộ các hằng số mặc định của bộ sinh segment: mức zoom 1,5× (slider 1–4×); ngưỡng thời gian gom nhóm click 2,5 s; ngưỡng không gian gom nhóm 0,15 (toạ độ chuẩn hoá); đệm trước click 0,4 s; đệm sau click 1,8 s; ngưỡng gộp khoảng trống giữa segment 0,8 s; thời lượng segment tối thiểu 1,0 s; ngưỡng khoảng cách cửa sổ chuyển động 0,08 (dải 0,02–0,2); ngưỡng sự kiện di chuyển 0,02; còn có các hằng `STOP_PADDING_SECONDS`, `MOVEMENT_WINDOW_SECONDS`, `SHAKE_FILTER_THRESHOLD`, `SHAKE_FILTER_WINDOW_MS`. File liên quan: `crates/project/src/configuration.rs`, `apps/desktop/src-tauri/src/recording.rs` — [Cap PR #1663](https://github.com/CapSoftware/Cap/pull/1663)
- Các PR liên quan của Cap (đọc qua bản mirror, CHƯA xác nhận trạng thái merge): khử trùng double-click và lọc right-click trước khi gom nhóm; mức zoom thích ứng — cụm click sát nhau zoom tới 2,5×, cụm trải rộng giảm còn 1,2× (nội suy tuyến tính theo khoảng cách cặp điểm lớn nhất); `dead_zone_radius` gộp các click gần nhau về không gian thành một segment dù cách xa về thời gian; mã nội suy ở `crates/rendering/src/zoom_focus_interpolation.rs` — [Cap PR #1664 (mirror)](https://gh.hhj520.top/CapSoftware/Cap/pull/1664); [Cap PR #1666 (mirror)](https://gh.hhj520.top/CapSoftware/Cap/pull/1666); [Cap PR #1667 (mirror)](https://gh.hhj520.top/CapSoftware/Cap/pull/1667)
- Tella (API công khai `POST /v1/videos/{id}/clips/{clipId}/zooms/auto`): chỉ dùng dữ liệu click chuột của bản ghi màn hình; mỗi click mở một cửa sổ zoom, các cửa sổ gần nhau được gộp; preset cường độ: gentle 1,5× (ease-in mềm), subtle 1,5× (chậm, mượt), moderate 2× (mặc định), focused 2× (ổn định nhanh), intense 2,5× (nhanh, gắt); `scale` ghi đè từ 1 đến 3,5; sinh lại sẽ thay thế zoom tự động cũ nhưng không đụng zoom thủ công; mỗi zoom có `startTimeMs`, `durationMs`, `scale`, `introTransition`, `outroTransition` — [Tella API: auto-generate zooms](https://www.tella.com/docs/api-reference/clips/auto-generate-zooms-from-mouse-activity.md)
- FocuSee (Windows + Mac): tự zoom mỗi khi có click; "zoom track" hiển thị các zoom sinh từ chuyển động chuột và click trong lúc quay; bấm vào khối zoom để tắt, đổi mức zoom, hoặc áp dụng thiết lập cho mọi đoạn zoom; thêm zoom thủ công bằng cách click lên zoom timeline; tab Cursor chỉnh cỡ, kiểu, màu, hiệu ứng click, âm thanh — [FocuSee guide: zoom-in effect](https://focusee.imobie.com/guide/zoom-in-effect.htm); [FocuSee guide: cursor effect](https://focusee.imobie.com/guide/cursor-effect.htm)
- Camtasia: có Cursor Editing cho phép làm mượt, đổi cỡ, đặt lại vị trí đường đi con trỏ sau khi quay (tức con trỏ được lưu thành dữ liệu riêng, không dính vào video) — [Camtasia What's New](https://www.techsmith.com/camtasia/whats-new/?tier=1)
- Bản clone mã nguồn mở chạy Windows:
  - Recordly (AGPL-3.0; Electron + PixiJS; helper native dùng Windows Graphics Capture + WASAPI; Windows 10 build 19041+): gợi ý zoom theo hoạt động con trỏ + vùng zoom thủ công; con trỏ render lại thay con trỏ thật với smoothing, motion blur, click bounce, sway, loop; nền/padding/bo góc/đổ bóng; webcam bubble; timeline trim/speed/annotation; xuất MP4 và GIF; preset tỉ lệ khung — [Recordly (bản fork)](https://github.com/nonomal/Recordly)
  - FlowCap (MIT; Tauri 2 + React; `getDisplayMedia()` → canvas 60 fps → `MediaRecorder`; FFmpeg WASM xuất MP4/GIF): zoom spring bám con trỏ khi đứng yên và zoom ra khi chuột di chuyển; poll vị trí con trỏ ~60 lần/giây qua lệnh native `get_cursor_pos`; zoom được áp trong compositor canvas ngay khi quay nên bị "nướng" vào video — [FlowCap](https://github.com/m1keology/FlowCap)
  - Trên macOS: Screenize (Apache 2.0, log vị trí con trỏ + click cạnh bản ghi rồi sinh keyframe zoom) và Reframed (dò cụm click, dựng keyframe theo thời gian dừng) — [Screenize](https://github.com/syi0808/screenize); [Reframed](https://github.com/jkuri/reframed)

### Inferences
- Pipeline đề xuất cho HP Screen (hậu kỳ, không phá huỷ):
  1. Khi quay: capture màn hình với con trỏ hệ thống ẩn khỏi khung hình (Windows Graphics Capture có tuỳ chọn không vẽ con trỏ — kiến thức nền), đồng thời ghi file sự kiện (JSON/binary) gồm `t_ms, x, y, loại (move/down/up/wheel/key), nút, hình dạng con trỏ, cửa sổ đang active`, dùng cùng đồng hồ với video. Lấy mẫu vị trí ≥60 Hz (FlowCap poll 60 Hz) hoặc dùng low-level mouse hook.
  2. Sinh segment (theo hằng số của Cap): bỏ double-click trùng và (tuỳ chọn) right-click; gom click cách nhau < 2,5 s và < 0,15 chiều rộng màn hình thành một nhóm; segment = [click đầu − 0,4 s, click cuối + 1,8 s]; gộp hai segment cách nhau < 0,8 s; bỏ segment < 1,0 s; mức zoom mặc định 1,5–2× (Cap 1,5×; Tella 2×), có thể thích ứng 1,2–2,5× theo độ trải của cụm click.
  3. Render: mỗi khung hình tính crop bằng spring quá giảm chấn cho viewport (ζ≈1,3–1,4, không nảy) và spring hơi nảy cho con trỏ (ζ≈0,77); khi đang zoom chỉ pan khi con trỏ ra khỏi 70% vùng giữa; vẽ lại con trỏ vector phóng to + motion blur tuỳ chọn.
  4. Timeline: hiển thị track zoom; kéo mép để đổi thời lượng, click để đổi mức zoom / chuyển Auto↔Manual (chọn tâm), nút "sinh lại" chỉ thay zoom tự động, giữ zoom thủ công (như Tella).
- Live (zoom ngay khi quay, kiểu FlowCap/ZoomIt) so với hậu kỳ:
  - Live: không cần editor, xuất file ngay, hợp livestream/họp; nhưng zoom bị nướng vào video (không sửa được, mất vùng ngoài khung), không thể "zoom trước click 0,4 s" vì không biết trước tương lai (nên luôn trễ), không lookahead để tránh pan thừa, và tốn GPU/CPU lúc quay.
  - Hậu kỳ: sửa được mọi thứ, zoom đón trước click, làm mượt con trỏ bằng dữ liệu tương lai, đổi tỉ lệ khung sau; đổi lại cần bước render/export và cần editor. Toàn bộ sản phẩm "đẹp" (Screen Studio, FocuSee, Tella, Cap, Recordly) chọn hậu kỳ.
  - Khuyến nghị: hậu kỳ là chính; có thể thêm chế độ "xuất nhanh" tự áp auto zoom mặc định không cần mở editor.
- Vì quay ở độ phân giải gốc rồi crop, zoom 2× trên màn 1080p chỉ còn 960×540 điểm ảnh thật → nên quay ở độ phân giải gốc cao nhất và giới hạn zoom mặc định ≤2× để tránh mờ.

### Gaps
- Screen Studio là mã đóng: không có mô tả kỹ thuật chính thức về thuật toán; thông số spring ở trên là của bản tái dựng cộng đồng/Cap, không chắc trùng với Screen Studio.
- Chưa đọc trực tiếp mã nguồn hiện hành của Cap (bản GPUI mới); các hằng số lấy từ một PR đã đóng và các PR mirror → có thể đã thay đổi.
- Không tìm được tài liệu về Cursorful, Rapidemo, và về SmartFocus của Camtasia (chỉ có nhắc tên ở nguồn bên thứ ba, chưa xác nhận ở tài liệu TechSmith).
- Chưa có nguồn cho ngưỡng shake filter, tham số densify và thuật toán motion blur cụ thể.
- Chưa kiểm chứng bằng tài liệu Microsoft về cách ẩn con trỏ trong Windows Graphics Capture và độ trễ của low-level hook; cần thử nghiệm.

## 4. Hiển thị click chuột và overlay phím bấm

### Takeaway
Quy ước phổ biến: vòng tròn/gợn sóng (ripple) tại điểm click với màu khác nhau cho trái/phải, quầng sáng (highlight/spotlight) quanh con trỏ, và "chip" phím hiển thị tổ hợp phím ở góc màn hình. Có hai trường phái: vẽ overlay thật lên màn hình lúc quay (PowerToys, Keyviz, KeyCastOW) và chỉ ghép vào video (Bandicam, Screen Studio, FocuSee, Recordly).

### Cited Findings
- Bandicam (Video > Settings > Mouse): hiệu ứng click là hình tròn xuất hiện mỗi lần click, chỉnh được cỡ, màu riêng cho trái và phải — mặc định đỏ (trái) và xanh dương (phải); highlight là quầng quanh con trỏ chỉnh cỡ/màu/độ trong, mặc định vàng; âm thanh click bật riêng cho trái/phải, thay được bằng file `lclick.wav` / `rclick.wav`; hiệu ứng click và highlight chỉ xuất hiện trong video, không hiện trên màn hình lúc quay — [Bandicam: mouse effects](https://www.bandicam.com/guide/overlay-cursor/)
- PowerToys Mouse Highlighter (bật/tắt mặc định Win+Shift+H): 3 chế độ Spotlight / Circle highlight / Ripple (Ripple là mặc định ở profile mới); màu nút chính mặc định `#A6BFFF00`, nút phụ `#A600BFFF` (trang Microsoft mô tả là xanh lá và xanh dương; mã màu ARGB với alpha A6); có "Always highlight color" (quầng thường trực) cho Spotlight/Circle; Radius tối thiểu 5 px; Fade delay và Fade duration (ms); với Ripple: Size mặc định 60 px (10–300), Intensity 0,7 (0,15–1,35), Duration 480 ms (60–2000), "Follow cursor while held" (gợn bám con trỏ khi giữ nút/kéo, mặc định bật), "Show crosshairs on right-click release" (mặc định bật) — [PowerToys Mouse utilities](https://learn.microsoft.com/en-us/windows/powertoys/mouse-utilities)
- PowerToys Find My Mouse: spotlight quanh con trỏ, kích hoạt bằng nhấn Ctrl trái/phải hai lần, lắc chuột hoặc phím tắt; nền mặc định `#000000`, spotlight `#FFFFFF`, bán kính 100 px, hoạt ảnh 500 ms; có danh sách app loại trừ và tắt khi Game Mode — [PowerToys Mouse utilities](https://learn.microsoft.com/en-us/windows/powertoys/mouse-utilities)
- PowerToys Mouse Pointer Crosshairs: vẽ chữ thập tâm tại con trỏ; opacity mặc định 75%, bán kính tâm 20 px, độ dày 5 px, hướng ngang/dọc/cả hai — [PowerToys Mouse utilities](https://learn.microsoft.com/en-us/windows/powertoys/mouse-utilities)
- Keyviz (GPL-3.0; Tauri + Rust; Windows .msi, macOS .dmg, Linux X11 tự build): hiển thị phím bấm và thao tác chuột theo thời gian thực, kể cả tổ hợp như Cmd + Click, Alt + Drag; có vệt lịch sử các phím gần nhất; hiện click và cuộn bánh xe ngay cạnh con trỏ; tuỳ biến màu (riêng cho phím modifier và phím thường), cỡ, bố cục, viền, nền, hoạt ảnh vào/ra, vị trí; lọc phím hiển thị bằng "hotkey filter" hoặc bộ lọc tuỳ chỉnh — [Keyviz GitHub](https://github.com/mulaRahul/keyviz)
- KeyCastOW: trình hiển thị phím cho Windows, miễn phí, mã nguồn mở, portable; có review nói gợn click chuột bị trễ — [AlternativeTo: KeyCastOW](https://alternativeto.net/software/keycastow/about)
- Carnac: trình hiển thị phím cho Windows; người dùng báo lỗi đa màn hình, không được bảo trì nhiều năm, không xử lý dấu (diacritics) và hay crash — [AlternativeTo: Carnac](https://alternativeto.net/software/carnac/)
- KeyCastr: overlay phím + highlight chuột, thiên về Mac — [AlternativeTo: KeyCastr](https://alternativeto.net/software/keycastr/)
- FocuSee: tab Cursor chỉnh cỡ, kiểu, màu, hiệu ứng click và âm thanh; một trang liệt kê nêu 8 kiểu con trỏ và 3 hiệu ứng click — [FocuSee guide: cursor effect](https://focusee.imobie.com/guide/cursor-effect.htm); [bestaitools FocuSee](https://www.bestaitools.com/tool/focusee)
- Recordly: con trỏ được render lại (thay con trỏ thật) với click bounce (con trỏ nảy khi click), sway, smoothing, motion blur, chỉnh cỡ; âm thanh click chỉ có dạng extension cộng đồng — [Recordly (bản fork)](https://github.com/nonomal/Recordly)
- Screen-Studio-Effects: spring con trỏ chuyển sang profile "snappy" trong 160 ms quanh click và profile "drag" khi kéo — tức click được thể hiện qua động học con trỏ chứ không chỉ bằng vòng tròn — [Screen-Studio-Effects](https://github.com/pythonlearner1025/Screen-Studio-Effects)
- Screen Studio có mục hướng dẫn "Mouse click sound", "Disable smooth mouse movement", "Hiding Cursor in Specific Sections" (chỉ thấy tiêu đề mục, chưa đọc nội dung) — [Screen Studio Guide](https://www.screen.studio/help)

### Inferences
- Bảng quy ước nên áp dụng cho HP Screen:
  - Click trái: vòng ripple lan ra rồi mờ dần (~300–500 ms; PowerToys mặc định 480 ms), một màu chủ đạo (Bandicam đỏ, PowerToys xanh lá).
  - Click phải: cùng hình nhưng màu khác (xanh dương ở cả Bandicam và PowerToys → xanh dương cho click phải là quy ước khá thống nhất).
  - Click giữa và cuộn: không có quy ước màu thống nhất trong các nguồn tìm được; Keyviz hiện biểu tượng cạnh con trỏ. Đề xuất: icon hình con chuột nhỏ cạnh con trỏ tô sáng nút đang nhấn / mũi tên lên-xuống khi cuộn.
  - Giữ nút/kéo: vòng bám theo con trỏ cho tới khi nhả (PowerToys "Follow cursor while held").
  - Highlight thường trực: đĩa mờ màu vàng quanh con trỏ (mặc định của Bandicam) hoặc spotlight làm tối phần còn lại.
  - Phím: chip bo góc ở giữa-dưới hoặc góc dưới, gộp tổ hợp ("Ctrl + Shift + S"), tự mờ sau 1–2 giây; chế độ "chỉ phím tắt" (chỉ hiện khi có modifier) nên là mặc định để không lộ nội dung gõ/mật khẩu.
- Live overlay so với ghép lúc export:
  - Live overlay (cửa sổ trong suốt, topmost, click-through, vẽ lên màn hình): người quay thấy đúng cái sẽ vào video, hoạt động cả khi livestream/họp, đơn giản; nhưng bị nướng vào video, không sửa/tắt được sau, và có thể trễ (KeyCastOW bị chê ripple trễ).
  - Ghép khi export (kiểu Bandicam/Screen Studio): cần log sự kiện (đã có sẵn cho auto zoom), cho đổi màu/kiểu/tắt sau khi quay, hiệu ứng scale đúng theo zoom, không ảnh hưởng hiệu năng lúc quay. Vì HP Screen đã cần log sự kiện cho auto zoom, ghép khi export là lựa chọn tự nhiên; có thể thêm live overlay tuỳ chọn cho người quay "một phát ăn ngay".
- Lưu ý riêng cho người dùng Việt: bộ gõ Unikey/EVKey/Telex sinh thêm phím Backspace và ký tự giả lập khi gõ dấu → overlay phím thô sẽ hiện chuỗi rối. Chế độ "chỉ phím tắt" né được vấn đề; Carnac bị chê không xử lý dấu cho thấy đây là điểm dễ sai. (Suy luận, chưa có nguồn trực tiếp về tương tác Unikey với các công cụ này.)
- Bảo mật: low-level keyboard hook có thể bị phần mềm diệt virus nghi là keylogger; nên chỉ hook khi đang quay, không lưu phím thường ra đĩa nếu ở chế độ chỉ-phím-tắt, và có ký số ứng dụng. (Suy luận.)

### Gaps
- Không tìm được nguồn trong lần này cho ZoomIt (zoom/vẽ/DemoType), Presentify (macOS), ShareX (tuỳ chọn hiển thị click khi quay) và phần hiệu ứng con trỏ chi tiết của Camtasia (cursor highlight/magnify/spotlight, click rings/ripple/scope) — cần đọc tài liệu chính thức của từng sản phẩm.
- Không có nguồn cho quy ước hiển thị click giữa và cuộn bánh xe.
- Chưa kiểm chứng tài liệu Win32 (WH_MOUSE_LL / WH_KEYBOARD_LL, Raw Input, layered window click-through) trong lần này; đây là kiến thức nền cần đối chiếu khi thiết kế.
- Số "8 kiểu con trỏ, 3 hiệu ứng click" của FocuSee lấy từ trang liệt kê bên thứ ba.

## 5. Chỉnh sửa nhanh sau khi quay

### Takeaway
Bộ tính năng editor lặp lại ở hầu hết sản phẩm kiểu Screen Studio (kể cả bản mã nguồn mở) là: trim/cắt, tốc độ, nền + padding + bo góc + đổ bóng, webcam bubble, zoom segment, annotation, preset tỉ lệ khung, xuất MP4 và GIF. Các bài review 2026 nhấn mạnh thêm transcript/caption và tóm tắt AI như kỳ vọng mặc định; không tìm được bài review nào xếp hạng rõ "must-have vs nice-to-have" cho từng tính năng chỉnh sửa.

### Cited Findings
- Recordly (clone mã nguồn mở, có bản Windows) gói đúng bộ tính năng "chuẩn thể loại": timeline với trim, vùng tốc độ (speed regions), annotation, audio; nền wallpaper/màu/gradient/blur, padding, bo góc, drop shadow; webcam bubble; preset tỉ lệ khung + chỉnh kích thước đầu ra; xuất MP4 và GIF (chọn chất lượng, frame rate GIF, lặp, preset kích thước GIF) — [Recordly (bản fork)](https://github.com/nonomal/Recordly)
- FlowCap (Windows, MIT): nền gradient, padding, bo góc, spotlight, webcam overlay, trim editor, xuất MP4 / GIF / WebM; quảng bá "không subscription, không watermark, không upload cloud" — [FlowCap](https://github.com/m1keology/FlowCap)
- Cap: editor mở ngay sau khi dừng quay; timeline hỗ trợ zoom segment thủ công hoặc tự động — [Cap open source page](https://cap.so/open-source-screen-recorder)
- FocuSee: chỉnh zoom theo từng đoạn, áp thiết lập cho tất cả đoạn, tuỳ biến con trỏ; định vị là trình quay auto-zoom cho Windows và Mac — [FocuSee guide: edit the recording](https://focusee.imobie.com/guide/edit-the-recording.htm)
- Zapier (hướng dẫn 03/2026) chọn Camtasia cho chỉnh sửa nâng cao; lưu ý có sản phẩm giới hạn gói free ở bản ghi dưới 1 phút, gói trả phí mới có chỉnh sửa, CTA, transcription và caption — [Zapier: best screen recording software](https://zapier.com/blog/best-screen-recording-software)
- Tella liệt kê Tella, Camtasia, Screen Studio, Cap và ScreenPal là nhóm có sẵn chỉnh sửa; Screen Studio được khen cho demo macOS bóng bẩy, Loom cho tin nhắn công việc bất đồng bộ; OBS miễn phí, mạnh nhưng chỉnh sửa hạn chế — nguồn là blog của một đối thủ, cần cân nhắc thiên lệch — [Tella: best screen recording software 2026](https://www.tella.com/blog/best-screen-recording-software)
- Transcript tự động, thư viện tìm kiếm được, chia chương thông minh và tóm tắt AI đang thành tính năng được kỳ vọng sẵn — [claap: best screen recorder guide 2026](https://claap.io/blog/best-screen-recorder)
- Screen Studio: motion blur, kiểu hoạt ảnh, ẩn con trỏ ở một số đoạn, âm thanh click là các mục trong phần "Look & Feel" của hướng dẫn — [Screen Studio Guide](https://www.screen.studio/help)

### Inferences
- Phân loại đề xuất (suy luận từ độ phổ biến của tính năng trong các sản phẩm đã khảo sát, không phải từ một bảng xếp hạng review):
  - Must-have: trim đầu/cuối và cắt đoạn giữa; crop; nền + padding + bo góc + đổ bóng (dấu hiệu nhận diện của thể loại "Screen Studio-like"); sửa/xoá zoom segment; bật/tắt và đổi kiểu hiệu ứng click; preset tỉ lệ 16:9, 9:16, 1:1, 4:5; xuất MP4 (H.264) không watermark; phụ đề tự động có sửa tay.
  - Nên có sớm: tốc độ theo đoạn (tua nhanh đoạn chờ), webcam bubble (tròn/bo góc, đổi vị trí), blur/che vùng nhạy cảm (quan trọng với video hướng dẫn có thông tin cá nhân), annotation (mũi tên, khung, chữ), xuất GIF, xoá khoảng lặng tự động.
  - Nice-to-have: motion blur, âm thanh click, con trỏ thay thế/đổi kiểu, lồng tiếng/dịch AI, xuất WebM, chia sẻ link cloud, tạo hướng dẫn từng bước.
- Với 9:16 và 1:1, auto zoom càng quan trọng vì khung hẹp không hiển thị nổi cả màn hình ngang: zoom segment nên kiêm vai trò "camera bám con trỏ" để tự reframe khi đổi tỉ lệ.
- "Không watermark, không cần tài khoản, xử lý local" là điểm bán hàng mà các clone mã nguồn mở (FlowCap, Recordly, Cap) nhấn mạnh → HP Screen nên giữ các tính năng lõi chạy offline.

### Gaps
- Không tìm được review độc lập nào đánh giá cụ thể blur vùng nhạy cảm, padding/nền, preset tỉ lệ khung hay xuất GIF theo mức must-have/nice-to-have; phân loại ở trên là suy luận.
- Chưa khảo sát chi tiết editor của ScreenPal, Clipchamp, ShareX, Bandicam (Bandicut) và Camtasia về từng tính năng chỉnh sửa.
- Chưa có dữ liệu về nhu cầu riêng của người dùng Việt (TikTok/Facebook Reels/Zalo, tỉ lệ khung và độ dài ưa dùng).
