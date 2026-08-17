<script lang="ts">
  import type { Channel, InboxItem } from '$lib/services/conversationService';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { cn } from '$lib/utils';
  import MessageSquare from 'lucide-svelte/icons/message-square';
  import MessageCircle from 'lucide-svelte/icons/message-circle';
  import Mail from 'lucide-svelte/icons/mail';
  import Phone from 'lucide-svelte/icons/phone';
  import Users from 'lucide-svelte/icons/users';
  import Trash2 from 'lucide-svelte/icons/trash-2';
  import Pin from 'lucide-svelte/icons/pin';
  import { parseEmoticons } from './emoticonUtils';
  import { getInitials, getParticipantsPreview } from './sidebarState';

  const channelIcons: Record<Channel, typeof MessageSquare> = {
    zalo: MessageSquare,
    whatsapp: Phone,
    messenger: MessageCircle,
    email: Mail
  };

  let {
    item,
    isSelected = false,
    isChecked = false,
    isPinned = false,
    isSelectionMode = false,
    onSelect,
    onToggleSelection,
    onTogglePin,
    onPromptDelete
  }: {
    item: InboxItem;
    isSelected?: boolean;
    isChecked?: boolean;
    isPinned?: boolean;
    isSelectionMode?: boolean;
    onSelect: (item: InboxItem) => void;
    onToggleSelection: (key: string, e?: Event) => void;
    onTogglePin: (e: MouseEvent, item: InboxItem) => void;
    onPromptDelete: (e: MouseEvent, item: InboxItem) => void;
  } = $props();

  const isGroup = $derived(item.kind === 'group');

  function handleClick() {
    if (isSelectionMode) {
      onToggleSelection(item.key);
    } else {
      onSelect(item);
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    // If the event originated from a child interactive control (such as Pin/Delete button or checkbox),
    // do NOT trigger the row selection action.
    const target = e.target as HTMLElement | null;
    if (
      target &&
      target !== e.currentTarget &&
      (target.tagName === 'BUTTON' || target.closest('button') || target.tagName === 'INPUT')
    ) {
      return;
    }

    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      if (isSelectionMode) {
        onToggleSelection(item.key);
      } else {
        onSelect(item);
      }
    }
  }
</script>

<div
  role="button"
  tabindex="0"
  data-inbox-item
  data-key={item.key}
  aria-current={isSelected ? 'true' : 'false'}
  onclick={handleClick}
  onkeydown={handleKeyDown}
  class={cn(
    'flex items-start gap-3 p-2.5 rounded-lg cursor-pointer transition-colors text-left select-none overflow-hidden box-border',
    isGroup ? 'h-[104px] min-h-[104px] max-h-[104px]' : 'h-[80px] min-h-[80px] max-h-[80px]',
    isPinned && 'border-l-2 border-amber-500/80',
    isSelectionMode && isChecked
      ? 'bg-primary/10 text-foreground font-medium'
      : isSelected
        ? 'bg-accent text-accent-foreground font-medium'
        : isPinned
          ? 'hover:bg-muted/60 text-foreground bg-amber-500/5'
          : 'hover:bg-muted/60 text-foreground'
  )}
>
  {#if isSelectionMode}
    <Checkbox
      checked={isChecked}
      tabindex={-1}
      class="mt-3 shrink-0 pointer-events-none"
    />
  {/if}

  {#if item.avatarUrl}
    <img
      src={item.avatarUrl}
      alt={item.title}
      class="w-10 h-10 rounded-full object-cover shrink-0"
    />
  {:else}
    <div
      class="w-10 h-10 rounded-full bg-primary/10 text-primary font-semibold flex items-center justify-center shrink-0 text-sm"
    >
      {getInitials(item.title)}
    </div>
  {/if}

  <div class="flex-1 min-w-0">
    <div class="flex items-center justify-between gap-1 group/item">
      <span class="font-semibold text-sm truncate">{item.title}</span>
      <div class="flex items-center gap-0.5 shrink-0">
        {#if isPinned}
          <button
            type="button"
            title="Unpin conversation"
            onclick={(e) => onTogglePin(e, item)}
            onkeydown={(e) => e.stopPropagation()}
            class="p-1 rounded text-amber-500 hover:bg-amber-500/10 shrink-0"
          >
            <Pin class="w-3.5 h-3.5 fill-amber-500" />
          </button>
        {:else}
          <button
            type="button"
            title="Pin conversation"
            onclick={(e) => onTogglePin(e, item)}
            onkeydown={(e) => e.stopPropagation()}
            class="opacity-0 group-hover/item:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-amber-500 rounded hover:bg-amber-500/10 shrink-0"
          >
            <Pin class="w-3.5 h-3.5" />
          </button>
        {/if}

        {#if !isSelectionMode}
          <button
            type="button"
            title="Delete conversation"
            onclick={(e) => onPromptDelete(e, item)}
            onkeydown={(e) => e.stopPropagation()}
            class="opacity-0 group-hover/item:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-destructive rounded hover:bg-destructive/10 shrink-0"
          >
            <Trash2 class="w-3.5 h-3.5" />
          </button>
        {/if}
      </div>
    </div>

    {#if item.lastMessagePreview}
      <p class="text-xs text-muted-foreground truncate mt-0.5 leading-tight">
        {parseEmoticons(item.lastMessagePreview)}
      </p>
    {/if}

    {#if isGroup}
      <div class="flex items-center gap-1.5 mt-1 text-xs text-muted-foreground leading-tight">
        <Users class="w-3.5 h-3.5 shrink-0" />
        <span class="shrink-0 font-medium">{item.participantCount}</span>
        {#if getParticipantsPreview(item)}
          <span class="truncate">({getParticipantsPreview(item)})</span>
        {/if}
      </div>
    {/if}

    {#if item.channels && item.channels.length > 0}
      <div class="flex items-center gap-1.5 mt-1.5 flex-nowrap overflow-x-hidden">
        {#each item.channels as ch (ch.channel)}
          {@const Icon = channelIcons[ch.channel] || MessageSquare}
          {@const count = isSelected ? 0 : (ch.unreadCount !== undefined ? ch.unreadCount : ch.messageCount)}
          {@const chName = ch.channel.toLowerCase()}
          <span
            class={cn(
              "inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-[10px] font-semibold transition-colors border shrink-0",
              count > 0
                ? "bg-emerald-600 text-white font-bold border-emerald-600 shadow-2xs"
                : chName === 'zalo'
                  ? "bg-blue-50 dark:bg-blue-950/40 text-blue-700 dark:text-blue-300 border-blue-200 dark:border-blue-800/40"
                  : chName === 'whatsapp'
                    ? "bg-emerald-50 dark:bg-emerald-950/40 text-emerald-700 dark:text-emerald-300 border-emerald-200 dark:border-emerald-800/40"
                    : chName === 'messenger'
                      ? "bg-purple-50 dark:bg-purple-950/40 text-purple-700 dark:text-purple-300 border-purple-200 dark:border-purple-800/40"
                      : "bg-rose-50 dark:bg-rose-950/40 text-rose-700 dark:text-rose-300 border-rose-200 dark:border-rose-800/40"
            )}
          >
            <Icon class="w-3 h-3" />
            {#if count > 0}
              <span class="font-bold">{count}</span>
            {/if}
          </span>
        {/each}
      </div>
    {/if}
  </div>
</div>
