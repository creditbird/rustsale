import { ImportLeads, Leads } from "rustsale/native";
import { startCore, type Page } from "./core";
import type { Lead } from "../components/LeadDetail";

/** The column order of an exported file; the same names the import maps back. */
export const CSV_HEADERS = [
  "Name",
  "Company",
  "Phone",
  "Email",
  "Status",
  "Value",
  "Currency",
  "Zalo_Status",
  "Zalo_UID",
  "Owner",
  "Address",
  "About",
  "Last_Activity",
];

const FORMULA_PREFIX = /^\s*[=+\-@\t\r]/u;
const PHONE_PREFIX = /^\s*0\d+/u;

/** Escapes one CSV field, keeping a leading zero and neutralising a spreadsheet formula. */
export function csvField(value: string | number | null | undefined): string {
  if (value === null || value === undefined) return '""';
  const text = String(value);
  const safe = FORMULA_PREFIX.test(text) || PHONE_PREFIX.test(text) ? `'${text}` : text;
  return `"${safe.replace(/"/g, '""')}"`;
}

function zaloLabel(lead: Lead): string {
  if (lead.zalo_status && lead.zalo_status.trim() !== "") return lead.zalo_status.trim();
  if (lead.has_zalo === 1) return "Verified";
  if (lead.has_zalo === 0) return "No Zalo";
  return "Unchecked";
}

/** The selected leads as the UTF-8 BOM CSV the save dialog writes, CRLF line endings. */
export function buildLeadsCsv(leads: readonly Lead[]): string {
  const header = CSV_HEADERS.map(csvField).join(",");
  const rows = leads.map((lead) =>
    [
      csvField(lead.name),
      csvField(lead.company),
      csvField(lead.phone),
      csvField(lead.email),
      csvField(lead.status),
      csvField(lead.value ?? 0),
      csvField(lead.currency ?? "VND"),
      csvField(zaloLabel(lead)),
      csvField(lead.zalo_uid),
      csvField(lead.owner),
      csvField(lead.address),
      csvField(lead.about),
      csvField(lead.last_activity),
    ].join(","),
  );
  return `\uFEFF${[header, ...rows].join("\r\n")}`;
}

/** `RustSale_Leads_YYYYMMDD_HHmmss.csv` from the local clock. */
export function exportFilename(now = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  const stamp = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}_${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  return `RustSale_Leads_${stamp}.csv`;
}

/** Every row matching `query`, page by page, so an export is not limited to the first page. */
export async function fetchAllLeads(query: Record<string, unknown>): Promise<Lead[]> {
  await startCore();
  const all: Lead[] = [];
  let cursor: string | null = null;
  for (;;) {
    const page = (await Leads.page(query, cursor, 200)) as Page<Lead>;
    all.push(...page.items);
    if (page.nextCursor === null) break;
    cursor = page.nextCursor;
  }
  return all;
}

/** Writes `leads` as CSV through the native save dialog; resolves the saved path or `null`. */
export async function saveLeadsCsv(leads: readonly Lead[]): Promise<string | null> {
  return (await ImportLeads.exportCsv(exportFilename(), buildLeadsCsv(leads))) as string | null;
}

export interface ColumnMapping {
  name_col: number | null;
  company_col: number | null;
  phone_col: number | null;
  email_col: number | null;
  status_col: number | null;
  value_col: number | null;
  address_col: number | null;
  about_col: number | null;
  owner_col: number | null;
}

export interface ImportInspection {
  file_type: string;
  sheets: string[];
  selected_sheet: string | null;
  headers: string[];
  sample_rows: string[][];
  total_rows_estimate: number;
}

/** The mapping fields in the order the dialog lists them. */
export const MAPPING_FIELDS: { key: keyof ColumnMapping; label: string }[] = [
  { key: "name_col", label: "Tên khách" },
  { key: "company_col", label: "Công ty" },
  { key: "phone_col", label: "Điện thoại" },
  { key: "email_col", label: "Email" },
  { key: "status_col", label: "Trạng thái" },
  { key: "value_col", label: "Giá trị" },
  { key: "address_col", label: "Địa chỉ" },
  { key: "about_col", label: "Ghi chú" },
  { key: "owner_col", label: "Phụ trách" },
];

export function emptyMapping(): ColumnMapping {
  return {
    name_col: null,
    company_col: null,
    phone_col: null,
    email_col: null,
    status_col: null,
    value_col: null,
    address_col: null,
    about_col: null,
    owner_col: null,
  };
}

/** Matches each header to the first unused CRM field it names, like the v2 importer did. */
export function autoMap(headers: readonly string[]): ColumnMapping {
  const mapping = emptyMapping();
  const isFree = (key: keyof ColumnMapping) => mapping[key] === null;
  const clean = (h: string) => h.toLowerCase().trim().replace(/[(*)_]/g, "");
  headers.forEach((header, index) => {
    const text = clean(header);
    if (isFree("name_col") && /(họ\s*và\s*tên|họ\s*tên|^tên|full\s*name|^name|khách\s*hàng)/i.test(text)) {
      mapping.name_col = index;
    } else if (isFree("phone_col") && /(điện\s*thoại|sđt|phone|mobile|tel|liên\s*hệ)/i.test(text)) {
      mapping.phone_col = index;
    } else if (isFree("email_col") && /(email|thư\s*điện\s*tử|e-mail|mail)/i.test(text)) {
      mapping.email_col = index;
    } else if (isFree("company_col") && /(công\s*ty|doanh\s*nghiệp|đơn\s*vị|company|corp|organization)/i.test(text)) {
      mapping.company_col = index;
    } else if (isFree("value_col") && /(giá\s*trị|ngân\s*sách|tiền|value|budget|deal|amount)/i.test(text)) {
      mapping.value_col = index;
    } else if (isFree("address_col") && /(địa\s*chỉ|address|nơi\s*ở|location|khu\s*vực)/i.test(text)) {
      mapping.address_col = index;
    } else if (isFree("about_col") && /(ghi\s*chú|mô\s*tả|note|about|description|yêu\s*cầu)/i.test(text)) {
      mapping.about_col = index;
    } else if (isFree("status_col") && /(trạng\s*thái|status|tình\s*trạng|stage)/i.test(text)) {
      mapping.status_col = index;
    } else if (isFree("owner_col") && /(phụ\s*trách|owner|nhân\s*viên|assignee)/i.test(text)) {
      mapping.owner_col = index;
    }
  });
  return mapping;
}

export interface ImportSummary {
  total_rows: number;
  inserted: number;
  updated: number;
  skipped_duplicates: number;
  failed: number;
}

export interface ImportProgress {
  processed_rows: number;
  total_rows: number;
  percentage: number;
}
