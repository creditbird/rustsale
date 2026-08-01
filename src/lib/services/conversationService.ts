import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type Channel = 'zalo' | 'whatsapp' | 'messenger' | 'email';
export type ConversationKind = 'direct' | 'group' | 'email_thread';
export type InboxItemKind = 'customer' | 'unknown' | 'group';
export type MessageDirection = 'inbound' | 'outbound' | 'system';
export type MessageStatus =
  | 'queued'
  | 'sending'
  | 'sent'
  | 'delivered'
  | 'read'
  | 'received'
  | 'failed'
  | 'removed';
export type AccountConnectionStatus =
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'degraded'
  | 'invalid';

export interface TimelineCursor {
  occurredAt: string;
  messageId: number;
}

export interface ChannelBadgeSummary {
  channel: Channel;
  messageCount: number;
}

export interface ConversationReplyTarget {
  conversationId: number;
  channelAccountId: number;
  channel: Channel;
  accountDisplayName: string | null;
  conversationKind: ConversationKind;
  lastInboundAt: string | null;
  lastMessageAt: string | null;
}

export interface ConversationParticipantSummary {
  identityId: number;
  displayName: string | null;
  avatarUrl: string | null;
  participantRole: string;
}

export interface InboxItem {
  key: string;
  kind: InboxItemKind;
  leadId: number | null;
  conversationId: number | null;
  identityId: number | null;
  title: string;
  avatarUrl: string | null;
  lastMessagePreview: string | null;
  lastMessageAt: string | null;
  channels: ChannelBadgeSummary[];
  replyTargets: ConversationReplyTarget[];
  participantCount: number;
  participants: ConversationParticipantSummary[];
}

export interface ConversationMessage {
  id: number;
  conversationId: number;
  channelAccountId: number;
  channel: Channel;
  accountDisplayName: string | null;
  senderIdentityId: number | null;
  senderDisplayName: string | null;
  direction: MessageDirection;
  providerMessageId: string | null;
  contentType: string;
  textContent: string | null;
  content: Record<string, unknown>;
  occurredAt: string;
  status: MessageStatus;
  removedAt: string | null;
  errorMessage: string | null;
  replyToMessageId: number | null;
}

export interface TimelinePage {
  items: ConversationMessage[];
  nextCursor: TimelineCursor | null;
}

export interface ConversationAccountRuntime {
  channelAccountId: number;
  channel: Channel;
  providerAccountKey: string;
  displayName: string | null;
  status: AccountConnectionStatus;
  lastConnectedAt: string | null;
  lastError: string | null;
}

export interface ConversationRuntimeStatus {
  accounts: ConversationAccountRuntime[];
}

export interface SendConversationMessageRequest {
  conversationId: number;
  channelAccountId: number;
  text: string;
  replyToMessageId: number | null;
}

export interface ConversationInvalidatedEvent {
  conversationId: number;
  leadId: number | null;
  channel: Channel;
  reason: 'message' | 'receipt' | 'reaction' | 'metadata' | 'link';
  messageId?: number;
}

// IPC Command Wrappers
export const listConversationInbox = () =>
  invoke<InboxItem[]>('list_conversation_inbox');

export const getContactTimeline = (
  leadId: number,
  activeChannels: Channel[],
  before: TimelineCursor | null,
  limit = 50
) => invoke<TimelinePage>('get_contact_timeline', { leadId, activeChannels, before, limit });

export const getConversationTimeline = (
  conversationId: number,
  before: TimelineCursor | null,
  limit = 50
) => invoke<TimelinePage>('get_conversation_timeline', { conversationId, before, limit });

export const linkConversationIdentity = (identityId: number, leadId: number) =>
  invoke<void>('link_conversation_identity', { request: { identityId, leadId } });

export const unlinkConversationIdentity = (identityId: number) =>
  invoke<void>('unlink_conversation_identity', { request: { identityId } });

export const sendConversationMessage = (request: SendConversationMessageRequest) =>
  invoke<ConversationMessage>('send_conversation_message', { request });

export const getConversationRuntimeStatus = () =>
  invoke<ConversationRuntimeStatus>('get_conversation_runtime_status');

export const restartConversationAccount = (providerAccountKey: string) =>
  invoke<void>('restart_conversation_account', { providerAccountKey });

// Ref-counted Event Bridge Logic
type InvalidationListener = (event: ConversationInvalidatedEvent) => void;
const invalidationListeners = new Set<InvalidationListener>();

export function onConversationInvalidated(listener: InvalidationListener): () => void {
  invalidationListeners.add(listener);
  return () => {
    invalidationListeners.delete(listener);
  };
}

const BRIDGE_RETRY_BASE_DELAY_MS = 100;
const BRIDGE_RETRY_MAX_DELAY_MS = 5_000;
let bridgeRefCount = 0;
let bridgeUnlisten: (() => void) | null = null;
let bridgePromise: Promise<() => void> | null = null;
let bridgeRetryTimer: ReturnType<typeof setTimeout> | null = null;
let bridgeRetryAttempt = 0;

function clearBridgeRetry(): void {
  if (bridgeRetryTimer) {
    clearTimeout(bridgeRetryTimer);
    bridgeRetryTimer = null;
  }
  bridgeRetryAttempt = 0;
}

function scheduleBridgeRetry(): void {
  if (bridgeRefCount <= 0 || bridgeRetryTimer || bridgeUnlisten || bridgePromise) return;

  const delay = Math.min(
    BRIDGE_RETRY_BASE_DELAY_MS * 2 ** bridgeRetryAttempt,
    BRIDGE_RETRY_MAX_DELAY_MS
  );
  bridgeRetryAttempt += 1;
  bridgeRetryTimer = setTimeout(() => {
    bridgeRetryTimer = null;
    ensureConversationEventBridgeConnected();
  }, delay);
}

export function ensureConversationEventBridgeConnected(): Promise<() => void> | null {
  if (bridgeRefCount > 0 && !bridgeUnlisten && !bridgePromise) {
    if (bridgeRetryTimer) {
      clearTimeout(bridgeRetryTimer);
      bridgeRetryTimer = null;
    }
    bridgePromise = listen<ConversationInvalidatedEvent>('conversation-invalidated', (event) => {
      for (const listener of invalidationListeners) {
        try {
          listener(event.payload);
        } catch (err) {
          console.error('Error in invalidationListener', err);
        }
      }
    })
      .then((cleanup) => {
        bridgeUnlisten = cleanup;
        bridgeRetryAttempt = 0;
        if (bridgeRefCount <= 0 && bridgeUnlisten) {
          bridgeUnlisten();
          bridgeUnlisten = null;
          bridgePromise = null;
        }
        return cleanup;
      })
      .catch((err) => {
        bridgePromise = null;
        bridgeUnlisten = null;
        console.error('Failed to init conversation event bridge', err);
        scheduleBridgeRetry();
        return () => {};
      });
  }
  return bridgePromise;
}

export function initConversationEventBridge(): () => void {
  bridgeRefCount += 1;
  ensureConversationEventBridgeConnected();

  let cleaned = false;
  return () => {
    if (cleaned) return;
    cleaned = true;
    bridgeRefCount = Math.max(0, bridgeRefCount - 1);

    if (bridgeRefCount === 0) {
      clearBridgeRetry();
      if (bridgeUnlisten) {
        bridgeUnlisten();
        bridgeUnlisten = null;
        bridgePromise = null;
      }
    }
  };
}

export function resetConversationEventBridgeState(): void {
  invalidationListeners.clear();
  clearBridgeRetry();
  if (bridgeUnlisten) {
    bridgeUnlisten();
    bridgeUnlisten = null;
  }
  bridgeRefCount = 0;
  bridgePromise = null;
}

export function getBridgeRefCount(): number {
  return bridgeRefCount;
}

export function getBridgePromise(): Promise<() => void> | null {
  return bridgePromise;
}
