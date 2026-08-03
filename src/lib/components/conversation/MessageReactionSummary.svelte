<script lang="ts">
  import type {
    ConversationReactionCode,
    ConversationReactionGroup
  } from '$lib/services/conversationService';

  let {
    groups,
    canReact = true,
    pendingReactions = [],
    onToggleReaction = () => {}
  }: {
    groups: ConversationReactionGroup[];
    canReact?: boolean;
    pendingReactions?: ConversationReactionCode[];
    onToggleReaction?: (reactionCode: ConversationReactionCode, currentActive: boolean) => void;
  } = $props();

  let expandedIcon = $state<string | null>(null);

  function iconToCode(icon: string): ConversationReactionCode {
    switch (icon) {
      case '❤️':
      case 'heart':
        return 'heart';
      case '👍':
      case 'like':
        return 'like';
      case '😆':
      case '😄':
      case 'haha':
        return 'haha';
      case '😮':
      case 'wow':
        return 'wow';
      case '😭':
      case '😢':
      case 'cry':
      case 'sad':
        return 'cry';
      case '😡':
      case 'angry':
        return 'angry';
      default:
        return 'like';
    }
  }

  function handleChipClick(group: ConversationReactionGroup) {
    if (canReact) {
      const code = iconToCode(group.reactionIcon);
      onToggleReaction(code, group.includesMe);
    } else {
      toggleGroup(group.reactionIcon);
    }
  }

  function toggleGroup(icon: string) {
    expandedIcon = expandedIcon === icon ? null : icon;
  }
</script>

{#if groups.length > 0}
  <div class="flex flex-wrap items-center gap-1 pt-1" data-testid="message-reaction-summary">
    {#each groups as group (group.reactionIcon)}
      {@const code = iconToCode(group.reactionIcon)}
      {@const pending = pendingReactions.includes(code)}
      <div class="relative">
        <button
          type="button"
          disabled={pending}
          class="rounded-full border border-border/60 bg-background/90 px-2 py-0.5 text-xs shadow-xs hover:bg-muted transition-colors disabled:opacity-50 {group.includesMe
            ? 'border-primary/50 bg-primary/10 text-primary font-medium'
            : ''}"
          data-current-account={group.includesMe ? 'true' : 'false'}
          aria-expanded={expandedIcon === group.reactionIcon}
          onclick={() => handleChipClick(group)}
          onmouseenter={() => (expandedIcon = group.reactionIcon)}
          onmouseleave={() => (expandedIcon = null)}
        >
          {group.reactionIcon} {group.count}
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
