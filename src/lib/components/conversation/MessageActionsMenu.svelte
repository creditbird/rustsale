<script lang="ts">
  import Ellipsis from 'lucide-svelte/icons/ellipsis';
  import type { ConversationReactionCode } from '$lib/services/conversationService';

  const QUICK_REACTIONS: Array<{ code: ConversationReactionCode; icon: string; label: string }> = [
    { code: 'heart', icon: '❤️', label: 'Thả cảm xúc ❤️' },
    { code: 'like', icon: '👍', label: 'Thả cảm xúc 👍' },
    { code: 'haha', icon: '😆', label: 'Thả cảm xúc 😆' },
    { code: 'wow', icon: '😮', label: 'Thả cảm xúc 😮' },
    { code: 'cry', icon: '😭', label: 'Thả cảm xúc 😭' },
    { code: 'angry', icon: '😡', label: 'Thả cảm xúc 😡' }
  ];

  let {
    open = $bindable(false),
    canReply,
    canCopy,
    canRecall,
    canReact = false,
    canForward = false,
    onReply,
    onCopy,
    onRecall,
    onForward = () => {},
    onToggleReaction = () => {}
  }: {
    open?: boolean;
    canReply: boolean;
    canCopy: boolean;
    canRecall: boolean;
    canReact?: boolean;
    canForward?: boolean;
    onReply: () => void;
    onCopy: () => void;
    onRecall: () => void;
    onForward?: () => void;
    onToggleReaction?: (code: ConversationReactionCode) => void;
  } = $props();

  function run(action: () => void) {
    open = false;
    action();
  }

  function handleQuickReaction(code: ConversationReactionCode) {
    open = false;
    onToggleReaction(code);
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
    ...(canForward ? [{ label: 'Chuyển tiếp', action: onForward, destructive: false }] : []),
    ...(canRecall ? [{ label: 'Thu hồi', action: onRecall, destructive: true }] : [])
  ]);
</script>

{#if canReply || canCopy || canRecall || canReact || canForward}
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
      <div class="absolute right-0 top-full z-30 mt-1 min-w-36 rounded-md border bg-popover p-1 text-sm text-popover-foreground shadow-md">
        {#if canReact}
          <div class="flex items-center justify-between border-b border-border/50 px-1 py-1 mb-1">
            {#each QUICK_REACTIONS as react (react.code)}
              <button
                type="button"
                aria-label={react.label}
                class="flex size-6 items-center justify-center rounded-sm text-xs hover:bg-muted transition-transform hover:scale-125"
                onclick={() => handleQuickReaction(react.code)}
              >
                {react.icon}
              </button>
            {/each}
          </div>
        {/if}
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
