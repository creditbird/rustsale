<script lang="ts">
  import {
    LayoutDashboard,
    MapPin,
    Users,
    Tags,
    MessagesSquare,
    MessageSquare,
    Mail,
    Kanban,
    UserCheck,
    Settings2,
    Sparkles,
    ChevronRight,
    Share2
  } from 'lucide-svelte';
  import { updaterService, type UpdaterState } from '$lib/services/updaterService';
  import { getCurrentUser, verifyDeviceAndSubscription, type DeviceVerificationResult } from '$lib/services/licenseService';
  import { getSupabaseClient } from '$lib/services/supabaseClient';
  import { onMount, onDestroy } from 'svelte';
  import type { User as SupabaseUser } from '@supabase/supabase-js';

  let { activeTab = $bindable(), onOpenLicense }: { activeTab: string; onOpenLicense?: () => void } = $props();

  let updaterState = $state<UpdaterState>(updaterService.getState());
  let currentUser = $state<SupabaseUser | null>(null);
  let verification = $state<DeviceVerificationResult | null>(null);
  let unsubscribeUpdater: (() => void) | null = null;
  let authListenerSub: { unsubscribe: () => void } | null = null;

  function getInitials(emailOrName?: string | null): string {
    if (!emailOrName) return 'RS';
    const name = emailOrName.split('@')[0];
    const parts = name.split(/[._-]/).filter(Boolean);
    if (parts.length >= 2) {
      return (parts[0][0] + parts[1][0]).toUpperCase();
    }
    return name.slice(0, 2).toUpperCase();
  }

  async function loadUserAndLicense() {
    try {
      const user = await getCurrentUser();
      currentUser = user;
      if (user) {
        verification = await verifyDeviceAndSubscription();
      } else {
        verification = null;
      }
    } catch (err) {
      console.warn('Failed to load user info in sidebar:', err);
    }
  }

  onMount(() => {
    unsubscribeUpdater = updaterService.subscribe((s) => {
      updaterState = s;
    });

    loadUserAndLicense();

    try {
      const supabase = getSupabaseClient();
      const { data } = supabase.auth.onAuthStateChange(async (_event, session) => {
        currentUser = session?.user ?? null;
        if (currentUser) {
          verification = await verifyDeviceAndSubscription();
        } else {
          verification = null;
        }
      });
      authListenerSub = data?.subscription ?? null;
    } catch (err) {
      console.warn('Failed to setup auth listener in sidebar:', err);
    }
  });

  onDestroy(() => {
    if (unsubscribeUpdater) {
      unsubscribeUpdater();
    }
    if (authListenerSub) {
      authListenerSub.unsubscribe();
    }
  });

  import { m } from '$lib/i18n';

  const menuItems = $derived([
    { id: 'dashboard', label: m.nav_dashboard(), icon: LayoutDashboard },
    { id: 'scraper', label: m.nav_scraper(), icon: MapPin },
    { id: 'leads', label: m.nav_leads(), icon: Users },
    { id: 'categories', label: m.nav_categories(), icon: Tags },
    { id: 'conversation', label: m.nav_conversation(), icon: MessagesSquare },
    { id: 'facebook_hub', label: m.nav_facebook_hub(), icon: Share2 },
    { id: 'outreach', label: m.nav_outreach(), icon: MessageSquare },
    { id: 'email_outreach', label: m.nav_email_outreach(), icon: Mail },
    { id: 'pipeline', label: m.nav_pipeline(), icon: Kanban },
    { id: 'contacts', label: m.nav_contacts(), icon: UserCheck },
    { id: 'settings', label: m.nav_settings(), icon: Settings2 }
  ]);
</script>

<aside class="w-64 h-screen bg-sidebar border-r border-sidebar-border flex flex-col text-sidebar-foreground select-none shrink-0">
  <!-- Brand Header -->
  <div class="h-16 px-4 border-b border-sidebar-border flex items-center justify-between gap-3">
    <div class="flex items-center gap-3 min-w-0">
      <img src="/logo.png" alt="RustSale Logo" class="size-10 rounded-lg object-contain shrink-0 shadow-2xs" />
      <div class="flex flex-col min-w-0">
        <div class="flex items-center gap-1.5">
          <span class="font-bold text-base tracking-tight text-foreground">RustSale</span>
          <span class="text-[10px] text-muted-foreground font-mono">v{updaterState.currentVersion}</span>
        </div>
        <span class="text-[11px] text-muted-foreground font-medium uppercase tracking-wider truncate">CRM & Outreach Pro</span>
      </div>
    </div>

    {#if updaterState.status === 'available'}
      <button
        type="button"
        onclick={() => updaterService.openModal()}
        title="New update available: v{updaterState.availableVersion}"
        class="size-7 rounded-lg bg-primary/15 text-primary hover:bg-primary/25 border border-primary/30 flex items-center justify-center transition-all animate-pulse shrink-0 cursor-pointer"
      >
        <Sparkles class="size-3.5" />
      </button>
    {/if}
  </div>

  <!-- Navigation Links -->
  <nav class="flex-1 px-3 py-4 flex flex-col gap-1 overflow-y-auto">
    {#each menuItems as item (item.id)}
      {@const Icon = item.icon}
      {@const isActive = activeTab === item.id}
      <button 
        type="button"
        class="w-full h-10 px-3 rounded-md flex items-center gap-3 text-sm font-medium transition-all duration-150 relative group cursor-pointer {isActive ? 'bg-sidebar-accent text-sidebar-accent-foreground font-semibold shadow-xs' : 'text-muted-foreground hover:bg-sidebar-accent/50 hover:text-foreground'}" 
        onclick={() => activeTab = item.id}
      >
        <Icon class="size-4 transition-colors {isActive ? 'text-primary' : 'text-muted-foreground group-hover:text-foreground'}" />
        <span class="truncate">{item.label}</span>
        {#if isActive}
          <div class="absolute left-0 top-2 bottom-2 w-1 bg-primary rounded-r-full"></div>
        {/if}
      </button>
    {/each}
  </nav>

  <!-- User / Status Footer (Interactive Account Card) -->
  <div class="p-2.5 border-t border-sidebar-border bg-sidebar-accent/15">
    <button
      type="button"
      onclick={onOpenLicense}
      class="w-full p-2 rounded-lg flex items-center justify-between gap-2.5 text-left transition-all duration-150 group cursor-pointer hover:bg-sidebar-accent/70 hover:shadow-2xs border border-transparent hover:border-sidebar-border"
      title={m.settings_license_title()}
    >
      <div class="flex items-center gap-2.5 overflow-hidden min-w-0">
        <div class="size-8 rounded-full bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20 flex items-center justify-center text-xs font-bold shrink-0 transition-transform group-hover:scale-105">
          {getInitials(currentUser?.email)}
        </div>
        <div class="flex flex-col overflow-hidden min-w-0">
          <span class="text-xs font-semibold truncate text-foreground group-hover:text-primary transition-colors">
            {currentUser?.email || 'Admin Workspace'}
          </span>
          <span class="text-[10px] font-medium flex items-center gap-1.5 truncate {currentUser ? 'text-emerald-600 dark:text-emerald-400' : 'text-muted-foreground'}">
            <span class="size-1.5 rounded-full {currentUser ? 'bg-emerald-500 animate-pulse' : 'bg-emerald-500/60'} shrink-0"></span>
            <span class="truncate">
              {#if currentUser}
                {verification?.plan_id ? `${verification.plan_id.toUpperCase()} Plan • HWID Active` : 'HWID Active'}
              {:else}
                HWID Active
              {/if}
            </span>
          </span>
        </div>
      </div>

      <div class="text-muted-foreground/60 group-hover:text-foreground transition-colors shrink-0 pr-0.5">
        <ChevronRight class="size-4 transition-transform group-hover:translate-x-0.5" />
      </div>
    </button>
  </div>
</aside>

