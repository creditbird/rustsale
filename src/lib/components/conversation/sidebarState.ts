import type { InboxItem } from '$lib/services/conversationService';

export const CUSTOMER_ROW_HEIGHT = 80;
export const GROUP_ROW_HEIGHT = 104;
export const ROW_GAP = 4; // space-y-1 gap
export const PINNED_HEADER_HEIGHT = 32;
export const KIND_HEADER_HEIGHT = 32;
export const DEFAULT_SIDEBAR_OVERSCAN = 5;
export const VIRTUALIZATION_THRESHOLD = 25;

export interface VirtualWindowOptions {
  items: InboxItem[];
  scrollTop: number;
  viewportHeight: number;
  overscan?: number;
  threshold?: number;
}

export interface VirtualWindowResult {
  startIndex: number;
  endIndex: number;
  topPadding: number;
  bottomPadding: number;
  totalHeight: number;
  isVirtualized: boolean;
}

/**
 * Returns exact measured row height including vertical gap for a conversation item.
 */
export function getItemHeight(item: InboxItem): number {
  return (item.kind === 'group' ? GROUP_ROW_HEIGHT : CUSTOMER_ROW_HEIGHT) + ROW_GAP;
}

/**
 * Finds the item index spanning a given pixel offset using binary search on prefix sums.
 */
export function findIndexAtOffset(offsets: number[], targetOffset: number): number {
  if (offsets.length <= 1 || targetOffset <= 0) return 0;
  let low = 0;
  let high = offsets.length - 2; // item index range [0 .. N-1]
  let result = 0;

  while (low <= high) {
    const mid = (low + high) >> 1;
    if (offsets[mid] <= targetOffset) {
      result = mid;
      low = mid + 1;
    } else {
      high = mid - 1;
    }
  }
  return result;
}

/**
 * Calculates total pixel height of the pinned section including header and item gaps.
 */
export function calculatePinnedSectionHeight(pinnedItems: InboxItem[]): number {
  if (!pinnedItems || pinnedItems.length === 0) return 0;
  let total = PINNED_HEADER_HEIGHT;
  for (let i = 0; i < pinnedItems.length; i++) {
    total += getItemHeight(pinnedItems[i]);
  }
  return total;
}

/**
 * Computes exact virtual window indices and padding spacers for high-performance rendering of conversation lists.
 */
export function computeVirtualWindow(options: VirtualWindowOptions): VirtualWindowResult {
  const {
    items,
    scrollTop = 0,
    viewportHeight = 0,
    overscan = DEFAULT_SIDEBAR_OVERSCAN,
    threshold = VIRTUALIZATION_THRESHOLD
  } = options;

  const totalItems = items ? items.length : 0;

  if (totalItems === 0) {
    return {
      startIndex: 0,
      endIndex: 0,
      topPadding: 0,
      bottomPadding: 0,
      totalHeight: 0,
      isVirtualized: false
    };
  }

  // Build prefix sums array
  const offsets = new Array<number>(totalItems + 1);
  offsets[0] = 0;
  for (let i = 0; i < totalItems; i++) {
    offsets[i + 1] = offsets[i] + getItemHeight(items[i]);
  }
  const totalHeight = offsets[totalItems];

  // Fallback: If total items is below threshold, or viewportHeight is 0 (e.g. unmeasured / jsdom test environment), render everything.
  if (totalItems <= threshold || viewportHeight <= 0) {
    return {
      startIndex: 0,
      endIndex: totalItems,
      topPadding: 0,
      bottomPadding: 0,
      totalHeight,
      isVirtualized: false
    };
  }

  // Clamp scrollTop safely within [0, totalHeight]
  const safeScrollTop = Math.max(0, Math.min(scrollTop, Math.max(0, totalHeight - 1)));
  const visibleBottom = safeScrollTop + viewportHeight;

  const rawStart = findIndexAtOffset(offsets, safeScrollTop);
  const rawEnd = findIndexAtOffset(offsets, visibleBottom) + 1;

  let startIndex = Math.max(0, rawStart - overscan);
  let endIndex = Math.min(totalItems, rawEnd + overscan);

  // Safety Clamp: Ensure startIndex never exceeds totalItems - 1 and window is never empty
  if (startIndex >= totalItems) {
    const avgHeight = totalHeight / totalItems;
    const countNeeded = Math.ceil(viewportHeight / avgHeight);
    startIndex = Math.max(0, totalItems - countNeeded);
  }
  startIndex = Math.max(0, Math.min(startIndex, totalItems - 1));
  endIndex = Math.max(startIndex + 1, Math.min(totalItems, endIndex));

  const topPadding = offsets[startIndex];
  const bottomPadding = Math.max(0, totalHeight - offsets[endIndex]);

  return {
    startIndex,
    endIndex,
    topPadding,
    bottomPadding,
    totalHeight,
    isVirtualized: true
  };
}

export function getInitials(title: string): string {
  if (!title) return '?';
  const clean = title.trim();
  if (/^unknown\b/i.test(clean)) {
    return 'UN';
  }
  const sanitized = clean.replace(/[()]/g, '').trim();
  const parts = sanitized.split(/\s+/).filter(Boolean);
  if (parts.length >= 2) {
    return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
  }
  return sanitized.slice(0, 2).toUpperCase();
}

export function getParticipantsPreview(item: InboxItem): string {
  if (!item.participants || item.participants.length === 0) return '';
  return item.participants.map((p) => p.displayName || 'Member').join(', ');
}
