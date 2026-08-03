<script lang="ts">
  import type { ConversationReactionCode } from '$lib/services/conversationService';

  export interface ReactionOption {
    code: ConversationReactionCode;
    icon: string;
    label: string;
  }

  export const REACTION_OPTIONS: ReactionOption[] = [
    { code: 'heart', icon: '❤️', label: 'Yêu thích' },
    { code: 'like', icon: '👍', label: 'Thích' },
    { code: 'haha', icon: '😆', label: 'Cười' },
    { code: 'wow', icon: '😮', label: 'Bất ngờ' },
    { code: 'cry', icon: '😭', label: 'Buồn' },
    { code: 'angry', icon: '😡', label: 'Tức giận' }
  ];

  let {
    messageId,
    activeReactions = [],
    disabledReactions = [],
    onSelect = () => {},
    onClose = () => {}
  }: {
    messageId?: number;
    activeReactions?: ConversationReactionCode[];
    disabledReactions?: ConversationReactionCode[];
    onSelect?: (code: ConversationReactionCode, active: boolean) => void;
    onClose?: () => void;
  } = $props();

  let focusedIndex = $state(0);
  let buttonRefs: (HTMLButtonElement | null)[] = $state([]);

  function isCodeActive(code: ConversationReactionCode): boolean {
    return activeReactions.includes(code);
  }

  function isCodeDisabled(code: ConversationReactionCode): boolean {
    return disabledReactions.includes(code);
  }

  function handleClick(code: ConversationReactionCode) {
    if (isCodeDisabled(code)) return;
    const activeState = isCodeActive(code);
    onSelect(code, activeState);
  }

  function focusIndex(index: number) {
    const nextIndex = (index + REACTION_OPTIONS.length) % REACTION_OPTIONS.length;
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

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  role="toolbar"
  tabindex="-1"
  aria-label="Chọn cảm xúc"
  class="flex items-center gap-1 rounded-full border bg-popover/95 p-1 text-popover-foreground shadow-lg backdrop-blur-xs select-none"
  onkeydown={handleKeydown}
  data-testid="message-reaction-picker"
>
  {#each REACTION_OPTIONS as option, index (option.code)}
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
