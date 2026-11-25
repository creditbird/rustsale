// Headless: sending one Zalo message and one email from the lead detail panel, plus Zalo invalid
// phone validation and queued/skipped result handling in LeadDetail and OutreachScreen.
// All send exports and account/template lists are stubbed: a test never reaches a real channel.
import { expect, open, test } from "nsh/test";
import { Host, Leads, LeadsMore } from "rustsale/native";
import {
  hasValidZaloUid,
  isValidZaloLeadTarget,
  isVnMobile,
  parseZaloOutreachResult,
  type ZaloOutreachStartResult,
} from "./stores/zaloTarget";
import { eventually } from "./testing";

const dir = `/tmp/rustsale-v3-ui-l5c-send-${Date.now()}`;

type App = Awaited<ReturnType<typeof open>>;
const has = (app: App, text: string) => app.texts().some((t) => t.includes(text));

interface Calls {
  zalo: unknown[][];
  email: unknown[][];
}

async function openLead(
  state: Calls,
  name: string,
  id: number,
  zaloResult: ZaloOutreachStartResult = { queued: 1, skipped: 0, skipped_leads: [] },
) {
  await Host.start(dir);
  const app = await open("./App.tsx", {
    modules: {
      "rustsale/native": {
        "Zalo.accounts": () => [
          { account_key: "acc-z", display_name: "Zalo Một", auth_status: "valid" },
        ],
        "Campaigns.outreachTemplates": () => [
          { id: 7, name: "Mẫu Zalo", content: "Chào [name]" },
        ],
        "Campaigns.startZaloOutreach": (...args: unknown[]) => {
          state.zalo.push(args);
          return new Promise((resolve) => setTimeout(() => resolve(zaloResult), 300));
        },
        "Templates.smtpAccounts": () => [
          { id: 3, name: "SMTP Chính", from_email: "sale@example.test" },
        ],
        "Templates.emailTemplates": () => [
          { id: 9, name: "Mẫu Email", subject: "Chào", content: "Nội dung" },
        ],
        "Campaigns.sendEmailOutreach": (...args: unknown[]) => {
          state.email.push(args);
          return new Promise((resolve) => setTimeout(() => resolve({ started: 1 }), 300));
        },
      },
    },
  });
  await app.setViewport(1280, 800);
  await app.press("id:nav-leads");
  await app.waitFor("id:lead-list");
  await app.type("id:lead-search", name);
  await app.waitFor(`id:lead-row-${id}`);
  await app.press(`id:lead-row-${id}`);
  await app.waitFor("id:lead-detail");
  return app;
}

async function openOutreachWithStub(
  state: Calls,
  zaloResult: ZaloOutreachStartResult,
  leadIds: number[],
) {
  await Host.start(dir);
  const app = await open("./App.tsx", {
    modules: {
      "rustsale/native": {
        "Zalo.accounts": () => [
          {
            account_key: "acc-out",
            display_name: "Zalo Chiến Dịch",
            auth_status: "logged_in",
            runtime_status: "connected",
            last_error: null,
          },
        ],
        "Campaigns.outreachTemplates": () => [
          { id: 11, name: "Mẫu Chiến Dịch", content: "Chào [name]", image_url: null, created_at: "" },
        ],
        "LeadsMore.outreachPage": () => ({
          items: leadIds.map((id) => ({
            id,
            name: `Lead CD ${id}`,
            company: null,
            status: "New",
            email: null,
            phone: "0901234567",
            has_zalo: 1,
            zalo_status: "VERIFIED",
            zalo_reached: 0,
          })),
          total: leadIds.length,
          next_cursor: null,
        }),
        "LeadsMore.outreachIds": () => leadIds,
        "Campaigns.startZaloOutreach": (...args: unknown[]) => {
          state.zalo.push(args);
          return Promise.resolve(zaloResult);
        },
      },
    },
  });
  await app.setViewport(1280, 800);
  await app.press("id:nav-outreach");
  await app.waitFor("id:zalo-search");
  await app.waitUntil(() => has(app, "Zalo Chiến Dịch"));
  await app.waitUntil(() => has(app, "Mẫu Chiến Dịch"));
  await app.waitUntil(() => has(app, `Đã chọn ${leadIds.length} / ${leadIds.length} lead`));
  return app;
}

test("zaloTarget helpers mirror core phone and UID validation rules", () => {
  expect(isVnMobile("0901234567")).toBe(true);
  expect(isVnMobile("+84 391-234-567")).toBe(true);
  expect(isVnMobile("0084987654321")).toBe(true);
  expect(isVnMobile("0901234567 (Zalo)")).toBe(true);
  expect(isVnMobile("abc")).toBe(false);
  expect(isVnMobile("12345")).toBe(false);
  expect(isVnMobile("0281234567")).toBe(false);
  expect(isVnMobile("")).toBe(false);

  expect(hasValidZaloUid("9876543210")).toBe(true);
  expect(hasValidZaloUid("0")).toBe(false);
  expect(hasValidZaloUid("0000")).toBe(false);
  expect(hasValidZaloUid("null")).toBe(false);
  expect(hasValidZaloUid("undefined")).toBe(false);
  expect(hasValidZaloUid(null)).toBe(false);

  expect(isValidZaloLeadTarget("abc", "9876543210")).toBe(true);
  expect(isValidZaloLeadTarget("abc", null)).toBe(false);
  expect(isValidZaloLeadTarget("12345", "0")).toBe(false);

  expect(parseZaloOutreachResult({ queued: 2, skipped: 1, skipped_leads: [{ lead_id: 5, reason: "invalid_phone" }] }))
    .toEqual({ queued: 2, skipped: 1, skipped_leads: [{ lead_id: 5, reason: "invalid_phone" }] });
  expect(parseZaloOutreachResult(null)).toEqual({ queued: 0, skipped: 0, skipped_leads: [] });
});

test("one Zalo message is sent with the chosen account and template, then logged", async () => {
  await Host.start(dir);
  const stamp = Date.now();
  const name = `Gui Zalo ${stamp}`;
  const id = (await Leads.add({ name, status: "New", phone: "0901234567" })) as number;
  try {
    const state: Calls = { zalo: [], email: [] };
    const app = await openLead(state, name, id);

    await app.press("id:lead-send-zalo");
    await app.waitFor("id:send-zalo-account-trigger");
    await app.waitUntil(() => has(app, "Zalo Một"));
    await app.press("id:send-zalo-confirm");
    await app.waitUntil(() => has(app, "Đang gửi…"));
    await eventually(async () => state.zalo.length === 1);
    await app.waitFor("id:send-zalo-confirm", { state: "gone" });

    expect(state.zalo[0][0]).toEqual(["acc-z"]);
    expect(state.zalo[0][1]).toBe(7);
    expect(state.zalo[0][2]).toEqual([id]);
    await eventually(async () => {
      const acts = (await LeadsMore.activities(id)) as { activity_type: string; content: string }[];
      return acts.some((a) => a.activity_type === "Zalo" && a.content.includes("Mẫu Zalo"));
    });
    await app.waitUntil(() => has(app, "Đã gửi Zalo: Mẫu Zalo"));
  } finally {
    await LeadsMore.delete(id);
  }
});

test("one email is sent with the chosen SMTP and template, then logged", async () => {
  await Host.start(dir);
  const stamp = Date.now();
  const name = `Gui Email ${stamp}`;
  const id = (await Leads.add({ name, email: `lead${stamp}@example.test`, status: "New" })) as number;
  try {
    const state: Calls = { zalo: [], email: [] };
    const app = await openLead(state, name, id);

    await app.press("id:lead-send-email");
    await app.waitFor("id:send-email-smtp-trigger");
    await app.waitUntil(() => has(app, "SMTP Chính"));
    await app.press("id:send-email-confirm");
    await eventually(async () => state.email.length === 1);
    await app.waitFor("id:send-email-confirm", { state: "gone" });

    expect(state.email[0][0]).toBe(3);
    expect(state.email[0][1]).toBe(9);
    expect(state.email[0][2]).toEqual([id]);
    await eventually(async () => {
      const acts = (await LeadsMore.activities(id)) as { activity_type: string; content: string }[];
      return acts.some((a) => a.activity_type === "Email" && a.content.includes("Mẫu Email"));
    });
    await app.waitUntil(() => has(app, "Đã gửi email: Mẫu Email"));
  } finally {
    await LeadsMore.delete(id);
  }
});

test("a lead without an email is refused before any SMTP call", async () => {
  await Host.start(dir);
  const stamp = Date.now();
  const name = `Gui thieu email ${stamp}`;
  const id = (await Leads.add({ name, status: "New" })) as number;
  try {
    const state: Calls = { zalo: [], email: [] };
    const app = await openLead(state, name, id);

    await app.press("id:lead-send-email");
    await app.waitUntil(() => has(app, "Khách chưa có email hợp lệ."));
    expect(state.email.length).toBe(0);
    expect(app.shows("id:send-email-confirm")).toBe(false);
  } finally {
    await LeadsMore.delete(id);
  }
});

test("a lead without a phone or Zalo is refused before any Zalo call", async () => {
  await Host.start(dir);
  const stamp = Date.now();
  const name = `Gui thieu sdt ${stamp}`;
  const id = (await Leads.add({ name, status: "New" })) as number;
  try {
    const state: Calls = { zalo: [], email: [] };
    const app = await openLead(state, name, id);

    await app.press("id:lead-send-zalo");
    await app.waitUntil(() => has(app, "Khách chưa có số điện thoại hoặc Zalo."));
    expect(state.zalo.length).toBe(0);
    expect(app.shows("id:send-zalo-confirm")).toBe(false);
  } finally {
    await LeadsMore.delete(id);
  }
});

test("a lead with an invalid phone ('abc' or '12345') is refused before any Zalo call and logs nothing", async () => {
  await Host.start(dir);
  const stamp = Date.now();
  const cases = [
    { label: "abc", phone: "abc" },
    { label: "short", phone: "12345" },
  ];
  for (const item of cases) {
    const name = `Gui sdt sai ${item.label} ${stamp}`;
    const id = (await Leads.add({ name, status: "New", phone: item.phone })) as number;
    try {
      const state: Calls = { zalo: [], email: [] };
      const app = await openLead(state, name, id);

      await app.press("id:lead-send-zalo");
      await app.waitUntil(() => has(app, "SĐT không hợp lệ, không gửi."));
      expect(state.zalo.length).toBe(0);
      expect(app.shows("id:send-zalo-confirm")).toBe(false);
      expect(has(app, "Đã xếp hàng gửi Zalo")).toBe(false);
      const acts = (await LeadsMore.activities(id)) as { activity_type: string; content: string }[];
      expect(acts.some((a) => a.activity_type === "Zalo")).toBe(false);
    } finally {
      await LeadsMore.delete(id);
    }
  }
});

test("when core skips the lead (queued = 0), LeadDetail reports error and logs no activity", async () => {
  await Host.start(dir);
  const stamp = Date.now();
  const name = `Gui core bo qua ${stamp}`;
  const id = (await Leads.add({ name, status: "New", phone: "0901234567" })) as number;
  try {
    const state: Calls = { zalo: [], email: [] };
    const app = await openLead(state, name, id, {
      queued: 0,
      skipped: 1,
      skipped_leads: [{ lead_id: id, reason: "invalid_phone" }],
    });

    await app.press("id:lead-send-zalo");
    await app.waitFor("id:send-zalo-account-trigger");
    await app.waitUntil(() => has(app, "Zalo Một"));
    await app.press("id:send-zalo-confirm");
    await eventually(async () => state.zalo.length === 1);
    await app.waitFor("id:send-zalo-error");
    await app.waitUntil(() => has(app, "SĐT không hợp lệ, không gửi."));

    expect(has(app, "Đã xếp hàng gửi Zalo")).toBe(false);
    expect(has(app, "Đã gửi Zalo: Mẫu Zalo")).toBe(false);
    const acts = (await LeadsMore.activities(id)) as { activity_type: string; content: string }[];
    expect(acts.some((a) => a.activity_type === "Zalo")).toBe(false);
  } finally {
    await LeadsMore.delete(id);
  }
});

test("when core returns both queued and skipped, LeadDetail reports both counts and logs activity", async () => {
  await Host.start(dir);
  const stamp = Date.now();
  const name = `Gui xep va bo ${stamp}`;
  const id = (await Leads.add({ name, status: "New", phone: "0901234567" })) as number;
  try {
    const state: Calls = { zalo: [], email: [] };
    const app = await openLead(state, name, id, {
      queued: 2,
      skipped: 1,
      skipped_leads: [{ lead_id: 999999, reason: "invalid_phone" }],
    });

    await app.press("id:lead-send-zalo");
    await app.waitFor("id:send-zalo-account-trigger");
    await app.waitUntil(() => has(app, "Zalo Một"));
    await app.press("id:send-zalo-confirm");
    await eventually(async () => state.zalo.length === 1);
    await app.waitFor("id:send-zalo-confirm", { state: "gone" });
    await app.waitUntil(() => has(app, "Đã xếp 2, bỏ 1"));

    await eventually(async () => {
      const acts = (await LeadsMore.activities(id)) as { activity_type: string; content: string }[];
      return acts.some((a) => a.activity_type === "Zalo" && a.content.includes("Mẫu Zalo"));
    });
  } finally {
    await LeadsMore.delete(id);
  }
});

test("OutreachScreen reports invalid phone error when queued = 0", async () => {
  const state: Calls = { zalo: [], email: [] };
  const app = await openOutreachWithStub(
    state,
    {
      queued: 0,
      skipped: 2,
      skipped_leads: [
        { lead_id: 101, reason: "invalid_phone" },
        { lead_id: 102, reason: "missing_phone" },
      ],
    },
    [101, 102],
  );

  await app.press("id:zalo-run");
  await eventually(async () => state.zalo.length === 1);
  await app.waitFor("id:zalo-run-error");
  await app.waitUntil(() => has(app, "SĐT không hợp lệ, không gửi."));
  expect(has(app, "Đã bắt đầu gửi")).toBe(false);
});

test("OutreachScreen reports queued and skipped counts when both are non-zero", async () => {
  const state: Calls = { zalo: [], email: [] };
  const app = await openOutreachWithStub(
    state,
    {
      queued: 2,
      skipped: 1,
      skipped_leads: [{ lead_id: 203, reason: "invalid_phone" }],
    },
    [201, 202, 203],
  );

  await app.press("id:zalo-run");
  await eventually(async () => state.zalo.length === 1);
  await app.waitUntil(() => has(app, "Đã xếp 2, bỏ 1"));
});
