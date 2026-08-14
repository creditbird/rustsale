<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Search, ChevronUp, ChevronDown, X } from 'lucide-svelte';

  let {
    open = $bindable(false),
    searchQuery = $bindable(''),
    matchCount = 0,
    currentIndex = 0,
    onNext = () => {},
    onPrev = () => {},
    onClose = () => {}
  }: {
    open?: boolean;
    searchQuery?: string;
    matchCount?: number;
    currentIndex?: number;
    onNext?: () => void;
    onPrev?: () => void;
    onClose?: () => void;
  } = $props();

  let inputElement = $state<HTMLInputElement | null>(null);

  $effect(() => {
    if (open) {
      setTimeout(() => inputElement?.focus(), 50);
    }
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      onClose();
    } else if (event.key === 'Enter') {
      if (event.shiftKey) {
        onPrev();
      } else {
        onNext();
      }
    }
  }
</script>

{#if open}
  <div
    class="flex items-center gap-2 p-2 bg-background border-b border-border shadow-xs animate-in fade-in slide-in-from-top-2 z-20 shrink-0"
    data-testid="in-thread-search-bar"
  >
    <div class="relative flex-1">
      <Search class="w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground" />
      <input
        bind:this={inputElement}
        type="text"
        placeholder="Search in conversation (Ctrl+F)..."
        bind:value={searchQuery}
        onkeydown={handleKeydown}
        class="flex h-8 w-full rounded-md border border-input bg-transparent pl-8 pr-16 text-xs shadow-xs transition-colors placeholder:text-muted-foreground focus-visible:outline-hidden focus-visible:ring-1 focus-visible:ring-ring"
      />

      {#if searchQuery.trim()}
        <span class="absolute right-2 top-1/2 -translate-y-1/2 text-[11px] text-muted-foreground font-medium select-none">
          {matchCount > 0 ? `${currentIndex + 1}/${matchCount}` : '0 results'}
        </span>
      {/if}
    </div>

    <div class="flex items-center gap-0.5">
      <Button
        type="button"
        variant="ghost"
        size="icon"
        class="size-7 text-muted-foreground hover:text-foreground"
        disabled={matchCount === 0}
        onclick={onPrev}
        title="Previous match (Shift+Enter)"
      >
        <ChevronUp class="size-4" />
      </Button>

      <Button
        type="button"
        variant="ghost"
        size="icon"
        class="size-7 text-muted-foreground hover:text-foreground"
        disabled={matchCount === 0}
        onclick={onNext}
        title="Next match (Enter)"
      >
        <ChevronDown class="size-4" />
      </Button>

      <Button
        type="button"
        variant="ghost"
        size="icon"
        class="size-7 text-muted-foreground hover:text-foreground"
        onclick={onClose}
        title="Close (Esc)"
      >
        <X class="size-4" />
      </Button>
    </div>
  </div>
{/if}
