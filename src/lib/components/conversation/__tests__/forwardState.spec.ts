import { describe, expect, test } from 'bun:test';
import {
  createForwardSnapshot,
  mergeForwardResult,
  canRetryForwardTarget,
  filterForwardTargets
} from '../forwardState';
import type {
  ConversationForwardTargetResult,
  ForwardConversationMessageResult,
  InboxItem
} from '$lib/services/conversationService';

describe('forwardState', () => {
  describe('createForwardSnapshot', () => {
    test('creates initial snapshot with defaults', () => {
      const snapshot = createForwardSnapshot(101);
      expect(snapshot.sourceMessageId).toBe(101);
      expect(snapshot.selectedTargetIds).toEqual([]);
      expect(typeof snapshot.clientOperationId).toBe('string');
      expect(snapshot.clientOperationId.length).toBeGreaterThan(0);
      expect(snapshot.status).toBe('idle');
      expect(snapshot.result).toBeNull();
      expect(snapshot.error).toBeNull();
    });

    test('accepts initial target IDs and custom operation ID', () => {
      const snapshot = createForwardSnapshot(101, [201, 202], 'op-123');
      expect(snapshot.sourceMessageId).toBe(101);
      expect(snapshot.selectedTargetIds).toEqual([201, 202]);
      expect(snapshot.clientOperationId).toBe('op-123');
    });
  });

  describe('mergeForwardResult', () => {
    test('merges full ForwardConversationMessageResult', () => {
      const snapshot = createForwardSnapshot(101, [201, 202], 'op-123');
      const fullResult: ForwardConversationMessageResult = {
        clientOperationId: 'op-123',
        sourceMessageId: 101,
        status: 'partial',
        targets: [
          {
            targetConversationId: 201,
            clientBatchId: 'batch-1',
            status: 'sent',
            errorCode: null,
            errorMessage: null
          },
          {
            targetConversationId: 202,
            clientBatchId: 'batch-2',
            status: 'failed',
            errorCode: 'NET_ERR',
            errorMessage: 'Network error'
          }
        ]
      };

      const updated = mergeForwardResult(snapshot, fullResult);
      expect(updated.status).toBe('submitted');
      expect(updated.result).toEqual(fullResult);
      expect(updated.error).toBeNull();
    });

    test('merges individual ConversationForwardTargetResult retry', () => {
      const snapshot = createForwardSnapshot(101, [201, 202], 'op-123');
      const initialResult: ForwardConversationMessageResult = {
        clientOperationId: 'op-123',
        sourceMessageId: 101,
        status: 'partial',
        targets: [
          {
            targetConversationId: 201,
            clientBatchId: 'batch-1',
            status: 'sent',
            errorCode: null,
            errorMessage: null
          },
          {
            targetConversationId: 202,
            clientBatchId: 'batch-2',
            status: 'failed',
            errorCode: 'NET_ERR',
            errorMessage: 'Network error'
          }
        ]
      };
      const submitted = mergeForwardResult(snapshot, initialResult);

      const retriedTarget: ConversationForwardTargetResult = {
        targetConversationId: 202,
        clientBatchId: 'batch-2-retry',
        status: 'sent',
        errorCode: null,
        errorMessage: null
      };

      const retriedSnapshot = mergeForwardResult(submitted, retriedTarget);
      expect(retriedSnapshot.result?.status).toBe('sent');
      expect(retriedSnapshot.result?.targets.find((t) => t.targetConversationId === 202)?.status).toBe(
        'sent'
      );
    });
  });

  describe('canRetryForwardTarget', () => {
    test('returns true for failed or delivery_unknown targets', () => {
      expect(
        canRetryForwardTarget({
          targetConversationId: 1,
          clientBatchId: 'b1',
          status: 'failed',
          errorCode: 'ERR',
          errorMessage: 'Fail'
        })
      ).toBe(true);

      expect(
        canRetryForwardTarget({
          targetConversationId: 1,
          clientBatchId: 'b1',
          status: 'delivery_unknown',
          errorCode: null,
          errorMessage: null
        })
      ).toBe(true);

      expect(canRetryForwardTarget('failed')).toBe(true);
      expect(canRetryForwardTarget('delivery_unknown')).toBe(true);
    });

    test('returns false for sent, sending, queued targets', () => {
      expect(canRetryForwardTarget('sent')).toBe(false);
      expect(canRetryForwardTarget('sending')).toBe(false);
      expect(canRetryForwardTarget('queued')).toBe(false);
    });
  });

  describe('filterForwardTargets', () => {
    const mockInbox: InboxItem[] = [
      {
        key: 'c1',
        kind: 'customer',
        leadId: 1,
        conversationId: 10,
        identityId: 100,
        title: 'Nguyen Van A',
        avatarUrl: null,
        lastMessagePreview: 'Hi',
        lastMessageAt: '2026-08-01T10:00:00Z',
        channels: [{ channel: 'zalo', messageCount: 1 }],
        replyTargets: [
          {
            conversationId: 10,
            channelAccountId: 1,
            channel: 'zalo',
            accountDisplayName: 'Zalo Account 1',
            conversationKind: 'direct',
            lastInboundAt: null,
            lastMessageAt: null
          }
        ],
        participantCount: 1,
        participants: [
          { identityId: 100, displayName: 'Nguyen Van A', avatarUrl: null, participantRole: 'customer' }
        ]
      },
      {
        key: 'c2',
        kind: 'customer',
        leadId: 2,
        conversationId: 20,
        identityId: 200,
        title: 'Tran Thi B',
        avatarUrl: null,
        lastMessagePreview: 'Hello',
        lastMessageAt: '2026-08-01T11:00:00Z',
        channels: [{ channel: 'zalo', messageCount: 2 }],
        replyTargets: [
          {
            conversationId: 20,
            channelAccountId: 2,
            channel: 'zalo',
            accountDisplayName: 'Zalo Account 2',
            conversationKind: 'direct',
            lastInboundAt: null,
            lastMessageAt: null
          }
        ],
        participantCount: 1,
        participants: [
          { identityId: 200, displayName: 'Tran Thi B', avatarUrl: null, participantRole: 'customer' }
        ]
      },
      {
        key: 'c3',
        kind: 'group',
        leadId: null,
        conversationId: 30,
        identityId: null,
        title: 'Nhom Support Zalo',
        avatarUrl: null,
        lastMessagePreview: 'OK',
        lastMessageAt: '2026-08-01T12:00:00Z',
        channels: [{ channel: 'zalo', messageCount: 5 }],
        replyTargets: [
          {
            conversationId: 30,
            channelAccountId: 1,
            channel: 'zalo',
            accountDisplayName: 'Zalo Account 1',
            conversationKind: 'group',
            lastInboundAt: null,
            lastMessageAt: null
          }
        ],
        participantCount: 3,
        participants: []
      }
    ];

    test('filters inbox by matching channelAccountId and excludes source conversation', () => {
      const filtered = filterForwardTargets(mockInbox, 1, 10, '');
      expect(filtered.map((item) => item.conversationId)).toEqual([30]);
    });

    test('filters by search query', () => {
      const filtered = filterForwardTargets(mockInbox, 1, null, 'Nhom');
      expect(filtered.map((item) => item.conversationId)).toEqual([30]);
    });
  });
});
