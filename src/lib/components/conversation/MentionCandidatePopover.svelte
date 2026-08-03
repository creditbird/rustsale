<script lang="ts">
  import {
    listConversationMentionCandidates,
    type ConversationMentionCandidate
  } from '$lib/services/conversationService';
  import { Badge } from '$lib/components/ui/badge';
  import { AlertTriangle } from 'lucide-svelte';

  let {
    conversationId,
    query,
    selectedIndex = 0,
    maxMentions = 20,
    currentMentionCount = 0,
    onSelect,
    onCandidatesLoaded = () => {}
  }: {
    conversationId: number;
    query: string;
    selectedIndex?: number;
    maxMentions?: number;
    currentMentionCount?: number;
    onSelect: (candidate: ConversationMentionCandidate) => void;
    onCandidatesLoaded?: (candidates: ConversationMentionCandidate[]) => void;
  } = $props();

  let candidates = $state<ConversationMentionCandidate[]>([]);
  let isLoading = $state(false);
  let isLimitReached = $derived(currentMentionCount >= maxMentions);

  let timer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    const q = query;
    const cid = conversationId;
    const limitExceeded = isLimitReached;

    if (timer) clearTimeout(timer);

    if (limitExceeded) {
      candidates = [];
      onCandidatesLoaded([]);
      return;
    }

    isLoading = true;
    timer = setTimeout(() => {
      void fetchCandidates(cid, q);
    }, 150);

    return () => {
      if (timer) clearTimeout(timer);
    };
  });

  async function fetchCandidates(cid: number, q: string): Promise<void> {
    try {
      const result = await listConversationMentionCandidates({
        conversationId: cid,
        query: q,
        limit: maxMentions
      });
      candidates = result;
      onCandidatesLoaded(result);
    } catch {
      candidates = [];
      onCandidatesLoaded([]);
    } finally {
      isLoading = false;
    }
  }

  function getInitials(name: string): string {
    if (!name) return '?';
    const parts = name.trim().split(/\s+/);
    if (parts.length >= 2) {
      return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
    }
    return name.slice(0, 2).toUpperCase();
  }
</script>

<div
  class="absolute bottom-full left-0 mb-2 w-80 max-h-64 overflow-y-auto rounded-lg border border-border bg-popover text-popover-foreground shadow-lg z-50 p-1"
  data-testid="mention-candidate-popover"
  role="listbox"
>
  {#if isLimitReached}
    <div
      class="flex items-center gap-2 p-3 text-xs text-amber-600 dark:text-amber-400 font-medium"
      data-testid="mention-limit-warning"
    >
      <AlertTriangle class="h-4 w-4 shrink-0" />
      <span>Đã đạt giới hạn tối đa {maxMentions} nhắc tên trên một tin nhắn</span>
    </div>
  {:else if isLoading && candidates.length === 0}
    <div class="p-3 text-center text-xs text-muted-foreground" data-testid="mention-loading">
      Đang tải danh sách thành viên...
    </div>
  {:else if candidates.length === 0}
    <div class="p-3 text-center text-xs text-muted-foreground" data-testid="mention-empty-notice">
      Không tìm thấy thành viên
    </div>
  {:else}
    <div class="py-1">
      {#each candidates as candidate, index (candidate.identityId)}
        <button
          type="button"
          class="flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground {index ===
          selectedIndex
            ? 'bg-accent text-accent-foreground font-medium'
            : ''}"
          role="option"
          aria-selected={index === selectedIndex}
          data-testid="mention-candidate-{candidate.identityId}"
          onclick={() => onSelect(candidate)}
        >
          <div class="h-7 w-7 rounded-full bg-muted flex items-center justify-center overflow-hidden shrink-0 text-[10px] font-semibold text-muted-foreground">
            {#if candidate.avatarUrl}
              <img src={candidate.avatarUrl} alt={candidate.displayName} class="h-full w-full object-cover" />
            {:else}
              <span>{getInitials(candidate.displayName)}</span>
            {/if}
          </div>
          <div class="flex min-w-0 flex-1 items-center justify-between gap-1.5">
            <span class="truncate text-sm font-medium">{candidate.displayName}</span>
            {#if candidate.participantRole && candidate.participantRole !== 'member'}
              <Badge variant="outline" class="text-[10px] uppercase px-1 py-0 shrink-0">
                {candidate.participantRole}
              </Badge>
            {/if}
          </div>
        </button>
      {/each}
    </div>
  {/if}
</div>
