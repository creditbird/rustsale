/** Pure formatting helpers for UI-46 (kept free of `nsh` imports for unit tests). */

/** Builds the SePay/VietQR transfer memo for an order code. */
export function transferMemoFor(orderCode: number): string {
  return `RS SEVQR TKPRS ${orderCode}`;
}

/** Formats VND amounts with dot grouping for display. */
export function formatVnd(amount: number): string {
  if (!Number.isFinite(amount)) return "0 VND";
  const rounded = Math.round(amount);
  const grouped = String(Math.abs(rounded)).replace(/\B(?=(\d{3})+(?!\d))/g, ".");
  return `${rounded < 0 ? "-" : ""}${grouped} VND`;
}

/** Formats an ISO timestamp into DD/MM/YYYY, safely handling invalid inputs. */
export function formatLicenseDate(iso?: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const dd = String(d.getDate()).padStart(2, "0");
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const yyyy = d.getFullYear();
  return `${dd}/${mm}/${yyyy}`;
}

/** Returns user initials for the sidebar account avatar. */
export function licenseInitials(email?: string | null): string {
  if (!email) return "RS";
  const name = email.split("@")[0] ?? "";
  const parts = name.split(/[._-]/).filter(Boolean);
  if (parts.length >= 2) {
    return `${parts[0][0] ?? ""}${parts[1][0] ?? ""}`.toUpperCase();
  }
  return (name.slice(0, 2) || "RS").toUpperCase();
}
