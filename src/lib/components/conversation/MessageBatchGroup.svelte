<script lang="ts">
  import type {
    ConversationCapabilities,
    ConversationMessage,
    ConversationReactionCode,
    MessageSendBatchStatus
  } from '$lib/services/conversationService';
  import MessageBubble from './MessageBubble.svelte';

  let {
    messages,
    isGroup = false,
    capabilitiesForMessage = () => undefined,
    pendingReactionsForMessage = () => [],
    onReply = () => {},
    onRecall = () => {},
    onForward = () => {},
    onToggleReaction = () => {}
  }: {
    messages: ConversationMessage[];
    isGroup?: boolean;
    capabilitiesForMessage?: (message: ConversationMessage) => ConversationCapabilities | undefined;
    pendingReactionsForMessage?: (messageId: number) => ConversationReactionCode[];
    onReply?: (message: ConversationMessage) => void;
    onRecall?: (message: ConversationMessage) => void;
    onForward?: (message: ConversationMessage) => void;
    onToggleReaction?: (
      message: ConversationMessage,
      code: ConversationReactionCode,
      currentActive?: boolean
    ) => void;
  } = $props();

  function getBatchStatus(batchMessages: ConversationMessage[]): MessageSendBatchStatus {
    const sentParts = batchMessages.filter((message) =>
      ['sent', 'delivered', 'read'].includes(message.status)
    ).length;
    const failedParts = batchMessages.filter((message) => message.status === 'failed').length;
    if (failedParts === batchMessages.length && batchMessages.length > 0) return 'failed';
    if (sentParts > 0 && sentParts + failedParts === batchMessages.length) return 'partial';
    if (sentParts === batchMessages.length && batchMessages.length > 0) return 'sent';
    if (sentParts > 0 || batchMessages.some((message) => message.status === 'sending')) return 'sending';
    return 'queued';
  }

  function statusLabel(status: MessageSendBatchStatus): string {
    switch (status) {
      case 'sending':
        return 'Đang gửi';
      case 'sent':
        return 'Đã gửi';
      case 'partial':
        return 'Đã gửi một phần';
      case 'failed':
        return 'Gửi thất bại';
      default:
        return 'Đang chờ gửi';
    }
  }
</script>

<section class="space-y-1" data-testid="message-batch-group">
  {#each messages as message (message.id)}
    <MessageBubble
      {message}
      {isGroup}
      showStatus={false}
      capabilities={capabilitiesForMessage(message)}
      pendingReactions={pendingReactionsForMessage(message.id)}
      {onReply}
      {onRecall}
      {onForward}
      {onToggleReaction}
    />
    {#if message.status === 'failed' && message.errorMessage}
      <p class="ml-auto max-w-[70%] px-3 text-xs text-destructive">{message.errorMessage}</p>
    {/if}
  {/each}
  <div class="flex justify-end px-3 text-[11px] font-medium text-muted-foreground">
    {statusLabel(getBatchStatus(messages))}
  </div>
</section>
