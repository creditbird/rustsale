import { expect, open, test } from "nsh/test";

test("pressing Thử lại runs the retry once and closes the toast", async () => {
  const app = await open("./harness/ToastHarness.tsx");
  await app.setViewport(900, 600);
  await app.press("id:fire");
  await app.waitFor("id:toast-retry");
  await app.press("id:toast-retry");
  await app.waitUntil(() => app.texts().some((t) => t.includes("retry chạy 1 lần")), { timeout: 4000 });
  await app.waitUntil(() => !app.texts().some((t) => t.includes("Dịch vụ chưa sẵn sàng")), { timeout: 4000 });
  expect(app.texts().some((t) => t.includes("retry chạy 1 lần"))).toBe(true);
});
