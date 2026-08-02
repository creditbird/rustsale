<script lang="ts">
  import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
  import type { Channel, ChannelBadgeSummary } from '$lib/services/conversationService';
  import MessageSquare from 'lucide-svelte/icons/message-square';
  import MessageCircle from 'lucide-svelte/icons/message-circle';
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
    whatsapp: Phone,
    messenger: MessageCircle,
    email: Mail
  };

  const channelLabels: Record<Channel, string> = {
    zalo: 'Zalo',
    whatsapp: 'WhatsApp',
    messenger: 'Messenger',
    email: 'Email'
  };

  let toggleValue = $state<Channel[]>([]);

  $effect(() => {
    toggleValue = [...activeChannels];
  });

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
        <ToggleGroup.Item
          value={ch.channel}
          aria-label={ch.channel}
          aria-pressed={isActive ? 'true' : 'false'}
          class="flex items-center gap-1.5 text-xs font-medium px-2.5 py-1 rounded-md border border-border transition-colors hover:bg-accent data-[state=on]:bg-accent data-[state=on]:text-accent-foreground"
        >
          <Icon class="w-3.5 h-3.5" />
          <span>{channelLabels[ch.channel] || ch.channel}</span>
          <span class="ml-1 rounded-full bg-muted-foreground/20 px-1.5 py-0.5 text-[10px] font-semibold">
            {ch.messageCount}
          </span>
        </ToggleGroup.Item>
      {/each}
    </ToggleGroup.Root>
  {/if}

  {#if activeChannels.length === 0}
    <p class="text-xs text-muted-foreground italic px-1">
      Không có kênh nào đang được hiển thị
    </p>
  {/if}
</div>
