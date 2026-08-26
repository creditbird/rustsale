<script lang="ts">
  import ZaloAccountsSettings from './settings/ZaloAccountsSettings.svelte';
  import EmailAccountsSettings from './settings/EmailAccountsSettings.svelte';
  import FacebookAccountsSettings from './settings/FacebookAccountsSettings.svelte';
  import AppUpdateSettings from './settings/AppUpdateSettings.svelte';
  import NotificationSettings from './settings/NotificationSettings.svelte';
  import LanguageSettings from './settings/LanguageSettings.svelte';
  import * as Card from '$lib/components/ui/card';
  import * as Tabs from '$lib/components/ui/tabs';
  import { Badge } from '$lib/components/ui/badge';
  import { Mail, MessageCircleMore, MessagesSquare, Settings2 } from 'lucide-svelte';
  import { m } from '$lib/i18n';
  import { tick } from 'svelte';

  let { onNavigateToHub }: { onNavigateToHub?: () => void } = $props();

  let activeChannel = $state('zalo');
  let zaloSettingsRef = $state<{ triggerQrLogin?: () => void } | null>(null);

  export function openQrLogin() {
    activeChannel = 'zalo';
    tick().then(() => {
      zaloSettingsRef?.triggerQrLogin?.();
    });
  }

  export function openFacebookTab() {
    activeChannel = 'facebook';
  }
</script>

<div class="flex h-full min-h-0 flex-col gap-5 overflow-y-auto pr-1">
  <Card.Root class="shrink-0 overflow-hidden">
    <Card.Header class="border-b border-border/70 bg-muted/20">
      <div class="flex items-start gap-3">
        <div class="rounded-xl bg-primary/10 p-2.5 text-primary"><Settings2 class="size-5" /></div>
        <div>
          <Card.Title class="text-base">{m.settings_channels_title()}</Card.Title>
          <Card.Description class="mt-1 max-w-3xl text-xs">
            {m.settings_channels_desc()}
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
      <Tabs.Trigger value="facebook" class="min-h-11 gap-2">
        <MessagesSquare class="size-4 text-blue-600" /> Facebook
      </Tabs.Trigger>
      <Tabs.Trigger value="whatsapp" class="min-h-11 gap-2" disabled>
        <MessageCircleMore class="size-4 text-emerald-600" /> WhatsApp <Badge variant="outline" class="text-[9px]">{m.common_coming_soon()}</Badge>
      </Tabs.Trigger>
    </Tabs.List>

    <Tabs.Content value="zalo" class="min-h-0"><ZaloAccountsSettings bind:this={zaloSettingsRef} /></Tabs.Content>
    <Tabs.Content value="email" class="min-h-0"><EmailAccountsSettings /></Tabs.Content>
    <Tabs.Content value="facebook" class="min-h-0"><FacebookAccountsSettings {onNavigateToHub} /></Tabs.Content>
  </Tabs.Root>

  <div>
    <NotificationSettings />
  </div>

  <div>
    <LanguageSettings />
  </div>

  <div class="pt-2 pb-6">
    <AppUpdateSettings />
  </div>
</div>
