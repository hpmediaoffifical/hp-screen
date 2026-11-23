/*! © 2026 HP Media (hpvn.media) — HP Screen. Bảo lưu mọi quyền. HP không có đại lý khác, không hợp tác với đơn vị nào, mọi sao chép đều là vi phạm. */
import assert from "node:assert/strict";
import { test } from "node:test";
import { classifyDigits, sensitiveGroups, type OcrWord } from "./pii.ts";

const word = (text: string, x: number, y = 100, w = 40, h = 20): OcrWord => ({ text, x, y, w, h });

test("digit runs are classified by shape", () => {
  assert.equal(classifyDigits("0901234567"), "phone");
  assert.equal(classifyDigits("09012345678"), "phone"); // 11 số
  assert.equal(classifyDigits("02412345678"), "phone"); // máy bàn 02x
  assert.equal(classifyDigits("84901234567"), "phone"); // +84
  assert.equal(classifyDigits("079203012345"), "id"); // CCCD 12 số
  assert.equal(classifyDigits("123456789"), "id"); // CMND cũ 9 số
  assert.equal(classifyDigits("4111111111111111"), "card"); // thẻ 16 số
  assert.equal(classifyDigits("12345678"), null); // quá ngắn
  assert.equal(classifyDigits("0100000000"), null); // đầu số không có thật (0 1…)
  assert.equal(classifyDigits("12062026"), null); // ngày tháng
  assert.equal(classifyDigits("0"), null);
});

test("a phone split across words is caught as one group", () => {
  const words = [word("SDT:", 0), word("0901", 60), word("234", 100), word("567", 130)];
  const groups = sensitiveGroups(words);
  assert.equal(groups.length, 1);
  assert.equal(groups[0].words.length, 3);
  assert.equal(groups[0].x, 60);
});

test("an email is caught, with or without spaces", () => {
  assert.equal(sensitiveGroups([word("phung@hpvn.media")]).length, 1);
  const split = [word("phung", 0, 100, 42, 20), word("@", 46, 100, 8, 20), word("hpvn.media", 58, 100, 66, 20)];
  assert.equal(sensitiveGroups(split).length, 1);
});

test("words on different lines are never joined", () => {
  const words = [word("0901", 0, 100), word("234", 60, 140)];
  assert.equal(sensitiveGroups(words).length, 0);
});

test("an id number written with dots is caught", () => {
  const words = [word("CCCD", 0), word("0792", 80), word("030", 120), word("123456", 155)];
  const groups = sensitiveGroups(words);
  assert.equal(groups.length, 1);
  assert.equal(groups[0].words.length, 3);
});

test("ordinary text and short numbers are left alone", () => {
  const words = [word("Họ", 0), word("và", 40), word("tên:", 70), word("2026", 120), word("OK", 170)];
  assert.equal(sensitiveGroups(words).length, 0);
});
