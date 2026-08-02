<script lang="ts">
  import type { ConversationAttachment } from '$lib/services/conversationService';
  import { formatBytes, safeConversationUrl } from './messageMedia';

  let { attachment }: { attachment: ConversationAttachment } = $props();
  let safeUrl = $derived(safeConversationUrl(attachment.remoteUrl));
</script>

<div class="file-message-content" data-testid="file-message-content">
  <div class="file-icon">📄</div>
  <div class="file-details">
    {#if safeUrl}
      <a href={safeUrl} target="_blank" rel="noopener noreferrer" class="file-name">
        {attachment.fileName ?? 'Tập tin'}
      </a>
    {:else}
      <span class="file-name">{attachment.fileName ?? 'Tập tin'}</span>
    {/if}
    <span class="file-size">{formatBytes(attachment.sizeBytes)}</span>
  </div>
</div>

<style>
  .file-message-content {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: rgba(0, 0, 0, 0.03);
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 8px;
  }

  .file-icon {
    font-size: 24px;
  }

  .file-details {
    display: flex;
    flex-direction: column;
  }

  .file-name {
    font-size: 13px;
    font-weight: 500;
    color: #2563eb;
    text-decoration: none;
  }

  .file-name:hover {
    text-decoration: underline;
  }

  .file-size {
    font-size: 11px;
    color: #6b7280;
  }
</style>
