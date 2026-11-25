import { expect, open, test } from "nsh/test";
import { ConversationNetwork, Conversations, Demo, Host } from "rustsale/native";
import { inboxQuery, isFiltered, parseDirectChat, accountLabel } from "./stores/inboxFilter";
import type { DirectChatAccount } from "./stores/inboxFilter";

let seeded: Promise<unknown> | undefined;
const dir = `/tmp/rustsale-v3-ui-inbox-l5-${Date.now()}`;

async function openInbox() {
  await Demo.prepare(dir);
  await Host.start(dir);
  seeded ??= Demo.seed();
  await seeded;
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-conversation");
  await app.waitFor("id:inbox-list");
  return app;
}
const has = (app: Awaited<ReturnType<typeof openInbox>>, s: string) => app.texts().some((t) => t.includes(s));
const account: DirectChatAccount = { id: 7, channel: "zalo", displayName: "Tài khoản demo", connectionStatus: "connected" };

// ---- pure helpers: no core needed ----

test("the inbox query carries the tab, the channel and the trimmed search", () => {
  expect(inboxQuery("all", "all", "")).toEqual({ sort: "recent" });
  expect(inboxQuery("unread", "all", "")).toEqual({ sort: "recent", unreadOnly: true });
  expect(inboxQuery("group", "zalo", "  lô 29 ")).toEqual({ sort: "recent", kind: "group", channel: "zalo", search: "lô 29" });
  expect(inboxQuery("unknown", "whatsapp", "").kind).toBe("unknown");
});

test("a filter is on when the tab, the channel or the search narrows the list", () => {
  expect(isFiltered("all", "all", "   ")).toBe(false);
  expect(isFiltered("unread", "all", "")).toBe(true);
  expect(isFiltered("all", "email", "")).toBe(true);
  expect(isFiltered("all", "all", "abc")).toBe(true);
});

test("the new-chat form says what is missing, one thing at a time", () => {
  const bad = (a: DirectChatAccount | null, t: string, n: string, m: string) => {
    const r = parseDirectChat(a, t, n, m);
    return r.ok ? "" : r.error;
  };
  expect(bad(null, "0900000000", "", "xin chào")).toBe("Chọn tài khoản để gửi.");
  expect(bad(account, "  ", "", "xin chào")).toBe("Nhập số điện thoại hoặc mã người nhận.");
  expect(bad(account, "0900000000", "", "   ")).toBe("Nhập tin nhắn đầu tiên.");
  const ok = parseDirectChat(account, " 0900000000 ", " An ", " xin chào ");
  expect(ok.ok).toBe(true);
  if (ok.ok) {
    expect(ok.value).toEqual({ channelAccountId: 7, phoneOrExternalId: "0900000000", displayName: "An", initialMessage: "xin chào" });
  }
  const noName = parseDirectChat(account, "0900000000", "", "hi");
  if (noName.ok) expect("displayName" in noName.value).toBe(false);
  expect(accountLabel(account)).toBe("Zalo · Tài khoản demo");
});

// ---- the screen ----

test("the unread tab shows what the core counts as unread, and all brings the rest back", async () => {
  const app = await openInbox();
  // Other test files share this core and may have read the seeded unread chat, so compare with the core, not the seed.
  type Page = { items: { key: string }[]; total: number };
  const unread = (await Conversations.inboxPage({ sort: "recent", unreadOnly: true }, null, 50)) as Page;
  const all = (await Conversations.inboxPage({ sort: "recent" }, null, 50)) as Page;
  await app.waitUntil(() => has(app, `${all.total} hội thoại`), { timeout: 8000 });
  await app.press("id:inbox-tab-unread");
  if (unread.total === 0) {
    await app.waitUntil(() => has(app, "Không có hội thoại khớp"), { timeout: 8000 });
  } else {
    await app.waitUntil(() => has(app, `${unread.total} hội thoại`), { timeout: 8000 });
    for (const item of unread.items) expect(app.shows(`id:inbox-${item.key}`)).toBe(true);
  }
  if (unread.total < all.total) {
    const read = all.items.find((i) => !unread.items.some((u) => u.key === i.key));
    if (read) expect(app.shows(`id:inbox-${read.key}`)).toBe(false);
  }
  await app.press("id:inbox-tab-all");
  await app.waitUntil(() => has(app, `${all.total} hội thoại`), { timeout: 8000 });
});

test("a tab with no match says so, and the filter hint is not the empty-inbox text", async () => {
  const app = await openInbox();
  await app.press("id:inbox-tab-group");
  await app.waitUntil(() => has(app, "Không có hội thoại khớp"), { timeout: 8000 });
  expect(has(app, "Hộp thư trống")).toBe(false);
  await app.press("id:inbox-tab-all");
  await app.waitUntil(() => app.shows("id:inbox-lead:1"), { timeout: 8000 });
});

test("the channel chips filter by channel", async () => {
  const app = await openInbox();
  await app.press("id:inbox-channel-whatsapp");
  await app.waitUntil(() => has(app, "Không có hội thoại khớp"), { timeout: 8000 });
  await app.press("id:inbox-channel-zalo");
  await app.waitUntil(() => app.shows("id:inbox-lead:1"), { timeout: 8000 });
});

test("typing in the search box narrows the inbox, clearing it restores the list", async () => {
  const app = await openInbox();
  await app.waitUntil(() => app.shows("id:inbox-lead:1"), { timeout: 8000 });
  await app.type("id:inbox-search", "zzz-no-such-conversation");
  await app.waitUntil(() => has(app, "Không có hội thoại khớp"), { timeout: 8000 });
  await app.clear("id:inbox-search");
  await app.waitUntil(() => app.shows("id:inbox-lead:1"), { timeout: 8000 });
});

test("new chat: the dialog refuses an empty form, then starts a chat without sending anything", async () => {
  const app = await openInbox();
  await app.press("id:new-chat");
  await app.waitFor("id:newchat-send");
  // The demo has one account, so it is already chosen: the first thing missing is the recipient.
  await app.press("id:newchat-send");
  await app.waitUntil(() => has(app, "Nhập số điện thoại hoặc mã người nhận."), { timeout: 4000 });
  await app.type("id:newchat-target", "0900000000");
  await app.press("id:newchat-send");
  await app.waitUntil(() => has(app, "Nhập tin nhắn đầu tiên."), { timeout: 4000 });
  await app.type("id:newchat-message", "tin thử, không gửi thật");
  await app.press("id:newchat-send");
  await app.waitUntil(() => !app.shows("id:newchat-send"), { timeout: 8000 });
  // The demo account has no live session: the first message waits in the outbox and is not sent.
  const page = (await Conversations.inboxPage({ sort: "recent" }, null, 50)) as { items: { key: string; title: string }[] };
  expect(page.items.length).toBe(4);
});

test("the core opens the chat with a queued, outbound first message (nothing is delivered)", async () => {
  await openInbox();
  const accounts = (await Conversations.directChatAccounts()) as DirectChatAccount[];
  expect(accounts.length).toBeGreaterThan(0);
  const first = (await ConversationNetwork.startDirect({
    channelAccountId: accounts[0].id,
    phoneOrExternalId: "0900000001",
    displayName: "Khách thử",
    initialMessage: "tin thử thứ hai, không gửi thật",
  })) as { direction: string; status: string; conversationId: number };
  expect(first.direction).toBe("outbound");
  expect(["queued", "failed"]).toContain(first.status);
  expect(first.status).not.toBe("sent");
  expect(first.status).not.toBe("delivered");
});
