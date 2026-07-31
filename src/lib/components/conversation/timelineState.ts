import type {
  Channel,
  ConversationMessage,
  TimelineCursor,
  TimelinePage
} from '$lib/services/conversationService';

export const TIMELINE_PAGE_SIZE = 50;
export const MAX_CACHED_MESSAGES = 200;
export const MAX_CHANNEL_VARIANTS = 4;

export interface TimelineVariant {
  items: ConversationMessage[];
  nextCursor: TimelineCursor | null;
  hasLoaded: boolean;
  loading: boolean;
}

export function channelSetKey(channels: Channel[]): string {
  return [...channels].sort().join('|');
}

function compareMessages(a: ConversationMessage, b: ConversationMessage): number {
  const timeA = new Date(a.occurredAt).getTime();
  const timeB = new Date(b.occurredAt).getTime();
  if (timeA !== timeB) {
    return timeA - timeB;
  }
  return a.id - b.id;
}

export function mergeMessages(
  current: ConversationMessage[],
  incoming: ConversationMessage[]
): ConversationMessage[] {
  const map = new Map<number, ConversationMessage>();
  for (const item of current) {
    map.set(item.id, item);
  }
  for (const item of incoming) {
    map.set(item.id, item);
  }
  const merged = Array.from(map.values());
  merged.sort(compareMessages);
  return merged;
}

export function filterMessages(
  items: ConversationMessage[],
  channels: Channel[]
): ConversationMessage[] {
  if (channels.length === 0) return [];
  const channelSet = new Set(channels);
  return items.filter((item) => channelSet.has(item.channel));
}

export function mergeTimelinePage(
  current: TimelineVariant,
  page: TimelinePage,
  mode: 'refresh' | 'older'
): TimelineVariant {
  const mergedItems = mergeMessages(current.items, page.items);

  let retainedItems: ConversationMessage[];
  if (mergedItems.length > MAX_CACHED_MESSAGES) {
    if (mode === 'refresh') {
      retainedItems = mergedItems.slice(mergedItems.length - MAX_CACHED_MESSAGES);
    } else {
      retainedItems = mergedItems.slice(0, MAX_CACHED_MESSAGES);
    }
  } else {
    retainedItems = mergedItems;
  }

  const nextCursor =
    mode === 'older' ? page.nextCursor : (current.nextCursor ?? page.nextCursor);

  return {
    items: retainedItems,
    nextCursor,
    hasLoaded: true,
    loading: false
  };
}

export function nearestSurvivingAnchor(
  previousItems: ConversationMessage[],
  nextItems: ConversationMessage[],
  anchorMessageId: number | null
): number | null {
  if (anchorMessageId === null || nextItems.length === 0) {
    return null;
  }

  const nextSet = new Set(nextItems.map((item) => item.id));
  if (nextSet.has(anchorMessageId)) {
    return anchorMessageId;
  }

  const prevIndex = previousItems.findIndex((item) => item.id === anchorMessageId);
  if (prevIndex === -1) {
    return nextItems[0]?.id ?? null;
  }

  let step = 1;
  while (prevIndex - step >= 0 || prevIndex + step < previousItems.length) {
    if (prevIndex + step < previousItems.length) {
      const candidate = previousItems[prevIndex + step];
      if (nextSet.has(candidate.id)) {
        return candidate.id;
      }
    }
    if (prevIndex - step >= 0) {
      const candidate = previousItems[prevIndex - step];
      if (nextSet.has(candidate.id)) {
        return candidate.id;
      }
    }
    step++;
  }

  return nextItems[0]?.id ?? null;
}

export class TimelineCache {
  private cache = new Map<string, TimelineVariant>();
  private accessOrder: string[] = [];
  private maxVariants: number;

  constructor(maxVariants = MAX_CHANNEL_VARIANTS) {
    this.maxVariants = maxVariants;
  }

  get(key: string): TimelineVariant | undefined {
    if (!this.cache.has(key)) return undefined;
    this.touch(key);
    return this.cache.get(key);
  }

  set(key: string, variant: TimelineVariant): void {
    if (key !== '') {
      this.touch(key);
    }
    this.cache.set(key, variant);
    this.evict();
  }

  has(key: string): boolean {
    return this.cache.has(key);
  }

  delete(key: string): boolean {
    this.accessOrder = this.accessOrder.filter((k) => k !== key);
    return this.cache.delete(key);
  }

  clear(): void {
    this.cache.clear();
    this.accessOrder = [];
  }

  get size(): number {
    return this.cache.size;
  }

  keys(): IterableIterator<string> {
    return this.cache.keys();
  }

  private touch(key: string): void {
    this.accessOrder = this.accessOrder.filter((k) => k !== key);
    this.accessOrder.push(key);
  }

  private evict(): void {
    while (this.accessOrder.length > this.maxVariants) {
      const lruKey = this.accessOrder.shift();
      if (lruKey !== undefined) {
        this.cache.delete(lruKey);
      }
    }
  }
}
