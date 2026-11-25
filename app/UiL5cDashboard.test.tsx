// Headless: the dashboard period switch and the pipeline stage distribution.
import { expect, open, test } from "nsh/test";
import { Host, Settings } from "rustsale/native";
import {
  dealInPeriod,
  leadInPeriod,
  periodStart,
  stageBreakdown,
  type CurrencySettings,
  type DashboardDeal,
  type DashboardLead,
} from "./stores/dashboard";

const dir = `/tmp/rustsale-v3-ui-l5c-dashboard-${Date.now()}`;
const vnd: CurrencySettings = { base_currency: "VND", exchange_rate: 25_400 };

function deal(id: number, stage: string, value: number, createdAt: string): DashboardDeal {
  return {
    id,
    lead_id: 1,
    title: `Deal ${id}`,
    value,
    stage,
    created_at: createdAt,
    currency: "VND",
  };
}

function lead(status: string, lastActivity: string): DashboardLead {
  return { status, last_activity: lastActivity };
}

test("a period keeps only the deals created inside it", () => {
  const now = new Date("2026-11-25T12:00:00Z");
  const inside = { created_at: new Date("2026-11-01T00:00:00Z").toISOString() };
  const outside = { created_at: new Date("2026-09-01T00:00:00Z").toISOString() };
  expect(dealInPeriod(inside, "", now)).toBe(true);
  expect(dealInPeriod(inside, "30", now)).toBe(true);
  expect(dealInPeriod(outside, "30", now)).toBe(false);
  expect(dealInPeriod(outside, "90", now)).toBe(true);
  expect(periodStart("year", now)?.getMonth()).toBe(0);
  expect(periodStart("", now)).toBeNull();
});

test("a period keeps only the leads inside it", () => {
  const now = new Date("2026-11-25T12:00:00Z");
  const recent = lead("New", "2026-11-10T08:00:00Z");
  const older = lead("Qualified", "2026-09-15T08:00:00Z");
  const lastYear = lead("New", "2025-12-20T08:00:00Z");
  const blank = lead("New", "");
  expect(leadInPeriod(recent, "", now)).toBe(true);
  expect(leadInPeriod(blank, "", now)).toBe(true);
  expect(leadInPeriod({ last_activity: null }, "", now)).toBe(true);
  expect(leadInPeriod(recent, "30", now)).toBe(true);
  expect(leadInPeriod(older, "30", now)).toBe(false);
  expect(leadInPeriod(older, "90", now)).toBe(true);
  expect(leadInPeriod(lastYear, "90", now)).toBe(false);
  expect(leadInPeriod(lastYear, "year", now)).toBe(false);
  expect(leadInPeriod(older, "year", now)).toBe(true);
  expect(leadInPeriod(blank, "30", now)).toBe(false);
  expect(leadInPeriod({ last_activity: null }, "30", now)).toBe(false);
});

test("stage breakdown counts and values every pipeline stage", () => {
  const now = new Date().toISOString();
  const rows = stageBreakdown(
    [deal(1, "Won", 100_000_000, now), deal(2, "Won", 50_000_000, now), deal(3, "Negotiation", 20_000_000, now)],
    vnd,
  );
  const won = rows.find((row) => row.stage === "Won")!;
  const negotiation = rows.find((row) => row.stage === "Negotiation")!;
  const prospecting = rows.find((row) => row.stage === "Prospecting")!;
  expect(won.count).toBe(2);
  expect(won.value).toBe(150_000_000);
  expect(won.pct).toBe(88);
  expect(negotiation.count).toBe(1);
  expect(negotiation.value).toBe(20_000_000);
  expect(prospecting.count).toBe(0);
  expect(rows.map((row) => row.stage)).toEqual([
    "Prospecting",
    "Demo",
    "Proposal",
    "Negotiation",
    "Won",
    "Lost",
  ]);
});

test("the dashboard switch narrows the KPIs (deals and leads) and the stage rows", async () => {
  const now = new Date();
  const older = new Date(now.getTime() - 60 * 86_400_000);
  const veryOld = new Date(now.getTime() - 120 * 86_400_000);
  const deals = [
    deal(1, "Won", 100_000_000, now.toISOString()),
    deal(2, "Won", 50_000_000, older.toISOString()),
    deal(3, "Negotiation", 20_000_000, now.toISOString()),
  ];
  const leads = [
    lead("New", now.toISOString()),
    lead("Qualified", older.toISOString()),
    lead("Contacted", veryOld.toISOString()),
    lead("Lost", now.toISOString()),
  ];
  await Host.start(dir);
  await Settings.updateCurrency("VND", 25_400);
  const app = await open("./App.tsx", {
    modules: {
      "rustsale/native": {
        "Deals.list": () => deals,
        "LeadsMore.list": () => leads,
      },
    },
  });
  await app.setViewport(1280, 800);
  await app.press("id:nav-dashboard");
  await app.waitFor("id:dash-kpi-revenue-value");
  await app.waitUntil(() => app.texts().some((text) => text.includes("150.000.000 ₫")));
  await app.waitUntil(() => app.texts().some((text) => text.includes("2 deal")));
  await app.waitUntil(() => app.texts().some((text) => text.includes("3 lead")));

  await app.press("id:dash-period-30");
  await app.waitUntil(() => app.texts().some((text) => text.includes("100.000.000 ₫")));
  await app.waitUntil(() => app.texts().some((text) => text.includes("1 lead")));
  await app.waitFor("id:dash-stage-Won");
  expect(app.texts().some((text) => text.includes("50.000.000 ₫"))).toBe(false);
  expect(app.texts().some((text) => text.includes("3 lead"))).toBe(false);

  await app.press("id:dash-period-90");
  await app.waitUntil(() => app.texts().some((text) => text.includes("150.000.000 ₫")));
  await app.waitUntil(() => app.texts().some((text) => text.includes("2 lead")));

  await app.press("id:dash-period-all");
  await app.waitUntil(() => app.texts().some((text) => text.includes("150.000.000 ₫")));
  await app.waitUntil(() => app.texts().some((text) => text.includes("3 lead")));
  await app.waitFor("id:dash-stage-Negotiation");
});
