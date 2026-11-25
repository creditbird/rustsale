import { expect, test } from "nsh/test";
import { parseAmount, valueAfterCurrencyChange } from "./stores/pipeline";

const val = (s: string, c: string) => { const r = parseAmount(s, c); return r.ok ? r.value : null; };

test("1.5 USD is one and a half dollars, not 15", () => {
  expect(val("1.5", "USD")).toBe(1.5);
  expect(val("1,234.50", "USD")).toBe(1234.5);
  expect(val("1,234", "USD")).toBe(1234);
  expect(val("12", "USD")).toBe(12);
});

test("VND takes whole numbers with . or , grouping thousands", () => {
  expect(val("1,234", "VND")).toBe(1234);
  expect(val("1.000.000", "VND")).toBe(1_000_000);
  expect(val("10000000", "VND")).toBe(10_000_000);
  expect(val("1 000 000", "VND")).toBe(1_000_000);
});

test("ambiguous or invalid values are rejected, not guessed", () => {
  expect(val("1.5", "VND")).toBeNull();
  expect(val("1.234,5", "VND")).toBeNull();
  expect(val("1.234,5", "USD")).toBeNull();
  expect(val("1.234", "USD")).toBeNull();
  expect(val("1.234.567", "USD")).toBeNull();
  expect(val("1.2.3", "VND")).toBeNull();
  expect(val("1.50.0", "USD")).toBeNull();
  expect(val("abc", "VND")).toBeNull();
  expect(val("12abc", "USD")).toBeNull();
  expect(val("-5", "VND")).toBeNull();
  expect(val("-1.5", "USD")).toBeNull();
  expect(val("", "VND")).toBeNull();
});

test("switching currency resets an untouched default, keeps a typed amount", () => {
  expect(valueAfterCurrencyChange("10000000", "USD")).toBe("500");
  expect(valueAfterCurrencyChange("500", "VND")).toBe("10000000");
  expect(valueAfterCurrencyChange("2500000", "USD")).toBe("2500000");
  expect(valueAfterCurrencyChange("", "USD")).toBe("");
});
