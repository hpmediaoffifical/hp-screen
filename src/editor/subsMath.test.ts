import assert from "node:assert/strict";
import { test } from "node:test";
import { burnFontPx, chipBottom, cueAt, deleteCue, mergeWithNext, setCueText, setCueTime, splitCue, wrapLines } from "./subsMath.ts";

const cue = (start_ms: number, end_ms: number, text: string) => ({ start_ms, end_ms, text });

test("the cue at a time is the one that covers it", () => {
  const cues = [cue(0, 1000, "a"), cue(1000, 2000, "b")];
  assert.equal(cueAt(cues, 999)?.text, "a");
  assert.equal(cueAt(cues, 1000)?.text, "b");
  assert.equal(cueAt(cues, 2000), null);
});

test("lines wrap like the core: one line up to 42 characters, else two balanced", () => {
  assert.deepEqual(wrapLines("  xin   chào  "), ["xin chào"]);
  const long = "một hai ba bốn năm sáu bảy tám chín mười mười một mười hai";
  const lines = wrapLines(long);
  assert.equal(lines.length, 2);
  assert.ok(lines.every((l) => l.length <= 42));
  assert.equal(lines.join(" "), long);
  assert.deepEqual(wrapLines(""), []);
});

test("merging joins the text and spans both cues", () => {
  const cues = [cue(0, 1000, "xin"), cue(1200, 2000, "chào bạn"), cue(3000, 4000, "c")];
  const out = mergeWithNext(cues, 0);
  assert.equal(out.length, 2);
  assert.deepEqual(out[0], cue(0, 2000, "xin chào bạn"));
  assert.equal(mergeWithNext(cues, 2), cues, "the last cue has nothing to merge with");
});

test("splitting at a time cuts the words in proportion and keeps both halves", () => {
  const cues = [cue(0, 4000, "một hai ba bốn")];
  const out = splitCue(cues, 0, 2000);
  assert.deepEqual(out, [cue(0, 2000, "một hai"), cue(2000, 4000, "ba bốn")]);
  assert.equal(splitCue(cues, 0, 50), cues, "too close to the start");
  assert.equal(splitCue([cue(0, 4000, "một")], 0, 2000).length, 1, "a single word is not split");
});

test("deleting, retiming and retexting change only that cue", () => {
  const cues = [cue(0, 1000, "a"), cue(1000, 2000, "b")];
  assert.deepEqual(deleteCue(cues, 0), [cue(1000, 2000, "b")]);
  assert.deepEqual(setCueText(cues, 1, "bé"), [cue(0, 1000, "a"), cue(1000, 2000, "bé")]);
  // Times stay ordered and at least 100 ms long.
  assert.deepEqual(setCueTime(cues, 0, 500, 300)[0], cue(500, 600, "a"));
  assert.deepEqual(setCueTime(cues, 1, -50, 2500)[1], cue(0, 2500, "b"));
});

test("an untyped line is never shown and does not hide the line under it", () => {
  const cues = [cue(0, 3000, "  "), cue(1000, 2000, "Chào")];
  assert.equal(cueAt(cues, 500), null);
  assert.equal(cueAt(cues, 1500)?.text, "Chào");
});

test("burned font size matches the core, smaller in tall frames", () => {
  assert.ok(Math.abs(burnFontPx(1920, 1080) - 48.6) < 0.01);
  assert.equal(burnFontPx(1080, 1920), 36);
  assert.equal(burnFontPx(100, 50), 10);
});

test("chips move above a burned subtitle", () => {
  assert.equal(chipBottom(null), 0.08);
  const one = chipBottom("Lưu");
  const two = chipBottom("dòng một\ndòng hai");
  assert.ok(one > 0.06 + 0.045 * 1.3, `above the box: ${one}`);
  assert.ok(two > one);
});
