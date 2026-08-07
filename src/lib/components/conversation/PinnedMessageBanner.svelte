<script lang="ts">
  import type { PinnedMessage } from '$lib/services/conversationService';
  import { Button } from '$lib/components/ui/button';
  import { Pin, X, ArrowDown } from 'lucide-svelte';

  let {
    pinnedMessages = [],
    onJumpToMessage = () => {},
    onUnpin = () => {}
  }: {
    pinnedMessages: PinnedMessage[];
    onJumpToMessage: (messageId: number) => void;
    onUnpin: (messageId: number) => void;
  } = $props();

  let latestPinned = $derived(pinnedMessages[0] ?? null);
</script>

{#if latestPinned}
  <div
    class="flex items-center justify-between gap-3 px-3 py-1.5 bg-accent/40 border-b border-border text-xs animate-in fade-in slide-in-from-top-1 z-10 shrink-0"
    data-testid="pinned-message-banner"
  >
    <div class="flex items-center gap-2 min-w-0 flex-1">
      <Pin class="size-3.5 text-primary shrink-0 rotate-45" />
      <div class="flex flex-col min-w-0">
        <span class="font-semibold text-primary text-[11px]">
          Tin nhắn đã ghim {pinnedMessages.length > 1 ? `(${pinnedMessages.length})` : ''}
        </span>
        <p class="truncate text-foreground/90 font-normal">
          {latestPinned.senderName ? `${latestPinned.senderName}: ` : ''}{latestPinned.messageText || 'Tin nhắn đính kèm/media'}
        </p>
      </div>
    </div>

    <div class="flex items-center gap-1 shrink-0">
      <Button
        type="button"
        variant="ghost"
        size="icon"
        class="size-6 text-muted-foreground hover:text-foreground"
        title="Xem tin nhắn gốc"
        onclick={() => onJumpToMessage(latestPinned.messageId)}
      >
        <ArrowDown class="size-3.5" />
      </Button>

      <Button
        type="button"
        variant="ghost"
        size="icon"
        class="size-6 text-muted-foreground hover:text-destructive"
        title="Bỏ ghim"
        onclick={() => onUnpin(latestPinned.messageId)}
      >
        <X class="size-3.5" />
      </Button>
    </div>
  </div>
{/if}
