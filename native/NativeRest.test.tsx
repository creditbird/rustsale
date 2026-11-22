// `nsh test` runs this in a headless runner linked with the crate: the module is the real one, on a
// throwaway data folder with the debug seed (3 synthetic leads, 2 deals). No real account or data.
import { expect, test } from "nsh/test";
import { Campaigns, Host, LeadsMore, Zalo } from "rustsale/native";

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
