/** Pure formatting and progress helpers for UI-45 (kept free of `nsh` imports for unit tests). */

export interface UpdateProgressState {
  received: number;
  total: number;
  percentage: number;
}

/** Formats a byte count into a human-readable string (B, KB, MB, GB). */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.min(sizes.length - 1, Math.floor(Math.log(bytes) / Math.log(k)));
  return `${Number((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}

/** Computes clamped byte counts and percentage from a raw `nsh/updater` progress event. */
export function computeUpdateProgress(
  receivedRaw: number | null | undefined,
  totalRaw?: number | null,
): UpdateProgressState {
  const received = Math.max(0, Number(receivedRaw ?? 0) || 0);
  const total =
    totalRaw !== null && totalRaw !== undefined ? Math.max(0, Number(totalRaw) || 0) : 0;
  const percentage = total > 0 ? Math.min(100, Math.round((received / total) * 100)) : 0;
  return { received, total, percentage };
}
