// Headless: the Facebook Hub screen (UI-36) with all `Facebook.*` native exports stubbed,
// so no Graph API, OAuth, or network calls occur and no shared core DB rows are mutated.
import { expect, open, test } from "nsh/test";
import { Host } from "rustsale/native";
import { eventually } from "./testing";
import {
  extractHostname,
  filterCommentLogs,
  parseCsvList,
  parseScheduledTime,
  validatePostDraft,
  validateRuleDraft,
  type CommentAutomationRule,
  type CommentLog,
  type CreateCommentAutomationPayload,
  type CreateFacebookPostPayload,
  type FacebookAccount,
  type FacebookScheduledPost,
  type PageHealth,
  type UpdateCommentAutomationPayload,
} from "./stores/facebookHub";

const dir = `/tmp/rustsale-v3-ui-facebook-hub-${Date.now()}`;

type App = Awaited<ReturnType<typeof open>>;
const has = (app: App, text: string) => app.texts().some((t) => t.includes(text));

interface StubState {
  accounts: FacebookAccount[];
  accountsFail: boolean;
  postsByPage: Record<string, FacebookScheduledPost[]>;
  rulesByPage: Record<string, CommentAutomationRule[]>;
  logsByPage: Record<string, CommentLog[]>;
  healthByPage: Record<string, PageHealth>;
  healthFail: boolean;
  scheduleFail: boolean;
  scheduleDelayMs: number;
  scheduleCalls: CreateFacebookPostPayload[];
  publishCalls: number[];
  cancelCalls: number[];
  createRuleCalls: CreateCommentAutomationPayload[];
  updateRuleCalls: UpdateCommentAutomationPayload[];
  deleteRuleCalls: number[];
  logsCalls: { pageId: string | null; limit: number | null }[];
  healthCalls: string[];
  nextPostId: number;
  nextRuleId: number;
}

function makeAccount(id: number, pageId: string, pageName: string, tokenStatus = "valid"): FacebookAccount {
  return {
    channelAccountId: id,
    pageId,
    pageName,
    avatarUrl: null,
    tokenStatus,
    grantedScopes: ["pages_manage_posts", "pages_read_engagement"],
    runtimeStatus: "connected",
    lastConnectedAt: "2026-11-25T01:00:00Z",
    lastError: null,
    updatedAt: "2026-11-25T01:00:00Z",
  };
}

function makeState(overrides: Partial<StubState> = {}): StubState {
  const pageA = makeAccount(101, "page-a", "Shop Alpha");
  const pageB = makeAccount(102, "page-b", "Shop Beta");
  return {
    accounts: [pageA, pageB],
    accountsFail: false,
    postsByPage: {
      "page-a": [],
      "page-b": [],
    },
    rulesByPage: {
      "page-a": [],
      "page-b": [],
    },
    logsByPage: {
      "page-a": [],
      "page-b": [],
    },
    healthByPage: {
      "page-a": {
        page_id: "page-a",
        token_status: "valid",
        granted_scopes: ["pages_manage_posts", "pages_read_engagement"],
        active_scheduled_count: 1,
        active_automations_count: 2,
        updated_at: "2026-11-25T05:00:00Z",
      },
      "page-b": {
        page_id: "page-b",
        token_status: "expired",
        granted_scopes: ["pages_show_list"],
        active_scheduled_count: 0,
        active_automations_count: 0,
        updated_at: "2026-11-25T05:00:00Z",
      },
    },
    healthFail: false,
    scheduleFail: false,
    scheduleDelayMs: 0,
    scheduleCalls: [],
    publishCalls: [],
    cancelCalls: [],
    createRuleCalls: [],
    updateRuleCalls: [],
    deleteRuleCalls: [],
    logsCalls: [],
    healthCalls: [],
    nextPostId: 500,
    nextRuleId: 800,
    ...overrides,
  };
}

function modulesFor(state: StubState) {
  return {
    modules: {
      "rustsale/native": {
        "Facebook.accounts": () => {
          if (state.accountsFail) {
            throw Object.assign(new Error("accounts unavailable"), { code: "unavailable" });
          }
          return state.accounts.map((a) => ({ ...a }));
        },
        "Facebook.scheduledPosts": (pageId: string | null) => {
          const list = pageId ? (state.postsByPage[pageId] ?? []) : Object.values(state.postsByPage).flat();
          return list.map((p) => ({ ...p, media_paths: [...p.media_paths] }));
        },
        "Facebook.schedulePost": async (payload: CreateFacebookPostPayload) => {
          state.scheduleCalls.push(payload);
          if (state.scheduleDelayMs > 0) {
            await new Promise((resolve) => setTimeout(resolve, state.scheduleDelayMs));
          }
          if (state.scheduleFail) {
            throw Object.assign(new Error("meta unavailable"), { code: "unavailable" });
          }
          const id = state.nextPostId++;
          const post: FacebookScheduledPost = {
            id,
            draft_group_id: payload.draft_group_id ?? `dg-${id}`,
            channel_account_id: payload.channel_account_id,
            page_id: payload.page_id,
            post_type: payload.post_type,
            message: payload.message,
            link_url: payload.link_url ?? null,
            media_paths: payload.media_paths ?? [],
            scheduling_mode: payload.scheduled_publish_time ? "meta" : "meta",
            scheduled_publish_time: payload.scheduled_publish_time ?? null,
            remote_post_id: null,
            status: payload.scheduled_publish_time ? "queued" : "draft",
            error_code: null,
            error_message: null,
            created_at: "2026-11-25T06:00:00Z",
            updated_at: "2026-11-25T06:00:00Z",
          };
          state.postsByPage[payload.page_id] = [post, ...(state.postsByPage[payload.page_id] ?? [])];
          return id;
        },
        "Facebook.publishPostNow": (postId: number) => {
          state.publishCalls.push(postId);
          for (const list of Object.values(state.postsByPage)) {
            const found = list.find((p) => p.id === postId);
            if (found) {
              found.status = "queued";
              found.scheduled_publish_time = null;
              return true;
            }
          }
          return false;
        },
        "Facebook.cancelScheduledPost": (postId: number) => {
          state.cancelCalls.push(postId);
          for (const key of Object.keys(state.postsByPage)) {
            const before = state.postsByPage[key].length;
            state.postsByPage[key] = state.postsByPage[key].filter((p) => p.id !== postId);
            if (state.postsByPage[key].length < before) return true;
          }
          return false;
        },
        "Facebook.commentAutomations": (pageId: string | null) => {
          const list = pageId ? (state.rulesByPage[pageId] ?? []) : Object.values(state.rulesByPage).flat();
          return list.map((r) => ({ ...r, keywords: [...r.keywords], post_ids: [...r.post_ids] }));
        },
        "Facebook.createCommentAutomation": (payload: CreateCommentAutomationPayload) => {
          state.createRuleCalls.push(payload);
          const id = state.nextRuleId++;
          const rule: CommentAutomationRule = {
            id,
            channel_account_id: payload.channel_account_id,
            page_id: payload.page_id,
            rule_name: payload.rule_name,
            post_ids: payload.post_ids ?? [],
            keywords: payload.keywords ?? [],
            match_mode: payload.match_mode ?? "contains",
            auto_like: payload.auto_like ?? true,
            public_reply_template: payload.public_reply_template ?? null,
            private_reply_template: payload.private_reply_template ?? null,
            is_enabled: payload.is_enabled ?? true,
            created_at: "2026-11-25T06:00:00Z",
            updated_at: "2026-11-25T06:00:00Z",
          };
          state.rulesByPage[payload.page_id] = [rule, ...(state.rulesByPage[payload.page_id] ?? [])];
          return id;
        },
        "Facebook.updateCommentAutomation": (payload: UpdateCommentAutomationPayload) => {
          state.updateRuleCalls.push(payload);
          for (const list of Object.values(state.rulesByPage)) {
            const found = list.find((r) => r.id === payload.id);
            if (found) {
              found.rule_name = payload.rule_name;
              found.post_ids = payload.post_ids ?? [];
              found.keywords = payload.keywords ?? [];
              found.match_mode = payload.match_mode ?? found.match_mode;
              found.auto_like = payload.auto_like ?? found.auto_like;
              found.public_reply_template = payload.public_reply_template ?? null;
              found.private_reply_template = payload.private_reply_template ?? null;
              found.is_enabled = payload.is_enabled ?? found.is_enabled;
              return true;
            }
          }
          return false;
        },
        "Facebook.deleteCommentAutomation": (ruleId: number) => {
          state.deleteRuleCalls.push(ruleId);
          for (const key of Object.keys(state.rulesByPage)) {
            const before = state.rulesByPage[key].length;
            state.rulesByPage[key] = state.rulesByPage[key].filter((r) => r.id !== ruleId);
            if (state.rulesByPage[key].length < before) return true;
          }
          return false;
        },
        "Facebook.commentLogs": (pageId: string | null, limit: number | null) => {
          state.logsCalls.push({ pageId, limit });
          const list = pageId ? (state.logsByPage[pageId] ?? []) : Object.values(state.logsByPage).flat();
          return list.map((l) => ({ ...l }));
        },
        "Facebook.pageHealth": (pageId: string) => {
          state.healthCalls.push(pageId);
          if (state.healthFail) {
            throw Object.assign(new Error("health query failed"), { code: "unavailable" });
          }
          const h = state.healthByPage[pageId];
          if (!h) {
            throw Object.assign(new Error("not found"), { code: "not_found" });
          }
          return { ...h, granted_scopes: [...h.granted_scopes] };
        },
      },
    },
  };
}

async function openFacebookHub(state: StubState): Promise<App> {
  await Host.start(dir);
  const app = await open("./App.tsx", modulesFor(state));
  await app.setViewport(1280, 1080);
  await app.press("id:nav-facebook_hub");
  await app.waitFor("id:fb-hub-screen");
  return app;
}

test("store helpers validate drafts, parse timestamps and filter logs", () => {
  const acc = makeAccount(1, "p-1", "Page 1");
  expect(parseCsvList(" giá , inbox, giá,  ")).toEqual(["giá", "inbox"]);
  expect(extractHostname("https://shop.example.vn/promo?x=1")).toBe("SHOP.EXAMPLE.VN");
  expect(extractHostname("not-a-url")).toBe("EXAMPLE.COM");
  expect(parseScheduledTime("2026-12-01 10:30")).not.toBeNull();
  expect(parseScheduledTime("bad-date")).toBeNull();

  expect(validatePostDraft({
    account: null,
    postType: "status",
    message: "hello",
    linkUrl: "",
    isScheduled: false,
    scheduledDateTime: "",
  })).not.toBeNull();

  expect(validatePostDraft({
    account: acc,
    postType: "link",
    message: "Ưu đãi mới",
    linkUrl: "ftp://bad",
    isScheduled: false,
    scheduledDateTime: "",
  })).toContain("http://");

  expect(validateRuleDraft({
    account: acc,
    ruleName: "Regex bad",
    matchMode: "regex",
    keywords: "[unclosed",
  })).toContain("Biểu thức chính quy");

  const logs: CommentLog[] = [
    {
      id: 1,
      comment_id: "c-ok",
      page_id: "p-1",
      post_id: "post-1",
      sender_psid: "u1",
      sender_name: "Nguyễn An",
      comment_text: "Cho xin giá",
      automation_id: 1,
      like_status: "success",
      public_reply_status: "success",
      private_reply_status: "success",
      skip_reason: null,
      error_message: null,
      created_at: "2026-11-25T05:00:00Z",
      updated_at: "2026-11-25T05:00:00Z",
    },
    {
      id: 2,
      comment_id: "c-fail",
      page_id: "p-1",
      post_id: "post-1",
      sender_psid: "u2",
      sender_name: "Trần Bình",
      comment_text: "Inbox shop",
      automation_id: 1,
      like_status: "success",
      public_reply_status: "failed",
      private_reply_status: "failed",
      skip_reason: null,
      error_message: "Graph rate limit",
      created_at: "2026-11-25T05:05:00Z",
      updated_at: "2026-11-25T05:05:00Z",
    },
  ];
  expect(filterCommentLogs(logs, "", "failed").map((l) => l.id)).toEqual([2]);
  expect(filterCommentLogs(logs, "an", "all").map((l) => l.id)).toEqual([1]);
});

test("empty state shows when no Facebook Page is connected and links to Settings", async () => {
  const state = makeState({ accounts: [] });
  const app = await openFacebookHub(state);
  await app.waitFor("id:fb-empty-state");
  expect(has(app, "Chưa kết nối Facebook Fanpage")).toBe(true);

  await app.press("id:fb-goto-settings");
  await app.waitFor("id:settings-tab-general");
});

test("selecting a Page, scheduling a post, publishing now and cancelling with confirmation", async () => {
  const stamp = Date.now();
  const state = makeState();
  const app = await openFacebookHub(state);
  await app.waitFor("id:fb-page-header");
  await app.waitUntil(() => has(app, "Shop Alpha"));

  // Switch active Page to Shop Beta
  await app.press("id:fb-page-select-trigger");
  await app.waitFor("id:fb-page-select-item-page-b");
  await app.press("id:fb-page-select-item-page-b");
  await app.waitUntil(() => has(app, "Shop Beta"));
  await app.waitUntil(() => has(app, "ID: page-b"));

  // Preview Desktop -> Mobile toggle
  await app.press("id:fb-preview-mobile");
  await app.waitUntil(() => has(app, "Di động"));

  // Empty submit is refused before calling Facebook.schedulePost
  await app.press("id:fb-post-submit");
  await app.waitFor("id:fb-post-error");
  expect(has(app, "Nội dung bài viết không được để trống")).toBe(true);
  expect(state.scheduleCalls.length).toBe(0);

  // Compose a photo post with media and schedule time
  await app.press("id:fb-post-type-photo");
  await app.waitFor("id:fb-media-input");
  await app.type("id:fb-media-input", `/tmp/banner-${stamp}.jpg`);
  await app.press("id:fb-media-add");
  await app.waitFor("id:fb-media-item-0");

  const msg = `Khuyến mãi ${stamp}`;
  await app.type("id:fb-post-message", msg);
  await app.press("id:fb-schedule-toggle");
  await app.waitFor("id:fb-schedule-datetime");
  await app.type("id:fb-schedule-datetime", "2026-12-01 10:30");

  await app.press("id:fb-post-submit");
  await eventually(async () => state.scheduleCalls.length === 1);
  expect(state.publishCalls.length).toBe(0);
  expect(state.scheduleCalls[0].page_id).toBe("page-b");
  expect(state.scheduleCalls[0].channel_account_id).toBe(102);
  expect(state.scheduleCalls[0].post_type).toBe("photo");
  expect(state.scheduleCalls[0].media_paths).toEqual([`/tmp/banner-${stamp}.jpg`]);
  expect(state.scheduleCalls[0].scheduled_publish_time).not.toBeNull();

  const createdId = state.postsByPage["page-b"][0].id;
  await app.waitFor(`id:fb-post-row-${createdId}`);
  expect(has(app, msg)).toBe(true);

  // Trigger Publish Now on the scheduled post
  await app.press(`id:fb-post-publish-${createdId}`);
  await eventually(async () => state.publishCalls.includes(createdId));

  // Cancel the post with confirmation dialog
  await app.press(`id:fb-post-cancel-${createdId}`);
  await app.waitFor("id:fb-post-cancel-confirm");
  await app.press("id:fb-post-cancel-confirm");
  await eventually(async () => state.cancelCalls.includes(createdId));
  await app.waitUntil(() => !has(app, msg));
});

test("comment automations CRUD, toggle and comment audit logs filtering and refresh", async () => {
  const stamp = Date.now();
  const state = makeState({
    logsByPage: {
      "page-a": [
        {
          id: 901,
          comment_id: `cmt-ok-${stamp}`,
          page_id: "page-a",
          post_id: "post-1",
          sender_psid: "psid-1",
          sender_name: `Khách Tốt ${stamp}`,
          comment_text: "Cho xin giá nhé",
          automation_id: null,
          like_status: "success",
          public_reply_status: "success",
          private_reply_status: "success",
          skip_reason: null,
          error_message: null,
          created_at: "2026-11-25T05:10:00Z",
          updated_at: "2026-11-25T05:10:00Z",
        },
        {
          id: 902,
          comment_id: `cmt-err-${stamp}`,
          page_id: "page-a",
          post_id: "post-1",
          sender_psid: "psid-2",
          sender_name: `Khách Lỗi ${stamp}`,
          comment_text: "Tư vấn ngay",
          automation_id: null,
          like_status: "success",
          public_reply_status: "failed",
          private_reply_status: "failed",
          skip_reason: null,
          error_message: "Token expired",
          created_at: "2026-11-25T05:12:00Z",
          updated_at: "2026-11-25T05:12:00Z",
        },
      ],
      "page-b": [],
    },
  });

  const app = await openFacebookHub(state);
  await app.waitFor("id:fb-tab-automations");
  await app.press("id:fb-tab-automations");
  await app.waitFor("id:fb-rules-card");

  // Create rule
  await app.press("id:fb-rule-add");
  await app.waitFor("id:fb-rule-name");
  const ruleTitle = `Rule Báo Giá ${stamp}`;
  await app.type("id:fb-rule-name", ruleTitle);
  await app.type("id:fb-rule-keywords", "giá, inbox");
  await app.press("id:fb-rule-mode-exact");
  await app.press("id:fb-rule-save");

  await eventually(async () => state.createRuleCalls.length === 1);
  expect(state.createRuleCalls[0].page_id).toBe("page-a");
  expect(state.createRuleCalls[0].rule_name).toBe(ruleTitle);
  expect(state.createRuleCalls[0].keywords).toEqual(["giá", "inbox"]);
  expect(state.createRuleCalls[0].match_mode).toBe("exact");

  const ruleId = state.rulesByPage["page-a"][0].id;
  await app.waitFor(`id:fb-rule-row-${ruleId}`);
  expect(has(app, ruleTitle)).toBe(true);

  // Toggle rule OFF
  await app.press(`id:fb-rule-toggle-${ruleId}`);
  await eventually(async () => state.updateRuleCalls.some((u) => u.id === ruleId && u.is_enabled === false));
  await app.waitUntil(() => has(app, "Đã tắt"));

  // Edit rule name
  await app.press(`id:fb-rule-edit-${ruleId}`);
  await app.waitFor("id:fb-rule-name");
  await app.clear("id:fb-rule-name");
  const editedTitle = `Rule Đã Sửa ${stamp}`;
  await app.type("id:fb-rule-name", editedTitle);
  await app.press("id:fb-rule-save");
  await eventually(async () => state.updateRuleCalls.some((u) => u.id === ruleId && u.rule_name === editedTitle));
  await app.waitUntil(() => has(app, editedTitle));

  // Delete rule with confirmation
  await app.press(`id:fb-rule-delete-${ruleId}`);
  await app.waitFor("id:fb-rule-delete-confirm");
  await app.press("id:fb-rule-delete-confirm");
  await eventually(async () => state.deleteRuleCalls.includes(ruleId));
  await app.waitUntil(() => !has(app, editedTitle));

  // Verify logs display, filter and refresh
  await app.waitFor("id:fb-log-row-901");
  await app.waitFor("id:fb-log-row-902");
  await app.press("id:fb-logs-filter-failed");
  await app.waitUntil(() => has(app, `Khách Lỗi ${stamp}`) && !has(app, `Khách Tốt ${stamp}`));
  await app.press("id:fb-logs-filter-all");
  await app.waitUntil(() => has(app, `Khách Tốt ${stamp}`));

  state.logsByPage["page-a"].unshift({
    id: 903,
    comment_id: `cmt-new-${stamp}`,
    page_id: "page-a",
    post_id: "post-2",
    sender_psid: "psid-3",
    sender_name: `Khách Mới ${stamp}`,
    comment_text: "Hàng còn không?",
    automation_id: null,
    like_status: "success",
    public_reply_status: "skipped",
    private_reply_status: "skipped",
    skip_reason: "outside_window",
    error_message: null,
    created_at: "2026-11-25T05:15:00Z",
    updated_at: "2026-11-25T05:15:00Z",
  });
  const beforeLogCalls = state.logsCalls.length;
  await app.press("id:fb-logs-refresh");
  await eventually(async () => state.logsCalls.length > beforeLogCalls);
  await app.waitUntil(() => has(app, `Khách Mới ${stamp}`));
});

test("page health metrics load and refresh on sub-tab 3 with compliance banner", async () => {
  const state = makeState();
  const app = await openFacebookHub(state);
  await app.waitFor("id:fb-tab-compliance");
  await app.press("id:fb-tab-compliance");
  await app.waitFor("id:fb-health-card");
  await app.waitFor("id:fb-compliance-card");

  expect(has(app, "pages_manage_posts")).toBe(true);
  expect(has(app, "Tuân thủ Xóa dữ liệu Meta")).toBe(true);

  state.healthByPage["page-a"] = {
    page_id: "page-a",
    token_status: "valid",
    granted_scopes: ["pages_manage_posts", "pages_messaging"],
    active_scheduled_count: 7,
    active_automations_count: 4,
    updated_at: "2026-11-25T07:30:00Z",
  };
  const beforeHealthCalls = state.healthCalls.length;
  await app.press("id:fb-health-refresh");
  await eventually(async () => state.healthCalls.length > beforeHealthCalls);
  await app.waitUntil(() => has(app, "pages_messaging"));
});

test("submit prevents double-click and surfaces core/API errors for post and page health", async () => {
  const stamp = Date.now();
  const state = makeState({ scheduleDelayMs: 250 });
  const app = await openFacebookHub(state);
  await app.waitFor("id:fb-post-message");

  // Double-click guard while submit is in flight
  await app.type("id:fb-post-message", `Chặn bấm lặp ${stamp}`);
  await app.press("id:fb-post-submit");
  await app.waitUntil(() => has(app, "Đang gửi…"));
  await app.press("id:fb-post-submit").catch(() => undefined);
  await eventually(async () => state.publishCalls.length === 1);
  expect(state.scheduleCalls.length).toBe(1);

  // Core/API failure on schedulePost shows user-facing error
  state.scheduleDelayMs = 0;
  state.scheduleFail = true;
  await app.type("id:fb-post-message", `Lỗi API ${stamp}`);
  await app.press("id:fb-post-submit");
  await app.waitFor("id:fb-post-error");
  expect(has(app, "Dịch vụ tạm thời chưa sẵn sàng")).toBe(true);

  // Core/API failure on pageHealth shows error banner and retry recovers
  await app.press("id:fb-tab-compliance");
  await app.waitFor("id:fb-health-refresh");
  state.healthFail = true;
  await app.press("id:fb-health-refresh");
  await app.waitFor("id:fb-health-error");
  expect(has(app, "Dịch vụ tạm thời chưa sẵn sàng")).toBe(true);

  state.healthFail = false;
  await app.press("id:fb-health-retry");
  await eventually(async () => !app.shows("id:fb-health-error"));
  expect(has(app, "pages_manage_posts")).toBe(true);
});
