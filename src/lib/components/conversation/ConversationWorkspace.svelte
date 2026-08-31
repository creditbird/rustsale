<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import {
    listConversationInbox,
    markConversationRead,
    getContactTimeline,
    getConversationTimeline,
    getIdentityTimeline,
    getConversationRuntimeStatus,
    listConversationMentionCandidates,
    initConversationEventBridge,
    onConversationInvalidated,
    onConversationMessageDelta,
    onConversationDeltaLagged,
    onConversationBridgeReconnected,
    recallConversationMessage,
    pinConversationMessage,
    unpinConversationMessage,
    listConversationPinnedMessages,
    syncFacebookConversations,
    syncZaloGroupChatHistory,
    syncWhatsAppConversations,
    resumeConversationDeltas,
    getConversationHighWaterMark,
    type InboxItem,
    type TimelinePage,
    type Channel,
    type ConversationCapabilities,
    type ConversationRuntimeStatus,
    type ConversationMessage,
    type ConversationInvalidatedEvent,
    type ConversationMessageDeltaEvent,
    type SendConversationBatchResult,
    type ConversationReactionCode,
    type ConversationReactionGroup,
    type PinnedMessage
  } from '$lib/services/conversationService';
  import { startMetaRealtimeListener } from '$lib/services/metaRealtimeService';
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
  import Facebook24hPolicyBanner from './Facebook24hPolicyBanner.svelte';
  import type { InboxReplyTarget } from '$lib/services/conversationService';
  import { evaluateFacebookWindow } from './facebookPolicyGate';
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
  import UserProfilePopover, { type UserClickPayload, isValidExternalZaloId } from './UserProfilePopover.svelte';
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
    runtimeRefreshIntervalMs = 30_000
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

  function normalizedDisplayName(value?: string | null): string {
    return value?.trim().replace(/\s+/g, ' ').toLowerCase() ?? '';
  }

  function hasUsableExternalZaloId(
    externalId?: string | null,
    identityId?: number | null
  ): boolean {
    if (!isValidExternalZaloId(externalId)) return false;
    return identityId == null || externalId?.trim() !== String(identityId);
  }

  async function resolveParticipantDetails(payload: UserClickPayload): Promise<{
    externalId: string | undefined;
    identityId: number | undefined;
  }> {
    let resolvedExtId = hasUsableExternalZaloId(payload.externalId, payload.identityId)
      ? payload.externalId?.trim()
      : undefined;
    let resolvedIdentityId = payload.identityId;
    const cleanName = normalizedDisplayName(payload.displayName);

    if (selectedItem?.participants && selectedItem.participants.length > 0) {
      const participant = selectedItem.participants.find((p) => {
        if (resolvedIdentityId && p.identityId === resolvedIdentityId) return true;
        if (resolvedExtId && p.externalId === resolvedExtId) return true;
        return cleanName !== '' && normalizedDisplayName(p.displayName) === cleanName;
      });

      if (participant) {
        if (
          !resolvedExtId &&
          hasUsableExternalZaloId(participant.externalId, participant.identityId)
        ) {
          resolvedExtId = participant.externalId ?? undefined;
        }
        if (!resolvedIdentityId && participant.identityId) {
          resolvedIdentityId = participant.identityId;
        }
      }
    }

    if (!resolvedExtId && selectedItem?.conversationId) {
      try {
        const candidates = await listConversationMentionCandidates({
          conversationId: selectedItem.conversationId,
          sourceMessageId: payload.sourceMessageId,
          query: payload.displayName || undefined
        });

        const exactCandidate = candidates.find((c) => {
          if (resolvedIdentityId && c.identityId === resolvedIdentityId) return true;
          if (resolvedExtId && c.externalId === resolvedExtId) return true;
          return cleanName !== '' && normalizedDisplayName(c.displayName) === cleanName;
        });
        const fuzzyMatches = exactCandidate || cleanName === ''
          ? []
          : candidates.filter((candidate) => {
              const candidateName = normalizedDisplayName(candidate.displayName);
              return candidateName.startsWith(`${cleanName} `) || cleanName.startsWith(`${candidateName} `);
            });
        const candidate = exactCandidate ?? (fuzzyMatches.length === 1 ? fuzzyMatches[0] : undefined);

        if (candidate) {
          if (hasUsableExternalZaloId(candidate.externalId, candidate.identityId)) {
            resolvedExtId = candidate.externalId;
          }
          if (candidate.identityId) {
            resolvedIdentityId = candidate.identityId;
          }
        }
      } catch (err) {
        console.error('Failed to resolve mention candidate for popover user click:', err);
      }
    }

    return {
      externalId: resolvedExtId || undefined,
      identityId: resolvedIdentityId || undefined
    };
  }

  async function handleUserClick(payload: UserClickPayload) {
    const hasExternalId = hasUsableExternalZaloId(payload.externalId, payload.identityId);
    userPopoverPayload = hasExternalId ? payload : { ...payload, externalId: undefined };
    userPopoverOpen = true;

    if (!hasExternalId) {
      const resolved = await resolveParticipantDetails(payload);
      if (resolved.externalId || resolved.identityId) {
        userPopoverPayload = {
          ...payload,
          identityId: resolved.identityId ?? payload.identityId ?? null,
          externalId: resolved.externalId
        };
      }
    }
  }

  async function handleOpenDirectChatFromPopover(payload: UserClickPayload) {
    let resolved = { externalId: payload.externalId, identityId: payload.identityId ?? undefined };
    if (!hasUsableExternalZaloId(resolved.externalId, resolved.identityId)) {
      resolved = await resolveParticipantDetails(payload);
    }

    const targetPhoneOrUid = resolved.externalId;
    const targetIdentityId = resolved.identityId ?? payload.identityId;

    const matching = findMatchingInboxItem({
      identityId: targetIdentityId ?? undefined,
      zalo_uid: targetPhoneOrUid,
      name: payload.displayName ?? undefined
    });

    if (matching) {
      selectItem(matching);
    } else {
      newChatInitialPhone = hasUsableExternalZaloId(targetPhoneOrUid, targetIdentityId)
        ? targetPhoneOrUid || ''
        : '';
      newChatInitialName = payload.displayName || '';
      newChatOpen = true;
    }
  }

  async function handleOpenLeadFromPopover(payload: UserClickPayload) {
    let resolved = { externalId: payload.externalId, identityId: payload.identityId ?? undefined };
    if (!hasUsableExternalZaloId(resolved.externalId, resolved.identityId)) {
      resolved = await resolveParticipantDetails(payload);
    }

    const targetPhoneOrUid = resolved.externalId;
    const targetIdentityId = resolved.identityId ?? payload.identityId;

    const matching = findMatchingInboxItem({
      identityId: targetIdentityId ?? undefined,
      zalo_uid: targetPhoneOrUid,
      name: payload.displayName ?? undefined
    });

    if (matching?.leadId) {
      selectItem(matching);
    } else {
      createLeadName = payload.displayName || '';
      createLeadPhoneOrUid = hasUsableExternalZaloId(targetPhoneOrUid, targetIdentityId)
        ? targetPhoneOrUid || ''
        : '';
      createLeadIdentityId = targetIdentityId ?? matching?.identityId ?? null;
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

  function handleImageClick(clickedUrl: string, clickedMessage?: ConversationMessage) {
    const items = currentVariant?.items ?? [];
    const images = extractGalleryImages(items);

    const safeClicked = safeConversationUrl(clickedUrl) ?? clickedUrl;
    if (safeClicked && !images.some((img) => img.url === safeClicked || img.url === clickedUrl)) {
      images.push({
        url: safeClicked,
        alt: 'Image',
        caption: clickedMessage?.textContent || 'Image',
        timestamp: clickedMessage?.occurredAt
      });
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

  // Production-ready memoization: keep stable object identity while typing
  // to prevent MessageComposer prop thrashing on every list_inbox refresh (434 items).
  // WhatsApp/Telegram keep current chat snapshot isolated from inbox list updates.
  let selectedItem = $state<InboxItem | null>(null);
  $effect(() => {
    const key = selectedKey;
    const currentInbox = inbox;
    untrack(() => {
      if (!key) {
        if (selectedItem !== null) selectedItem = null;
        return;
      }
      const found = currentInbox.find((i) => i.key === key) ?? null;
      if (!found) {
        if (selectedItem !== null) selectedItem = null;
        return;
      }
      if (selectedItem && selectedItem.key === found.key) {
        const prev = selectedItem;
        const sameTargets =
          prev.replyTargets.length === found.replyTargets.length &&
          prev.replyTargets.every((t, idx) => {
            const f = found.replyTargets[idx];
            return (
              f !== undefined &&
              t.conversationId === f.conversationId &&
              t.channelAccountId === f.channelAccountId &&
              t.channel === f.channel &&
              t.accountDisplayName === f.accountDisplayName &&
              t.conversationKind === f.conversationKind &&
              t.lastInboundAt === f.lastInboundAt &&
              t.lastMessageAt === f.lastMessageAt
            );
          });
        const sameChannels =
          prev.channels.length === found.channels.length &&
          prev.channels.every((c, idx) => c === found.channels[idx]);

        const sameItem =
          prev.title === found.title &&
          prev.lastMessageAt === found.lastMessageAt &&
          prev.lastMessagePreview === found.lastMessagePreview &&
          prev.avatarUrl === found.avatarUrl &&
          prev.kind === found.kind &&
          prev.leadId === found.leadId &&
          prev.conversationId === found.conversationId &&
          prev.participantCount === found.participantCount &&
          sameChannels &&
          sameTargets;
        if (sameItem) return; // keep previous reference — no downstream re-render

        // If last message changed on the active conversation from background/inbox sync, ensure timeline is refreshed
        if (prev.lastMessageAt !== found.lastMessageAt || prev.lastMessagePreview !== found.lastMessagePreview) {
          if (currentVariantKey) {
            timelineStale.add(currentVariantKey);
            void refreshOpenTimeline();
          }
        }
      }
      selectedItem = found;
    });
  });

  // --- Facebook 24-Hour Policy Gate ---
  // Policy binds to the reply target the COMPOSER actually resolved
  // (onTargetResolved), not to the first Messenger target on the item — a
  // unified item may carry Messenger + Zalo targets, or several Pages.
  let facebookNowTick = $state(Date.now());
  let facebookSelectedTag: string | null = $state(null);
  const facebookTargetState = $state<{ target: InboxReplyTarget | null }>({ target: null });

  function handleTargetResolved(target: InboxReplyTarget): void {
    facebookTargetState.target = target;
  }

  // Reset the chosen CSKH tag whenever the send target identity changes.
  $effect(() => {
    const t = facebookTargetState.target;
    void `${t?.conversationId}:${t?.channelAccountId}:${selectedItem?.key}`;
    facebookSelectedTag = null;
  });

  // Live countdown for the policy banner (30s cadence is plenty for hh:mm).
  $effect(() => {
    const id = window.setInterval(() => (facebookNowTick = Date.now()), 30_000);
    return () => window.clearInterval(id);
  });

  let isMessengerTarget = $derived(facebookTargetState.target?.channel === 'messenger');
  let facebookPolicyState = $derived(
    isMessengerTarget
      ? {
          lastInboundAtMs: facebookTargetState.target?.lastInboundAt
            ? Date.parse(facebookTargetState.target.lastInboundAt)
            : null,
          selectedTag: facebookSelectedTag,
          nowMs: facebookNowTick
        }
      : null
  );

  const pinnedMessagesCache = new Map<number, PinnedMessage[]>();
  const pinnedMessagesInFlight = new Map<number, Promise<PinnedMessage[]>>();
  const pinnedFetchGeneration = new Map<number, number>();
  const pinnedPendingForced = new Map<number, boolean>();
  let pinnedItemRequestEpoch = 0;

  function getItemConversationIds(item: InboxItem | null): number[] {
    if (!item) return [];
    if (item.conversationId !== null) return [item.conversationId];
    return (item.replyTargets || [])
      .map((t) => t.conversationId)
      .filter((id): id is number => typeof id === 'number' && id > 0);
  }

  function mergeAndSortPins(pins: PinnedMessage[]): PinnedMessage[] {
    const seen = new Set<number>();
    const unique: PinnedMessage[] = [];
    for (const p of pins) {
      if (!seen.has(p.id)) {
        seen.add(p.id);
        unique.push(p);
      }
    }
    return unique.sort((a, b) => {
      const ta = Date.parse(a.pinnedAt) || 0;
      const tb = Date.parse(b.pinnedAt) || 0;
      return tb - ta;
    });
  }

  $effect(() => {
    const item = selectedItem;
    if (item) {
      void loadPinnedMessagesForItem(item);
    } else {
      pinnedMessages = [];
    }
  });

  async function fetchPinnedMessagesForConv(
    conversationId: number,
    forceRefresh = false
  ): Promise<PinnedMessage[]> {
    if (pinnedMessagesCache.has(conversationId) && !forceRefresh) {
      return pinnedMessagesCache.get(conversationId)!;
    }

    if (pinnedMessagesInFlight.has(conversationId)) {
      if (forceRefresh) {
        pinnedPendingForced.set(conversationId, true);
      }
      try {
        await pinnedMessagesInFlight.get(conversationId);
      } catch {}
      if (pinnedPendingForced.get(conversationId)) {
        pinnedPendingForced.delete(conversationId);
        return fetchPinnedMessagesForConv(conversationId, true);
      }
      return pinnedMessagesCache.get(conversationId) ?? [];
    }

    const generation = (pinnedFetchGeneration.get(conversationId) ?? 0) + 1;
    pinnedFetchGeneration.set(conversationId, generation);

    const promise = (async () => {
      try {
        const list = await listConversationPinnedMessages(conversationId);
        if (pinnedFetchGeneration.get(conversationId) === generation) {
          pinnedMessagesCache.set(conversationId, list);
        }
        return list;
      } catch (err) {
        console.error('Failed to fetch pinned messages for conversation', conversationId, err);
        return pinnedMessagesCache.get(conversationId) ?? [];
      }
    })();

    pinnedMessagesInFlight.set(conversationId, promise);
    try {
      await promise;
    } finally {
      if (pinnedMessagesInFlight.get(conversationId) === promise) {
        pinnedMessagesInFlight.delete(conversationId);
      }
    }

    if (pinnedPendingForced.get(conversationId)) {
      pinnedPendingForced.delete(conversationId);
      return fetchPinnedMessagesForConv(conversationId, true);
    }

    return pinnedMessagesCache.get(conversationId) ?? [];
  }

  async function loadPinnedMessagesForItem(item: InboxItem, forceRefresh = false) {
    const itemKey = item.key;
    const epoch = ++pinnedItemRequestEpoch;
    const convIds = getItemConversationIds(item);

    if (convIds.length === 0) {
      if (selectedItem?.key === itemKey && epoch === pinnedItemRequestEpoch) {
        pinnedMessages = [];
      }
      return;
    }

    const instantPins: PinnedMessage[] = [];
    let hasAllCached = true;
    for (const cid of convIds) {
      const cached = pinnedMessagesCache.get(cid);
      if (cached && Array.isArray(cached)) {
        instantPins.push(...cached);
      } else {
        hasAllCached = false;
      }
    }
    if (selectedItem?.key === itemKey) {
      pinnedMessages = mergeAndSortPins(instantPins);
    }
    if (hasAllCached && !forceRefresh) return;

    try {
      const results = await Promise.all(
        convIds.map((cid) => fetchPinnedMessagesForConv(cid, forceRefresh))
      );
      if (selectedItem?.key === itemKey && epoch === pinnedItemRequestEpoch) {
        pinnedMessages = mergeAndSortPins(results.flat());
      }
    } catch {
      // Ignored
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
      pinnedMessagesCache.delete(convId);
      if (selectedItem) {
        await loadPinnedMessagesForItem(selectedItem, true);
      }
    } catch (err) {
      console.error('Failed to pin/unpin message', err);
    }
  }

  async function handleUnpinMessage(messageId: number) {
    const pin = pinnedMessages.find((p) => p.messageId === messageId);
    const convId = pin?.conversationId ?? selectedItem?.conversationId;
    if (!convId) return;
    try {
      await unpinConversationMessage(convId, messageId);
      pinnedMessagesCache.delete(convId);
      if (selectedItem) {
        await loadPinnedMessagesForItem(selectedItem, true);
      }
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
            refreshOpenTimeline();
          }
        }
      });
    }
  });

  $effect(() => {
    if (activeTab !== 'conversation') return;

    const stopRealtime = startMetaRealtimeListener();

    let focusDebounceTimer: ReturnType<typeof setTimeout> | null = null;
    const onWindowFocus = () => {
      if (focusDebounceTimer) clearTimeout(focusDebounceTimer);
      focusDebounceTimer = setTimeout(() => {
        void syncFacebookConversations().catch(() => {});
      }, 500);
    };
    window.addEventListener('focus', onWindowFocus);

    const interval = setInterval(() => {
      if (runtimeRefreshIntervalMs > 0) {
        void refreshRuntimeStatus(true);
      }
      const hasConnectedMessenger = runtime?.accounts?.some(
        (a) => a.channel === 'messenger' && a.status === 'connected'
      );
      if (hasConnectedMessenger) {
        void syncFacebookConversations().catch(() => {});
      }
    }, runtimeRefreshIntervalMs > 0 ? runtimeRefreshIntervalMs : 30_000);

    return () => {
      stopRealtime();
      if (focusDebounceTimer) clearTimeout(focusDebounceTimer);
      window.removeEventListener('focus', onWindowFocus);
      clearInterval(interval);
    };
  });

  let newChatOpen = $state(false);
  let newChatInitialPhone = $state('');
  let newChatInitialName = $state('');
  let handledTargetLeadKey = $state<string | null>(null);

  function findMatchingInboxItem(lead: TargetLeadPayload): InboxItem | null {
    if (!lead || inbox.length === 0) return null;

    // 0. Match by identityId (excluding group chats)
    if (lead.identityId) {
      const matchIdentity = inbox.find(
        (i) => i.kind !== 'group' && (i.identityId === lead.identityId || i.key === `identity:${lead.identityId}`)
      );
      if (matchIdentity) return matchIdentity;
    }

    // 1. Match by leadId (excluding group chats)
    if (lead.leadId) {
      const match = inbox.find((i) => i.kind !== 'group' && i.leadId === lead.leadId);
      if (match) return match;
    }

    // 2. Match by zalo_uid (excluding group chats)
    if (lead.zalo_uid) {
      const cleanUid = lead.zalo_uid.trim();
      if (cleanUid) {
        const matchUid = inbox.find((i) => {
          if (i.kind === 'group') return false;
          const keyMatch = i.key ? i.key.includes(cleanUid) : false;
          const titleMatch = i.title ? i.title.includes(cleanUid) : false;
          const participantMatch = i.participants?.some(
            (p) => String(p.identityId) === cleanUid || (p.externalId && p.externalId.trim() === cleanUid)
          );
          return keyMatch || titleMatch || participantMatch;
        });
        if (matchUid) return matchUid;
      }
    }

    // 3. Match by phone number (excluding group chats)
    const cleanPhone = lead.phone ? lead.phone.replace(/[^0-9]/g, '') : '';
    if (cleanPhone && cleanPhone.length >= 6) {
      const matchPhone = inbox.find((i) => {
        if (i.kind === 'group') return false;
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

  let isComponentMounted = false;

  // Event Bridge registration
  onMount(() => {
    isComponentMounted = true;
    const cleanupInvalidated = onConversationInvalidated(handleInvalidatedEvent);
    const cleanupDelta = onConversationMessageDelta(handleMessageDelta);
    const cleanupLag = onConversationDeltaLagged(() => {
      void initializeHighWaterMarkAndCatchUp();
    });
    const cleanupReconnect = onConversationBridgeReconnected(() => {
      void initializeHighWaterMarkAndCatchUp();
    });

    let hwmFocusTimer: ReturnType<typeof setTimeout> | null = null;
    const onFocus = () => {
      if (hwmFocusTimer) clearTimeout(hwmFocusTimer);
      hwmFocusTimer = setTimeout(() => {
        void initializeHighWaterMarkAndCatchUp();
      }, 300);
    };
    const onVisibility = () => {
      if (typeof document !== 'undefined' && document.visibilityState === 'visible') {
        if (hwmFocusTimer) clearTimeout(hwmFocusTimer);
        hwmFocusTimer = setTimeout(() => {
          void initializeHighWaterMarkAndCatchUp();
        }, 300);
      }
    };

    if (typeof window !== 'undefined') {
      window.addEventListener('focus', onFocus);
      document.addEventListener('visibilitychange', onVisibility);
    }

    void initializeHighWaterMarkAndCatchUp();

    return () => {
      isComponentMounted = false;
      cleanupInvalidated();
      cleanupDelta();
      cleanupLag();
      cleanupReconnect();
      if (hwmFocusTimer) clearTimeout(hwmFocusTimer);
      if (typeof window !== 'undefined') {
        window.removeEventListener('focus', onFocus);
        document.removeEventListener('visibilitychange', onVisibility);
      }
      if (inboxRefreshTimer) clearTimeout(inboxRefreshTimer);
      if (timelineRefreshTimer) clearTimeout(timelineRefreshTimer);
      if (invalidationFallbackTimer) clearTimeout(invalidationFallbackTimer);
      if (replayFallbackTimer) {
        clearTimeout(replayFallbackTimer);
        replayFallbackTimer = null;
      }
    };
  });

  function getEnvMode(): string {
    try {
      if (typeof import.meta !== 'undefined' && (import.meta as unknown as { env?: { MODE?: string } }).env?.MODE) {
        return (import.meta as unknown as { env: { MODE: string } }).env.MODE;
      }
    } catch {}
    try {
      if (typeof process !== 'undefined' && (process as unknown as { env?: { NODE_ENV?: string } })?.env?.NODE_ENV) {
        return (process as unknown as { env: { NODE_ENV: string } }).env.NODE_ENV as string;
      }
    } catch {}
    return 'production';
  }
  const IS_TEST = getEnvMode() === 'test';

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
        if (IS_TEST) {
          void prefetchInboxTimelines(inbox);
        } else if (typeof window !== 'undefined' && 'requestIdleCallback' in window) {
          window.requestIdleCallback(() => {
            void prefetchInboxTimelines(inbox);
          });
        } else {
          setTimeout(() => {
            void prefetchInboxTimelines(inbox);
          }, 300);
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
      // Trigger background sync for Facebook Messenger accounts
      syncFacebookConversations().catch((err) => {
        console.warn('Initial Facebook Messenger sync error:', err);
      });
    }
  }

  const inFlightVariantFetches = new Map<string, Promise<TimelinePage | null>>();

  async function prefetchInboxTimelines(items: InboxItem[]) {
    if (!items || items.length === 0) return;
    const targets = IS_TEST ? items.slice(0, 10) : items.slice(0, 3);
    for (const target of targets) {
      const channels =
        target.kind === 'group' || target.leadId === null
          ? []
          : (activeChannelsPerKey.get(target.key) ?? target.channels.map((c) => c.channel));
      const key =
        target.kind === 'group' || target.leadId === null
          ? `${target.key}:group`
          : `${target.key}:${channelSetKey(channels)}`;

      if (!variantsCache.has(key) && !inFlightVariantFetches.has(key)) {
        void fetchVariantFirstPage(target, channels, key, null);
      }
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
      if (selectedKey) {
        const updatedSelectedItem = inbox.find((i) => i.key === selectedKey);
        if (updatedSelectedItem) {
          selectedItem = updatedSelectedItem;
        } else if (inbox.length > 0) {
          selectItem(inbox[0]);
        }
      }
      if (selectedItem?.kind === 'group' && selectedItem?.conversationId !== null) {
        void syncZaloGroupChatHistory(selectedItem.conversationId).catch(() => {});
      }
    } catch (err) {
      console.error('Failed to refresh inbox', err);
    }
  }

  function selectItem(item: InboxItem) {
    if (selectedKey !== item.key) {
      messageSequenceMap.clear();
      pendingInvalidationMessageIds.clear();
      pendingGeneralInvalidation = false;
      replayGeneration += 1;
      if (replayFallbackTimer) {
        clearTimeout(replayFallbackTimer);
        replayFallbackTimer = null;
      }
      replayFallbackRetries = 0;
      isResumingDeltas = false;
    }
    selectedKey = item.key;
    selectedItem = item;

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

    loadOrFetchVariant(item, activeChannels, false);
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

    loadOrFetchVariant(selectedItem, updated, false);
  }

  function loadOrFetchVariant(item: InboxItem, channels: Channel[], preserveAnchor: boolean) {
    const key =
      item.kind === 'group' || item.leadId === null
        ? `${item.key}:group`
        : `${item.key}:${channelSetKey(channels)}`;

    const existing = variantsCache.get(key);
    const anchor = preserveAnchor ? timelineRef?.captureAnchor() ?? null : null;

    if (existing && !timelineStale.has(key)) {
      currentVariant = existing;
      if (anchor) {
        timelineRef?.restoreAnchor({ messageId: anchor.messageId, offsetTop: anchor.offsetTop });
      }
    } else {
      const initialVar: TimelineVariant = existing
        ? { ...existing, loading: existing.hasLoaded ? false : true }
        : { items: [], nextCursor: null, hasLoaded: false, loading: true };

      variantsCache.set(key, initialVar);
      currentVariant = initialVar;

      fetchVariantFirstPage(item, channels, key, anchor);
    }
  }

  const variantInvalidationEpoch = new Map<string, number>();

  async function fetchVariantFirstPage(
    item: InboxItem,
    channels: Channel[],
    key: string,
    anchor: { messageId: number; offsetTop: number } | null
  ): Promise<TimelinePage | null> {
    const existing = variantsCache.get(key) ?? {
      items: [],
      nextCursor: null,
      hasLoaded: false,
      loading: true
    };

    const loadingState: TimelineVariant = {
      ...existing,
      loading: existing.hasLoaded ? false : true
    };
    variantsCache.set(key, loadingState);
    if (currentVariantKey === key) {
      currentVariant = loadingState;
    }

    if (inFlightVariantFetches.has(key)) {
      try {
        await inFlightVariantFetches.get(key);
      } catch {
        // Handled by original caller
      }
      if (timelineStale.has(key)) {
        return fetchVariantFirstPage(item, channels, key, anchor);
      }
      const cached = variantsCache.get(key);
      return cached
        ? {
            items: cached.items,
            nextCursor: cached.nextCursor,
            snapshotSequence: lastSeenSequence
          }
        : null;
    }

    const fetchPromise = (async (): Promise<TimelinePage | null> => {
      while (true) {
        const startEpoch = variantInvalidationEpoch.get(key) ?? 0;
        try {
          let page: TimelinePage;
          if (item.kind === 'group') {
            if (item.conversationId === null) throw new Error('Group conversation ID is missing');
            void syncZaloGroupChatHistory(item.conversationId).catch(() => {});
            page = await getConversationTimeline(item.conversationId, null, 50);
          } else if (item.kind === 'unknown') {
            if (item.identityId === null) throw new Error('Unknown contact identity ID is missing');
            page = await getIdentityTimeline(item.identityId, null, 50);
          } else {
            if (item.leadId === null) throw new Error('Customer lead ID is missing');
            page = await getContactTimeline(item.leadId, channels, null, 50);
          }

          const currentEpoch = variantInvalidationEpoch.get(key) ?? 0;
          if (currentEpoch !== startEpoch) {
            // An invalidation occurred while this fetch was in flight.
            // Retry under the new epoch without committing obsolete snapshot.
            continue;
          }

          const latestExisting = variantsCache.get(key) || existing;
          const updated = mergeTimelinePage(latestExisting, page, 'refresh', {
            messageSequenceMap
          });
          variantsCache.set(key, updated);

          const isCurrentActiveKey =
            isComponentMounted &&
            currentVariantKey === key &&
            selectedItem !== null &&
            key ===
              (selectedItem.kind === 'group' || selectedItem.leadId === null
                ? `${selectedItem.key}:group`
                : `${selectedItem.key}:${channelSetKey(activeChannels)}`);

          if (isCurrentActiveKey) {
            if (page.snapshotSequence && page.snapshotSequence > 0) {
              if (page.snapshotSequence > lastSeenSequence) {
                lastSeenSequence = page.snapshotSequence;
              }
              for (const [mid, seq] of messageSequenceMap.entries()) {
                if (seq <= page.snapshotSequence) {
                  messageSequenceMap.delete(mid);
                }
              }
            }

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

          timelineStale.delete(key);
          return page;
        } catch (err) {
          console.error('Failed to fetch timeline variant', err);
          const latestExisting = variantsCache.get(key) || existing;
          const failed = { ...latestExisting, loading: false };
          variantsCache.set(key, failed);
          timelineStale.add(key);
          if (currentVariantKey === key) {
            currentVariant = failed;
          }
          return null;
        }
      }
    })().finally(() => {
      inFlightVariantFetches.delete(key);
    });

    inFlightVariantFetches.set(key, fetchPromise);
    return await fetchPromise;
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

      const latestExisting = variantsCache.get(key) || currentVariant;
      const updated = mergeTimelinePage(latestExisting, page, 'older');
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

  export async function refreshOpenTimeline(preserveScroll: boolean = true): Promise<TimelinePage | null> {
    if (!selectedItem) return null;
    if (selectedItem.kind !== 'group' && activeChannels.length === 0) return null;

    const key = currentVariantKey;
    variantInvalidationEpoch.set(
      key,
      (variantInvalidationEpoch.get(key) ?? 0) + 1
    );
    timelineStale.add(key);

    let anchor: { messageId: number; offsetTop: number } | null = null;
    if (preserveScroll && timelineRef) {
      anchor = timelineRef.captureAnchor();
    }
    return await fetchVariantFirstPage(selectedItem, activeChannels, key, anchor);
  }

  function eventAffectsItem(item: InboxItem, event: ConversationInvalidatedEvent): boolean {
    if (event.conversationId === 0) {
      if (event.channel) {
        return (
          item.channels.some((c) => c.channel === event.channel) ||
          item.replyTargets.some((target) => target.channel === event.channel)
        );
      }
      return true;
    }
    if (item.kind === 'customer') {
      if (item.leadId !== null && event.leadId !== null && item.leadId === event.leadId) {
        return true;
      }
      if (item.conversationId !== null && item.conversationId === event.conversationId) {
        return true;
      }
      if (item.replyTargets.some((target) => target.conversationId === event.conversationId)) {
        return true;
      }
      if (event.channel && event.leadId === null) {
        return (
          item.channels.some((c) => c.channel === event.channel) ||
          item.replyTargets.some((t) => t.channel === event.channel)
        );
      }
      return false;
    }
    if (item.kind === 'group') {
      return (
        (item.conversationId !== null && event.conversationId === item.conversationId) ||
        item.replyTargets.some((target) => target.conversationId === event.conversationId)
      );
    }
    if (item.conversationId !== null && item.conversationId === event.conversationId) {
      return true;
    }
    if (item.replyTargets.some((target) => target.conversationId === event.conversationId)) {
      return true;
    }
    if (event.channel) {
      return (
        item.channels.some((c) => c.channel === event.channel) ||
        item.replyTargets.some((t) => t.channel === event.channel)
      );
    }
    return false;
  }

  function getWorkspaceEnvMode(): string {
    try {
      // @ts-ignore
      if (typeof import.meta !== 'undefined' && (import.meta as unknown as { env?: { MODE?: string } }).env?.MODE) {
        // @ts-ignore
        return (import.meta as unknown as { env: { MODE: string } }).env.MODE;
      }
    } catch {}
    try {
      if (typeof process !== 'undefined' && (process as unknown as { env?: { NODE_ENV?: string } })?.env?.NODE_ENV) {
        return (process as unknown as { env: { NODE_ENV: string } }).env.NODE_ENV as string;
      }
    } catch {}
    return 'production';
  }
  const IS_TEST_WORKSPACE = getWorkspaceEnvMode() === 'test';

  let lastSeenSequence = 0;
  let isResumingDeltas = false;
  let liveDeltaBuffer: ConversationMessageDeltaEvent[] = [];
  let pendingReplayTargetSeq: number | null = null;
  let replayGeneration = 0;
  let replayFallbackRetries = 0;
  const messageSequenceMap = new Map<number, number>();

  const pendingInvalidationMessageIds = new Set<number>();
  let pendingGeneralInvalidation = false;
  let invalidationFallbackTimer: ReturnType<typeof setTimeout> | null = null;

  function scheduleInvalidationFallback() {
    if (invalidationFallbackTimer) return;
    invalidationFallbackTimer = setTimeout(() => {
      invalidationFallbackTimer = null;
      let shouldRefresh = pendingGeneralInvalidation;
      if (!shouldRefresh) {
        for (const mid of pendingInvalidationMessageIds) {
          if (!currentVariant.items.some((m) => m.id === mid)) {
            shouldRefresh = true;
            break;
          }
        }
      }
      pendingInvalidationMessageIds.clear();
      pendingGeneralInvalidation = false;
      if (shouldRefresh) {
        void refreshOpenTimeline();
      }
    }, IS_TEST_WORKSPACE ? 20 : 60);
  }

  export interface DeltaPaintTelemetrySample {
    sequence: number;
    deltaToPaintMs: number;
    dbToPaintMs: number;
    timestamp: number;
  }
  const deltaPaintSamples: DeltaPaintTelemetrySample[] = [];

  export function getDeltaPaintTelemetrySummary() {
    if (deltaPaintSamples.length === 0) {
      return {
        count: 0,
        deltaToPaint: { p50: 0, p95: 0, p99: 0, max: 0, avg: 0 },
        dbToPaint: { p50: 0, p95: 0, p99: 0, max: 0, avg: 0 },
        p50: 0,
        p95: 0,
        p99: 0,
        max: 0,
        avg: 0
      };
    }
    const deltaLatencies = deltaPaintSamples.map((s) => s.deltaToPaintMs).sort((a, b) => a - b);
    const dbLatencies = deltaPaintSamples.map((s) => s.dbToPaintMs).sort((a, b) => a - b);
    const count = deltaLatencies.length;
    const calc = (arr: number[]) => {
      const p50 = arr[Math.floor(count * 0.50)];
      const p95 = arr[Math.min(count - 1, Math.floor(count * 0.95))];
      const p99 = arr[Math.min(count - 1, Math.floor(count * 0.99))];
      const max = arr[count - 1];
      const avg = Math.round((arr.reduce((acc, v) => acc + v, 0) / count) * 100) / 100;
      return { p50, p95, p99, max, avg };
    };
    const deltaStats = calc(deltaLatencies);
    const dbStats = calc(dbLatencies);
    return {
      count,
      deltaToPaint: deltaStats,
      dbToPaint: dbStats,
      ...deltaStats
    };
  }

  if (typeof window !== 'undefined') {
    (window as unknown as { __GET_DELTA_PAINT_TELEMETRY__?: typeof getDeltaPaintTelemetrySummary }).__GET_DELTA_PAINT_TELEMETRY__ = getDeltaPaintTelemetrySummary;
  }

  async function initializeHighWaterMarkAndCatchUp() {
    try {
      const hwm = await getConversationHighWaterMark();
      if (!hwm || typeof hwm.currentSequence !== 'number') return;
      if (lastSeenSequence === 0) {
        lastSeenSequence = hwm.currentSequence;
      } else if (typeof hwm.minSequence === 'number' && lastSeenSequence < hwm.minSequence) {
        // Gap is beyond retention floor! Authoritative full snapshot is required.
        lastSeenSequence = hwm.currentSequence;
        if (selectedItem) {
          void refreshOpenTimeline();
        }
      } else if (lastSeenSequence < hwm.currentSequence) {
        await resumeMissedDeltas(lastSeenSequence, hwm.currentSequence);
      }
    } catch (err) {
      console.error('[OmniChat Delta] Failed to get high water mark:', err);
    }
  }

  const REPLAY_BATCH_SIZE = 200;
  const MAX_REPLAY_FALLBACK_RETRIES = 3;
  let replayFallbackTimer: ReturnType<typeof setTimeout> | null = null;

  export async function resumeMissedDeltas(fromSeq: number, targetSeq?: number): Promise<void> {
    if (!isComponentMounted) return;
    if (isResumingDeltas) {
      if (targetSeq !== undefined) {
        pendingReplayTargetSeq = Math.max(pendingReplayTargetSeq ?? 0, targetSeq);
      }
      return;
    }

    isResumingDeltas = true;
    const thisGeneration = ++replayGeneration;
    const initialTargetKey = selectedItem
      ? selectedItem.kind === 'group' || selectedItem.leadId === null
        ? `${selectedItem.key}:group`
        : `${selectedItem.key}:${channelSetKey(activeChannels)}`
      : null;

    let currentCursor = fromSeq;
    let fallbackNeeded = false;

    try {
      while (isComponentMounted && thisGeneration === replayGeneration) {
        const deltas = await resumeConversationDeltas(
          currentCursor,
          REPLAY_BATCH_SIZE
        );

        if (!isComponentMounted || thisGeneration !== replayGeneration) {
          return;
        }

        if (deltas.length === 0) {
          break;
        }

        deltas.sort((a, b) => a.sequence - b.sequence);

        for (const delta of deltas) {
          if (!isComponentMounted || thisGeneration !== replayGeneration) return;
          if (delta.sequence > lastSeenSequence) {
            applySingleDelta(delta);
          }
          currentCursor = Math.max(currentCursor, delta.sequence);
        }

        if (deltas.length < REPLAY_BATCH_SIZE) {
          break;
        }
        if (targetSeq !== undefined && currentCursor >= targetSeq) {
          break;
        }
      }
      replayFallbackRetries = 0;
    } catch (err) {
      console.error(
        `[OmniChat Delta] Failed to resume missed deltas from sequence ${fromSeq}:`,
        err
      );
      fallbackNeeded = true;
    } finally {
      if (thisGeneration === replayGeneration) {
        if (fallbackNeeded && isComponentMounted) {
          const currentKey = selectedItem
            ? selectedItem.kind === 'group' || selectedItem.leadId === null
              ? `${selectedItem.key}:group`
              : `${selectedItem.key}:${channelSetKey(activeChannels)}`
            : null;

          if (currentKey === initialTargetKey && initialTargetKey !== null) {
            try {
              const page = await refreshOpenTimeline();
              if (page && isComponentMounted && thisGeneration === replayGeneration) {
                const seq = page.snapshotSequence ?? 0;
                const effectiveTarget = pendingReplayTargetSeq !== null
                  ? Math.max(targetSeq ?? 0, pendingReplayTargetSeq)
                  : targetSeq;

                if (effectiveTarget === undefined || seq >= effectiveTarget) {
                  replayFallbackRetries = 0;
                  fallbackNeeded = false;
                  // Invalidate all other cached conversation variants and bump epochs so in-flight fetches cannot overwrite with stale snapshots
                  for (const cachedKey of variantsCache.keys()) {
                    if (cachedKey !== currentKey) {
                      variantInvalidationEpoch.set(
                        cachedKey,
                        (variantInvalidationEpoch.get(cachedKey) ?? 0) + 1
                      );
                      timelineStale.add(cachedKey);
                    }
                  }
                  // Refresh inbox list so sidebar previews/unread counts reflect latest DB state
                  void refreshInbox();
                } else {
                  scheduleReplayFallbackRetry(seq, effectiveTarget);
                }
              } else if (!page && isComponentMounted && thisGeneration === replayGeneration) {
                scheduleReplayFallbackRetry(fromSeq, targetSeq);
              }
            } catch (snapshotErr) {
              console.error('[OmniChat Delta] Authoritative snapshot reload failed:', snapshotErr);
              if (isComponentMounted && thisGeneration === replayGeneration) {
                scheduleReplayFallbackRetry(fromSeq, targetSeq);
              }
            }
          } else if (initialTargetKey) {
            variantInvalidationEpoch.set(
              initialTargetKey,
              (variantInvalidationEpoch.get(initialTargetKey) ?? 0) + 1
            );
            timelineStale.add(initialTargetKey);
            fallbackNeeded = false;
          }
        }

        if (isComponentMounted) {
          isResumingDeltas = false;
          // First drain any contiguous deltas enabled by the completed replay
          drainLiveDeltaBuffer(false);

          if (!fallbackNeeded) {
            const nextTarget = pendingReplayTargetSeq;
            pendingReplayTargetSeq = null;

            if (nextTarget !== null && nextTarget > lastSeenSequence) {
              void resumeMissedDeltas(lastSeenSequence, nextTarget);
            } else {
              // Drain remaining buffer and allow gap resumption if needed
              drainLiveDeltaBuffer(true);
            }
          } else {
            // When fallback is unresolved, preserve pendingReplayTargetSeq for scheduled retry
            drainLiveDeltaBuffer(false);
          }
        }
      }
    }
  }

  function drainLiveDeltaBuffer(canTriggerGapResumption: boolean = true) {
    if (!isComponentMounted || liveDeltaBuffer.length === 0) return;
    liveDeltaBuffer.sort((a, b) => a.sequence - b.sequence);

    while (liveDeltaBuffer.length > 0) {
      const buffered = liveDeltaBuffer[0];
      if (buffered.sequence <= lastSeenSequence) {
        liveDeltaBuffer.shift();
        continue;
      }
      if (buffered.sequence === lastSeenSequence + 1) {
        liveDeltaBuffer.shift();
        applySingleDelta(buffered);
        continue;
      }
      break;
    }

    if (
      canTriggerGapResumption &&
      liveDeltaBuffer.length > 0 &&
      !isResumingDeltas &&
      !replayFallbackTimer
    ) {
      const highestBuffered = liveDeltaBuffer[liveDeltaBuffer.length - 1].sequence;
      void resumeMissedDeltas(lastSeenSequence, highestBuffered);
    }
  }

  function scheduleReplayFallbackRetry(fromSeq: number, targetSeq?: number) {
    if (!isComponentMounted) return;
    if (replayFallbackRetries < MAX_REPLAY_FALLBACK_RETRIES) {
      const effectiveTarget = pendingReplayTargetSeq !== null
        ? Math.max(targetSeq ?? 0, pendingReplayTargetSeq)
        : targetSeq;

      const delayMs = IS_TEST_WORKSPACE ? 20 : Math.min(1000 * Math.pow(2, replayFallbackRetries), 4000);
      if (replayFallbackTimer) clearTimeout(replayFallbackTimer);
      replayFallbackTimer = setTimeout(() => {
        replayFallbackTimer = null;
        if (!isComponentMounted) return;
        if (isResumingDeltas) {
          if (effectiveTarget !== undefined) {
            pendingReplayTargetSeq = Math.max(pendingReplayTargetSeq ?? 0, effectiveTarget);
          }
          scheduleReplayFallbackRetry(fromSeq, effectiveTarget);
          return;
        }
        replayFallbackRetries++;
        void resumeMissedDeltas(lastSeenSequence > fromSeq ? lastSeenSequence : fromSeq, effectiveTarget);
      }, delayMs);
    } else {
      console.error(
        `[OmniChat Delta] Replay fallback exceeded maximum retry attempts (${MAX_REPLAY_FALLBACK_RETRIES}). Buffer preserved.`
      );
    }
  }

  function applySingleDelta(delta: ConversationMessageDeltaEvent) {
    if (delta.sequence > lastSeenSequence) {
      lastSeenSequence = delta.sequence;
    }

    // 1. Instant Open Timeline Store Upsert
    if (selectedItem) {
      const affectsOpenItem =
        selectedItem.conversationId === delta.conversationId ||
        selectedItem.replyTargets.some((t) => t.conversationId === delta.conversationId) ||
        (selectedItem.kind === 'customer' &&
          selectedItem.leadId !== null &&
          delta.leadId !== null &&
          selectedItem.leadId === delta.leadId);

      if (affectsOpenItem) {
        messageSequenceMap.set(delta.message.id, delta.sequence);

        const cleanedItems = currentVariant.items.filter(
          (item) =>
            !(
              item.id < 0 &&
              delta.message.sendBatch?.sendBatchId &&
              item.sendBatch?.sendBatchId === delta.message.sendBatch.sendBatchId
            )
        );
        const nextItems = mergeMessages(cleanedItems, [delta.message]);
        currentVariant = {
          ...currentVariant,
          items: nextItems,
          hasLoaded: true,
          loading: false
        };
        if (currentVariantKey) {
          variantsCache.set(currentVariantKey, currentVariant);
        }
      }
    }

    // 2. Fast In-Memory Sidebar Preview & Timestamp update
    const affectedItem = inbox.find(
      (item) =>
        item.conversationId === delta.conversationId ||
        item.replyTargets.some((t) => t.conversationId === delta.conversationId) ||
        (item.kind === 'customer' &&
          item.leadId !== null &&
          delta.leadId !== null &&
          item.leadId === delta.leadId)
    );
    if (affectedItem) {
      affectedItem.lastMessagePreview =
        delta.message.textContent ||
        (delta.message.contentType === 'image' ? '[Hình ảnh]' : '[Tin nhắn]');
      affectedItem.lastMessageAt = delta.message.occurredAt;
      inbox = [...inbox].sort(
        (a, b) =>
          new Date(b.lastMessageAt || 0).getTime() - new Date(a.lastMessageAt || 0).getTime()
      );
    }
  }

  function recordPaintTelemetry(receivePerf: number, delta: ConversationMessageDeltaEvent) {
    if (typeof window !== 'undefined' && typeof window.requestAnimationFrame === 'function') {
      window.requestAnimationFrame(() => {
        window.requestAnimationFrame(() => {
          const paintPerf = typeof performance !== 'undefined' ? performance.now() : Date.now();
          const deltaToPaintMs = Math.round((paintPerf - receivePerf) * 100) / 100;
          const dbToPaintMs = delta.emittedAtMs > 0 ? Date.now() - delta.emittedAtMs : deltaToPaintMs;
          deltaPaintSamples.push({
            sequence: delta.sequence,
            deltaToPaintMs,
            dbToPaintMs,
            timestamp: Date.now()
          });
          if (deltaPaintSamples.length > 500) {
            deltaPaintSamples.shift();
          }
          if (typeof console !== 'undefined' && console.debug) {
            console.debug(
              `[OmniChat Telemetry] seq=${delta.sequence} deltaToPaint=${deltaToPaintMs}ms dbToPaint=${dbToPaintMs}ms`
            );
          }
        });
      });
    }
  }

  function handleMessageDelta(delta: ConversationMessageDeltaEvent) {
    const receiveMs = typeof performance !== 'undefined' ? performance.now() : Date.now();

    if (isResumingDeltas) {
      liveDeltaBuffer.push(delta);
      if (delta.sequence > lastSeenSequence + 1) {
        pendingReplayTargetSeq = Math.max(pendingReplayTargetSeq ?? 0, delta.sequence);
      }
      return;
    }

    // Gap detection: if sequence is ahead by > 1, replay missing deltas from SQLite
    if (lastSeenSequence > 0 && delta.sequence > lastSeenSequence + 1) {
      console.warn(
        `[OmniChat Delta] Gap detected: expected ${lastSeenSequence + 1}, received ${delta.sequence}. Buffering and resuming...`
      );
      liveDeltaBuffer.push(delta);
      void resumeMissedDeltas(lastSeenSequence, delta.sequence);
      return;
    }

    if (lastSeenSequence > 0 && delta.sequence <= lastSeenSequence) {
      return;
    }

    applySingleDelta(delta);
    drainLiveDeltaBuffer();
    recordPaintTelemetry(receiveMs, delta);
  }

  function handleInvalidatedEvent(event: ConversationInvalidatedEvent) {
    if (event.reason === 'account_status') {
      void refreshRuntimeStatus(true);
    }
    const isActive = activeTab === 'conversation';
    const affectedItems = inbox.filter((item) => eventAffectsItem(item, event));

    for (const item of affectedItems) {
      for (const key of variantsCache.keys()) {
        if (key.startsWith(`${item.key}:`)) {
          variantInvalidationEpoch.set(key, (variantInvalidationEpoch.get(key) ?? 0) + 1);
          timelineStale.add(key);
        }
      }
    }

    if (event.conversationId > 0) {
      pinnedMessagesCache.delete(event.conversationId);
      if (selectedItem) {
        const itemConvIds = getItemConversationIds(selectedItem);
        if (itemConvIds.includes(event.conversationId)) {
          void loadPinnedMessagesForItem(selectedItem, true);
        }
      }
    }

    if (isActive) {
      // Sidebar Inbox: 100ms debounce (50ms in test) to keep sidebar smooth without high-frequency refetches
      const inboxDelay = IS_TEST_WORKSPACE ? 50 : 100;
      if (inboxRefreshTimer) clearTimeout(inboxRefreshTimer);
      inboxRefreshTimer = setTimeout(() => {
        void refreshInbox();
      }, inboxDelay);

      const affectsOpenItem = selectedItem !== null && eventAffectsItem(selectedItem, event);

      if (affectsOpenItem) {
        if (event.reason !== 'message') {
          // Only trigger full 50-item timeline IPC reload for non-message invalidations
          // (such as participant joins, pinning, unpinning, reactions, title changes, profile syncs).
          // Message deltas are directly upserted into timeline via handleMessageDelta.
          const timelineDelay = IS_TEST_WORKSPACE ? 10 : 0;
          if (timelineRefreshTimer) clearTimeout(timelineRefreshTimer);
          timelineRefreshTimer = setTimeout(() => {
            void refreshOpenTimeline();
          }, timelineDelay);
        } else {
          // Bounded fallback with burst-safe coalescing:
          if (!event.messageId) {
            pendingGeneralInvalidation = true;
            scheduleInvalidationFallback();
          } else {
            const hasMessage = currentVariant.items.some((m) => m.id === event.messageId);
            if (!hasMessage) {
              pendingInvalidationMessageIds.add(event.messageId);
              scheduleInvalidationFallback();
            }
          }
        }
      }
    } else {
      inboxStale = true;
      if (selectedItem) {
        const affectsOpenItem = eventAffectsItem(selectedItem, event);

        if (affectsOpenItem && currentVariantKey) {
          variantInvalidationEpoch.set(
            currentVariantKey,
            (variantInvalidationEpoch.get(currentVariantKey) ?? 0) + 1
          );
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
    if (!selectedItem) return;

    if (result.messages.length === 0) {
      if (result.clientBatchId) {
        const cleanedItems = currentVariant.items.filter(
          (item) => !(item.id < 0 && item.sendBatch?.sendBatchId === result.clientBatchId)
        );
        currentVariant = { ...currentVariant, items: cleanedItems };
        if (currentVariantKey) {
          variantsCache.set(currentVariantKey, currentVariant);
        }
      }
      return;
    }

    const realBatchIds = new Set(
      result.messages
        .filter((m) => m.id > 0)
        .map((m) => m.sendBatch?.sendBatchId || result.clientBatchId)
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
    if (currentVariantKey) {
      variantsCache.set(currentVariantKey, currentVariant);
    }

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
        i.replyTargets?.some((t) => t.conversationId === msg.conversationId)
    );
    if (createdItem) {
      selectItem(createdItem);
    }
  }

  function handleConversationDeleted() {
    selectedKey = null;
    refreshInbox();
  }

  function getParticipantsPreview(item: InboxItem): string {
    if (!item.participants || item.participants.length === 0) return '';
    const names = item.participants
      .map((p) => {
        const name = p.displayName?.trim();
        if (name && !name.startsWith('+12036') && !name.startsWith('12036')) {
          return name;
        }
        return null;
      })
      .filter((n): n is string => Boolean(n));
    if (names.length === 0) return '';
    return names.slice(0, 5).join(', ');
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
                  title="View synchronized members and add Lead/Category"
                >
                  <Users class="w-3.5 h-3.5 shrink-0 group-hover:scale-110 transition-transform" />
                  <span class="font-medium underline decoration-dashed underline-offset-2">{selectedItem.participantCount} members</span>
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
              title="Search in conversation (Ctrl+F)"
              onclick={() => (inThreadSearchOpen = !inThreadSearchOpen)}
              class="h-8 w-8 text-muted-foreground hover:text-foreground"
            >
              <Search class="w-4 h-4" />
            </Button>

            <Button
              variant="outline"
              size="icon"
              title="Delete this conversation"
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
            <span class="truncate">Account status degraded: {runtimeError}</span>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onclick={() => refreshRuntimeStatus()}
            class="h-7 text-xs flex items-center gap-1 shrink-0"
          >
            <RefreshCw class="w-3 h-3" />
            <span>Retry</span>
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
          ? 'No channels currently visible'
          : 'No messages in this conversation'}
        onLoadOlder={handleLoadOlder}
        isGroup={selectedItem.kind === 'group'}
        conversationTitle={selectedItem.title}
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


      <!-- Facebook 24-Hour Policy Gate (Messenger target only) -->
      {#if isMessengerTarget}
        <div class="px-4 pt-2">
          <Facebook24hPolicyBanner
            lastInboundAt={facebookTargetState.target?.lastInboundAt ?? null}
            nowMs={facebookNowTick}
            bind:selectedTag={facebookSelectedTag}
          />
        </div>
      {/if}

      <!-- Message Composer -->
      <MessageComposer
        item={selectedItem}
        runtime={runtime}
        disabled={false}
        replyTarget={replySelection}
        onCancelReply={() => (replySelection = null)}
        onBatchQueued={handleBatchQueued}
        {onOpenSettings}
        {onOpenQrLogin}
        onRuntimeRefresh={() => refreshRuntimeStatus()}
        onTargetResolved={handleTargetResolved}
        facebookPolicy={facebookPolicyState}
      />
    {:else}
      <div class="flex-1 flex flex-col items-center justify-center p-6 text-muted-foreground text-center">
        <MessageSquare class="w-12 h-12 mb-3 opacity-30" />
        <h3 class="font-medium text-base text-foreground mb-1">No conversation selected</h3>
        <p class="text-xs text-muted-foreground">
          {inbox.length === 0 ? 'No conversations' : 'Select a conversation from the list on the left'}
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
    channel={selectedItem?.replyTargets?.[0]?.channel ?? selectedItem?.channels?.[0]?.channel ?? 'zalo'}
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
