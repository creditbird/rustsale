<script lang="ts">
  import type {
    ConversationReactionCode,
    ConversationReactionGroup
  } from '$lib/services/conversationService';
  import { normalizeReactionIcon, reactionIconToCode } from './reactionDisplay';

  let {
    groups,
    canReact = true,
    pendingReactions = [],
    onToggleReaction = () => Promise.resolve(null)
  }: {
    groups: ConversationReactionGroup[];
    canReact?: boolean;
    pendingReactions?: ConversationReactionCode[];
    onToggleReaction?: (
      reactionCode: ConversationReactionCode,
      action: 'increment' | 'remove'
    ) => Promise<unknown> | void;
  } = $props();

  let expandedIcon = $state<string | null>(null);
  let optimisticIncrements = $state(new Map<string, number>());

  function clearOptimisticIncrement(icon: string) {
    if (!optimisticIncrements.has(icon)) return;
    const next = new Map(optimisticIncrements);
    next.delete(icon);
    optimisticIncrements = next;
  }

  function handleChipClick(group: ConversationReactionGroup) {
    const code = reactionIconToCode(group.reactionIcon);
    if (canReact && code !== null) {
      const next = new Map(optimisticIncrements);
      next.set(group.reactionIcon, (next.get(group.reactionIcon) ?? 0) + 1);
      optimisticIncrements = next;
      const mutation = onToggleReaction(code, 'increment');
      if (mutation && typeof mutation.finally === 'function') {
        void mutation.finally(() => clearOptimisticIncrement(group.reactionIcon));
      } else {
        queueMicrotask(() => clearOptimisticIncrement(group.reactionIcon));
      }
    } else {
      toggleGroup(group.reactionIcon);
    }
  }

  function handleChipContextMenu(event: MouseEvent, group: ConversationReactionGroup) {
    const code = reactionIconToCode(group.reactionIcon);
    if (canReact && code !== null) {
      event.preventDefault();
      event.stopPropagation();
      if (group.includesMe) {
        void onToggleReaction(code, 'remove');
      }
    }
  }

  function toggleGroup(icon: string) {
    expandedIcon = expandedIcon === icon ? null : icon;
  }
</script>

{#if groups.length > 0}
  <div class="flex flex-wrap items-center gap-1 pt-1" data-testid="message-reaction-summary">
    {#each groups as group (group.reactionIcon)}
      {@const code = reactionIconToCode(group.reactionIcon)}
      {@const displayIcon = normalizeReactionIcon(group.reactionIcon)}
      {@const pending = code !== null && pendingReactions.includes(code)}
      {@const displayCount = group.count + (optimisticIncrements.get(group.reactionIcon) ?? 0)}
      <div class="relative">
        <button
          type="button"
          disabled={pending}
          title={canReact ? 'Click chuột trái để thả thêm (+1). Chuột phải để xóa cảm xúc.' : ''}
          class="rounded-full border border-border/60 bg-background/90 px-2 py-0.5 text-xs shadow-xs hover:bg-muted transition-colors disabled:opacity-50 {group.includesMe
            ? 'border-primary/50 bg-primary/10 text-primary font-medium'
            : ''}"
          data-current-account={group.includesMe ? 'true' : 'false'}
          aria-expanded={expandedIcon === group.reactionIcon}
          onclick={() => handleChipClick(group)}
          oncontextmenu={(e) => handleChipContextMenu(e, group)}
          onmouseenter={() => (expandedIcon = group.reactionIcon)}
          onmouseleave={() => (expandedIcon = null)}
        >
          {displayIcon} {displayCount}
        </button>
        {#if expandedIcon === group.reactionIcon}
          <div class="absolute bottom-full left-0 z-20 mb-1 min-w-40 rounded-md border bg-popover p-2 text-popover-foreground shadow-md">
            <ul class="m-0 space-y-1 p-0 text-xs">
              {#each group.actors as actor (`${actor.externalId}-${actor.identityId ?? 'external'}`)}
                <li class="list-none whitespace-nowrap">
                  {actor.displayName || actor.externalId}
                </li>
              {/each}
            </ul>
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}
