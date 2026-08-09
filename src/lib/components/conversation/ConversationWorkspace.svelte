<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import {
    listConversationInbox,
    markConversationRead,
    getContactTimeline,
    getConversationTimeline,
    getIdentityTimeline,
    getConversationRuntimeStatus,
    initConversationEventBridge,
    onConversationInvalidated,
    recallConversationMessage,
    pinConversationMessage,
    unpinConversationMessage,
    listConversationPinnedMessages,
    type InboxItem,

    type Channel,
    type ConversationCapabilities,
    type ConversationRuntimeStatus,
    type ConversationMessage,
    type ConversationInvalidatedEvent,
    type SendConversationBatchResult,
    type ConversationReactionCode,
    type ConversationReactionGroup,
    type PinnedMessage
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
  import RecallMessageDialog from './RecallMessageDialog.svelte';
  import ForwardConversationDialog from './ForwardConversationDialog.svelte';
  import MessageReactionMutationState from './MessageReactionMutationState.svelte';
  import PinnedMessageBanner from './PinnedMessageBanner.svelte';
  import InThreadSearchBar from './InThreadSearchBar.svelte';
  import type { ConversationReplySelection } from './composerDraftState';
  import { attachmentLabel, safeConversationUrl, extractGalleryImages } from './messageMedia';
  import { reactionIconToCode } from './reactionDisplay';
  import { Button } from '$lib/components/ui/button';
  import { AlertCircle, RefreshCw, Users, MessageSquare, Trash2, Search } from 'lucide-svelte';
  import DeleteConversationDialog from './DeleteConversationDialog.svelte';
  import ImageGalleryModal, { type GalleryImage } from './ImageGalleryModal.svelte';


  let {
    activeTab = 'conversation',
    onOpenSettings = () => {},
    onOpenQrLogin = () => {},
    runtimeRefreshIntervalMs = 5_000
  }: {
    activeTab?: string;
    onOpenSettings?: () => void;
    onOpenQrLogin?: (accountKey?: string) => void;
    runtimeRefreshIntervalMs?: number;
  } = $props();

  let galleryOpen = $state(false);
  let galleryImages = $state<GalleryImage[]>([]);
  let galleryIndex = $state(0);

  function handleImageClick(clickedUrl: string, _clickedMessage: ConversationMessage) {
    const items = currentVariant?.items ?? [];
    const images = extractGalleryImages(items);

    const safeClicked = safeConversationUrl(clickedUrl) ?? clickedUrl;
    if (images.length === 0 && safeClicked) {
      images.push({ url: safeClicked, alt: 'Hình ảnh' });
    }

    galleryImages = images;
    const foundIdx = images.findIndex((img) => img.url === safeClicked || img.url === clickedUrl);
    galleryIndex = foundIdx >= 0 ? foundIdx : 0;
    galleryOpen = true;
  }

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
  let mutationStateRef = $state<ReturnType<typeof MessageReactionMutationState> | null>(null);

  let replySelection = $state<ConversationReplySelection | null>(null);
  let recallCandidate = $state<ConversationMessage | null>(null);
  let recallBusy = $state(false);
  let recallError = $state<string | null>(null);
  let forwardCandidate = $state<ConversationMessage | null>(null);
  let forwardDialogOpen = $state(false);
  let actionItemKey = $state<string | null>(null);

  let pinnedMessages = $state<PinnedMessage[]>([]);
  let pinnedMessageIds = $derived(new Set((pinnedMessages || []).map((p) => p.messageId)));
  let inThreadSearchOpen = $state(false);

  let inThreadQuery = $state('');
  let inThreadMatchIndex = $state(0);

  let inboxRefreshTimer: ReturnType<typeof setTimeout> | null = null;
  let timelineRefreshTimer: ReturnType<typeof setTimeout> | null = null;

  let selectedItem = $derived(inbox.find((i) => i.key === selectedKey) ?? null);

  $effect(() => {
    const convId = selectedItem?.conversationId;
    if (convId) {
      void loadPinnedMessages(convId);
    } else {
      pinnedMessages = [];
    }
  });

  async function loadPinnedMessages(conversationId: number) {
    try {
      pinnedMessages = await listConversationPinnedMessages(conversationId);
    } catch {
      pinnedMessages = [];
    }
  }

  async function handlePinMessage(message: ConversationMessage) {
    const convId = message.conversationId || selectedItem?.conversationId;
    if (!convId) return;
    try {
      const isAlreadyPinned = pinnedMessages.some((p) => p.messageId === message.id);
      if (isAlreadyPinned) {
        await unpinConversationMessage(convId, message.id);
      } else {
        await pinConversationMessage(convId, message.id, 'user');
      }
      await loadPinnedMessages(convId);
    } catch (err) {
      console.error('Failed to pin/unpin message', err);
    }
  }

  async function handleUnpinMessage(messageId: number) {
    const convId = selectedItem?.conversationId;
    if (!convId) return;
    try {
      await unpinConversationMessage(convId, messageId);
      await loadPinnedMessages(convId);
    } catch (err) {
      console.error('Failed to unpin message', err);
    }
  }

  let inThreadMatches = $derived.by(() => {
    if (!inThreadQuery.trim()) return [];
    const q = inThreadQuery.toLowerCase().trim();
    const items = currentVariant?.items ?? [];
    return items.filter((msg) => {
      const text = msg.textContent || '';
      return text.toLowerCase().includes(q);
    });
  });

  function handleSearchNext() {
    if (inThreadMatches.length === 0) return;
    inThreadMatchIndex = (inThreadMatchIndex + 1) % inThreadMatches.length;
    scrollToMatchedMessage(inThreadMatches[inThreadMatchIndex]?.id);
  }

  function handleSearchPrev() {
    if (inThreadMatches.length === 0) return;
    inThreadMatchIndex = (inThreadMatchIndex - 1 + inThreadMatches.length) % inThreadMatches.length;
    scrollToMatchedMessage(inThreadMatches[inThreadMatchIndex]?.id);
  }

  function scrollToMatchedMessage(messageId?: number) {
    if (!messageId) return;
    const el = document.querySelector(`[data-message-id="${messageId}"]`);
    if (el) {
      el.scrollIntoView({ behavior: 'smooth', block: 'center' });
      el.classList.add('ring-2', 'ring-primary');
      setTimeout(() => el.classList.remove('ring-2', 'ring-primary'), 2000);
    }
  }

  $effect(() => {
    if (selectedKey !== actionItemKey) {
      actionItemKey = selectedKey;
      replySelection = null;
      recallCandidate = null;
      recallError = null;
      forwardCandidate = null;
      forwardDialogOpen = false;
      inThreadSearchOpen = false;
      inThreadQuery = '';
    }
  });


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

    item.unreadCount = 0;
    if (item.channels) {
      item.channels = item.channels.map((ch) => ({ ...ch, unreadCount: 0 }));
    }

    markConversationRead(item.key).catch((err) => {
      console.error('Failed to mark conversation read', err);
    });

    if (item.kind === 'group') {

      activeChannels = [];
    } else {
      const availableChannels = item.channels.map((channel) => channel.channel);
      const cachedChannels = (activeChannelsPerKey.get(item.key) ?? []).filter((channel) =>
        availableChannels.includes(channel)
      );
      activeChannels = cachedChannels.length > 0 ? cachedChannels : availableChannels;
      activeChannelsPerKey.set(item.key, activeChannels);
    }

    loadOrFetchVariant(item, activeChannels, true);
  }

  function handleSidebarSelect(item: InboxItem) {
    if (selectedKey === item.key) return;
    selectItem(item);
  }

  function handleToggleChannel(ch: Channel) {
    if (!selectedItem || selectedItem.kind === 'group' || selectedItem.leadId === null) return;

    let updated: Channel[];
    if (activeChannels.includes(ch)) {
      if (activeChannels.length === 1) {
        activeChannels = [...activeChannels];
        return;
      }
      updated = activeChannels.filter((c) => c !== ch);
    } else {
      updated = [...activeChannels, ch];
    }

    activeChannels = updated;
    activeChannelsPerKey.set(selectedItem.key, updated);

    loadOrFetchVariant(selectedItem, updated);
  }

  async function loadOrFetchVariant(
    item: InboxItem,
    channels: Channel[],
    isNewSelection = false
  ) {
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

    const anchor = isNewSelection ? null : (timelineRef?.captureAnchor() ?? null);

    if (variantsCache.has(key)) {
      const cached = variantsCache.get(key)!;
      currentVariant = cached;
      if (anchor) {
        const survivingId = nearestSurvivingAnchor(currentVariant.items, cached.items, anchor.messageId);
        if (survivingId !== null) {
          timelineRef?.restoreAnchor({ messageId: survivingId, offsetTop: anchor.offsetTop });
        } else {
          tick().then(() => timelineRef?.scrollToBottom());
        }
      } else {
        tick().then(() => timelineRef?.scrollToBottom());
      }
      if ((timelineStale.has(key) || !cached.hasLoaded) && !cached.loading) {
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
      if (item.kind === 'group') {
        if (item.conversationId === null) throw new Error('Group conversation ID is missing');
        page = await getConversationTimeline(item.conversationId, null, 50);
      } else if (item.kind === 'unknown') {
        if (item.identityId === null) throw new Error('Unknown contact identity ID is missing');
        page = await getIdentityTimeline(item.identityId, null, 50);
      } else {
        if (item.leadId === null) throw new Error('Customer lead ID is missing');
        page = await getContactTimeline(item.leadId, channels, null, 50);
      }

      const updated = mergeTimelinePage(currentVariant, page, 'refresh');
      variantsCache.set(key, updated);
      timelineStale.delete(key);

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
          } else {
            tick().then(() => timelineRef?.scrollToBottom());
          }
        } else {
          tick().then(() => timelineRef?.scrollToBottom());
        }
      }
    } catch (err) {
      console.error('Failed to fetch timeline variant', err);
      const failed = { ...currentVariant, loading: false };
      variantsCache.set(key, failed);
      timelineStale.add(key);
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
      if (selectedItem.kind === 'group') {
        if (selectedItem.conversationId === null) throw new Error('Group conversation ID is missing');
        page = await getConversationTimeline(
          selectedItem.conversationId,
          currentVariant.nextCursor,
          50
        );
      } else if (selectedItem.kind === 'unknown') {
        if (selectedItem.identityId === null) throw new Error('Unknown contact identity ID is missing');
        page = await getIdentityTimeline(selectedItem.identityId, currentVariant.nextCursor, 50);
      } else {
        if (selectedItem.leadId === null) throw new Error('Customer lead ID is missing');
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

  function eventAffectsItem(item: InboxItem, event: ConversationInvalidatedEvent): boolean {
    if (item.kind === 'customer') {
      return (
        (item.leadId !== null && event.leadId === item.leadId) ||
        item.replyTargets.some((target) => target.conversationId === event.conversationId)
      );
    }
    if (item.kind === 'group') {
      return item.conversationId !== null && event.conversationId === item.conversationId;
    }
    return item.replyTargets.some((target) => target.conversationId === event.conversationId);
  }

  function handleInvalidatedEvent(event: ConversationInvalidatedEvent) {
    const isActive = activeTab === 'conversation';
    const affectedItems = inbox.filter((item) => eventAffectsItem(item, event));

    for (const item of affectedItems) {
      for (const key of variantsCache.keys()) {
        if (key.startsWith(`${item.key}:`)) {
          timelineStale.add(key);
        }
      }
    }

    if (isActive) {
      if (inboxRefreshTimer) clearTimeout(inboxRefreshTimer);
      inboxRefreshTimer = setTimeout(() => {
        refreshInbox();
      }, 50);

      const affectsOpenItem = selectedItem !== null && eventAffectsItem(selectedItem, event);

      if (affectsOpenItem) {
        timelineStale.delete(currentVariantKey);
        if (timelineRefreshTimer) clearTimeout(timelineRefreshTimer);
        timelineRefreshTimer = setTimeout(() => {
          refreshOpenTimeline();
        }, 50);
      }
    } else {
      inboxStale = true;
      if (selectedItem) {
        const affectsOpenItem = eventAffectsItem(selectedItem, event);

        if (affectsOpenItem && currentVariantKey) {
          timelineStale.add(currentVariantKey);
        }
      }
    }
  }

  function messagePreview(message: ConversationMessage): string {
    if (message.textContent?.trim()) return message.textContent.trim();
    const attachment = message.attachments[0];
    if (attachment?.fileName) return attachment.fileName;
    if (attachment) return attachmentLabel(attachment.mediaType);
    if (message.contentType === 'sticker') return 'Sticker';
    return attachmentLabel(message.contentType);
  }

  function handleBatchQueued(result: SendConversationBatchResult) {
    if (!selectedItem || result.messages.length === 0) return;

    const realBatchIds = new Set(
      result.messages
        .filter((m) => m.id > 0)
        .map((m) => m.sendBatch?.sendBatchId)
        .filter(Boolean)
    );
    const cleanedItems = currentVariant.items.filter(
      (item) =>
        !(
          item.id < 0 &&
          item.sendBatch?.sendBatchId &&
          realBatchIds.has(item.sendBatch.sendBatchId)
        )
    );

    const merged = mergeMessages(cleanedItems, result.messages);
    currentVariant = { ...currentVariant, items: merged };
    variantsCache.set(currentVariantKey, currentVariant);

    const lastMessage = [...result.messages].sort((left, right) => {
      const indexDiff = (left.batchIndex ?? 0) - (right.batchIndex ?? 0);
      return indexDiff !== 0 ? indexDiff : left.id - right.id;
    }).at(-1)!;
    inbox = inbox.map((item) => {
      if (item.key === selectedItem?.key) {
        return {
          ...item,
          lastMessagePreview: messagePreview(lastMessage),
          lastMessageAt: lastMessage.occurredAt
        };
      }
      return item;
    });

    replySelection = null;

    tick().then(() => {
      timelineRef?.scrollToBottom();
    });
  }

  function capabilitiesForMessage(message: ConversationMessage): ConversationCapabilities | undefined {
    return runtime.accounts.find((account) => account.channelAccountId === message.channelAccountId)
      ?.capabilities;
  }

  function handleReply(message: ConversationMessage): void {
    replySelection = {
      messageId: message.id,
      conversationId: message.conversationId,
      channelAccountId: message.channelAccountId,
      preview: {
        messageId: message.id,
        senderDisplayName: message.senderDisplayName,
        contentType: message.contentType,
        textContent: message.textContent,
        mediaType: message.attachments[0]?.mediaType ?? null,
        occurredAt: message.occurredAt
      }
    };
  }

  function handleRecall(message: ConversationMessage): void {
    recallCandidate = message;
    recallError = null;
  }

  function handleForward(message: ConversationMessage): void {
    forwardCandidate = message;
    forwardDialogOpen = true;
  }

  async function confirmRecall(): Promise<void> {
    if (!recallCandidate || recallBusy) return;
    recallBusy = true;
    recallError = null;
    try {
      const tombstone = await recallConversationMessage(recallCandidate.id);
      currentVariant = {
        ...currentVariant,
        items: mergeMessages(currentVariant.items, [tombstone])
      };
      variantsCache.set(currentVariantKey, currentVariant);
      if (replySelection?.messageId === tombstone.id) replySelection = null;
      recallCandidate = null;
    } catch (error) {
      recallError = error instanceof Error ? error.message : String(error);
    } finally {
      recallBusy = false;
    }
  }

  function handleReactionsUpdated(messageId: number, reactions: ConversationReactionGroup[]) {
    const updated = currentVariant.items.map((msg) =>
      msg.id === messageId ? { ...msg, reactions } : msg
    );
    currentVariant = { ...currentVariant, items: updated };
    variantsCache.set(currentVariantKey, currentVariant);
  }

  function handleToggleReaction(
    message: ConversationMessage,
    code: ConversationReactionCode,
    currentActive?: boolean
  ) {
    const isActive =
      typeof currentActive === 'boolean'
        ? currentActive
        : message.reactions.some(
            (g) => g.includesMe && reactionIconToCode(g.reactionIcon) === code
          );
    void mutationStateRef?.executeMutation(message.id, code, isActive);
  }

  function handleLinked(leadId: number) {
    refreshInbox();
    refreshOpenTimeline();
  }

  function handleUnlinked() {
    refreshInbox();
    refreshOpenTimeline();
  }

  let deleteHeaderDialogOpen = $state(false);

  function handleConversationCreated(msg: ConversationMessage) {
    refreshInbox();
    refreshOpenTimeline();
  }

  function handleConversationDeleted() {
    selectedKey = null;
    refreshInbox();
  }

  function getParticipantsPreview(item: InboxItem): string {
    if (!item.participants || item.participants.length === 0) return '';
    return item.participants.map((p) => p.displayName || 'Thành viên').join(', ');
  }
</script>

<div class="flex h-full w-full min-h-0 bg-background overflow-hidden">
  <MessageReactionMutationState
    bind:this={mutationStateRef}
    onReactionsUpdated={handleReactionsUpdated}
  />

  <!-- Column 1: Sidebar -->
  <div class="w-[320px] shrink-0 h-full min-h-0 border-r border-border flex flex-col">
    <ConversationSidebar
      items={inbox}
      selectedKey={selectedKey}
      loading={inboxLoading}
      onSelect={handleSidebarSelect}
      onCreated={handleConversationCreated}
      onDeleted={handleConversationDeleted}
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

          <div class="flex items-center gap-2">
            {#if selectedItem.kind === 'unknown'}
              <UnknownContactActions item={selectedItem} onLinked={handleLinked} />
            {:else if selectedItem.kind === 'customer'}
              <LinkedLeadActions item={selectedItem} onRelinked={handleLinked} onUnlinked={handleUnlinked} />
            {/if}

            <Button
              variant="outline"
              size="icon"
              title="Tìm trong cuộc trò chuyện (Ctrl+F)"
              onclick={() => (inThreadSearchOpen = !inThreadSearchOpen)}
              class="h-8 w-8 text-muted-foreground hover:text-foreground"
            >
              <Search class="w-4 h-4" />
            </Button>

            <Button
              variant="outline"
              size="icon"
              title="Xóa cuộc trò chuyện này"
              onclick={() => (deleteHeaderDialogOpen = true)}
              class="h-8 w-8 text-muted-foreground hover:text-destructive hover:bg-destructive/10"
            >
              <Trash2 class="w-4 h-4" />
            </Button>
          </div>
        </div>

        {#if selectedItem.kind !== 'group'}
          <ChannelBadges
            channels={selectedItem.channels}
            activeChannels={activeChannels}
            onToggle={handleToggleChannel}
          />
        {/if}
      </div>

      <!-- In-Thread Search Bar & Pinned Banner -->
      <InThreadSearchBar
        bind:open={inThreadSearchOpen}
        bind:searchQuery={inThreadQuery}
        matchCount={inThreadMatches.length}
        currentIndex={inThreadMatchIndex}
        onNext={handleSearchNext}
        onPrev={handleSearchPrev}
        onClose={() => (inThreadSearchOpen = false)}
      />

      <PinnedMessageBanner
        pinnedMessages={pinnedMessages}
        onJumpToMessage={scrollToMatchedMessage}
        onUnpin={handleUnpinMessage}
      />

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
        isGroup={selectedItem.kind === 'group'}
        {capabilitiesForMessage}
        pendingReactionsForMessage={(msgId) => mutationStateRef?.getPendingReactionCodes(msgId) ?? []}
        {pinnedMessageIds}
        onReply={handleReply}

        onRecall={handleRecall}
        onForward={handleForward}
        onPin={handlePinMessage}
        onToggleReaction={handleToggleReaction}
        onImageClick={handleImageClick}
      />


      <!-- Message Composer -->
      <MessageComposer
        item={selectedItem}
        runtime={runtime}
        disabled={runtimeLoading}
        replyTarget={replySelection}
        onCancelReply={() => (replySelection = null)}
        onBatchQueued={handleBatchQueued}
        {onOpenSettings}
        {onOpenQrLogin}
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

  <DeleteConversationDialog
    bind:open={deleteHeaderDialogOpen}
    item={selectedItem}
    onDeleted={handleConversationDeleted}
  />
  <RecallMessageDialog
    message={recallCandidate}
    busy={recallBusy}
    error={recallError}
    onConfirm={confirmRecall}
    onCancel={() => {
      if (!recallBusy) {
        recallCandidate = null;
        recallError = null;
      }
    }}
  />
  <ForwardConversationDialog
    bind:open={forwardDialogOpen}
    sourceMessage={forwardCandidate}
    inbox={inbox}
    maxTargets={forwardCandidate
      ? capabilitiesForMessage(forwardCandidate)?.forward.maxTargets ?? 10
      : 10}
  />
  <ImageGalleryModal
    open={galleryOpen}
    images={galleryImages}
    initialIndex={galleryIndex}
    onClose={() => (galleryOpen = false)}
    onIndexChange={(idx) => (galleryIndex = idx)}
  />
</div>
