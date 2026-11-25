// Headless: the Leads screen against the real native module on a throwaway folder.
import { expect, open, test } from "nsh/test";
import { eventually } from "./testing";
import { Host, Leads } from "rustsale/native";

const dir = `/tmp/rustsale-v3-ui2-leads-${Date.now()}`;

async function openLeads() {
  await Host.start(dir);
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-leads");
  await app.waitFor("id:lead-list");
  return app;
}
const has = (app: Awaited<ReturnType<typeof openLeads>>, s: string) => app.texts().some((t) => t.includes(s));

test("the list shows the seeded leads from the core", async () => {
  const app = await openLeads();
  await app.waitUntil(() => has(app, "3 / 3 khách"));
  expect(has(app, "Sarah Harrington")).toBe(true);
});

test("search narrows the list in the core", async () => {
  const app = await openLeads();
  await app.type("id:lead-search", "cloudsync");
  await app.waitUntil(() => has(app, "1 / 1 khách"));
  expect(has(app, "Jordan Smith")).toBe(true);
  expect(has(app, "Sarah Harrington")).toBe(false);
});

test("opening a lead and changing its status writes through the core", async () => {
  const app = await openLeads();
  await app.waitFor("id:lead-row-1");
  await app.press("id:lead-row-1");
  await app.waitFor("id:lead-detail");
  await app.press("id:set-status-Won");
  await eventually(async () => ((await Leads.details(1)) as { status: string }).status === "Won");
});

test("adding a lead needs a name, then appears in the list", async () => {
  const app = await openLeads();
  await app.press("id:lead-add");
  await app.waitFor("id:new-lead-save");
  await app.press("id:new-lead-save");
  await app.waitFor("id:lead-form-error");
  await app.type("id:new-lead-name", "Khách Mới Test");
  await app.press("id:new-lead-save");
  await app.waitUntil(() => has(app, "Khách Mới Test"));
});
