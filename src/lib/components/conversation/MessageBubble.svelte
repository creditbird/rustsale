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
  import { Smile, Check, CheckCheck, Clock, AlertCircle, Pin, Undo2 } from 'lucide-svelte';
  import * as m from '$lib/paraglide/messages';

  let {
    message,
    isGroup = false,
    conversationTitle = '',
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
    conversationTitle?: string;
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
      sourceMessageId?: number;
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
        return 'Queued';
      case 'sending':
        return 'Sending';
      case 'sent':
        return 'Sent';
      case 'delivered':
        return 'Delivered';
      case 'read':
        return 'Read';
      case 'failed':
        return message.errorMessage ? `Failed: ${message.errorMessage}` : 'Failed';
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

  const SENDER_COLORS = [
    '#00a884', // Emerald / WA Green
    '#1f7aec', // Blue
    '#e542a3', // Pink / Magenta
    '#dfa600', // Amber / Gold
    '#f15c27', // Orange
    '#a633d6', // Purple
    '#029046', // Dark Green
    '#0070e0', // Sky Blue
    '#d32f2f', // Crimson
    '#00897b', // Teal
  ];

  function getSenderColor(seed: string | number | null | undefined): string {
    if (!seed) return SENDER_COLORS[0];
    const str = String(seed);
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
      hash = (hash << 5) - hash + str.charCodeAt(i);
      hash |= 0;
    }
    return SENDER_COLORS[Math.abs(hash) % SENDER_COLORS.length];
  }

  function getInitials(name: string | null | undefined): string {
    if (!name) return '?';
    const clean = name.replace(/^[~+]/, '').trim();
    if (!clean) return '?';
    if (/^\d/.test(clean)) {
      return '#';
    }
    const words = clean.split(/\s+/).filter(Boolean);
    if (words.length >= 2) {
      return (words[0][0] + words[1][0]).toUpperCase();
    }
    return clean.slice(0, 1).toUpperCase();
  }

  function formatSenderPhone(externalId: string | null | undefined): string | null {
    if (!externalId) return null;
    const digits = externalId.replace(/\D/g, '');
    if (!digits) return null;
    if (digits.length >= 14 && digits.startsWith('12036')) {
      // WhatsApp internal LID - not a phone number
      return null;
    }
    if (digits.startsWith('84') && digits.length >= 10 && digits.length <= 12) {
      return `+84 ${digits.slice(2, 4)} ${digits.slice(4, 7)} ${digits.slice(7)}`;
    }
    if (digits.startsWith('44') && digits.length >= 11 && digits.length <= 13) {
      return `+44 ${digits.slice(2, 6)} ${digits.slice(6)}`;
    }
    if (digits.startsWith('351') && digits.length >= 11 && digits.length <= 13) {
      return `+351 ${digits.slice(3, 6)} ${digits.slice(6, 9)} ${digits.slice(9)}`;
    }
    if (digits.startsWith('60') && digits.length >= 10 && digits.length <= 12) {
      return `+60 ${digits.slice(2, 4)}-${digits.slice(4, 7)} ${digits.slice(7)}`;
    }
    if (digits.startsWith('1') && digits.length === 11) {
      return `+1 (${digits.slice(1, 4)}) ${digits.slice(4, 7)}-${digits.slice(7)}`;
    }
    if (digits.length <= 15) {
      return `+${digits}`;
    }
    return null;
  }

  // 1. Is this a real human name (not phone, not group title, not LID, not "Member")?
  let resolvedHumanName = $derived.by(() => {
    if (!message.senderDisplayName?.trim()) return null;
    const trimmed = message.senderDisplayName.trim();
    const lower = trimmed.toLowerCase();
    const isGroupTitle =
      conversationTitle &&
      (lower === conversationTitle.trim().toLowerCase() ||
        trimmed === conversationTitle.trim());
    if (
      !trimmed.startsWith('+12036') &&
      !trimmed.startsWith('12036') &&
      lower !== 'member' &&
      lower !== 'whatsapp group' &&
      !isGroupTitle &&
      !trimmed.startsWith('+') &&
      !/^\d+$/.test(trimmed.replace(/\s+/g, ''))
    ) {
      return trimmed.replace(/^~\s*/, '').trim();
    }
    return null;
  });

  // 2. Formatted phone number from senderExternalId or senderDisplayName (if phone)
  let resolvedPhoneNumber = $derived.by(() => {
    if (message.senderExternalId?.trim()) {
      const phone = formatSenderPhone(message.senderExternalId);
      if (phone) return phone;
    }
    if (message.senderDisplayName?.trim()) {
      const phone = formatSenderPhone(message.senderDisplayName);
      if (phone) return phone;
    }
    return null;
  });

  // 3. Main display label: HumanName OR FormattedPhone OR 'Member'
  let senderDisplayLabel = $derived.by(() => {
    if (resolvedHumanName) {
      return resolvedHumanName;
    }
    if (resolvedPhoneNumber) {
      return resolvedPhoneNumber;
    }
    return 'Member';
  });

  // 4. Secondary phone to display on the right side: only if human name is displayed
  let secondaryPhoneDisplay = $derived.by(() => {
    if (resolvedHumanName && resolvedPhoneNumber) {
      return resolvedPhoneNumber;
    }
    return null;
  });

  let senderDisplayNameResolved = $derived.by(() => {
    return resolvedHumanName || resolvedPhoneNumber || 'Member';
  });

  let senderColor = $derived(
    getSenderColor(message.senderIdentityId || message.senderExternalId || message.senderDisplayName)
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

  let activeReactionCodes = $derived(
    displayReactions.filter((g) => g.includesMe).map((g) => reactionIconToCode(g.reactionIcon)!)
  );

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

  function handleSelectReaction(code: ConversationReactionCode, action: 'increment' | 'remove' = 'increment') {
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
  class="group/bubble flex w-full my-1.5 items-start gap-2 {isOutbound ? 'justify-end' : 'justify-start'}"
  data-message-id={message.id}
  role="group"
  aria-label={`Message ${message.id}`}
  use:registerContextMenu
  onmouseenter={() => (isHovered = true)}
  onmouseleave={() => {
    isHovered = false;
  }}
>
  {#if !isOutbound && isGroup}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
      class="shrink-0 mt-0.5 cursor-pointer select-none"
      title={senderDisplayNameResolved}
      onclick={(e) => {
        e.stopPropagation();
        onUserClick({
          identityId: message.senderIdentityId,
          displayName: senderDisplayNameResolved,
          externalId: message.senderExternalId || undefined,
          targetEl: e.currentTarget as HTMLElement
        });
      }}
    >
      {#if message.senderAvatarUrl}
        <img
          src={message.senderAvatarUrl}
          alt={senderDisplayNameResolved}
          class="size-7 rounded-full object-cover shadow-xs border border-border/40"
        />
      {:else}
        <div
          class="size-7 rounded-full flex items-center justify-center text-[10.5px] font-bold text-white shadow-xs"
          style="background-color: {senderColor}"
        >
          {getInitials(senderDisplayNameResolved)}
        </div>
      {/if}
    </div>
  {/if}

  <div
    class="relative max-w-[82%] sm:max-w-[72%] rounded-2xl px-3.5 py-2.5 transition-colors text-[13.5px] leading-relaxed font-sans space-y-1.5 shadow-[0_1px_0.5px_rgba(11,20,26,0.13)] {isOutbound
      ? 'bg-[#d9fdd3] dark:bg-[#005c4b] text-[#111b21] dark:text-[#e9edef] border border-[#b7e4b1]/60 dark:border-[#005c4b]/80 rounded-tr-xs font-normal'
      : 'bg-white dark:bg-[#202c33] text-[#111b21] dark:text-[#e9edef] border border-slate-200/80 dark:border-slate-700/60 rounded-tl-xs font-normal'}"
  >
    <!-- Header info: Sender name / Channel / Account -->
    <div class="flex items-start justify-between gap-2">
      <div class="flex-1 min-w-0 flex items-center gap-2 text-xs opacity-90 font-medium flex-wrap">
        {#if !isOutbound && isGroup}
          <div class="flex-1 min-w-0 flex items-center justify-between gap-3 text-xs mb-0.5">
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <span
              class="font-bold text-[12.5px] cursor-pointer hover:underline select-none truncate"
              style="color: {senderColor}"
              data-slot="sender-name"
              onclick={(e) => {
                e.stopPropagation();
                onUserClick({
                  identityId: message.senderIdentityId,
                  displayName: senderDisplayNameResolved,
                  externalId: message.senderExternalId || undefined,
                  targetEl: e.currentTarget as HTMLElement
                });
              }}
            >
              {#if resolvedHumanName}
                {resolvedHumanName}
              {:else if resolvedPhoneNumber}
                {resolvedPhoneNumber}
              {:else}
                Member
              {/if}
            </span>
            {#if secondaryPhoneDisplay}
              <span class="text-[11px] text-muted-foreground/75 font-normal shrink-0">
                {secondaryPhoneDisplay}
              </span>
            {/if}
          </div>
        {:else if !isOutbound && (message.senderDisplayName) && message.senderDisplayName !== message.accountDisplayName}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <span
            class="font-bold text-[#075e54] dark:text-[#25d366] cursor-pointer hover:underline hover:opacity-80 transition-all select-none"
            data-slot="sender-name"
            onclick={(e) => {
              e.stopPropagation();
              onUserClick({
                identityId: message.senderIdentityId,
                displayName: message.senderDisplayName ? message.senderDisplayName.replace(/^~+\s*/, '') : '',
                externalId: message.senderExternalId || undefined,
                targetEl: e.currentTarget as HTMLElement
              });
            }}
          >
            {message.senderDisplayName.replace(/^~+\s*/, '')}
          </span>
        {/if}

        {#if message.channel && !isGroup}
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

        {#if message.accountDisplayName && !isGroup}
          <span class="truncate max-w-[150px] opacity-75" title={message.accountDisplayName}>
            {message.accountDisplayName}
          </span>
        {/if}
      </div>

      <div class="flex items-center gap-1">
        {#if canReact}
          <Popover.Root bind:open={showPicker}>
            <Popover.Trigger
              aria-label="Add reaction"
              title="Add reaction"
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
          aria-label={isPinned ? 'Unpin message' : 'Pin message'}
          title={isPinned ? 'Unpin message' : 'Pin message'}
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
        <Undo2 class="size-4 shrink-0" />
        <span>{m.conversation_message_recalled()}</span>
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
            <span class="font-semibold text-muted-foreground">From:</span> {emailFrom}
          </div>
        {/if}
        {#if emailTo}
          <div>
            <span class="font-semibold text-muted-foreground">To:</span> {emailTo}
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
        <MessageReplyPreview replyTarget={message.replyTarget} onClick={() => onReply(message)} />
      {/if}
      <MessageContentRenderer
        {message}
        onImageClick={(url) => onImageClick(url, message)}
        onMentionClick={(identityId, mentionText, externalId, targetEl) => {
          const cleanName = mentionText.startsWith('@') ? mentionText.slice(1).trim() : mentionText.trim();
          onUserClick({
            identityId: identityId ?? null,
            displayName: cleanName,
            externalId: externalId || undefined,
            sourceMessageId: message.id,
            targetEl
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
