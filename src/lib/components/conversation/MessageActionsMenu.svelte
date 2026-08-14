<script lang="ts">
  import Ellipsis from 'lucide-svelte/icons/ellipsis';
  import type { ConversationReactionCode } from '$lib/services/conversationService';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';

  const QUICK_REACTIONS: Array<{ code: ConversationReactionCode; icon: string; label: string }> = [
    { code: 'heart', icon: '❤️', label: 'React ❤️' },
    { code: 'like', icon: '👍', label: 'React 👍' },
    { code: 'haha', icon: '😆', label: 'React 😆' },
    { code: 'wow', icon: '😮', label: 'React 😮' },
    { code: 'cry', icon: '😭', label: 'React 😭' },
    { code: 'angry', icon: '😡', label: 'React 😡' }
  ];

  let {
    open = $bindable(false),
    canReply,
    canCopy,
    canRecall,
    canReact = false,
    canForward = false,
    isPinned = false,
    onReply,
    onCopy,
    onRecall,
    onForward = () => {},
    onPin = () => {},
    onToggleReaction = () => {}
  }: {
    open?: boolean;
    canReply: boolean;
    canCopy: boolean;
    canRecall: boolean;
    canReact?: boolean;
    canForward?: boolean;
    isPinned?: boolean;
    onReply: () => void;
    onCopy: () => void;
    onRecall: () => void;
    onForward?: () => void;
    onPin?: () => void;
    onToggleReaction?: (code: ConversationReactionCode) => void;
  } = $props();


  let contentElement = $state<HTMLElement | null>(null);

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

  function dismissOnOutside(node: HTMLElement) {
    const handlePointerDown = (event: PointerEvent) => {
      if (!open || !(event.target instanceof Node)) return;
      if (node.contains(event.target) || contentElement?.contains(event.target)) return;
      open = false;
    };

    document.addEventListener('pointerdown', handlePointerDown, true);
    return {
      destroy() {
        document.removeEventListener('pointerdown', handlePointerDown, true);
      }
    };
  }

  let actions = $derived([
    ...(canReply ? [{ label: 'Reply', action: onReply, destructive: false }] : []),
    ...(canCopy ? [{ label: 'Copy', action: onCopy, destructive: false }] : []),
    { label: isPinned ? 'Unpin' : 'Pin message', action: onPin, destructive: false },
    ...(canForward ? [{ label: 'Forward', action: onForward, destructive: false }] : []),
    ...(canRecall ? [{ label: 'Recall', action: onRecall, destructive: true }] : [])
  ]);

</script>

{#if canReply || canCopy || canRecall || canReact || canForward || Boolean(onPin)}

  <div class="relative" use:dismissOnOutside>
    <DropdownMenu.Root bind:open>
      <DropdownMenu.Trigger
        type="button"
        data-testid="message-actions-trigger"
        aria-label="Message actions"
        aria-expanded={open}
        class="grid size-7 place-items-center rounded-full text-muted-foreground hover:bg-muted hover:text-foreground"
        onkeydown={handleTriggerKeydown}
      >
        <Ellipsis class="size-4" />
      </DropdownMenu.Trigger>
      <DropdownMenu.Content
        bind:ref={contentElement}
        align="end"
        side="bottom"
        sideOffset={4}
        class="z-50 min-w-36 w-auto rounded-md border p-1 text-sm shadow-md ring-0"
      >
        {#if canReact}
          <div class="mb-1 flex items-center justify-between border-b border-border/50 px-1 py-1">
            {#each QUICK_REACTIONS as react (react.code)}
              <button
                type="button"
                aria-label={react.label}
                class="flex size-6 items-center justify-center rounded-sm text-xs transition-transform hover:scale-125 hover:bg-muted"
                onclick={() => handleQuickReaction(react.code)}
              >
                {react.icon}
              </button>
            {/each}
          </div>
        {/if}
        {#each actions as action (action.label)}
          <DropdownMenu.Item
            variant={action.destructive ? 'destructive' : 'default'}
            class="rounded-sm px-2 py-1.5"
            onclick={() => run(action.action)}
          >
            {action.label}
          </DropdownMenu.Item>
        {/each}
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  </div>
{/if}
