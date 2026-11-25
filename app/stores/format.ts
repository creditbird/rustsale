/** What the conversation and inbox screens need to show a message the way people read it. */

export interface TimelineMessage {
  id: number;
  conversationId?: number;
  channel?: string;
  direction: string;
  contentType?: string;
  textContent: string | null;
  occurredAt: string;
  status: string;
  senderDisplayName?: string | null;
  attachments?: unknown[];
}

const pad = (n: number) => String(n).padStart(2, "0");

/** "HH:MM" in the machine's own time zone (the core stores UTC). */
export function hhmm(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "" : `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

const KIND: [string, string][] = [
  ["sticker", "[Sticker]"],
  ["image", "[Ảnh]"],
  ["photo", "[Ảnh]"],
  ["video", "[Video]"],
  ["voice", "[Tin thoại]"],
  ["audio", "[Tin thoại]"],
  ["file", "[Tệp]"],
  ["link", "[Liên kết]"],
];

/** The text of a bubble: the message text, or a label for a message that has none (sticker, photo...). */
export function bodyOf(m: Pick<TimelineMessage, "textContent" | "contentType" | "attachments">): string {
  const text = m.textContent?.trim() ?? "";
  if (text !== "") return m.textContent ?? "";
  const kind = (m.contentType ?? "").toLowerCase();
  for (const [needle, label] of KIND) if (kind.includes(needle)) return label;
  if ((m.attachments?.length ?? 0) > 0) return "[Tệp đính kèm]";
  return kind === "" || kind === "text" ? "[Tin trống]" : "[Tin nhắn]";
}

/** Oldest first; the id breaks a tie so the order never flips between loads. */
export function byTime(a: TimelineMessage, b: TimelineMessage): number {
  if (a.occurredAt !== b.occurredAt) return a.occurredAt < b.occurredAt ? -1 : 1;
  return a.id - b.id;
}

/**
 * Puts `incoming` into `current` (a message already there is replaced by its newer copy) and returns
 * the whole list oldest first: the newest message ends at the bottom, an older page lands above.
 */
export function mergeMessages<T extends TimelineMessage>(current: T[], incoming: T[]): T[] {
  const seen = new Map<number, T>();
  for (const m of current) seen.set(m.id, m);
  for (const m of incoming) seen.set(m.id, m);
  return Array.from(seen.values()).sort(byTime);
}
