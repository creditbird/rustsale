import { expect, open, test } from "nsh/test";
import { Demo, Host } from "rustsale/native";

let seeded: Promise<unknown> | undefined;
const dir = `/tmp/rustsale-v3-ui2-conv-${Date.now()}`;

async function openConversation() {
  await Host.start(dir);
  seeded ??= Demo.seed();
  await seeded;
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-conversation");
  await app.waitFor("id:inbox-list");
  return app;
}
const has = (app: Awaited<ReturnType<typeof openConversation>>, s: string) => app.texts().some((t) => t.includes(s));

test("the inbox lists the seeded conversations, and choosing one loads its timeline", async () => {
  const app = await openConversation();
  await app.waitUntil(() => has(app, "3 hội thoại"), { timeout: 8000 });
  await app.press("id:inbox-lead:1");
  await app.waitUntil(() => has(app, "báo giá lô 29"), { timeout: 8000 });
  expect(has(app, "Sarah Harrington")).toBe(true);
});

test("sending with an empty composer is refused before the core is asked", async () => {
  const app = await openConversation();
  await app.press("id:inbox-lead:1");
  await app.waitFor("id:send");
  await app.press("id:send");
  await app.waitUntil(() => has(app, "Nhập nội dung"), { timeout: 4000 });
});
