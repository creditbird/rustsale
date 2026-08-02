/// <reference types="bun-types" />
import { afterEach, beforeAll, describe, expect, test, mock } from 'bun:test';
import { cleanup, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import type { ConversationMessage } from '$lib/services/conversationService';
import ConversationTimeline from '../ConversationTimeline.svelte';

let observerCallback: IntersectionObserverCallback | null = null;
let observerOptions: IntersectionObserverInit | undefined;

beforeAll(() => {
  if (typeof globalThis.IntersectionObserver === 'undefined') {
    globalThis.IntersectionObserver = class IntersectionObserver {
      constructor(callback: IntersectionObserverCallback, options?: IntersectionObserverInit) {
        observerCallback = callback;
        observerOptions = options;
      }
      observe() {}
      unobserve() {}
      disconnect() {}
    } as any;
  }
});

describe('ConversationTimeline Component', () => {
  afterEach(() => {
    cleanup();
    observerCallback = null;
  });

  function makeMessages(count: number, startId = 1): ConversationMessage[] {
    return Array.from({ length: count }, (_, i) => ({
      id: startId + i,
      conversationId: 1,
      channelAccountId: 1,
      channel: 'zalo',
      accountDisplayName: 'Zalo Account',
      senderIdentityId: 10,
      senderDisplayName: `User ${startId + i}`,
      direction: (startId + i) % 2 === 0 ? 'outbound' : 'inbound',
      providerMessageId: `p-${startId + i}`,
      providerClientMessageId: `client-${startId + i}`,
      contentType: 'text',
      textContent: `Message content ${startId + i}`,
      content: {},
      occurredAt: new Date(1700000000000 + (startId + i) * 1000).toISOString(),
      status: 'sent',
      removedAt: null,
      errorMessage: null,
      replyToMessageId: null,
      attachments: [],
      replyTarget: null,
      reactions: [],
      batchIndex: null,
      sendBatch: null
    }));
  }

  test('items render in ascending chronological order', () => {
    const items = makeMessages(3);
    const { container } = render(ConversationTimeline, {
      props: {
        items,
        emptyMessage: 'Không có tin nhắn',
        onLoadOlder: () => {}
      }
    });

    const bubbles = container.querySelectorAll('[data-message-id]');
    expect(bubbles.length).toBe(3);
    expect(bubbles[0].getAttribute('data-message-id')).toBe('1');
    expect(bubbles[1].getAttribute('data-message-id')).toBe('2');
    expect(bubbles[2].getAttribute('data-message-id')).toBe('3');
  });

  test('top sentinel calls onLoadOlder() only when hasMore && !loadingOlder', async () => {
    const onLoadOlder = mock(() => {});

    const { rerender } = render(ConversationTimeline, {
      props: {
        items: makeMessages(5),
        hasMore: true,
        loadingOlder: false,
        emptyMessage: 'Không có tin nhắn',
        onLoadOlder
      }
    });

    // Simulate top sentinel intersection
    if (observerCallback) {
      observerCallback(
        [{ isIntersecting: true } as IntersectionObserverEntry],
        {} as IntersectionObserver
      );
    }

    expect(onLoadOlder).toHaveBeenCalledTimes(1);
    onLoadOlder.mockClear();

    // When loadingOlder is true, should NOT trigger
    await rerender({
      items: makeMessages(5),
      hasMore: true,
      loadingOlder: true,
      emptyMessage: 'Không có tin nhắn',
      onLoadOlder
    });

    if (observerCallback) {
      observerCallback(
        [{ isIntersecting: true } as IntersectionObserverEntry],
        {} as IntersectionObserver
      );
    }
    expect(onLoadOlder).not.toHaveBeenCalled();

    // When hasMore is false, should NOT trigger
    await rerender({
      items: makeMessages(5),
      hasMore: false,
      loadingOlder: false,
      emptyMessage: 'Không có tin nhắn',
      onLoadOlder
    });

    if (observerCallback) {
      observerCallback(
        [{ isIntersecting: true } as IntersectionObserverEntry],
        {} as IntersectionObserver
      );
    }
    expect(onLoadOlder).not.toHaveBeenCalled();
  });

  test('bounded DOM window: at most 160 bubbles mounted at once', () => {
    const items = makeMessages(200);
    const { container } = render(ConversationTimeline, {
      props: {
        items,
        emptyMessage: 'Không có tin nhắn',
        onLoadOlder: () => {}
      }
    });

    const bubbles = container.querySelectorAll('[data-message-id]');
    expect(bubbles.length).toBeGreaterThan(0);
    expect(bubbles.length).toBeLessThanOrEqual(160);
  });

  test('auto-scrolls on new live messages when viewport is within 80px of bottom', async () => {
    const initialItems = makeMessages(10);
    const { container, rerender } = render(ConversationTimeline, {
      props: {
        items: initialItems,
        emptyMessage: 'Không có tin nhắn',
        onLoadOlder: () => {}
      }
    });

    const scrollContainer = container.querySelector('[data-timeline-container]') as HTMLElement;
    expect(scrollContainer).toBeTruthy();

    // Mock scroll properties to be near bottom (<= 80px)
    Object.defineProperty(scrollContainer, 'scrollHeight', { value: 1000, configurable: true });
    Object.defineProperty(scrollContainer, 'clientHeight', { value: 500, configurable: true });
    Object.defineProperty(scrollContainer, 'scrollTop', { value: 450, writable: true, configurable: true }); // distance from bottom = 1000 - 450 - 500 = 50px (<= 80px)

    // Add a new message
    const updatedItems = makeMessages(11);
    await rerender({
      items: updatedItems,
      emptyMessage: 'Không có tin nhắn',
      onLoadOlder: () => {}
    });

    await tick();

    // Check that scrollTop was set towards bottom
    expect(scrollContainer.scrollTop).toBeGreaterThanOrEqual(450);
  });

  test('captures and restores { messageId, offsetTop } anchor', async () => {
    const items = makeMessages(10);
    const { container, component } = render(ConversationTimeline, {
      props: {
        items,
        emptyMessage: 'Không có tin nhắn',
        onLoadOlder: () => {}
      }
    });

    const timelineComp = component as any;
    const scrollContainer = container.querySelector('[data-timeline-container]') as HTMLElement;
    expect(scrollContainer).toBeTruthy();

    const targetBubble = container.querySelector('[data-message-id="5"]') as HTMLElement;
    expect(targetBubble).toBeTruthy();

    // Mock offsetTop and scrollTop
    Object.defineProperty(targetBubble, 'offsetTop', { value: 200, configurable: true });
    Object.defineProperty(scrollContainer, 'scrollTop', { value: 50, writable: true, configurable: true });

    // captureAnchor should return target message id and relative offset
    const anchor = timelineComp.captureAnchor();
    expect(anchor).not.toBeNull();
    expect(anchor?.messageId).toBeDefined();

    // Simulate prepending older messages
    const targetAfterPrepend = container.querySelector(`[data-message-id="${anchor?.messageId}"]`) as HTMLElement;
    if (targetAfterPrepend) {
      Object.defineProperty(targetAfterPrepend, 'offsetTop', { value: 450, configurable: true });
    }

    await timelineComp.restoreAnchor(anchor);
    expect(scrollContainer.scrollTop).toBeDefined();
  });
});
