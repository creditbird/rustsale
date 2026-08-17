/// <reference types="bun-types" />
import { describe, expect, it } from 'bun:test';
import {
  computeVirtualWindow,
  DEFAULT_SIDEBAR_ITEM_HEIGHT,
  DEFAULT_SIDEBAR_OVERSCAN,
  VIRTUALIZATION_THRESHOLD
} from '../sidebarState';

describe('sidebarState - computeVirtualWindow', () => {
  it('returns unvirtualized full range when totalItems <= VIRTUALIZATION_THRESHOLD', () => {
    const result = computeVirtualWindow({
      totalItems: 25,
      scrollTop: 100,
      viewportHeight: 500
    });

    expect(result.isVirtualized).toBe(false);
    expect(result.startIndex).toBe(0);
    expect(result.endIndex).toBe(25);
    expect(result.topPadding).toBe(0);
    expect(result.bottomPadding).toBe(0);
    expect(result.totalHeight).toBe(25 * DEFAULT_SIDEBAR_ITEM_HEIGHT);
  });

  it('returns unvirtualized full range when viewportHeight is 0 (test / unmeasured fallback)', () => {
    const result = computeVirtualWindow({
      totalItems: 500,
      scrollTop: 0,
      viewportHeight: 0
    });

    expect(result.isVirtualized).toBe(false);
    expect(result.startIndex).toBe(0);
    expect(result.endIndex).toBe(500);
    expect(result.topPadding).toBe(0);
    expect(result.bottomPadding).toBe(0);
    expect(result.totalHeight).toBe(500 * DEFAULT_SIDEBAR_ITEM_HEIGHT);
  });

  it('calculates virtual slice correctly at scrollTop = 0 with overscan', () => {
    // 200 items, itemHeight = 72, viewportHeight = 720 (10 visible items)
    // rawStart = 0, rawEnd = 10, overscan = 6 -> startIndex = 0, endIndex = 16
    const result = computeVirtualWindow({
      totalItems: 200,
      itemHeight: 72,
      scrollTop: 0,
      viewportHeight: 720,
      overscan: 6
    });

    expect(result.isVirtualized).toBe(true);
    expect(result.startIndex).toBe(0);
    expect(result.endIndex).toBe(16);
    expect(result.topPadding).toBe(0);
    expect(result.bottomPadding).toBe((200 - 16) * 72);
    expect(result.totalHeight).toBe(200 * 72);
  });

  it('calculates virtual slice correctly when scrolled to middle', () => {
    // scrollTop = 720 (10th item), viewportHeight = 720 (10 items)
    // rawStart = 10, rawEnd = 20, overscan = 5
    // startIndex = 10 - 5 = 5, endIndex = 20 + 5 = 25
    const result = computeVirtualWindow({
      totalItems: 100,
      itemHeight: 72,
      scrollTop: 720,
      viewportHeight: 720,
      overscan: 5
    });

    expect(result.isVirtualized).toBe(true);
    expect(result.startIndex).toBe(5);
    expect(result.endIndex).toBe(25);
    expect(result.topPadding).toBe(5 * 72);
    expect(result.bottomPadding).toBe((100 - 25) * 72);
    expect(result.totalHeight).toBe(100 * 72);
  });

  it('bounds endIndex to totalItems when scrolled near bottom', () => {
    // 50 items, scrolled to bottom (scrollTop = 3000)
    const result = computeVirtualWindow({
      totalItems: 50,
      itemHeight: 72,
      scrollTop: 3000,
      viewportHeight: 600,
      overscan: 6
    });

    expect(result.isVirtualized).toBe(true);
    expect(result.endIndex).toBe(50);
    expect(result.bottomPadding).toBe(0);
    expect(result.startIndex).toBeLessThan(50);
    expect(result.topPadding).toBe(result.startIndex * 72);
  });

  it('handles negative scrollTop gracefully', () => {
    const result = computeVirtualWindow({
      totalItems: 100,
      itemHeight: 72,
      scrollTop: -50,
      viewportHeight: 500
    });

    expect(result.isVirtualized).toBe(true);
    expect(result.startIndex).toBe(0);
    expect(result.topPadding).toBe(0);
  });
});
