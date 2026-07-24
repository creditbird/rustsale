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

  let activeTab = $state('dashboard');
  let leads = $state<Lead[]>([]);
  let errorMsg = $state<string | null>(null);

  async function loadLeads() {
    try {
      leads = await crmService.getLeads();
      errorMsg = null;
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    }
  }

  // Load leads initially and when switching to the leads or contacts tab
  $effect(() => {
    if (activeTab === 'leads' || activeTab === 'contacts') {
      loadLeads();
    }
  });

  onMount(loadLeads);
</script>

<div class="app-shell">
  <Sidebar bind:activeTab />

  <main class="main-content">
    <div class="scroll-container">
      {#if errorMsg}
        <div class="error-banner">
          <span class="material-icons">error_outline</span>
          <span>{errorMsg}</span>
        </div>
      {/if}

      {#if activeTab === 'dashboard'}
        <Dashboard />
      {:else if activeTab === 'leads'}
        <LeadsList leads={leads.filter(l => l.status !== 'Converted')} onRefresh={loadLeads} />
      {:else if activeTab === 'contacts'}
        <LeadsList leads={leads.filter(l => l.status === 'Converted')} onRefresh={loadLeads} />
      {:else if activeTab === 'categories'}
        <CategoryManager />
      {:else if activeTab === 'pipeline'}
        <KanbanBoard />
      {/if}

      <div class:hidden={activeTab !== 'scraper'}>
        <GoogleMapsScraper />
      </div>
      <div class:hidden={activeTab !== 'outreach'}>
        <ZaloOutreach />
      </div>
      <div class:hidden={activeTab !== 'email_outreach'}>
        <EmailOutreach />
      </div>
    </div>
  </main>
</div>

<style>
  .error-banner {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    padding: var(--spacing-md);
    background-color: rgba(244, 63, 94, 0.08);
    border: 1px solid var(--color-danger);
    border-radius: var(--rounded-md);
    color: var(--color-danger);
    font-size: 13px;
    margin-bottom: var(--spacing-md);
  }
  .hidden {
    display: none !important;
  }
</style>

