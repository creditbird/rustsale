<script lang="ts">
  import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
  import type { Channel, ChannelBadgeSummary } from '$lib/services/conversationService';
  import MessageSquare from 'lucide-svelte/icons/message-square';
  import MessageCircle from 'lucide-svelte/icons/message-circle';
  import MessageCircleMore from 'lucide-svelte/icons/message-circle-more';
  import Mail from 'lucide-svelte/icons/mail';
  import Phone from 'lucide-svelte/icons/phone';

  let {
    channels,
    activeChannels,
    onToggle
  }: {
    channels: ChannelBadgeSummary[];
    activeChannels: Channel[];
    onToggle: (channel: Channel) => void;
  } = $props();

  const channelIcons: Record<Channel, typeof MessageSquare> = {
    zalo: MessageSquare,
    whatsapp: MessageCircleMore,
    messenger: MessageCircle,
    email: Mail
  };

  const channelLabels: Record<Channel, string> = {
    zalo: 'Zalo',
    whatsapp: 'WhatsApp',
    messenger: 'Messenger',
    email: 'Email'
  };

  const channelStyles: Record<Channel, { active: string; inactive: string; countActive: string; countInactive: string }> = {
    zalo: {
      active: 'bg-blue-600 text-white border-blue-600 shadow-2xs hover:bg-blue-700 data-[state=on]:bg-blue-600 data-[state=on]:text-white data-[state=on]:border-blue-600',
      inactive: 'bg-blue-50 dark:bg-blue-950/50 text-blue-700 dark:text-blue-300 border-blue-200 dark:border-blue-800/60 hover:bg-blue-100 dark:hover:bg-blue-900/60 data-[state=off]:bg-blue-50 data-[state=off]:text-blue-700',
      countActive: 'bg-white/25 text-white font-extrabold',
      countInactive: 'bg-blue-500/20 text-blue-800 dark:text-blue-200 font-extrabold'
    },
    whatsapp: {
      active: 'bg-emerald-600 text-white border-emerald-600 shadow-2xs hover:bg-emerald-700 data-[state=on]:bg-emerald-600 data-[state=on]:text-white data-[state=on]:border-emerald-600',
      inactive: 'bg-emerald-50 dark:bg-emerald-950/50 text-emerald-700 dark:text-emerald-300 border-emerald-200 dark:border-emerald-800/60 hover:bg-emerald-100 dark:hover:bg-emerald-900/60 data-[state=off]:bg-emerald-50 data-[state=off]:text-emerald-700',
      countActive: 'bg-white/25 text-white font-extrabold',
      countInactive: 'bg-emerald-500/20 text-emerald-800 dark:text-emerald-200 font-extrabold'
    },
    messenger: {
      active: 'bg-purple-600 text-white border-purple-600 shadow-2xs hover:bg-purple-700 data-[state=on]:bg-purple-600 data-[state=on]:text-white data-[state=on]:border-purple-600',
      inactive: 'bg-purple-50 dark:bg-purple-950/50 text-purple-700 dark:text-purple-300 border-purple-200 dark:border-purple-800/60 hover:bg-purple-100 dark:hover:bg-purple-900/60 data-[state=off]:bg-purple-50 data-[state=off]:text-purple-700',
      countActive: 'bg-white/25 text-white font-extrabold',
      countInactive: 'bg-purple-500/20 text-purple-800 dark:text-purple-200 font-extrabold'
    },
    email: {
      active: 'bg-rose-600 text-white border-rose-600 shadow-2xs hover:bg-rose-700 data-[state=on]:bg-rose-600 data-[state=on]:text-white data-[state=on]:border-rose-600',
      inactive: 'bg-rose-50 dark:bg-rose-950/50 text-rose-700 dark:text-rose-300 border-rose-200 dark:border-rose-800/60 hover:bg-rose-100 dark:hover:bg-rose-900/60 data-[state=off]:bg-rose-50 data-[state=off]:text-rose-700',
      countActive: 'bg-white/25 text-white font-extrabold',
      countInactive: 'bg-rose-500/20 text-rose-800 dark:text-rose-200 font-extrabold'
    }
  };

  let toggleValue = $derived([...activeChannels]);

  function handleValueChange(nextValue: string[]) {
    const nextChannels = nextValue as Channel[];
    if (nextChannels.length === 0 && activeChannels.length > 0) {
      toggleValue = [...activeChannels];
      return;
    }

    const changedChannel = channels
      .map((channel) => channel.channel)
      .find(
        (channel) =>
          activeChannels.includes(channel) !== nextChannels.includes(channel)
      );
    if (changedChannel) onToggle(changedChannel);
  }
</script>

<div class="flex flex-col gap-2">
  {#if channels.length > 0}
    <ToggleGroup.Root
      type="multiple"
      bind:value={toggleValue}
      onValueChange={handleValueChange}
      class="flex flex-wrap items-center gap-1.5"
    >
      {#each channels as ch (ch.channel)}
        {@const Icon = channelIcons[ch.channel] || MessageSquare}
        {@const isActive = activeChannels.includes(ch.channel)}
        {@const style = channelStyles[ch.channel] || channelStyles.zalo}
        <ToggleGroup.Item
          value={ch.channel}
          aria-label={ch.channel}
          aria-pressed={isActive ? 'true' : 'false'}
          class="flex items-center gap-1.5 text-xs font-semibold px-3 py-1 rounded-full border transition-all duration-150 {isActive ? style.active : style.inactive}"
        >
          <Icon class="w-3.5 h-3.5" />
          <span>{channelLabels[ch.channel] || ch.channel}</span>
          <span class="ml-1 rounded-full px-2 py-0.5 text-[11px] font-bold shrink-0 {isActive ? style.countActive : style.countInactive}">
            {ch.messageCount}
          </span>
        </ToggleGroup.Item>
      {/each}
    </ToggleGroup.Root>
  {/if}

  {#if activeChannels.length === 0}
    <p class="text-xs text-muted-foreground italic px-1">
      No channels currently visible
    </p>
  {/if}
</div>
