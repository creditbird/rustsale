<script lang="ts">
  import type { ConversationQuickTemplate } from '$lib/services/conversationService';
  import { rankQuickTemplates } from './quickTemplateState';

  let {
    templates = [],
    query = '',
    selectedIndex = 0,
    onSelect = () => {},
    onOpenManager = () => {}
  }: {
    templates?: ConversationQuickTemplate[];
    query?: string;
    selectedIndex?: number;
    onSelect?: (template: ConversationQuickTemplate) => void;
    onOpenManager?: () => void;
  } = $props();

  let rankedTemplates = $derived(rankQuickTemplates(templates, query));
</script>

<div
  class="quick-template-popover border border-border bg-popover text-popover-foreground shadow-md rounded-lg w-80 overflow-hidden text-xs flex flex-col z-50"
  data-testid="quick-template-popover"
>
  <div class="flex items-center justify-between px-3 py-2 border-b border-border bg-muted/40">
    <span class="font-medium text-muted-foreground text-[11px] uppercase tracking-wider">
      Quick Message Templates
    </span>
    <button
      type="button"
      class="text-xs text-primary hover:underline font-medium cursor-pointer"
      onclick={onOpenManager}
      data-testid="manage-quick-templates-btn"
    >
      Manage Templates
    </button>
  </div>

  <div class="max-h-60 overflow-y-auto p-1 divide-y divide-border/30" role="listbox">
    {#if rankedTemplates.length === 0}
      <div class="p-4 text-center text-muted-foreground text-xs">
        No message templates found
      </div>
    {:else}
      {#each rankedTemplates as item, i (item.id)}
        <button
          type="button"
          role="option"
          class="w-full text-left p-2 rounded flex flex-col gap-1 transition-colors cursor-pointer {i === selectedIndex
            ? 'bg-accent text-accent-foreground font-medium'
            : 'hover:bg-muted/50'}"
          aria-selected={i === selectedIndex ? 'true' : 'false'}
          onclick={() => onSelect(item)}
          data-testid="quick-template-item-{item.id}"
        >
          <div class="flex items-center justify-between gap-2">
            <span class="font-semibold text-primary text-xs font-mono">
              /{item.shortcut}
            </span>
            {#if item.category}
              <span class="px-1.5 py-0.5 text-[10px] rounded bg-muted text-muted-foreground">
                {item.category}
              </span>
            {/if}
          </div>
          <div class="font-medium line-clamp-1 text-foreground">
            {item.title}
          </div>
          <div class="text-[11px] text-muted-foreground line-clamp-2 leading-tight">
            {item.bodyText}
          </div>
        </button>
      {/each}
    {/if}
  </div>
</div>
