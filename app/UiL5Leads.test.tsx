// Headless: the Leads screen's category filter and its edit form, against the real native module.
// Each case makes its own lead and category, so the shared demo folder does not affect the answers.
import { expect, open, test } from "nsh/test";
import { eventually } from "./testing";
import { Categories, Host, Leads, LeadsMore } from "rustsale/native";

const dir = `/tmp/rustsale-v3-ui-l5a-leads-${Date.now()}`;

interface LeadRow {
  id: number;
  name: string;
  value: number | null;
  status: string;
}

async function openLeads() {
  await Host.start(dir);
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-leads");
  await app.waitFor("id:lead-list");
  return app;
}
const has = (app: Awaited<ReturnType<typeof openLeads>>, s: string) => app.texts().some((t) => t.includes(s));

async function openLead(id: number, name: string) {
  const app = await openLeads();
  await app.type("id:lead-search", name);
  await app.waitFor(`id:lead-row-${id}`);
  await app.press(`id:lead-row-${id}`);
  await app.waitFor("id:lead-edit");
  return app;
}

test("the category filter narrows the list in the core", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const leadId = (await Leads.add({ name: `Lead lọc ${stamp}`, status: "New" })) as number;
  const categoryId = (await Categories.create(`Nhóm lọc ${stamp}`)) as number;
  await Categories.addLeads([leadId], categoryId);
  const app = await openLeads();
  await app.waitFor(`id:lead-cat-${categoryId}`);
  await app.press(`id:lead-cat-${categoryId}`);
  await app.waitUntil(() => has(app, "1 / 1 khách"));
  expect(app.shows(`id:lead-row-${leadId}`)).toBe(true);
  await app.press("id:lead-cat-all");
  const total = ((await Leads.page({ sort: "name" }, null, 50)) as { total: number }).total;
  await app.waitUntil(() => has(app, `50 / ${total} khách`) || has(app, `${total} / ${total} khách`));
});

test("a lead is edited in the detail panel and saved through the core", async () => {
  const stamp = Date.now();
  const original = `Lead sửa ${stamp}`;
  await Host.start(dir);
  const id = (await Leads.add({ name: original, value: 12345, status: "New" })) as number;
  const app = await openLead(id, original);
  await app.press("id:lead-edit");
  await app.waitFor("id:lead-edit-name");
  await app.type("id:lead-edit-name", " đã sửa");
  await app.press("id:lead-edit-save");
  await eventually(async () => ((await Leads.details(id)) as LeadRow).name === `${original} đã sửa`);
  await app.waitUntil(() => has(app, `${original} đã sửa`));
});

test("a bad value is refused before the core is asked", async () => {
  const stamp = Date.now();
  const original = `Lead giá ${stamp}`;
  await Host.start(dir);
  const id = (await Leads.add({ name: original, value: 12345, status: "New" })) as number;
  const app = await openLead(id, original);
  await app.press("id:lead-edit");
  await app.waitFor("id:lead-edit-value");
  await app.clear("id:lead-edit-value");
  await app.type("id:lead-edit-value", "abc");
  await app.press("id:lead-edit-save");
  await app.waitFor("id:lead-edit-error");
  expect(has(app, "Giá trị phải là số không âm.")).toBe(true);
  expect(((await Leads.details(id)) as LeadRow).value).toBe(12345);
});

test("editing a lead deleted elsewhere refreshes the list", async () => {
  const stamp = Date.now();
  const original = `Lead mất ${stamp}`;
  await Host.start(dir);
  const id = (await Leads.add({ name: original, status: "New" })) as number;
  const app = await openLead(id, original);
  await app.press("id:lead-edit");
  await app.waitFor("id:lead-edit-name");
  await LeadsMore.delete(id);
  await app.press("id:lead-edit-save");
  await app.waitFor("id:lead-detail", { state: "gone" });
  await app.waitFor(`id:lead-row-${id}`, { state: "gone" });
});
