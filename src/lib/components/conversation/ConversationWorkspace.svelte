<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import {
    listConversationInbox,
    getContactTimeline,
    getConversationTimeline,
    getConversationRuntimeStatus,
    initConversationEventBridge,
    onConversationInvalidated,
    type InboxItem,
    type Channel,
    type ConversationRuntimeStatus,
    type ConversationMessage,
    type ConversationInvalidatedEvent
  } from '$lib/services/conversationService';
  import {
    TimelineCache,
    channelSetKey,
    filterMessages,
    mergeMessages,
    mergeTimelinePage,
    nearestSurvivingAnchor,
    MAX_CHANNEL_VARIANTS,
    type TimelineVariant
  } from './timelineState';
  import ConversationSidebar from './ConversationSidebar.svelte';
  import ChannelBadges from './ChannelBadges.svelte';
  import UnknownContactActions from './UnknownContactActions.svelte';
  import LinkedLeadActions from './LinkedLeadActions.svelte';
  import ConversationTimeline from './ConversationTimeline.svelte';
  import MessageComposer from './MessageComposer.svelte';
  import { Button } from '$lib/components/ui/button';
  import { AlertCircle, RefreshCw, Users, MessageSquare } from 'lucide-svelte';

  let {
    activeTab = 'conversation',
    onOpenSettings = () => {},
    runtimeRefreshIntervalMs = 5_000
  }: {
    activeTab?: string;
    onOpenSettings?: () => void;
    runtimeRefreshIntervalMs?: number;
  } = $props();

  let inbox = $state<InboxItem[]>([]);
  let inboxLoading = $state(false);
  let selectedKey = $state<string | null>(null);
  let activeChannelsPerKey = $state(new Map<string, Channel[]>());
  let activeChannels = $state<Channel[]>([]);
  let runtime = $state<ConversationRuntimeStatus>({ accounts: [] });
  let runtimeLoading = $state(false);
  let runtimeError = $state<string | null>(null);
  let runtimeRefreshInFlight = false;

  // Variant caching and staleness
  let variantsCache = new TimelineCache(MAX_CHANNEL_VARIANTS);
  let currentVariant = $state<TimelineVariant>({
    items: [],
    nextCursor: null,
    hasLoaded: false,
    loading: false
  });
  let loadingOlder = $state(false);

  let inboxStale = $state(true);
  let timelineStale = $state(new Set<string>());
  let hasBeenActivated = $state(false);

  let timelineRef = $state<ReturnType<typeof ConversationTimeline> | null>(null);

  let inboxRefreshTimer: ReturnType<typeof setTimeout> | null = null;
  let timelineRefreshTimer: ReturnType<typeof setTimeout> | null = null;

  let selectedItem = $derived(inbox.find((i) => i.key === selectedKey) ?? null);

  let currentVariantKey = $derived.by(() => {
    if (!selectedItem) return '';
    if (selectedItem.kind === 'group' || selectedItem.leadId === null) {
      return `${selectedItem.key}:group`;
    }
    return `${selectedItem.key}:${channelSetKey(activeChannels)}`;
  });

  // Track activeTab activation cleanly with untrack
  $effect(() => {
    const isActive = activeTab === 'conversation';

    if (isActive) {
      untrack(() => {
        if (!hasBeenActivated) {
          hasBeenActivated = true;
          loadInitialData();
        } else {
          if (inboxStale) {
            refreshInbox();
          }
          if (selectedItem && currentVariantKey && timelineStale.has(currentVariantKey)) {
            timelineStale.delete(currentVariantKey);
            refreshOpenTimeline();
          }
        }
      });
    }
  });

  $effect(() => {
    if (activeTab !== 'conversation' || runtimeRefreshIntervalMs <= 0) return;

    const interval = setInterval(() => {
      void refreshRuntimeStatus(true);
    }, runtimeRefreshIntervalMs);

    return () => clearInterval(interval);
  });

  // Event Bridge registration
  onMount(() => {
    const cleanupBridge = initConversationEventBridge();
    const cleanupInvalidated = onConversationInvalidated(handleInvalidatedEvent);

    return () => {
      cleanupInvalidated();
      cleanupBridge();
      if (inboxRefreshTimer) clearTimeout(inboxRefreshTimer);
      if (timelineRefreshTimer) clearTimeout(timelineRefreshTimer);
    };
  });

  async function loadInitialData() {
    inboxLoading = true;
    runtimeLoading = true;
    runtimeError = null;

    try {
      const [inboxRes, runtimeRes] = await Promise.allSettled([
        listConversationInbox(),
        getConversationRuntimeStatus()
      ]);

      if (inboxRes.status === 'fulfilled') {
        inbox = inboxRes.value;
        inboxStale = false;
        if (!selectedKey && inbox.length > 0) {
          selectItem(inbox[0]);
        }
      }

      if (runtimeRes.status === 'fulfilled') {
        runtime = runtimeRes.value;
      } else {
        runtimeError =
          runtimeRes.reason instanceof Error
            ? runtimeRes.reason.message
            : String(runtimeRes.reason);
      }
    } finally {
      inboxLoading = false;
      runtimeLoading = false;
    }
  }

  async function refreshRuntimeStatus(silent = false) {
    if (runtimeRefreshInFlight) return;
    runtimeRefreshInFlight = true;
    if (!silent) runtimeLoading = true;
    runtimeError = null;
    try {
      runtime = await getConversationRuntimeStatus();
    } catch (err) {
      runtimeError = err instanceof Error ? err.message : String(err);
    } finally {
      if (!silent) runtimeLoading = false;
      runtimeRefreshInFlight = false;
    }
  }

  async function refreshInbox() {
    try {
      inbox = await listConversationInbox();
      inboxStale = false;
      if (selectedKey && !inbox.some((i) => i.key === selectedKey) && inbox.length > 0) {
        selectItem(inbox[0]);
      }
    } catch (err) {
      console.error('Failed to refresh inbox', err);
    }
  }

  function selectItem(item: InboxItem) {
    selectedKey = item.key;

    if (item.kind === 'group' || item.leadId === null) {
      activeChannels = [];
    } else {
      if (activeChannelsPerKey.has(item.key)) {
        activeChannels = activeChannelsPerKey.get(item.key)!;
      } else {
        activeChannels = item.channels.map((c) => c.channel);
        activeChannelsPerKey.set(item.key, activeChannels);
      }
    }

    loadOrFetchVariant(item, activeChannels);
  }

  function handleSidebarSelect(item: InboxItem) {
    if (selectedKey === item.key) return;
    selectItem(item);
  }

  function handleToggleChannel(ch: Channel) {
    if (!selectedItem || selectedItem.kind === 'group' || selectedItem.leadId === null) return;

    let updated: Channel[];
    if (activeChannels.includes(ch)) {
      updated = activeChannels.filter((c) => c !== ch);
    } else {
      updated = [...activeChannels, ch];
    }

    activeChannels = updated;
    activeChannelsPerKey.set(selectedItem.key, updated);

    loadOrFetchVariant(selectedItem, updated);
  }

  async function loadOrFetchVariant(item: InboxItem, channels: Channel[]) {
    if (item.kind !== 'group' && item.leadId !== null && channels.length === 0) {
      currentVariant = {
        items: [],
        nextCursor: null,
        hasLoaded: true,
        loading: false
      };
      return;
    }

    const key =
      item.kind === 'group' || item.leadId === null
        ? `${item.key}:group`
        : `${item.key}:${channelSetKey(channels)}`;

    const anchor = timelineRef?.captureAnchor() ?? null;

    if (variantsCache.has(key)) {
      const cached = variantsCache.get(key)!;
      currentVariant = cached;
      if (anchor) {
        const survivingId = nearestSurvivingAnchor(currentVariant.items, cached.items, anchor.messageId);
        if (survivingId !== null) {
          timelineRef?.restoreAnchor({ messageId: survivingId, offsetTop: anchor.offsetTop });
        }
      }
      if (!cached.hasLoaded && !cached.loading) {
        fetchVariantFirstPage(item, channels, key, anchor);
      }
    } else {
      // Create initial variant with instant filtered messages across existing cache
      const allItemMsgs: ConversationMessage[] = [];
      for (const cacheKey of variantsCache.keys()) {
        if (cacheKey.startsWith(`${item.key}:`)) {
          const v = variantsCache.get(cacheKey);
          if (v) allItemMsgs.push(...v.items);
        }
      }

      const instantFiltered =
        item.kind === 'group' || item.leadId === null
          ? mergeMessages([], allItemMsgs)
          : filterMessages(mergeMessages([], allItemMsgs), channels);

      const initialVar: TimelineVariant = {
        items: instantFiltered,
        nextCursor: null,
        hasLoaded: false,
        loading: true
      };

      variantsCache.set(key, initialVar);
      currentVariant = initialVar;

      fetchVariantFirstPage(item, channels, key, anchor);
    }
  }

  async function fetchVariantFirstPage(
    item: InboxItem,
    channels: Channel[],
    key: string,
    anchor: { messageId: number; offsetTop: number } | null
  ) {
    const existing = variantsCache.get(key) || currentVariant;
    currentVariant = { ...existing, loading: true };
    variantsCache.set(key, currentVariant);

    try {
      let page;
      if (item.kind === 'group' || item.leadId === null) {
        if (item.conversationId === null) return;
        page = await getConversationTimeline(item.conversationId, null, 50);
      } else {
        if (item.leadId === null) return;
        page = await getContactTimeline(item.leadId, channels, null, 50);
      }

      const updated = mergeTimelinePage(currentVariant, page, 'refresh');
      variantsCache.set(key, updated);

      if (currentVariantKey === key) {
        currentVariant = updated;
        if (anchor) {
          const survivingId = nearestSurvivingAnchor(
            existing.items,
            updated.items,
            anchor.messageId
          );
          if (survivingId !== null) {
            timelineRef?.restoreAnchor({ messageId: survivingId, offsetTop: anchor.offsetTop });
          }
        }
      }
    } catch (err) {
      console.error('Failed to fetch timeline variant', err);
      const failed = { ...currentVariant, loading: false };
      variantsCache.set(key, failed);
      if (currentVariantKey === key) {
        currentVariant = failed;
      }
    }
  }

  async function handleLoadOlder() {
    if (!selectedItem || loadingOlder || !currentVariant.nextCursor) return;
    if (selectedItem.kind !== 'group' && activeChannels.length === 0) return;

    loadingOlder = true;
    const key = currentVariantKey;

    try {
      let page;
      if (selectedItem.kind === 'group' || selectedItem.leadId === null) {
        if (selectedItem.conversationId === null) return;
        page = await getConversationTimeline(
          selectedItem.conversationId,
          currentVariant.nextCursor,
          50
        );
      } else {
        if (selectedItem.leadId === null) return;
        page = await getContactTimeline(
          selectedItem.leadId,
          activeChannels,
          currentVariant.nextCursor,
          50
        );
      }

      const updated = mergeTimelinePage(currentVariant, page, 'older');
      variantsCache.set(key, updated);
      if (currentVariantKey === key) {
        currentVariant = updated;
      }
    } catch (err) {
      console.error('Failed to load older messages', err);
    } finally {
      loadingOlder = false;
    }
  }

  async function refreshOpenTimeline() {
    if (!selectedItem) return;
    if (selectedItem.kind !== 'group' && activeChannels.length === 0) return;

    const anchor = timelineRef?.captureAnchor() ?? null;
    await fetchVariantFirstPage(selectedItem, activeChannels, currentVariantKey, anchor);
  }

  function handleInvalidatedEvent(event: ConversationInvalidatedEvent) {
    const isActive = activeTab === 'conversation';

    if (isActive) {
      if (inboxRefreshTimer) clearTimeout(inboxRefreshTimer);
      inboxRefreshTimer = setTimeout(() => {
        refreshInbox();
      }, 50);

      const affectsOpenItem =
        selectedItem !== null &&
        ((selectedItem.kind !== 'group' &&
          selectedItem.leadId !== null &&
          event.leadId === selectedItem.leadId) ||
          (selectedItem.kind === 'group' &&
            selectedItem.conversationId !== null &&
            event.conversationId === selectedItem.conversationId));

      if (affectsOpenItem) {
        if (timelineRefreshTimer) clearTimeout(timelineRefreshTimer);
        timelineRefreshTimer = setTimeout(() => {
          refreshOpenTimeline();
        }, 50);
      }
    } else {
      inboxStale = true;
      if (selectedItem) {
        const affectsOpenItem =
          (selectedItem.kind !== 'group' &&
            selectedItem.leadId !== null &&
            event.leadId === selectedItem.leadId) ||
          (selectedItem.kind === 'group' &&
            selectedItem.conversationId !== null &&
            event.conversationId === selectedItem.conversationId);

        if (affectsOpenItem && currentVariantKey) {
          timelineStale.add(currentVariantKey);
        }
      }
    }
  }

  function handleQueuedMessage(msg: ConversationMessage) {
    if (!selectedItem) return;

    const merged = mergeMessages(currentVariant.items, [msg]);
    currentVariant = { ...currentVariant, items: merged };
    variantsCache.set(currentVariantKey, currentVariant);

    // Update inbox preview
    inbox = inbox.map((item) => {
      if (item.key === selectedItem?.key) {
        return {
          ...item,
          lastMessagePreview: msg.textContent || 'Tin nhắn mới',
          lastMessageAt: msg.occurredAt
        };
      }
      return item;
    });

    tick().then(() => {
      timelineRef?.scrollToBottom();
    });
  }

  function handleLinked(leadId: number) {
    refreshInbox();
    refreshOpenTimeline();
  }

  function handleUnlinked() {
    refreshInbox();
    refreshOpenTimeline();
  }

  function getParticipantsPreview(item: InboxItem): string {
    if (!item.participants || item.participants.length === 0) return '';
    return item.participants.map((p) => p.displayName || 'Thành viên').join(', ');
  }
</script>

<div class="flex h-full w-full min-h-0 bg-background overflow-hidden">
  <!-- Column 1: Sidebar -->
  <div class="w-[320px] shrink-0 h-full min-h-0 border-r border-border">
    <ConversationSidebar
      items={inbox}
      selectedKey={selectedKey}
      loading={inboxLoading}
      onSelect={handleSidebarSelect}
    />
  </div>

  <!-- Column 2: Selected Conversation Workspace -->
  <div class="flex-1 flex flex-col h-full min-h-0 bg-background relative overflow-hidden">
    {#if selectedItem}
      <!-- Header -->
      <div class="p-3 border-b border-border flex flex-col gap-2 bg-background shrink-0">
        <div class="flex items-center justify-between gap-3">
          <div class="flex items-center gap-3 min-w-0">
            {#if selectedItem.avatarUrl}
              <img
                src={selectedItem.avatarUrl}
                alt={selectedItem.title}
                class="w-9 h-9 rounded-full object-cover shrink-0"
              />
            {:else}
              <div
                class="w-9 h-9 rounded-full bg-primary/10 text-primary font-semibold flex items-center justify-center shrink-0 text-xs"
              >
                {selectedItem.title ? selectedItem.title.slice(0, 2).toUpperCase() : '?'}
              </div>
            {/if}

            <div class="min-w-0">
              <h2 class="font-semibold text-sm truncate">{selectedItem.title}</h2>
              {#if selectedItem.kind === 'group'}
                <div class="flex items-center gap-1.5 text-xs text-muted-foreground truncate">
                  <Users class="w-3.5 h-3.5 shrink-0" />
                  <span>{selectedItem.participantCount} thành viên</span>
                  {#if getParticipantsPreview(selectedItem)}
                    <span class="truncate">({getParticipantsPreview(selectedItem)})</span>
                  {/if}
                </div>
              {/if}
            </div>
          </div>

          {#if selectedItem.kind === 'unknown'}
            <UnknownContactActions item={selectedItem} onLinked={handleLinked} />
          {:else if selectedItem.kind === 'customer' || selectedItem.kind === 'lead'}
            <LinkedLeadActions item={selectedItem} onRelinked={handleLinked} onUnlinked={handleUnlinked} />
          {/if}
        </div>

        {#if selectedItem.kind !== 'group'}
          <ChannelBadges
            channels={selectedItem.channels}
            activeChannels={activeChannels}
            onToggle={handleToggleChannel}
          />
        {/if}
      </div>

      <!-- Runtime status error notice if any -->
      {#if runtimeError}
        <div
          class="bg-destructive/10 text-destructive text-xs p-2.5 border-b border-destructive/20 flex items-center justify-between gap-2 shrink-0"
          role="alert"
        >
          <div class="flex items-center gap-1.5 min-w-0">
            <AlertCircle class="w-4 h-4 shrink-0" />
            <span class="truncate">Trạng thái tài khoản gián đoạn: {runtimeError}</span>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onclick={() => refreshRuntimeStatus()}
            class="h-7 text-xs flex items-center gap-1 shrink-0"
          >
            <RefreshCw class="w-3 h-3" />
            <span>Thử lại</span>
          </Button>
        </div>
      {/if}

      <!-- Timeline -->
      <ConversationTimeline
        bind:this={timelineRef}
        items={currentVariant.items}
        loading={currentVariant.loading}
        loadingOlder={loadingOlder}
        hasMore={Boolean(currentVariant.nextCursor)}
        emptyMessage={activeChannels.length === 0 && selectedItem.kind !== 'group'
          ? 'Không có kênh nào đang được hiển thị'
          : 'Chưa có tin nhắn trong cuộc trò chuyện này'}
        onLoadOlder={handleLoadOlder}
      />

      <!-- Message Composer -->
      <MessageComposer
        item={selectedItem}
        runtime={runtime}
        disabled={runtimeLoading}
        onQueued={handleQueuedMessage}
        {onOpenSettings}
        onRuntimeRefresh={() => refreshRuntimeStatus()}
      />
    {:else}
      <div class="flex-1 flex flex-col items-center justify-center p-6 text-muted-foreground text-center">
        <MessageSquare class="w-12 h-12 mb-3 opacity-30" />
        <h3 class="font-medium text-base text-foreground mb-1">Chưa có hội thoại nào được chọn</h3>
        <p class="text-xs text-muted-foreground">
          {inbox.length === 0 ? 'Chưa có hội thoại' : 'Chọn một cuộc trò chuyện từ danh sách bên trái'}
        </p>
      </div>
    {/if}
  </div>
</div>
