<script lang="ts">
  import type { Channel, InboxItem } from '$lib/services/conversationService';
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

  let {
    items,
    selectedKey,
    loading = false,
    onSelect
  }: {
    items: InboxItem[];
    selectedKey: string | null;
    loading?: boolean;
    onSelect: (item: InboxItem) => void;
  } = $props();

  let searchQuery = $state('');

  const channelIcons: Record<Channel, typeof MessageSquare> = {
    zalo: MessageSquare,
    whatsapp: Phone,
    messenger: MessageCircle,
    email: Mail
  };

  let filteredItems = $derived(
    items.filter((item) => {
      if (!searchQuery.trim()) return true;
      const q = searchQuery.toLowerCase().trim();
      const titleMatch = item.title.toLowerCase().includes(q);
      const previewMatch = (item.lastMessagePreview ?? '').toLowerCase().includes(q);
      return titleMatch || previewMatch;
    })
  );

  let customerItems = $derived(
    filteredItems.filter((i) => i.kind === 'customer' || i.kind === 'unknown')
  );

  let groupItems = $derived(
    filteredItems.filter((i) => i.kind === 'group')
  );

  function getInitials(title: string): string {
    if (!title) return '?';
    const clean = title.trim();
    const parts = clean.split(/\s+/);
    if (parts.length >= 2) {
      return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
    }
    return clean.slice(0, 2).toUpperCase();
  }

  function getParticipantsPreview(item: InboxItem): string {
    if (!item.participants || item.participants.length === 0) return '';
    return item.participants.map((p) => p.displayName || 'Thành viên').join(', ');
  }
</script>

<div class="flex flex-col h-full w-full bg-background border-r border-border">
  <div class="p-3 border-b border-border">
    <div class="relative">
      <Search class="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
      <Input
        type="text"
        placeholder="Tìm kiếm cuộc trò chuyện..."
        bind:value={searchQuery}
        class="pl-9 text-sm"
      />
    </div>
  </div>

  <ScrollArea class="flex-1">
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
                    <div class="flex items-center justify-between gap-1">
                      <span class="font-semibold text-sm truncate">{item.title}</span>
                    </div>

                    {#if item.lastMessagePreview}
                      <p class="text-xs text-muted-foreground truncate mt-0.5">
                        {item.lastMessagePreview}
                      </p>
                    {/if}

                    {#if item.channels && item.channels.length > 0}
                      <div class="flex items-center gap-1.5 mt-1.5 flex-wrap">
                        {#each item.channels as ch (ch.channel)}
                          {@const Icon = channelIcons[ch.channel] || MessageSquare}
                          <span
                            class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-muted/80 text-[11px] text-muted-foreground"
                          >
                            <Icon class="w-3 h-3" />
                            <span class="font-semibold">{ch.messageCount}</span>
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
                    <div class="flex items-center justify-between gap-1">
                      <span class="font-semibold text-sm truncate">{item.title}</span>
                    </div>

                    {#if item.lastMessagePreview}
                      <p class="text-xs text-muted-foreground truncate mt-0.5">
                        {item.lastMessagePreview}
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
                          <span
                            class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-muted/80 text-[11px] text-muted-foreground"
                          >
                            <Icon class="w-3 h-3" />
                            <span class="font-semibold">{ch.messageCount}</span>
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
</div>
