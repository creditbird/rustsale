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
      alt={attachment.fileName ?? 'Image'}
      loading="lazy"
      class="message-image max-h-[260px] max-w-[260px] rounded-lg object-cover"
    />
  </div>
{:else}
  <div class="image-fallback text-xs italic text-muted-foreground">[Image unavailable]</div>
{/if}

