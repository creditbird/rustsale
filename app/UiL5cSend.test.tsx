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

async function openLead(state: Calls, name: string, id: number) {
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

test("one Zalo message is sent with the chosen account and template, then logged", async () => {
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
    return acts.some((a) => a.activity_type === "Zalo" && a.content.includes("Mẫu Zalo"));
  });
  await app.waitUntil(() => has(app, "Đã gửi Zalo: Mẫu Zalo"));
});

test("one email is sent with the chosen SMTP and template, then logged", async () => {
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
    return acts.some((a) => a.activity_type === "Email" && a.content.includes("Mẫu Email"));
  });
  await app.waitUntil(() => has(app, "Đã gửi email: Mẫu Email"));
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
