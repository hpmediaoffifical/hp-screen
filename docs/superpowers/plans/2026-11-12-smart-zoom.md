# Zoom thông minh (bước 5b) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Zoom bằng tay qua một khung kéo thả trên video, có điểm lia ◆, tốc độ và các phím Z/K; zoom tự động khôn hơn với ba mức độ.

**Architecture:**
- `hp-core::zoom` có thêm `ZoomKey`, `Speed`, `Level`.
- `camera_track` nội suy giữa các điểm ◆ rồi cho camera đi theo bằng lò xo, co giãn theo tốc độ.
- `auto_segments` nhận `AutoInput`, gồm click, cuộn, gõ, con trỏ và chặn mép.
- Phép tính khung kéo thả nằm trong `zoomMath.ts`.
- `Preview` vẽ lớp khung khi ở chế độ chỉnh khung.
- Bộ quay ghi dấu `type`.

**Tech Stack:** Rust (hp-core, Tauri 2), React/TS, `node --test`.

**Spec:** `docs/superpowers/specs/2026-11-12-smart-zoom-design.md`

## Global Constraints

- Mọi giá trị là hằng có tên trong `zoom.rs`:

  | Thông số | Giá trị |
  |---|---|
  | Hệ số tốc độ | Chậm 0,6; Vừa 1; Nhanh 2; Tức thì là nhảy thẳng tới đích |
  | Điểm ◆ | cách nhau hơn 50 ms |
  | Tâm | 0–1 |
  | Mức phóng | 1–4 |
  | Phím Z | đoạn 3000 ms, mức 1,5×, không thêm nếu còn dưới 500 ms |
  | Mức phóng tự động | `clamp(min(2.0, 1/(m+0.2)), 1.25, 2.0)` |
  | Chặn mép | 4% |
  | Giữ sau thao tác | Ít 1800 ms, Vừa 1800 ms, Nhiều 1200 ms |
  | Ít: thao tác đi kèm | gõ hoặc cuộn trong 2000 ms sau click |
  | Gõ hoặc cuộn | giữ thêm 1000 ms, tối đa 20000 ms sau click cuối |
  | Trở ra sớm | ở ngoài 1000 ms |
  | Dấu `type` | tối đa một dòng mỗi 300 ms |

- File chỉnh sửa vẫn là `v: 1`. Các trường mới (`keys`, `speed`, `level`) có `serde(default)`, nên file cũ đọc ra đúng như trước.
- Nhật ký vẫn là `LOG_VERSION = 1`. Dòng `{"e":"type","t":ms}` không bao giờ ghi phím nào.
- Không sửa `SnipOverlay.tsx`, `snip.rs`. Các file dùng chung chỉ được thêm vào.
- Không giả lập chuột hay phím trên máy người dùng.
- Commit kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. Dời cả một đoạn trên rãnh Zoom: các điểm ◆ đi theo đoạn, không bị bỏ. Kéo mép đoạn qua một điểm: điểm đó bị bỏ. Test `movingASegmentMovesItsKeys` ở Task 4.
2. Bấm Z khi còn dưới 0,5 s tới cuối video, hoặc tới đoạn sau: không thêm gì. Test `zAtTheEndAddsNothing` ở Task 4.
3. Kéo góc khung ra quá mép hình, hoặc nhỏ hơn mức 4×: khung bị giới hạn, không lật ngược. Test `resizeIsClampedAndNeverFlips` ở Task 4.
4. Đổi "Vừa khung" ↔ "Lấp đầy" sau khi đã đặt khung: khung vẫn đúng chỗ, vì lưu theo tâm và mức phóng. Test `frameRoundTripsInFillMode` ở Task 4.
5. Đoạn có điểm ◆ nằm một phần trong vùng đã cắt bỏ: bản xuất không lỗi, và camera tại các khung còn lại vẫn đúng. Test `keyframes_inside_a_cut_export` ở Task 5.

---

### Task 1: Điểm ◆, tốc độ và đường đi camera (`hp-core::zoom`)

**Files:**
- Modify: `crates/hp-core/src/zoom.rs`
- Modify: `crates/hp-core/src/spring.rs`, chỉ để thêm `Spring::scaled(f: f32) -> Spring`

**Interfaces:**
- Produces:
  - `ZoomKey { t_ms: i64, cx: f32, cy: f32, scale: f32 }`, Serialize/Deserialize.
  - `enum Speed { Slow, Normal, Fast, Instant }`, serde lowercase, mặc định `Normal`, có `fn factor(self) -> Option<f32>` (Instant trả `None`).
  - `enum Level { Low, Normal, High }`, serde `"low" | "normal" | "high"`, mặc định `Normal`.
  - `ZoomSegment.keys: Vec<ZoomKey>`, `ZoomSegment.speed: Speed`, `ZoomDoc.level: Level`.
  - `pub fn target_at(seg: &ZoomSegment, t_ms: i64) -> Option<Camera>`: đích theo các điểm; `None` khi đoạn đi theo con trỏ và không có điểm.
- `camera_track` giữ nguyên chữ ký.

- [ ] **Step 1: Viết test trước, trong `zoom.rs`**

```rust
#[test] fn old_segments_read_with_defaults()        // JSON không có keys/speed/level → keys rỗng, Normal, Normal; camera_track y như trước (so với track của cùng doc)
#[test] fn keys_are_reached_at_their_time()         // đoạn 0–6000, khung chính (0.25,0.25,2), điểm t=3000 (0.75,0.75,2), speed Fast: |cx-0.75|,|cy-0.75| < 0.02 tại t=3000+200
#[test] fn target_eases_between_keys()               // target_at tại giữa P0–P1 = trung điểm; tại 1/4 khoảng < 1/4 quãng (smoothstep); scale nội suy theo log: tại giữa 1→4 bằng 2
#[test] fn instant_jumps_to_the_target()            // speed Instant: camera tại start_ms+1 = khung chính; tại t_key+1 = điểm; không nội suy giữa hai điểm
#[test] fn slow_is_slower_and_fast_is_faster()      // thời gian tới 90% của mức 2×: Fast < Normal < Slow
#[test] fn normalized_sorts_and_drops_bad_keys()    // ngoài (start,end) bị bỏ; cách < 50 ms bị bỏ; scale bị kẹp vào 1–4; center None + keys → center = tâm điểm đầu
#[test] fn a_split_segment_keeps_keys_in_each_part() // đoạn A 0–6000 có điểm 1000 và 5000, đoạn B 2000–3000 đè lên: mảnh 0–2000 có điểm 1000, mảnh 3000–6000 có điểm 5000 và khung chính bằng target_at(A, 3000)
```

- [ ] **Step 2: Chạy, phải FAIL**

Run: `cargo test -p hp-core zoom`
Expected: biên dịch lỗi vì chưa có `ZoomKey` / `Speed` / `target_at`.

- [ ] **Step 3: Làm**

- `Spring::scaled(f)` trả về tension × f², friction × f, mass giữ nguyên.
- `target_at`:
  - P0 là (start_ms, center, scale); sau đó lần lượt là các `keys`.
  - Giữa Pi và Pi+1, `u = smoothstep((t-ti)/(ti+1-ti))`. Tâm nội suy tuyến tính theo u; mức phóng là `exp(lerp(ln si, ln si+1, u))`.
  - Sau điểm cuối thì giữ nguyên điểm cuối.
  - Instant không nội suy: lấy Pi với ti ≤ t.
- `camera_track`:
  - Trong đoạn mà `target_at` trả `Some`, dùng đích đó; còn lại giữ cách đi theo con trỏ như hiện nay.
  - Lò xo là `CAMERA.scaled(f)` theo tốc độ của đoạn hiện tại, hoặc của đoạn vừa kết thúc khi đang trở ra.
  - Với Instant: gán thẳng giá trị bằng đích và vận tốc bằng 0.
- `normalized`:
  - xử lý `keys` như test;
  - khi tách đoạn, mảnh sau có khung chính là `target_at(gốc, a)` (giữ tâm `None` nếu gốc đi theo con trỏ và không có điểm).

- [ ] **Step 4: Chạy, phải PASS**

Run: `cargo test -p hp-core`
Expected: tất cả pass, kể cả 22 test zoom cũ.

- [ ] **Step 5: Commit** `feat(core): zoom keyframes, speed and eased camera between them`

### Task 2: Zoom tự động khôn hơn (`hp-core::zoom`, `effects`, `input_log`)

**Files:**
- Modify: `crates/hp-core/src/zoom.rs`, `crates/hp-core/src/effects.rs`, `crates/hp-core/src/input_log.rs`
- Modify: `src-tauri/src/edit_cmds.rs` (`auto_zoom`)

**Interfaces:**
- Consumes: `ZoomKey`, `Level` (Task 1).
- Produces:
  - `Event::Type { t: i64 }`, dạng JSON `{"e":"type","t":ms}`.
  - `Effects.typing: Vec<i64>`, `Effects.edge_guard: bool` (true khi `header.mode == "monitor"`), `Effects.wheel_times: Vec<i64>`. Các trường này được serialize ra TS, giao diện có thể bỏ qua.
  - `pub struct AutoInput<'a> { pub clicks: &'a [(i64, f32, f32)], pub wheels: &'a [(i64, f32, f32)], pub typing: &'a [i64], pub cursor: &'a dyn Fn(i64) -> Option<(f32, f32)>, pub edge_guard: bool }`, tọa độ theo 0–1.
  - `pub fn auto_segments(input: &AutoInput, level: Level, duration_ms: i64) -> Vec<ZoomSegment>`. Chữ ký này thay chữ ký cũ, và các test cũ được sửa theo.

- [ ] **Step 1: Viết test trước**

```rust
// input_log.rs
#[test] fn type_lines_are_read()                       // parse_log đọc {"e":"type","t":500}; một dòng {"e":"future","t":1} bị bỏ qua, không lỗi
// effects.rs
#[test] fn typing_and_edge_guard_come_from_the_log()   // header mode "monitor" → edge_guard true; "region" → false; typing = các t của Type
// zoom.rs
#[test] fn auto_scale_follows_the_spread()             // một click → scale 2.0; nhóm có bbox 0.6 → 1.25
#[test] fn far_groups_pan_with_keys()                  // nhóm (0.2,0.2) t=1000 và nhóm (0.8,0.8) t=2500: một đoạn, center (0.2,0.2), keys == [ZoomKey{t_ms:2100, cx:0.8, cy:0.8, scale:2.0}]
#[test] fn edge_clicks_are_ignored_when_guarded()      // click (0.5,0.99) với edge_guard: không đoạn nào ở Normal; ở High thì có
#[test] fn low_level_needs_more_than_a_lone_click()    // một click lẻ: Low → 0 đoạn; một click rồi typing t+800: Low → 1 đoạn
#[test] fn typing_keeps_the_zoom()                     // click t=1000, typing mỗi 500 ms tới 6000: end = 7000; typing tới 30000 → end = 1000+20000
#[test] fn scrolling_inside_the_view_keeps_the_zoom()  // wheel trong khung kéo dài; wheel ngoài khung không
#[test] fn leaving_the_view_ends_early()               // click t=1000 ở (0.5,0.5) scale 2; cursor ra (0.95,0.95) từ t=1300: end = 2300 (thay vì 2800)
#[test] fn high_level_holds_shorter()                  // một click t=1000: High end = 2200, Normal end = 2800
```

- [ ] **Step 2: Chạy, phải FAIL**

Run: `cargo test -p hp-core`
Expected: lỗi biên dịch, do chữ ký `auto_segments` mới và `Event::Type` chưa có.

- [ ] **Step 3: Làm** theo đúng thứ tự quy tắc 1–7 trong mục 4 của spec.
  - Thông số theo `Level` nằm trong `fn level_params(l: Level) -> LevelParams { min_clicks, hold_ms, edge_guard_applies }`.
  - Khung dùng để xét "trong khung", ở quy tắc 5 và 6, là tâm và mức phóng của nhóm cuối trong đoạn.
  - Đoạn tự động có `manual: false` và `speed: Normal`.
  - `edit_cmds::auto_zoom` dựng `AutoInput`:
    - `clicks` lấy từ `fx.ripples`;
    - `wheels` là `fx.wheel_times` ghép với `cursor_at`;
    - `typing` lấy từ `fx.typing`;
    - `cursor` lấy từ `fx.cursor_at`, chia cho w/h;
    - `edge_guard` lấy từ `fx.edge_guard`;
    - rồi gọi `regenerate(&doc.zoom.segments, auto_segments(&input, doc.zoom.level, duration_ms))`.

- [ ] **Step 4: Chạy, phải PASS**

Run: `cargo test -p hp-core && cargo test -p hp-screen --lib edit_cmds`
Expected: pass.

- [ ] **Step 5: Commit** `feat(core): smarter auto zoom: levels, auto scale, pan keys, typing and scroll hold, early exit, edge guard`

### Task 3: Bộ quay ghi dấu "đang gõ" (`input_hook.rs`)

**Files:**
- Modify: `src-tauri/src/input_hook.rs`

**Interfaces:**
- Consumes: `Event::Type` (Task 2).
- Produces: `fn typing_tick(key: &KeyStroke, combo_logged: bool, last: &mut Option<i64>, t: i64) -> Option<Event>`, một hàm thuần để test.

- [ ] **Step 1: Viết test trước**

```rust
#[test] fn plain_keys_leave_a_type_mark_without_the_key()  // phím 'A' không Ctrl: Some(Event::Type{t}); dòng JSON == {"e":"type","t":..} (không có trường k)
#[test] fn type_marks_are_at_most_every_300_ms()           // t=0,100,299 → 1 dấu; t=300 → dấu thứ 2
#[test] fn logged_combos_and_modifiers_make_no_type_mark() // Ctrl+S (đã ghi Key) và phím Shift riêng lẻ: None
#[test] fn no_type_mark_while_drawing()                    // keep_event(Event::Type{..}, drawing=true) == false
```

- [ ] **Step 2: Chạy, phải FAIL**

Run: `cargo test -p hp-screen --lib input_hook`
Expected: lỗi biên dịch, vì chưa có `typing_tick`.

- [ ] **Step 3: Làm**
  - Ở nhánh `Raw::KeyDown`: khi `combo` là `None` và phím không phải phím lặp, gọi `typing_tick`.
  - Phím bổ trợ (Ctrl, Shift, Alt, Win đứng riêng) không tính.
  - `keep_event` loại `Event::Type` khi đang vẽ.

- [ ] **Step 4: Chạy, phải PASS**

Run: `cargo test -p hp-screen --lib input_hook`
Expected: pass.

- [ ] **Step 5: Commit** `feat: recorder marks typing moments (never the keys) for smarter zoom`

### Task 4: Kiểu TS và phép tính khung (`editApi.ts`, `zoomMath.ts`)

**Files:**
- Modify: `src/editApi.ts`, `src/editor/zoomMath.ts`
- Test: `src/editor/zoomMath.test.ts`

**Interfaces:**
- Consumes: `sourceView(cam, srcW, srcH, fill)` từ `layoutMath.ts`, và JSON của Task 1–2.
- Produces (`zoomMath.ts`):
  - `cameraFromView(view: View, srcW, srcH, fill: number | null): Camera`, phép ngược của `sourceView`.
  - `moveFrame(cam, dx, dy, srcW, srcH, fill): Camera`, với dx, dy theo px nguồn; khung không ra ngoài hình.
  - `resizeFrame(cam, corner: "nw"|"ne"|"sw"|"se", px: number, py: number, srcW, srcH, fill): Camera`: góc đối diện đứng yên khi có thể, mức phóng trong 1–4.
  - `activeKey(seg: ZoomSegment, ms: number): number`, trả -1 cho khung chính hoặc chỉ số trong `keys`.
  - `setActiveFrame(seg, ms, cam): ZoomSegment`: ghi vào điểm đang hoạt động, đặt `manual: true`, và đặt `center` khi đang là `null`.
  - `addKey(seg, ms, cam): ZoomSegment | null`, null khi ngoài đoạn hoặc cách điểm khác ≤ 50 ms.
  - `moveSegment(seg, dMs): ZoomSegment`: dời cả các điểm.
  - `addSegmentAt(segments, ms, durationMs, center): { segments, index } | null`.
- Kiểu trong `editApi.ts`:
  - `ZoomKey { t_ms; cx; cy; scale }`
  - `Speed = "slow"|"normal"|"fast"|"instant"`
  - `Level = "low"|"normal"|"high"`
  - `ZoomSegment.keys: ZoomKey[]`, `ZoomSegment.speed: Speed`
  - `ZoomDoc.level: Level`

- [ ] **Step 1: Viết test trước**

```ts
test("frameRoundTripsInFillMode")              // với fill null và fill 9/16: cameraFromView(sourceView(c)) ≈ c (sai số 1e-6) cho c = (0.3,0.6,2)
test("moveFrame stays inside the picture")     // dời quá mép → khung chạm mép, không vượt
test("resizeIsClampedAndNeverFlips")           // kéo góc se qua góc nw → scale ≤ 4, w > 0; kéo ra ngoài hình → scale ≥ 1
test("resize keeps the opposite corner")       // kéo se: góc nw của view giữ nguyên (sai số 1 px) khi không chạm giới hạn
test("activeKey picks the last key at or before")
test("setActiveFrame edits the active key and makes the segment manual")
test("addKey refuses near another key")        // cách 40 ms → null; ngoài đoạn → null
test("movingASegmentMovesItsKeys")             // moveSegment(+500): mọi t_ms + 500
test("zAtTheEndAddsNothing")                   // duration 10000, ms 9600 → null; ms 2000 với đoạn sau ở 3500 → đoạn 2000–3500; mặc định 3000 ms, scale 1.5, manual true, speed normal, keys []
```

- [ ] **Step 2: Chạy, phải FAIL**

Run: `node --test src/editor/zoomMath.test.ts`
Expected: lỗi import.

- [ ] **Step 3: Làm**
  - Trong `ZoomTrack.tsx`, kéo mép đoạn thì bỏ các điểm nằm ngoài khoảng mới; dời cả đoạn thì dùng `moveSegment`.

- [ ] **Step 4: Chạy, phải PASS**

Run: `pnpm test && pnpm tsc --noEmit`
Expected: pass.

- [ ] **Step 5: Commit** `feat: zoom frame math and keyframe helpers for the edit page`

### Task 5: Giao diện (chế độ chỉnh khung, điểm ◆, tốc độ, mức độ, Z/K) và xuất

**Files:**
- Create: `src/editor/ZoomFrame.tsx`, lớp khung kéo thả đặt trong `preview-frame`.
- Modify: `src/editor/Preview.tsx`:
  - thêm prop `frameEdit?: { camera: Camera; onChange(cam: Camera, first: boolean): void }`;
  - khi có prop này: không zoom, chỉ vẽ toàn hình, và hiện `ZoomFrame`.
- Modify:
  - `src/pages/EditPage.tsx`: thẻ Zoom, phím Z/K/Delete; bỏ "Chọn tâm trên video".
  - `src/editor/ZoomTrack.tsx`: hình thoi; màu nhạt cho đoạn tự động.
  - `src/theme.css`: chỉ thêm vào.
- Modify: `src-tauri/src/export.rs` tests.
- Modify: `docs/manual-test-checklist.md`: thêm Z9–Z16.

**Interfaces:**
- Consumes: mọi thứ ở Task 4; `zoomAuto`, `zoomTrack`.

- [ ] **Step 1: Viết test xuất trước**, trong `export.rs`:

```rust
#[test] fn keyframes_pan_the_export()       // nguồn bốn góc màu (make_quadrants), đoạn 0–4000 center (0.25,0.25) scale 2, key t=2000 (0.75,0.75) 2, speed Instant: khung xuất ở 1000 ms toàn màu góc trên trái, ở 3000 ms toàn màu góc dưới phải
#[test] fn keyframes_inside_a_cut_export()  // như trên, cắt 1500–2500: xuất thành công, độ dài đúng, khung ở 3000 ms nguồn (= 2000 ms ra) là màu góc dưới phải
```

- [ ] **Step 2: Chạy, phải FAIL hoặc PASS ngay.** Nếu PASS ngay thì ghi vào ledger rằng Task 1 đã đủ cho phần xuất, kèm Ruling.

Run: `cargo test -p hp-screen --lib keyframes`

- [ ] **Step 3: Làm giao diện**
  - **Chế độ chỉnh khung:** chỉ khi đủ bốn điều kiện của spec 3. Khung ở vị trí camera của `zoom_track` tại vị trí đang phát. Kéo khung thì `setActiveFrame`, và một lần kéo là một bước hoàn tác (`first`).
  - **Thẻ Zoom:**
    - hàng Ít / Vừa / Nhiều: đổi mức thì `zoomAuto` ngay;
    - nút "+ Zoom tại đây";
    - nút "◆ Thêm điểm";
    - "Đoạn có N điểm lia" và nút "Xóa điểm này";
    - hàng Chậm / Vừa / Nhanh / Tức thì;
    - nút "Xóa các đoạn tự động";
    - giữ nguyên thanh trượt mức phóng (sửa vào điểm đang hoạt động), "Theo con trỏ" và "Xóa đoạn zoom".
  - **Phím:**
    - Z dùng `addSegmentAt`, tâm là con trỏ của `session.effects` tại ms (chia cho w/h), không thì 0,5;
    - K dùng `addKey` với camera hiện tại;
    - Delete xóa điểm đang chọn trước, rồi mới tới đoạn.
  - **Rãnh Zoom:**
    - hình thoi 9 px, kéo được trong đoạn;
    - bấm vào thì chọn điểm và tua tới;
    - đoạn `manual: false` có độ đậm 55%.
  - **Chọn đoạn:** tua tới `start_ms` nếu vị trí đang phát nằm ngoài đoạn.

- [ ] **Step 4: Chạy toàn bộ**

Run: `cargo test -p hp-core && cargo test -p hp-screen --lib && pnpm test && pnpm tsc --noEmit`
Expected: pass hết.

- [ ] **Step 5: Thêm Z9–Z16 vào danh sách thử tay**:
  - Z9: kéo khung;
  - Z10: kéo góc;
  - Z11: ◆ lia A→B→C;
  - Z12: tốc độ;
  - Z13: phím Z / K;
  - Z14: Ít/Vừa/Nhiều;
  - Z15: xóa đoạn tự động;
  - Z16: gõ phím giữ zoom (video quay mới).

- [ ] **Step 6: Commit** `feat: zoom frame on the preview, keyframes on the zoom track, speed, levels, Z and K`
