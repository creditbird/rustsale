// `nsh test` runs this in a headless runner linked with the crate: the module is the real one, on a
// throwaway data folder. Every native failure crossing into TSX must carry a stable `code`.
import { expect, test } from "nsh/test";
import {
  Ai,
  Campaigns,
  Categories,
  ConversationActions,
  ConversationNetwork,
  Conversations,
  Deals,
  Desktop,
  Facebook,
  GoogleMapsMore,
  Host,
  ImportLeads,
  Leads,
  LeadsMore,
  Templates,
  Zalo
} from "rustsale/native";

const dir = `/tmp/rustsale-v3-nsh-test-errcode-${Date.now()}`;

async function expectError(promise: Promise<unknown>, code: string): Promise<void> {
  try {
    await promise;
  } catch (error) {
    const err = error as { code?: unknown; message?: unknown };
    expect(err.code).toBe(code);
    expect(String(err.message).startsWith(`${code}: `)).toBe(false);
    expect(String(err.message).length).toBeGreaterThan(0);
    return;
  }
  throw new Error(`expected a rejection with code ${code}`);
}

test("native errors reach TSX with a code and a clean message", async () => {
  await Host.start(dir);
  await expectError(Ai.leadEvidenceContext(4_294_967_297), "invalid_request");
  await expectError(Zalo.verifySession("qa-errcode-missing"), "not_found");
  await expectError(Templates.testSmtpConnection("127.0.0.1", 1), "internal");
  await expectError(Templates.deleteSmtpServer(4_294_967_297), "invalid_request");
  await expectError(Templates.deleteEmailTemplate(4_294_967_297), "invalid_request");
  await expectError(Campaigns.whatsappCampaign(4_294_967_297), "not_found");
  await expectError(Facebook.syncConversations("qa-errcode-missing-page"), "not_found");
  await expectError(
    ConversationNetwork.startDirect({
      channelAccountId: 424242,
      phoneOrExternalId: "0900000000",
      displayName: null,
      initialMessage: "qa errcode"
    }),
    "not_found"
  );
  await expectError(ImportLeads.inspectFile("/tmp/qa-errcode-missing.csv", null), "not_found");
});

test("native ids must be whole and non-negative before any row is touched", async () => {
  await Host.start(dir);
  const campaignId = Number(
    await Campaigns.saveUnifiedCampaign(
      {
        id: null,
        name: "QA int4",
        description: null,
        channel_type: "zalo",
        status: "DRAFT",
        audience_filter_json: null,
        schedule_config_json: null,
        created_at: "",
        updated_at: ""
      },
      []
    )
  );
  expect(campaignId).toBeGreaterThan(0);

  for (const bad of [1.5, -1, Number.NaN, 2 ** 53 + 2]) {
    await expectError(Campaigns.deleteUnifiedCampaign(bad), "invalid_request");
  }
  const campaigns = (await Campaigns.unifiedCampaigns()) as Array<{ id?: number | null }>;
  expect(campaigns.some((row) => row.id === campaignId)).toBe(true);

  await expectError(Ai.reviewProposal(1.5, "APPROVED", "qa"), "invalid_request");
  await expectError(ConversationNetwork.groupMembers(1.5), "invalid_request");
  await expectError(GoogleMapsMore.resolveBounds(-1), "invalid_request");
  await expectError(Facebook.publishPostNow(1.5), "invalid_request");
  await expectError(
    ConversationActions.contactTimeline(1.5, ["zalo"], null, 0),
    "invalid_request"
  );
  await expectError(ConversationActions.pinnedMessages(Number.NaN), "invalid_request");
  await expectError(Conversations.timeline(1.5, null, 0), "invalid_request");
  await expectError(Desktop.setActiveConversationFocus(1.5, true, 0), "invalid_request");
});

test("raw id parameters reject fractions and keep the row", async () => {
  await Host.start(dir);
  const leadId = Number(await Leads.add({ name: "QA int4 keeper", status: "New" }));

  for (const bad of [4.5, -1, Number.NaN, 2 ** 53 + 2]) {
    await expectError(Leads.details(bad), "invalid_request");
  }
  await expectError(Leads.updateStatus(4.5, "Lost"), "invalid_request");
  await expectError(Leads.byCategoryPage(1.5, null, null, 10), "invalid_request");
  await expectError(Leads.activitiesPage(1.5, null, 10), "invalid_request");
  await expectError(LeadsMore.delete(4.5), "invalid_request");
  await expectError(LeadsMore.outreachPage({ channel: "email" }, 1.5, 10), "invalid_request");
  await expectError(LeadsMore.outreachIds({ channel: "email" }, [1.5]), "invalid_request");
  await expectError(Categories.delete(1.5), "invalid_request");
  await expectError(Deals.delete(1.5), "invalid_request");
  await expectError(Templates.updateEmailTemplate(4.5, "QA", "s", "c"), "invalid_request");
  await expectError(Templates.template(1.5), "invalid_request");
  await expectError(Templates.deleteTemplate(-1), "invalid_request");
  await expectError(Templates.deleteFolder(4.5), "invalid_request");
  await expectError(Templates.incrementUsage(4.5), "invalid_request");
  await expectError(Templates.testSmtpConnection("127.0.0.1", 1.5), "invalid_request");

  const lead = (await Leads.details(leadId)) as { id: number; status: string };
  expect(lead.id).toBe(leadId);
  expect(lead.status).toBe("New");
});
