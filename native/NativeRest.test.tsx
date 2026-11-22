// `nsh test` runs this in a headless runner linked with the crate: the module is the real one, on a
// throwaway data folder with the debug seed (3 synthetic leads, 2 deals). No real account or data.
import { writeFile } from "node:fs/promises";
import { expect, test } from "nsh/test";
import {
  Ai,
  Campaigns,
  Desktop,
  Facebook,
  Host,
  ImportLeads,
  LeadsMore,
  Util,
  Zalo
} from "rustsale/native";

type Row = Record<string, any>;
const row = (value: unknown) => value as Row;
const rows = (value: unknown) => value as Row[];

const dir = `/tmp/rustsale-v3-nsh-test-rest-${Date.now()}`;

test("campaigns: outreach templates and the unified campaign lifecycle", async () => {
  await Host.start(dir);

  const templateId = (await Campaigns.createOutreachTemplate(
    "Bulk outreach",
    "Chao {name}",
    null
  )) as number;
  let templates = rows(await Campaigns.outreachTemplates());
  expect(templates.some((item) => item.id === templateId)).toBe(true);

  await Campaigns.updateOutreachTemplate(
    templateId,
    "Bulk outreach 2",
    "Chao ban",
    "https://example.invalid/a.png"
  );
  templates = rows(await Campaigns.outreachTemplates());
  const updated = templates.find((item) => item.id === templateId);
  expect(updated?.name).toBe("Bulk outreach 2");
  expect(updated?.image_url).toBe("https://example.invalid/a.png");

  await Campaigns.deleteOutreachTemplate(templateId);
  templates = rows(await Campaigns.outreachTemplates());
  expect(templates.some((item) => item.id === templateId)).toBe(false);

  const campaignId = (await Campaigns.saveUnifiedCampaign(
    {
      id: null,
      name: "Bulk unified",
      description: null,
      channel_type: "zalo",
      status: "DRAFT",
      audience_filter_json: null,
      schedule_config_json: null,
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z"
    },
    [
      {
        id: null,
        campaign_id: 0,
        step_order: 1,
        channel: "zalo",
        delay_minutes: 0,
        condition_type: "ALWAYS",
        template_content: "Chao {name}",
        ai_prompt_override: null,
        created_at: "2026-01-01T00:00:00Z",
        template_id: null
      }
    ]
  )) as number;
  expect(campaignId).toBeGreaterThan(0);

  let campaigns = rows(await Campaigns.unifiedCampaigns());
  expect(campaigns.some((item) => item.id === campaignId)).toBe(true);

  await Campaigns.startUnifiedCampaign(campaignId);
  campaigns = rows(await Campaigns.unifiedCampaigns());
  expect(campaigns.find((item) => item.id === campaignId)?.status).toBe("ACTIVE");

  await Campaigns.pauseUnifiedCampaign(campaignId);
  campaigns = rows(await Campaigns.unifiedCampaigns());
  expect(campaigns.find((item) => item.id === campaignId)?.status).toBe("PAUSED");

  const leadId = rows(await LeadsMore.list())[0].id as number;
  const enrolled = (await Campaigns.enrollLeadsIntoCampaign(campaignId, [
    leadId
  ])) as number;
  expect(enrolled).toBeGreaterThanOrEqual(1);

  const analytics = row(await Campaigns.unifiedCampaignAnalytics(campaignId));
  expect(analytics.campaign_id).toBe(campaignId);
  expect(analytics.total_enrolled).toBeGreaterThanOrEqual(1);

  await Campaigns.deleteUnifiedCampaign(campaignId);
  campaigns = rows(await Campaigns.unifiedCampaigns());
  expect(campaigns.some((item) => item.id === campaignId)).toBe(false);
});

test("campaigns: WhatsApp campaign create, populate and cancel", async () => {
  await Host.start(dir);

  await expect(
    Campaigns.createWhatsappCampaign({
      name: "   ",
      template_id: null,
      assigned_account_ids: [1],
      rate_limit_min_delay_secs: null,
      rate_limit_max_delay_secs: null,
      burst_limit_per_batch: null,
      burst_pause_secs: null,
      quiet_hours_enabled: null,
      quiet_hours_start: null,
      quiet_hours_end: null,
      timezone: null
    })
  ).rejects.toThrow();

  const campaignId = (await Campaigns.createWhatsappCampaign({
    name: "Bulk WhatsApp",
    template_id: null,
    assigned_account_ids: [1],
    rate_limit_min_delay_secs: null,
    rate_limit_max_delay_secs: null,
    burst_limit_per_batch: null,
    burst_pause_secs: null,
    quiet_hours_enabled: null,
    quiet_hours_start: null,
    quiet_hours_end: null,
    timezone: null
  })) as number;
  expect(campaignId).toBeGreaterThan(0);

  const inserted = (await Campaigns.populateWhatsappCampaignRecipients(
    campaignId,
    [
      {
        lead_id: null,
        recipient_phone_raw: "0901234567",
        template: "Chao ban",
        lead_vars: null
      }
    ],
    null
  )) as number;
  expect(inserted).toBe(1);

  const campaign = row(await Campaigns.whatsappCampaign(campaignId));
  expect(campaign.name).toBe("Bulk WhatsApp");
  expect(campaign.total_recipients).toBe(1);
  expect(rows(await Campaigns.whatsappCampaigns()).some((item) => item.id === campaignId)).toBe(
    true
  );

  const recipients = rows(
    await Campaigns.whatsappCampaignRecipients(campaignId, 10, 0)
  );
  expect(recipients.length).toBe(1);
  expect(recipients[0].recipient_phone_raw).toBe("0901234567");

  await Campaigns.pauseWhatsappCampaign(campaignId);
  expect(row(await Campaigns.whatsappCampaign(campaignId)).status).toBe("paused");
  await Campaigns.resumeWhatsappCampaign(campaignId);
  expect(row(await Campaigns.whatsappCampaign(campaignId)).status).toBe("queued");
  await Campaigns.cancelWhatsappCampaign(campaignId);
  expect(row(await Campaigns.whatsappCampaign(campaignId)).status).toBe("cancelled");
});

test("zalo: account list is empty and a missing session rejects", async () => {
  await Host.start(dir);
  expect(await Zalo.accounts()).toEqual([]);
  await expect(Zalo.verifySession("missing-imei")).rejects.toThrow();
});

test("facebook: empty lists, rejected tokens and missing page errors", async () => {
  await Host.start(dir);
  expect(await Facebook.accounts()).toEqual([]);
  expect(await Facebook.scheduledPosts(null)).toEqual([]);
  expect(await Facebook.commentAutomations(null)).toEqual([]);
  expect(await Facebook.commentLogs(null, 10)).toEqual([]);

  await expect(
    Facebook.saveAccount({ pageId: "  ", accessToken: "" })
  ).rejects.toThrow();
  await expect(Facebook.discoverPages("   ", null)).rejects.toThrow();
  await expect(Facebook.pageHealth("missing-page")).rejects.toThrow();
  await expect(Facebook.syncConversations("missing-page")).rejects.toThrow();
  expect(await Facebook.ingestWebhookEvents("missing-page", [])).toBe(0);
  await expect(
    Facebook.schedulePost({
      draft_group_id: null,
      channel_account_id: 1,
      page_id: "missing-page",
      post_type: "text",
      message: "Bulk post",
      link_url: null,
      media_paths: [],
      scheduled_publish_time: null
    })
  ).rejects.toThrow();
});

test("ai: proposals start empty and invalid reviews or a missing lead reject", async () => {
  await Host.start(dir);
  expect(await Ai.pendingProposals()).toEqual([]);
  await expect(Ai.reviewProposal(1, "MAYBE", "qa")).rejects.toThrow();
  await expect(Ai.leadEvidenceContext(999_999)).rejects.toThrow();
});

test("import: inspect a CSV and import its rows as leads", async () => {
  await Host.start(dir);

  await expect(ImportLeads.inspectFile("/tmp/does-not-exist.csv", null)).rejects.toThrow();

  const csvPath = `/tmp/rustsale-import-rest-${Date.now()}.csv`;
  await writeFile(csvPath, "name,phone,email\nBulk Imported,0909123456,bulk@example.invalid\n");

  const inspection = row(await ImportLeads.inspectFile(csvPath, null));
  expect(inspection.file_type).toBe("csv");
  expect(inspection.headers).toContain("name");

  const summary = row(
    await ImportLeads.execute({
      file_path: csvPath,
      sheet_name: null,
      has_header: true,
      header_row_index: 0,
      mapping: {
        name_col: 0,
        company_col: null,
        phone_col: 1,
        email_col: 2,
        status_col: null,
        value_col: null,
        address_col: null,
        about_col: null,
        owner_col: null
      },
      conflict_strategy: "skip",
      default_category_id: null,
      default_status: null,
      default_owner: null,
      default_currency: null,
      normalize_phone: true
    })
  );
  expect(summary.inserted + summary.updated).toBeGreaterThanOrEqual(1);
  expect(rows(await LeadsMore.list()).some((item) => item.name === "Bulk Imported")).toBe(true);
});

test("util: greeting and hardware id", async () => {
  expect(await Util.greet("Bulk")).toBe("Hello, Bulk! You've been greeted from Rust!");
  const hardware = row(await Util.hardwareId());
  expect(hardware.hardware_id.length).toBeGreaterThan(10);
  expect(typeof hardware.hostname).toBe("string");
});

test("desktop: focus sequence accepts newer updates and rejects older ones", async () => {
  await Host.start(dir);
  const first = row(await Desktop.setActiveConversationFocus(7, true, 10));
  expect(first.accepted).toBe(true);
  expect(first.backend_seq).toBe(10);
  const stale = row(await Desktop.setActiveConversationFocus(8, true, 9));
  expect(stale.accepted).toBe(false);
  expect(stale.backend_seq).toBe(10);
});
