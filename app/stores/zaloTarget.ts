/** Structured skip record returned inside `ZaloOutreachStartResult`. */
export interface SkippedZaloLead {
  lead_id: number;
  reason: string;
}

/** Structured result returned by `Campaigns.startZaloOutreach`. */
export interface ZaloOutreachStartResult {
  queued: number;
  skipped: number;
  skipped_leads: SkippedZaloLead[];
}

const INVALID_UIDS = new Set(["0", "-0", "-1", "null", "undefined"]);
const VN_MOBILE_PREFIXES = new Set(["03", "05", "07", "08", "09"]);

/** Mirrors `rustsale_core::commands::sanitize_valid_zalo_uid`. */
export function hasValidZaloUid(uid: string | null | undefined): boolean {
  const trimmed = (uid ?? "").trim();
  if (trimmed === "" || INVALID_UIDS.has(trimmed.toLowerCase())) return false;
  return !/^0+$/.test(trimmed);
}

/** Mirrors `rustsale_core::commands::normalize_phone_digits`. */
export function normalizePhoneDigits(phone: string): string {
  const stop = phone.search(/[^0-9 +\-().,[\]]/);
  const phonePart = stop === -1 ? phone : phone.slice(0, stop);
  const digits = phonePart.replace(/[^0-9]/g, "");
  if (digits.startsWith("0084")) return `0${digits.slice(4)}`;
  if (digits.startsWith("84")) return `0${digits.slice(2)}`;
  return digits;
}

/** Mirrors `rustsale_core::commands::is_vn_mobile`. */
export function isVnMobile(phone: string | null | undefined): boolean {
  if (!phone) return false;
  const normalized = normalizePhoneDigits(phone);
  return normalized.length === 10 && VN_MOBILE_PREFIXES.has(normalized.slice(0, 2));
}

/** Mirrors `rustsale_core::commands::is_valid_zalo_lead_target`. */
export function isValidZaloLeadTarget(
  phone: string | null | undefined,
  zaloUid: string | null | undefined,
): boolean {
  return hasValidZaloUid(zaloUid) || isVnMobile(phone);
}

/** Parses `Campaigns.startZaloOutreach`'s result, failing closed (`queued: 0`) if malformed. */
export function parseZaloOutreachResult(raw: unknown): ZaloOutreachStartResult {
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (typeof obj.queued === "number" && typeof obj.skipped === "number") {
      const skippedLeads = Array.isArray(obj.skipped_leads)
        ? (obj.skipped_leads as SkippedZaloLead[])
        : [];
      return {
        queued: obj.queued,
        skipped: obj.skipped,
        skipped_leads: skippedLeads,
      };
    }
  }
  return { queued: 0, skipped: 0, skipped_leads: [] };
}
