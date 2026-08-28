<script lang="ts">
  import type { ConversationAttachment } from '$lib/services/conversationService';
  import { downloadConversationAttachment } from '$lib/services/conversationService';
  import { safeConversationUrl, formatBytes, isEncryptedWhatsAppMediaUrl } from './messageMedia';
  import { ImageIcon, Expand, Loader2, ArrowDownToLine, AlertCircle } from 'lucide-svelte';

  let {
    attachment,
    onImageClick = () => {}
  }: {
    attachment: ConversationAttachment;
    onImageClick?: (url: string) => void;
  } = $props();

  let remoteUrl = $derived(safeConversationUrl(attachment.remoteUrl));
  let thumbnailUrl = $derived(safeConversationUrl(attachment.thumbnailUrl));
  let imageError = $state(false);
  let isDownloading = $state(false);
  let downloadError = $state<string | null>(null);
  let downloadErrorCode = $state<string | null>(null);
  let localResolvedUrl = $state<string | null>(null);
  let pendingOpen = $state(false);

  type NormalizedDownloadError = {
    code: string | null;
    message: string;
  };

  function errorObject(value: unknown): NormalizedDownloadError | null {
    if (typeof value !== 'object' || value === null) return null;
    const record = value as Record<string, unknown>;
    const code = typeof record.code === 'string' ? record.code : null;
    const message = typeof record.message === 'string' ? record.message.trim() : '';
    if (!code && !message) return null;
    return {
      code,
      message: message || 'Không thể tải ảnh'
    };
  }

  function normalizeDownloadError(error: unknown): NormalizedDownloadError {
    const direct = errorObject(error);
    if (direct) return direct;
    if (error instanceof Error) {
      return { code: null, message: error.message || 'Không thể tải ảnh' };
    }
    if (typeof error === 'string') {
      try {
        const parsed = errorObject(JSON.parse(error));
        if (parsed) return parsed;
      } catch {
        // Tauri can also reject with a plain string; preserve it verbatim below.
      }
      return { code: null, message: error.trim() || 'Không thể tải ảnh' };
    }
    return { code: null, message: 'Không thể tải ảnh' };
  }

  // WhatsApp CDN references contain encrypted bytes. Neither a full-media URL
  // nor a legacy thumbnail URL from that CDN is a browser-renderable image.
  let encryptedRemoteUrl = $derived(isEncryptedWhatsAppMediaUrl(attachment.remoteUrl));
  let encryptedThumbnailUrl = $derived(isEncryptedWhatsAppMediaUrl(attachment.thumbnailUrl));
  let isWhatsAppEncrypted = $derived(encryptedRemoteUrl || encryptedThumbnailUrl);

  // Check if we need to download the full media payload.
  // For WhatsApp attachments, if remoteUrl is an encrypted CDN URL and we don't have
  // localResolvedUrl, the current displayUrl is only the low-res protobuf/staging thumbnail.
  // localPath is intentionally not exposed to the webview; the backend resolves it safely.
  let isFullMediaDownloaded = $derived(Boolean(localResolvedUrl));
  let needsFullMediaDownload = $derived(
    Boolean(encryptedRemoteUrl && !isFullMediaDownloaded)
  );

  let displayUrl = $derived(
    localResolvedUrl ||
      (!encryptedThumbnailUrl && !imageError && thumbnailUrl ? thumbnailUrl : null) ||
      (!encryptedRemoteUrl && !imageError && remoteUrl ? remoteUrl : null)
  );

  let isRetryable = $derived(
    downloadErrorCode
      ? ['provider_failure', 'account_disconnected', 'provider_rate_limited'].includes(
          downloadErrorCode
        )
      : !downloadError?.includes('khóa giải mã') &&
          !downloadError?.includes('không còn khóa') &&
          !downloadError?.includes('yêu cầu người gửi gửi lại')
  );

  async function triggerDownload(openAfterDownload = false) {
    if (isFullMediaDownloaded && displayUrl) {
      if (openAfterDownload) {
        onImageClick(displayUrl);
      }
      return;
    }
    if (isDownloading) {
      if (openAfterDownload) {
        pendingOpen = true;
      }
      return;
    }
    if (!attachment.id) {
      if (openAfterDownload && displayUrl) {
        onImageClick(displayUrl);
      }
      return;
    }
    if (downloadError && !isRetryable) {
      if (openAfterDownload && displayUrl) {
        onImageClick(displayUrl);
      }
      return;
    }

    isDownloading = true;
    downloadError = null;
    downloadErrorCode = null;
    if (openAfterDownload) {
      pendingOpen = true;
    }
    try {
      const updated = await downloadConversationAttachment(attachment.id);
      if (updated.thumbnailUrl) {
        localResolvedUrl = updated.thumbnailUrl;
        if (openAfterDownload || pendingOpen) {
          onImageClick(updated.thumbnailUrl);
        }
      } else if (openAfterDownload || pendingOpen) {
        if (displayUrl) {
          onImageClick(displayUrl);
        } else {
          downloadError = 'Không thể tải ảnh';
        }
      }
    } catch (error: unknown) {
      const normalizedError = normalizeDownloadError(error);
      downloadError = normalizedError.message;
      downloadErrorCode = normalizedError.code;
      if ((openAfterDownload || pendingOpen) && displayUrl) {
        onImageClick(displayUrl);
      }
    } finally {
      isDownloading = false;
      pendingOpen = false;
    }
  }

  // Automatically attempt download/preview resolution if no thumbnail yet or if full resolution is pending
  $effect(() => {
    if ((!displayUrl || needsFullMediaDownload) && !isDownloading && !downloadError && attachment.id) {
      triggerDownload(false);
    }
  });

  function handleImageError() {
    imageError = true;
  }

  function handleClick() {
    if (needsFullMediaDownload && !isFullMediaDownloaded) {
      triggerDownload(true);
    } else if (displayUrl) {
      onImageClick(displayUrl);
    }
  }
</script>

{#if displayUrl}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="image-message-content group relative cursor-pointer overflow-hidden rounded-xl border border-border/30 bg-muted/20 shadow-sm transition-all duration-200 hover:border-border/60 hover:shadow-md"
    data-testid="image-message-content"
    onclick={handleClick}
  >
    <img
      src={displayUrl}
      alt={attachment.fileName ?? 'Image'}
      loading="lazy"
      onerror={handleImageError}
      class="message-image block min-w-[180px] min-h-[120px] max-h-[300px] max-w-[320px] w-auto h-auto rounded-xl object-contain bg-black/5 dark:bg-white/5 transition-transform duration-200 group-hover:scale-[1.02]"
    />
    {#if isDownloading}
      <div
        class="absolute inset-0 flex items-center justify-center bg-black/30 backdrop-blur-[1px]"
      >
        <div class="flex items-center gap-1.5 rounded-full bg-black/70 px-3 py-1 text-xs text-white shadow-lg backdrop-blur-sm">
          <Loader2 class="size-3.5 animate-spin" />
          <span>Đang tải HD...</span>
        </div>
      </div>
    {:else}
      <div
        class="absolute inset-0 flex items-center justify-center bg-black/25 opacity-0 backdrop-blur-[1px] transition-opacity duration-200 group-hover:opacity-100"
      >
        <div class="flex size-9 items-center justify-center rounded-full bg-black/60 text-white shadow-lg backdrop-blur-sm">
          <Expand class="size-4" />
        </div>
      </div>
    {/if}
  </div>
{:else}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="image-fallback flex max-w-[260px] cursor-pointer items-center justify-between gap-2.5 rounded-xl border border-border/50 bg-muted/40 p-3 text-xs transition-colors hover:bg-muted/60"
    data-testid="image-message-fallback"
    onclick={() => triggerDownload(true)}
  >
    <div class="flex items-center gap-2.5 min-w-0">
      <div class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
        {#if isDownloading}
          <Loader2 class="size-5 animate-spin" />
        {:else if downloadError}
          <AlertCircle class="size-5 text-destructive" />
        {:else}
          <ImageIcon class="size-5" />
        {/if}
      </div>
      <div class="flex min-w-0 flex-col gap-0.5">
        <span class="truncate font-medium text-foreground">{attachment.fileName ?? 'Hình ảnh'}</span>
        <span class="text-[11px] text-muted-foreground">
          {#if isDownloading}
            Đang tải...
          {:else if downloadError}
            {downloadError}{#if isRetryable} • Thử lại{/if}
          {:else}
            {attachment.sizeBytes ? formatBytes(attachment.sizeBytes) : 'Nhấn để tải'}
          {/if}
        </span>
      </div>
    </div>
    {#if !isDownloading && isWhatsAppEncrypted && isRetryable}
      <div class="flex size-7 shrink-0 items-center justify-center rounded-full bg-background/80 text-muted-foreground shadow-xs hover:text-foreground">
        <ArrowDownToLine class="size-3.5" />
      </div>
    {/if}
  </div>
{/if}
