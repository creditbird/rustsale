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
  import GroupMembersModal from './GroupMembersModal.svelte';
  import NewChatDialog from './NewChatDialog.svelte';
  import UserProfilePopover, { type UserClickPayload } from './UserProfilePopover.svelte';
  import CreateLeadDialog from './CreateLeadDialog.svelte';

  export interface TargetLeadPayload {
    leadId?: number;
    phone?: string;
    name?: string;
    zalo_uid?: string;
    identityId?: number;
  }

  let {
    activeTab = 'conversation',
    targetLead = null,
    onOpenSettings = () => {},
    onOpenQrLogin = () => {},
    runtimeRefreshIntervalMs = 5_000
  }: {
    activeTab?: string;
    targetLead?: TargetLeadPayload | null;
    onOpenSettings?: () => void;
    onOpenQrLogin?: (accountKey?: string) => void;
    runtimeRefreshIntervalMs?: number;
  } = $props();

  let galleryOpen = $state(false);
  let galleryImages = $state<GalleryImage[]>([]);
  let galleryIndex = $state(0);
  let groupMembersModalOpen = $state(false);
  let userPopoverOpen = $state(false);
  let userPopoverPayload = $state<UserClickPayload | null>(null);

  let createLeadOpen = $state(false);
  let createLeadName = $state('');
  let createLeadPhoneOrUid = $state('');
  let createLeadIdentityId = $state<number | null>(null);

  function handleUserClick(payload: UserClickPayload) {
    userPopoverPayload = payload;
    userPopoverOpen = true;
  }

  function handleOpenDirectChatFromPopover(payload: UserClickPayload) {
    let targetPhoneOrUid = payload.externalId;
    if (!targetPhoneOrUid && selectedItem?.participants) {
      const match = selectedItem.participants.find((p) => {
        if (payload.identityId && p.identityId === payload.identityId) return true;
        if (payload.displayName && p.displayName && p.displayName.trim().toLowerCase() === payload.displayName.trim().toLowerCase()) return true;
        return false;
      });
      if (match?.externalId) {
        targetPhoneOrUid = match.externalId;
      }
    }

    const matching = findMatchingInboxItem({
      identityId: payload.identityId ?? undefined,
      zalo_uid: targetPhoneOrUid,
      name: payload.displayName ?? undefined
    });

    if (matching) {
      selectItem(matching);
    } else {
      newChatInitialPhone = targetPhoneOrUid || (payload.identityId ? String(payload.identityId) : '');
      newChatInitialName = payload.displayName || '';
      newChatOpen = true;
    }
  }

  function handleOpenLeadFromPopover(payload: UserClickPayload) {
    let targetPhoneOrUid = payload.externalId;
    if (!targetPhoneOrUid && selectedItem?.participants) {
      const match = selectedItem.participants.find((p) => {
        if (payload.identityId && p.identityId === payload.identityId) return true;
        if (payload.displayName && p.displayName && p.displayName.trim().toLowerCase() === payload.displayName.trim().toLowerCase()) return true;
        return false;
      });
      if (match?.externalId) {
        targetPhoneOrUid = match.externalId;
      }
    }

    const matching = findMatchingInboxItem({
      identityId: payload.identityId ?? undefined,
      zalo_uid: targetPhoneOrUid,
      name: payload.displayName ?? undefined
    });

    if (matching) {
      selectItem(matching);
    } else {
      createLeadName = payload.displayName || '';
      createLeadPhoneOrUid = targetPhoneOrUid || (payload.identityId ? String(payload.identityId) : '');
      createLeadIdentityId = payload.identityId ?? null;
      createLeadOpen = true;
    }
  }

  function handleLeadCreatedFromDialog(leadId: number) {
    void refreshInbox();
    const matching = findMatchingInboxItem({
      leadId,
      zalo_uid: createLeadPhoneOrUid,
      name: createLeadName
    });
    if (matching) {
      selectItem(matching);
    }
  }

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

  let newChatOpen = $state(false);
  let newChatInitialPhone = $state('');
  let newChatInitialName = $state('');
  let handledTargetLeadKey = $state<string | null>(null);

  function findMatchingInboxItem(lead: TargetLeadPayload): InboxItem | null {
    if (!lead || inbox.length === 0) return null;

    // 0. Match by identityId
    if (lead.identityId) {
      const matchIdentity = inbox.find(
        (i) => i.identityId === lead.identityId || i.key === `identity:${lead.identityId}`
      );
      if (matchIdentity) return matchIdentity;
    }

    // 1. Match by leadId
    if (lead.leadId) {
      const match = inbox.find((i) => i.leadId === lead.leadId);
      if (match) return match;
    }

    // 2. Match by zalo_uid
    if (lead.zalo_uid) {
      const cleanUid = lead.zalo_uid.trim();
      if (cleanUid) {
        const matchUid = inbox.find((i) => {
          const keyMatch = i.key ? i.key.includes(cleanUid) : false;
          const titleMatch = i.title ? i.title.includes(cleanUid) : false;
          const participantMatch = i.participants?.some((p) => String(p.identityId) === cleanUid);
          return keyMatch || titleMatch || participantMatch;
        });
        if (matchUid) return matchUid;
      }
    }

    // 3. Match by phone number
    const cleanPhone = lead.phone ? lead.phone.replace(/[^0-9]/g, '') : '';
    if (cleanPhone && cleanPhone.length >= 6) {
      const matchPhone = inbox.find((i) => {
        const titleClean = i.title ? i.title.replace(/[^0-9]/g, '') : '';
        const keyClean = i.key ? i.key.replace(/[^0-9]/g, '') : '';
        const participantPhoneMatch = i.participants?.some((p) => {
          const pNameClean = p.displayName ? p.displayName.replace(/[^0-9]/g, '') : '';
          return pNameClean && pNameClean.includes(cleanPhone);
        });
        return (titleClean && titleClean.includes(cleanPhone)) || (keyClean && keyClean.includes(cleanPhone)) || participantPhoneMatch;
      });
      if (matchPhone) return matchPhone;
    }

    // 4. Match by Lead Name (excluding group chats)
    if (lead.name) {
      const leadNameLower = lead.name.trim().toLowerCase();
      const matchName = inbox.find((i) => {
        if (!i.title || i.kind === 'group') return false;
        const titleLower = i.title.trim().toLowerCase();
        return titleLower.includes(leadNameLower) || leadNameLower.includes(titleLower);
      });
      if (matchName) return matchName;
    }

    return null;
  }

  function applyTargetLead(): boolean {
    const hasValidLead = Boolean(targetLead && (targetLead.leadId || targetLead.phone || targetLead.name || targetLead.zalo_uid));
    if (!hasValidLead || !targetLead) return false;

    const found = findMatchingInboxItem(targetLead);
    if (found) {
      selectItem(found);
      return true;
    } else {
      newChatInitialPhone = targetLead.phone || targetLead.zalo_uid || '';
      newChatInitialName = targetLead.name || '';
      newChatOpen = true;
      return true;
    }
  }

  $effect(() => {
    const hasValidLead = Boolean(targetLead && (targetLead.leadId || targetLead.phone || targetLead.name || targetLead.zalo_uid));
    if (activeTab === 'conversation' && hasValidLead && targetLead) {
      const targetKey = `${targetLead.leadId || ''}_${targetLead.phone || ''}_${targetLead.name || ''}_${targetLead.zalo_uid || ''}`;
      if (handledTargetLeadKey !== targetKey) {
        handledTargetLeadKey = targetKey;
        if (inbox.length > 0) {
          applyTargetLead();
        }
      }
    }
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

        const handledTarget = applyTargetLead();
        if (!handledTarget && !selectedKey && inbox.length > 0) {
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
    action: 'increment' | 'remove' = 'increment'
  ) {
    return mutationStateRef?.executeMutation(message.id, code, action) ?? Promise.resolve(null);
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

  async function handleConversationCreated(msg: ConversationMessage) {
    await refreshInbox();
    const createdItem = inbox.find(
      (i) =>
        i.conversationId === msg.conversationId ||
        (msg.senderIdentityId && i.identityId === msg.senderIdentityId) ||
        (msg.senderIdentityId && i.key === `identity:${msg.senderIdentityId}`)
    );
    if (createdItem) {
      selectItem(createdItem);
    } else if (inbox.length > 0) {
      selectItem(inbox[0]);
    }
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
                <button
                  type="button"
                  class="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-primary transition-colors cursor-pointer truncate text-left group"
                  onclick={() => (groupMembersModalOpen = true)}
                  title="Xem các thành viên đã đồng bộ và thêm Lead/Category"
                >
                  <Users class="w-3.5 h-3.5 shrink-0 group-hover:scale-110 transition-transform" />
                  <span class="font-medium underline decoration-dashed underline-offset-2">{selectedItem.participantCount} thành viên</span>
                  {#if getParticipantsPreview(selectedItem)}
                    <span class="truncate">({getParticipantsPreview(selectedItem)})</span>
                  {/if}
                </button>
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
        onUserClick={handleUserClick}
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
  <GroupMembersModal
    open={groupMembersModalOpen}
    conversationId={selectedItem?.conversationId ?? null}
    groupTitle={selectedItem?.title ?? ''}
    onClose={() => (groupMembersModalOpen = false)}
  />
  <NewChatDialog
    bind:open={newChatOpen}
    initialPhone={newChatInitialPhone}
    initialDisplayName={newChatInitialName}
    onSuccess={handleConversationCreated}
  />
  <UserProfilePopover
    open={userPopoverOpen}
    payload={userPopoverPayload}
    onClose={() => (userPopoverOpen = false)}
    onOpenDirectChat={handleOpenDirectChatFromPopover}
    onOpenLead={handleOpenLeadFromPopover}
  />
  <CreateLeadDialog
    open={createLeadOpen}
    initialName={createLeadName}
    initialPhoneOrUid={createLeadPhoneOrUid}
    identityId={createLeadIdentityId}
    onClose={() => (createLeadOpen = false)}
    onSuccess={handleLeadCreatedFromDialog}
  />
</div>
