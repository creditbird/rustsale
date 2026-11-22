import { expect, open, test } from "nsh/test";
import { Conversations, Demo, Host } from "rustsale/native";

let seeded: Promise<unknown> | undefined;
const dir = `/tmp/rustsale-v3-ui2-conv-${Date.now()}`;

async function openConversation(entry = "./App.tsx") {
  await Demo.prepare(dir);
  await Host.start(dir);
  seeded ??= Demo.seed();
  await seeded;
  const app = await open(entry);
  await app.setViewport(1280, 800);
  await app.press("id:nav-conversation");
  await app.waitFor("id:inbox-list");
  return app;
}
const has = (app: Awaited<ReturnType<typeof openConversation>>, s: string) => app.texts().some((t) => t.includes(s));

// First on purpose: the seed's unread line is cleared (in the core) by the first test that opens that chat.
test("opening a conversation clears its unread badge at once", async () => {
  const app = await openConversation();
  const page = (await Conversations.inboxPage({ sort: "recent" }, null, 50)) as { items: { key: string; unreadCount: number; title: string }[] };
  const mine = page.items.find((i) => i.key === "lead:3");
  if (!mine || mine.unreadCount === 0) throw new Error("the seed has no unread conversation to open");
  await app.waitUntil(() => app.texts().includes(String(mine.unreadCount)), { timeout: 8000 });
  await app.press("id:inbox-lead:3");
  await app.waitUntil(() => !app.texts().includes(String(mine.unreadCount)), { timeout: 4000 });
});

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

test("the newest message is at the bottom, the oldest is not on screen", async () => {
  const app = await openConversation();
  await app.press("id:inbox-lead:1");
  await app.waitUntil(() => has(app, "báo giá lô 29"), { timeout: 8000 });
  expect(has(app, "lô hàng số 0 ạ")).toBe(false);
});

test("a sticker and a photo show a label, not an empty bubble", async () => {
  const app = await openConversation();
  await app.press("id:inbox-lead:3");
  await app.waitUntil(() => has(app, "[Sticker]"), { timeout: 8000 });
  expect(has(app, "[Ảnh]")).toBe(true);
});

test("a message delta from the core shows up without reopening the chat", async () => {
  const app = await openConversation("./harness/EventApp.tsx");
  await app.press("id:inbox-lead:1");
  await app.waitUntil(() => has(app, "báo giá lô 29"), { timeout: 8000 });
  await app.press("id:emit-delta");
  await app.waitUntil(() => has(app, "tin mới từ sự kiện"), { timeout: 4000 });
});

test("the old conversation-updated name no longer reloads anything", async () => {
  const app = await openConversation("./harness/EventApp.tsx");
  await app.press("id:inbox-lead:1");
  await app.waitUntil(() => has(app, "báo giá lô 29"), { timeout: 8000 });
  await app.press("id:emit-updated");
  await new Promise((r) => setTimeout(r, 400));
  expect(has(app, "không được hiện")).toBe(false);
});
