/** The read-only summary the Tổng quan tab shows, kept out of the screen so a test can check it. */

export interface DashboardLead {
  status: string;
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
  const lost = deals.filter((deal) => deal.stage === "Lost");
  const open = deals
    .filter((deal) => deal.stage !== "Won" && deal.stage !== "Lost")
    .sort((a, b) => amountOf(b) - amountOf(a));
  const sum = (rows: DashboardDeal[]) => rows.reduce((total, deal) => total + amountOf(deal), 0);
  return {
    wonValue: sum(won),
    wonCount: won.length,
    activeLeads: leads.filter((lead) => lead.status !== "Converted" && lead.status !== "Lost").length,
    pipelineValue: sum(open),
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
