// Headless: the 5 lead filters the table sends to the core's paged query (status plus owner, value
// range, Zalo state and outreach state). The core's own filtering is pinned by its Rust tests.
import { expect, open, test } from "nsh/test";
import { Host, Leads } from "rustsale/native";

const dir = `/tmp/rustsale-v3-ui-l5b-filters-${Date.now()}`;

type Query = Record<string, unknown>;
type App = Awaited<ReturnType<typeof open>>;

async function openLeads() {
  await Host.start(dir);
  const calls: Query[] = [];
  const app = await open("./App.tsx", {
    modules: {
      "rustsale/native": {
        "Leads.owners": () => ["An", "Binh"],
        "Leads.page": (query: unknown) => {
          calls.push(query as Query);
          return { items: [], total: 0, nextCursor: null };
        },
      },
    },
  });
  await app.setViewport(1280, 800);
  await app.press("id:nav-leads");
  await app.waitFor("id:lead-list");
  await app.waitUntil(() => calls.length > 0);
  return { app, calls };
}

async function choose(app: App, filter: string, item: string) {
  await app.press(`id:lead-filter-${filter}-trigger`);
  await app.press(`id:lead-filter-${filter}-item-${item}`);
}

test("the status chips and the four advanced filters narrow the core query", async () => {
  const { app, calls } = await openLeads();

  await app.press("id:status-Won");
  await app.waitUntil(() => calls.some((q) => q.status === "Won"));

  await choose(app, "owner", "An");
  await app.waitUntil(() => calls.some((q) => q.owner === "An" && q.status === "Won"));

  await choose(app, "value", "10to50m");
  await app.waitUntil(() => calls.some(
    (q) => q.owner === "An" && q.minValue === 10_000_000 && q.maxValue === 50_000_000,
  ));

  await choose(app, "zalo", "verified");
  await app.waitUntil(() => calls.some(
    (q) => q.owner === "An" && q.zalo === "verified",
  ));

  await choose(app, "outreach", "notReached");
  await app.waitUntil(() => calls.some((q) => q.zaloReached === false && q.zalo === "verified"));
});

test("clearing the filters drops every one of them from the query", async () => {
  const { app, calls } = await openLeads();
  await choose(app, "owner", "Binh");
  await app.waitUntil(() => calls.some((q) => q.owner === "Binh"));
  await app.press("id:lead-filter-clear");
  await app.waitUntil(() => {
    const last = calls[calls.length - 1];
    return last.owner === undefined
      && last.minValue === undefined
      && last.zalo === undefined
      && last.zaloReached === undefined
      && last.status === undefined;
  });
});

test("the core itself applies the owner, value and Zalo filters", async () => {
  await Host.start(dir);
  const stamp = Date.now();
  const cheap = await Leads.add({ name: `Core re ${stamp}`, owner: `Chi ${stamp}`, value: 5_000_000 });
  const rich = await Leads.add({ name: `Core dat ${stamp}`, owner: `Chi ${stamp}`, value: 80_000_000 });
  expect(await Leads.owners()).toContain(`Chi ${stamp}`);

  const byOwner = (await Leads.page({ owner: `Chi ${stamp}` }, null, 50)) as { total: number };
  expect(byOwner.total).toBe(2);

  const accentedOwner = `Đặng Văn ${stamp}`;
  const accented = await Leads.add({ name: `Core viet ${stamp}`, owner: accentedOwner, value: 7_000_000 });
  expect(await Leads.owners()).toContain(accentedOwner);
  const byAccented = (await Leads.page({ owner: accentedOwner }, null, 50)) as {
    items: { id: number }[];
    total: number;
  };
  expect(byAccented.total).toBe(1);
  expect(byAccented.items[0].id).toBe(accented);

  const byValue = (await Leads.page(
    { owner: `Chi ${stamp}`, minValue: 10_000_000 },
    null,
    50
  )) as { items: { id: number }[]; total: number };
  expect(byValue.total).toBe(1);
  expect(byValue.items[0].id).toBe(rich);

  const zalo = (state: string) =>
    Leads.page({ search: String(stamp), zalo: state }, null, 50) as Promise<{ total: number }>;
  expect((await zalo("unchecked")).total).toBeGreaterThanOrEqual(2);
  expect((await zalo("verified")).total).toBe(0);
  expect(cheap).toBeGreaterThan(0);
});
