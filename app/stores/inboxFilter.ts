/** Pure helpers of the inbox filter and the new-chat form: no native calls, so they test without a core. */

export type InboxTab = "all" | "unread" | "customer" | "group" | "unknown";
export type InboxChannel = "all" | "zalo" | "whatsapp" | "messenger" | "email";

export const TABS: { id: InboxTab; label: string }[] = [
  { id: "all", label: "Tất cả" },
  { id: "unread", label: "Chưa đọc" },
  { id: "customer", label: "Khách" },
  { id: "group", label: "Nhóm" },
  { id: "unknown", label: "Người lạ" },
];

export const CHANNELS: { id: InboxChannel; label: string }[] = [
  { id: "all", label: "Mọi kênh" },
  { id: "zalo", label: "Zalo" },
  { id: "whatsapp", label: "WhatsApp" },
  { id: "messenger", label: "Messenger" },
  { id: "email", label: "Email" },
];

/** The `InboxQuery` the core takes (`kind`, `channel`, `unreadOnly` and `search` are its own filters). */
export interface InboxQuery {
  sort: "recent";
  search?: string;
  kind?: "customer" | "group" | "unknown";
  channel?: Exclude<InboxChannel, "all">;
  unreadOnly?: boolean;
}

export function inboxQuery(tab: InboxTab, channel: InboxChannel, search: string): InboxQuery {
  const query: InboxQuery = { sort: "recent" };
  const text = search.trim();
  if (text !== "") query.search = text;
  if (tab === "unread") query.unreadOnly = true;
  else if (tab !== "all") query.kind = tab;
  if (channel !== "all") query.channel = channel;
  return query;
}

/** Whether any filter narrows the inbox (so an empty list reads "nothing matches", not "inbox is empty"). */
export const isFiltered = (tab: InboxTab, channel: InboxChannel, search: string): boolean =>
  tab !== "all" || channel !== "all" || search.trim() !== "";

/** An account `ConversationNetwork.startDirect` can open a chat from (`Conversations.directChatAccounts`). */
export interface DirectChatAccount {
  id: number;
  channel: string;
  displayName: string | null;
  connectionStatus: string;
}

export const accountLabel = (a: DirectChatAccount): string =>
  `${a.channel === "whatsapp" ? "WhatsApp" : "Zalo"} · ${a.displayName ?? `tài khoản ${a.id}`}`;

export interface DirectChatRequest {
  channelAccountId: number;
  phoneOrExternalId: string;
  displayName?: string;
  initialMessage: string;
}

export type Parsed<T> = { ok: true; value: T } | { ok: false; error: string };

/** The new-chat form as the core's request, or the sentence that says what to fill in. */
export function parseDirectChat(
  account: DirectChatAccount | null,
  target: string,
  name: string,
  message: string,
): Parsed<DirectChatRequest> {
  if (account === null) return { ok: false, error: "Chọn tài khoản để gửi." };
  const to = target.trim();
  if (to === "") return { ok: false, error: "Nhập số điện thoại hoặc mã người nhận." };
  const text = message.trim();
  if (text === "") return { ok: false, error: "Nhập tin nhắn đầu tiên." };
  const value: DirectChatRequest = { channelAccountId: account.id, phoneOrExternalId: to, initialMessage: text };
  const label = name.trim();
  if (label !== "") value.displayName = label;
  return { ok: true, value };
}
