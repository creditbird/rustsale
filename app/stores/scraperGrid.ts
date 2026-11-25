/** Pure helpers of the Scraper screen's grid mode and job retries: no native calls, so they test without a core. */

/** `[latMin, latMax, lngMin, lngMax]`, the order the core takes everywhere. */
export type Bbox = [number, number, number, number];

/** The backend refuses a grid step outside this range (B8), so the form says so before asking. */
export const STEP_MIN_KM = 0.2;
export const STEP_MAX_KM = 50;
export const STEP_DEFAULT_KM = 2;

/** `GoogleMapsMore.planGrid` answer. */
export interface GridPlan {
  area_km2: number;
  width_km: number;
  height_km: number;
  requested_step_km: number;
  effective_step_km: number;
  estimated_cells: number;
  recommended_step_km: number | null;
  exceeds_limit: boolean;
  estimated_chunks: number;
}

/** One entry of `GoogleMapsMore.presetProvinces` (camelCase, unlike the catalog's bounds). */
export interface PresetBounds { minLat: number; maxLat: number; minLng: number; maxLng: number }

/** `GoogleMapsMore.resolveBounds` answer. */
export interface CatalogBounds { min_lat: number; max_lat: number; min_lng: number; max_lng: number }

export type Parsed<T> = { ok: true; value: T } | { ok: false; error: string };

/** A number typed by hand; a decimal comma counts as a point. Empty or non-numeric text is `null`. */
export function parseNumber(text: string): number | null {
  const t = text.trim().replace(",", ".");
  if (t === "") return null;
  const n = Number(t);
  return Number.isFinite(n) ? n : null;
}

/** The four bbox fields of the form as one bbox, or the sentence that tells which field is wrong. */
export function parseBbox(latMin: string, latMax: string, lngMin: string, lngMax: string): Parsed<Bbox> {
  const values = [parseNumber(latMin), parseNumber(latMax), parseNumber(lngMin), parseNumber(lngMax)];
  if (values.some((v) => v === null)) return { ok: false, error: "Cần nhập đủ 4 toạ độ (vĩ độ nhỏ/lớn, kinh độ nhỏ/lớn)." };
  const [a, b, c, d] = values as number[];
  if (Math.abs(a) > 90 || Math.abs(b) > 90) return { ok: false, error: "Vĩ độ phải nằm trong khoảng -90 đến 90." };
  if (Math.abs(c) > 180 || Math.abs(d) > 180) return { ok: false, error: "Kinh độ phải nằm trong khoảng -180 đến 180." };
  if (a >= b) return { ok: false, error: "Vĩ độ nhỏ phải nhỏ hơn vĩ độ lớn." };
  if (c >= d) return { ok: false, error: "Kinh độ nhỏ phải nhỏ hơn kinh độ lớn." };
  return { ok: true, value: [a, b, c, d] };
}

/** The grid step in km, inside the range the backend accepts. */
export function parseStep(text: string): Parsed<number> {
  const n = parseNumber(text);
  if (n === null) return { ok: false, error: "Cần nhập bước lưới (km)." };
  if (n < STEP_MIN_KM || n > STEP_MAX_KM) return { ok: false, error: `Bước lưới phải từ ${STEP_MIN_KM} đến ${STEP_MAX_KM} km.` };
  return { ok: true, value: n };
}

export const presetBbox = (p: PresetBounds): Bbox => [p.minLat, p.maxLat, p.minLng, p.maxLng];
export const catalogBbox = (b: CatalogBounds): Bbox => [b.min_lat, b.max_lat, b.min_lng, b.max_lng];

/** A coordinate as the form field shows it: at most 4 decimals (about 11 m), no trailing zeros. */
export function fmtCoord(n: number): string {
  return String(Number(n.toFixed(4)));
}

/** Lower-case, accent-free text so "ho chi minh" finds "Thành phố Hồ Chí Minh". */
export function fold(text: string): string {
  return text.normalize("NFD").replace(/[\u0300-\u036f]/g, "").replace(/đ/g, "d").replace(/Đ/g, "D").toLowerCase().trim();
}

/** The names that contain `query` (accents and case ignored), in their given order; all of them for an empty query. */
export function filterNames(names: string[], query: string): string[] {
  const q = fold(query);
  return q === "" ? names : names.filter((n) => fold(n).includes(q));
}

/** Whether the core takes "Thử lại ô lỗi" for a job in this status (only a finished job with failures). */
export const canRetryUnits = (status: string, failedUnits: number): boolean =>
  (status === "partial" || status === "failed") && failedUnits > 0;

/** Whether the core takes "Thử lại email": it reopens a `partial` job only. */
export const canRetryEmail = (status: string): boolean => status === "partial";

/** 12345 as "12.345" (the Vietnamese grouping), without relying on the engine's locale data. */
export function thousands(n: number): string {
  return String(Math.trunc(n)).replace(/\B(?=(\d{3})+(?!\d))/g, ".");
}

/** One line about the plan: cells, area and step, and what to do when it is over the limit. */
export function planSummary(plan: GridPlan): string {
  const base = `~${thousands(plan.estimated_cells)} ô · ${thousands(Math.round(plan.area_km2))} km² · bước ${plan.effective_step_km} km`;
  if (!plan.exceeds_limit) return base;
  const hint = plan.recommended_step_km !== null ? ` Thử bước từ ${plan.recommended_step_km} km.` : " Hãy thu nhỏ vùng hoặc tăng bước.";
  return `${base}. Vượt giới hạn số ô.${hint}`;
}

/** An id-safe form of a name ("Thành phố Hồ Chí Minh" -> "thanh-pho-ho-chi-minh") for test ids. */
export function slug(text: string): string {
  return fold(text).replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
}
