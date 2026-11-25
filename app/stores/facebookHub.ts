import type { BadgeTone } from "../ui/Badge";

/** Managed Facebook Page account as returned by `Facebook.accounts()`. */
export interface FacebookAccount {
  channelAccountId: number;
  pageId: string;
  pageName: string | null;
  avatarUrl: string | null;
  tokenStatus: string;
  grantedScopes: string[];
  runtimeStatus: string;
  lastConnectedAt: string | null;
  lastError: string | null;
  updatedAt: string;
}

export type FacebookPostType =
  | "status"
  | "link"
  | "photo"
  | "multi_photo"
  | "video"
  | "reel";

/** Scheduled / recent Facebook post as returned by `Facebook.scheduledPosts()`. */
export interface FacebookScheduledPost {
  id: number;
  draft_group_id: string;
  channel_account_id: number;
  page_id: string;
  post_type: FacebookPostType | string;
  message: string;
  link_url: string | null;
  media_paths: string[];
  scheduling_mode: "meta" | "local_queued" | string;
  scheduled_publish_time: string | null;
  remote_post_id: string | null;
  status: string;
  error_code: string | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateFacebookPostPayload {
  draft_group_id?: string;
  channel_account_id: number;
  page_id: string;
  post_type: FacebookPostType;
  message: string;
  link_url: string | null;
  media_paths: string[];
  scheduled_publish_time: string | null;
}

export type CommentMatchMode = "contains" | "all" | "exact" | "regex";

/** Comment automation rule as returned by `Facebook.commentAutomations()`. */
export interface CommentAutomationRule {
  id: number;
  channel_account_id: number;
  page_id: string;
  rule_name: string;
  post_ids: string[];
  keywords: string[];
  match_mode: CommentMatchMode | string;
  auto_like: boolean;
  public_reply_template: string | null;
  private_reply_template: string | null;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateCommentAutomationPayload {
  channel_account_id: number;
  page_id: string;
  rule_name: string;
  post_ids: string[];
  keywords: string[];
  match_mode: CommentMatchMode;
  auto_like: boolean;
  public_reply_template: string | null;
  private_reply_template: string | null;
  is_enabled: boolean;
}

export interface UpdateCommentAutomationPayload {
  id: number;
  rule_name: string;
  post_ids: string[];
  keywords: string[];
  match_mode: CommentMatchMode;
  auto_like: boolean;
  public_reply_template: string | null;
  private_reply_template: string | null;
  is_enabled: boolean;
}

/** Audit log entry as returned by `Facebook.commentLogs()`. */
export interface CommentLog {
  id: number;
  comment_id: string;
  page_id: string;
  post_id: string | null;
  sender_psid: string;
  sender_name: string | null;
  comment_text: string | null;
  automation_id: number | null;
  like_status: string;
  public_reply_status: string;
  private_reply_status: string;
  skip_reason: string | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
}

/** Page token and health snapshot as returned by `Facebook.pageHealth(pageId)`. */
export interface PageHealth {
  page_id: string;
  token_status: string;
  granted_scopes: string[];
  active_scheduled_count: number;
  active_automations_count: number;
  updated_at: string;
}

export type FacebookHubTab = "composer" | "automations" | "compliance";
export type PreviewDevice = "desktop" | "mobile";
export type LogStatusFilter = "all" | "success" | "failed" | "skipped";

export const SUB_TABS: { id: FacebookHubTab; label: string }[] = [
  { id: "composer", label: "Đăng bài & Lịch đăng" },
  { id: "automations", label: "Tự động bình luận & Nhật ký" },
  { id: "compliance", label: "Sức khoẻ Page & Tuân thủ" },
];

export const POST_TYPES: { id: FacebookPostType; label: string }[] = [
  { id: "status", label: "Trạng thái" },
  { id: "link", label: "Liên kết" },
  { id: "photo", label: "Hình ảnh" },
  { id: "multi_photo", label: "Nhiều ảnh" },
  { id: "video", label: "Video" },
  { id: "reel", label: "Reel" },
];

export const MATCH_MODES: { id: CommentMatchMode; label: string }[] = [
  { id: "contains", label: "Chứa từ (Bất kỳ)" },
  { id: "all", label: "Chứa tất cả" },
  { id: "exact", label: "Khớp chính xác" },
  { id: "regex", label: "Regex" },
];

export const LOG_FILTERS: { id: LogStatusFilter; label: string }[] = [
  { id: "all", label: "Tất cả" },
  { id: "success", label: "Thành công" },
  { id: "failed", label: "Thất bại" },
  { id: "skipped", label: "Bỏ qua" },
];

/** Splits a comma-separated field into trimmed, deduplicated non-empty tokens. */
export function parseCsvList(raw: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const part of raw.split(",")) {
    const item = part.trim();
    if (item === "" || seen.has(item)) continue;
    seen.add(item);
    out.push(item);
  }
  return out;
}

/**
 * Parses a user-entered date/time string (`YYYY-MM-DD HH:MM`, `YYYY-MM-DDTHH:MM`, or full RFC3339)
 * into an RFC3339 UTC timestamp for `Facebook.schedulePost`, or `null` when invalid.
 */
export function parseScheduledTime(raw: string): string | null {
  const trimmed = raw.trim();
  if (trimmed === "") return null;
  const normalized = trimmed.includes("T") ? trimmed : trimmed.replace(" ", "T");
  const d = new Date(normalized);
  if (Number.isNaN(d.getTime())) return null;
  return d.toISOString();
}

/** Extracts a display hostname from an HTTP(S) URL without relying on DOM URL globals. */
export function extractHostname(rawUrl: string): string {
  const trimmed = rawUrl.trim();
  const m = trimmed.match(/^https?:\/\/([^/?#:]+)/i);
  return m?.[1] ? m[1].toUpperCase() : "EXAMPLE.COM";
}

/** Validates the composer form before calling the native module. */
export function validatePostDraft(draft: {
  account: FacebookAccount | null;
  postType: FacebookPostType;
  message: string;
  linkUrl: string;
  isScheduled: boolean;
  scheduledDateTime: string;
}): string | null {
  if (!draft.account || !draft.account.pageId || !draft.account.channelAccountId) {
    return "Vui lòng kết nối và chọn Fanpage Facebook hợp lệ trước khi đăng bài.";
  }
  if (draft.message.trim() === "") {
    return "Nội dung bài viết không được để trống.";
  }
  if (draft.postType === "link") {
    const link = draft.linkUrl.trim();
    if (link === "") return "Cần nhập URL liên kết cho bài đăng dạng Liên kết.";
    if (!/^https?:\/\/\S+$/i.test(link)) {
      return "URL liên kết phải bắt đầu bằng http:// hoặc https://.";
    }
  }
  if (draft.isScheduled) {
    if (draft.scheduledDateTime.trim() === "") {
      return "Cần nhập ngày giờ hẹn đăng (ví dụ: 2026-12-01 10:30).";
    }
    if (parseScheduledTime(draft.scheduledDateTime) === null) {
      return "Ngày giờ hẹn đăng không hợp lệ. Dùng định dạng YYYY-MM-DD HH:MM hoặc ISO 8601.";
    }
  }
  return null;
}

/** Validates a comment automation rule form before calling the native module. */
export function validateRuleDraft(draft: {
  account: FacebookAccount | null;
  ruleName: string;
  matchMode: CommentMatchMode;
  keywords: string;
}): string | null {
  if (!draft.account || !draft.account.pageId || !draft.account.channelAccountId) {
    return "Vui lòng kết nối và chọn Fanpage Facebook hợp lệ trước khi lưu quy tắc.";
  }
  if (draft.ruleName.trim() === "") {
    return "Cần nhập tên quy tắc.";
  }
  if (draft.matchMode === "regex") {
    for (const pattern of parseCsvList(draft.keywords)) {
      try {
        new RegExp(pattern);
      } catch {
        return `Biểu thức chính quy không hợp lệ: “${pattern}”.`;
      }
    }
  }
  return null;
}

/** Filters comment audit logs by search text and outcome bucket. */
export function filterCommentLogs(
  logs: CommentLog[],
  query: string,
  statusFilter: LogStatusFilter,
): CommentLog[] {
  const q = query.trim().toLowerCase();
  return logs.filter((log) => {
    if (statusFilter === "success") {
      const ok =
        log.like_status === "success" ||
        log.public_reply_status === "success" ||
        log.private_reply_status === "success";
      if (!ok) return false;
    } else if (statusFilter === "failed") {
      const bad =
        log.like_status === "failed" ||
        log.public_reply_status === "failed" ||
        log.private_reply_status === "failed" ||
        (log.error_message !== null && log.error_message.trim() !== "");
      if (!bad) return false;
    } else if (statusFilter === "skipped") {
      const skipped =
        (log.skip_reason !== null && log.skip_reason.trim() !== "") ||
        log.like_status === "skipped" ||
        log.public_reply_status === "skipped" ||
        log.private_reply_status === "skipped";
      if (!skipped) return false;
    }

    if (q === "") return true;
    const haystack = [
      log.comment_id,
      log.sender_name ?? "",
      log.sender_psid,
      log.comment_text ?? "",
      log.post_id ?? "",
      log.skip_reason ?? "",
      log.error_message ?? "",
    ]
      .join(" ")
      .toLowerCase();
    return haystack.includes(q);
  });
}

export function postTypeLabel(postType: string): string {
  return POST_TYPES.find((t) => t.id === postType)?.label ?? postType;
}

export function matchModeLabel(mode: string): string {
  return MATCH_MODES.find((m) => m.id === mode)?.label ?? mode;
}

export function postStatusLabel(status: string): string {
  if (status === "published") return "Đã đăng";
  if (status === "scheduled_on_meta") return "Meta đã lên lịch";
  if (status === "queued") return "Hàng đợi";
  if (status === "publishing") return "Đang đăng";
  if (status === "draft") return "Bản nháp";
  if (status === "failed") return "Thất bại";
  if (status === "overdue_prompt") return "Quá hạn";
  if (status === "delivery_unknown") return "Chưa rõ kết quả";
  return status;
}

export function postStatusTone(status: string): BadgeTone {
  if (status === "published") return "success";
  if (status === "failed" || status === "overdue_prompt" || status === "delivery_unknown") return "danger";
  if (status === "queued" || status === "publishing" || status === "draft") return "warning";
  return "neutral";
}

export function tokenStatusLabel(status: string | undefined): string {
  if (status === "valid") return "Token hoạt động";
  if (status === "expired") return "Token hết hạn";
  if (status === "invalid") return "Token không hợp lệ";
  if (status === "revoked") return "Token bị thu hồi";
  return "Token thiếu / chưa xác minh";
}

export function tokenStatusTone(status: string | undefined): BadgeTone {
  if (status === "valid") return "success";
  if (status === "expired" || status === "invalid" || status === "revoked") return "danger";
  return "warning";
}

export function logStepTone(status: string): BadgeTone {
  if (status === "success") return "success";
  if (status === "failed") return "danger";
  if (status === "pending" || status === "queued") return "warning";
  return "neutral";
}

/** Matches core SQL in `publish_facebook_post_now_impl`. */
export function canPublishNow(status: string): boolean {
  return status === "draft" || status === "failed" || status === "queued" || status === "overdue_prompt";
}

/** Matches core SQL in `cancel_facebook_scheduled_post_impl`. */
export function canCancelPost(status: string): boolean {
  return status === "draft" || status === "queued" || status === "overdue_prompt";
}

const pad = (n: number) => String(n).padStart(2, "0");

/** Formats an ISO timestamp in local time, or `"Ngay lập tức"` when absent. */
export function formatDateTime(iso: string | null | undefined, emptyLabel = "Ngay lập tức"): string {
  if (!iso) return emptyLabel;
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
