<script lang="ts">
  import type { ConversationReplyTarget } from '$lib/services/conversationService';

  let { replyTarget = null }: { replyTarget?: ConversationReplyTarget | null } = $props();
</script>

{#if replyTarget}
  <div class="message-reply-preview" data-testid="message-reply-preview">
    <div class="reply-line"></div>
    <div class="reply-body">
      <span class="reply-sender">{replyTarget.senderDisplayName ?? 'Tin nhắn'}</span>
      <span class="reply-text">{replyTarget.textContent ?? replyTarget.contentType}</span>
    </div>
  </div>
{/if}

<style>
  .message-reply-preview {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    background: rgba(0, 0, 0, 0.04);
    border-radius: 4px;
    margin-bottom: 4px;
    font-size: 11px;
  }

  .reply-line {
    width: 2px;
    height: 20px;
    background-color: #3b82f6;
    border-radius: 1px;
  }

  .reply-body {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .reply-sender {
    font-weight: 600;
    color: #3b82f6;
  }

  .reply-text {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: #6b7280;
  }
</style>
