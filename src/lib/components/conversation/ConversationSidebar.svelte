<script lang="ts">
  import type { Channel, ConversationMessage, InboxItem } from '$lib/services/conversationService';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import { Skeleton } from '$lib/components/ui/skeleton';
  import { cn } from '$lib/utils';

  import MessageSquare from 'lucide-svelte/icons/message-square';
  import MessageCircle from 'lucide-svelte/icons/message-circle';
  import Mail from 'lucide-svelte/icons/mail';
  import Phone from 'lucide-svelte/icons/phone';
  import Search from 'lucide-svelte/icons/search';
  import Users from 'lucide-svelte/icons/users';
  import Plus from 'lucide-svelte/icons/plus';
  import Trash2 from 'lucide-svelte/icons/trash-2';
  import Pin from 'lucide-svelte/icons/pin';
  import CheckSquare from 'lucide-svelte/icons/check-square';
  import X from 'lucide-svelte/icons/x';
  import { onMount } from 'svelte';

  import NewChatDialog from './NewChatDialog.svelte';
  import DeleteConversationDialog from './DeleteConversationDialog.svelte';
  import { parseEmoticons } from './emoticonUtils';

  let {
    items,
    selectedKey,
    loading = false,
    onSelect,
    onCreated,
    onDeleted
  }: {
    items: InboxItem[];
    selectedKey: string | null;
    loading?: boolean;
    onSelect: (item: InboxItem) => void;
    onCreated?: (message: ConversationMessage) => void;
    onDeleted?: () => void;
  } = $props();

  let searchQuery = $state('');
  let activeKindTab = $state<'all' | 'customer' | 'group'>('all');
  let activeFilterTab = $state<'all' | 'unread' | 'priority' | 'stuck'>('all');
  let newChatOpen = $state(false);
  let deleteDialogOpen = $state(false);
  let itemToDelete = $state<InboxItem | null>(null);
  let itemsToDelete = $state<InboxItem[]>([]);
  let pinnedKeys = $state<Set<string>>(new Set());

  // Multi-selection mode states
  let isSelectionMode = $state(false);
  let selectedKeys = $state<Set<string>>(new Set());

  onMount(() => {
    try {
      const saved = localStorage.getItem('rustsale_pinned_conversations');
      if (saved) {
        pinnedKeys = new Set(JSON.parse(saved));
      }
    } catch {}
  });

  function togglePinConversation(e: MouseEvent, item: InboxItem) {
    e.stopPropagation();
    const next = new Set(pinnedKeys);
    if (next.has(item.key)) {
      next.delete(item.key);
    } else {
      next.add(item.key);
    }
    pinnedKeys = next;
    try {
      localStorage.setItem('rustsale_pinned_conversations', JSON.stringify([...next]));
    } catch {}
  }


  const channelIcons: Record<Channel, typeof MessageSquare> = {
    zalo: MessageSquare,
    whatsapp: Phone,
    messenger: MessageCircle,
    email: Mail
  };

  function isUnreadItem(item: InboxItem): boolean {
    if (item.replyTargets && item.replyTargets.length > 0) {
      return item.replyTargets.some((t) => {
        if (!t.lastInboundAt) return false;
        if (!t.lastMessageAt) return true;
        return new Date(t.lastInboundAt).getTime() >= new Date(t.lastMessageAt).getTime();
      });
    }
    if (item.kind === 'group') {
      return item.channels && item.channels.some((c) => c.messageCount > 0);
    }
    return item.kind === 'unknown';
  }


  function isStuckItem(item: InboxItem): boolean {
    if (!isUnreadItem(item) || !item.lastMessageAt) return false;
    const time = new Date(item.lastMessageAt).getTime();
    if (isNaN(time)) return false;
    const diffHours = (Date.now() - time) / (1000 * 60 * 60);
    return diffHours >= 2;
  }

  function isPriorityItem(item: InboxItem): boolean {
    return item.leadId !== null || (item.channels && item.channels.some((c) => c.messageCount >= 3));
  }


  let unreadCount = $derived(items.filter(isUnreadItem).length);
  let priorityCount = $derived(items.filter(isPriorityItem).length);
  let stuckCount = $derived(items.filter(isStuckItem).length);

  let allCount = $derived(items.length);
  let customerCount = $derived(items.filter((i) => i.kind === 'customer' || i.kind === 'unknown').length);
  let groupCount = $derived(items.filter((i) => i.kind === 'group').length);

  let filteredItems = $derived(
    items.filter((item) => {
      // 1. Text search filter
      if (searchQuery.trim()) {
        const q = searchQuery.toLowerCase().trim();
        const titleMatch = item.title.toLowerCase().includes(q);
        const previewMatch = (item.lastMessagePreview ?? '').toLowerCase().includes(q);
        if (!titleMatch && !previewMatch) return false;
      }

      // 2. Triage tab filter
      if (activeFilterTab === 'unread') return isUnreadItem(item);
      if (activeFilterTab === 'priority') return isPriorityItem(item);
      if (activeFilterTab === 'stuck') return isStuckItem(item);

      return true;
    })
  );

  let filteredByKindItems = $derived(
    filteredItems.filter((item) => {
      if (activeKindTab === 'customer') return item.kind === 'customer' || item.kind === 'unknown';
      if (activeKindTab === 'group') return item.kind === 'group';
      return true;
    })
  );


  function sortByLatest(a: InboxItem, b: InboxItem): number {
    const aAt = a.lastMessageAt ?? '';
    const bAt = b.lastMessageAt ?? '';
    if (aAt === bAt) return 0;
    return bAt.localeCompare(aAt);
  }

  let pinnedItems = $derived(
    filteredByKindItems.filter((i) => pinnedKeys.has(i.key)).slice().sort(sortByLatest)
  );

  let unpinnedItems = $derived(
    filteredByKindItems.filter((i) => !pinnedKeys.has(i.key)).slice().sort(sortByLatest)
  );

  let customerItems = $derived(
    filteredByKindItems.filter((i) => !pinnedKeys.has(i.key) && (i.kind === 'customer' || i.kind === 'unknown')).slice().sort(sortByLatest)
  );

  let groupItems = $derived(
    filteredByKindItems.filter((i) => !pinnedKeys.has(i.key) && i.kind === 'group').slice().sort(sortByLatest)
  );

  // Multi-selection derived helpers
  let visibleItems = $derived(filteredByKindItems);
  let visibleSelectedItems = $derived(
    visibleItems.filter((item) => selectedKeys.has(item.key))
  );
  let selectedCount = $derived(visibleSelectedItems.length);
  let isAllSelected = $derived(
    visibleItems.length > 0 && visibleSelectedItems.length === visibleItems.length
  );

  function toggleSelectionMode() {
    isSelectionMode = !isSelectionMode;
    if (!isSelectionMode) {
      selectedKeys = new Set();
    }
  }

  function toggleItemSelection(key: string, e?: Event) {
    if (e) e.stopPropagation();
    const next = new Set(selectedKeys);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    selectedKeys = next;
  }

  function toggleSelectAll() {
    if (isAllSelected) {
      selectedKeys = new Set();
    } else {
      selectedKeys = new Set(visibleItems.map((item) => item.key));
    }
  }

  function promptBatchDelete() {
    if (visibleSelectedItems.length === 0) return;
    itemToDelete = null;
    itemsToDelete = visibleSelectedItems;
    deleteDialogOpen = true;
  }

  function handleDeleted() {
    selectedKeys = new Set();
    isSelectionMode = false;
    itemToDelete = null;
    itemsToDelete = [];
    onDeleted?.();
  }

  function getInitials(title: string): string {
    if (!title) return '?';
    const clean = title.trim();
    if (/^unknown\b/i.test(clean)) {
      return 'UN';
    }
    const sanitized = clean.replace(/[()]/g, '').trim();
    const parts = sanitized.split(/\s+/).filter(Boolean);
    if (parts.length >= 2) {
      return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
    }
    return sanitized.slice(0, 2).toUpperCase();
  }

  function getParticipantsPreview(item: InboxItem): string {
    if (!item.participants || item.participants.length === 0) return '';
    return item.participants.map((p) => p.displayName || 'Member').join(', ');
  }

  function promptDelete(e: MouseEvent, item: InboxItem) {
    e.stopPropagation();
    itemToDelete = item;
    itemsToDelete = [];
    deleteDialogOpen = true;
  }
</script>

<div class="flex flex-col h-full w-full bg-background border-r border-border min-h-0">
  <div class="p-3 border-b border-border space-y-2 shrink-0">
    {#if isSelectionMode}
      <div class="flex items-center justify-between gap-2 py-0.5">
        <div class="flex items-center gap-2">
          <div
            role="button"
            tabindex="0"
            data-select-all
            class="h-7 px-2 text-xs font-medium gap-1.5 inline-flex items-center rounded-md hover:bg-muted cursor-pointer select-none"
            onclick={toggleSelectAll}
            onkeydown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                toggleSelectAll();
              }
            }}
          >
            <Checkbox checked={isAllSelected} tabindex={-1} class="pointer-events-none" />
            <span>All ({visibleItems.length})</span>
          </div>
          <span class="text-xs font-semibold text-muted-foreground">
            Selected {selectedCount}
          </span>
        </div>
        <div class="flex items-center gap-1">
          <Button
            variant="destructive"
            size="sm"
            class="h-7 text-xs px-2.5 font-medium gap-1"
            disabled={selectedCount === 0}
            onclick={promptBatchDelete}
          >
            <Trash2 class="w-3.5 h-3.5" />
            <span>Delete ({selectedCount})</span>
          </Button>
          <Button
            variant="ghost"
            size="sm"
            class="h-7 w-7 p-0 text-muted-foreground hover:text-foreground"
            title="Exit selection mode"
            onclick={toggleSelectionMode}
          >
            <X class="w-4 h-4" />
          </Button>
        </div>
      </div>
    {:else}
      <div class="flex items-center justify-between gap-2">
        <span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Conversations</span>
        <div class="flex items-center gap-1">
          <Button
            variant="outline"
            size="sm"
            class="h-7 text-xs gap-1 px-2 font-medium"
            title="Select multiple conversations"
            onclick={toggleSelectionMode}
          >
            <CheckSquare class="w-3.5 h-3.5" />
            <span>Select</span>
          </Button>
          <Button
            variant="default"
            size="sm"
            class="h-7 text-xs gap-1 px-2.5 font-medium"
            onclick={() => (newChatOpen = true)}
          >
            <Plus class="w-3.5 h-3.5" />
            <span>New Chat</span>
          </Button>
        </div>
      </div>
    {/if}

    <div class="relative">
      <Search class="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
      <Input
        type="text"
        placeholder="Search conversations..."
        bind:value={searchQuery}
        class="pl-9 text-sm h-8"
      />
    </div>

    <!-- Kind Filter Tabs -->
    <div class="flex items-center gap-1 bg-muted/60 p-1 rounded-lg border border-border/40 select-none text-xs font-medium">
      <button
        type="button"
        data-kind-tab="all"
        class={cn(
          "flex-1 py-1 px-2 rounded-md transition-all text-center cursor-pointer",
          activeKindTab === 'all'
            ? "bg-background text-foreground font-semibold shadow-xs border border-border/50"
            : "text-muted-foreground hover:text-foreground"
        )}
        onclick={() => (activeKindTab = 'all')}
      >
        All ({allCount})
      </button>
      <button
        type="button"
        data-kind-tab="customer"
        class={cn(
          "flex-1 py-1 px-2 rounded-md transition-all text-center cursor-pointer",
          activeKindTab === 'customer'
            ? "bg-background text-foreground font-semibold shadow-xs border border-border/50"
            : "text-muted-foreground hover:text-foreground"
        )}
        onclick={() => (activeKindTab = 'customer')}
      >
        Customers ({customerCount})
      </button>
      <button
        type="button"
        data-kind-tab="group"
        class={cn(
          "flex-1 py-1 px-2 rounded-md transition-all text-center cursor-pointer",
          activeKindTab === 'group'
            ? "bg-background text-foreground font-semibold shadow-xs border border-border/50"
            : "text-muted-foreground hover:text-foreground"
        )}
        onclick={() => (activeKindTab = 'group')}
      >
        Groups ({groupCount})
      </button>
    </div>

    <!-- Triage Filter Bar -->
    <div class="flex items-center gap-1 overflow-x-auto no-scrollbar pt-1 select-none">
      <button
        type="button"
        class={cn(
          "px-2 py-0.5 rounded-full text-[11px] font-medium transition-colors shrink-0 cursor-pointer",
          activeFilterTab === 'all'
            ? "bg-primary text-primary-foreground font-semibold"
            : "bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground"
        )}
        onclick={() => (activeFilterTab = 'all')}
      >
        All ({items.length})
      </button>
      <button
        type="button"
        class={cn(
          "px-2 py-0.5 rounded-full text-[11px] font-medium transition-colors shrink-0 cursor-pointer",
          activeFilterTab === 'unread'
            ? "bg-primary text-primary-foreground font-semibold"
            : "bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground"
        )}
        onclick={() => (activeFilterTab = 'unread')}
      >
        Unread ({unreadCount})
      </button>
      <button
        type="button"
        class={cn(
          "px-2 py-0.5 rounded-full text-[11px] font-medium transition-colors shrink-0 cursor-pointer",
          activeFilterTab === 'priority'
            ? "bg-primary text-primary-foreground font-semibold"
            : "bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground"
        )}
        onclick={() => (activeFilterTab = 'priority')}
      >
        Priority ({priorityCount})
      </button>
      <button
        type="button"
        class={cn(
          "px-2 py-0.5 rounded-full text-[11px] font-medium transition-colors shrink-0 cursor-pointer",
          activeFilterTab === 'stuck'
            ? "bg-destructive text-destructive-foreground font-semibold"
            : "bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground"
        )}
        onclick={() => (activeFilterTab = 'stuck')}
      >
        Stuck ({stuckCount})
      </button>
    </div>
  </div>

  <ScrollArea class="flex-1 min-h-0 w-full h-full">
    {#if loading}
      <div class="space-y-3 p-3">
        {#each Array(5) as _, i}
          <div class="flex items-center gap-3">
            <Skeleton class="w-10 h-10 rounded-full shrink-0" />
            <div class="flex-1 space-y-2">
              <Skeleton class="h-4 w-3/4" />
              <Skeleton class="h-3 w-1/2" />
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <div class="p-2 space-y-4">
        {#if pinnedItems.length > 0}
          <div>
            <h3 class="text-xs font-semibold text-amber-500 uppercase tracking-wider px-3 py-1.5 flex items-center gap-1.5">
              <Pin class="w-3.5 h-3.5 fill-amber-500" />
              <span>Pinned ({pinnedItems.length})</span>
            </h3>
            <div class="space-y-1">
              {#each pinnedItems as item (item.key)}
                {@const isSelected = selectedKey === item.key}
                {@const isChecked = selectedKeys.has(item.key)}
                <div
                  role="button"
                  tabindex="0"
                  data-inbox-item
                  data-key={item.key}
                  aria-current={isSelected ? 'true' : 'false'}
                  onclick={() => (isSelectionMode ? toggleItemSelection(item.key) : onSelect(item))}
                  onkeydown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      if (isSelectionMode) {
                        toggleItemSelection(item.key);
                      } else {
                        onSelect(item);
                      }
                    }
                  }}
                  class={cn(
                    'flex items-start gap-3 p-2.5 rounded-lg cursor-pointer transition-colors text-left select-none border-l-2 border-amber-500/80',
                    isSelectionMode && isChecked
                      ? 'bg-primary/10 text-foreground font-medium'
                      : isSelected
                        ? 'bg-accent text-accent-foreground font-medium'
                        : 'hover:bg-muted/60 text-foreground bg-amber-500/5'
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
                        <button
                          type="button"
                          title="Unpin conversation"
                          onclick={(e) => togglePinConversation(e, item)}
                          class="p-1 rounded text-amber-500 hover:bg-amber-500/10 shrink-0"
                        >
                          <Pin class="w-3.5 h-3.5 fill-amber-500" />
                        </button>
                        {#if !isSelectionMode}
                          <button
                            type="button"
                            title="Delete conversation"
                            onclick={(e) => promptDelete(e, item)}
                            class="opacity-0 group-hover/item:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-destructive rounded hover:bg-destructive/10 shrink-0"
                          >
                            <Trash2 class="w-3.5 h-3.5" />
                          </button>
                        {/if}
                      </div>
                    </div>

                    {#if item.lastMessagePreview}
                      <p class="text-xs text-muted-foreground truncate mt-0.5">
                        {parseEmoticons(item.lastMessagePreview)}
                      </p>
                    {/if}

                    {#if item.channels && item.channels.length > 0}
                      <div class="flex items-center gap-1.5 mt-1.5 flex-wrap">
                        {#each item.channels as ch (ch.channel)}
                          {@const Icon = channelIcons[ch.channel] || MessageSquare}
                          {@const count = isSelected ? 0 : (ch.unreadCount !== undefined ? ch.unreadCount : ch.messageCount)}
                          <span
                            class={cn(
                              "inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] font-medium transition-colors",
                              count > 0
                                ? "bg-destructive/15 text-destructive font-bold ring-1 ring-destructive/30"
                                : "bg-muted/80 text-muted-foreground"
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
              {/each}
            </div>
          </div>
        {/if}

        {#if activeKindTab === 'all'}
          {#if unpinnedItems.length > 0}
            <div>
              <div class="space-y-1">
                {#each unpinnedItems as item (item.key)}
                  {@const isSelected = selectedKey === item.key}
                  {@const isChecked = selectedKeys.has(item.key)}
                  <div
                    role="button"
                    tabindex="0"
                    data-inbox-item
                    data-key={item.key}
                    aria-current={isSelected ? 'true' : 'false'}
                    onclick={() => (isSelectionMode ? toggleItemSelection(item.key) : onSelect(item))}
                    onkeydown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault();
                        if (isSelectionMode) {
                          toggleItemSelection(item.key);
                        } else {
                          onSelect(item);
                        }
                      }
                    }}
                    class={cn(
                      'flex items-start gap-3 p-2.5 rounded-lg cursor-pointer transition-colors text-left select-none',
                      isSelectionMode && isChecked
                        ? 'bg-primary/10 text-foreground font-medium'
                        : isSelected
                          ? 'bg-accent text-accent-foreground font-medium'
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
                          <button
                            type="button"
                            title="Pin conversation"
                            onclick={(e) => togglePinConversation(e, item)}
                            class="opacity-0 group-hover/item:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-amber-500 rounded hover:bg-amber-500/10 shrink-0"
                          >
                            <Pin class="w-3.5 h-3.5" />
                          </button>
                          {#if !isSelectionMode}
                            <button
                              type="button"
                              title="Delete conversation"
                              onclick={(e) => promptDelete(e, item)}
                              class="opacity-0 group-hover/item:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-destructive rounded hover:bg-destructive/10 shrink-0"
                            >
                              <Trash2 class="w-3.5 h-3.5" />
                            </button>
                          {/if}
                        </div>
                      </div>

                      {#if item.lastMessagePreview}
                        <p class="text-xs text-muted-foreground truncate mt-0.5">
                          {parseEmoticons(item.lastMessagePreview)}
                        </p>
                      {/if}

                      {#if item.kind === 'group'}
                        <div class="flex items-center gap-1.5 mt-1 text-xs text-muted-foreground">
                          <Users class="w-3.5 h-3.5 shrink-0" />
                          <span class="shrink-0 font-medium">{item.participantCount}</span>
                          {#if getParticipantsPreview(item)}
                            <span class="truncate">({getParticipantsPreview(item)})</span>
                          {/if}
                        </div>
                      {/if}

                      {#if item.channels && item.channels.length > 0}
                        <div class="flex items-center gap-1.5 mt-1.5 flex-wrap">
                          {#each item.channels as ch (ch.channel)}
                            {@const Icon = channelIcons[ch.channel] || MessageSquare}
                            {@const count = isSelected ? 0 : (ch.unreadCount !== undefined ? ch.unreadCount : ch.messageCount)}
                            {@const chName = ch.channel.toLowerCase()}
                            <span
                              class={cn(
                                "inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-[10px] font-semibold transition-colors border",
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
                {/each}
              </div>
            </div>
          {/if}
        {:else if activeKindTab === 'customer'}
          {#if customerItems.length > 0}
            <div>
              <h3 class="text-xs font-semibold text-muted-foreground uppercase tracking-wider px-3 py-1.5">
                Customers ({customerItems.length})
              </h3>
              <div class="space-y-1">
                {#each customerItems as item (item.key)}
                  {@const isSelected = selectedKey === item.key}
                  {@const isChecked = selectedKeys.has(item.key)}
                  <div
                    role="button"
                    tabindex="0"
                    data-inbox-item
                    data-key={item.key}
                    aria-current={isSelected ? 'true' : 'false'}
                    onclick={() => (isSelectionMode ? toggleItemSelection(item.key) : onSelect(item))}
                    onkeydown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault();
                        if (isSelectionMode) {
                          toggleItemSelection(item.key);
                        } else {
                          onSelect(item);
                        }
                      }
                    }}
                    class={cn(
                      'flex items-start gap-3 p-2.5 rounded-lg cursor-pointer transition-colors text-left select-none',
                      isSelectionMode && isChecked
                        ? 'bg-primary/10 text-foreground font-medium'
                        : isSelected
                          ? 'bg-accent text-accent-foreground font-medium'
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
                          <button
                            type="button"
                            title="Pin conversation"
                            onclick={(e) => togglePinConversation(e, item)}
                            class="opacity-0 group-hover/item:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-amber-500 rounded hover:bg-amber-500/10 shrink-0"
                          >
                            <Pin class="w-3.5 h-3.5" />
                          </button>
                          {#if !isSelectionMode}
                            <button
                              type="button"
                              title="Delete conversation"
                              onclick={(e) => promptDelete(e, item)}
                              class="opacity-0 group-hover/item:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-destructive rounded hover:bg-destructive/10 shrink-0"
                            >
                              <Trash2 class="w-3.5 h-3.5" />
                            </button>
                          {/if}
                        </div>
                      </div>

                      {#if item.lastMessagePreview}
                        <p class="text-xs text-muted-foreground truncate mt-0.5">
                          {parseEmoticons(item.lastMessagePreview)}
                        </p>
                      {/if}

                      {#if item.channels && item.channels.length > 0}
                        <div class="flex items-center gap-1.5 mt-1.5 flex-wrap">
                          {#each item.channels as ch (ch.channel)}
                            {@const Icon = channelIcons[ch.channel] || MessageSquare}
                            {@const count = isSelected ? 0 : (ch.unreadCount !== undefined ? ch.unreadCount : ch.messageCount)}
                            {@const chName = ch.channel.toLowerCase()}
                            <span
                              class={cn(
                                "inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-[10px] font-semibold transition-colors border",
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
                {/each}
              </div>
            </div>
          {/if}
        {:else if activeKindTab === 'group'}
          {#if groupItems.length > 0}
            <div>
              <h3 class="text-xs font-semibold text-muted-foreground uppercase tracking-wider px-3 py-1.5">
                Groups ({groupItems.length})
              </h3>
              <div class="space-y-1">
                {#each groupItems as item (item.key)}
                  {@const isSelected = selectedKey === item.key}
                  {@const isChecked = selectedKeys.has(item.key)}
                  <div
                    role="button"
                    tabindex="0"
                    data-inbox-item
                    data-key={item.key}
                    aria-current={isSelected ? 'true' : 'false'}
                    onclick={() => (isSelectionMode ? toggleItemSelection(item.key) : onSelect(item))}
                    onkeydown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault();
                        if (isSelectionMode) {
                          toggleItemSelection(item.key);
                        } else {
                          onSelect(item);
                        }
                      }
                    }}
                    class={cn(
                      'flex items-start gap-3 p-2.5 rounded-lg cursor-pointer transition-colors text-left select-none',
                      isSelectionMode && isChecked
                        ? 'bg-primary/10 text-foreground font-medium'
                        : isSelected
                          ? 'bg-accent text-accent-foreground font-medium'
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
                          <button
                            type="button"
                            title="Pin conversation"
                            onclick={(e) => togglePinConversation(e, item)}
                            class="opacity-0 group-hover/item:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-amber-500 rounded hover:bg-amber-500/10 shrink-0"
                          >
                            <Pin class="w-3.5 h-3.5" />
                          </button>
                          {#if !isSelectionMode}
                            <button
                              type="button"
                              title="Delete conversation"
                              onclick={(e) => promptDelete(e, item)}
                              class="opacity-0 group-hover/item:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-destructive rounded hover:bg-destructive/10 shrink-0"
                            >
                              <Trash2 class="w-3.5 h-3.5" />
                            </button>
                          {/if}
                        </div>
                      </div>

                      {#if item.lastMessagePreview}
                        <p class="text-xs text-muted-foreground truncate mt-0.5">
                          {parseEmoticons(item.lastMessagePreview)}
                        </p>
                      {/if}

                      <div class="flex items-center gap-1.5 mt-1 text-xs text-muted-foreground">
                        <Users class="w-3.5 h-3.5 shrink-0" />
                        <span class="shrink-0 font-medium">{item.participantCount}</span>
                        {#if getParticipantsPreview(item)}
                          <span class="truncate">({getParticipantsPreview(item)})</span>
                        {/if}
                      </div>

                      {#if item.channels && item.channels.length > 0}
                        <div class="flex items-center gap-1.5 mt-1.5 flex-wrap">
                          {#each item.channels as ch (ch.channel)}
                            {@const Icon = channelIcons[ch.channel] || MessageSquare}
                            {@const count = isSelected ? 0 : (ch.unreadCount !== undefined ? ch.unreadCount : ch.messageCount)}
                            <span
                              class={cn(
                                "inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] font-medium transition-colors",
                                count > 0
                                  ? "bg-destructive/15 text-destructive font-bold ring-1 ring-destructive/30"
                                  : "bg-muted/80 text-muted-foreground"
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
                {/each}
              </div>
            </div>
          {/if}
        {/if}

        {#if unpinnedItems.length === 0 && pinnedItems.length === 0}
          <div class="p-4 text-center text-sm text-muted-foreground">
            No conversations found
          </div>
        {/if}

      </div>
    {/if}
  </ScrollArea>

  <NewChatDialog bind:open={newChatOpen} onSuccess={onCreated} />
  <DeleteConversationDialog
    bind:open={deleteDialogOpen}
    item={itemToDelete}
    items={itemsToDelete}
    onDeleted={handleDeleted}
  />
</div>
