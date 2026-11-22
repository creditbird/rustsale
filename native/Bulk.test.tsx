// `nsh test` runs this in a headless runner linked with the crate: the module is the real one, on a
// throwaway data folder with the debug seed (3 synthetic leads, 2 deals). No real account or data.
import { expect, test } from "nsh/test";
import {
  Categories,
  ConversationActions,
  Deals,
  Host,
  Leads,
  LeadsMore,
  Templates
} from "rustsale/native";

// The module's lists and records are JSON (`unknown` in the declarations): read them as these.
type Row = Record<string, any>;
const row = (value: unknown) => value as Row;
const rows = (value: unknown) => value as Row[];

const dir = `/tmp/rustsale-v3-nsh-test-bulk-${Date.now()}`;

test("deals: list the seed, create, move stage, delete", async () => {
  await Host.start(dir);
  const leads = rows(await LeadsMore.list());
  expect(leads.length).toBeGreaterThanOrEqual(3);
  const leadId = leads[0].id as number;

  const seeded = rows(await Deals.list());
  expect(seeded.length).toBeGreaterThanOrEqual(2);

  const deal = row(
    await Deals.create({
      leadId,
      title: "Bulk deal",
      value: 1250,
      stage: "Proposal",
      currency: "VND"
    })
  );
  expect(deal.title).toBe("Bulk deal");
  expect(deal.stage).toBe("Proposal");
  expect(deal.currency).toBe("VND");

  await Deals.updateStage(deal.id, "Won");
  const moved = rows(await Deals.list()).find((item) => item.id === deal.id);
  expect(moved?.stage).toBe("Won");

  await Deals.delete(deal.id);
  expect(rows(await Deals.list()).some((item) => item.id === deal.id)).toBe(false);

  await expect(
    Deals.create({ leadId, title: "Bad deal", value: 1, stage: "New", currency: "EUR" })
  ).rejects.toThrow();
});

test("categories: create, assign, read back, remove, delete", async () => {
  await Host.start(dir);
  const leadId = rows(await LeadsMore.list())[0].id as number;

  const categoryId = (await Categories.create("Bulk category")) as number;
  await Categories.addLeads([leadId], categoryId);

  const forLead = rows(await Categories.forLead(leadId));
  expect(forLead.some((item) => item.id === categoryId)).toBe(true);
  const members = rows(await Categories.leads(categoryId));
  expect(members.some((item) => item.id === leadId)).toBe(true);

  await Categories.removeLeads([leadId], categoryId);
  expect(rows(await Categories.leads(categoryId)).length).toBe(0);

  await Categories.delete(categoryId);
  expect(rows(await Categories.list()).some((item) => item.id === categoryId)).toBe(false);

  await expect(Categories.create("   ")).rejects.toThrow();
});

test("leads more: outreach paging, ids, activities and edits", async () => {
  await Host.start(dir);

  const page = row(await LeadsMore.outreachPage({ channel: "email" }, null, 2));
  expect(page.total).toBeGreaterThanOrEqual(3);
  expect(page.items.length).toBe(2);
  expect(page.next_cursor).not.toBeNull();

  const ids = (await LeadsMore.outreachIds({ channel: "email" }, [])) as number[];
  expect(ids.length).toBeGreaterThanOrEqual(3);
  const leadId = ids[0];

  await LeadsMore.addActivity({
    leadId,
    activityType: "note",
    content: "Bulk activity",
    performedBy: "Bulk test"
  });
  const activities = rows(await LeadsMore.activities(leadId));
  expect(activities.some((item) => item.content === "Bulk activity")).toBe(true);
  expect(rows(await LeadsMore.recentActivities(5)).length).toBeGreaterThan(0);

  const tempId = (await Leads.add({ name: "Bulk temp" })) as number;
  await LeadsMore.update({
    id: tempId,
    name: "Bulk temp 2",
    company: "Bulk Co",
    status: "New",
    value: 0,
    owner: "Tester",
    email: "",
    phone: "",
    about: null,
    address: null,
    currency: "VND"
  });
  expect(row(await Leads.details(tempId)).name).toBe("Bulk temp 2");

  await expect(
    LeadsMore.update({ id: tempId, name: "Bad currency", currency: "EUR" })
  ).rejects.toThrow();

  await LeadsMore.delete(tempId);
  await expect(Leads.details(tempId)).rejects.toThrow();
});

test("templates: email CRUD, template centre and SMTP accounts", async () => {
  await Host.start(dir);

  expect(Array.isArray(await Templates.emailTemplates())).toBe(true);
  const emailId = (await Templates.createEmailTemplate("Bulk email", "Hi", "Body")) as number;
  await Templates.updateEmailTemplate(emailId, "Bulk email 2", "Hi 2", "Body 2");
  expect(
    rows(await Templates.emailTemplates()).some((item) => item.id === emailId)
  ).toBe(true);
  await Templates.deleteEmailTemplate(emailId);
  expect(
    rows(await Templates.emailTemplates()).some((item) => item.id === emailId)
  ).toBe(false);

  const folderId = (await Templates.createFolder("Bulk folder", "#112233")) as number;
  const templateId = (await Templates.createTemplate({
    folder_id: folderId,
    name: "Bulk template",
    channel: "zalo",
    stage: null,
    subject: null,
    content: "Hello {{name}}",
    media_url: null,
    ai_prompt: null,
    tags_json: null
  })) as number;
  const template = row(await Templates.template(templateId));
  expect(template.name).toBe("Bulk template");
  expect(
    rows(await Templates.list({ channel: "zalo" })).some((item) => item.id === templateId)
  ).toBe(true);
  await Templates.incrementUsage(templateId);
  expect(row(await Templates.template(templateId)).usage_count).toBe(1);
  expect(await Templates.template(999999)).toBeNull();

  await Templates.deleteTemplate(templateId);
  await Templates.deleteFolder(folderId);

  expect(Array.isArray(await Templates.smtpAccounts())).toBe(true);
  const smtpId = (await Templates.saveSmtpServer({
    name: "Bulk SMTP",
    host: "127.0.0.1",
    port: 2525,
    username: "bulk",
    password: "",
    from_email: "bulk@example.test",
    from_name: "Bulk",
    security: "none"
  })) as number;
  expect(
    rows(await Templates.smtpAccounts()).some((item) => item.id === smtpId)
  ).toBe(true);
  await Templates.deleteSmtpServer(smtpId);

  await expect(Templates.createTemplate(null)).rejects.toThrow();
});

test("conversation actions: empty timelines, quick templates, pins and errors", async () => {
  await Host.start(dir);
  const leadId = rows(await LeadsMore.list())[0].id as number;

  const timeline = row(await ConversationActions.contactTimeline(leadId, [], null, 50));
  expect(Array.isArray(timeline.items)).toBe(true);
  expect(timeline.items.length).toBe(0);

  const identity = row(await ConversationActions.identityTimeline(999999, null, 50));
  expect(Array.isArray(identity.items)).toBe(true);

  const water = row(await ConversationActions.highWaterMark());
  expect(typeof water.currentSequence).toBe("number");
  expect(Array.isArray(await ConversationActions.resumeDeltas(0, 10))).toBe(true);
  expect(rows(await ConversationActions.pinnedMessages(999999))).toEqual([]);
  expect(rows(await ConversationActions.mentionCandidates({ conversationId: 999999 }))).toEqual(
    []
  );

  const quick = row(
    await ConversationActions.createQuickTemplate({
      shortcut: "/bulk",
      title: "Bulk quick",
      bodyText: "Hi there"
    })
  );
  expect(quick.shortcut).toBe("bulk");
  const updated = row(
    await ConversationActions.updateQuickTemplate({
      id: quick.id,
      shortcut: "/bulk2",
      title: "Bulk quick 2",
      bodyText: "Hi again",
      isActive: true
    })
  );
  expect(updated.shortcut).toBe("bulk2");
  await ConversationActions.markQuickTemplateUsed({ id: quick.id });
  expect(
    rows(await ConversationActions.quickTemplates(null)).some((item) => item.id === quick.id)
  ).toBe(true);
  await ConversationActions.deleteQuickTemplate({ id: quick.id });
  expect(
    rows(await ConversationActions.quickTemplates(null)).some((item) => item.id === quick.id)
  ).toBe(false);

  await expect(ConversationActions.deleteConversation({})).rejects.toThrow();
  await expect(ConversationActions.forwardOperation("bulk-missing")).rejects.toThrow();
});
