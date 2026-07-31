<script lang="ts">
  import { onMount } from 'svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import Dashboard from '$lib/components/Dashboard.svelte';
  import LeadsList from '$lib/components/LeadsList.svelte';
  import KanbanBoard from '$lib/components/KanbanBoard.svelte';
  import GoogleMapsScraper from '$lib/components/GoogleMapsScraper.svelte';
  import ZaloOutreach from '$lib/components/ZaloOutreach.svelte';
  import EmailOutreach from '$lib/components/EmailOutreach.svelte';
  import CategoryManager from '$lib/components/CategoryManager.svelte';
  import * as crmService from '$lib/services/crmService';
  import type { Lead } from '$lib/services/crmService';
  import { AlertCircle, Search, Bell } from 'lucide-svelte';

  let activeTab = $state('dashboard');
  let leads = $state<Lead[]>([]);
  let errorMsg = $state<string | null>(null);

  const tabTitles: Record<string, { title: string; subtitle: string }> = {
    dashboard: { title: 'Dashboard System', subtitle: 'Overview KPI statistics and recent lead activity' },
    scraper: { title: 'Google Maps Scraper', subtitle: 'Extract local business lead data dynamically across regions' },
    leads: { title: 'Lead Management', subtitle: 'Potential lead contacts, acquisition details and verification status' },
    categories: { title: 'Category Manager', subtitle: 'Organize, tag and structure lead segments' },
    outreach: { title: 'Zalo Outreach Manager', subtitle: 'Authenticate sessions, customize templates, and launch Zalo flows' },
    email_outreach: { title: 'Email Outreach Manager', subtitle: 'Configure SMTP servers, HTML templates, and targeted email broadcasts' },
    pipeline: { title: 'Deal Pipeline', subtitle: 'Kanban view for deal stages and sales conversion workflow' },
    contacts: { title: 'Converted Contacts', subtitle: 'Official converted customer database' }
  };

  let isStale = $state(false);
  let hasLoaded = $state(false);

  let reloadTimer: ReturnType<typeof setTimeout> | null = null;
  function debouncedLoadLeads() {
    if (reloadTimer) clearTimeout(reloadTimer);
    reloadTimer = setTimeout(() => {
      reloadTimer = null;
      if (activeTab === 'leads' || activeTab === 'contacts') {
        loadLeads();
      } else {
        isStale = true;
      }
    }, 300);
  }

  async function loadLeads() {
    try {
      leads = await crmService.getLeads();
      errorMsg = null;
      hasLoaded = true;
      isStale = false;
    } catch (err) {
      hasLoaded = false;
      errorMsg = err instanceof Error ? err.message : String(err);
    }
  }

  $effect(() => {
    if (activeTab === 'leads' || activeTab === 'contacts') {
      if (isStale || !hasLoaded) {
        isStale = false;
        loadLeads();
      }
    } else {
      if (reloadTimer) {
        clearTimeout(reloadTimer);
        reloadTimer = null;
        isStale = true;
      }
    }
  });

  onMount(() => {
    const unsubBridge = crmService.initScraperEventBridge();
    const unsub = crmService.onLeadsChanged(() => {
      if (activeTab === 'leads' || activeTab === 'contacts') {
        debouncedLoadLeads();
      } else {
        isStale = true;
      }
    });
    return () => {
      unsub();
      unsubBridge();
      if (reloadTimer) clearTimeout(reloadTimer);
    };
  });
</script>

<div class="flex w-screen h-screen overflow-hidden bg-background text-foreground antialiased font-sans">
  <!-- Left Sidebar Navigation -->
  <Sidebar bind:activeTab />

  <!-- Main View Container -->
  <main class="flex-1 flex flex-col h-full overflow-hidden bg-muted/20">
    <!-- Top Header Bar -->
    <header class="h-16 px-6 border-b border-border bg-background flex items-center justify-between shrink-0 shadow-2xs">
      <div>
        <h1 class="text-base font-bold text-foreground tracking-tight">
          {tabTitles[activeTab]?.title || 'RustSale Pro'}
        </h1>
        <p class="text-xs text-muted-foreground">
          {tabTitles[activeTab]?.subtitle || 'Enterprise CRM Workspace'}
        </p>
      </div>

      <div class="flex items-center gap-3">
        <!-- Quick Search -->
        <div class="relative hidden md:block w-64">
          <Search class="w-4 h-4 absolute left-3 top-2.5 text-muted-foreground" />
          <input
            type="text"
            placeholder="Quick search..."
            class="w-full h-9 pl-9 pr-4 rounded-md border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring transition-all"
          />
        </div>

        <!-- Notifications -->
        <button
          type="button"
          class="w-9 h-9 rounded-md border border-input bg-background flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-accent transition-colors relative"
          title="Notifications"
        >
          <Bell class="w-4 h-4" />
          <span class="w-2 h-2 rounded-full bg-emerald-500 absolute top-2 right-2"></span>
        </button>
      </div>
    </header>

    <!-- Main Scrollable Content Area -->
    <div class="flex-1 overflow-y-auto p-6 space-y-6">
      {#if errorMsg}
        <div class="p-4 rounded-lg border border-destructive/30 bg-destructive/10 text-destructive text-xs flex items-center gap-3 animate-fade-in">
          <AlertCircle class="w-4 h-4 shrink-0" />
          <span class="font-medium">{errorMsg}</span>
        </div>
      {/if}

      <div class={activeTab === 'dashboard' ? '' : 'hidden'}>
        <Dashboard {activeTab} />
      </div>
      <div class={activeTab === 'scraper' ? '' : 'hidden'}>
        <GoogleMapsScraper />
      </div>
      <div class={activeTab === 'leads' ? '' : 'hidden'}>
        <LeadsList leads={leads.filter(l => l.status !== 'Converted')} onRefresh={loadLeads} />
      </div>
      <div class={activeTab === 'contacts' ? '' : 'hidden'}>
        <LeadsList leads={leads.filter(l => l.status === 'Converted')} onRefresh={loadLeads} />
      </div>
      <div class={activeTab === 'categories' ? '' : 'hidden'}>
        <CategoryManager {activeTab} />
      </div>
      <div class={activeTab === 'outreach' ? '' : 'hidden'}>
        <ZaloOutreach {activeTab} />
      </div>
      <div class={activeTab === 'email_outreach' ? '' : 'hidden'}>
        <EmailOutreach {activeTab} />
      </div>
      <div class={activeTab === 'pipeline' ? '' : 'hidden'}>
        <KanbanBoard />
      </div>
    </div>
  </main>
</div>
