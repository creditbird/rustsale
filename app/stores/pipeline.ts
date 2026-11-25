import type { Lead } from "../components/LeadDetail";

/** A deal as the core returns it (snake_case, as the v2 commands did). */
export interface Deal {
  id: number;
  lead_id: number;
  title: string;
  value: number | null;
  stage: string;
  created_at: string | null;
  currency: string | null;
}

/** The stages of v2's board, in order. The core accepts any stage string; others land in OTHER. */
export const STAGES: { key: string; label: string; prob: string }[] = [
  { key: "Prospecting", label: "Tiềm năng", prob: "20%" },
  { key: "Demo", label: "Demo", prob: "40%" },
  { key: "Proposal", label: "Báo giá", prob: "60%" },
  { key: "Negotiation", label: "Đàm phán", prob: "80%" },
  { key: "Won", label: "Thành công", prob: "100%" },
];

/** The sixth column: deals whose stage is not one of STAGES (v2 hid these). */
export const OTHER = "__other";

export function columnOf(stage: string): string {
  return STAGES.some((s) => s.key === stage) ? stage : OTHER;
}

/** The stage key one step along the board from `stage`, or null at the ends (OTHER has no neighbours). */
export function neighbour(stage: string, step: -1 | 1): string | null {
  const i = STAGES.findIndex((s) => s.key === stage);
  if (i < 0) return step === 1 ? STAGES[0].key : null;
  const j = i + step;
  return j >= 0 && j < STAGES.length ? STAGES[j].key : null;
}

/** Groups deals into columns once per change (not once per card). Keeps the core's order within a column. */
export function group(deals: Deal[], query: string, leads: Map<number, Lead>): Record<string, Deal[]> {
  const q = query.trim().toLowerCase();
  const out: Record<string, Deal[]> = { [OTHER]: [] };
  for (const s of STAGES) out[s.key] = [];
  for (const d of deals) {
    if (q !== "") {
      const lead = leads.get(d.lead_id);
      const hay = `${d.title} ${lead?.name ?? ""} ${lead?.company ?? ""}`.toLowerCase();
      if (!hay.includes(q)) continue;
    }
    out[columnOf(d.stage)].push(d);
  }
  return out;
}

/** The total of a column, per currency, as people read it: "35 tr · 500 USD". Empty column: "—". */
export function totalOf(deals: Deal[]): string {
  const sums = new Map<string, number>();
  for (const d of deals) {
    const c = d.currency ?? "VND";
    sums.set(c, (sums.get(c) ?? 0) + (d.value ?? 0));
  }
  const parts: string[] = [];
  for (const [c, v] of sums) {
    if (v <= 0) continue;
    parts.push(c === "VND" ? (v >= 1_000_000 ? `${Math.round(v / 100_000) / 10} tr` : `${v}`) : `${v} ${c}`);
  }
  return parts.length === 0 ? "—" : parts.join(" · ");
}

export type AmountResult = { ok: true; value: number } | { ok: false; message: string };

/**
 * Reads the "Giá trị" field. The core stores a deal value as a REAL in the currency's major unit
 * (VND dong, USD dollars), so the result is that number, never minor units.
 * VND: whole number; "." or "," (one kind only) may group thousands in threes ("1.000.000", "1,234").
 * USD: "," groups thousands, "." starts at most 2 decimals ("1.5", "1,234.50").
 * Anything that could be read two ways ("1.234" USD, "1.234,5") is rejected rather than guessed.
 */
export function parseAmount(raw: string, currency: string): AmountResult {
  const s = raw.replace(/\s/g, "");
  const bad = (message: string): AmountResult => ({ ok: false, message });
  if (s === "") return bad("Nhập giá trị của deal.");
  if (s.startsWith("-")) return bad("Giá trị không được âm.");
  if (!/^[0-9.,]+$/.test(s)) return bad("Giá trị chỉ gồm chữ số và dấu phân tách.");
  if (currency === "USD") {
    const m = /^(\d{1,3}(?:,\d{3})+|\d+)(?:\.(\d{1,2}))?$/.exec(s);
    if (m === null) return bad("USD: tối đa 2 số lẻ, dấu phẩy ngăn hàng nghìn (vd 1,234.50). Giá trị này mơ hồ, hãy nhập lại.");
    return { ok: true, value: Number(`${m[1].replace(/,/g, "")}${m[2] ? `.${m[2]}` : ""}`) };
  }
  const m = /^(\d{1,3}(?:([.,])\d{3}(?:\2\d{3})*)|\d+)$/.exec(s);
  if (m === null) return bad("VND chỉ nhận số nguyên, dấu . hoặc , chỉ để ngăn hàng nghìn (vd 1.000.000). Giá trị này mơ hồ, hãy nhập lại.");
  const value = Number(s.replace(/[.,]/g, ""));
  return Number.isSafeInteger(value) ? { ok: true, value } : bad("Giá trị quá lớn.");
}

export const DEFAULT_DEAL_VALUE: Record<string, string> = { VND: "10000000", USD: "500" };

/** The amount field after the currency changes: an untouched default follows the currency, a number the user typed stays. */
export function valueAfterCurrencyChange(current: string, next: string): string {
  const untouched = Object.values(DEFAULT_DEAL_VALUE).includes(current.trim());
  return untouched ? (DEFAULT_DEAL_VALUE[next] ?? current) : current;
}
