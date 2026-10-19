<script lang="ts">
  import type { ConversationReactionCode } from '$lib/services/conversationService';

  export interface ReactionOption {
    code: ConversationReactionCode;
    icon: string;
    label: string;
  }

  export const REACTION_OPTIONS: ReactionOption[] = [
    { code: 'heart', icon: '❤️', label: 'Love' },
    { code: 'like', icon: '👍', label: 'Like' },
    { code: 'haha', icon: '😆', label: 'Haha' },
    { code: 'wow', icon: '😮', label: 'Wow' },
    { code: 'cry', icon: '😭', label: 'Sad' },
    { code: 'angry', icon: '😡', label: 'Angry' }
  ];

  let {
    messageId,
    activeReactions = [],
    disabledReactions = [],
    supportedReactions = REACTION_OPTIONS.map((option) => option.code),
    onSelect = () => {},
    onClose = () => {}
  }: {
    messageId?: number;
    activeReactions?: ConversationReactionCode[];
    disabledReactions?: ConversationReactionCode[];
    supportedReactions?: ConversationReactionCode[];
    onSelect?: (code: ConversationReactionCode, action: 'increment') => void;
    onClose?: () => void;
  } = $props();

  let focusedIndex = $state(0);
  let buttonRefs: (HTMLButtonElement | null)[] = $state([]);
  let visibleOptions = $derived(
    REACTION_OPTIONS.filter((option) => supportedReactions.includes(option.code))
  );

  function isCodeActive(code: ConversationReactionCode): boolean {
    return activeReactions.includes(code);
  }

  function isCodeDisabled(code: ConversationReactionCode): boolean {
    return disabledReactions.includes(code);
  }

  function handleClick(code: ConversationReactionCode) {
    if (isCodeDisabled(code)) return;
    onSelect(code, 'increment');
  }

  function focusIndex(index: number) {
    if (visibleOptions.length === 0) return;
    const nextIndex = (index + visibleOptions.length) % visibleOptions.length;
    focusedIndex = nextIndex;
    buttonRefs[nextIndex]?.focus();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
      event.preventDefault();
      focusIndex(focusedIndex + 1);
    } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
      event.preventDefault();
      focusIndex(focusedIndex - 1);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    }
  }

  function handleItemKeydown(event: KeyboardEvent, code: ConversationReactionCode) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      handleClick(code);
    }
  }
</script>

<div
  role="toolbar"
  tabindex="-1"
  aria-label="Choose reaction"
  class="flex items-center gap-1 rounded-full border bg-popover/95 p-1 text-popover-foreground shadow-lg backdrop-blur-xs select-none"
  onkeydown={handleKeydown}
  data-testid="message-reaction-picker"
>
  {#each visibleOptions as option, index (option.code)}
    {@const active = isCodeActive(option.code)}
    {@const disabled = isCodeDisabled(option.code)}
    <button
      type="button"
      bind:this={buttonRefs[index]}
      tabindex={index === focusedIndex ? 0 : -1}
      aria-label={option.label}
      aria-pressed={active}
      {disabled}
      class="flex size-7 items-center justify-center rounded-full text-base transition-transform hover:scale-125 focus:outline-hidden focus:ring-2 focus:ring-ring focus:ring-offset-1 disabled:opacity-40 disabled:hover:scale-100 {active ? 'bg-accent/60 scale-110 ring-1 ring-primary' : ''}"
      onclick={() => handleClick(option.code)}
      onkeydown={(e) => handleItemKeydown(e, option.code)}
      onfocus={() => (focusedIndex = index)}
    >
      <span>{option.icon}</span>
    </button>
  {/each}
</div>
