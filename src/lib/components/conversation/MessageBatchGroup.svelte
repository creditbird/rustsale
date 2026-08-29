<script lang="ts">
  import type {
    ConversationCapabilities,
    ConversationMessage,
    ConversationReactionCode,
    ConversationReactionMutationResult,
    MessageSendBatchStatus
  } from '$lib/services/conversationService';
  import MessageBubble from './MessageBubble.svelte';
  import { Check, CheckCheck, Clock, AlertCircle } from 'lucide-svelte';

  let {
    messages,
    isGroup = false,
    conversationTitle = '',
    capabilitiesForMessage = () => undefined,
    pendingReactionsForMessage = () => [],
    onReply = () => {},
    onRecall = () => {},
    onForward = () => {},
    onPin = () => {},
    onToggleReaction = () => Promise.resolve(null),
    onImageClick = () => {},
    onUserClick = () => {}
  }: {
    messages: ConversationMessage[];
    isGroup?: boolean;
    conversationTitle?: string;
    capabilitiesForMessage?: (message: ConversationMessage) => ConversationCapabilities | undefined;
    pendingReactionsForMessage?: (messageId: number) => ConversationReactionCode[];
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
  } = $props();

  function getBatchStatus(batchMessages: ConversationMessage[]): MessageSendBatchStatus | 'delivered' | 'read' {
    const failedParts = batchMessages.filter((message) => message.status === 'failed').length;
    if (failedParts === batchMessages.length && batchMessages.length > 0) return 'failed';
    const sentParts = batchMessages.filter((message) =>
      ['sent', 'delivered', 'read'].includes(message.status)
    ).length;
    if (sentParts > 0 && failedParts > 0 && sentParts + failedParts === batchMessages.length) return 'partial';
    if (batchMessages.length > 0 && batchMessages.every((message) => message.status === 'read')) return 'read';
    if (batchMessages.length > 0 && batchMessages.every((message) => ['delivered', 'read'].includes(message.status))) return 'delivered';
    if (sentParts === batchMessages.length && batchMessages.length > 0) return 'sent';
    if (sentParts > 0 || batchMessages.some((message) => message.status === 'sending')) return 'sending';
    return 'queued';
  }

  function statusLabel(status: MessageSendBatchStatus | 'delivered' | 'read'): string {
    switch (status) {
      case 'sending':
        return 'Sending';
      case 'read':
        return 'Read';
      case 'delivered':
        return 'Delivered';
      case 'sent':
        return 'Sent';
      case 'partial':
        return 'Partially sent';
      case 'failed':
        return 'Failed to send';
      default:
        return 'Queued';
    }
  }

  const batchStatus = $derived(getBatchStatus(messages));
</script>

<section class="space-y-1" data-testid="message-batch-group">
  {#each messages as message (message.id)}
    <MessageBubble
      {message}
      {isGroup}
      {conversationTitle}
      showStatus={false}
      capabilities={capabilitiesForMessage(message)}
      pendingReactions={pendingReactionsForMessage(message.id)}
      {onReply}
      {onRecall}
      {onForward}
      {onPin}
      {onToggleReaction}
      {onImageClick}
      {onUserClick}
    />
    {#if message.status === 'failed' && message.errorMessage}
      <p class="ml-auto max-w-[70%] px-3 text-xs text-destructive">{message.errorMessage}</p>
    {/if}
  {/each}
  <div class="flex items-center justify-end gap-1 px-3 text-[11px] font-medium text-muted-foreground">
    {#if batchStatus === 'read'}
      <CheckCheck class="size-3.5 text-[#53bdeb] font-bold" />
    {:else if batchStatus === 'delivered'}
      <CheckCheck class="size-3.5 text-[#667781] dark:text-[#8696a0]" />
    {:else if batchStatus === 'sent'}
      <Check class="size-3.5 text-[#667781] dark:text-[#8696a0]" />
    {:else if batchStatus === 'sending' || batchStatus === 'queued'}
      <Clock class="size-3 text-muted-foreground animate-spin" />
    {:else if batchStatus === 'failed' || batchStatus === 'partial'}
      <AlertCircle class="size-3.5 text-destructive font-bold" />
    {/if}
    <span>{statusLabel(batchStatus)}</span>
  </div>
</section>
