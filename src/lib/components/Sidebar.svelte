<script lang="ts">
  let { activeTab = $bindable() } = $props();

  const menuItems = [
    { id: 'dashboard', label: 'Dashboard', icon: 'dashboard' },
    { id: 'leads', label: 'Leads & Contacts', icon: 'people' },
    { id: 'pipeline', label: 'Deal Pipeline', icon: 'view_kanban' }
  ];
</script>

<aside class="sidebar">
  <div class="logo-container">
    <span class="material-icons logo-icon">insights</span>
    <span class="logo-text">RustSale Pro</span>
  </div>
  <nav class="sidebar-nav">
    {#each menuItems as item}
      <button 
        class="nav-item" 
        class:active={activeTab === item.id} 
        onclick={() => activeTab = item.id}
      >
        <span class="material-icons">{item.icon}</span>
        <span class="nav-label">{item.label}</span>
      </button>
    {/each}
  </nav>
</aside>

<style>
  .sidebar {
    inline-size: 240px;
    block-size: 100vh;
    background: var(--color-surface-container-low);
    color: var(--color-on-surface-variant);
    display: flex;
    flex-direction: column;
    border-inline-end: 1px solid var(--color-outline-variant);
  }
  .logo-container {
    padding-block: var(--spacing-lg);
    padding-inline: var(--spacing-md);
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    border-block-end: 1px solid var(--color-outline-variant);
  }
  .logo-icon {
    color: var(--color-success);
    font-size: 24px;
  }
  .logo-text {
    font-family: var(--font-family);
    font-weight: 700;
    font-size: 16px;
    color: var(--color-primary);
  }
  .sidebar-nav {
    display: flex;
    flex-direction: column;
    padding-block: var(--spacing-md);
    padding-inline: 0;
    gap: var(--spacing-xs);
    flex: 1;
  }
  .nav-item {
    background: none;
    border: none;
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
    padding: var(--spacing-md);
    color: var(--color-on-surface-variant);
    cursor: pointer;
    text-align: left;
    transition: var(--transition-fast);
    font-family: var(--font-family);
    font-size: 14px;
    inline-size: 100%;
  }
  .nav-item:hover {
    background: var(--color-surface-container-high);
    color: var(--color-primary);
  }
  .nav-item.active {
    background: var(--color-surface-container-highest);
    color: var(--color-primary);
    border-inline-start: 4px solid var(--color-success);
  }
</style>
