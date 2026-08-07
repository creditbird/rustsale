<script lang="ts">
  import { onMount, tick } from 'svelte';
  import type {
    ConversationCapabilities,
    ConversationMessage,
    ConversationReactionCode
  } from '$lib/services/conversationService';
  import MessageBubble from './MessageBubble.svelte';
  import MessageBatchGroup from './MessageBatchGroup.svelte';
  import { expandWindowForBatchBoundaries, groupTimelineItems } from './timelineState';

  const MAX_DOM_ITEMS = 160;

  let {
    items,
    loading = false,
    loadingOlder = false,
    hasMore = false,
    emptyMessage,
    onLoadOlder,
    isGroup = false,
    capabilitiesForMessage = () => undefined,
    pendingReactionsForMessage = () => [],
    pinnedMessageIds = new Set<number>(),
    onReply = () => {},
    onRecall = () => {},
    onForward = () => {},
    onPin = () => {},
    onToggleReaction = () => {},
    onImageClick = () => {}
  }: {
    items: ConversationMessage[];
    loading?: boolean;
    loadingOlder?: boolean;
    hasMore?: boolean;
    emptyMessage: string;
    onLoadOlder: () => void;
    isGroup?: boolean;
    capabilitiesForMessage?: (message: ConversationMessage) => ConversationCapabilities | undefined;
    pendingReactionsForMessage?: (messageId: number) => ConversationReactionCode[];
    pinnedMessageIds?: Set<number>;
    onReply?: (message: ConversationMessage) => void;

    onRecall?: (message: ConversationMessage) => void;
    onForward?: (message: ConversationMessage) => void;
    onPin?: (message: ConversationMessage) => void;
    onToggleReaction?: (

      message: ConversationMessage,
      code: ConversationReactionCode,
      currentActive?: boolean
    ) => void;
    onImageClick?: (url: string, message: ConversationMessage) => void;
  } = $props();

  let containerEl: HTMLElement | null = $state(null);
  let topSentinelEl: HTMLElement | null = $state(null);
  let windowStart = $state(0);

  // Keep windowStart bounded whenever items change
  $effect(() => {
    if (items.length <= MAX_DOM_ITEMS) {
      windowStart = 0;
    } else if (windowStart + MAX_DOM_ITEMS > items.length) {
      windowStart = Math.max(0, items.length - MAX_DOM_ITEMS);
    }
  });

  let virtualWindow = $derived.by(() => {
    const start = items.length <= MAX_DOM_ITEMS ? 0 : windowStart;
    const end = items.length <= MAX_DOM_ITEMS
      ? items.length
      : Math.min(items.length, windowStart + MAX_DOM_ITEMS);
    return expandWindowForBatchBoundaries(items, start, end);
  });
  let mountedItems = $derived(items.slice(virtualWindow.start, virtualWindow.end));
  let renderItems = $derived(groupTimelineItems(mountedItems));

  // Auto-scroll tracking for new live messages and conversation switching
  let previousItemsRef: ConversationMessage[] | null = null;

  $effect(() => {
    const currentItems = items;
    if (previousItemsRef !== currentItems) {
      const isInitialOrSelection =
        previousItemsRef === null ||
        currentItems.length === 0 ||
        previousItemsRef.length === 0 ||
        currentItems[0]?.conversationId !== previousItemsRef[0]?.conversationId;

      const wasNear = isNearBottom();
      previousItemsRef = currentItems;

      if (isInitialOrSelection || wasNear) {
        tick().then(() => {
          scrollToBottom();
        });
      }
    }
  });

  // IntersectionObserver for top sentinel
  onMount(() => {
    if (typeof IntersectionObserver === 'undefined') return;

    const observer = new IntersectionObserver(
      (entries) => {
        const entry = entries[0];
        if (entry && entry.isIntersecting) {
          if (windowStart > 0) {
            windowStart = Math.max(0, windowStart - 40);
          } else if (hasMore && !loadingOlder) {
            onLoadOlder();
          }
        }
      },
      {
        root: containerEl,
        threshold: 0.1
      }
    );

    if (topSentinelEl) {
      observer.observe(topSentinelEl);
    }

    return () => {
      observer.disconnect();
    };
  });

  // Exported Component Methods for bind:this
  export function captureAnchor(): { messageId: number; offsetTop: number } | null {
    if (!containerEl) return null;
    const bubbles = containerEl.querySelectorAll<HTMLElement>('[data-message-id]');
    for (const el of Array.from(bubbles)) {
      const relTop = el.offsetTop - containerEl.scrollTop;
      if (relTop >= 0) {
        const id = Number(el.getAttribute('data-message-id'));
        if (!isNaN(id)) {
          return { messageId: id, offsetTop: relTop };
        }
      }
    }
    const first = bubbles[0];
    if (first) {
      const id = Number(first.getAttribute('data-message-id'));
      if (!isNaN(id)) {
        return { messageId: id, offsetTop: first.offsetTop - containerEl.scrollTop };
      }
    }
    return null;
  }

  export async function restoreAnchor(
    anchor: { messageId: number; offsetTop: number } | null
  ): Promise<void> {
    if (!anchor || !containerEl) return;
    await tick();
    const el = containerEl.querySelector<HTMLElement>(`[data-message-id="${anchor.messageId}"]`);
    if (el) {
      containerEl.scrollTop = el.offsetTop - anchor.offsetTop;
    }
  }

  export function isNearBottom(): boolean {
    if (!containerEl) return true;
    const threshold = 180;
    const distance =
      containerEl.scrollHeight - containerEl.scrollTop - containerEl.clientHeight;
    return distance <= threshold;
  }

  export async function scrollToBottom(): Promise<void> {
    await tick();
    if (containerEl) {
      containerEl.scrollTop = containerEl.scrollHeight;
    }
  }

  function handleScroll() {
    if (!containerEl) return;
    const distanceToBottom =
      containerEl.scrollHeight - containerEl.scrollTop - containerEl.clientHeight;
    const distanceToTop = containerEl.scrollTop;

    // Expand window when scrolling up/down if timeline > MAX_DOM_ITEMS
    if (items.length > MAX_DOM_ITEMS) {
      if (distanceToTop < 200 && windowStart > 0) {
        windowStart = Math.max(0, windowStart - 20);
      } else if (distanceToBottom < 200 && windowStart + MAX_DOM_ITEMS < items.length) {
        windowStart = Math.min(items.length - MAX_DOM_ITEMS, windowStart + 20);
      }
    }

    // Trigger load older if scrolled very near top
    if (distanceToTop < 50 && hasMore && !loadingOlder) {
      onLoadOlder();
    }
  }
</script>

<div
  bind:this={containerEl}
  onscroll={handleScroll}
  class="flex-1 overflow-y-auto p-4 pb-6 space-y-2 relative scroll-smooth"
  data-timeline-container
>
  <!-- Loading older indicator / Top Sentinel -->
  <div bind:this={topSentinelEl} class="w-full min-h-[1px] py-1 flex justify-center">
    {#if loadingOlder}
      <div class="flex items-center gap-2 text-xs text-muted-foreground py-2" data-slot="loading-older">
        <svg
          class="animate-spin h-4 w-4 text-primary"
          xmlns="http://www.w3.org/2000/svg"
          fill="none"
          viewBox="0 0 24 24"
        >
          <circle
            class="opacity-25"
            cx="12"
            cy="12"
            r="10"
            stroke="currentColor"
            stroke-width="4"
          ></circle>
          <path
            class="opacity-75"
            fill="currentColor"
            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
          ></path>
        </svg>
        <span>Đang tải tin nhắn cũ...</span>
      </div>
    {/if}
  </div>

  {#if loading && items.length === 0}
    <!-- Initial Loading Skeleton -->
    <div class="space-y-4 py-4 animate-pulse">
      <div class="flex justify-start">
        <div class="bg-muted/50 rounded-2xl p-4 w-2/3 h-16"></div>
      </div>
      <div class="flex justify-end">
        <div class="bg-primary/20 rounded-2xl p-4 w-1/2 h-12"></div>
      </div>
      <div class="flex justify-start">
        <div class="bg-muted/50 rounded-2xl p-4 w-3/4 h-20"></div>
      </div>
    </div>
  {:else if items.length === 0}
    <!-- Empty Message State -->
    <div class="flex flex-col items-center justify-center h-64 text-muted-foreground text-sm space-y-2">
      <svg
        xmlns="http://www.w3.org/2000/svg"
        width="32"
        height="32"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
        class="opacity-40"
      >
        <path d="M7.9 20A9 9 0 1 0 4 16.1L2 22Z" />
      </svg>
      <span>{emptyMessage}</span>
    </div>
  {:else}
    <!-- Mounted Bounded Bubbles -->
    {#each renderItems as renderItem (renderItem.kind === 'batch' ? `${renderItem.batchId}:${renderItem.messages[0]?.id}` : `message:${renderItem.message.id}`)}
      {#if renderItem.kind === 'batch'}
        <MessageBatchGroup
          messages={renderItem.messages}
          {isGroup}
          {capabilitiesForMessage}
          {pendingReactionsForMessage}
          {onReply}
          {onRecall}
          {onForward}
          {onPin}
          {onToggleReaction}
          {onImageClick}
        />
      {:else}
        <MessageBubble
          message={renderItem.message}
          {isGroup}
          capabilities={capabilitiesForMessage(renderItem.message)}
          pendingReactions={pendingReactionsForMessage(renderItem.message.id)}
          isPinned={pinnedMessageIds.has(renderItem.message.id)}
          {onReply}
          {onRecall}
          {onForward}
          {onPin}
          {onToggleReaction}
          {onImageClick}
        />


      {/if}
    {/each}
    <!-- Bottom margin spacer for easy viewing & interaction above composer -->
    <div class="h-6 w-full shrink-0" aria-hidden="true" data-slot="timeline-bottom-spacer"></div>
  {/if}
</div>
