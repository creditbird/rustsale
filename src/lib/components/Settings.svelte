<script lang="ts">
  import ZaloAccountsSettings from './settings/ZaloAccountsSettings.svelte';
  import EmailAccountsSettings from './settings/EmailAccountsSettings.svelte';
  import FacebookAccountsSettings from './settings/FacebookAccountsSettings.svelte';
  import WhatsAppAccountsSettings from './settings/WhatsAppAccountsSettings.svelte';
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
  let whatsappSettingsRef = $state<{ triggerQrLogin?: () => void } | null>(null);

  export function openQrLogin() {
    activeChannel = 'zalo';
    tick().then(() => {
      zaloSettingsRef?.triggerQrLogin?.();
    });
  }

  export function openFacebookTab() {
    activeChannel = 'facebook';
  }

  export function openWhatsAppTab() {
    activeChannel = 'whatsapp';
  }

  export function openWhatsAppQrLogin() {
    activeChannel = 'whatsapp';
    tick().then(() => {
      whatsappSettingsRef?.triggerQrLogin?.();
    });
  }
</script>

<div class="flex flex-col gap-6 pb-8">
  <!-- Channel & Account Settings Section -->
  <Card.Root class="overflow-hidden border-border/80">
    <Card.Header class="border-b border-border/60 bg-muted/10 pb-4">
      <div class="flex items-start gap-3">
        <div class="rounded-xl bg-primary/10 p-2.5 text-primary shrink-0">
          <Settings2 class="size-5" />
        </div>
        <div>
          <Card.Title class="text-base font-bold text-foreground">
            {m.settings_channels_title()}
          </Card.Title>
          <Card.Description class="mt-0.5 text-xs text-muted-foreground">
            {m.settings_channels_desc()}
          </Card.Description>
        </div>
      </div>
    </Card.Header>

    <Card.Content class="p-5 sm:p-6 flex flex-col gap-6">
      <Tabs.Root bind:value={activeChannel} class="flex flex-col gap-5">
        <Tabs.List class="grid h-auto w-full grid-cols-2 gap-1.5 rounded-xl bg-muted/50 p-1.5 sm:grid-cols-4" variant="default">
          <Tabs.Trigger value="zalo" class="h-10 gap-2 rounded-lg text-xs font-semibold data-[state=active]:bg-background data-[state=active]:shadow-xs">
            <MessageCircleMore class="size-4 text-emerald-600 shrink-0" /> Zalo
          </Tabs.Trigger>
          <Tabs.Trigger value="email" class="h-10 gap-2 rounded-lg text-xs font-semibold data-[state=active]:bg-background data-[state=active]:shadow-xs">
            <Mail class="size-4 text-sky-600 shrink-0" /> Email
          </Tabs.Trigger>
          <Tabs.Trigger value="facebook" class="h-10 gap-2 rounded-lg text-xs font-semibold data-[state=active]:bg-background data-[state=active]:shadow-xs">
            <MessagesSquare class="size-4 text-blue-600 shrink-0" /> Facebook
          </Tabs.Trigger>
          <Tabs.Trigger value="whatsapp" class="h-10 gap-2 rounded-lg text-xs font-semibold data-[state=active]:bg-background data-[state=active]:shadow-xs">
            <MessageCircleMore class="size-4 text-emerald-600 shrink-0" /> WhatsApp
          </Tabs.Trigger>
        </Tabs.List>

        <div class="pt-1">
          <Tabs.Content value="zalo"><ZaloAccountsSettings bind:this={zaloSettingsRef} /></Tabs.Content>
          <Tabs.Content value="email"><EmailAccountsSettings /></Tabs.Content>
          <Tabs.Content value="facebook"><FacebookAccountsSettings {onNavigateToHub} /></Tabs.Content>
          <Tabs.Content value="whatsapp"><WhatsAppAccountsSettings bind:this={whatsappSettingsRef} /></Tabs.Content>
        </div>
      </Tabs.Root>
    </Card.Content>
  </Card.Root>

  <!-- Notification & System Tray Section -->
  <NotificationSettings />

  <!-- Language & Localization Section -->
  <LanguageSettings />

  <!-- App Update Section -->
  <AppUpdateSettings />
</div>
