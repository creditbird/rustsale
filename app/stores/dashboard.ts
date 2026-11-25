/** The read-only summary the Tổng quan tab shows, kept out of the screen so a test can check it. */

export interface DashboardLead {
  status: string;
  /** The `leads` table has no `created_at` column; `last_activity` is stamped on insert and used by v2 for period filtering. */
  last_activity?: string | null;
  created_at?: string | null;
}

export interface DashboardDeal {
  id: number;
  lead_id: number;
  title: string;
  value: number;
  stage: string;
  created_at: string;
  currency: string | null;
}

export interface DashboardActivity {
  id: number;
  activity_type: string;
  content: string;
  created_at: string;
}

export interface CurrencySettings {
  base_currency: string;
  exchange_rate: number;
}

export interface DashboardSummary {
  wonValue: number;
  wonCount: number;
  activeLeads: number;
  pipelineValue: number;
  /** How many deals are open in all; `deals` below is only the first few of them. */
  openCount: number;
  /** The open deals worth showing, dearest first (already converted to the base currency). */
  deals: DashboardDeal[];
}

/** Converts a deal value into the base currency, the way the v2 dashboard did. */
export function convertAmount(
  value: number,
  from: string | null,
  to: string,
  exchangeRate: number,
): number {
  const currency = from === null || from === "" ? "VND" : from;
  const rate = Number.isFinite(exchangeRate) && exchangeRate > 0 ? exchangeRate : 25400;
  if (currency === to) return value;
  if (currency === "USD" && to === "VND") return value * rate;
  if (currency === "VND" && to === "USD") return value / rate;
  return value;
}

/** The KPIs and the open deals of the dashboard; a lead counts as active until it is won or lost. */
export function summarize(
  leads: DashboardLead[],
  deals: DashboardDeal[],
  currency: CurrencySettings,
): DashboardSummary {
  const amountOf = (deal: DashboardDeal) =>
    convertAmount(deal.value, deal.currency, currency.base_currency, currency.exchange_rate);
  const won = deals.filter((deal) => deal.stage === "Won");
  const open = deals
    .filter((deal) => deal.stage !== "Won" && deal.stage !== "Lost")
    .sort((a, b) => amountOf(b) - amountOf(a));
  const sum = (rows: DashboardDeal[]) => rows.reduce((total, deal) => total + amountOf(deal), 0);
  return {
    wonValue: sum(won),
    wonCount: won.length,
    activeLeads: leads.filter((lead) => lead.status !== "Converted" && lead.status !== "Lost").length,
    pipelineValue: sum(open),
    openCount: open.length,
    deals: open.slice(0, 5),
  };
}

/** A money figure as people read it here: thousands with ".", one decimal with "," and the unit. */
export function displayMoney(value: number, currency: string): string {
  const unit = currency === "VND" ? "₫" : currency === "USD" ? "$" : currency;
  const rounded = Math.round((Number.isFinite(value) ? value : 0) * 100) / 100;
  const [whole, fraction] = String(rounded).split(".");
  const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, ".");
  return fraction ? `${grouped},${fraction} ${unit}` : `${grouped} ${unit}`;
}

/** The window of leads and deals the dashboard shows, as the person picks it. */
export type Period = "" | "30" | "90" | "year";

/** Midnight at the start of `period`, or `null` for all records. */
export function periodStart(period: Period, now: Date = new Date()): Date | null {
  if (period === "") return null;
  const start = new Date(now.getTime());
  start.setHours(0, 0, 0, 0);
  if (period === "year") {
    start.setMonth(0, 1);
    return start;
  }
  const days = period === "30" ? 30 : 90;
  start.setDate(start.getDate() - (days - 1));
  return start;
}

/** Whether an ISO timestamp falls inside `period`. */
export function dateInPeriod(
  timestamp: string | null | undefined,
  period: Period,
  now: Date = new Date(),
): boolean {
  const start = periodStart(period, now);
  if (start === null) return true;
  if (!timestamp) return false;
  const time = new Date(timestamp).getTime();
  return Number.isFinite(time) && time >= start.getTime();
}

/** Whether a deal created at `created_at` falls inside `period`. */
export function dealInPeriod(
  deal: { created_at: string },
  period: Period,
  now: Date = new Date(),
): boolean {
  return dateInPeriod(deal.created_at, period, now);
}

/** Whether a lead falls inside `period` (prefers `created_at` if present, otherwise `last_activity`). */
export function leadInPeriod(
  lead: { created_at?: string | null; last_activity?: string | null },
  period: Period,
  now: Date = new Date(),
): boolean {
  return dateInPeriod(lead.created_at || lead.last_activity, period, now);
}

/** The pipeline stages the distribution shows, in the order the v2 dashboard used. */
export const DASHBOARD_STAGES: { stage: string; label: string }[] = [
  { stage: "Prospecting", label: "Tìm kiếm Tiềm năng" },
  { stage: "Demo", label: "Demo / Giới thiệu" },
  { stage: "Proposal", label: "Gửi Báo giá" },
  { stage: "Negotiation", label: "Đàm phán" },
  { stage: "Won", label: "Thành công" },
  { stage: "Lost", label: "Thất bại" },
];

export interface StageSlice {
  stage: string;
  label: string;
  count: number;
  value: number;
  /** The share of all these deals' value, rounded to a whole percent. */
  pct: number;
}

/** Counts and converted value per pipeline stage, with each stage's share of the total. */
export function stageBreakdown(
  deals: DashboardDeal[],
  currency: CurrencySettings,
): StageSlice[] {
  const amountOf = (deal: DashboardDeal) =>
    convertAmount(deal.value, deal.currency, currency.base_currency, currency.exchange_rate);
  const total = deals.reduce((sum, deal) => sum + amountOf(deal), 0);
  return DASHBOARD_STAGES.map(({ stage, label }) => {
    const rows = deals.filter((deal) => deal.stage === stage);
    const value = rows.reduce((sum, deal) => sum + amountOf(deal), 0);
    return {
      stage,
      label,
      count: rows.length,
      value,
      pct: total > 0 ? Math.round((value / total) * 100) : 0,
    };
  });
}
