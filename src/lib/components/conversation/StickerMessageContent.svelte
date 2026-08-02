<script lang="ts">
  import type { ConversationAttachment } from '$lib/services/conversationService';
  import { safeConversationUrl } from './messageMedia';

  let { attachment }: { attachment: ConversationAttachment } = $props();
  let safeUrl = $derived(safeConversationUrl(attachment.remoteUrl));
</script>

{#if safeUrl}
  <div class="sticker-message-content" data-testid="sticker-message-content">
    <img src={safeUrl} alt="Sticker" class="message-sticker" />
  </div>
{:else}
  <div class="sticker-fallback">[Sticker]</div>
{/if}

<style>
  .message-sticker {
    width: 120px;
    height: 120px;
    object-fit: contain;
  }
  .sticker-fallback {
    font-size: 12px;
    color: #6b7280;
  }
</style>
