<script lang="ts">
  import type { ConversationMessage, MessageStatus } from '$lib/services/conversationService';
  import MessageReplyPreview from './MessageReplyPreview.svelte';
  import MessageContentRenderer from './MessageContentRenderer.svelte';

  let {
    message,
    isGroup = false
  }: {
    message: ConversationMessage;
    isGroup?: boolean;
  } = $props();

  function formatTimestamp(isoString: string): string {
    try {
      const date = new Date(isoString);
      if (isNaN(date.getTime())) return isoString;
      return new Intl.DateTimeFormat('vi-VN', {
        dateStyle: 'short',
        timeStyle: 'short'
      }).format(date);
    } catch {
      return isoString;
    }
  }

  function getStatusLabel(status: MessageStatus): string {
    switch (status) {
      case 'queued':
        return 'Đang chờ';
      case 'sending':
        return 'Đang gửi';
      case 'sent':
        return 'Đã gửi';
      case 'delivered':
        return 'Đã phát';
      case 'read':
        return 'Đã xem';
      case 'failed':
        return 'Thất bại';
      default:
        return status;
    }
  }

  function isValidMediaUrl(url: unknown): boolean {
    if (typeof url !== 'string') return false;
    const trimmed = url.trim();
    if (!trimmed) return false;
    if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) return true;
    if (trimmed.startsWith('asset://') || trimmed.startsWith('asset:')) return true;
    if (
      trimmed.startsWith('http://asset.localhost') ||
      trimmed.startsWith('http://tauri.localhost') ||
      trimmed.startsWith('tauri:')
    ) {
      return true;
    }
    if (trimmed.startsWith('data:')) {
      return true;
    }
    if (
      trimmed.startsWith('/') ||
      trimmed.startsWith('./') ||
      trimmed.startsWith('../') ||
      trimmed.startsWith('file://') ||
      trimmed.startsWith('file:') ||
      trimmed.startsWith('blob:')
    ) {
      return true;
    }
    return false;
  }

  let isOutbound = $derived(message.direction === 'outbound');
  let isRemoved = $derived(message.status === 'removed' || message.removedAt !== null);

  let mediaUrl = $derived(
    typeof message.content?.url === 'string'
      ? message.content.url
      : typeof message.content?.src === 'string'
        ? message.content.src
        : typeof message.content?.href === 'string'
          ? message.content.href
          : typeof message.content?.path === 'string'
            ? message.content.path
            : null
  );

  let isEmail = $derived(message.contentType === 'email');
  let isImage = $derived(
    message.contentType === 'image' || message.contentType.startsWith('image/')
  );
  let isText = $derived(message.contentType === 'text');

  let displayText = $derived(
    typeof message.textContent === 'string' && message.textContent.trim() !== ''
      ? message.textContent
      : typeof message.content?.caption === 'string' && message.content.caption.trim() !== ''
        ? message.content.caption
        : typeof message.content?.description === 'string' && message.content.description.trim() !== ''
          ? message.content.description
          : typeof message.content?.title === 'string' && message.content.title.trim() !== ''
            ? message.content.title
            : typeof message.content?.text === 'string' && message.content.text.trim() !== ''
              ? message.content.text
              : typeof message.content?.message === 'string' && message.content.message.trim() !== ''
                ? message.content.message
                : typeof message.content?.subTitle === 'string' && message.content.subTitle.trim() !== ''
                  ? message.content.subTitle
                  : null
  );

  let emailSubject = $derived(
    typeof message.content?.subject === 'string' ? message.content.subject : null
  );
  let emailFrom = $derived(
    typeof message.content?.from === 'string' ? message.content.from : null
  );
  let emailTo = $derived(
    Array.isArray(message.content?.to)
      ? message.content.to.join(', ')
      : typeof message.content?.to === 'string'
        ? message.content.to
        : null
  );
</script>

<div
  class="flex w-full my-1.5 {isOutbound ? 'justify-end' : 'justify-start'}"
  data-message-id={message.id}
>
  <div
    class="max-w-[80%] sm:max-w-[70%] rounded-2xl p-3 shadow-xs transition-colors text-sm font-sans space-y-1.5 {isOutbound
      ? 'bg-primary text-primary-foreground rounded-tr-xs'
      : 'bg-muted/60 text-foreground border border-border/50 rounded-tl-xs'}"
  >
    <!-- Header info: Sender name / Channel / Account -->
    <div class="flex items-center gap-2 text-xs opacity-75 font-medium flex-wrap">
      {#if !isOutbound && (isGroup || message.senderDisplayName)}
        <span class="font-semibold text-foreground/90" data-slot="sender-name">
          {message.senderDisplayName || 'Khách hàng'}
        </span>
      {/if}

      {#if message.channel}
        <span class="capitalize px-1.5 py-0.5 rounded-sm bg-background/20 text-[10px] uppercase tracking-wider font-semibold">
          {message.channel}
        </span>
      {/if}

      {#if message.accountDisplayName}
        <span class="truncate max-w-[150px]" title={message.accountDisplayName}>
          {message.accountDisplayName}
        </span>
      {/if}
    </div>

    <!-- Message Content Section -->
    {#if isRemoved}
      <!-- Tombstone -->
      <div class="flex items-center gap-2 italic opacity-70 py-1" data-slot="tombstone">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="lucide lucide-trash-2"
        >
          <path d="3 6h18" />
          <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
          <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
        </svg>
        <span>Tin nhắn đã bị xóa</span>
      </div>
    {:else if isEmail}
      <!-- Email Card -->
      <div class="rounded-lg border border-border/40 p-2.5 bg-background/30 text-foreground space-y-1.5 text-xs">
        {#if emailSubject}
          <div class="font-bold text-sm text-foreground">
            {emailSubject}
          </div>
        {/if}
        {#if emailFrom}
          <div>
            <span class="font-semibold text-muted-foreground">Từ:</span> {emailFrom}
          </div>
        {/if}
        {#if emailTo}
          <div>
            <span class="font-semibold text-muted-foreground">Đến:</span> {emailTo}
          </div>
        {/if}
        {#if displayText}
          <div class="mt-2 text-sm whitespace-pre-wrap font-normal border-t border-border/30 pt-2">
            {displayText}
          </div>
        {/if}
      </div>
    {:else}
      {#if message.replyTarget}
        <MessageReplyPreview replyTarget={message.replyTarget} />
      {/if}
      <MessageContentRenderer {message} />
    {/if}

    <!-- Footer info: Timestamp & Status (for outbound) -->
    <div class="flex items-center justify-end gap-1.5 text-[11px] opacity-70 mt-1 select-none">
      <span>{formatTimestamp(message.occurredAt)}</span>
      {#if isOutbound && !isRemoved}
        <span class="font-medium" data-slot="status-label">
          · {getStatusLabel(message.status)}
        </span>
      {/if}
    </div>
  </div>
</div>
