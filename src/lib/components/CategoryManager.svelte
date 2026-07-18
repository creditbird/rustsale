<script lang="ts">
  import { onMount } from 'svelte';
  import * as crmService from '$lib/services/crmService';
  import type { Category, Lead } from '$lib/services/crmService';

  let categories = $state<Category[]>([]);
  let activeCategoryId = $state<number | null>(null);
  let newCategoryName = $state('');
  
  let categoryLeads = $state<Lead[]>([]);
  let allLeads = $state<Lead[]>([]);
  let addLeadsSearch = $state('');

  let selectedToRemove = $state<Set<number>>(new Set());
  let selectedToAdd = $state<Set<number>>(new Set());

  let loading = $state(false);
  let errorMsg = $state<string | null>(null);
  let successMsg = $state<string | null>(null);

  // Load all categories on mount
  async function loadCategories() {
    try {
      loading = true;
      categories = await crmService.getCategories();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadCategories();
    loadAllLeads();
  });

  async function loadAllLeads() {
    try {
      allLeads = await crmService.getLeads();
    } catch (err) {
      console.error(err);
    }
  }

  async function loadCategoryLeads(categoryId: number) {
    try {
      loading = true;
      categoryLeads = await crmService.getLeadsByCategory(categoryId);
      selectedToRemove = new Set();
      selectedToAdd = new Set();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (activeCategoryId) {
      loadCategoryLeads(activeCategoryId);
    } else {
      categoryLeads = [];
    }
  });

  let activeCategory = $derived(categories.find(c => c.id === activeCategoryId));

  let availableLeads = $derived(
    allLeads.filter(lead => 
      !categoryLeads.some(cl => cl.id === lead.id) &&
      (addLeadsSearch ? (lead.name || '').toLowerCase().includes(addLeadsSearch.toLowerCase()) || 
       (lead.company || '').toLowerCase().includes(addLeadsSearch.toLowerCase()) : true)
    )
  );

  function showMessage(msg: string, isError = false) {
    if (isError) {
      errorMsg = msg;
      setTimeout(() => errorMsg = null, 4000);
    } else {
      successMsg = msg;
      setTimeout(() => successMsg = null, 4000);
    }
  }

  async function handleCreateCategory() {
    if (!newCategoryName.trim()) return;
    try {
      await crmService.createCategory(newCategoryName.trim());
      newCategoryName = '';
      await loadCategories();
      showMessage('Category created successfully.');
    } catch (err) {
      showMessage(err instanceof Error ? err.message : String(err), true);
    }
  }

  async function handleDeleteCategory(id: number) {
    if (!confirm('Are you sure you want to delete this category?')) return;
    try {
      await crmService.deleteCategory(id);
      if (activeCategoryId === id) activeCategoryId = null;
      await loadCategories();
      showMessage('Category deleted.');
    } catch (err) {
      showMessage(err instanceof Error ? err.message : String(err), true);
    }
  }

  async function handleRemoveLeads() {
    if (!activeCategoryId || selectedToRemove.size === 0) return;
    try {
      await crmService.removeLeadsFromCategory(Array.from(selectedToRemove), activeCategoryId);
      await loadCategoryLeads(activeCategoryId);
      await loadAllLeads();
      showMessage('Leads removed from category.');
    } catch (err) {
      showMessage(err instanceof Error ? err.message : String(err), true);
    }
  }

  async function handleAddLeads() {
    if (!activeCategoryId || selectedToAdd.size === 0) return;
    try {
      await crmService.addLeadsToCategory(Array.from(selectedToAdd), activeCategoryId);
      await loadCategoryLeads(activeCategoryId);
      await loadAllLeads();
      showMessage('Leads added to category.');
    } catch (err) {
      showMessage(err instanceof Error ? err.message : String(err), true);
    }
  }

  function toggleRemoveSelection(id: number) {
    if (selectedToRemove.has(id)) selectedToRemove.delete(id);
    else selectedToRemove.add(id);
    selectedToRemove = new Set(selectedToRemove);
  }

  function toggleRemoveAll(e: Event) {
    const checked = (e.target as HTMLInputElement).checked;
    if (checked) {
      categoryLeads.forEach(l => { if (l.id) selectedToRemove.add(l.id); });
    } else {
      selectedToRemove.clear();
    }
    selectedToRemove = new Set(selectedToRemove);
  }

  function toggleAddSelection(id: number) {
    if (selectedToAdd.has(id)) selectedToAdd.delete(id);
    else selectedToAdd.add(id);
    selectedToAdd = new Set(selectedToAdd);
  }

  function toggleAddAll(e: Event) {
    const checked = (e.target as HTMLInputElement).checked;
    if (checked) {
      availableLeads.forEach(l => { if (l.id) selectedToAdd.add(l.id); });
    } else {
      selectedToAdd.clear();
    }
    selectedToAdd = new Set(selectedToAdd);
  }
</script>

<div class="category-manager">
  {#if errorMsg}
    <div class="alert alert-error">
      <span class="material-icons">error_outline</span> {errorMsg}
    </div>
  {/if}
  {#if successMsg}
    <div class="alert alert-success">
      <span class="material-icons">check_circle</span> {successMsg}
    </div>
  {/if}

  <div class="layout">
    <!-- LEFT PANEL: Category List -->
    <aside class="sidebar-panel glass-panel">
      <div class="panel-header">
        <h3>Categories</h3>
      </div>
      
      <div class="add-category">
        <input 
          type="text" 
          bind:value={newCategoryName} 
          placeholder="New Category..." 
          onkeydown={(e) => { if (e.key === 'Enter') handleCreateCategory(); }}
        />
        <button class="icon-btn primary" onclick={handleCreateCategory} aria-label="Add Category">
          <span class="material-icons">add</span>
        </button>
      </div>

      <ul class="category-list">
        {#each categories as category}
          <li class="category-item" class:active={activeCategoryId === category.id}>
            <button class="category-btn" onclick={() => activeCategoryId = category.id!}>
              <span class="material-icons">folder</span>
              <span class="name">{category.name}</span>
            </button>
            <button class="icon-btn danger delete-btn" onclick={() => handleDeleteCategory(category.id!)}>
              <span class="material-icons">delete</span>
            </button>
          </li>
        {/each}
        {#if categories.length === 0 && !loading}
          <div class="empty-state">No categories found.</div>
        {/if}
      </ul>
    </aside>

    <!-- RIGHT PANEL: Category Details -->
    <main class="main-panel">
      {#if activeCategoryId && activeCategory}
        <div class="panel-header glass-panel">
          <h2>{activeCategory.name}</h2>
          <span class="subtitle">{categoryLeads.length} lead(s) in this category</span>
        </div>

        <div class="tables-container">
          <!-- Current Leads Table -->
          <div class="table-section glass-panel">
            <div class="section-header">
              <h3>Leads in Category</h3>
              {#if selectedToRemove.size > 0}
                <button class="action-btn danger" onclick={handleRemoveLeads}>
                  <span class="material-icons">remove_circle</span>
                  Remove Selected ({selectedToRemove.size})
                </button>
              {/if}
            </div>
            
            <div class="table-wrapper">
              <table>
                <thead>
                  <tr>
                    <th class="checkbox-col">
                      <input 
                        type="checkbox" 
                        checked={categoryLeads.length > 0 && selectedToRemove.size === categoryLeads.length}
                        indeterminate={selectedToRemove.size > 0 && selectedToRemove.size < categoryLeads.length}
                        onchange={toggleRemoveAll}
                      />
                    </th>
                    <th>Name</th>
                    <th>Company</th>
                    <th>Status</th>
                  </tr>
                </thead>
                <tbody>
                  {#each categoryLeads as lead}
                    <tr class:selected={selectedToRemove.has(lead.id!)}>
                      <td class="checkbox-col">
                        <input 
                          type="checkbox" 
                          checked={selectedToRemove.has(lead.id!)}
                          onchange={() => toggleRemoveSelection(lead.id!)}
                        />
                      </td>
                      <td class="primary-cell">{lead.name}</td>
                      <td>{lead.company}</td>
                      <td><span class="status-badge {lead.status.toLowerCase()}">{lead.status}</span></td>
                    </tr>
                  {/each}
                  {#if categoryLeads.length === 0}
                    <tr><td colspan="4" class="empty-state">No leads in this category.</td></tr>
                  {/if}
                </tbody>
              </table>
            </div>
          </div>

          <!-- Add Leads Table -->
          <div class="table-section glass-panel">
            <div class="section-header">
              <h3>Add Leads to Category</h3>
              {#if selectedToAdd.size > 0}
                <button class="action-btn success" onclick={handleAddLeads}>
                  <span class="material-icons">add_circle</span>
                  Add Selected ({selectedToAdd.size})
                </button>
              {/if}
            </div>
            
            <div class="search-box">
              <span class="material-icons search-icon">search</span>
              <input type="text" bind:value={addLeadsSearch} placeholder="Search leads to add..." />
            </div>

            <div class="table-wrapper">
              <table>
                <thead>
                  <tr>
                    <th class="checkbox-col">
                      <input 
                        type="checkbox" 
                        checked={availableLeads.length > 0 && selectedToAdd.size === availableLeads.length}
                        indeterminate={selectedToAdd.size > 0 && selectedToAdd.size < availableLeads.length}
                        onchange={toggleAddAll}
                      />
                    </th>
                    <th>Name</th>
                    <th>Company</th>
                    <th>Status</th>
                  </tr>
                </thead>
                <tbody>
                  {#each availableLeads as lead}
                    <tr class:selected={selectedToAdd.has(lead.id!)}>
                      <td class="checkbox-col">
                        <input 
                          type="checkbox" 
                          checked={selectedToAdd.has(lead.id!)}
                          onchange={() => toggleAddSelection(lead.id!)}
                        />
                      </td>
                      <td class="primary-cell">{lead.name}</td>
                      <td>{lead.company}</td>
                      <td><span class="status-badge {lead.status.toLowerCase()}">{lead.status}</span></td>
                    </tr>
                  {/each}
                  {#if availableLeads.length === 0}
                    <tr><td colspan="4" class="empty-state">No available leads to add.</td></tr>
                  {/if}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      {:else}
        <div class="glass-panel empty-selection">
          <span class="material-icons huge-icon">folder_open</span>
          <h2>Select a category</h2>
          <p>Choose a category from the left panel to manage its leads.</p>
        </div>
      {/if}
    </main>
  </div>
</div>

<style>
  .category-manager {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
    height: 100%;
  }

  .layout {
    display: flex;
    gap: var(--spacing-md);
    height: calc(100vh - 120px);
    overflow: hidden;
  }

  .glass-panel {
    background: var(--color-surface-container-lowest);
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-lg);
    box-shadow: var(--shadow-soft);
  }

  .sidebar-panel {
    width: 320px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .main-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
    overflow: hidden;
  }

  .panel-header {
    padding: var(--spacing-md) var(--spacing-lg);
    border-bottom: 1px solid var(--color-outline-variant);
  }

  .panel-header h2 {
    font-size: 20px;
    font-weight: 700;
    color: var(--color-on-surface);
  }

  .panel-header h3 {
    font-size: 16px;
    font-weight: 600;
    color: var(--color-on-surface);
  }

  .subtitle {
    font-size: 13px;
    color: var(--color-on-surface-variant);
  }

  .add-category {
    display: flex;
    padding: var(--spacing-md);
    gap: var(--spacing-xs);
    border-bottom: 1px solid var(--color-outline-variant);
  }

  .add-category input {
    flex: 1;
    padding: var(--spacing-sm);
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-md);
    background: var(--color-surface-container-low);
    color: var(--color-on-surface);
    outline: none;
    font-size: 13px;
  }
  
  .add-category input:focus {
    border-color: var(--color-primary);
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border: none;
    border-radius: var(--rounded-md);
    cursor: pointer;
    transition: var(--transition-fast);
  }
  
  .icon-btn.primary {
    background: var(--color-primary);
    color: var(--color-on-primary);
  }
  .icon-btn.primary:hover {
    background: var(--color-primary-container);
  }

  .icon-btn.danger {
    background: transparent;
    color: var(--color-on-surface-variant);
  }
  .icon-btn.danger:hover {
    background: rgba(244, 63, 94, 0.1);
    color: var(--color-danger);
  }

  .category-list {
    list-style: none;
    padding: 0;
    margin: 0;
    overflow-y: auto;
    flex: 1;
  }

  .category-item {
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--color-outline-variant);
    transition: var(--transition-fast);
  }
  
  .category-item:hover {
    background: var(--color-surface-container-low);
  }
  
  .category-item.active {
    background: var(--color-primary-container);
    border-left: 4px solid var(--color-primary);
  }
  .category-item.active .name, .category-item.active .material-icons {
    color: var(--color-on-primary-container);
  }

  .category-btn {
    flex: 1;
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    padding: var(--spacing-md);
    background: none;
    border: none;
    text-align: left;
    cursor: pointer;
    color: var(--color-on-surface-variant);
  }

  .category-btn .name {
    font-weight: 500;
    font-size: 14px;
  }
  
  .delete-btn {
    margin-right: var(--spacing-xs);
    opacity: 0;
  }
  .category-item:hover .delete-btn {
    opacity: 1;
  }

  .tables-container {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
    flex: 1;
    overflow-y: auto;
    padding-bottom: var(--spacing-md);
  }

  .table-section {
    display: flex;
    flex-direction: column;
    max-height: 400px;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--spacing-sm) var(--spacing-lg);
    border-bottom: 1px solid var(--color-outline-variant);
    background: var(--color-surface-container-low);
  }

  .action-btn {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
    padding: var(--spacing-xs) var(--spacing-sm);
    border: none;
    border-radius: var(--rounded-md);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: var(--transition-fast);
  }
  .action-btn.danger {
    background: rgba(244, 63, 94, 0.1);
    color: var(--color-danger);
  }
  .action-btn.danger:hover {
    background: var(--color-danger);
    color: #fff;
  }
  .action-btn.success {
    background: rgba(16, 185, 129, 0.1);
    color: var(--color-success);
  }
  .action-btn.success:hover {
    background: var(--color-success);
    color: #fff;
  }
  
  .action-btn .material-icons {
    font-size: 16px;
  }

  .search-box {
    position: relative;
    padding: var(--spacing-sm) var(--spacing-md);
    border-bottom: 1px solid var(--color-outline-variant);
  }

  .search-box input {
    width: 100%;
    padding: var(--spacing-sm) var(--spacing-sm) var(--spacing-sm) 32px;
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-md);
    background: var(--color-surface-container-lowest);
    color: var(--color-on-surface);
    font-size: 13px;
    outline: none;
  }

  .search-box input:focus {
    border-color: var(--color-primary);
  }

  .search-icon {
    position: absolute;
    left: calc(var(--spacing-md) + 8px);
    top: 50%;
    transform: translateY(-50%);
    font-size: 16px;
    color: var(--color-on-surface-variant);
    pointer-events: none;
  }

  .table-wrapper {
    overflow-y: auto;
    flex: 1;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    text-align: left;
  }

  th {
    background: var(--color-surface-container-low);
    color: var(--color-on-surface-variant);
    font-weight: 600;
    padding: var(--spacing-sm) var(--spacing-md);
    position: sticky;
    top: 0;
    z-index: 10;
  }

  td {
    padding: var(--spacing-sm) var(--spacing-md);
    border-bottom: 1px solid var(--color-outline-variant);
    color: var(--color-on-surface);
  }

  tr:hover {
    background: var(--color-surface-container-low);
  }

  tr.selected {
    background: rgba(16, 185, 129, 0.05);
  }

  .checkbox-col {
    width: 40px;
    text-align: center;
  }

  .primary-cell {
    font-weight: 500;
  }

  .empty-state {
    text-align: center;
    padding: var(--spacing-xl);
    color: var(--color-on-surface-variant);
    font-style: italic;
  }

  .empty-selection {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--color-on-surface-variant);
  }

  .huge-icon {
    font-size: 64px;
    margin-bottom: var(--spacing-md);
    opacity: 0.5;
  }

  .alert {
    padding: var(--spacing-sm) var(--spacing-md);
    border-radius: var(--rounded-md);
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    font-size: 14px;
    font-weight: 500;
  }

  .alert-error {
    background: rgba(244, 63, 94, 0.1);
    color: var(--color-danger);
    border: 1px solid var(--color-danger);
  }

  .alert-success {
    background: rgba(16, 185, 129, 0.1);
    color: var(--color-success);
    border: 1px solid var(--color-success);
  }

  .status-badge {
    display: inline-block;
    padding: 2px 8px;
    border-radius: var(--rounded-full);
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
  }

  .status-badge.new { background: #e0f2fe; color: #0369a1; }
  .status-badge.contacted { background: #fef3c7; color: #b45309; }
  .status-badge.qualified { background: rgba(16, 185, 129, 0.1); color: var(--color-success); }
  .status-badge.lost { background: rgba(244, 63, 94, 0.1); color: var(--color-danger); }
  .status-badge.converted { background: #e0e7ff; color: #4338ca; }
</style>
