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

export interface InboxReplyTarget {
  conversationId: number;
  channelAccountId: number;
  channel: Channel;
  accountDisplayName: string | null;
  conversationKind: ConversationKind;
  lastInboundAt: string | null;
  lastMessageAt: string | null;
}

export type ConversationReactionCode = 'heart' | 'like' | 'haha' | 'wow' | 'cry' | 'angry';

export interface ConversationMentionInput {
  identityId: number;
  displayText: string;
  startUtf16: number;
  lengthUtf16: number;
}

export interface ConversationMention {
  identityId: number | null;
  externalId: string;
  displayText: string;
  startUtf16: number;
  lengthUtf16: number;
}

export interface ConversationMentionCandidate {
  identityId: number;
  externalId: string;
  displayName: string;
  avatarUrl: string | null;
  participantRole: string;
}

export interface ListConversationMentionCandidatesRequest {
  conversationId: number;
  query?: string;
  limit?: number;
}

export interface ConversationQuickTemplate {
  id: number;
  shortcut: string;
  title: string;
  bodyText: string;
  category: string | null;
  isActive: boolean;
  usageCount: number;
  lastUsedAt: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ListConversationQuickTemplatesRequest {
  category?: string;
  searchQuery?: string;
  includeInactive?: boolean;
}

export interface CreateConversationQuickTemplateRequest {
  shortcut: string;
  title: string;
  bodyText: string;
  category?: string | null;
}

export interface UpdateConversationQuickTemplateRequest {
  id: number;
  shortcut: string;
  title: string;
  bodyText: string;
  category?: string | null;
  isActive: boolean;
}

export interface DeleteConversationQuickTemplateRequest {
  id: number;
}

export interface MarkConversationQuickTemplateUsedRequest {
  id: number;
}

export interface SetConversationReactionRequest {
  messageId: number;
  reaction: ConversationReactionCode;
  active: boolean;
  clientMutationId: string;
}

export interface ConversationReactionMutationResult {
  clientMutationId: string;
  messageId: number;
  reaction: ConversationReactionCode;
  requestedActive: boolean;
  status: 'running' | 'succeeded' | 'failed' | 'delivery_unknown';
  reactions: ConversationReactionGroup[];
  errorCode: string | null;
  errorMessage: string | null;
}

export interface ForwardConversationMessageRequest {
  sourceMessageId: number;
  targetConversationIds: number[];
  clientOperationId: string;
}

export interface ConversationForwardTargetResult {
  targetConversationId: number;
  clientBatchId: string;
  status: 'queued' | 'sending' | 'sent' | 'failed' | 'delivery_unknown';
  errorCode: string | null;
  errorMessage: string | null;
}

export interface ForwardConversationMessageResult {
  clientOperationId: string;
  sourceMessageId: number;
  status: 'queued' | 'sending' | 'partial' | 'sent' | 'failed' | 'delivery_unknown';
  targets: ConversationForwardTargetResult[];
}

export interface RetryConversationForwardTargetRequest {
  clientOperationId: string;
  targetConversationId: number;
}

export interface ConversationMessageActions {
  reply: boolean;
  copy: boolean;
  recall: boolean;
  forward: boolean;
  react: boolean;
}

export interface ConversationReactionCapabilities {
  read: boolean;
  write: boolean;
  supported: ConversationReactionCode[];
}

export interface ConversationForwardCapabilities {
  enabled: boolean;
  maxTargets: number;
  maxTargetsPerForward?: number;
}

export interface ConversationMentionCapabilities {
  enabled: boolean;
  maxPerMessage: number;
  maxMentionsPerMessage?: number;
  supportsAllMention?: boolean;
}

export interface ConversationQuickTemplateCapabilities {
  enabled: boolean;
  maxTemplates: number;
}

export interface ConversationCapabilities {
  maxTextLength: number;
  maxAttachmentsPerBatch: number;
  maxFileSizeBytes: number;
  supportedImageTypes: string[];
  supportedVideoTypes: string[];
  supportedAudioTypes: string[];
  supportedDocTypes: string[];
  supportsStickers: boolean;
  supportsQuoteReply: boolean;
  supportsRecall: boolean;
  recallWindowSeconds: number | null;
  supportsReactions: boolean;
  reactions: ConversationReactionCapabilities;
  forward: ConversationForwardCapabilities;
  mentions: ConversationMentionCapabilities;
  quickTemplates: ConversationQuickTemplateCapabilities;
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
  replyTargets: InboxReplyTarget[];
  participantCount: number;
  participants: ConversationParticipantSummary[];
}

export interface ConversationAttachment {
  id: number;
  mediaType: string;
  mimeType: string;
  fileName: string | null;
  sizeBytes: number | null;
  remoteUrl: string | null;
  thumbnailUrl: string | null;
  width: number | null;
  height: number | null;
  durationSeconds: number | null;
}

export interface ConversationReplyTarget {
  messageId: number | null;
  senderDisplayName: string | null;
  contentType: string;
  textContent: string | null;
  mediaType: string | null;
  occurredAt: string | null;
}

export interface ConversationReactionActor {
  identityId: number | null;
  externalId: string;
  displayName: string | null;
  avatarUrl: string | null;
}

export interface ConversationReactionGroup {
  reactionIcon: string;
  count: number;
  includesMe: boolean;
  actors: ConversationReactionActor[];
}

export type MessageSendBatchStatus = 'queued' | 'sending' | 'sent' | 'partial' | 'failed';

export interface ConversationSendBatchSummary {
  sendBatchId: string;
  status: MessageSendBatchStatus;
  totalParts: number;
  sentParts: number;
  failedParts: number;
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
  contentType: string;
  textContent: string | null;
  content: Record<string, unknown>;
  occurredAt: string;
  status: MessageStatus;
  removedAt: string | null;
  errorMessage: string | null;
  replyToMessageId: number | null;
  attachments: ConversationAttachment[];
  replyTarget: ConversationReplyTarget | null;
  reactions: ConversationReactionGroup[];
  batchIndex: number | null;
  sendBatch: ConversationSendBatchSummary | null;
  mentions?: ConversationMention[];
  actions?: ConversationMessageActions;
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
  capabilities: ConversationCapabilities;
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

export interface StagedConversationAttachment {
  stagedAttachmentId: string;
  mediaType: string;
  mimeType: string;
  fileName: string;
  fileSizeBytes: number;
  width: number | null;
  height: number | null;
  durationSeconds: number | null;
  expiresAt: string;
}

export type SendPartContent =
  | { type: 'text'; text: string; mentions?: ConversationMentionInput[] }
  | { type: 'sticker'; stickerId: number; cateId: number; stickerType: number }
  | { type: 'attachment'; stagedAttachmentId: string };

export type SendPartRequest = { clientPartId: string } & SendPartContent;

export interface SendConversationBatchRequest {
  conversationId: number;
  clientBatchId: string;
  replyToMessageId: number | null;
  parts: SendPartRequest[];
}

export interface SendConversationBatchResult {
  batchId: string;
  clientBatchId: string;
  messages: ConversationMessage[];
}

export interface ZaloStickerReference {
  stickerId: number;
  cateId: number;
  stickerType: number;
  url: string;
}

export interface StageClipboardImageRequest {
  conversationId: number;
  fileName: string;
  mimeType: string;
  base64Data: string;
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

export const getIdentityTimeline = (
  identityId: number,
  before: TimelineCursor | null,
  limit = 50
) => invoke<TimelinePage>('get_identity_timeline', { identityId, before, limit });

export const linkConversationIdentity = (identityId: number, leadId: number) =>
  invoke<void>('link_conversation_identity', { request: { identityId, leadId } });

export const unlinkConversationIdentity = (identityId: number) =>
  invoke<void>('unlink_conversation_identity', { request: { identityId } });

export const unlinkConversationLead = (leadId: number) =>
  invoke<void>('unlink_conversation_lead', { request: { leadId } });

export const relinkConversationLead = (sourceLeadId: number, targetLeadId: number) =>
  invoke<void>('relink_conversation_lead', { request: { sourceLeadId, targetLeadId } });

export const sendConversationMessage = (request: SendConversationMessageRequest) =>
  invoke<ConversationMessage>('send_conversation_message', { request });

export interface DeleteConversationRequest {
  conversationId?: number | null;
  identityId?: number | null;
  leadId?: number | null;
}

export interface StartDirectConversationRequest {
  channelAccountId: number;
  phoneOrExternalId: string;
  displayName?: string | null;
  initialMessage: string;
}

export const getConversationRuntimeStatus = () =>
  invoke<ConversationRuntimeStatus>('get_conversation_runtime_status');

export const restartConversationAccount = (providerAccountKey: string) =>
  invoke<void>('restart_conversation_account', { providerAccountKey });

export const deleteConversation = (request: DeleteConversationRequest) =>
  invoke<void>('delete_conversation', { request });

export const startDirectConversation = (request: StartDirectConversationRequest) =>
  invoke<ConversationMessage>('start_direct_conversation', { request });

export const stageConversationAttachmentPaths = (conversationId: number, paths: string[]) =>
  invoke<StagedConversationAttachment[]>('stage_conversation_attachment_paths', { conversationId, paths });

export const stageConversationClipboardImage = (request: StageClipboardImageRequest) =>
  invoke<StagedConversationAttachment>('stage_conversation_clipboard_image', { request });

export const discardConversationStagedAttachment = (stagedAttachmentId: string) =>
  invoke<void>('discard_conversation_staged_attachment', { stagedAttachmentId });

export const sendConversationBatch = (request: SendConversationBatchRequest) =>
  invoke<SendConversationBatchResult>('send_conversation_batch', { request });

export const searchZaloStickers = (conversationId: number, query: string, limit = 50) =>
  invoke<ZaloStickerReference[]>('search_zalo_stickers', { conversationId, query, limit });

export const recallConversationMessage = (messageId: number) =>
  invoke<ConversationMessage>('recall_conversation_message', { request: { messageId } });

export const setConversationReaction = (request: SetConversationReactionRequest) =>
  invoke<ConversationReactionMutationResult>('set_conversation_reaction', { request });

export const forwardConversationMessage = (request: ForwardConversationMessageRequest) =>
  invoke<ForwardConversationMessageResult>('forward_conversation_message', { request });

export const getConversationForwardOperation = (clientOperationId: string) =>
  invoke<ForwardConversationMessageResult>('get_conversation_forward_operation', {
    clientOperationId
  });

export const retryConversationForwardTarget = (request: RetryConversationForwardTargetRequest) =>
  invoke<ConversationForwardTargetResult>('retry_conversation_forward_target', { request });

export const listConversationQuickTemplates = (request: ListConversationQuickTemplatesRequest = {}) =>
  invoke<ConversationQuickTemplate[]>('list_conversation_quick_templates', { request });

export const createConversationQuickTemplate = (request: CreateConversationQuickTemplateRequest) =>
  invoke<ConversationQuickTemplate>('create_conversation_quick_template', { request });

export const updateConversationQuickTemplate = (request: UpdateConversationQuickTemplateRequest) =>
  invoke<ConversationQuickTemplate>('update_conversation_quick_template', { request });

export const deleteConversationQuickTemplate = (id: number) =>
  invoke<void>('delete_conversation_quick_template', { request: { id } });

export const markConversationQuickTemplateUsed = (id: number) =>
  invoke<void>('mark_conversation_quick_template_used', { request: { id } });

export const listConversationMentionCandidates = (request: ListConversationMentionCandidatesRequest) =>
  invoke<ConversationMentionCandidate[]>('list_conversation_mention_candidates', { request });

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
