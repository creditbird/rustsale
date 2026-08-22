// Facebook 24-Hour Policy Gate — pure logic (revised plan Task 4).
//
// Production policy model (aligned with current Meta Messenger Platform):
// - 0..24h after the customer's last inbound message: free-form sending.
// - 24h..7d: ONLY the HUMAN_AGENT tag may be used, and only if the Page has
//   the Human Agent scope approved by Meta App Review.
// - Beyond 7 days: sending is not possible through any standard mechanism.
//
// The legacy CONFIRMED_EVENT_UPDATE / POST_PURCHASE_UPDATE / ACCOUNT_UPDATE
// tags are intentionally NOT offered: Meta retired them (Graph error 100).
//
// Missing or unparseable interaction time FAILS CLOSED (treated as outside
// every window).

export const WINDOW_MS = 24 * 60 * 60 * 1000;
export const HUMAN_AGENT_WINDOW_MS = 7 * WINDOW_MS;

export const FACEBOOK_MESSAGE_TAGS = [
  {
    value: 'HUMAN_AGENT',
    labelVi: 'Nhân viên CSKH — cửa sổ 7 ngày (yêu cầu Meta phê duyệt scope)'
  }
] as const;

export type FacebookMessageTag = (typeof FACEBOOK_MESSAGE_TAGS)[number]['value'];

export interface FacebookWindowStatus {
  /** Within 24h: free-form sending allowed without any tag. */
  freeWindow: boolean;
  /** Within 7d: the HUMAN_AGENT tag may still be used. */
  humanAgentEligible: boolean;
  /** Milliseconds remaining in the free 24h window (0 once closed). */
  remainingMs: number;
}

/**
 * Evaluates both windows against a fixed `nowMs` (dependency-injected for
 * deterministic tests/UI ticking). Accepts epoch-ms or RFC-3339 strings.
 */
export function evaluateFacebookWindow(
  lastInboundAt: number | string | null | undefined,
  nowMs: number
): FacebookWindowStatus {
  let ms: number;
  if (typeof lastInboundAt === 'number') {
    ms = lastInboundAt;
  } else if (typeof lastInboundAt === 'string' && lastInboundAt.length > 0) {
    ms = Date.parse(lastInboundAt);
  } else {
    return { freeWindow: false, humanAgentEligible: false, remainingMs: 0 };
  }
  if (!Number.isFinite(ms)) {
    return { freeWindow: false, humanAgentEligible: false, remainingMs: 0 };
  }
  const elapsed = nowMs - ms;
  const remainingMs = Math.max(0, WINDOW_MS - elapsed);
  return {
    freeWindow: elapsed < WINDOW_MS,
    humanAgentEligible: elapsed >= 0 && elapsed < HUMAN_AGENT_WINDOW_MS,
    remainingMs
  };
}

/**
 * Outbound gate. Inside 24h everything is allowed. Outside 24h but within
 * 7d requires exactly the HUMAN_AGENT tag. Beyond 7d nothing unlocks.
 */
export function validateOutboundPolicy(
  status: FacebookWindowStatus,
  selectedTag: string | null
): { allowed: boolean; reason: string | null } {
  if (status.freeWindow) {
    return { allowed: true, reason: null };
  }
  if (!status.humanAgentEligible) {
    return {
      allowed: false,
      reason: 'Đã quá 7 ngày kể từ tương tác cuối — không thể gửi tin nhắn mới qua Messenger.'
    };
  }
  if (selectedTag === 'HUMAN_AGENT') {
    return { allowed: true, reason: null };
  }
  return {
    allowed: false,
    reason: 'Đã quá cửa sổ 24h — chỉ có thể gửi bằng thẻ HUMAN_AGENT.'
  };
}

/** Formats remaining milliseconds as "23h 05m" style countdown text. */
export function formatRemainingHm(remainingMs: number): string {
  const safe = Math.max(0, remainingMs);
  const h = Math.floor(safe / (60 * 60 * 1000));
  const m = Math.floor((safe % (60 * 60 * 1000)) / 60_000);
  return `${h}h ${String(m).padStart(2, '0')}m`;
}
