<script lang="ts">
  import type {
    ConversationCapabilities,
    ConversationMessage,
    ConversationReactionCode,
    ConversationReactionGroup,
    ConversationReactionMutationResult,
    MessageStatus
  } from '$lib/services/conversationService';
  import MessageReplyPreview from './MessageReplyPreview.svelte';
  import MessageContentRenderer from './MessageContentRenderer.svelte';
  import MessageActionsMenu from './MessageActionsMenu.svelte';
  import MessageReactionSummary from './MessageReactionSummary.svelte';
  import MessageReactionPicker from './MessageReactionPicker.svelte';
  import { safeConversationUrl } from './messageMedia';
  import { reactionCodeToIcon, reactionIconToCode } from './reactionDisplay';
  import * as Popover from '$lib/components/ui/popover';
  import { Smile, Check, CheckCheck, Clock, AlertCircle, Pin } from 'lucide-svelte';

  let {
    message,
    isGroup = false,
    showStatus = true,
    capabilities,
    pendingReactions = [],
    isPinned = false,
    onReply = () => {},
    onRecall = () => {},
    onForward = () => {},
    onPin = () => {},
    onToggleReaction = () => Promise.resolve(null),
    onImageClick = () => {},
    onUserClick = () => {}
  }: {
    message: ConversationMessage;
    isGroup?: boolean;
    showStatus?: boolean;
    capabilities?: ConversationCapabilities;
    pendingReactions?: ConversationReactionCode[];
    isPinned?: boolean;
    onReply?: (message: ConversationMessage) => void;
    onRecall?: (message: ConversationMessage) => void;
    onForward?: (message: ConversationMessage) => void;
    onPin?: (message: ConversationMessage) => void;
    onToggleReaction?: (
      message: ConversationMessage,
      code: ConversationReactionCode,
      action?: 'increment' | 'remove'
    ) => Promise<ConversationReactionMutationResult | null>;
    onImageClick?: (url: string, message: ConversationMessage) => void;
    onUserClick?: (payload: {
      identityId: number | null;
      displayName: string | null;
      externalId?: string;
      targetEl?: HTMLElement;
    }) => void;
  } = $props();

  let actionsOpen = $state(false);
  let showPicker = $state(false);
  let isHovered = $state(false);
  let optimisticPickerIncrements = $state(new Map<ConversationReactionCode, number>());

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
        return message.errorMessage ? `Thất bại: ${message.errorMessage}` : 'Thất bại';
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

  let displayReactions = $derived.by(() => {
    if (optimisticPickerIncrements.size === 0) return message.reactions;

    const mergedGroups: ConversationReactionGroup[] = [];
    const mergedCodes = new Set<ConversationReactionCode>();

    for (const group of message.reactions) {
      const code = reactionIconToCode(group.reactionIcon);
      const optimisticCount = code === null ? 0 : (optimisticPickerIncrements.get(code) ?? 0);
      if (code !== null) mergedCodes.add(code);
      mergedGroups.push(
        optimisticCount > 0 ? { ...group, count: group.count + optimisticCount } : group
      );
    }

    for (const [code, count] of optimisticPickerIncrements) {
      if (count <= 0 || mergedCodes.has(code)) continue;
      mergedGroups.push({
        reactionIcon: reactionCodeToIcon(code),
        count,
        includesMe: true,
        actors: []
      });
    }

    return mergedGroups;
  });

  let activeReactionCodes = $derived.by(() => {
    const codes: ConversationReactionCode[] = [];
    for (const group of displayReactions) {
      if (group.includesMe) {
        const code = reactionIconToCode(group.reactionIcon);
        if (code !== null && !codes.includes(code)) codes.push(code);
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

  function handleSelectReaction(code: ConversationReactionCode, action: 'increment' | 'remove') {
    showPicker = false;
    if (action === 'remove') {
      void onToggleReaction(message, code, action);
      return;
    }

    const next = new Map(optimisticPickerIncrements);
    next.set(code, (next.get(code) ?? 0) + 1);
    optimisticPickerIncrements = next;

    const mutation = onToggleReaction(message, code, action);
    void mutation
      .finally(() => {
        const currentCount = optimisticPickerIncrements.get(code) ?? 0;
        if (currentCount <= 0) return;
        const remaining = new Map(optimisticPickerIncrements);
        if (currentCount === 1) remaining.delete(code);
        else remaining.set(code, currentCount - 1);
        optimisticPickerIncrements = remaining;
      })
      .catch(() => {});
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
    class="relative max-w-[82%] sm:max-w-[72%] rounded-2xl px-3.5 py-2.5 transition-colors text-[13.5px] leading-relaxed font-sans space-y-1.5 shadow-[0_1px_0.5px_rgba(11,20,26,0.13)] {isOutbound
      ? 'bg-[#d9fdd3] dark:bg-[#005c4b] text-[#111b21] dark:text-[#e9edef] border border-[#b7e4b1]/60 dark:border-[#005c4b]/80 rounded-tr-xs font-normal'
      : 'bg-white dark:bg-[#202c33] text-[#111b21] dark:text-[#e9edef] border border-slate-200/80 dark:border-slate-700/60 rounded-tl-xs font-normal'}"
  >
    <!-- Header info: Sender name / Channel / Account -->
    <div class="flex items-start justify-between gap-2">
      <div class="flex items-center gap-2 text-xs opacity-90 font-medium flex-wrap">
        {#if !isOutbound && (isGroup || message.senderDisplayName) && message.senderDisplayName !== message.accountDisplayName}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <span
            class="font-bold text-[#075e54] dark:text-[#25d366] cursor-pointer hover:underline hover:opacity-80 transition-all select-none"
            data-slot="sender-name"
            onclick={(e) => {
              e.stopPropagation();
              onUserClick({
                identityId: message.senderIdentityId,
                displayName: message.senderDisplayName,
                externalId: message.senderExternalId || undefined,
                targetEl: e.currentTarget as HTMLElement
              });
            }}
          >
            {message.senderDisplayName}
          </span>
        {/if}

        {#if message.channel}
          {@const ch = message.channel.toLowerCase()}
          <span class={ch === 'zalo'
            ? 'px-1.5 py-0.5 rounded-md bg-blue-500/15 text-blue-700 dark:text-blue-300 border border-blue-400/30 text-[10px] uppercase tracking-wider font-bold'
            : ch === 'whatsapp'
              ? 'px-1.5 py-0.5 rounded-md bg-emerald-500/15 text-emerald-700 dark:text-emerald-300 border border-emerald-400/30 text-[10px] uppercase tracking-wider font-bold'
              : ch === 'messenger'
                ? 'px-1.5 py-0.5 rounded-md bg-purple-500/15 text-purple-700 dark:text-purple-300 border border-purple-400/30 text-[10px] uppercase tracking-wider font-bold'
                : 'px-1.5 py-0.5 rounded-md bg-rose-500/15 text-rose-700 dark:text-rose-300 border border-rose-400/30 text-[10px] uppercase tracking-wider font-bold'}>
            {message.channel}
          </span>
        {/if}

        {#if message.accountDisplayName}
          <span class="truncate max-w-[150px] opacity-75" title={message.accountDisplayName}>
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
              class="grid size-7 place-items-center rounded-full text-muted-foreground transition-opacity hover:bg-muted/50 hover:text-foreground focus:opacity-100 group-hover/bubble:opacity-100 {showPicker ? 'opacity-100 bg-muted/50 text-foreground' : 'opacity-0'}"
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

        <button
          type="button"
          aria-label={isPinned ? 'Bỏ ghim tin nhắn' : 'Ghim tin nhắn'}
          title={isPinned ? 'Bỏ ghim tin nhắn' : 'Ghim tin nhắn'}
          class="grid size-7 place-items-center rounded-full text-muted-foreground transition-opacity hover:bg-muted/50 hover:text-foreground focus:opacity-100 group-hover/bubble:opacity-100 {isPinned ? 'opacity-100 text-amber-500 bg-amber-500/10' : 'opacity-0'}"
          onclick={() => onPin(message)}
        >
          <Pin class="size-3.5 {isPinned ? 'fill-amber-500' : ''}" />
        </button>

        <MessageActionsMenu
          bind:open={actionsOpen}
          {canReply}
          {canCopy}
          {canRecall}
          {canReact}
          {canForward}
          {isPinned}
          onReply={() => onReply(message)}
          onCopy={() => void handleCopy()}
          onRecall={() => onRecall(message)}
          onForward={() => onForward(message)}
          onPin={() => onPin(message)}
          onToggleReaction={(code) => handleSelectReaction(code, 'increment')}
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
      <MessageContentRenderer
        {message}
        onImageClick={(url) => onImageClick(url, message)}
        onMentionClick={(identityId, mentionText) => {
          const cleanName = mentionText.startsWith('@') ? mentionText.slice(1) : mentionText;
          onUserClick({
            identityId,
            displayName: cleanName
          });
        }}
      />
    {/if}

    {#if !isRemoved && displayReactions.length > 0}
      <MessageReactionSummary
        groups={displayReactions}
        {canReact}
        {pendingReactions}
        onToggleReaction={(code, action) => onToggleReaction(message, code, action)}
      />
    {/if}

    <!-- Footer info: Timestamp & Status (for outbound) -->
    <div class="flex items-center justify-end gap-1.5 text-[11px] opacity-70 mt-1 select-none font-medium">
      <span>{formatTimestamp(message.occurredAt)}</span>
      {#if showStatus && isOutbound && !isRemoved}
        <span class="inline-flex items-center gap-0.5 font-medium" data-slot="status-label" title={getStatusLabel(message.status)}>
          {#if message.status === 'read'}
            <CheckCheck class="size-3.5 text-[#53bdeb] font-bold" />
          {:else if message.status === 'delivered'}
            <CheckCheck class="size-3.5 text-[#667781] dark:text-[#8696a0]" />
          {:else if message.status === 'sent'}
            <Check class="size-3.5 text-[#667781] dark:text-[#8696a0]" />
          {:else if message.status === 'sending' || message.status === 'queued'}
            <Clock class="size-3 text-muted-foreground animate-spin" />
          {:else if message.status === 'failed'}
            <AlertCircle class="size-3.5 text-destructive font-bold" />
          {/if}
          <span class="sr-only">{getStatusLabel(message.status)}</span>
        </span>
      {/if}

    </div>

  </div>
</div>
