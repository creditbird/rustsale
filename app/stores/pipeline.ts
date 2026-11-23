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
