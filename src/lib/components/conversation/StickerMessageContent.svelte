<script lang="ts">
  import type { ConversationAttachment } from '$lib/services/conversationService';
  import { getZaloStickerDetail } from '$lib/services/conversationService';
  import { extractStickerUrl } from './messageMedia';

  let {
    attachment,
    content,
    conversationId
  }: {
    attachment?: ConversationAttachment | null;
    content?: Record<string, unknown> | null;
    conversationId?: number | null;
  } = $props();

  let initialUrl = $derived(extractStickerUrl(attachment, content));
  let resolvedUrl = $state<string | null>(null);
  let safeUrl = $derived(initialUrl ?? resolvedUrl);
  let hasError = $state(false);

  $effect(() => {
    if (!initialUrl && !resolvedUrl && conversationId) {
      const stickerId = extractStickerId(attachment, content);
      if (stickerId !== null) {
        getZaloStickerDetail(conversationId, stickerId)
          .then((ref) => {
            if (ref?.url) resolvedUrl = ref.url;
          })
          .catch(() => {});
      }
    }
  });

  function extractStickerId(
    att?: ConversationAttachment | null,
    cnt?: Record<string, unknown> | null
  ): number | null {
    const rawId = cnt?.stickerId ?? cnt?.id ?? cnt?.sticker_id ?? att?.providerAttachmentId;
    if (typeof rawId === 'number') return rawId;
    if (typeof rawId === 'string' && /^\d+$/.test(rawId)) return parseInt(rawId, 10);
    return null;
  }
</script>

{#if safeUrl && !hasError}
  <div class="sticker-message-content" data-testid="sticker-message-content">
    <img
      src={safeUrl}
      alt="Sticker"
      class="h-[120px] w-[120px] object-contain"
      onerror={() => (hasError = true)}
    />
  </div>
{:else}
  <div class="inline-flex items-center gap-1.5 rounded-lg bg-muted/60 px-3 py-1.5 text-xs font-medium text-muted-foreground border border-border/40" data-testid="sticker-fallback-badge">
    <span class="text-sm">🎭</span>
    <span>[Sticker]</span>
  </div>
{/if}
