// Headless: the Tổng quan tab against the real native module on the shared demo folder.
// The numbers are computed from the core here, never assumed (other screens add leads and deals).
import { expect, open, test } from "nsh/test";
import { Deals, Host, LeadsMore, Settings } from "rustsale/native";
import { displayMoney } from "./stores/dashboard";

const dir = `/tmp/rustsale-v3-ui-l5a-dashboard-${Date.now()}`;

interface DealRow {
  id: number;
  title: string;
  value: number;
  stage: string;
  currency: string | null;
}

interface Currency {
  base_currency: string;
  exchange_rate: number;
}

interface LeadRow {
  id: number;
  name: string;
  status: string;
}

async function openDashboard() {
  await Host.start(dir);
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-dashboard");
  await app.waitFor("id:dash-kpi-revenue");
  return app;
}
const has = (app: Awaited<ReturnType<typeof openDashboard>>, s: string) => app.texts().some((t) => t.includes(s));

function convert(value: number, from: string | null, to: string, rate: number): number {
  const currency = from ?? "VND";
  if (currency === to) return value;
  if (currency === "USD" && to === "VND") return value * rate;
  if (currency === "VND" && to === "USD") return value / rate;
  return value;
}

function total(rows: DealRow[], currency: Currency): number {
  return rows.reduce(
    (sum, deal) => sum + convert(deal.value, deal.currency, currency.base_currency, currency.exchange_rate),
    0,
  );
}

test("the KPIs match the leads and deals the core holds", async () => {
  const app = await openDashboard();
  const deals = (await Deals.list()) as DealRow[];
  const leads = (await LeadsMore.list()) as LeadRow[];
  const currency = (await Settings.currency()) as Currency;
  const won = deals.filter((deal) => deal.stage === "Won");
  const openDeals = deals.filter((deal) => deal.stage !== "Won" && deal.stage !== "Lost");
  const activeLeads = leads.filter((lead) => lead.status !== "Converted" && lead.status !== "Lost").length;
  const revenue = displayMoney(total(won, currency), currency.base_currency);
  const pipeline = displayMoney(total(openDeals, currency), currency.base_currency);
  await app.waitUntil(() =>
    app.texts().includes(revenue) &&
    app.texts().includes(`${won.length} deal`) &&
    app.texts().includes(`${activeLeads} lead`) &&
    app.texts().includes(pipeline),
  );
  expect(app.texts()).toContain(revenue);
  expect(app.texts()).toContain(pipeline);
  expect(app.texts()).toContain("Acme Platform Expansion");
});

test("the recent activity list is the newest five of the core", async () => {
  const app = await openDashboard();
  const recent = (await LeadsMore.recentActivities(5)) as { activity_type: string; content: string }[];
  expect(recent.length).toBeGreaterThan(0);
  await app.waitUntil(() =>
    recent.every((activity) => app.texts().includes(`${activity.activity_type}: ${activity.content}`)),
  );
});

test("refresh picks up a deal created elsewhere", async () => {
  const stamp = Date.now();
  const title = `Deal L5a ${stamp}`;
  const app = await openDashboard();
  const lead = ((await LeadsMore.list()) as LeadRow[])[0];
  await Deals.create({ leadId: lead.id, title, value: 888000000, stage: "Proposal", currency: "VND" });
  await app.press("id:dash-refresh");
  await app.waitUntil(() => has(app, title));
  const deals = (await Deals.list()) as DealRow[];
  const currency = (await Settings.currency()) as Currency;
  const openDeals = deals.filter((deal) => deal.stage !== "Won" && deal.stage !== "Lost");
  await app.waitUntil(() => app.texts().includes(displayMoney(total(openDeals, currency), currency.base_currency)));
});
