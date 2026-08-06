<script lang="ts">
  import { onDestroy } from 'svelte';
  import { searchZaloStickers, type ZaloStickerReference } from '$lib/services/conversationService';
  import { safeConversationUrl } from './messageMedia';
  import { Input } from '$lib/components/ui/input';
  import { Button } from '$lib/components/ui/button';
  import { Loader2, Search } from 'lucide-svelte';

  let {
    conversationId,
    disabled = false,
    onSelect = () => {},
    search = searchZaloStickers
  }: {
    conversationId: number;
    disabled?: boolean;
    onSelect?: (sticker: ZaloStickerReference) => void;
    search?: (conversationId: number, query: string, limit: number) => Promise<ZaloStickerReference[]>;
  } = $props();

  let query = $state('');
  let stickers = $state<ZaloStickerReference[]>([]);
  let isLoading = $state(false);
  let error = $state<string | null>(null);
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  let currentRequestId = 0;

  async function performSearch(searchTerm: string) {
    if (!conversationId || disabled) return;
    const reqId = ++currentRequestId;
    isLoading = true;
    error = null;

    try {
      const results = await search(conversationId, searchTerm.trim(), 40);
      if (reqId === currentRequestId) {
        stickers = results;
      }
    } catch (err) {
      if (reqId === currentRequestId) {
        error = err instanceof Error ? err.message : String(err);
      }
    } finally {
      if (reqId === currentRequestId) {
        isLoading = false;
      }
    }
  }

  function handleInput(e: Event) {
    const value = (e.target as HTMLInputElement).value;
    query = value;
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      performSearch(query);
    }, 300);
  }

  $effect(() => {
    performSearch('');
  });

  onDestroy(() => {
    currentRequestId += 1;
    if (debounceTimer) clearTimeout(debounceTimer);
  });
</script>

<div
  class="flex h-[300px] w-[320px] flex-col overflow-hidden rounded-lg border bg-popover text-popover-foreground shadow-lg"
  data-testid="zalo-sticker-picker"
>
  <div class="relative p-2 border-b">
    <Search class="absolute left-4 top-1/2 size-4 -translate-y-1/2 text-muted-foreground pointer-events-none" />
    <Input
      type="text"
      placeholder="Tìm nhãn dán..."
      value={query}
      oninput={handleInput}
      {disabled}
      class="h-8 pl-8 text-xs"
      data-testid="sticker-search-input"
    />
  </div>

  {#if isLoading}
    <div class="flex flex-1 items-center justify-center gap-2 p-4 text-xs text-muted-foreground" data-testid="sticker-loading">
      <Loader2 class="size-4 animate-spin" />
      <span>Đang tải...</span>
    </div>
  {:else if error}
    <div class="flex flex-1 items-center justify-center p-4 text-xs text-destructive" data-testid="sticker-error">
      {error}
    </div>
  {:else if stickers.length === 0}
    <div class="flex flex-1 items-center justify-center p-4 text-xs text-muted-foreground" data-testid="sticker-empty">
      Không tìm thấy sticker nào
    </div>
  {:else}
    <div class="grid flex-1 grid-cols-4 gap-2 overflow-y-auto p-2" data-testid="sticker-grid">
      {#each stickers as sticker (sticker.stickerId)}
        {@const safeUrl = safeConversationUrl(sticker.url)}
        {#if safeUrl}
          <Button
            type="button"
            variant="ghost"
            class="h-16 w-full p-1 hover:bg-accent"
            {disabled}
            onclick={() => onSelect(sticker)}
            data-testid="sticker-item-btn"
          >
            <img src={safeUrl} alt={`Sticker ${sticker.stickerId}`} class="size-14 object-contain" loading="lazy" />
          </Button>
        {/if}
      {/each}
    </div>
  {/if}
</div>
