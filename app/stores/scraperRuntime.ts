import { codeOf } from "./core";

/** What the Scraper screen shows next to "Trình duyệt:" and whether the failure is worth a toast. */
export interface RuntimeVerdict {
  label: string;
  /** True when the check itself broke (not just "the browser is not installed"). */
  toast: boolean;
}

/**
 * Reads the code `GoogleMaps.runtimeStatus()` failed with. `unavailable` is the normal answer of a machine
 * without the CloakBrowser sidecar: the jobs stay readable, so it is a quiet "chưa sẵn sàng". Any other code
 * (`internal`, `not_found`, ...) means the check itself went wrong and must not pass as "not installed".
 */
export function runtimeVerdict(e: unknown): RuntimeVerdict {
  if (codeOf(e) === "unavailable") return { label: "chưa sẵn sàng", toast: false };
  return { label: "không kiểm tra được", toast: true };
}
