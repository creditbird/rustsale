/// <reference types="bun-types" />
import { describe, expect, it } from 'bun:test';
import type { InboxItem } from '$lib/services/conversationService';
import {
  computeVirtualWindow,
  getItemHeight,
  findIndexAtOffset,
  calculatePinnedSectionHeight,
  CUSTOMER_ROW_HEIGHT,
  GROUP_ROW_HEIGHT,
  ROW_GAP,
  PINNED_HEADER_HEIGHT,
  VIRTUALIZATION_THRESHOLD
} from '../sidebarState';

function createMockItem(idx: number, kind: 'customer' | 'group' | 'unknown' = 'customer'): InboxItem {
  return {
    key: `${kind}-${idx}`,
    kind,
    leadId: idx,
    conversationId: idx,
    identityId: 1000 + idx,
    title: `Item ${idx}`,
    avatarUrl: null,
    lastMessagePreview: `Message ${idx}`,
    lastMessageAt: new Date(2026, 7, 1, 10, idx).toISOString(),
    channels: [{ channel: 'zalo', messageCount: 1 }],
    replyTargets: [],
    participantCount: kind === 'group' ? 5 : 1,
    participants: []
  };
}

describe('sidebarState - getItemHeight', () => {
  it('returns exactly 84px for customer and unknown items', () => {
    const customer = createMockItem(1, 'customer');
    const unknown = createMockItem(2, 'unknown');
    expect(getItemHeight(customer)).toBe(CUSTOMER_ROW_HEIGHT + ROW_GAP);
    expect(getItemHeight(unknown)).toBe(CUSTOMER_ROW_HEIGHT + ROW_GAP);
    expect(getItemHeight(customer)).toBe(84);
  });

  it('returns exactly 108px for group items', () => {
    const group = createMockItem(1, 'group');
    expect(getItemHeight(group)).toBe(GROUP_ROW_HEIGHT + ROW_GAP);
    expect(getItemHeight(group)).toBe(108);
  });
});

describe('sidebarState - calculatePinnedSectionHeight', () => {
  it('returns 0 when pinnedItems is empty', () => {
    expect(calculatePinnedSectionHeight([])).toBe(0);
  });

  it('calculates total pinned section height including header and variable items', () => {
    const pinned = [
      createMockItem(1, 'customer'), // 84
      createMockItem(2, 'group')     // 108
    ];
    // PINNED_HEADER_HEIGHT (32) + 84 + 108 = 224
    expect(calculatePinnedSectionHeight(pinned)).toBe(PINNED_HEADER_HEIGHT + 84 + 108);
    expect(calculatePinnedSectionHeight(pinned)).toBe(224);
  });
});

describe('sidebarState - findIndexAtOffset', () => {
  it('finds correct index in cumulative offsets', () => {
    const offsets = [0, 84, 168, 276, 360]; // items: 84, 84, 108, 84
    expect(findIndexAtOffset(offsets, 0)).toBe(0);
    expect(findIndexAtOffset(offsets, 50)).toBe(0);
    expect(findIndexAtOffset(offsets, 84)).toBe(1);
    expect(findIndexAtOffset(offsets, 100)).toBe(1);
    expect(findIndexAtOffset(offsets, 200)).toBe(2);
    expect(findIndexAtOffset(offsets, 300)).toBe(3);
  });
});

describe('sidebarState - computeVirtualWindow', () => {
  it('returns empty result when items is empty', () => {
    const result = computeVirtualWindow({
      items: [],
      scrollTop: 100,
      viewportHeight: 500
    });

    expect(result.startIndex).toBe(0);
    expect(result.endIndex).toBe(0);
    expect(result.totalHeight).toBe(0);
    expect(result.isVirtualized).toBe(false);
  });

  it('returns unvirtualized full range when items.length <= VIRTUALIZATION_THRESHOLD', () => {
    const items = Array.from({ length: 20 }, (_, i) => createMockItem(i, 'customer'));
    const result = computeVirtualWindow({
      items,
      scrollTop: 100,
      viewportHeight: 500
    });

    expect(result.isVirtualized).toBe(false);
    expect(result.startIndex).toBe(0);
    expect(result.endIndex).toBe(20);
    expect(result.topPadding).toBe(0);
    expect(result.bottomPadding).toBe(0);
    expect(result.totalHeight).toBe(20 * 84);
  });

  it('returns unvirtualized full range when viewportHeight is 0 (test / unmeasured fallback)', () => {
    const items = Array.from({ length: 100 }, (_, i) => createMockItem(i, 'customer'));
    const result = computeVirtualWindow({
      items,
      scrollTop: 0,
      viewportHeight: 0
    });

    expect(result.isVirtualized).toBe(false);
    expect(result.startIndex).toBe(0);
    expect(result.endIndex).toBe(100);
    expect(result.topPadding).toBe(0);
    expect(result.bottomPadding).toBe(0);
  });

  it('computes virtual window correctly with mixed customer and group items', () => {
    // 50 items: even are customer (84px), odd are group (108px)
    const items = Array.from({ length: 50 }, (_, i) =>
      createMockItem(i, i % 2 === 0 ? 'customer' : 'group')
    );

    // ScrollTop = 960 (approx 10 items in), viewportHeight = 600, overscan = 3
    const result = computeVirtualWindow({
      items,
      scrollTop: 960,
      viewportHeight: 600,
      overscan: 3
    });

    expect(result.isVirtualized).toBe(true);
    expect(result.startIndex).toBeGreaterThan(0);
    expect(result.endIndex).toBeLessThanOrEqual(50);
    expect(result.startIndex).toBeLessThan(result.endIndex);
    expect(result.topPadding).toBeGreaterThan(0);
    expect(result.bottomPadding).toBeGreaterThan(0);
    expect(result.topPadding + result.bottomPadding).toBeLessThan(result.totalHeight);
  });

  it('safely clamps range when scrollTop is far past totalHeight (deep scroll then filtered)', () => {
    const items = Array.from({ length: 40 }, (_, i) => createMockItem(i, 'customer'));
    // User was at scrollTop = 6000, then search shrunk items to 40 (totalHeight = 3360)
    const result = computeVirtualWindow({
      items,
      scrollTop: 6000,
      viewportHeight: 600,
      overscan: 3
    });

    expect(result.isVirtualized).toBe(true);
    expect(result.startIndex).toBeLessThan(40);
    expect(result.endIndex).toBe(40);
    expect(result.startIndex).toBeLessThan(result.endIndex);
    expect(result.endIndex - result.startIndex).toBeGreaterThanOrEqual(1);
  });

  it('handles negative scrollTop without error', () => {
    const items = Array.from({ length: 50 }, (_, i) => createMockItem(i, 'customer'));
    const result = computeVirtualWindow({
      items,
      scrollTop: -200,
      viewportHeight: 500
    });

    expect(result.isVirtualized).toBe(true);
    expect(result.startIndex).toBe(0);
    expect(result.topPadding).toBe(0);
  });
});
