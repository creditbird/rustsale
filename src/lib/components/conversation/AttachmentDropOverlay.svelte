<script lang="ts">
  let { active = false }: { active?: boolean } = $props();
</script>

{#if active}
  <div
    class="attachment-drop-overlay pointer-events-none absolute inset-0 z-50 flex items-center justify-center rounded-lg border-2 border-dashed border-blue-500 bg-blue-500/15 backdrop-blur-[2px]"
    data-testid="attachment-drop-overlay"
  >
    <div
      class="drop-message flex flex-col items-center gap-2 rounded-xl bg-background p-4 px-6 font-semibold text-blue-600 shadow-lg border border-border/40 dark:text-blue-400"
    >
      <span class="drop-icon text-3xl">📁</span>
      <span>Drop files here to send</span>
    </div>
  </div>
{/if}

