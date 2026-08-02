<script lang="ts">
  import type { ConversationAttachment } from '$lib/services/conversationService';
  import { formatMediaDuration, safeConversationUrl } from './messageMedia';

  let { attachment }: { attachment: ConversationAttachment } = $props();
  let safeUrl = $derived(safeConversationUrl(attachment.remoteUrl));
</script>

{#if safeUrl}
  <div class="space-y-1 rounded-lg border border-border/30 bg-background/30 p-2" data-testid="audio-message-content">
    <audio src={safeUrl} controls preload="metadata" class="max-w-full">
      <track kind="captions" />
    </audio>
    <div class="flex items-center justify-between gap-3 text-[11px] opacity-70">
      <span class="truncate">{attachment.fileName || 'Âm thanh'}</span>
      {#if attachment.durationSeconds}
        <span>{formatMediaDuration(attachment.durationSeconds)}</span>
      {/if}
    </div>
  </div>
{:else}
  <div class="text-xs italic text-muted-foreground">[Âm thanh không khả dụng]</div>
{/if}
