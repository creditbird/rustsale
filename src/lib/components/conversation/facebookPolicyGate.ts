// Facebook 24-Hour Policy Gate — pure logic (revised plan Task 4).
//
// Meta Messenger policy: an agent may send free-form messages to a customer
// only within 24 hours of that customer's last interaction. Outside the
// window, sending requires one of the approved Message Tags. Missing or
// unparseable interaction time FAILS CLOSED (treated as outside the window).

export interface FacebookWindowStatus {
  outsideWindow: boolean;
  remainingMs: number;
}

export const WINDOW_MS = 24 * 60 * 60 * 1000;

export const FACEBOOK_MESSAGE_TAGS = [
  { value: 'CONFIRMED_EVENT_UPDATE', labelVi: 'Xác nhận sự kiện' },
  { value: 'POST_PURCHASE_UPDATE', labelVi: 'Cập nhật đơn hàng / vận chuyển' },
  { value: 'ACCOUNT_UPDATE', labelVi: 'Cập nhật tài khoản' },
  { value: 'HUMAN_AGENT', labelVi: 'Nhân viên CSKH (yêu cầu Page duyệt scope)' }
] as const;

export type FacebookMessageTag = (typeof FACEBOOK_MESSAGE_TAGS)[number]['value'];

/**
 * Evaluates the 24-hour customer-service window against a fixed `nowMs`
 * (dependency-injected for deterministic tests/UI ticking). Accepts either an
 * epoch-ms number or an RFC-3339 string; missing/unparseable input FAILS
 * CLOSED as outside the window.
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
    return { outsideWindow: true, remainingMs: 0 };
  }
  if (!Number.isFinite(ms)) {
    return { outsideWindow: true, remainingMs: 0 };
  }
  const elapsed = nowMs - ms;
  const remainingMs = Math.max(0, WINDOW_MS - elapsed);
  return { outsideWindow: remainingMs <= 0, remainingMs };
}

/**
 * Outbound gate: inside the window everything is allowed; outside the window
 * a Message Tag MUST be selected.
 */
export function validateOutboundPolicy(
  outsideWindow: boolean,
  selectedTag: string | null
): { allowed: boolean; reason: string | null } {
  if (!outsideWindow) {
    return { allowed: true, reason: null };
  }
  if (!selectedTag) {
    return {
      allowed: false,
      reason: 'Đã quá cửa sổ 24h — vui lòng chọn thẻ tin nhắn CSKH hợp lệ trước khi gửi.'
    };
  }
  return { allowed: true, reason: null };
}

/** Formats remaining milliseconds as "23h 05m" style countdown text. */
export function formatRemainingHm(remainingMs: number): string {
  const safe = Math.max(0, remainingMs);
  const h = Math.floor(safe / (60 * 60 * 1000));
  const m = Math.floor((safe % (60 * 60 * 1000)) / 60_000);
  return `${h}h ${String(m).padStart(2, '0')}m`;
}
