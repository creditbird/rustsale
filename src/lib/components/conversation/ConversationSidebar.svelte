<script lang="ts">
  import type { Channel, ConversationMessage, InboxItem } from '$lib/services/conversationService';
  import { Button } from '$lib/components/ui/button';
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
  let activeFilterTab = $state<'all' | 'unread' | 'priority' | 'stuck'>('all');
  let newChatOpen = $state(false);
  let deleteDialogOpen = $state(false);
  let itemToDelete = $state<InboxItem | null>(null);
  let pinnedKeys = $state<Set<string>>(new Set());

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


  function sortByLatest(a: InboxItem, b: InboxItem): number {
    const aAt = a.lastMessageAt ?? '';
    const bAt = b.lastMessageAt ?? '';
    if (aAt === bAt) return 0;
    return bAt.localeCompare(aAt);
  }



  let pinnedItems = $derived(
    filteredItems.filter((i) => pinnedKeys.has(i.key)).slice().sort(sortByLatest)
  );

  let customerItems = $derived(
    filteredItems.filter((i) => !pinnedKeys.has(i.key) && (i.kind === 'customer' || i.kind === 'unknown')).slice().sort(sortByLatest)
  );

  let groupItems = $derived(
    filteredItems.filter((i) => !pinnedKeys.has(i.key) && i.kind === 'group').slice().sort(sortByLatest)
  );



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
    return item.participants.map((p) => p.displayName || 'Thành viên').join(', ');
  }

  function promptDelete(e: MouseEvent, item: InboxItem) {
    e.stopPropagation();
    itemToDelete = item;
    deleteDialogOpen = true;
  }
</script>

<div class="flex flex-col h-full w-full bg-background border-r border-border min-h-0">
  <div class="p-3 border-b border-border space-y-2 shrink-0">
    <div class="flex items-center justify-between gap-2">
      <span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Hội thoại</span>
      <Button
        variant="default"
        size="sm"
        class="h-7 text-xs gap-1 px-2.5 font-medium"
        onclick={() => (newChatOpen = true)}
      >
        <Plus class="w-3.5 h-3.5" />
        <span>Chat mới</span>
      </Button>
    </div>
    <div class="relative">
      <Search class="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
      <Input
        type="text"
        placeholder="Tìm kiếm cuộc trò chuyện..."
        bind:value={searchQuery}
        class="pl-9 text-sm h-8"
      />
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
        Tất cả ({items.length})
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
        Chưa đọc ({unreadCount})
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
        Ưu tiên ({priorityCount})
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
        Kẹt ({stuckCount})
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
              <span>Đã ghim ({pinnedItems.length})</span>
            </h3>
            <div class="space-y-1">
              {#each pinnedItems as item (item.key)}
                {@const isSelected = selectedKey === item.key}
                <div
                  role="button"
                  tabindex="0"
                  data-inbox-item
                  data-key={item.key}
                  aria-current={isSelected ? 'true' : 'false'}
                  onclick={() => onSelect(item)}
                  onkeydown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      onSelect(item);
                    }
                  }}
                  class={cn(
                    'flex items-start gap-3 p-2.5 rounded-lg cursor-pointer transition-colors text-left select-none border-l-2 border-amber-500/80',
                    isSelected
                      ? 'bg-accent text-accent-foreground font-medium'
                      : 'hover:bg-muted/60 text-foreground bg-amber-500/5'
                  )}
                >
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
                          title="Bỏ ghim cuộc trò chuyện"
                          onclick={(e) => togglePinConversation(e, item)}
                          class="p-1 rounded text-amber-500 hover:bg-amber-500/10 shrink-0"
                        >
                          <Pin class="w-3.5 h-3.5 fill-amber-500" />
                        </button>
                        <button
                          type="button"
                          title="Xóa cuộc trò chuyện"
                          onclick={(e) => promptDelete(e, item)}
                          class="opacity-0 group-hover/item:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-destructive rounded hover:bg-destructive/10 shrink-0"
                        >
                          <Trash2 class="w-3.5 h-3.5" />
                        </button>
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

        {#if customerItems.length > 0}
          <div>
            <h3 class="text-xs font-semibold text-muted-foreground uppercase tracking-wider px-3 py-1.5">
              Khách hàng
            </h3>
            <div class="space-y-1">
              {#each customerItems as item (item.key)}
                {@const isSelected = selectedKey === item.key}
                <div
                  role="button"
                  tabindex="0"
                  data-inbox-item
                  data-key={item.key}
                  aria-current={isSelected ? 'true' : 'false'}
                  onclick={() => onSelect(item)}
                  onkeydown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      onSelect(item);
                    }
                  }}
                  class={cn(
                    'flex items-start gap-3 p-2.5 rounded-lg cursor-pointer transition-colors text-left select-none',
                    isSelected
                      ? 'bg-accent text-accent-foreground font-medium'
                      : 'hover:bg-muted/60 text-foreground'
                  )}
                >
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
                          title="Ghim cuộc trò chuyện lên đầu"
                          onclick={(e) => togglePinConversation(e, item)}
                          class="opacity-0 group-hover/item:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-amber-500 rounded hover:bg-amber-500/10 shrink-0"
                        >
                          <Pin class="w-3.5 h-3.5" />
                        </button>
                        <button
                          type="button"
                          title="Xóa cuộc trò chuyện"
                          onclick={(e) => promptDelete(e, item)}
                          class="opacity-0 group-hover/item:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-destructive rounded hover:bg-destructive/10 shrink-0"
                        >
                          <Trash2 class="w-3.5 h-3.5" />
                        </button>
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

        {#if groupItems.length > 0}
          <div>
            <h3 class="text-xs font-semibold text-muted-foreground uppercase tracking-wider px-3 py-1.5">
              Nhóm
            </h3>
            <div class="space-y-1">
              {#each groupItems as item (item.key)}
                {@const isSelected = selectedKey === item.key}
                <div
                  role="button"
                  tabindex="0"
                  data-inbox-item
                  data-key={item.key}
                  aria-current={isSelected ? 'true' : 'false'}
                  onclick={() => onSelect(item)}
                  onkeydown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      onSelect(item);
                    }
                  }}
                  class={cn(
                    'flex items-start gap-3 p-2.5 rounded-lg cursor-pointer transition-colors text-left select-none',
                    isSelected
                      ? 'bg-accent text-accent-foreground font-medium'
                      : 'hover:bg-muted/60 text-foreground'
                  )}
                >
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
                          title="Ghim cuộc trò chuyện lên đầu"
                          onclick={(e) => togglePinConversation(e, item)}
                          class="opacity-0 group-hover/item:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-amber-500 rounded hover:bg-amber-500/10 shrink-0"
                        >
                          <Pin class="w-3.5 h-3.5" />
                        </button>
                        <button
                          type="button"
                          title="Xóa cuộc trò chuyện"
                          onclick={(e) => promptDelete(e, item)}
                          class="opacity-0 group-hover/item:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-destructive rounded hover:bg-destructive/10 shrink-0"
                        >
                          <Trash2 class="w-3.5 h-3.5" />
                        </button>
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

        {#if customerItems.length === 0 && groupItems.length === 0}
          <div class="p-4 text-center text-sm text-muted-foreground">
            Không tìm thấy cuộc trò chuyện nào
          </div>
        {/if}
      </div>
    {/if}
  </ScrollArea>

  <NewChatDialog bind:open={newChatOpen} onSuccess={onCreated} />
  <DeleteConversationDialog bind:open={deleteDialogOpen} item={itemToDelete} onDeleted={onDeleted} />
</div>
