// Headless tests for RS #71 — Advanced Conversation workspace:
// UI-47 (Lead linking/creation/relink/unlink) & UI-48 (Pinned banner, In-thread search,
// Quick reply templates CRUD + picker, Facebook 24h policy banner, Staged attachment queue).
// Shared-DB safe: never assumes empty lists, narrows lists by unique stamp before waitFor,
// and deletes all created records in try/finally.
import { expect, open, test } from "nsh/test";
import {
  ConversationActions,
  ConversationNetwork,
  Conversations,
  Demo,
  Host,
  Leads,
  LeadsMore,
} from "rustsale/native";
import { eventually } from "./testing";
import {
  attachmentTypeLabel,
  evaluateFacebookWindow,
  findSlashToken,
  findThreadMatches,
  formatBytes,
  formatRemainingHm,
  HUMAN_AGENT_WINDOW_MS,
  parseAttachmentPaths,
  rankQuickTemplates,
  replaceSlashToken,
  stepMatchIndex,
  validateFacebookOutbound,
  validateQuickTemplate,
  WINDOW_MS,
  type InboxItem,
  type QuickTemplate,
} from "./stores/conversationAdv";

let seeded: Promise<unknown> | undefined;
const dir = `/tmp/rustsale-v3-ui-conv-adv-${Date.now()}`;

async function ensureCore() {
  await Demo.prepare(dir);
  await Host.start(dir);
  seeded ??= Demo.seed();
  await seeded;
}

async function openConversation(modules?: Record<string, Record<string, (...args: unknown[]) => unknown>>) {
  await ensureCore();
  const app = await open("./App.tsx", modules ? { modules } : undefined);
  await app.setViewport(1280, 800);
  await app.press("id:nav-conversation");
  await app.waitFor("id:inbox-list");
  return app;
}

type App = Awaited<ReturnType<typeof openConversation>>;
const has = (app: App, s: string) => app.texts().some((t) => t.includes(s));

// ---- 1. Pure unit tests (no core needed) ----

test("facebook 24h policy gate evaluates inside 24h, 24h..7d HUMAN_AGENT, >7d, missing and clock skew", () => {
  const now = Date.parse("2026-11-25T12:00:00.000Z");

  const inside = evaluateFacebookWindow(now - 2 * 60 * 60 * 1000, now);
  expect(inside.freeWindow).toBe(true);
  expect(inside.humanAgentEligible).toBe(true);
  expect(formatRemainingHm(inside.remainingMs)).toBe("22h 00m");
  expect(validateFacebookOutbound(inside, null).allowed).toBe(true);

  const outside = evaluateFacebookWindow(now - WINDOW_MS - 60_000, now);
  expect(outside.freeWindow).toBe(false);
  expect(outside.humanAgentEligible).toBe(true);
  expect(validateFacebookOutbound(outside, null).allowed).toBe(false);
  expect(validateFacebookOutbound(outside, "CONFIRMED_EVENT_UPDATE").allowed).toBe(false);
  expect(validateFacebookOutbound(outside, "HUMAN_AGENT").allowed).toBe(true);

  const expired = evaluateFacebookWindow(now - HUMAN_AGENT_WINDOW_MS - 60_000, now);
  expect(expired.freeWindow).toBe(false);
  expect(expired.humanAgentEligible).toBe(false);
  expect(validateFacebookOutbound(expired, "HUMAN_AGENT").allowed).toBe(false);

  const missing = evaluateFacebookWindow(null, now);
  expect(missing.freeWindow).toBe(false);
  expect(missing.humanAgentEligible).toBe(false);

  const futureSkew = evaluateFacebookWindow(now + 10 * 60 * 1000, now);
  expect(futureSkew.freeWindow).toBe(false);
  expect(futureSkew.humanAgentEligible).toBe(false);
});

test("quick template slash token, ranking and validation follow v2 rules", () => {
  expect(findSlashToken("/gia", 4)).toEqual({ start: 0, end: 4, query: "gia" });
  expect(findSlashToken("chào /bao", 9)).toEqual({ start: 5, end: 9, query: "bao" });
  expect(findSlashToken("https://example.com", 19)).toBeNull();
  expect(findSlashToken("/gia ", 5)).toBeNull();

  const replaced = replaceSlashToken("chào /bao nhé", { start: 5, end: 9, query: "bao" }, "báo giá");
  expect(replaced.value).toBe("chào báo giá nhé");

  const sample: QuickTemplate[] = [
    { id: 1, shortcut: "baogia", title: "Báo giá", bodyText: "Gửi bảng giá", category: "Sales", isActive: true, usageCount: 2, lastUsedAt: null, createdAt: "", updatedAt: "" },
    { id: 2, shortcut: "bao", title: "Chào", bodyText: "Xin chào", category: null, isActive: true, usageCount: 1, lastUsedAt: null, createdAt: "", updatedAt: "" },
    { id: 3, shortcut: "baocao", title: "Báo cáo", bodyText: "Ẩn", category: null, isActive: false, usageCount: 99, lastUsedAt: null, createdAt: "", updatedAt: "" },
  ];
  const ranked = rankQuickTemplates(sample, "bao");
  expect(ranked.map((r) => r.id)).toEqual([2, 1]);

  expect(validateQuickTemplate({ shortcut: " ", title: "T", bodyText: "B", category: "", isActive: true }, sample, null).ok).toBe(false);
  expect(validateQuickTemplate({ shortcut: "a b", title: "T", bodyText: "B", category: "", isActive: true }, sample, null).ok).toBe(false);
  expect(validateQuickTemplate({ shortcut: "/BAOGIA", title: "T", bodyText: "B", category: "", isActive: true }, sample, null).ok).toBe(false);
  const okEdit = validateQuickTemplate({ shortcut: "/baogia", title: "Báo giá mới", bodyText: "Nội dung mới", category: " Sales ", isActive: true }, sample, 1);
  expect(okEdit.ok).toBe(true);
  if (okEdit.ok) {
    expect(okEdit.value.shortcut).toBe("baogia");
    expect(okEdit.value.category).toBe("Sales");
  }
});

test("in-thread search matching and attachment helpers handle edge cases", () => {
  const msgs = [
    { id: 10, direction: "inbound", textContent: "Xin báo giá lô 1", occurredAt: "2026-07-20T08:00:00Z", status: "received" },
    { id: 11, direction: "outbound", textContent: "Đã gửi file", occurredAt: "2026-07-20T08:01:00Z", status: "sent" },
    { id: 12, direction: "inbound", textContent: "Cảm ơn báo giá", occurredAt: "2026-07-20T08:02:00Z", status: "received" },
  ];
  expect(findThreadMatches(msgs, "BÁO GIÁ")).toEqual([10, 12]);
  expect(findThreadMatches(msgs, "   ")).toEqual([]);
  expect(stepMatchIndex(0, -1, 2)).toBe(1);
  expect(stepMatchIndex(1, 1, 2)).toBe(0);
  expect(formatBytes(0)).toBe("0 B");
  expect(formatBytes(2048)).toBe("2.0 KB");
  expect(attachmentTypeLabel("image")).toBe("Ảnh");
  expect(attachmentTypeLabel("document")).toBe("Tệp");
  expect(parseAttachmentPaths(" /tmp/a.png , /tmp/b.pdf \n ")).toEqual(["/tmp/a.png", "/tmp/b.pdf"]);
});

// ---- 2. UI-47: Lead creation, linking, relinking, unlinking ----

test("UI-47: unknown contact can create & link a lead, relink to another lead, unlink and relink again", async () => {
  await ensureCore();
  const stamp = String(Date.now());
  const phone = `0987${stamp.slice(-6)}`;
  const contactName = `Khách lạ ${stamp}`;
  const createdLeadName = `Lead mới ${stamp}`;
  const altLeadName = `Lead đổi ${stamp}`;

  const accounts = (await Conversations.directChatAccounts()) as { id: number }[];
  expect(accounts.length).toBeGreaterThan(0);

  let conversationId: number | null = null;
  let identityId: number | null = null;
  let createdLeadId: number | null = null;
  let altLeadId: number | null = null;

  try {
    const started = (await ConversationNetwork.startDirect({
      channelAccountId: accounts[0].id,
      phoneOrExternalId: phone,
      displayName: contactName,
      initialMessage: `Tin thử UI47 ${stamp}`,
    })) as { conversationId: number };
    conversationId = started.conversationId;

    const initialPage = (await Conversations.inboxPage({ search: contactName, sort: "recent" }, null, 20)) as { items: InboxItem[] };
    const unknownRow = initialPage.items.find((r) => r.replyTargets.some((t) => t.conversationId === conversationId));
    expect(unknownRow).toBeDefined();
    identityId = unknownRow?.identityId ?? unknownRow?.participants?.[0]?.identityId ?? null;
    expect(identityId).not.toBeNull();

    altLeadId = (await Leads.add({
      name: altLeadName,
      company: `Công ty đổi ${stamp}`,
      status: "Qualified",
      phone,
    })) as number;

    const app = await openConversation();
    // Narrow the inbox by unique stamp so the unknown row is visible on shared DB.
    await app.type("id:inbox-search", stamp);
    await app.waitFor(`id:inbox-${unknownRow!.key}`, { timeout: 8000 });
    await app.press(`id:inbox-${unknownRow!.key}`);
    await app.waitFor("id:conv-create-lead-btn", { timeout: 8000 });
    expect(app.shows("id:conv-link-btn")).toBe(true);

    // 1) Create lead & link (with empty-name validation first)
    await app.press("id:conv-create-lead-btn");
    await app.waitFor("id:create-lead-name", { timeout: 4000 });
    await app.clear("id:create-lead-name");
    await app.press("id:create-lead-submit");
    await app.waitFor("id:create-lead-error", { timeout: 4000 });

    await app.type("id:create-lead-name", createdLeadName);
    await app.type("id:create-lead-company", `Công ty tạo ${stamp}`);
    await app.press("id:create-lead-submit");

    // Conversation reloads into linked customer state
    await app.waitFor("id:conv-relink-btn", { timeout: 8000 });
    expect(app.shows("id:conv-unlink-btn")).toBe(true);
    await app.waitUntil(() => has(app, createdLeadName), { timeout: 8000 });

    const leadPage = (await Leads.page({ search: createdLeadName }, null, 10)) as { items: { id: number; name: string }[] };
    createdLeadId = leadPage.items.find((l) => l.name === createdLeadName)?.id ?? null;
    expect(createdLeadId).not.toBeNull();

    // 2) Relink to altLeadId
    await app.press("id:conv-relink-btn");
    await app.waitFor("id:link-lead-search", { timeout: 4000 });
    await app.type("id:link-lead-search", altLeadName);
    await app.waitFor(`id:link-lead-option-${altLeadId}`, { timeout: 8000 });
    await app.press(`id:link-lead-option-${altLeadId}`);
    await app.press("id:link-lead-confirm");
    await app.waitUntil(() => has(app, altLeadName), { timeout: 8000 });

    // 3) Unlink back to unknown contact
    await app.press("id:conv-unlink-btn");
    await app.waitFor("id:unlink-lead-confirm", { timeout: 4000 });
    await app.press("id:unlink-lead-confirm");
    await app.waitFor("id:conv-create-lead-btn", { timeout: 8000 });
    expect(app.shows("id:conv-link-btn")).toBe(true);

    // 4) Link existing lead back via Link Lead dialog
    await app.press("id:conv-link-btn");
    await app.waitFor("id:link-lead-search", { timeout: 4000 });
    await app.type("id:link-lead-search", createdLeadName);
    await app.waitFor(`id:link-lead-option-${createdLeadId}`, { timeout: 8000 });
    await app.press(`id:link-lead-option-${createdLeadId}`);
    await app.press("id:link-lead-confirm");
    await app.waitFor("id:conv-relink-btn", { timeout: 8000 });
  } finally {
    if (createdLeadId !== null) {
      await ConversationActions.unlinkLead({ leadId: createdLeadId }).catch(() => {});
    }
    if (altLeadId !== null) {
      await ConversationActions.unlinkLead({ leadId: altLeadId }).catch(() => {});
    }
    if (identityId !== null) {
      await ConversationActions.unlinkIdentity({ identityId }).catch(() => {});
      await ConversationActions.deleteConversation({ identityId }).catch(() => {});
    }
    if (conversationId !== null) {
      await ConversationActions.deleteConversation({ conversationId }).catch(() => {});
    }
    if (createdLeadId !== null) {
      await LeadsMore.delete(createdLeadId).catch(() => {});
    }
    if (altLeadId !== null) {
      await LeadsMore.delete(altLeadId).catch(() => {});
    }
  }
});

// ---- 3. UI-48: Pinned message banner & In-thread search ----

test("UI-48: pinning shows the pinned banner, unpinning clears it, and in-thread search steps matches", async () => {
  await ensureCore();
  const page = (await ConversationActions.contactTimeline(1, ["zalo"], null, 50)) as {
    items: { id: number; conversationId: number; textContent: string | null }[];
  };
  const newest = page.items[page.items.length - 1];
  expect(newest).toBeDefined();
  const quoteMatches = page.items.filter((m) => (m.textContent ?? "").toLowerCase().includes("báo giá")).length;
  expect(quoteMatches).toBeGreaterThan(1);

  try {
    const app = await openConversation();
    await app.press("id:inbox-lead:1");
    await app.waitUntil(() => has(app, "báo giá lô 29"), { timeout: 8000 });

    // Pin the newest visible message
    await app.waitFor(`id:msg-pin-${newest.id}`, { timeout: 8000 });
    await app.press(`id:msg-pin-${newest.id}`);
    await app.waitFor("id:pinned-message-banner", { timeout: 8000 });
    expect(has(app, "Tin đã ghim")).toBe(true);

    // Jump & unpin from banner
    await app.press("id:pin-jump-btn");
    await app.press("id:pin-unpin-btn");
    await app.waitUntil(() => !app.shows("id:pinned-message-banner"), { timeout: 8000 });

    // In-thread search
    await app.press("id:conv-search-toggle");
    await app.waitFor("id:thread-search-input", { timeout: 4000 });
    await app.type("id:thread-search-input", "báo giá lô 29");
    await app.waitUntil(() => has(app, "1/1"), { timeout: 4000 });
    expect(app.shows("id:thread-search-preview")).toBe(true);

    // Multi-match navigation
    await app.clear("id:thread-search-input");
    await app.type("id:thread-search-input", "báo giá");
    await app.waitUntil(() => has(app, `1/${quoteMatches}`), { timeout: 4000 });
    await app.press("id:thread-search-next");
    await app.waitUntil(() => has(app, `2/${quoteMatches}`), { timeout: 4000 });
    await app.press("id:thread-search-prev");
    await app.waitUntil(() => has(app, `1/${quoteMatches}`), { timeout: 4000 });

    await app.press("id:thread-search-close");
    await app.waitUntil(() => !app.shows("id:in-thread-search-bar"), { timeout: 4000 });
  } finally {
    await ConversationActions.unpinMessage({
      conversationId: newest.conversationId,
      messageId: newest.id,
    }).catch(() => {});
  }
});

// ---- 4. UI-48: Quick reply templates CRUD, slash picker & markQuickTemplateUsed ----

test("UI-48: quick reply templates support create, edit, slash picker insertion, usage increment and delete", async () => {
  await ensureCore();
  const stamp = String(Date.now());
  const shortcut = `qt${stamp.slice(-6)}`;
  const initialTitle = `Mẫu thử ${stamp}`;
  const updatedTitle = `Mẫu sửa ${stamp}`;
  const body = `Nội dung báo giá ${stamp}`;
  let createdId: number | null = null;

  try {
    const app = await openConversation();
    await app.press("id:inbox-lead:1");
    await app.waitFor("id:composer-templates-btn", { timeout: 8000 });

    // Open picker -> manager
    await app.press("id:composer-templates-btn");
    await app.waitFor("id:manage-quick-templates-btn", { timeout: 4000 });
    await app.press("id:manage-quick-templates-btn");
    await app.waitFor("id:qt-add-btn", { timeout: 4000 });

    // Create with empty-form validation first
    await app.press("id:qt-add-btn");
    await app.waitFor("id:qt-form-save", { timeout: 4000 });
    await app.press("id:qt-form-save");
    await app.waitFor("id:qt-form-error", { timeout: 4000 });

    await app.type("id:qt-form-shortcut", shortcut);
    await app.type("id:qt-form-category", "Bán hàng");
    await app.type("id:qt-form-title", initialTitle);
    await app.type("id:qt-form-body", body);
    await app.press("id:qt-form-save");

    // Narrow list by stamp & verify created row
    await app.waitFor("id:qt-manager-search", { timeout: 8000 });
    await app.type("id:qt-manager-search", stamp);
    await app.waitUntil(() => has(app, initialTitle), { timeout: 8000 });

    const stored = (await ConversationActions.quickTemplates({ includeInactive: true })) as QuickTemplate[];
    createdId = stored.find((t) => t.shortcut === shortcut)?.id ?? null;
    expect(createdId).not.toBeNull();

    // Edit title
    await app.press(`id:qt-edit-${createdId}`);
    await app.waitFor("id:qt-form-title", { timeout: 4000 });
    await app.clear("id:qt-form-title");
    await app.type("id:qt-form-title", updatedTitle);
    await app.press("id:qt-form-save");
    await app.waitUntil(() => has(app, updatedTitle), { timeout: 8000 });

    // Close manager, trigger via slash in composer, select item
    await app.press("id:qt-manager-close");
    await app.waitUntil(() => !app.shows("id:qt-add-btn"), { timeout: 4000 });

    await app.type("id:composer", `/${shortcut}`);
    await app.waitFor(`id:quick-tpl-item-${createdId}`, { timeout: 8000 });
    await app.press(`id:quick-tpl-item-${createdId}`);
    await app.waitUntil(() => !app.shows("id:quick-template-popover"), { timeout: 4000 });

    // Verify usageCount incremented in core
    await eventually(async () => {
      const list = (await ConversationActions.quickTemplates({ includeInactive: true })) as QuickTemplate[];
      const row = list.find((t) => t.id === createdId);
      return (row?.usageCount ?? 0) >= 1;
    }, 8000);

    // Delete from manager
    await app.press("id:composer-templates-btn");
    await app.waitFor("id:manage-quick-templates-btn", { timeout: 4000 });
    await app.press("id:manage-quick-templates-btn");
    await app.waitFor("id:qt-manager-search", { timeout: 4000 });
    await app.type("id:qt-manager-search", stamp);
    await app.waitFor(`id:qt-delete-${createdId}`, { timeout: 8000 });
    await app.press(`id:qt-delete-${createdId}`);
    await app.waitFor("id:qt-delete-confirm", { timeout: 4000 });
    await app.press("id:qt-delete-confirm");
    await app.waitUntil(() => !has(app, updatedTitle), { timeout: 8000 });
    createdId = null;
  } finally {
    if (createdId !== null) {
      await ConversationActions.deleteQuickTemplate({ id: createdId }).catch(() => {});
    }
  }
});

// ---- 5. UI-48: Staged attachment preview queue ----

test("UI-48: attachment staging bar stages a file into the preview queue and discards it cleanly", async () => {
  const app = await openConversation();
  await app.press("id:inbox-lead:1");
  await app.waitFor("id:composer-attach-btn", { timeout: 8000 });

  await app.press("id:composer-attach-btn");
  await app.waitFor("id:attach-path-input", { timeout: 4000 });

  // Empty path rejected cleanly
  await app.press("id:attach-stage-btn");
  await app.waitFor("id:attach-stage-error", { timeout: 4000 });

  // Stage the guaranteed sandbox marker file created by Demo.prepare(dir)
  await app.type("id:attach-path-input", `${dir}/.rustsale-demo-sandbox`);
  await app.press("id:attach-stage-btn");
  await app.waitFor("id:attachment-preview-queue", { timeout: 8000 });
  expect(has(app, ".rustsale-demo-sandbox")).toBe(true);

  // Remove staged attachment via discardStagedAttachment
  await app.press("id:remove-attachment-att-1");
  await app.waitUntil(() => !app.shows("id:attachment-preview-queue"), { timeout: 8000 });
});

// ---- 6. UI-48: Facebook 24h policy banner (all 3 windows, isolated stubs) ----

test("UI-48: Facebook 24h policy banner renders free, HUMAN_AGENT and expired windows and gates sendBatch", async () => {
  const now = Date.now();
  const sentBatches: Record<string, unknown>[] = [];
  const fbItems: InboxItem[] = [
    {
      key: "identity:9001",
      kind: "unknown",
      leadId: null,
      conversationId: 9001,
      identityId: 9001,
      title: "FB Trong 24h",
      lastMessagePreview: "Chào shop",
      lastMessageAt: new Date(now - 2 * 60 * 60 * 1000).toISOString(),
      unreadCount: 0,
      channels: [{ channel: "messenger", messageCount: 1, unreadCount: 0 }],
      replyTargets: [{
        conversationId: 9001,
        channelAccountId: 90,
        channel: "messenger",
        conversationKind: "direct",
        lastInboundAt: new Date(now - 2 * 60 * 60 * 1000).toISOString(),
      }],
    },
    {
      key: "identity:9002",
      kind: "unknown",
      leadId: null,
      conversationId: 9002,
      identityId: 9002,
      title: "FB Ngoài 24h",
      lastMessagePreview: "Hôm qua hỏi giá",
      lastMessageAt: new Date(now - 36 * 60 * 60 * 1000).toISOString(),
      unreadCount: 0,
      channels: [{ channel: "messenger", messageCount: 1, unreadCount: 0 }],
      replyTargets: [{
        conversationId: 9002,
        channelAccountId: 90,
        channel: "messenger",
        conversationKind: "direct",
        lastInboundAt: new Date(now - 36 * 60 * 60 * 1000).toISOString(),
      }],
    },
    {
      key: "identity:9003",
      kind: "unknown",
      leadId: null,
      conversationId: 9003,
      identityId: 9003,
      title: "FB Quá 7 ngày",
      lastMessagePreview: "Tuần trước",
      lastMessageAt: new Date(now - 9 * 24 * 60 * 60 * 1000).toISOString(),
      unreadCount: 0,
      channels: [{ channel: "messenger", messageCount: 1, unreadCount: 0 }],
      replyTargets: [{
        conversationId: 9003,
        channelAccountId: 90,
        channel: "messenger",
        conversationKind: "direct",
        lastInboundAt: new Date(now - 9 * 24 * 60 * 60 * 1000).toISOString(),
      }],
    },
  ];

  const app = await openConversation({
    "rustsale/native": {
      "Conversations.inboxPage": () => ({ items: fbItems, total: fbItems.length, nextCursor: null }),
      "ConversationActions.identityTimeline": (id: unknown) => ({
        items: [{
          id: Number(id) * 10,
          conversationId: Number(id),
          channel: "messenger",
          direction: "inbound",
          contentType: "text",
          textContent: "Tin nhắn Facebook",
          occurredAt: new Date(now - 36 * 60 * 60 * 1000).toISOString(),
          status: "received",
        }],
        nextCursor: null,
      }),
      "ConversationActions.pinnedMessages": () => [],
      "ConversationActions.quickTemplates": () => [],
      "ConversationActions.sendBatch": (req: unknown) => {
        sentBatches.push(req as Record<string, unknown>);
        return { batchId: "ok", messages: [] };
      },
    },
  });

  // Inside 24h -> free window countdown
  await app.waitFor("id:inbox-identity:9001", { timeout: 8000 });
  await app.press("id:inbox-identity:9001");
  await app.waitFor("id:facebook-policy-inside", { timeout: 8000 });
  expect(has(app, "Cửa sổ miễn phí 24h")).toBe(true);

  // Outside 24h (within 7d) -> blocked without tag, allowed with HUMAN_AGENT
  await app.press("id:inbox-identity:9002");
  await app.waitFor("id:facebook-policy-outside", { timeout: 8000 });
  await app.type("id:composer", "Hỗ trợ tiếp");
  await app.press("id:send");
  await app.waitUntil(() => has(app, "HUMAN_AGENT"), { timeout: 4000 });
  expect(sentBatches.length).toBe(0);

  await app.press("id:fb-tag-human-agent");
  await app.press("id:send");
  await app.waitUntil(() => sentBatches.length === 1, { timeout: 8000 });
  expect(sentBatches[0].messageTag).toBe("HUMAN_AGENT");

  // Beyond 7d -> expired hard block
  await app.press("id:inbox-identity:9003");
  await app.waitFor("id:facebook-policy-expired", { timeout: 8000 });
  await app.type("id:composer", "Không gửi được");
  await app.press("id:send");
  await app.waitUntil(() => has(app, "7 ngày"), { timeout: 4000 });
  expect(sentBatches.length).toBe(1);
});
