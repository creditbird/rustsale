<script lang="ts">
  import Ellipsis from 'lucide-svelte/icons/ellipsis';

  let {
    open = $bindable(false),
    canReply,
    canCopy,
    canRecall,
    onReply,
    onCopy,
    onRecall
  }: {
    open?: boolean;
    canReply: boolean;
    canCopy: boolean;
    canRecall: boolean;
    onReply: () => void;
    onCopy: () => void;
    onRecall: () => void;
  } = $props();

  function run(action: () => void) {
    open = false;
    action();
  }

  function handleTriggerKeydown(event: KeyboardEvent) {
    if (event.shiftKey && event.key === 'F10') {
      event.preventDefault();
      open = true;
    }
  }

  let actions = $derived([
    ...(canReply ? [{ label: 'Trả lời', action: onReply, destructive: false }] : []),
    ...(canCopy ? [{ label: 'Sao chép', action: onCopy, destructive: false }] : []),
    ...(canRecall ? [{ label: 'Thu hồi', action: onRecall, destructive: true }] : [])
  ]);
</script>

{#if canReply || canCopy || canRecall}
  <div class="relative">
    <button
      type="button"
      data-testid="message-actions-trigger"
      aria-label="Thao tác tin nhắn"
      aria-expanded={open}
      class="grid size-7 place-items-center rounded-full text-muted-foreground hover:bg-muted hover:text-foreground"
      onclick={() => (open = !open)}
      onkeydown={handleTriggerKeydown}
    >
      <Ellipsis class="size-4" />
    </button>
    {#if open}
      <div class="absolute right-0 top-full z-30 mt-1 min-w-32 rounded-md border bg-popover p-1 text-sm text-popover-foreground shadow-md">
        {#each actions as action (action.label)}
          <button
            type="button"
            class="w-full rounded-sm px-2 py-1.5 text-left hover:bg-muted {action.destructive ? 'text-destructive' : ''}"
            onclick={() => run(action.action)}
          >
            {action.label}
          </button>
        {/each}
      </div>
    {/if}
  </div>
{/if}
