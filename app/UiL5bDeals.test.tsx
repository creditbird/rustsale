// Headless: the lead detail panel shows the deals attached to the open lead, and only those.
import { expect, open, test } from "nsh/test";
import { Deals, Host, Leads } from "rustsale/native";

const dir = `/tmp/rustsale-v3-ui-l5b-deals-${Date.now()}`;

type App = Awaited<ReturnType<typeof open>>;
const has = (app: App, text: string) => app.texts().some((t) => t.includes(text));

async function openLead(app: App, id: number, name: string) {
  await app.type("id:lead-search", name);
  await app.waitFor(`id:lead-row-${id}`);
  await app.press(`id:lead-row-${id}`);
  await app.waitFor("id:lead-detail");
}

test("a deal of the open lead is listed with its stage and value", async () => {
  await Host.start(dir);
  const stamp = Date.now();
  const name = `Deal lead ${stamp}`;
  const leadId = (await Leads.add({ name, status: "Qualified" })) as number;
  const dealId = (await Deals.create({
    leadId,
    title: `Hợp đồng ${stamp}`,
    value: 15_000_000,
    stage: "Proposal",
  })) as { id: number };

  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-leads");
  await app.waitFor("id:lead-list");
  await openLead(app, leadId, name);

  await app.waitFor(`id:lead-deal-${dealId.id}`);
  expect(has(app, `Hợp đồng ${stamp}`)).toBe(true);
  expect(has(app, "Proposal")).toBe(true);
  expect(has(app, "15 tr")).toBe(true);
  expect(app.shows("id:lead-deals-empty")).toBe(false);
});

test("deals of another lead stay out of the panel", async () => {
  await Host.start(dir);
  const stamp = Date.now();
  const mine = `Deal mine ${stamp}`;
  const other = `Deal other ${stamp}`;
  const myId = (await Leads.add({ name: mine, status: "New" })) as number;
  const otherId = (await Leads.add({ name: other, status: "New" })) as number;
  await Deals.create({ leadId: myId, title: `Của tôi ${stamp}`, value: 1_000_000, stage: "New" });
  await Deals.create({ leadId: otherId, title: `Của người khác ${stamp}`, value: 2_000_000, stage: "New" });

  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-leads");
  await app.waitFor("id:lead-list");
  await openLead(app, myId, mine);

  expect(has(app, `Của tôi ${stamp}`)).toBe(true);
  expect(has(app, `Của người khác ${stamp}`)).toBe(false);
});

test("a lead without deals says so instead of an empty block", async () => {
  await Host.start(dir);
  const stamp = Date.now();
  const name = `Deal none ${stamp}`;
  const id = (await Leads.add({ name, status: "New" })) as number;

  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-leads");
  await app.waitFor("id:lead-list");
  await openLead(app, id, name);

  await app.waitFor("id:lead-deals-empty");
  expect(has(app, "Chưa có deal.")).toBe(true);
});
