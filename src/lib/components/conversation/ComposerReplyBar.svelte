<script lang="ts">
  import type { ConversationReplySelection } from './composerDraftState';

  let {
    reply = null,
    onCancel = () => {}
  }: {
    reply?: ConversationReplySelection | null;
    onCancel?: () => void;
  } = $props();
</script>

{#if reply}
  <div class="composer-reply-bar" data-testid="composer-reply-bar">
    <div class="reply-indicator"></div>
    <div class="reply-content">
      <span class="reply-sender">{reply.preview.senderDisplayName ?? 'Tin nhắn'}</span>
      <span class="reply-text">{reply.preview.textContent ?? reply.preview.contentType}</span>
    </div>
    <button
      type="button"
      class="reply-cancel-btn"
      aria-label="Cancel reply"
      onclick={onCancel}
      data-testid="cancel-reply-btn"
    >
      ✕
    </button>
  </div>
{/if}

<style>
  .composer-reply-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    background: var(--surface-secondary, #f3f4f6);
    border-top: 1px solid var(--border-color, #e5e7eb);
    font-size: 12px;
  }

  .reply-indicator {
    width: 3px;
    height: 24px;
    background-color: #3b82f6;
    border-radius: 2px;
  }

  .reply-content {
    display: flex;
    flex-direction: column;
    flex: 1;
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
    color: #4b5563;
  }

  .reply-cancel-btn {
    border: none;
    background: none;
    cursor: pointer;
    color: #9ca3af;
    padding: 2px 6px;
    font-size: 12px;
    border-radius: 4px;
  }

  .reply-cancel-btn:hover {
    color: #ef4444;
    background: #fee2e2;
  }
</style>
