<script lang="ts">
  import Sidebar from '$lib/components/Sidebar.svelte';
  import Dashboard from '$lib/components/Dashboard.svelte';

  let activeTab = $state('dashboard');
</script>

<div class="app-shell">
  <Sidebar bind:activeTab />

  <main class="main-content">
    <div class="scroll-container">
      {#if activeTab === 'dashboard'}
        <Dashboard />
      {:else if activeTab === 'leads'}
        <div class="view-container">
          <h1 class="view-title">Leads & Contacts</h1>
          <p class="placeholder-text">Leads & Contacts Placeholder - View switching implemented successfully.</p>
        </div>
      {:else if activeTab === 'pipeline'}
        <div class="view-container">
          <h1 class="view-title">Deal Pipeline</h1>
          <p class="placeholder-text">Deal Pipeline Placeholder - View switching implemented successfully.</p>
        </div>
      {/if}
    </div>
  </main>
</div>

<style>
  .view-container {
    padding: var(--spacing-xl);
    background-color: var(--color-surface-container-lowest);
    border-radius: var(--rounded-xl);
    border: 1px solid var(--color-outline-variant);
    min-block-size: calc(100vh - (2 * var(--spacing-lg)) - (2 * var(--spacing-xl)));
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
  }

  .view-title {
    font-size: 24px;
    font-weight: 700;
    color: var(--color-on-surface);
  }

  .placeholder-text {
    font-size: 14px;
    color: var(--color-on-surface-variant);
  }
</style>
