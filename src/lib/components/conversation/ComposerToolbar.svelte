<script lang="ts">
  import type { ConversationCapabilities, ZaloStickerReference } from '$lib/services/conversationService';
  import { Button } from '$lib/components/ui/button';
  import * as Popover from '$lib/components/ui/popover';
  import EmojiPicker from './EmojiPicker.svelte';
  import ZaloStickerPicker from './ZaloStickerPicker.svelte';
  import { Paperclip, Smile, Tag, Zap } from 'lucide-svelte';

  let {
    capabilities,
    targetConversationId = null,
    disabled = false,
    activePopover = null,
    onEmojiSelect = () => {},
    onStickerSelect = () => {},
    onToggleEmoji = () => {},
    onToggleSticker = () => {},
    onPickAttachment = () => {},
    onToggleQuickTemplates = () => {}
  }: {
    capabilities: ConversationCapabilities;
    targetConversationId?: number | null;
    disabled?: boolean;
    activePopover?: 'emoji' | 'sticker' | null;
    onEmojiSelect?: (emoji: string) => void;
    onStickerSelect?: (sticker: ZaloStickerReference) => void;
    onToggleEmoji?: () => void;
    onToggleSticker?: () => void;
    onPickAttachment?: () => void;
    onToggleQuickTemplates?: () => void;
  } = $props();

  let emojiOpen = $derived(activePopover === 'emoji');
  let stickerOpen = $derived(activePopover === 'sticker');

  function handleOpenChangeEmoji(open: boolean) {
    if (open !== emojiOpen) {
      onToggleEmoji();
    }
  }

  function handleOpenChangeSticker(open: boolean) {
    if (open !== stickerOpen) {
      onToggleSticker();
    }
  }

  function handleSelectEmoji(emoji: string) {
    onEmojiSelect(emoji);
  }
</script>

<div class="flex items-center gap-1 p-0.5" data-testid="composer-toolbar">
  <Popover.Root open={emojiOpen} onOpenChange={handleOpenChangeEmoji}>
    <Popover.Trigger
      disabled={disabled}
      aria-label="Toggle emoji picker"
      class="inline-flex size-8 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground focus:outline-hidden disabled:opacity-50 disabled:pointer-events-none cursor-pointer"
      data-testid="toggle-emoji-btn"
    >
      <Smile class="size-4" />
    </Popover.Trigger>
    <Popover.Content
      side="top"
      align="end"
      sideOffset={8}
      class="w-auto border-none bg-transparent p-0 shadow-none ring-0 z-50"
    >
      <EmojiPicker onSelect={handleSelectEmoji} />
    </Popover.Content>
  </Popover.Root>

  {#if capabilities.supportsStickers}
    <Popover.Root open={stickerOpen} onOpenChange={handleOpenChangeSticker}>
      <Popover.Trigger
        disabled={disabled}
        aria-label="Toggle sticker picker"
        class="inline-flex size-8 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground focus:outline-hidden disabled:opacity-50 disabled:pointer-events-none cursor-pointer"
        data-testid="toggle-sticker-btn"
      >
        <Tag class="size-4" />
      </Popover.Trigger>
      <Popover.Content
        side="top"
        align="end"
        sideOffset={8}
        class="w-auto border-none bg-transparent p-0 shadow-none ring-0 z-50"
      >
        {#if targetConversationId !== null}
          <ZaloStickerPicker
            conversationId={targetConversationId}
            disabled={disabled}
            onSelect={onStickerSelect}
          />
        {/if}
      </Popover.Content>
    </Popover.Root>
  {/if}

  {#if capabilities.maxAttachmentsPerBatch > 0}
    <Button
      type="button"
      variant="ghost"
      size="icon"
      class="size-8 text-muted-foreground hover:text-foreground"
      {disabled}
      aria-label="Attach file"
      onclick={onPickAttachment}
      data-testid="attach-file-btn"
    >
      <Paperclip class="size-4" />
    </Button>
  {/if}

  {#if capabilities.quickTemplates?.enabled}
    <Button
      type="button"
      variant="ghost"
      size="icon"
      class="size-8 text-muted-foreground hover:text-foreground"
      {disabled}
      aria-label="Manage quick templates"
      onclick={onToggleQuickTemplates}
      data-testid="toggle-quick-templates-btn"
    >
      <Zap class="size-4" />
    </Button>
  {/if}
</div>
