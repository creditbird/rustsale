// Headless: the WhatsApp Outreach screen against the real native module.
// The create test stubs only the connected-account list, so no real WhatsApp account is touched.
import { expect, open, test } from "nsh/test";
import { eventually } from "./testing";
import { Campaigns, Host, Leads, LeadsMore } from "rustsale/native";

const dir = `/tmp/rustsale-v3-ui-l3-wa-${Date.now()}`;

const has = (app: Awaited<ReturnType<typeof openWhatsApp>>, s: string) => app.texts().some((t) => t.includes(s));

async function openWhatsApp(options?: Parameters<typeof open>[1]) {
  await Host.start(dir);
  const app = await open("./App.tsx", options);
  await app.setViewport(1280, 800);
  await app.press("id:nav-whatsapp_outreach");
  await app.waitFor("id:wa-search");
  return app;
}

async function createCampaign(name: string, quiet = false): Promise<number> {
  return (await Campaigns.createWhatsappCampaign({
    name,
    template_id: null,
    assigned_account_ids: [1],
    rate_limit_min_delay_secs: 8,
    rate_limit_max_delay_secs: 20,
    burst_limit_per_batch: 25,
    burst_pause_secs: 60,
    quiet_hours_enabled: quiet,
    quiet_hours_start: "00:00",
    quiet_hours_end: "23:59",
    timezone: "Asia/Ho_Chi_Minh",
  })) as number;
}

test("an unknown outreach channel is invalid_request", async () => {
  await Host.start(dir);
  await expect(LeadsMore.outreachPage({ channel: "sms", segmentation: "all" }, null, 1))
    .rejects.toMatchObject({ code: "invalid_request" });
});

test("campaigns, search, status filter and the recipient ledger come from the core", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const first = await createCampaign(`WA L3A ${stamp}`);
  await createCampaign(`WA L3B ${stamp}`);
  await Campaigns.populateWhatsappCampaignRecipients(first, [
    { lead_id: null, recipient_phone_raw: "+84901234567", template: "Chào [name]", lead_vars: { name: "An" } },
    { lead_id: null, recipient_phone_raw: "+84901234568", template: "Chào [name]", lead_vars: { name: "Bình" } },
  ], "Chào [name]");

  const app = await openWhatsApp();
  await app.waitUntil(() => has(app, `WA L3A ${stamp}`));
  await app.waitUntil(() => has(app, `WA L3B ${stamp}`));

  await app.type("id:wa-search", `WA L3A ${stamp}`);
  await app.waitUntil(() => has(app, `WA L3A ${stamp}`) && !has(app, `WA L3B ${stamp}`));
  await app.press("id:wa-filter-running");
  await app.waitUntil(() => has(app, "Chưa có chiến dịch nào"));
  await app.press("id:wa-filter-all");
  await app.waitUntil(() => has(app, `WA L3A ${stamp}`));

  await app.clear("id:wa-search");
  await app.waitUntil(() => has(app, `WA L3B ${stamp}`));
  await app.press(`id:wa-detail-${first}`);
  await app.waitFor("id:wa-recipients-list");
  await app.waitUntil(() => has(app, "84901234567") && has(app, "84901234568"));
  await app.waitUntil(() => has(app, "2 / 2 người nhận"));
  await app.press("id:wa-details-close");
  await app.type("id:wa-search", `WA L3B ${stamp}`);
  await app.waitUntil(() => has(app, `WA L3B ${stamp}`));
});

test("the WhatsApp picker keeps international and landline numbers Zalo drops", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  await Leads.add({ name: `WA intl ${stamp}`, phone: "+14155552671", status: "New" });
  await Leads.add({ name: `WA land ${stamp}`, phone: "02838221234", status: "New" });
  const app = await openWhatsApp();
  await app.press("id:wa-new");
  await app.waitFor("id:wa-name");
  await app.type("id:wap-search", `${stamp}`);
  await app.wait(400);
  await app.waitUntil(() => has(app, "Đã chọn 2 / 2 lead"));
  await app.waitUntil(() => has(app, `WA intl ${stamp}`) && has(app, `WA land ${stamp}`));
});

test("the create dialog validates, previews manual phones and creates through the core", async () => {
  const stamp = Date.now();
  const name = `WA L3 create ${stamp}`;
  await Host.start(dir);
  await Leads.add({ name: `Lead L3 ${stamp}`, phone: "0901234567", status: "New" });
  const app = await openWhatsApp({
    modules: {
      "rustsale/native": {
        "ConversationNetwork.whatsappAccounts": () => [
          {
            id: 1,
            provider_account_key: "demo-account",
            display_name: "Demo WhatsApp",
            phone_number: "+84900000000",
          },
        ],
      },
    },
  });
  await app.press("id:wa-new");
  await app.waitFor("id:wa-name");
  await app.press("id:wa-create-submit");
  await app.waitFor("id:wa-create-error");
  await app.waitUntil(() => has(app, "Cần nhập tên chiến dịch"));

  await app.type("id:wa-name", name);
  await app.press("id:wa-mode-manual");
  await app.type("id:wa-manual", "0901234567, 0901234567, 0912345678");
  await app.waitUntil(() => has(app, "Số hợp lệ sau chuẩn hoá (2)"));
  await app.press("id:wa-mode-crm");

  await app.type("id:wap-search", `Lead L3 ${stamp}`);
  await app.wait(400);
  await app.waitUntil(() => has(app, "Đã chọn 1 / 1 lead"));
  await app.press("id:wa-create-submit");
  await app.waitUntil(() => !app.shows("id:wa-name"));
  await app.waitUntil(() => has(app, name));
  await eventually(async () => {
    const rows = (await Campaigns.whatsappCampaigns()) as { name: string; total_recipients: number }[];
    return rows.some((row) => row.name === name && row.total_recipients === 1);
  });
});

test("start, resume and cancel change the status in the core", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const first = await createCampaign(`WA L3 run ${stamp}`, true);
  const app = await openWhatsApp();
  await app.waitUntil(() => has(app, `WA L3 run ${stamp}`));

  await app.press(`id:wa-start-${first}`);
  await app.waitUntil(() => has(app, "queued") || has(app, "running"));
  if (app.shows(`id:wa-pause-${first}`)) {
    await app.press(`id:wa-pause-${first}`);
  } else {
    await Campaigns.pauseWhatsappCampaign(first);
    await app.press("id:wa-refresh");
  }
  await app.waitFor(`id:wa-resume-${first}`, { timeout: 10000 });
  await app.press(`id:wa-resume-${first}`);
  await app.waitFor(`id:wa-start-${first}`);
  await app.waitUntil(() => has(app, "queued"));

  const second = await createCampaign(`WA L3 cancel ${stamp}`, true);
  await Campaigns.pauseWhatsappCampaign(second);
  await app.press("id:wa-refresh");
  await app.waitFor(`id:wa-cancel-${second}`);
  await app.press(`id:wa-cancel-${second}`);
  await app.waitUntil(() => has(app, "cancelled"));
});

test("the pause button pauses a campaign the core reports as running", async () => {
  const stamp = Date.now();
  const paused: number[] = [];
  const running = {
    id: 77,
    name: `WA L3 running ${stamp}`,
    template_id: null,
    status: "running",
    total_recipients: 3,
    sent_count: 1,
    delivered_count: 0,
    read_count: 0,
    failed_count: 0,
    skipped_count: 0,
    assigned_account_ids: [1],
    rate_limit_min_delay_secs: 8,
    rate_limit_max_delay_secs: 20,
    burst_limit_per_batch: 25,
    burst_pause_secs: 60,
    quiet_hours_enabled: false,
    quiet_hours_start: "22:00",
    quiet_hours_end: "07:30",
    timezone: "Asia/Ho_Chi_Minh",
    last_heartbeat_at: null,
    started_at: null,
    completed_at: null,
    created_at: "2026-11-23T02:00:00Z",
    updated_at: "2026-11-23T02:00:00Z",
  };
  await Host.start(dir);
  const app = await openWhatsApp({
    modules: {
      "rustsale/native": {
        "Campaigns.whatsappCampaigns": () => [running],
        "Campaigns.pauseWhatsappCampaign": (id: number) => { paused.push(id); return null; },
      },
    },
  });
  await app.waitUntil(() => has(app, `WA L3 running ${stamp}`));
  await app.press("id:wa-pause-77");
  await app.waitUntil(() => paused.includes(77));
});

test("the recipient ledger pages past the first fifty", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const id = await createCampaign(`WA L3 page ${stamp}`);
  const rows = [];
  for (let index = 0; index < 60; index += 1) {
    rows.push({
      lead_id: null,
      recipient_phone_raw: `+8490${String(1000000 + index)}`,
      template: "Chào",
      lead_vars: {},
    });
  }
  await Campaigns.populateWhatsappCampaignRecipients(id, rows, "Chào");

  const app = await openWhatsApp();
  await app.type("id:wa-search", `WA L3 page ${stamp}`);
  await app.waitUntil(() => has(app, `WA L3 page ${stamp}`));
  await app.press(`id:wa-detail-${id}`);
  await app.waitUntil(() => has(app, "50 / 60 người nhận"));
  await app.press("id:wa-recipients-more");
  await app.waitUntil(() => has(app, "60 / 60 người nhận"));
});
