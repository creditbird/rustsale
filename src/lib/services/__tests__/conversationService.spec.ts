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
    if (command === 'get_contact_timeline' || command === 'get_conversation_timeline') {
      return { items: [], nextCursor: null };
    }
    if (command === 'get_conversation_runtime_status') {
      return { accounts: [] };
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
        providerMessageId: null,
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
  initConversationEventBridge,
  linkConversationIdentity,
  unlinkConversationIdentity,
  listConversationInbox,
  onConversationInvalidated,
  resetConversationEventBridgeState,
  restartConversationAccount,
  sendConversationMessage,
  getBridgeRefCount,
  getBridgePromise,
  type ConversationInvalidatedEvent
} from '../conversationService';

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
