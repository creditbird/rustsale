// Headless: sending one Zalo message and one email from the lead detail panel. The two send
// exports and the account/template lists are stubbed: a test never reaches a real channel.
import { expect, open, test } from "nsh/test";
import { Host, Leads, LeadsMore } from "rustsale/native";
import { eventually } from "./testing";

const dir = `/tmp/rustsale-v3-ui-l5c-send-${Date.now()}`;

type App = Awaited<ReturnType<typeof open>>;
const has = (app: App, text: string) => app.texts().some((t) => t.includes(text));

interface Calls {
  zalo: unknown[][];
  email: unknown[][];
}

interface StubZaloAccount {
  account_key: string;
  display_name: string | null;
  auth_status: string;
}

const DEFAULT_ZALO_ACCOUNTS: StubZaloAccount[] = [
  { account_key: "acc-z", display_name: "Zalo Một", auth_status: "valid" },
];

async function openLead(
  state: Calls,
  name: string,
  id: number,
  zaloAccounts: StubZaloAccount[] = DEFAULT_ZALO_ACCOUNTS,
) {
  await Host.start(dir);
  const app = await open("./App.tsx", {
    modules: {
      "rustsale/native": {
        "Zalo.accounts": () => zaloAccounts,
        "Campaigns.outreachTemplates": () => [
          { id: 7, name: "Mẫu Zalo", content: "Chào [name]" },
        ],
        "Campaigns.startZaloOutreach": (...args: unknown[]) => {
          state.zalo.push(args);
          return new Promise((resolve) => setTimeout(() => resolve({ started: 1 }), 300));
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

test("one Zalo message is queued with the chosen account and template, then logged", async () => {
  const stamp = Date.now();
  const name = `Gui Zalo ${stamp}`;
  const id = (await Leads.add({ name, status: "New", phone: "0901234567" })) as number;
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
    return acts.some(
      (a) => a.activity_type === "Zalo" && a.content === "Đã xếp hàng gửi Zalo: Mẫu Zalo",
    );
  });
  await app.waitUntil(() => has(app, "Đã xếp hàng gửi Zalo: Mẫu Zalo"));
});

test("one email is queued with the chosen SMTP and template, then logged", async () => {
  const stamp = Date.now();
  const name = `Gui Email ${stamp}`;
  const id = (await Leads.add({ name, email: `lead${stamp}@example.test`, status: "New" })) as number;
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
    return acts.some(
      (a) => a.activity_type === "Email" && a.content === "Đã xếp hàng gửi email: Mẫu Email",
    );
  });
  await app.waitUntil(() => has(app, "Đã xếp hàng gửi email: Mẫu Email"));
});

test("a lead without an email is refused before any SMTP call", async () => {
  const stamp = Date.now();
  const name = `Gui thieu email ${stamp}`;
  const id = (await Leads.add({ name, status: "New" })) as number;
  const state: Calls = { zalo: [], email: [] };
  const app = await openLead(state, name, id);

  await app.press("id:lead-send-email");
  await app.waitUntil(() => has(app, "Khách chưa có email hợp lệ."));
  expect(state.email.length).toBe(0);
  expect(app.shows("id:send-email-confirm")).toBe(false);
});

test("a lead without a phone or Zalo is refused before any Zalo call", async () => {
  const stamp = Date.now();
  const name = `Gui thieu sdt ${stamp}`;
  const id = (await Leads.add({ name, status: "New" })) as number;
  const state: Calls = { zalo: [], email: [] };
  const app = await openLead(state, name, id);

  await app.press("id:lead-send-zalo");
  await app.waitUntil(() => has(app, "Khách chưa có số điện thoại hoặc Zalo."));
  expect(state.zalo.length).toBe(0);
  expect(app.shows("id:send-zalo-confirm")).toBe(false);
});

test("Zalo send is disabled with an error when no account has valid auth_status", async () => {
  const stamp = Date.now();
  const name = `Gui Zalo het han ${stamp}`;
  const id = (await Leads.add({ name, status: "New", phone: "0901234567" })) as number;
  const state: Calls = { zalo: [], email: [] };
  const app = await openLead(state, name, id, [
    { account_key: "acc-expired", display_name: "Zalo Hết hạn", auth_status: "expired" },
  ]);

  await app.press("id:lead-send-zalo");
  await app.waitFor("id:send-zalo-error");
  await app.waitUntil(() => has(app, "Chưa có tài khoản Zalo hợp lệ"));
  expect(has(app, "Zalo Hết hạn")).toBe(false);
  await expect(app.press("id:send-zalo-confirm")).rejects.toMatchObject({
    code: "ERR_NSH_TEST_DISABLED",
  });
  expect(state.zalo.length).toBe(0);
  expect(app.shows("id:send-zalo-confirm")).toBe(true);
});

test("Zalo send preselects the valid account when an expired account comes first", async () => {
  const stamp = Date.now();
  const name = `Gui Zalo hop le ${stamp}`;
  const id = (await Leads.add({ name, status: "New", phone: "0901234567" })) as number;
  const state: Calls = { zalo: [], email: [] };
  const app = await openLead(state, name, id, [
    { account_key: "acc-expired", display_name: "Zalo Hết hạn", auth_status: "expired" },
    { account_key: "acc-valid", display_name: "Zalo Hợp lệ", auth_status: "valid" },
  ]);

  await app.press("id:lead-send-zalo");
  await app.waitFor("id:send-zalo-account-trigger");
  await app.waitUntil(() => has(app, "Zalo Hợp lệ"));
  expect(has(app, "Zalo Hết hạn")).toBe(false);
  expect(app.shows("id:send-zalo-error")).toBe(false);
  await app.press("id:send-zalo-confirm");
  await eventually(async () => state.zalo.length === 1);
  await app.waitFor("id:send-zalo-confirm", { state: "gone" });

  expect(state.zalo[0][0]).toEqual(["acc-valid"]);
  expect(state.zalo[0][1]).toBe(7);
  expect(state.zalo[0][2]).toEqual([id]);
});
