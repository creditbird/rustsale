<script lang="ts">
  let { active = false }: { active?: boolean } = $props();
</script>

{#if active}
  <div class="attachment-drop-overlay" data-testid="attachment-drop-overlay">
    <div class="drop-message">
      <span class="drop-icon">📁</span>
      <span>Thả file vào đây để gửi</span>
    </div>
  </div>
{/if}

<style>
  .attachment-drop-overlay {
    position: absolute;
    inset: 0;
    z-index: 50;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(59, 130, 246, 0.15);
    border: 2px dashed #3b82f6;
    border-radius: 8px;
    backdrop-filter: blur(2px);
    pointer-events: none;
  }

  .drop-message {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    background: #ffffff;
    padding: 16px 24px;
    border-radius: 12px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
    font-weight: 600;
    color: #1d4ed8;
  }

  .drop-icon {
    font-size: 32px;
  }
</style>
