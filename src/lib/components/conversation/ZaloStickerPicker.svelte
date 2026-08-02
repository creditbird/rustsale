<script lang="ts">
  import { onDestroy } from 'svelte';
  import { searchZaloStickers, type ZaloStickerReference } from '$lib/services/conversationService';
  import { safeConversationUrl } from './messageMedia';

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

<div class="sticker-picker-container" data-testid="zalo-sticker-picker">
  <div class="search-bar">
    <input
      type="text"
      placeholder="Tìm nhãn dán..."
      value={query}
      oninput={handleInput}
      {disabled}
      class="sticker-search-input"
      data-testid="sticker-search-input"
    />
  </div>

  {#if isLoading}
    <div class="sticker-loading" data-testid="sticker-loading">Đang tải...</div>
  {:else if error}
    <div class="sticker-error" data-testid="sticker-error">{error}</div>
  {:else if stickers.length === 0}
    <div class="sticker-empty" data-testid="sticker-empty">Không tìm thấy sticker nào</div>
  {:else}
    <div class="sticker-grid" data-testid="sticker-grid">
      {#each stickers as sticker (sticker.stickerId)}
        {@const safeUrl = safeConversationUrl(sticker.url)}
        {#if safeUrl}
          <button
            type="button"
            class="sticker-item"
            {disabled}
            onclick={() => onSelect(sticker)}
            data-testid="sticker-item-btn"
          >
            <img src={safeUrl} alt={`Sticker ${sticker.stickerId}`} loading="lazy" />
          </button>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .sticker-picker-container {
    width: 320px;
    height: 300px;
    display: flex;
    flex-direction: column;
    background: #ffffff;
    border: 1px solid #e5e7eb;
    border-radius: 8px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
    overflow: hidden;
  }

  .search-bar {
    padding: 8px;
    border-bottom: 1px solid #e5e7eb;
  }

  .sticker-search-input {
    width: 100%;
    padding: 6px 10px;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 13px;
    outline: none;
  }

  .sticker-search-input:focus {
    border-color: #3b82f6;
  }

  .sticker-loading,
  .sticker-error,
  .sticker-empty {
    padding: 16px;
    text-align: center;
    font-size: 13px;
    color: #6b7280;
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .sticker-error {
    color: #ef4444;
  }

  .sticker-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
    padding: 8px;
    overflow-y: auto;
    flex: 1;
  }

  .sticker-item {
    border: none;
    background: none;
    cursor: pointer;
    padding: 4px;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .sticker-item:hover {
    background: #f3f4f6;
  }

  .sticker-item img {
    width: 60px;
    height: 60px;
    object-fit: contain;
  }
</style>
