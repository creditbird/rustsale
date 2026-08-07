<script lang="ts">
  import { EMOJI_CATEGORIES } from './emojiData';
  import * as Tabs from '$lib/components/ui/tabs';
  import { Check, Heart, Smile, Sparkles, ThumbsUp, User } from 'lucide-svelte';
  import type { Component } from 'svelte';

  let { onSelect = () => {} }: { onSelect?: (emoji: string) => void } = $props();
  let activeTab = $state('smileys');

  const CATEGORY_ICONS: Record<string, any> = {
    smileys: Smile,
    gestures: ThumbsUp,
    hearts: Heart,
    people: User,
    objects: Sparkles,
    symbols: Check
  };

  let activeCategory = $derived(
    EMOJI_CATEGORIES.find((cat) => cat.id === activeTab) ?? EMOJI_CATEGORIES[0]
  );
</script>

<div
  class="flex h-[260px] w-[280px] flex-col overflow-hidden rounded-lg border bg-popover text-popover-foreground shadow-lg"
  data-testid="emoji-picker"
>
  <Tabs.Root bind:value={activeTab} class="flex h-full flex-col">
    <Tabs.List class="grid w-full grid-cols-6 border-b bg-muted/50 p-1">
      {#each EMOJI_CATEGORIES as cat (cat.id)}
        {@const Icon = CATEGORY_ICONS[cat.id] ?? Smile}
        <Tabs.Trigger value={cat.id} title={cat.label} class="h-7 px-0 py-0">
          <Icon class="size-3.5" />
        </Tabs.Trigger>
      {/each}
    </Tabs.List>

    <div class="flex-1 overflow-y-auto p-2">
      <div class="grid grid-cols-6 gap-1">
        {#each activeCategory.emojis as emoji (emoji)}
          <button
            type="button"
            class="inline-flex size-9 items-center justify-center rounded-md text-xl hover:bg-accent hover:text-accent-foreground cursor-pointer transition-colors select-none focus:outline-none focus:ring-1 focus:ring-ring"
            onclick={() => onSelect(emoji)}
            data-testid="emoji-option"
          >
            {emoji}
          </button>
        {/each}
      </div>
    </div>
  </Tabs.Root>
</div>
