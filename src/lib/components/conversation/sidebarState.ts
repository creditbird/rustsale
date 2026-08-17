import type { InboxItem } from '$lib/services/conversationService';

export const DEFAULT_SIDEBAR_ITEM_HEIGHT = 72;
export const DEFAULT_SIDEBAR_OVERSCAN = 6;
export const VIRTUALIZATION_THRESHOLD = 30;

export interface VirtualWindowOptions {
  totalItems: number;
  itemHeight?: number;
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
 * Computes the virtual window indices and padding spacers for high-performance rendering of large conversation lists.
 */
export function computeVirtualWindow(options: VirtualWindowOptions): VirtualWindowResult {
  const {
    totalItems,
    itemHeight = DEFAULT_SIDEBAR_ITEM_HEIGHT,
    scrollTop = 0,
    viewportHeight = 0,
    overscan = DEFAULT_SIDEBAR_OVERSCAN,
    threshold = VIRTUALIZATION_THRESHOLD
  } = options;

  const totalHeight = totalItems * itemHeight;

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

  const safeScrollTop = Math.max(0, scrollTop);
  const rawStart = Math.floor(safeScrollTop / itemHeight);
  const rawEnd = Math.ceil((safeScrollTop + viewportHeight) / itemHeight);

  const startIndex = Math.max(0, rawStart - overscan);
  const endIndex = Math.min(totalItems, rawEnd + overscan);

  const topPadding = startIndex * itemHeight;
  const bottomPadding = Math.max(0, (totalItems - endIndex) * itemHeight);

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
