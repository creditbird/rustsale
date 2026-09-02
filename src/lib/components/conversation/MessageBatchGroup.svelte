<script lang="ts">
  import type {
    ConversationCapabilities,
    ConversationMessage,
    ConversationReactionCode,
    ConversationReactionMutationResult
  } from '$lib/services/conversationService';
  import MessageBubble from './MessageBubble.svelte';

  let {
    messages,
    isGroup = false,
    conversationTitle = '',
    capabilitiesForMessage = () => undefined,
    pendingReactionsForMessage = () => [],
    pinnedMessageIds = new Set<number>(),
    onReply = () => {},
    onRecall = () => {},
    onForward = () => {},
    onPin = () => {},
    onToggleReaction = () => Promise.resolve(null),
    onImageClick = () => {},
    onUserClick = () => {},
    onJumpToMessage = () => {}
  }: {
    messages: ConversationMessage[];
    isGroup?: boolean;
    conversationTitle?: string;
    capabilitiesForMessage?: (message: ConversationMessage) => ConversationCapabilities | undefined;
    pendingReactionsForMessage?: (messageId: number) => ConversationReactionCode[];
    pinnedMessageIds?: Set<number>;
    onReply?: (message: ConversationMessage) => void;
    onRecall?: (message: ConversationMessage) => void;
    onForward?: (message: ConversationMessage) => void;
    onPin?: (message: ConversationMessage) => void;
    onToggleReaction?: (
      message: ConversationMessage,
      code: ConversationReactionCode,
      action?: 'increment' | 'remove'
    ) => Promise<ConversationReactionMutationResult | null>;
    onImageClick?: (url: string, message: ConversationMessage) => void;
    onUserClick?: (payload: {
      identityId: number | null;
      displayName: string | null;
      externalId?: string;
      sourceMessageId?: number;
      targetEl?: HTMLElement;
    }) => void;
    onJumpToMessage?: (messageId: number) => void;
  } = $props();
</script>

<section class="space-y-1" data-testid="message-batch-group">
  {#each messages as message (message.id)}
    <MessageBubble
      {message}
      {isGroup}
      {conversationTitle}
      showStatus={true}
      capabilities={capabilitiesForMessage(message)}
      pendingReactions={pendingReactionsForMessage(message.id)}
      isPinned={pinnedMessageIds.has(message.id)}
      {onReply}
      {onRecall}
      {onForward}
      {onPin}
      {onToggleReaction}
      {onImageClick}
      {onUserClick}
      {onJumpToMessage}
    />
    {#if message.status === 'failed' && message.errorMessage}
      <p class="ml-auto max-w-[70%] px-3 text-xs text-destructive">{message.errorMessage}</p>
    {/if}
  {/each}
</section>
