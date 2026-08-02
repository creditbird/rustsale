<script lang="ts">
  import type { ConversationReactionGroup } from '$lib/services/conversationService';

  let { groups }: { groups: ConversationReactionGroup[] } = $props();
  let expandedIcon = $state<string | null>(null);

  function toggleGroup(icon: string) {
    expandedIcon = expandedIcon === icon ? null : icon;
  }
</script>

{#if groups.length > 0}
  <div class="flex flex-wrap items-center gap-1 pt-1" data-testid="message-reaction-summary">
    {#each groups as group (group.reactionIcon)}
      <div class="relative">
        <button
          type="button"
          class="rounded-full border border-border/60 bg-background/90 px-2 py-0.5 text-xs shadow-xs hover:bg-muted"
          data-current-account={group.includesMe ? 'true' : 'false'}
          aria-expanded={expandedIcon === group.reactionIcon}
          onclick={() => toggleGroup(group.reactionIcon)}
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
