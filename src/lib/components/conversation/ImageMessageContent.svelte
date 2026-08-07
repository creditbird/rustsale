<script lang="ts">
  import type { ConversationAttachment } from '$lib/services/conversationService';
  import { safeConversationUrl } from './messageMedia';

  let {
    attachment,
    onImageClick = () => {}
  }: {
    attachment: ConversationAttachment;
    onImageClick?: (url: string) => void;
  } = $props();
  let safeUrl = $derived(safeConversationUrl(attachment.remoteUrl));
</script>

{#if safeUrl}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="image-message-content cursor-pointer transition-transform hover:scale-[1.02]"
    data-testid="image-message-content"
    onclick={() => onImageClick(safeUrl)}
  >
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
