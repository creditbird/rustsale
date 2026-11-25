// `nsh test` runs this in a headless runner linked with the crate: the module is the real one, on a
// throwaway data folder. Ids and the SMTP port are checked after serde, before the core stores a
// row or opens a socket.
import { expect, test } from "nsh/test";
import {
  Campaigns,
  ConversationActions,
  Deals,
  Host,
  Leads,
  LeadsMore,
  Templates
} from "rustsale/native";

type Row = Record<string, any>;
const rows = (value: unknown) => value as Row[];

const dir = `/tmp/rustsale-v3-nsh-test-serde-bodies-${Date.now()}`;

async function expectError(
  promise: Promise<unknown>,
  code: string,
  message?: string
): Promise<void> {
  try {
    await promise;
  } catch (error) {
    const err = error as { code?: unknown; message?: unknown };
    expect(err.code).toBe(code);
    if (message !== undefined) expect(err.message).toBe(message);
    return;
  }
  throw new Error(`expected a rejection with code ${code}`);
}

async function smtpCount(): Promise<number> {
  return rows(await Templates.smtpAccounts()).length;
}

async function dealCount(): Promise<number> {
  return rows(await Deals.list()).length;
}

async function campaignCount(): Promise<number> {
  return rows(await Campaigns.unifiedCampaigns()).length;
}

test("int5 item1: saveSmtpServer refuses a port outside 1..65535 and stores nothing", async () => {
  await Host.start(dir);
  const before = await smtpCount();
  for (const port of [70000, -1, 0]) {
    await expectError(
      Templates.saveSmtpServer({
        id: null,
        name: `QA int5 port ${port}`,
        host: "127.0.0.1",
        port,
        from_email: "int5@example.test"
      }),
      "invalid_request",
      "port is out of range"
    );
  }
  expect(await smtpCount()).toBe(before);
});

test("int5 item2: saveSmtpServer refuses a negative id instead of inserting a row", async () => {
  await Host.start(dir);
  const before = await smtpCount();
  await expectError(
    Templates.saveSmtpServer({
      id: -1,
      name: "QA int5 negative id",
      host: "127.0.0.1",
      port: 2525,
      from_email: "int5@example.test"
    }),
    "invalid_request",
    "id is out of range"
  );
  expect(await smtpCount()).toBe(before);
});

test("int5 item3: Deals.create refuses a negative leadId and stores nothing", async () => {
  await Host.start(dir);
  const before = await dealCount();
  await expectError(
    Deals.create({
      leadId: -1,
      title: "QA int5 negative lead",
      value: 1000,
      stage: "New"
    }),
    "invalid_request",
    "id is out of range"
  );
  expect(await dealCount()).toBe(before);
});

test("int5 item4: lead bodies refuse negative ids and leave the lead as it was", async () => {
  await Host.start(dir);
  const leadId = Number(await Leads.add({ name: "QA int5 lead keeper", status: "New" }));

  await expectError(
    LeadsMore.update({
      id: -1,
      name: "QA int5 overwritten",
      company: "",
      status: "Lost",
      value: 0,
      owner: "",
      email: "",
      phone: "",
      about: null,
      address: null,
      currency: "VND"
    }),
    "invalid_request",
    "id is out of range"
  );
  await expectError(
    ConversationActions.linkIdentity({ identityId: -1, leadId }),
    "invalid_request",
    "id is out of range"
  );
  await expectError(
    ConversationActions.linkIdentity({ identityId: 1, leadId: -1 }),
    "invalid_request",
    "id is out of range"
  );

  const lead = (await Leads.details(leadId)) as { id: number; name: string; status: string };
  expect(lead.id).toBe(leadId);
  expect(lead.name).toBe("QA int5 lead keeper");
  expect(lead.status).toBe("New");
});

test("int5 item5: testSmtpConnection refuses port 0 before opening a socket", async () => {
  await Host.start(dir);
  await expectError(
    Templates.testSmtpConnection("127.0.0.1", 0),
    "invalid_request",
    "port is out of range"
  );
});

test("int5 breadth: other form bodies refuse negative ids before the core sees them", async () => {
  await Host.start(dir);
  const campaignsBefore = await campaignCount();

  await expectError(
    Campaigns.createWhatsappCampaign({
      name: "QA int5 negative template",
      template_id: -1,
      assigned_account_ids: []
    }),
    "invalid_request",
    "id is out of range"
  );
  await expectError(
    Campaigns.createWhatsappCampaign({
      name: "QA int5 negative account",
      template_id: null,
      assigned_account_ids: [-1]
    }),
    "invalid_request",
    "id is out of range"
  );
  await expectError(
    Campaigns.saveUnifiedCampaign(
      {
        id: -1,
        name: "QA int5 negative campaign",
        description: null,
        channel_type: "zalo",
        status: "DRAFT",
        audience_filter_json: null,
        schedule_config_json: null,
        created_at: "",
        updated_at: ""
      },
      []
    ),
    "invalid_request",
    "id is out of range"
  );
  expect(await campaignCount()).toBe(campaignsBefore);

  await expectError(
    Templates.updateTemplate({
      id: -1,
      folder_id: null,
      name: "QA int5 template",
      channel: "email",
      stage: null,
      subject: null,
      content: "body",
      media_url: null,
      ai_prompt: null,
      tags_json: null
    }),
    "invalid_request",
    "id is out of range"
  );
  await expectError(
    Templates.createTemplate({
      folder_id: -1,
      name: "QA int5 template",
      channel: "email",
      stage: null,
      subject: null,
      content: "body",
      media_url: null,
      ai_prompt: null,
      tags_json: null
    }),
    "invalid_request",
    "id is out of range"
  );
  await expectError(Leads.page({ categoryId: -1 }, null, 10), "invalid_request", "id is out of range");

  await expectError(
    ConversationActions.pinMessage({ conversationId: -1, messageId: 1 }),
    "invalid_request",
    "id is out of range"
  );
  await expectError(
    ConversationActions.pinMessage({ conversationId: 1, messageId: -1 }),
    "invalid_request",
    "id is out of range"
  );
  await expectError(
    ConversationActions.sendMessage({ conversationId: 1, channelAccountId: -1, text: "qa" }),
    "invalid_request",
    "id is out of range"
  );
  await expectError(
    ConversationActions.deleteConversation({ leadId: -1 }),
    "invalid_request",
    "id is out of range"
  );
});

test("int5 valid: saveSmtpServer still stores a valid account", async () => {
  await Host.start(dir);
  const before = await smtpCount();
  const id = (await Templates.saveSmtpServer({
    id: null,
    name: "QA int5 valid port",
    host: "127.0.0.1",
    port: 2525,
    from_email: "int5@example.test"
  })) as number;
  expect(id).toBeGreaterThan(0);
  expect(await smtpCount()).toBe(before + 1);
  await Templates.deleteSmtpServer(id);
  expect(await smtpCount()).toBe(before);
});
