<script lang="ts">
  import { EMOJI_CATEGORIES } from './emojiData';

  let { onSelect = () => {} }: { onSelect?: (emoji: string) => void } = $props();
  let activeTab = $state('smileys');
</script>

<div class="emoji-picker-container" data-testid="emoji-picker">
  <div class="category-tabs">
    {#each EMOJI_CATEGORIES as cat (cat.id)}
      <button
        type="button"
        class="tab-btn"
        class:active={activeTab === cat.id}
        title={cat.label}
        onclick={() => (activeTab = cat.id)}
      >
        {cat.emojis[0]}
      </button>
    {/each}
  </div>

  <div class="emoji-grid">
    {#each EMOJI_CATEGORIES.find((c) => c.id === activeTab)?.emojis ?? [] as emoji}
      <button
        type="button"
        class="emoji-btn"
        onclick={() => onSelect(emoji)}
        data-testid="emoji-option"
      >
        {emoji}
      </button>
    {/each}
  </div>
</div>

<style>
  .emoji-picker-container {
    width: 280px;
    height: 220px;
    display: flex;
    flex-direction: column;
    background: #ffffff;
    border: 1px solid #e5e7eb;
    border-radius: 8px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
    overflow: hidden;
  }

  .category-tabs {
    display: flex;
    border-bottom: 1px solid #e5e7eb;
    background: #f9fafb;
    padding: 4px;
    gap: 2px;
  }

  .tab-btn {
    flex: 1;
    border: none;
    background: none;
    font-size: 16px;
    padding: 4px;
    cursor: pointer;
    border-radius: 4px;
  }

  .tab-btn.active {
    background: #ffffff;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
  }

  .emoji-grid {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 4px;
    padding: 8px;
    overflow-y: auto;
    flex: 1;
  }

  .emoji-btn {
    border: none;
    background: none;
    font-size: 20px;
    padding: 4px;
    cursor: pointer;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .emoji-btn:hover {
    background: #f3f4f6;
  }
</style>
