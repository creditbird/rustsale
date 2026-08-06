<script lang="ts">
  import type {
    ConversationCapabilities,
    ConversationMessage,
    ConversationReactionCode,
    MessageStatus
  } from '$lib/services/conversationService';
  import MessageReplyPreview from './MessageReplyPreview.svelte';
  import MessageContentRenderer from './MessageContentRenderer.svelte';
  import MessageActionsMenu from './MessageActionsMenu.svelte';
  import MessageReactionSummary from './MessageReactionSummary.svelte';
  import MessageReactionPicker from './MessageReactionPicker.svelte';
  import { safeConversationUrl } from './messageMedia';
  import { reactionIconToCode } from './reactionDisplay';
  import * as Popover from '$lib/components/ui/popover';
  import { Smile } from 'lucide-svelte';

  let {
    message,
    isGroup = false,
    showStatus = true,
    capabilities,
    pendingReactions = [],
    onReply = () => {},
    onRecall = () => {},
    onForward = () => {},
    onToggleReaction = () => {}
  }: {
    message: ConversationMessage;
    isGroup?: boolean;
    showStatus?: boolean;
    capabilities?: ConversationCapabilities;
    pendingReactions?: ConversationReactionCode[];
    onReply?: (message: ConversationMessage) => void;
    onRecall?: (message: ConversationMessage) => void;
    onForward?: (message: ConversationMessage) => void;
    onToggleReaction?: (
      message: ConversationMessage,
      code: ConversationReactionCode,
      currentActive?: boolean
    ) => void;
  } = $props();

  let actionsOpen = $state(false);
  let showPicker = $state(false);
  let isHovered = $state(false);

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

  let copyValue = $derived.by(() => {
    if (message.textContent?.trim()) return message.textContent.trim();
    const contentUrl = safeConversationUrl(mediaUrl);
    if (contentUrl) return contentUrl;
    for (const attachment of message.attachments) {
      const attachmentUrl = safeConversationUrl(attachment.remoteUrl);
      if (attachmentUrl) return attachmentUrl;
    }
    return null;
  });

  let canReply = $derived(!isRemoved && (message.actions?.reply ?? true));
  let canCopy = $derived(
    !isRemoved && Boolean(message.actions?.copy ?? true) && copyValue !== null
  );
  let withinRecallWindow = $derived.by(() => {
    const windowSeconds = capabilities?.recallWindowSeconds;
    if (windowSeconds === null || windowSeconds === undefined) return true;
    const occurredAt = new Date(message.occurredAt).getTime();
    if (!Number.isFinite(occurredAt)) return false;
    return Date.now() - occurredAt <= windowSeconds * 1000;
  });
  let canRecall = $derived(
    isOutbound &&
      !isRemoved &&
      Boolean(message.actions?.recall ?? true) &&
      ['sent', 'delivered', 'read'].includes(message.status) &&
      withinRecallWindow
  );
  let canReact = $derived(
    !isRemoved && (message.actions?.react ?? capabilities?.reactions.write ?? true)
  );
  let canForward = $derived(
    !isRemoved && (message.actions?.forward ?? capabilities?.forward.enabled ?? true)
  );

  let activeReactionCodes = $derived.by(() => {
    const codes: ConversationReactionCode[] = [];
    for (const group of message.reactions) {
      if (group.includesMe) {
        const code = reactionIconToCode(group.reactionIcon);
        if (!codes.includes(code)) codes.push(code);
      }
    }
    return codes;
  });

  async function handleCopy(): Promise<void> {
    if (!copyValue || !navigator.clipboard?.writeText) return;
    await navigator.clipboard.writeText(copyValue).catch(() => {});
  }

  function registerContextMenu(node: HTMLElement) {
    const openActions = (event: MouseEvent) => {
      event.preventDefault();
      actionsOpen = true;
    };
    node.addEventListener('contextmenu', openActions);
    return {
      destroy() {
        node.removeEventListener('contextmenu', openActions);
      }
    };
  }

  function handleSelectReaction(code: ConversationReactionCode, activeState: boolean) {
    showPicker = false;
    onToggleReaction(message, code, activeState);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="group/bubble flex w-full my-1.5 {isOutbound ? 'justify-end' : 'justify-start'}"
  data-message-id={message.id}
  role="group"
  aria-label={`Tin nhắn ${message.id}`}
  use:registerContextMenu
  onmouseenter={() => (isHovered = true)}
  onmouseleave={() => {
    isHovered = false;
  }}
>
  <div
    class="relative max-w-[80%] sm:max-w-[70%] rounded-2xl p-3 shadow-xs transition-colors text-sm font-sans space-y-1.5 {isOutbound
      ? 'bg-primary text-primary-foreground rounded-tr-xs'
      : 'bg-muted/60 text-foreground border border-border/50 rounded-tl-xs'}"
  >
    <!-- Header info: Sender name / Channel / Account -->
    <div class="flex items-start justify-between gap-2">
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

      <div class="flex items-center gap-1">
        {#if canReact}
          <Popover.Root bind:open={showPicker}>
            <Popover.Trigger
              aria-label="Thả cảm xúc"
              title="Thả cảm xúc"
              class="grid size-7 place-items-center rounded-full text-muted-foreground transition-opacity hover:bg-muted hover:text-foreground focus:opacity-100 group-hover/bubble:opacity-100 {showPicker ? 'opacity-100 bg-muted text-foreground' : 'opacity-0'}"
            >
              <Smile class="size-4" />
            </Popover.Trigger>
            <Popover.Content
              side="top"
              align="start"
              sideOffset={4}
              class="w-auto rounded-full bg-transparent p-0 shadow-none ring-0"
            >
        <MessageReactionPicker
          messageId={message.id}
          activeReactions={activeReactionCodes}
          disabledReactions={pendingReactions}
          supportedReactions={capabilities?.reactions.supported ?? []}
          onSelect={handleSelectReaction}
          onClose={() => (showPicker = false)}
        />
            </Popover.Content>
          </Popover.Root>
        {/if}

        <MessageActionsMenu
          bind:open={actionsOpen}
          {canReply}
          {canCopy}
          {canRecall}
          {canReact}
          {canForward}
          onReply={() => onReply(message)}
          onCopy={() => void handleCopy()}
          onRecall={() => onRecall(message)}
          onForward={() => onForward(message)}
          onToggleReaction={(code) => onToggleReaction(message, code)}
        />
      </div>
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

    {#if !isRemoved && message.reactions.length > 0}
      <MessageReactionSummary
        groups={message.reactions}
        {canReact}
        {pendingReactions}
        onToggleReaction={(code, currentActive) => onToggleReaction(message, code, currentActive)}
      />
    {/if}

    <!-- Footer info: Timestamp & Status (for outbound) -->
    <div class="flex items-center justify-end gap-1.5 text-[11px] opacity-70 mt-1 select-none">
      <span>{formatTimestamp(message.occurredAt)}</span>
      {#if showStatus && isOutbound && !isRemoved}
        <span class="font-medium" data-slot="status-label">
          · {getStatusLabel(message.status)}
        </span>
      {/if}
    </div>
  </div>
</div>
