<script lang="ts">
  import * as Card from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import {
    Bell,
    BellOff,
    Volume2,
    VolumeX,
    EyeOff,
    Eye,
    Minimize2,
    Send,
    ShieldAlert,
    CheckCircle2,
    Loader2
  } from 'lucide-svelte';
  import { toast } from 'svelte-sonner';
  import { onMount } from 'svelte';
  import { m } from '$lib/i18n';
  import {
    getNotificationSettings,
    updateNotificationSettings,
    getTrayState,
    setCloseToTray,
    setTrayDnd,
    testDesktopNotification,
    subscribeTrayDndChanged,
    type NotificationSettings as SettingsType,
    type TrayConfig
  } from '$lib/services/desktopNotificationService';
  import { isAutostartEnabled, setAutostart } from '$lib/services/autostartService';

  let settings = $state<SettingsType>({
    enabled: true,
    sound_enabled: true,
    privacy_mode: false,
    mute_groups: false
  });

  let tray = $state<TrayConfig>({
    close_to_tray: true,
    dnd_enabled: false
  });

  let autostartEnabled = $state(false);
  let loading = $state(true);
  let testing = $state(false);

  onMount(() => {
    let active = true;
    let unsubDnd: (() => void) | null = null;
    let dndEventReceived = false;

    (async () => {
      try {
        // 1. Establish subscription first and ensure listener is active
        try {
          const unsub = await subscribeTrayDndChanged((enabled) => {
            if (active) {
              dndEventReceived = true;
              tray.dnd_enabled = enabled;
            }
          });
          if (active) {
            unsubDnd = unsub;
          } else {
            unsub();
            return;
          }
        } catch (subErr) {
          console.warn('Failed to subscribe to tray DND updates:', subErr);
        }

        // 2. Read snapshots only after listener is confirmed active
        const [s, t, auto] = await Promise.all([
          getNotificationSettings(),
          getTrayState(),
          isAutostartEnabled()
        ]);
        if (active) {
          if (s) settings = s;
          if (t) {
            tray.close_to_tray = t.close_to_tray;
            if (!dndEventReceived) {
              tray.dnd_enabled = t.dnd_enabled;
            }
          }
          autostartEnabled = auto;
        }
      } catch (err) {
        console.warn('Error loading notification settings:', err);
      } finally {
        if (active) {
          loading = false;
        }
      }
    })();

    return () => {
      active = false;
      if (unsubDnd) {
        unsubDnd();
      }
    };
  });

  async function handleToggleEnabled() {
    const next = !settings.enabled;
    settings.enabled = next;
    try {
      await updateNotificationSettings({ ...settings });
    } catch (err) {
      settings.enabled = !next;
      toast.error(m.settings_notif_err_update_notif());
    }
  }

  async function handleToggleCloseToTray() {
    const next = !tray.close_to_tray;
    tray.close_to_tray = next;
    try {
      await setCloseToTray(next);
    } catch (err) {
      tray.close_to_tray = !next;
      toast.error(m.settings_notif_err_update_tray());
    }
  }

  async function handleToggleAutostart() {
    const next = !autostartEnabled;
    autostartEnabled = next;
    try {
      await setAutostart(next);
    } catch (err) {
      autostartEnabled = !next;
      toast.error(m.settings_notif_err_update_autostart());
    }
  }

  async function handleTogglePrivacyMode() {
    const next = !settings.privacy_mode;
    settings.privacy_mode = next;
    try {
      await updateNotificationSettings({ ...settings });
    } catch (err) {
      settings.privacy_mode = !next;
      toast.error(m.settings_notif_err_update_privacy());
    }
  }

  async function handleToggleSound() {
    const next = !settings.sound_enabled;
    settings.sound_enabled = next;
    try {
      await updateNotificationSettings({ ...settings });
    } catch (err) {
      settings.sound_enabled = !next;
      toast.error(m.settings_notif_err_update_sound());
    }
  }

  async function handleToggleMuteGroups() {
    const next = !settings.mute_groups;
    settings.mute_groups = next;
    try {
      await updateNotificationSettings({ ...settings });
    } catch (err) {
      settings.mute_groups = !next;
      toast.error(m.settings_notif_err_update_mute_groups());
    }
  }

  async function handleToggleDnd() {
    const next = !tray.dnd_enabled;
    tray.dnd_enabled = next;
    try {
      await setTrayDnd(next);
    } catch (err) {
      tray.dnd_enabled = !next;
      toast.error(m.settings_notif_err_update_dnd());
    }
  }

  async function handleTestNotification() {
    testing = true;
    try {
      await testDesktopNotification();
      toast.success(m.settings_notif_test_success());
    } catch (err) {
      console.error('Test notification failed:', err);
      toast.error(m.settings_notif_err_test());
    } finally {
      testing = false;
    }
  }
</script>

<div class="flex flex-col gap-5">
  <Card.Root class="overflow-hidden border-border/80">
    <Card.Header class="bg-muted/10 border-b border-border/60 pb-4">
      <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div class="flex items-start gap-3">
          <div class="rounded-xl bg-primary/10 p-2.5 text-primary shrink-0">
            <Bell class="size-5" />
          </div>
          <div>
            <Card.Title class="text-base font-bold text-foreground">
              {m.settings_notifications_title()}
            </Card.Title>
            <Card.Description class="text-xs text-muted-foreground mt-0.5">
              {m.settings_notifications_desc()}
            </Card.Description>
          </div>
        </div>

        <div>
          <Button
            variant="outline"
            size="sm"
            class="h-8 gap-1.5 text-xs font-medium"
            disabled={testing || !settings.enabled}
            onclick={handleTestNotification}
          >
            {#if testing}
              <Loader2 class="size-3.5 animate-spin" />
            {:else}
              <Send class="size-3.5" />
            {/if}
            {m.settings_notif_test_btn()}
          </Button>
        </div>
      </div>
    </Card.Header>

    <Card.Content class="p-6 space-y-4">
      {#if tray.dnd_enabled}
        <div class="rounded-lg border border-amber-500/30 bg-amber-500/10 p-3.5 flex items-center justify-between gap-3 text-amber-600 dark:text-amber-400 text-xs">
          <div class="flex items-center gap-2">
            <ShieldAlert class="size-4 shrink-0" />
            <span>{m.settings_notif_dnd_active()}</span>
          </div>
          <Button variant="outline" size="sm" class="h-7 text-xs border-amber-500/30 hover:bg-amber-500/20" onclick={handleToggleDnd}>
            {m.settings_notif_dnd_disable_btn()}
          </Button>
        </div>
      {/if}

      <!-- Toggle rows -->
      <div class="divide-y divide-border/60">
        <!-- 1. Enable Desktop Notifications -->
        <div class="py-3.5 flex items-center justify-between gap-4 first:pt-0">
          <div class="space-y-0.5">
            <div class="flex items-center gap-2">
              <span class="text-sm font-medium text-foreground">
                {m.settings_notif_desktop_enabled()}
              </span>
              {#if settings.enabled}
                <Badge variant="outline" class="text-[10px] bg-emerald-500/10 text-emerald-600 border-emerald-500/30">
                  {m.settings_notif_status_active()}
                </Badge>
              {:else}
                <Badge variant="outline" class="text-[10px] bg-muted text-muted-foreground">
                  {m.settings_notif_status_disabled()}
                </Badge>
              {/if}
            </div>
            <p class="text-xs text-muted-foreground">
              {m.settings_notif_desktop_desc()}
            </p>
          </div>
          <button
            type="button"
            role="switch"
            aria-label={m.settings_notif_desktop_enabled()}
            aria-checked={settings.enabled}
            onclick={handleToggleEnabled}
            class="relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-hidden focus:ring-2 focus:ring-primary focus:ring-offset-2 {settings.enabled ? 'bg-primary' : 'bg-muted'}"
          >
            <span
              class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow-sm ring-0 transition duration-200 ease-in-out {settings.enabled ? 'translate-x-5' : 'translate-x-0'}"
            ></span>
          </button>
        </div>

        <!-- 2. Close to Tray -->
        <div class="py-3.5 flex items-center justify-between gap-4">
          <div class="space-y-0.5">
            <div class="flex items-center gap-2">
              <span class="text-sm font-medium text-foreground">
                {m.settings_notif_close_to_tray()}
              </span>
              {#if tray.close_to_tray}
                <Badge variant="outline" class="text-[10px] bg-primary/10 text-primary border-primary/30">
                  {m.settings_notif_status_background()}
                </Badge>
              {/if}
            </div>
            <p class="text-xs text-muted-foreground">
              {m.settings_notif_close_to_tray_desc()}
            </p>
          </div>
          <button
            type="button"
            role="switch"
            aria-label={m.settings_notif_close_to_tray()}
            aria-checked={tray.close_to_tray}
            onclick={handleToggleCloseToTray}
            class="relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-hidden focus:ring-2 focus:ring-primary focus:ring-offset-2 {tray.close_to_tray ? 'bg-primary' : 'bg-muted'}"
          >
            <span
              class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow-sm ring-0 transition duration-200 ease-in-out {tray.close_to_tray ? 'translate-x-5' : 'translate-x-0'}"
            ></span>
          </button>
        </div>

        <!-- 3. Launch at System Startup -->
        <div class="py-3.5 flex items-center justify-between gap-4">
          <div class="space-y-0.5">
            <div class="flex items-center gap-2">
              <span class="text-sm font-medium text-foreground">
                {m.settings_notif_autostart()}
              </span>
              {#if autostartEnabled}
                <Badge variant="outline" class="text-[10px] bg-primary/10 text-primary border-primary/30">
                  {m.settings_notif_status_autostart()}
                </Badge>
              {/if}
            </div>
            <p class="text-xs text-muted-foreground">
              {m.settings_notif_autostart_desc()}
            </p>
          </div>
          <button
            type="button"
            role="switch"
            aria-label={m.settings_notif_autostart()}
            aria-checked={autostartEnabled}
            onclick={handleToggleAutostart}
            class="relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-hidden focus:ring-2 focus:ring-primary focus:ring-offset-2 {autostartEnabled ? 'bg-primary' : 'bg-muted'}"
          >
            <span
              class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow-sm ring-0 transition duration-200 ease-in-out {autostartEnabled ? 'translate-x-5' : 'translate-x-0'}"
            ></span>
          </button>
        </div>

        <!-- 4. Notification Sound -->
        <div class="py-3.5 flex items-center justify-between gap-4">
          <div class="space-y-0.5">
            <div class="flex items-center gap-2">
              <span class="text-sm font-medium text-foreground">
                {m.settings_notif_sound()}
              </span>
            </div>
            <p class="text-xs text-muted-foreground">
              {m.settings_notif_sound_desc()}
            </p>
          </div>
          <button
            type="button"
            role="switch"
            aria-label={m.settings_notif_sound()}
            aria-checked={settings.sound_enabled}
            onclick={handleToggleSound}
            class="relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-hidden focus:ring-2 focus:ring-primary focus:ring-offset-2 {settings.sound_enabled ? 'bg-primary' : 'bg-muted'}"
          >
            <span
              class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow-sm ring-0 transition duration-200 ease-in-out {settings.sound_enabled ? 'translate-x-5' : 'translate-x-0'}"
            ></span>
          </button>
        </div>

        <!-- 4. Privacy Mode -->
        <div class="py-3.5 flex items-center justify-between gap-4">
          <div class="space-y-0.5">
            <div class="flex items-center gap-2">
              <span class="text-sm font-medium text-foreground">
                {m.settings_notif_privacy_mode()}
              </span>
            </div>
            <p class="text-xs text-muted-foreground">
              {m.settings_notif_privacy_mode_desc()}
            </p>
          </div>
          <button
            type="button"
            role="switch"
            aria-label={m.settings_notif_privacy_mode()}
            aria-checked={settings.privacy_mode}
            onclick={handleTogglePrivacyMode}
            class="relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-hidden focus:ring-2 focus:ring-primary focus:ring-offset-2 {settings.privacy_mode ? 'bg-primary' : 'bg-muted'}"
          >
            <span
              class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow-sm ring-0 transition duration-200 ease-in-out {settings.privacy_mode ? 'translate-x-5' : 'translate-x-0'}"
            ></span>
          </button>
        </div>

        <!-- 5. Mute Groups -->
        <div class="py-3.5 flex items-center justify-between gap-4 last:pb-0">
          <div class="space-y-0.5">
            <div class="flex items-center gap-2">
              <span class="text-sm font-medium text-foreground">
                {m.settings_notif_mute_groups()}
              </span>
            </div>
            <p class="text-xs text-muted-foreground">
              {m.settings_notif_mute_groups_desc()}
            </p>
          </div>
          <button
            type="button"
            role="switch"
            aria-label={m.settings_notif_mute_groups()}
            aria-checked={settings.mute_groups}
            onclick={handleToggleMuteGroups}
            class="relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-hidden focus:ring-2 focus:ring-primary focus:ring-offset-2 {settings.mute_groups ? 'bg-primary' : 'bg-muted'}"
          >
            <span
              class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow-sm ring-0 transition duration-200 ease-in-out {settings.mute_groups ? 'translate-x-5' : 'translate-x-0'}"
            ></span>
          </button>
        </div>
      </div>
    </Card.Content>
  </Card.Root>
</div>
