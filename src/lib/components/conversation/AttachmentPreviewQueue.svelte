<script lang="ts">
  import type { ComposerAttachmentItem } from './composerDraftState';

  let {
    items = [],
    onRemove
  }: {
    items?: ComposerAttachmentItem[];
    onRemove?: (item: ComposerAttachmentItem) => void;
  } = $props();

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function handleRemove(item: ComposerAttachmentItem) {
    if (onRemove) {
      onRemove(item);
    }
  }
</script>

{#if items.length > 0}
  <div class="attachment-preview-queue" data-testid="attachment-preview-queue">
    {#each items as item (item.localId)}
      <div class="attachment-chip" data-status={item.status} data-testid="attachment-item">
        {#if item.previewDataUrl}
          <img src={item.previewDataUrl} alt={item.fileName} class="chip-thumbnail" />
        {:else}
          <div class="chip-icon">
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

        <div class="chip-info">
          <span class="chip-filename" title={item.fileName}>{item.fileName}</span>
          <span class="chip-size">{formatSize(item.sizeBytes)}</span>
        </div>

        {#if item.status === 'staging'}
          <span class="chip-status staging" data-testid="status-staging">Staging...</span>
        {:else if item.status === 'error'}
          <span class="chip-status error" title={item.errorMessage ?? 'Error'} data-testid="status-error">
            Failed
          </span>
        {/if}

        <button
          type="button"
          class="chip-remove-btn"
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

<style>
  .attachment-preview-queue {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 8px 12px;
    background-color: var(--surface-secondary, #f3f4f6);
    border-top: 1px solid var(--border-color, #e5e7eb);
  }

  .attachment-chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
    background: #ffffff;
    border: 1px solid var(--border-color, #d1d5db);
    border-radius: 6px;
    font-size: 12px;
    max-width: 200px;
  }

  .attachment-chip[data-status='error'] {
    border-color: #ef4444;
    background: #fef2f2;
  }

  .chip-thumbnail {
    width: 28px;
    height: 28px;
    object-fit: cover;
    border-radius: 4px;
  }

  .chip-icon {
    font-size: 16px;
  }

  .chip-info {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .chip-filename {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 500;
    color: #374151;
  }

  .chip-size {
    font-size: 10px;
    color: #6b7280;
  }

  .chip-status {
    font-size: 10px;
    font-weight: 600;
  }

  .chip-status.staging {
    color: #3b82f6;
  }

  .chip-status.error {
    color: #ef4444;
  }

  .chip-remove-btn {
    border: none;
    background: none;
    cursor: pointer;
    color: #9ca3af;
    padding: 2px 4px;
    font-size: 12px;
    border-radius: 4px;
  }

  .chip-remove-btn:hover {
    color: #ef4444;
    background: #fee2e2;
  }
</style>
