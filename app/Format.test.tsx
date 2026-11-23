import { expect, test } from "nsh/test";
import { bodyOf, hhmm, mergeMessages, type TimelineMessage } from "./stores/format";
import { failure, MESSAGE_FOR_CODE } from "./stores/core";
import { runtimeVerdict } from "./stores/scraperRuntime";

const msg = (id: number, at: string, text: string | null = "x", type = "text"): TimelineMessage => ({
  id, direction: "inbound", contentType: type, textContent: text, occurredAt: at, status: "received",
});

test("times show in the machine's own time zone, not UTC", () => {
  const iso = "2026-07-20T17:05:00.000Z";
  const d = new Date(iso);
  const expected = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  expect(hhmm(iso)).toBe(expected);
  // At any offset other than UTC the old slice (17:05) is wrong; at +07 the local time is 00:05.
  const offsetMinutes = -d.getTimezoneOffset();
  const shifted = (((17 * 60 + 5 + offsetMinutes) % 1440) + 1440) % 1440;
  expect(hhmm(iso)).toBe(`${String(Math.floor(shifted / 60)).padStart(2, "0")}:${String(shifted % 60).padStart(2, "0")}`);
  expect(hhmm("not a date")).toBe("");
});

test("messages stay oldest first: the newest ends last and an older page lands on top", () => {
  const newest = [msg(10, "2026-07-20T10:00:00Z"), msg(11, "2026-07-20T10:05:00Z")];
  const olderPage = [msg(8, "2026-07-20T09:00:00Z"), msg(9, "2026-07-20T09:30:00Z")];
  const all = mergeMessages(newest, olderPage);
  expect(all.map((m) => m.id)).toEqual([8, 9, 10, 11]);
  expect(all[0].occurredAt < all[all.length - 1].occurredAt).toBe(true);
  // A page delivered newest first (the documented order) ends up the same.
  expect(mergeMessages([], newest.slice().reverse()).map((m) => m.id)).toEqual([10, 11]);
  // A message that arrives twice (page and event) is kept once.
  expect(mergeMessages(all, [msg(11, "2026-07-20T10:05:00Z", "sửa")]).length).toBe(4);
});

test("a message with no text gets a label, never an empty bubble", () => {
  expect(bodyOf(msg(1, "2026-07-20T10:00:00Z", null, "sticker"))).toBe("[Sticker]");
  expect(bodyOf(msg(2, "2026-07-20T10:00:00Z", null, "image"))).toBe("[Ảnh]");
  expect(bodyOf(msg(3, "2026-07-20T10:00:00Z", "  ", "text")).length).toBeGreaterThan(0);
  expect(bodyOf(msg(4, "2026-07-20T10:00:00Z", "chào", "text"))).toBe("chào");
});

test("every native code maps to a Vietnamese sentence, never the core's English text", () => {
  for (const code of ["invalid_request", "not_found", "conflict", "unavailable", "internal", "something_new"]) {
    const f = failure(Object.assign(new Error("raw english text from the core"), { code }));
    expect(f.message.includes("raw english")).toBe(false);
    expect(/[ạảãàáâấầẩẫậăắằẳẵặéèẻẽẹêếềểễệíìỉĩịóòỏõọôốồổỗộơớờởỡợúùủũụưứừửữựýỳỷỹỵđ]/i.test(f.message)).toBe(true);
  }
  expect(failure(Object.assign(new Error("x"), { code: "unavailable" }), { unavailable: "kênh chưa đăng nhập" }).message).toBe("kênh chưa đăng nhập");
  expect(MESSAGE_FOR_CODE.invalid_request.length).toBeGreaterThan(0);
});

test("scraper runtime check: only unavailable means the browser is not installed", () => {
  const err = (code: string) => Object.assign(new Error("x"), { code });
  expect(runtimeVerdict(err("unavailable"))).toEqual({ label: "chưa sẵn sàng", toast: false });
  for (const code of ["not_found", "internal", "invalid_request"]) {
    expect(runtimeVerdict(err(code))).toEqual({ label: "không kiểm tra được", toast: true });
  }
  expect(runtimeVerdict("boom").toast).toBe(true);
});
