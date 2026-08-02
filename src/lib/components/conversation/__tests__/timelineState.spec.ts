import { describe, expect, it } from 'bun:test';
import type { Channel, ConversationMessage } from '$lib/services/conversationService';
import {
  channelSetKey,
  filterMessages,
  mergeMessages,
  mergeTimelinePage,
  nearestSurvivingAnchor,
  TimelineCache,
  MAX_CACHED_MESSAGES,
  MAX_CHANNEL_VARIANTS,
  TIMELINE_PAGE_SIZE
} from '../timelineState';

function createMessage(
  id: number,
  occurredAt: string,
  channel: Channel = 'zalo',
  textContent = `Message ${id}`
): ConversationMessage {
  return {
    id,
    conversationId: 1,
    channelAccountId: 10,
    channel,
    accountDisplayName: 'Zalo Account',
    senderIdentityId: 100,
    senderDisplayName: 'Customer',
    direction: 'inbound',
    providerMessageId: `prov-${id}`,
    contentType: 'text',
    textContent,
    content: {},
    occurredAt,
    status: 'received',
    removedAt: null,
    errorMessage: null,
    replyToMessageId: null
  };
}

describe('timelineState', () => {
  describe('constants', () => {
    it('defines expected timeline constants', () => {
      expect(TIMELINE_PAGE_SIZE).toBe(50);
      expect(MAX_CACHED_MESSAGES).toBe(200);
      expect(MAX_CHANNEL_VARIANTS).toBe(4);
    });
  });

  describe('channelSetKey', () => {
    it('sorts and formats channel set keys deterministically', () => {
      expect(channelSetKey(['email', 'zalo'])).toBe('email|zalo');
      expect(channelSetKey(['zalo', 'email'])).toBe('email|zalo');
      expect(channelSetKey(['messenger', 'whatsapp', 'zalo', 'email'])).toBe('email|messenger|whatsapp|zalo');
      expect(channelSetKey(['zalo'])).toBe('zalo');
      expect(channelSetKey([])).toBe('');
    });
  });

  describe('mergeMessages', () => {
    it('merges messages chronologically and deduplicates by id', () => {
      const msg1 = createMessage(1, '2026-07-31T10:00:00Z');
      const msg2 = createMessage(2, '2026-07-31T10:05:00Z');
      const msg2Updated = createMessage(2, '2026-07-31T10:05:00Z', 'zalo', 'Updated msg 2');

      const merged = mergeMessages([msg2], [msg1, msg2Updated]);
      expect(merged.map((m) => m.id)).toEqual([1, 2]);
      expect(merged[1].textContent).toBe('Updated msg 2');
    });

    it('sorts tie-breaker by message id ascending when occurredAt is identical', () => {
      const msgA = createMessage(10, '2026-07-31T10:00:00Z');
      const msgB = createMessage(5, '2026-07-31T10:00:00Z');

      const merged = mergeMessages([msgA], [msgB]);
      expect(merged.map((m) => m.id)).toEqual([5, 10]);
    });
  });

  describe('filterMessages', () => {
    it('filters messages by active channels', () => {
      const zaloMsg = createMessage(1, '2026-07-31T10:00:00Z', 'zalo');
      const emailMsg = createMessage(2, '2026-07-31T10:01:00Z', 'email');
      const whatsappMsg = createMessage(3, '2026-07-31T10:02:00Z', 'whatsapp');

      const filteredZalo = filterMessages([zaloMsg, emailMsg, whatsappMsg], ['zalo']);
      expect(filteredZalo.map((m) => m.id)).toEqual([1]);

      const filteredMulti = filterMessages([zaloMsg, emailMsg, whatsappMsg], ['zalo', 'email']);
      expect(filteredMulti.map((m) => m.id)).toEqual([1, 2]);
    });

    it('returns an empty array when active channels list is empty', () => {
      const zaloMsg = createMessage(1, '2026-07-31T10:00:00Z', 'zalo');
      const emailMsg = createMessage(2, '2026-07-31T10:01:00Z', 'email');

      expect(filterMessages([zaloMsg, emailMsg], [])).toEqual([]);
    });
  });

  describe('mergeTimelinePage', () => {
    it('drops stale server messages on refresh while retaining optimistic queued messages', () => {
      const retained = createMessage(1, '2026-08-02T00:40:00Z');
      const staleFailed = {
        ...createMessage(428, '2026-08-02T00:45:54Z'),
        direction: 'outbound' as const,
        status: 'failed' as const,
        providerMessageId: null
      };
      const optimisticQueued = {
        ...createMessage(999, '2026-08-02T01:01:00Z'),
        direction: 'outbound' as const,
        status: 'queued' as const,
        providerMessageId: null
      };
      const serverLatest = createMessage(2, '2026-08-02T01:00:00Z');

      const merged = mergeTimelinePage(
        {
          items: [retained, staleFailed, optimisticQueued],
          nextCursor: null,
          hasLoaded: true,
          loading: false
        },
        { items: [retained, serverLatest], nextCursor: null },
        'refresh'
      );

      expect(merged.items.map((message) => message.id)).toEqual([1, 2, 999]);
    });

    it('retains newest 200 items when mode is refresh', () => {
      const baseTime = 1700000000000;
      const currentItems = Array.from({ length: 150 }, (_, i) =>
        createMessage(i + 1, new Date(baseTime + i * 1000).toISOString())
      );

      const newPageItems = Array.from({ length: 100 }, (_, i) =>
        createMessage(i + 151, new Date(baseTime + (i + 150) * 1000).toISOString())
      );

      const variant = {
        items: currentItems,
        nextCursor: { occurredAt: new Date(baseTime).toISOString(), messageId: 1 },
        hasLoaded: true,
        loading: false
      };

      const merged = mergeTimelinePage(
        variant,
        {
          items: newPageItems,
          nextCursor: {
            occurredAt: newPageItems[0].occurredAt,
            messageId: newPageItems[0].id
          }
        },
        'refresh'
      );

      expect(merged.items.length).toBe(200);
      // Mode 'refresh' keeps the newest 200 items (IDs 51 through 250)
      expect(merged.items[0].id).toBe(51);
      expect(merged.items[merged.items.length - 1].id).toBe(250);
      expect(merged.hasLoaded).toBe(true);
      expect(merged.loading).toBe(false);
      expect(merged.nextCursor).toEqual({ occurredAt: new Date(baseTime).toISOString(), messageId: 1 });
    });

    it('retains oldest 200 items when mode is older', () => {
      const baseTime = 1700000000000;
      const currentItems = Array.from({ length: 150 }, (_, i) =>
        createMessage(i + 101, new Date(baseTime + (i + 100) * 1000).toISOString())
      );

      const olderPageItems = Array.from({ length: 100 }, (_, i) =>
        createMessage(i + 1, new Date(baseTime + i * 1000).toISOString())
      );

      const variant = {
        items: currentItems,
        nextCursor: { occurredAt: new Date(baseTime + 100 * 1000).toISOString(), messageId: 101 },
        hasLoaded: true,
        loading: false
      };

      const merged = mergeTimelinePage(
        variant,
        { items: olderPageItems, nextCursor: { occurredAt: new Date(baseTime).toISOString(), messageId: 1 } },
        'older'
      );

      expect(merged.items.length).toBe(200);
      // Mode 'older' keeps the oldest 200 items (IDs 1 through 200)
      expect(merged.items[0].id).toBe(1);
      expect(merged.items[merged.items.length - 1].id).toBe(200);
      expect(merged.nextCursor).toEqual({ occurredAt: new Date(baseTime).toISOString(), messageId: 1 });
    });
  });

  describe('nearestSurvivingAnchor', () => {
    it('returns anchorMessageId if anchor survives in nextItems', () => {
      const prev = [
        createMessage(1, '2026-07-31T10:01:00Z'),
        createMessage(2, '2026-07-31T10:02:00Z'),
        createMessage(3, '2026-07-31T10:03:00Z')
      ];
      const next = [createMessage(2, '2026-07-31T10:02:00Z'), createMessage(3, '2026-07-31T10:03:00Z')];

      expect(nearestSurvivingAnchor(prev, next, 2)).toBe(2);
    });

    it('returns null if anchorMessageId is null or nextItems is empty', () => {
      const prev = [createMessage(1, '2026-07-31T10:01:00Z')];
      const next = [createMessage(1, '2026-07-31T10:01:00Z')];

      expect(nearestSurvivingAnchor(prev, next, null)).toBeNull();
      expect(nearestSurvivingAnchor(prev, [], 1)).toBeNull();
    });

    it('picks the nearest surviving chronological neighbor when anchor is filtered out', () => {
      const msg1 = createMessage(1, '2026-07-31T10:01:00Z', 'email');
      const msg2 = createMessage(2, '2026-07-31T10:02:00Z', 'zalo');
      const msg3 = createMessage(3, '2026-07-31T10:03:00Z', 'email');
      const msg4 = createMessage(4, '2026-07-31T10:04:00Z', 'zalo');

      const prev = [msg1, msg2, msg3, msg4];
      const next = [msg1, msg3]; // zalo messages (2 and 4) hidden

      // Anchor was msg2 (index 1). Nearest neighbor in prev is msg3 (index 2) or msg1 (index 0).
      expect(nearestSurvivingAnchor(prev, next, 2)).toBe(3);
    });
  });

  describe('TimelineCache', () => {
    it('enforces maximum 4 channel variants with LRU eviction', () => {
      const cache = new TimelineCache();

      const variant = (id: number) => ({
        items: [createMessage(id, '2026-07-31T10:00:00Z')],
        nextCursor: null,
        hasLoaded: true,
        loading: false
      });

      cache.set('email', variant(1));
      cache.set('zalo', variant(2));
      cache.set('messenger', variant(3));
      cache.set('whatsapp', variant(4));

      expect(cache.size).toBe(4);

      // Access 'email' to update recency
      expect(cache.get('email')?.items[0].id).toBe(1);

      // Add a 5th variant 'email|zalo' -> should evict 'zalo' (least recently used)
      cache.set('email|zalo', variant(5));

      expect(cache.size).toBe(4);
      expect(cache.has('zalo')).toBe(false);
      expect(cache.has('email')).toBe(true);
      expect(cache.has('messenger')).toBe(true);
      expect(cache.has('whatsapp')).toBe(true);
      expect(cache.has('email|zalo')).toBe(true);
    });
  });
});
