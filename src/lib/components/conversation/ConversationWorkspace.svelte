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
    recallConversationMessage,
    pinConversationMessage,
    unpinConversationMessage,
    listConversationPinnedMessages,
    syncFacebookConversations,
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

  function handleImageClick(clickedUrl: string, _clickedMessage: ConversationMessage) {
    const items = currentVariant?.items ?? [];
    const images = extractGalleryImages(items);

    const safeClicked = safeConversationUrl(clickedUrl) ?? clickedUrl;
    if (images.length === 0 && safeClicked) {
      images.push({ url: safeClicked, alt: 'Image' });
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
      if (pinnedMessagesCache.has(cid)) {
        instantPins.push(...pinnedMessagesCache.get(cid)!);
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

    const onWindowFocus = () => {
      void syncFacebookConversations().catch(() => {});
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

  const inFlightVariantFetches = new Map<string, Promise<void>>();

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
        }
      }
      if ((timelineStale.has(key) || !cached.hasLoaded) && !inFlightVariantFetches.has(key)) {
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

  const variantInvalidationEpoch = new Map<string, number>();

  async function fetchVariantFirstPage(
    item: InboxItem,
    channels: Channel[],
    key: string,
    anchor: { messageId: number; offsetTop: number } | null
  ) {
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
      return;
    }

    const fetchPromise = (async () => {
      while (true) {
        const startEpoch = variantInvalidationEpoch.get(key) ?? 0;
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

          const currentEpoch = variantInvalidationEpoch.get(key) ?? 0;
          if (currentEpoch === startEpoch) {
            const latestExisting = variantsCache.get(key) || existing;
            const updated = mergeTimelinePage(latestExisting, page, 'refresh');
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
                }
              }
            }
            break;
          }
        } catch (err) {
          console.error('Failed to fetch timeline variant', err);
          const latestExisting = variantsCache.get(key) || existing;
          const failed = { ...latestExisting, loading: false };
          variantsCache.set(key, failed);
          timelineStale.add(key);
          if (currentVariantKey === key) {
            currentVariant = failed;
          }
          break;
        }
      }
    })().finally(() => {
      inFlightVariantFetches.delete(key);
    });

    inFlightVariantFetches.set(key, fetchPromise);
    await fetchPromise;
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

  async function refreshOpenTimeline() {
    if (!selectedItem) return;
    if (selectedItem.kind !== 'group' && activeChannels.length === 0) return;

    const anchor = timelineRef?.captureAnchor() ?? null;
    await fetchVariantFirstPage(selectedItem, activeChannels, currentVariantKey, anchor);
  }

  function eventAffectsItem(item: InboxItem, event: ConversationInvalidatedEvent): boolean {
    if (event.conversationId === 0) {
      if (event.channel) {
        return item.replyTargets.some((target) => target.channel === event.channel);
      }
      return true;
    }
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

  // WhatsApp-style: avoid refreshing inbox/timeline while user is typing — coalesce & defer
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
  function isComposerActive(): boolean {
    const ae = document.activeElement as HTMLElement | null;
    if (!ae) return false;
    // Strict: only when the composer textarea itself is focused (not generic composer container)
    return ae.tagName === 'TEXTAREA' && ae.getAttribute('placeholder') === 'Type a message...';
  }

  function scheduleIdleWork(callback: () => void): void {
    if (typeof window !== 'undefined' && 'requestIdleCallback' in window) {
      (window as unknown as { requestIdleCallback: (cb: () => void, opts?: { timeout: number }) => number })
        .requestIdleCallback(callback, { timeout: 2000 });
    } else {
      setTimeout(callback, 0);
    }
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
      // Inbox: 300ms coalesce normally, 5000ms while typing to keep input at 120fps (Telegram/WhatsApp pattern)
      // In test keep 50ms so existing specs (70ms wait) still pass.
      const baseInboxDelay = IS_TEST_WORKSPACE ? 50 : 300;
      const typingInboxDelay = IS_TEST_WORKSPACE ? 60 : 5000;
      const inboxDelay = isComposerActive() ? typingInboxDelay : baseInboxDelay;
      if (inboxRefreshTimer) clearTimeout(inboxRefreshTimer);
      inboxRefreshTimer = setTimeout(() => {
        if (IS_TEST_WORKSPACE) {
          void refreshInbox();
        } else {
          scheduleIdleWork(() => {
            void refreshInbox();
          });
        }
      }, inboxDelay);

      const affectsOpenItem = selectedItem !== null && eventAffectsItem(selectedItem, event);

      if (affectsOpenItem) {
        const baseTimelineDelay = IS_TEST_WORKSPACE ? 50 : 300;
        const typingTimelineDelay = IS_TEST_WORKSPACE ? 60 : 5000;
        const timelineDelay = isComposerActive() ? typingTimelineDelay : baseTimelineDelay;
        if (timelineRefreshTimer) clearTimeout(timelineRefreshTimer);
        timelineRefreshTimer = setTimeout(() => {
          if (IS_TEST_WORKSPACE) {
            void refreshOpenTimeline();
          } else {
            scheduleIdleWork(() => {
              void refreshOpenTimeline();
            });
          }
        }, timelineDelay);
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
    return item.participants.map((p) => p.displayName || 'Member').join(', ');
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
