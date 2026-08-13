import { beforeEach, describe, expect, mock, test } from 'bun:test';

let listenCallCount = 0;
let unlistenCallCount = 0;
let listenFailuresRemaining = 0;
let activeListenCount = 0;
let deferNextListen = false;
let resolveDeferredListen: (() => void) | null = null;
let eventCallback: ((event: { payload: unknown }) => void) | null = null;
let invokedCommands: Array<{ command: string; args?: Record<string, unknown> }> = [];

function createUnlisten(): () => void {
  activeListenCount += 1;
  let cleaned = false;
  return () => {
    if (cleaned) return;
    cleaned = true;
    activeListenCount -= 1;
    unlistenCallCount += 1;
  };
}

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string, args?: Record<string, unknown>) => {
    invokedCommands.push({ command, args });
    if (command === 'list_conversation_inbox') {
      return [];
    }
    if (
      command === 'get_contact_timeline' ||
      command === 'get_conversation_timeline' ||
      command === 'get_identity_timeline'
    ) {
      return { items: [], nextCursor: null };
    }
    if (command === 'get_conversation_runtime_status') {
      return { accounts: [] };
    }
    if (command === 'sync_and_deep_scan_group_members') {
      return deepScanResponse;
    }
    if (command === 'batch_import_group_members_to_leads') {
      return groupImportResponse;
    }
    if (command === 'send_conversation_message') {
      return {
        id: 100,
        conversationId: args?.request ? (args.request as Record<string, unknown>).conversationId : 0,
        channelAccountId: args?.request ? (args.request as Record<string, unknown>).channelAccountId : 0,
        channel: 'zalo',
        accountDisplayName: null,
        senderIdentityId: null,
        senderDisplayName: null,
        direction: 'outbound',
        contentType: 'text',
        textContent: args?.request ? (args.request as Record<string, unknown>).text : '',
        content: {},
        occurredAt: '2026-07-31T00:00:00.000Z',
        status: 'sent',
        removedAt: null,
        errorMessage: null,
        replyToMessageId: null
      };
    }
    return undefined;
  }
}));

mock.module('@tauri-apps/api/event', () => ({
  listen: async (eventName: string, cb: (event: { payload: unknown }) => void) => {
    listenCallCount += 1;
    eventCallback = cb;
    if (listenFailuresRemaining > 0) {
      listenFailuresRemaining -= 1;
      throw new Error('transient listen failure');
    }
    if (deferNextListen) {
      deferNextListen = false;
      return new Promise<() => void>((resolve) => {
        resolveDeferredListen = () => resolve(createUnlisten());
      });
    }
    return createUnlisten();
  }
}));

import {
  getContactTimeline,
  getConversationRuntimeStatus,
  getConversationTimeline,
  getIdentityTimeline,
  initConversationEventBridge,
  linkConversationIdentity,
  relinkConversationLead,
  unlinkConversationIdentity,
  unlinkConversationLead,
  listConversationInbox,
  onConversationInvalidated,
  resetConversationEventBridgeState,
  deleteConversation,
  deleteConversationsBatch,
  restartConversationAccount,
  sendConversationMessage,
  sendConversationBatch,
  stageConversationAttachmentPaths,
  stageConversationClipboardImage,
  discardConversationStagedAttachment,
  searchZaloStickers,
  recallConversationMessage,
  setConversationReaction,
  forwardConversationMessage,
  getConversationForwardOperation,
  retryConversationForwardTarget,
  listConversationQuickTemplates,
  createConversationQuickTemplate,
  updateConversationQuickTemplate,
  deleteConversationQuickTemplate,
  markConversationQuickTemplateUsed,
  listConversationMentionCandidates,
  syncAndDeepScanGroupMembers,
  batchImportGroupMembersToLeads,
  getBridgeRefCount,
  getBridgePromise,
  type ConversationInvalidatedEvent,
  type GroupMemberScanResult,
  type BatchImportGroupMembersRequest,
  type BatchImportGroupMembersResult
} from '../conversationService';

const deepScanResponse = {
  members: [],
  discovered_count: 0,
  profile_hydrated_count: 0,
  membership_complete: false,
  pages_attempted: 2,
  pages_succeeded: 1,
  messages_scanned: 17,
  stop_reason: 'timeout',
  warning: 'Timed out waiting for account-wide Zalo group history backfill',
  diagnostics: {
    direct_snapshot: {
      attempted: true,
      complete: false,
      member_count: 0,
      warning: null
    },
    group_link: {
      attempted: false,
      pages_attempted: 0,
      pages_succeeded: 0,
      items_inspected: 0,
      committed_uid_count: 0,
      link_retrieved: false,
      warning: null
    },
    board: {
      attempted: true,
      pages_attempted: 2,
      pages_succeeded: 1,
      items_inspected: 4,
      polls_attempted: 1,
      polls_succeeded: 0,
      committed_uid_count: 2,
      stop_reason: 'board_page_failed',
      warnings: [
        {
          source: 'poll',
          code: 'poll_detail_failed',
          context: 'poll_id=7',
          message: 'poll unavailable'
        }
      ]
    },
    history_backfill: {
      provider_account_key: 'account-a',
      scope: 'account_group_messages',
      outcome: 'timed_out',
      joined_existing_run: false,
      pages_requested: 2,
      pages_received: 1,
      start_cursor: 'cursor-start',
      durable_cursor: 'cursor-durable',
      stop_reason: 'timeout',
      warning: 'Timed out waiting for account-wide Zalo group history backfill'
    },
    local_evidence: {
      messages_scanned: 17,
      participant_uid_count: 1,
      sender_uid_count: 2,
      mention_uid_count: 1,
      reaction_uid_count: 1,
      quote_owner_uid_count: 1,
      total_uid_count: 5,
      committed_uid_count: 5,
      rejected_uid_count: 0
    },
    profile_hydration: {
      attempted: 5,
      succeeded: 4,
      failed: 1,
      warnings: []
    },
    partial_result: true,
    warnings: [
      {
        stage: 'history_backfill',
        code: 'timed_out',
        context: 'timeout',
        message: 'Timed out waiting for account-wide Zalo group history backfill'
      }
    ]
  }
} satisfies GroupMemberScanResult;

const groupImportRequest = {
  conversation_id: 143,
  members: [
    {
      identity_id: 901,
      external_id: 'uid-901',
      display_name: 'Caller display is not membership evidence',
      avatar_url: null,
      phone: null
    }
  ],
  category_ids: [7],
  new_category_name: 'Imported Group',
  group_title: 'Group 143',
  skip_existing: true
} satisfies BatchImportGroupMembersRequest;

const groupImportResponse = {
  imported_count: 0,
  updated_count: 0,
  skipped_count: 1,
  assigned_category_ids: [7]
} satisfies BatchImportGroupMembersResult;

beforeEach(() => {
  resetConversationEventBridgeState();
  listenCallCount = 0;
  unlistenCallCount = 0;
  listenFailuresRemaining = 0;
  activeListenCount = 0;
  deferNextListen = false;
  resolveDeferredListen = null;
  eventCallback = null;
  invokedCommands = [];
});

describe('conversationService IPC command wrappers', () => {
  test('syncAndDeepScanGroupMembers mirrors structured diagnostics and exact IPC args', async () => {
    const result = await syncAndDeepScanGroupMembers(143);

    expect(invokedCommands).toContainEqual({
      command: 'sync_and_deep_scan_group_members',
      args: { conversationId: 143 }
    });
    expect(result.diagnostics.history_backfill.scope).toBe('account_group_messages');
    expect(result.diagnostics.history_backfill.outcome).toBe('timed_out');
    expect(result.diagnostics.board.warnings[0]?.code).toBe('poll_detail_failed');
    expect(result.diagnostics.local_evidence.quote_owner_uid_count).toBe(1);
  });

  test('batchImportGroupMembersToLeads includes conversation membership scope', async () => {
    const result = await batchImportGroupMembersToLeads(groupImportRequest);

    expect(invokedCommands).toContainEqual({
      command: 'batch_import_group_members_to_leads',
      args: { request: groupImportRequest }
    });
    expect(result.skipped_count).toBe(1);
  });

  test('listConversationInbox calls list_conversation_inbox command', async () => {
    const inbox = await listConversationInbox();
    expect(inbox).toEqual([]);
    expect(invokedCommands).toContainEqual({
      command: 'list_conversation_inbox',
      args: undefined
    });
  });

  test('getContactTimeline matches exact IPC call structure', async () => {
    await getContactTimeline(
      7,
      ['zalo', 'email'],
      { occurredAt: '2026-07-31T01:00:00.000Z', messageId: 12 },
      50
    );

    expect(invokedCommands).toContainEqual({
      command: 'get_contact_timeline',
      args: {
        leadId: 7,
        activeChannels: ['zalo', 'email'],
        before: { occurredAt: '2026-07-31T01:00:00.000Z', messageId: 12 },
        limit: 50
      }
    });
  });

  test('getConversationTimeline calls get_conversation_timeline with args', async () => {
    await getConversationTimeline(15, null, 20);

    expect(invokedCommands).toContainEqual({
      command: 'get_conversation_timeline',
      args: {
        conversationId: 15,
        before: null,
        limit: 20
      }
    });
  });

  test('getIdentityTimeline calls get_identity_timeline with args', async () => {
    await getIdentityTimeline(28, null, 50);

    expect(invokedCommands).toContainEqual({
      command: 'get_identity_timeline',
      args: {
        identityId: 28,
        before: null,
        limit: 50
      }
    });
  });

  test('linkConversationIdentity calls link_conversation_identity with request wrapper', async () => {
    await linkConversationIdentity(10, 7);

    expect(invokedCommands).toContainEqual({
      command: 'link_conversation_identity',
      args: {
        request: { identityId: 10, leadId: 7 }
      }
    });
  });

  test('unlinkConversationIdentity calls unlink_conversation_identity with request wrapper', async () => {
    await unlinkConversationIdentity(10);

    expect(invokedCommands).toContainEqual({
      command: 'unlink_conversation_identity',
      args: {
        request: { identityId: 10 }
      }
    });
  });

  test('unlinkConversationLead calls unlink_conversation_lead with request wrapper', async () => {
    await unlinkConversationLead(7);

    expect(invokedCommands).toContainEqual({
      command: 'unlink_conversation_lead',
      args: {
        request: { leadId: 7 }
      }
    });
  });

  test('relinkConversationLead calls relink_conversation_lead with request wrapper', async () => {
    await relinkConversationLead(7, 8);

    expect(invokedCommands).toContainEqual({
      command: 'relink_conversation_lead',
      args: {
        request: { sourceLeadId: 7, targetLeadId: 8 }
      }
    });
  });

  test('sendConversationMessage matches exact IPC call structure', async () => {
    await sendConversationMessage({
      conversationId: 31,
      channelAccountId: 4,
      text: 'Xin chào',
      replyToMessageId: null
    });

    expect(invokedCommands).toContainEqual({
      command: 'send_conversation_message',
      args: {
        request: {
          conversationId: 31,
          channelAccountId: 4,
          text: 'Xin chào',
          replyToMessageId: null
        }
      }
    });
  });

  test('getConversationRuntimeStatus calls get_conversation_runtime_status command', async () => {
    await getConversationRuntimeStatus();

    expect(invokedCommands).toContainEqual({
      command: 'get_conversation_runtime_status',
      args: undefined
    });
  });

  test('restartConversationAccount calls restart_conversation_account command', async () => {
    await restartConversationAccount('zalo_acc_1');

    expect(invokedCommands).toContainEqual({
      command: 'restart_conversation_account',
      args: {
        providerAccountKey: 'zalo_acc_1'
      }
    });
  });

  test('sendConversationBatch calls send_conversation_batch command', async () => {
    await sendConversationBatch({
      conversationId: 31,
      clientBatchId: 'batch-1',
      replyToMessageId: 9,
      parts: [
        { clientPartId: 'part-1', type: 'text', text: 'Xin chào' }
      ]
    });

    expect(invokedCommands).toContainEqual({
      command: 'send_conversation_batch',
      args: {
        request: {
          conversationId: 31,
          clientBatchId: 'batch-1',
          replyToMessageId: 9,
          parts: [
            { clientPartId: 'part-1', type: 'text', text: 'Xin chào' }
          ]
        }
      }
    });
  });

  test('stageConversationAttachmentPaths calls stage_conversation_attachment_paths command', async () => {
    await stageConversationAttachmentPaths(31, ['/path/image.png']);

    expect(invokedCommands).toContainEqual({
      command: 'stage_conversation_attachment_paths',
      args: { conversationId: 31, paths: ['/path/image.png'] }
    });
  });

  test('stageConversationClipboardImage calls stage_conversation_clipboard_image command', async () => {
    await stageConversationClipboardImage({
      conversationId: 31,
      fileName: 'clip.png',
      mimeType: 'image/png',
      base64Data: 'abc123'
    });

    expect(invokedCommands).toContainEqual({
      command: 'stage_conversation_clipboard_image',
      args: {
        request: {
          conversationId: 31,
          fileName: 'clip.png',
          mimeType: 'image/png',
          base64Data: 'abc123'
        }
      }
    });
  });

  test('discardConversationStagedAttachment calls discard_conversation_staged_attachment command', async () => {
    await discardConversationStagedAttachment('staged-1');

    expect(invokedCommands).toContainEqual({
      command: 'discard_conversation_staged_attachment',
      args: { stagedAttachmentId: 'staged-1' }
    });
  });

  test('searchZaloStickers calls search_zalo_stickers command', async () => {
    await searchZaloStickers(31, 'chào', 20);

    expect(invokedCommands).toContainEqual({
      command: 'search_zalo_stickers',
      args: { conversationId: 31, query: 'chào', limit: 20 }
    });
  });

  test('recallConversationMessage calls recall_conversation_message command', async () => {
    await recallConversationMessage(105);

    expect(invokedCommands).toContainEqual({
      command: 'recall_conversation_message',
      args: { request: { messageId: 105 } }
    });
  });

  test('deleteConversationsBatch calls delete_conversations_batch with requests array', async () => {
    const requests = [
      { conversationId: 10, identityId: null, leadId: null },
      { conversationId: 20, identityId: null, leadId: null }
    ];
    await deleteConversationsBatch(requests);

    expect(invokedCommands).toContainEqual({
      command: 'delete_conversations_batch',
      args: { requests }
    });
  });

  test('setConversationReaction calls set_conversation_reaction with request', async () => {
    const req = {
      messageId: 10,
      reaction: 'heart' as const,
      active: true,
      clientMutationId: 'mut-1'
    };
    await setConversationReaction(req);

    expect(invokedCommands).toContainEqual({
      command: 'set_conversation_reaction',
      args: { request: req }
    });
  });

  test('forwardConversationMessage calls forward_conversation_message with request', async () => {
    const req = {
      sourceMessageId: 10,
      targetConversationIds: [20, 21],
      clientOperationId: 'op-1'
    };
    await forwardConversationMessage(req);

    expect(invokedCommands).toContainEqual({
      command: 'forward_conversation_message',
      args: { request: req }
    });
  });

  test('getConversationForwardOperation calls get_conversation_forward_operation', async () => {
    await getConversationForwardOperation('op-1');

    expect(invokedCommands).toContainEqual({
      command: 'get_conversation_forward_operation',
      args: { clientOperationId: 'op-1' }
    });
  });

  test('retryConversationForwardTarget calls retry_conversation_forward_target with request', async () => {
    const req = {
      clientOperationId: 'op-1',
      targetConversationId: 20
    };
    await retryConversationForwardTarget(req);

    expect(invokedCommands).toContainEqual({
      command: 'retry_conversation_forward_target',
      args: { request: req }
    });
  });

  test('listConversationQuickTemplates calls list_conversation_quick_templates with request', async () => {
    const req = { category: 'sales', includeInactive: false };
    await listConversationQuickTemplates(req);

    expect(invokedCommands).toContainEqual({
      command: 'list_conversation_quick_templates',
      args: { request: req }
    });
  });

  test('createConversationQuickTemplate calls create_conversation_quick_template with request', async () => {
    const req = {
      shortcut: 'hello',
      title: 'Greeting',
      bodyText: 'Hello world',
      category: 'general'
    };
    await createConversationQuickTemplate(req);

    expect(invokedCommands).toContainEqual({
      command: 'create_conversation_quick_template',
      args: { request: req }
    });
  });

  test('updateConversationQuickTemplate calls update_conversation_quick_template with request', async () => {
    const req = {
      id: 5,
      shortcut: 'hello2',
      title: 'Greeting updated',
      bodyText: 'Hello world updated',
      category: 'general',
      isActive: true
    };
    await updateConversationQuickTemplate(req);

    expect(invokedCommands).toContainEqual({
      command: 'update_conversation_quick_template',
      args: { request: req }
    });
  });

  test('deleteConversationQuickTemplate calls delete_conversation_quick_template with request id wrapper', async () => {
    await deleteConversationQuickTemplate(5);

    expect(invokedCommands).toContainEqual({
      command: 'delete_conversation_quick_template',
      args: { request: { id: 5 } }
    });
  });

  test('markConversationQuickTemplateUsed calls mark_conversation_quick_template_used with request id wrapper', async () => {
    await markConversationQuickTemplateUsed(5);

    expect(invokedCommands).toContainEqual({
      command: 'mark_conversation_quick_template_used',
      args: { request: { id: 5 } }
    });
  });

  test('listConversationMentionCandidates calls list_conversation_mention_candidates with request', async () => {
    const req = { conversationId: 12, query: 'nam', limit: 10 };
    await listConversationMentionCandidates(req);

    expect(invokedCommands).toContainEqual({
      command: 'list_conversation_mention_candidates',
      args: { request: req }
    });
  });
});

describe('conversationService ref-counted invalidation bridge', () => {
  test('two initConversationEventBridge consumers create one Tauri listener', async () => {
    const unsub1 = initConversationEventBridge();
    expect(getBridgeRefCount()).toBe(1);

    const unsub2 = initConversationEventBridge();
    expect(getBridgeRefCount()).toBe(2);

    await getBridgePromise();

    expect(listenCallCount).toBe(1);

    unsub1();
    unsub2();
  });

  test('both onConversationInvalidated subscribers receive the same event', async () => {
    const received1: ConversationInvalidatedEvent[] = [];
    const received2: ConversationInvalidatedEvent[] = [];

    const unsubSub1 = onConversationInvalidated((event) => {
      received1.push(event);
    });
    const unsubSub2 = onConversationInvalidated((event) => {
      received2.push(event);
    });

    const unsubBridge = initConversationEventBridge();
    await getBridgePromise();

    const sampleEvent: ConversationInvalidatedEvent = {
      conversationId: 31,
      leadId: 7,
      channel: 'zalo',
      reason: 'message',
      messageId: 100
    };

    expect(eventCallback).not.toBeNull();
    eventCallback!({ payload: sampleEvent });

    expect(received1).toEqual([sampleEvent]);
    expect(received2).toEqual([sampleEvent]);

    unsubBridge();
    unsubSub1();
    unsubSub2();
  });

  test('removing the final bridge consumer invokes the Tauri unlisten callback once', async () => {
    const unsub1 = initConversationEventBridge();
    const unsub2 = initConversationEventBridge();
    await getBridgePromise();

    expect(unlistenCallCount).toBe(0);

    unsub1();
    expect(unlistenCallCount).toBe(0);

    unsub2();
    expect(unlistenCallCount).toBe(1);
  });

  test('a failed initial listen schedules one bounded retry and final cleanup cancels the retry', async () => {
    listenFailuresRemaining = 1;

    const unsub = initConversationEventBridge();
    expect(getBridgeRefCount()).toBe(1);

    // Wait for the retry loop to trigger
    const deadline = Date.now() + 1_000;
    while (listenCallCount < 2 && Date.now() < deadline) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }

    expect(listenCallCount).toBe(2);
    expect(getBridgeRefCount()).toBe(1);
    expect(getBridgePromise()).not.toBeNull();

    unsub();
    expect(unlistenCallCount).toBe(1);
  });

  test('cancels scheduled retry when unsubscribed before retry fires', async () => {
    listenFailuresRemaining = 1;

    const unsub = initConversationEventBridge();
    const failedPromise = getBridgePromise();
    await failedPromise;

    unsub();
    await new Promise((resolve) => setTimeout(resolve, 150));

    expect(getBridgeRefCount()).toBe(0);
    expect(listenCallCount).toBe(1);
    expect(activeListenCount).toBe(0);
  });
});
