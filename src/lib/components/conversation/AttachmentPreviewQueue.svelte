<script lang="ts">
  import type { ComposerAttachmentItem } from './composerDraftState';
  import { formatBytes, safeImagePreviewDataUrl } from './messageMedia';

  let {
    items = [],
    onRemove
  }: {
    items?: ComposerAttachmentItem[];
    onRemove?: (item: ComposerAttachmentItem) => void;
  } = $props();

  function handleRemove(item: ComposerAttachmentItem) {
    if (onRemove) {
      onRemove(item);
    }
  }
</script>

{#if items.length > 0}
  <div class="attachment-preview-queue flex flex-wrap gap-2 border-t border-border/40 bg-muted/30 p-2" data-testid="attachment-preview-queue">
    {#each items as item (item.localId)}
      {@const safePreview = safeImagePreviewDataUrl(item.previewDataUrl)}
      <div
        class="attachment-chip inline-flex max-w-[200px] items-center gap-2 rounded-md border border-border bg-background p-1 px-2 text-xs data-[status=error]:border-destructive data-[status=error]:bg-destructive/10"
        data-status={item.status}
        data-testid="attachment-item"
      >
        {#if safePreview}
          <img src={safePreview} alt={item.fileName} class="chip-thumbnail h-7 w-7 rounded object-cover" />
        {:else}
          <div class="chip-icon text-base">
            {#if item.attachmentType === 'image'}
              🖼️
            {:else if item.attachmentType === 'video'}
              🎥
            {:else if item.attachmentType === 'audio'}
              🎵
            {:else}
              📄
            {/if}
          </div>
        {/if}

        <div class="chip-info flex flex-col overflow-hidden">
          <span class="chip-filename truncate font-medium text-foreground" title={item.fileName}>{item.fileName}</span>
          <span class="chip-size text-[10px] text-muted-foreground">{formatBytes(item.sizeBytes)}</span>
        </div>

        {#if item.status === 'staging'}
          <span class="chip-status staging text-[10px] font-semibold text-blue-500" data-testid="status-staging">Staging...</span>
        {:else if item.status === 'error'}
          <span class="chip-status error text-[10px] font-semibold text-destructive" title={item.errorMessage ?? 'Error'} data-testid="status-error">
            Failed
          </span>
        {:else if item.status === 'removing'}
          <span class="chip-status staging text-[10px] font-semibold text-blue-500" data-testid="status-removing">Removing...</span>
        {/if}

        <button
          type="button"
          class="chip-remove-btn cursor-pointer rounded border-none bg-transparent px-1 py-[2px] text-xs text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
          aria-label={`Remove ${item.fileName}`}
          onclick={() => handleRemove(item)}
          data-testid="remove-attachment-btn"
        >
          ✕
        </button>
      </div>
    {/each}
  </div>
{/if}

