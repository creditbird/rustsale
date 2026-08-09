<script lang="ts">
  import type { ConversationAttachment } from '$lib/services/conversationService';
  import { formatBytes, safeConversationUrl, openExternalUrl } from './messageMedia';

  let { attachment }: { attachment: ConversationAttachment } = $props();
  let safeUrl = $derived(safeConversationUrl(attachment.remoteUrl));
</script>

<div class="file-message-content flex items-center gap-2 rounded-lg border border-border/40 bg-muted/20 p-2" data-testid="file-message-content">
  <div class="file-icon text-2xl">📄</div>
  <div class="file-details flex flex-col">
    {#if safeUrl}
      <a
        href={safeUrl}
        target="_blank"
        rel="noopener noreferrer"
        class="file-name text-[13px] font-medium text-primary no-underline hover:underline"
        onclick={(e) => {
          e.stopPropagation();
          e.preventDefault();
          void openExternalUrl(safeUrl);
        }}
      >
        {attachment.fileName ?? 'Tập tin'}
      </a>
    {:else}
      <span class="file-name text-[13px] font-medium text-foreground">{attachment.fileName ?? 'Tập tin'}</span>
    {/if}
    <span class="file-size text-[11px] text-muted-foreground">{formatBytes(attachment.sizeBytes)}</span>
  </div>
</div>

