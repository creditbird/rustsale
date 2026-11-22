// `nsh test` runs this in a headless runner linked with the crate: the module is the real one, on a
// throwaway data folder with the debug seed (3 synthetic leads, 2 deals). No real account or data.
import { expect, open, test } from "nsh/test";
import { Conversations, GoogleMaps, Host, Leads, Settings } from "rustsale/native";

// The module's lists and records are JSON (`unknown` in the declarations): read them as these.
type Page = { items: Record<string, any>[]; total: number; nextCursor: string | null };
type Row = Record<string, any>;
const page = (value: unknown) => value as Page;
const row = (value: unknown) => value as Row;

const dir = `/tmp/rustsale-v3-nsh-test-${Date.now()}`;

test("the core starts on a v3 folder and refuses the live one", async () => {
  await expect(Host.start("/home/x/.local/share/com.rustsale.crm")).rejects.toThrow();
  const info = await Host.start(dir);
  expect(info.dataDir).toBe(dir);
  expect(info.localApiPort).toBe(8797);
  const again = await Host.start(dir);
  expect(again.dataDir).toBe(dir);
});

test("leads page by cursor and filter", async () => {
  await Host.start(dir);
  const first = page(await Leads.page({ sort: "name" }, null, 2));
  expect(first.total).toBe(3);
  expect(first.items.length).toBe(2);
  expect(first.nextCursor).not.toBeNull();
  const second = page(await Leads.page({ sort: "name" }, first.nextCursor, 2));
  expect(second.items.length).toBe(1);
  expect(second.nextCursor).toBeNull();
  const found = page(await Leads.page({ search: "cloudsync" }, null, 50));
  expect(found.total).toBe(1);
});

test("a bad cursor rejects with a code", async () => {
  await Host.start(dir);
  await expect(Leads.page({ sort: "name" }, "not-a-cursor", 2)).rejects.toThrow();
});

test("leads add, status and details", async () => {
  await Host.start(dir);
  const id = (await Leads.add({ name: "Synthetic Co", status: "New" })) as number;
  await Leads.updateStatus(id, "Qualified");
  const lead = row(await Leads.details(id));
  expect(lead.name).toBe("Synthetic Co");
  expect(lead.status).toBe("Qualified");
  expect(lead.currency).toBe("VND");
  const activities = page(await Leads.activitiesPage(null, null, 10));
  expect(activities.total).toBeGreaterThan(0);
  const categories = await Leads.categories();
  expect(Array.isArray(categories)).toBe(true);
});

test("conversations inbox is empty on a fresh folder", async () => {
  await Host.start(dir);
  const inbox = page(await Conversations.inboxPage(null, null, 50));
  expect(inbox.total).toBe(0);
  const status = await Conversations.runtimeStatus();
  expect(status).toBeDefined();
});

test("google maps jobs list is empty and paging a missing job rejects", async () => {
  await Host.start(dir);
  expect(await GoogleMaps.listJobs()).toEqual([]);
  await expect(GoogleMaps.getJob("missing")).rejects.toThrow();
});

test("settings round trip", async () => {
  await Host.start(dir);
  const currency = row(await Settings.currency());
  expect(currency.base_currency).toBe("VND");
  const notifications = row(await Settings.notifications());
  await Settings.updateNotifications({ ...notifications, mute_groups: true });
  expect(row(await Settings.notifications()).mute_groups).toBe(true);
  const tray = row(await Settings.tray());
  expect(typeof tray.close_to_tray).toBe("boolean");
});

test("the entry screen shows the leads", async () => {
  const app = await open("./App.tsx");
  await app.waitFor("Sarah Harrington");
  expect(app.texts()).toContain("Jordan Smith");
});
