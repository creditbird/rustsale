<script lang="ts">
  import ZaloAccountsSettings from './settings/ZaloAccountsSettings.svelte';
  import EmailAccountsSettings from './settings/EmailAccountsSettings.svelte';
  import * as Card from '$lib/components/ui/card';
  import * as Tabs from '$lib/components/ui/tabs';
  import { Badge } from '$lib/components/ui/badge';
  import { Mail, MessageCircleMore, MessagesSquare, Settings2 } from 'lucide-svelte';

  let activeChannel = $state('zalo');
</script>

<div class="flex h-full min-h-0 flex-col gap-5 overflow-y-auto">
  <Card.Root class="shrink-0 overflow-hidden">
    <Card.Header class="border-b border-border/70 bg-muted/20">
      <div class="flex items-start gap-3">
        <div class="rounded-xl bg-primary/10 p-2.5 text-primary"><Settings2 class="size-5" /></div>
        <div>
          <Card.Title class="text-base">Channels & Accounts</Card.Title>
          <Card.Description class="mt-1 max-w-3xl text-xs">
            Connect provider accounts once, then reuse them safely across Conversations and Outreach.
          </Card.Description>
        </div>
      </div>
    </Card.Header>
  </Card.Root>

  <Tabs.Root bind:value={activeChannel} class="min-h-0 gap-4">
    <Tabs.List class="grid h-auto w-full grid-cols-2 gap-1 lg:grid-cols-4" variant="default">
      <Tabs.Trigger value="zalo" class="min-h-11 gap-2">
        <MessageCircleMore class="size-4 text-emerald-600" /> Zalo
      </Tabs.Trigger>
      <Tabs.Trigger value="email" class="min-h-11 gap-2">
        <Mail class="size-4 text-sky-600" /> Email
      </Tabs.Trigger>
      <Tabs.Trigger value="facebook" class="min-h-11 gap-2" disabled>
        <MessagesSquare class="size-4 text-blue-600" /> Facebook <Badge variant="outline" class="text-[9px]">Coming soon</Badge>
      </Tabs.Trigger>
      <Tabs.Trigger value="whatsapp" class="min-h-11 gap-2" disabled>
        <MessageCircleMore class="size-4 text-emerald-600" /> WhatsApp <Badge variant="outline" class="text-[9px]">Coming soon</Badge>
      </Tabs.Trigger>
    </Tabs.List>

    <Tabs.Content value="zalo" class="min-h-0"><ZaloAccountsSettings /></Tabs.Content>
    <Tabs.Content value="email" class="min-h-0"><EmailAccountsSettings /></Tabs.Content>
  </Tabs.Root>
</div>
