<script lang="ts">
  import type { ConversationAttachment } from '$lib/services/conversationService';
  import { formatMediaDuration, safeConversationUrl } from './messageMedia';

  let { attachment }: { attachment: ConversationAttachment } = $props();
  let safeUrl = $derived(safeConversationUrl(attachment.remoteUrl));
  let safePoster = $derived(safeConversationUrl(attachment.thumbnailUrl));
</script>

{#if safeUrl}
  <div class="space-y-1" data-testid="video-message-content">
    <video
      src={safeUrl}
      poster={safePoster ?? undefined}
      controls
      preload="metadata"
      class="max-h-80 max-w-full rounded-lg border border-border/30 bg-black"
    >
      <track kind="captions" />
    </video>
    {#if attachment.durationSeconds}
      <div class="text-[11px] opacity-70">{formatMediaDuration(attachment.durationSeconds)}</div>
    {/if}
  </div>
{:else}
  <div class="text-xs italic text-muted-foreground">[Video unavailable]</div>
{/if}
