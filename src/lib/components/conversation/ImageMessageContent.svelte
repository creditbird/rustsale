<script lang="ts">
  import type { ConversationAttachment } from '$lib/services/conversationService';
  import { safeConversationUrl } from './messageMedia';

  let { attachment }: { attachment: ConversationAttachment } = $props();
  let safeUrl = $derived(safeConversationUrl(attachment.remoteUrl || attachment.localPath));
</script>

{#if safeUrl}
  <div class="image-message-content" data-testid="image-message-content">
    <img
      src={safeUrl}
      alt={attachment.fileName ?? 'Hình ảnh'}
      loading="lazy"
      class="message-image"
    />
  </div>
{:else}
  <div class="image-fallback">[Hình ảnh không khả dụng]</div>
{/if}

<style>
  .message-image {
    max-width: 260px;
    max-height: 260px;
    border-radius: 8px;
    object-fit: cover;
  }
  .image-fallback {
    font-size: 12px;
    color: #6b7280;
    font-style: italic;
  }
</style>
