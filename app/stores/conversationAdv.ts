import { bodyOf, type TimelineMessage } from "./format";

export interface ChannelBadgeSummary {
  channel: string;
  messageCount: number;
  unreadCount: number;
}

export interface InboxReplyTarget {
  conversationId: number;
  channelAccountId: number;
  channel: string;
  accountDisplayName?: string | null;
  conversationKind: string;
  lastInboundAt?: string | null;
  lastMessageAt?: string | null;
}

export interface ConversationParticipantSummary {
  identityId: number;
  externalId?: string | null;
  displayName?: string | null;
  avatarUrl?: string | null;
  participantRole: string;
}

export interface InboxItem {
  key: string;
  kind?: "customer" | "unknown" | "group";
  leadId?: number | null;
  conversationId?: number | null;
  identityId?: number | null;
  title: string;
  avatarUrl?: string | null;
  lastMessagePreview: string | null;
  lastMessageAt: string | null;
  unreadCount: number;
  channels?: ChannelBadgeSummary[];
  replyTargets: InboxReplyTarget[];
  participantCount?: number;
  participants?: ConversationParticipantSummary[];
}

export interface PinnedMessage {
  id: number;
  conversationId: number;
  messageId: number;
  pinnedBy: string | null;
  pinnedAt: string;
  messageText: string | null;
  senderName: string | null;
}

export interface QuickTemplate {
  id: number;
  shortcut: string;
  title: string;
  bodyText: string;
  category: string | null;
  isActive: boolean;
  usageCount: number;
  lastUsedAt: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface StagedAttachment {
  stagedAttachmentId: string;
  mediaType: string;
  mimeType: string;
  fileName: string;
  fileSizeBytes: number;
  width?: number | null;
  height?: number | null;
  durationSeconds?: number | null;
  expiresAt: string;
}

export interface ComposerAttachmentItem {
  localId: string;
  stagedAttachmentId: string | null;
  fileName: string;
  mediaType: string;
  mimeType: string;
  sizeBytes: number;
  status: "staging" | "ready" | "removing" | "error";
  errorMessage?: string | null;
}

// ---- Lead linking helpers (UI-47) ----

export function resolveIdentityId(item: InboxItem): number | null {
  if (typeof item.identityId === "number" && item.identityId > 0) return item.identityId;
  const first = item.participants?.[0]?.identityId;
  return typeof first === "number" && first > 0 ? first : null;
}

export function resolveInitialPhone(item: InboxItem): string {
  const ext = item.participants?.[0]?.externalId?.trim() ?? "";
  return /^[+0-9][0-9\s-]{6,18}$/.test(ext) ? ext : "";
}

export function validateNewLeadForm(name: string): string | null {
  return name.trim() === "" ? "Cần nhập tên khách hàng." : null;
}

export function itemConversationIds(item: InboxItem): number[] {
  const ids = new Set<number>();
  if (typeof item.conversationId === "number" && item.conversationId > 0) ids.add(item.conversationId);
  for (const t of item.replyTargets) {
    if (typeof t.conversationId === "number" && t.conversationId > 0) ids.add(t.conversationId);
  }
  return Array.from(ids);
}

export function mergePinnedMessages(lists: PinnedMessage[][]): PinnedMessage[] {
  const seen = new Map<number, PinnedMessage>();
  for (const list of lists) {
    for (const p of list) seen.set(p.messageId, p);
  }
  return Array.from(seen.values()).sort((a, b) => {
    if (a.pinnedAt !== b.pinnedAt) return b.pinnedAt.localeCompare(a.pinnedAt);
    return b.id - a.id;
  });
}

// ---- In-thread search helpers (UI-48) ----

export function findThreadMatches(messages: TimelineMessage[], query: string): number[] {
  const needle = query.trim().toLowerCase();
  if (needle === "") return [];
  const matches: number[] = [];
  for (const m of messages) {
    const body = bodyOf(m).toLowerCase();
    const raw = (m.textContent ?? "").toLowerCase();
    if (body.includes(needle) || raw.includes(needle)) matches.push(m.id);
  }
  return matches;
}

export function stepMatchIndex(current: number, delta: number, total: number): number {
  if (total <= 0) return 0;
  return ((current + delta) % total + total) % total;
}

// ---- Quick templates helpers (UI-48) ----

export const MAX_QUICK_TEMPLATES = 500;

export interface SlashToken {
  start: number;
  end: number;
  query: string;
}

export function findSlashToken(value: string, caret: number): SlashToken | null {
  if (caret < 0 || caret > value.length) return null;
  for (let i = caret - 1; i >= 0; i -= 1) {
    const ch = value[i];
    if (/\s/.test(ch)) return null;
    if (ch === "/") {
      const boundary = i === 0 || /\s/.test(value[i - 1]);
      return boundary ? { start: i, end: caret, query: value.slice(i + 1, caret) } : null;
    }
  }
  return null;
}

export function replaceSlashToken(
  value: string,
  token: SlashToken,
  body: string,
): { value: string; caret: number } {
  const before = value.slice(0, token.start);
  const after = value.slice(token.end);
  return { value: before + body + after, caret: token.start + body.length };
}

export function rankQuickTemplates(items: QuickTemplate[], query: string): QuickTemplate[] {
  const active = items.filter((item) => item.isActive);
  const q = query.trim().toLowerCase();
  if (q === "") {
    return [...active].sort((a, b) =>
      b.usageCount !== a.usageCount ? b.usageCount - a.usageCount : a.shortcut.localeCompare(b.shortcut),
    );
  }
  const ranked: { item: QuickTemplate; tier: number }[] = [];
  for (const item of active) {
    const shortcut = item.shortcut.toLowerCase();
    const title = item.title.toLowerCase();
    const category = item.category?.toLowerCase() ?? "";
    const body = item.bodyText.toLowerCase();
    let tier = 0;
    if (shortcut === q) tier = 1;
    else if (shortcut.startsWith(q)) tier = 2;
    else if (shortcut.includes(q)) tier = 3;
    else if (title.includes(q) || category.includes(q) || body.includes(q)) tier = 4;
    if (tier > 0) ranked.push({ item, tier });
  }
  ranked.sort((a, b) => {
    if (a.tier !== b.tier) return a.tier - b.tier;
    if (b.item.usageCount !== a.item.usageCount) return b.item.usageCount - a.item.usageCount;
    return a.item.shortcut.localeCompare(b.item.shortcut);
  });
  return ranked.map((entry) => entry.item);
}

export interface QuickTemplateDraft {
  shortcut: string;
  title: string;
  bodyText: string;
  category: string;
  isActive: boolean;
}

export type QuickTemplateValidation =
  | { ok: true; value: { shortcut: string; title: string; bodyText: string; category: string | null; isActive: boolean } }
  | { ok: false; error: string };

export function validateQuickTemplate(
  draft: QuickTemplateDraft,
  existing: QuickTemplate[],
  editingId: number | null,
): QuickTemplateValidation {
  let shortcut = draft.shortcut.trim();
  if (shortcut.startsWith("/")) shortcut = shortcut.slice(1).trim();
  if (shortcut === "") return { ok: false, error: "Cần nhập phím tắt (ví dụ: baogia)." };
  if (/\s/.test(shortcut)) return { ok: false, error: "Phím tắt không được chứa khoảng trắng." };
  const title = draft.title.trim();
  if (title === "") return { ok: false, error: "Cần nhập tiêu đề mẫu." };
  const bodyText = draft.bodyText.trim();
  if (bodyText === "") return { ok: false, error: "Cần nhập nội dung mẫu." };
  const dup = existing.some(
    (t) => t.shortcut.toLowerCase() === shortcut.toLowerCase() && (editingId === null || t.id !== editingId),
  );
  if (dup) return { ok: false, error: `Phím tắt "/${shortcut}" đã tồn tại.` };
  const category = draft.category.trim();
  return {
    ok: true,
    value: { shortcut, title, bodyText, category: category === "" ? null : category, isActive: draft.isActive },
  };
}

// ---- Facebook 24-Hour Policy Gate (UI-48) ----

export const WINDOW_MS = 24 * 60 * 60 * 1000;
export const HUMAN_AGENT_WINDOW_MS = 7 * WINDOW_MS;
const CLOCK_SKEW_GUARD_MS = 5 * 60 * 1000;

export const FACEBOOK_MESSAGE_TAGS = [
  { value: "HUMAN_AGENT", label: "HUMAN_AGENT (CSKH trong 7 ngày)" },
] as const;

export type FacebookMessageTag = (typeof FACEBOOK_MESSAGE_TAGS)[number]["value"];

export interface FacebookWindowStatus {
  freeWindow: boolean;
  humanAgentEligible: boolean;
  remainingMs: number;
}

export function evaluateFacebookWindow(
  lastInboundAt: number | string | null | undefined,
  nowMs: number,
): FacebookWindowStatus {
  let ms: number;
  if (typeof lastInboundAt === "number") {
    ms = lastInboundAt;
  } else if (typeof lastInboundAt === "string" && lastInboundAt.trim() !== "") {
    ms = Date.parse(lastInboundAt);
  } else {
    return { freeWindow: false, humanAgentEligible: false, remainingMs: 0 };
  }
  if (!Number.isFinite(ms)) return { freeWindow: false, humanAgentEligible: false, remainingMs: 0 };
  if (ms - nowMs > CLOCK_SKEW_GUARD_MS) {
    return { freeWindow: false, humanAgentEligible: false, remainingMs: 0 };
  }
  const elapsed = nowMs - ms;
  const remainingMs = Math.max(0, WINDOW_MS - elapsed);
  return {
    freeWindow: elapsed < WINDOW_MS,
    humanAgentEligible: elapsed >= 0 && elapsed < HUMAN_AGENT_WINDOW_MS,
    remainingMs,
  };
}

export function validateFacebookOutbound(
  status: FacebookWindowStatus,
  selectedTag: string | null,
): { allowed: boolean; reason: string | null } {
  if (status.freeWindow) return { allowed: true, reason: null };
  if (!status.humanAgentEligible) {
    return {
      allowed: false,
      reason: "Đã quá cửa sổ 7 ngày của Facebook Messenger. Chỉ gửi được khi khách nhắn tin lại.",
    };
  }
  if (selectedTag === "HUMAN_AGENT") return { allowed: true, reason: null };
  return {
    allowed: false,
    reason: "Đã quá 24 giờ kể từ tin nhắn cuối của khách. Chọn thẻ HUMAN_AGENT để tiếp tục gửi.",
  };
}

export function formatRemainingHm(remainingMs: number): string {
  const safe = Math.max(0, remainingMs);
  const h = Math.floor(safe / (60 * 60 * 1000));
  const m = Math.floor((safe % (60 * 60 * 1000)) / 60_000);
  return `${h}h ${String(m).padStart(2, "0")}m`;
}

export function resolveFacebookLastInboundAt(
  target: InboxReplyTarget | null,
  messages: TimelineMessage[],
): string | null {
  if (!target || target.channel !== "messenger") return null;
  if (target.lastInboundAt && target.lastInboundAt.trim() !== "") return target.lastInboundAt;
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const m = messages[i];
    if (m.direction === "inbound" && (m.channel === undefined || m.channel === "messenger")) {
      return m.occurredAt;
    }
  }
  return null;
}

// ---- Staged attachment queue helpers (UI-48) ----

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  if (bytes < 1024) return `${Math.round(bytes)} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function attachmentTypeLabel(mediaType: string): string {
  const t = mediaType.toLowerCase();
  if (t === "image") return "Ảnh";
  if (t === "video") return "Video";
  if (t === "audio" || t === "voice") return "Âm thanh";
  return "Tệp";
}

export function parseAttachmentPaths(raw: string): string[] {
  return raw
    .split(/[\r\n,]+/)
    .map((p) => p.trim())
    .filter((p) => p.length > 0);
}
