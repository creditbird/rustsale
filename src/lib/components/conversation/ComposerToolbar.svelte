<script lang="ts">
  let {
    disabled = false,
    onToggleEmoji = () => {},
    onToggleSticker = () => {},
    onPickAttachment = () => {}
  }: {
    disabled?: boolean;
    onToggleEmoji?: () => void;
    onToggleSticker?: () => void;
    onPickAttachment?: () => void;
  } = $props();
</script>

<div class="composer-toolbar" data-testid="composer-toolbar">
  <button
    type="button"
    class="toolbar-btn"
    {disabled}
    aria-label="Toggle emoji picker"
    onclick={onToggleEmoji}
    data-testid="toggle-emoji-btn"
  >
    😊
  </button>

  <button
    type="button"
    class="toolbar-btn"
    {disabled}
    aria-label="Toggle sticker picker"
    onclick={onToggleSticker}
    data-testid="toggle-sticker-btn"
  >
    🏷️
  </button>

  <button
    type="button"
    class="toolbar-btn"
    {disabled}
    aria-label="Attach file"
    onclick={onPickAttachment}
    data-testid="attach-file-btn"
  >
    📎
  </button>
</div>

<style>
  .composer-toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 4px;
  }

  .toolbar-btn {
    border: none;
    background: none;
    cursor: pointer;
    font-size: 16px;
    padding: 4px 6px;
    border-radius: 4px;
    color: #4b5563;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .toolbar-btn:hover:not(:disabled) {
    background: #f3f4f6;
    color: #111827;
  }

  .toolbar-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
