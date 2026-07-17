<script lang="ts">
  import type { Lead } from '$lib/services/crmService';
  import * as crmService from '$lib/services/crmService';
  import LeadDetails from './LeadDetails.svelte';

  let { leads = [], onRefresh } = $props<{ leads: Lead[]; onRefresh?: () => void }>();
  let selectedLeadId = $state<number | null>(null);

  // Lead creation modal states
  let showAddModal = $state(false);
  let name = $state('');
  let company = $state('');
  let value = $state(0);
  let owner = $state('Self');
  let email = $state('');
  let phone = $state('');
  let about = $state('');
  let saving = $state(false);
  let modalErrorMsg = $state<string | null>(null);

  // Search, filter, and pagination states
  let searchQuery = $state('');
  let filterStatus = $state('');
  let filterOwner = $state('');
  let filterValueRange = $state(''); // 'all', 'under_10k', '10k_to_50k', 'above_50k'
  let filterZaloStatus = $state(''); // '', 'verified', 'no_zalo', 'unchecked'
  let filterOutreachStatus = $state(''); // '', 'reached', 'not_reached'
  let currentPage = $state(1);
  let pageSize = $state(10);

  // Derived variables
  let uniqueStatuses = $derived(Array.from(new Set(leads.map((l: Lead) => l.status))));
  let uniqueOwners = $derived(Array.from(new Set(leads.map((l: Lead) => l.owner).filter(Boolean))) as string[]);

  let filteredLeads = $derived(
    leads.filter((lead: Lead) => {
      // 1. Search Query
      if (searchQuery.trim()) {
        const query = searchQuery.toLowerCase().trim();
        const matchesName = lead.name.toLowerCase().includes(query);
        const matchesCompany = lead.company.toLowerCase().includes(query);
        const matchesEmail = lead.email.toLowerCase().includes(query);
        const matchesOwner = (lead.owner || '').toLowerCase().includes(query);
        if (!matchesName && !matchesCompany && !matchesEmail && !matchesOwner) {
          return false;
        }
      }
      
      // 2. Status Filter
      if (filterStatus && lead.status !== filterStatus) {
        return false;
      }
      
      // 3. Owner Filter
      if (filterOwner && lead.owner !== filterOwner) {
        return false;
      }
      
      // 4. Value Range Filter
      if (filterValueRange) {
        const val = lead.value || 0;
        if (filterValueRange === 'under_10k' && val >= 10000) return false;
        if (filterValueRange === '10k_to_50k' && (val < 10000 || val > 50000)) return false;
        if (filterValueRange === 'above_50k' && val <= 50000) return false;
      }

      // 5. Zalo Status Filter
      if (filterZaloStatus) {
        if (filterZaloStatus === 'verified' && lead.has_zalo !== 1) return false;
        if (filterZaloStatus === 'no_zalo' && lead.has_zalo !== 0) return false;
        if (filterZaloStatus === 'unchecked' && lead.has_zalo !== null && lead.has_zalo !== undefined) return false;
      }

      // 6. Outreach Status Filter
      if (filterOutreachStatus) {
        if (filterOutreachStatus === 'reached' && lead.zalo_reached !== 1) return false;
        if (filterOutreachStatus === 'not_reached' && lead.zalo_reached !== 0) return false;
      }
      
      return true;
    })
  );

  // Reset to first page when filters change
  $effect(() => {
    // track dependencies
    const _s = searchQuery;
    const _st = filterStatus;
    const _o = filterOwner;
    const _v = filterValueRange;
    const _z = filterZaloStatus;
    const _or = filterOutreachStatus;
    const _p = pageSize;
    currentPage = 1;
  });

  let paginatedLeads = $derived(
    filteredLeads.slice((currentPage - 1) * pageSize, currentPage * pageSize)
  );

  let totalPages = $derived(Math.max(1, Math.ceil(filteredLeads.length / pageSize)));
  let totalEntries = $derived(filteredLeads.length);
  let showingStart = $derived(totalEntries === 0 ? 0 : (currentPage - 1) * pageSize + 1);
  let showingEnd = $derived(Math.min(totalEntries, currentPage * pageSize));

  async function handleAddLead(e: Event) {
    e.preventDefault();
    if (!name.trim() || !company.trim() || !email.trim()) return;

    saving = true;
    modalErrorMsg = null;
    try {
      await crmService.addLead(
        name.trim(),
        company.trim(),
        'New',
        value,
        owner.trim(),
        email.trim(),
        phone.trim(),
        about.trim()
      );
      // Reset form
      name = '';
      company = '';
      value = 0;
      owner = 'Self';
      email = '';
      phone = '';
      about = '';
      showAddModal = false;
      // Refresh list
      if (onRefresh) {
        onRefresh();
      }
    } catch (err) {
      modalErrorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      saving = false;
    }
  }
</script>

<div class="leads-container">
  <div class="list-header">
    <div class="header-row">
      <h2>Leads & Contacts</h2>
      <button class="add-lead-btn" onclick={() => { showAddModal = true; modalErrorMsg = null; }}>
        <span class="material-icons">add</span>
        <span>Add Lead</span>
      </button>
    </div>
    <p class="subtitle">Select a lead row to view details, notes, and activity history.</p>
  </div>

  <div class="controls-bar">
    <div class="search-box">
      <span class="material-icons search-icon">search</span>
      <input 
        type="text" 
        bind:value={searchQuery} 
        placeholder="Search by name, company, email, owner..." 
      />
    </div>

    <div class="filters-box">
      {#if uniqueStatuses.length > 1}
        <select bind:value={filterStatus} class="filter-select">
          <option value="">All Statuses</option>
          {#each uniqueStatuses as status}
            <option value={status}>{status}</option>
          {/each}
        </select>
      {/if}

      <select bind:value={filterOwner} class="filter-select">
        <option value="">All Owners</option>
        {#each uniqueOwners as owner}
          <option value={owner}>{owner}</option>
        {/each}
      </select>

      <select bind:value={filterValueRange} class="filter-select">
        <option value="">All Values</option>
        <option value="under_10k">Under $10k</option>
        <option value="10k_to_50k">$10k - $50k</option>
        <option value="above_50k">Above $50k</option>
      </select>

      <select bind:value={filterZaloStatus} class="filter-select">
        <option value="">All Zalo Statuses</option>
        <option value="verified">Verified Zalo</option>
        <option value="no_zalo">No Zalo</option>
        <option value="unchecked">Unchecked</option>
      </select>

      <select bind:value={filterOutreachStatus} class="filter-select">
        <option value="">All Outreach Statuses</option>
        <option value="reached">Reached</option>
        <option value="not_reached">Not Reached</option>
      </select>

      {#if searchQuery || filterStatus || filterOwner || filterValueRange || filterZaloStatus || filterOutreachStatus}
        <button 
          class="clear-filters-btn" 
          onclick={() => { searchQuery = ''; filterStatus = ''; filterOwner = ''; filterValueRange = ''; filterZaloStatus = ''; filterOutreachStatus = ''; }}
        >
          Clear
        </button>
      {/if}
    </div>
  </div>

  <div class="table-wrapper">
    <table class="leads-table">
      <thead>
        <tr>
          <th>Name</th>
          <th>Company</th>
          <th>Address</th>
          <th>Status</th>
          <th>Value</th>
          <th>Owner</th>
        </tr>
      </thead>
      <tbody>
        {#if paginatedLeads.length === 0}
          <tr>
            <td colspan="6" class="empty-state">No matching records found.</td>
          </tr>
        {:else}
          {#each paginatedLeads as lead}
            <tr 
              tabindex="0"
              onclick={() => { if (lead.id !== undefined) selectedLeadId = lead.id; }}
              onkeydown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  if (lead.id !== undefined) selectedLeadId = lead.id;
                }
              }}
            >
              <td>
                <div class="lead-name-cell">
                  <div class="avatar-sm">
                    {lead.name.split(' ').map((n: string) => n[0]).join('').toUpperCase().substring(0, 2)}
                  </div>
                  <div style="display: flex; flex-direction: column;">
                    <strong>{lead.name}</strong>
                    {#if lead.phone}
                      <span style="font-size: 11px; color: var(--color-on-surface-variant); display: flex; align-items: center; gap: 4px; margin-top: 2px;">
                        {lead.phone}
                        {#if lead.has_zalo === 1}
                          <span class="zalo-badge verified" title="Zalo Verified" style="color: #0284c7; display: inline-flex; align-items: center;">
                            <span class="material-icons" style="font-size: 12px;">check_circle</span>
                          </span>
                        {:else if lead.has_zalo === 0}
                          <span class="zalo-badge unverified" title="No Zalo" style="color: #94a3b8; display: inline-flex; align-items: center;">
                            <span class="material-icons" style="font-size: 12px;">cancel</span>
                          </span>
                        {/if}
                      </span>
                    {/if}
                  </div>
                </div>
              </td>
              <td>{lead.company}</td>
              <td>{lead.address || '—'}</td>
              <td>
                <span class="status-badge {lead.status.toLowerCase()}">{lead.status}</span>
              </td>
              <td class="value-cell">${(lead.value || 0).toLocaleString()}</td>
              <td>{lead.owner || '—'}</td>
            </tr>
          {/each}
        {/if}
      </tbody>
    </table>
  </div>

  {#if totalEntries > 0}
    <div class="pagination-bar">
      <div class="pagination-info">
        Showing {showingStart} to {showingEnd} of {totalEntries} entries
      </div>
      
      <div class="pagination-controls">
        <label for="page-size-select" class="sr-only">Entries per page</label>
        <select id="page-size-select" bind:value={pageSize} class="page-size-select">
          <option value={5}>5 per page</option>
          <option value={10}>10 per page</option>
          <option value={25}>25 per page</option>
        </select>

        <button 
          class="page-btn" 
          disabled={currentPage === 1} 
          onclick={() => currentPage--}
        >
          Previous
        </button>
        
        {#each Array(totalPages) as _, i}
          <button 
            class="page-btn" 
            class:active={currentPage === i + 1}
            onclick={() => currentPage = i + 1}
          >
            {i + 1}
          </button>
        {/each}
        
        <button 
          class="page-btn" 
          disabled={currentPage === totalPages} 
          onclick={() => currentPage++}
        >
          Next
        </button>
      </div>
    </div>
  {/if}

  {#if selectedLeadId !== null}
    <LeadDetails leadId={selectedLeadId} onClose={() => selectedLeadId = null} onRefresh={onRefresh} />
  {/if}
</div>

{#if showAddModal}
  <!-- Modal Backdrop -->
  <div 
    class="modal-backdrop" 
    onclick={() => showAddModal = false} 
    role="presentation"
  ></div>

  <!-- Modal Dialog -->
  <div class="modal-container" role="dialog" aria-modal="true" aria-labelledby="modal-title">
    <header class="modal-header">
      <h3 id="modal-title">Create New Lead</h3>
      <button class="close-btn" onclick={() => showAddModal = false} aria-label="Close modal">
        <span class="material-icons">close</span>
      </button>
    </header>

    {#if modalErrorMsg}
      <div class="error-banner">
        <span class="material-icons">error_outline</span>
        <span>{modalErrorMsg}</span>
      </div>
    {/if}

    <form onsubmit={handleAddLead} class="modal-form">
      <div class="form-grid">
        <div class="form-group">
          <label for="lead-name">Name *</label>
          <input type="text" id="lead-name" bind:value={name} required placeholder="Full Name" />
        </div>

        <div class="form-group">
          <label for="lead-company">Company *</label>
          <input type="text" id="lead-company" bind:value={company} required placeholder="Company Name" />
        </div>

        <div class="form-group">
          <label for="lead-email">Email *</label>
          <input type="email" id="lead-email" bind:value={email} required placeholder="email@example.com" />
        </div>

        <div class="form-group">
          <label for="lead-phone">Phone</label>
          <input type="text" id="lead-phone" bind:value={phone} placeholder="Phone number" />
        </div>

        <div class="form-group">
          <label for="lead-value">Estimated Value ($)</label>
          <input type="number" id="lead-value" bind:value={value} min="0" />
        </div>

        <div class="form-group">
          <label for="lead-owner">Owner</label>
          <input type="text" id="lead-owner" bind:value={owner} placeholder="Owner Name" />
        </div>
      </div>

      <div class="form-group full-width">
        <label for="lead-about">About / Description</label>
        <textarea id="lead-about" bind:value={about} rows="3" placeholder="Lead details..."></textarea>
      </div>

      <div class="modal-actions">
        <button type="button" class="cancel-btn" onclick={() => showAddModal = false}>Cancel</button>
        <button type="submit" class="save-btn" disabled={saving}>
          {#if saving}Saving...{:else}Save Lead{/if}
        </button>
      </div>
    </form>
  </div>
{/if}

<style>
  .controls-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--spacing-md);
    flex-wrap: wrap;
    background: var(--color-surface-container-lowest);
    border: 1px solid var(--color-outline-variant);
    padding: var(--spacing-sm) var(--spacing-md);
    border-radius: var(--rounded-md);
    box-shadow: var(--shadow-soft);
  }
  .search-box {
    position: relative;
    flex: 1;
    min-inline-size: 200px;
    max-inline-size: 400px;
    display: flex;
    align-items: center;
  }
  .search-icon {
    position: absolute;
    left: var(--spacing-sm);
    color: var(--color-on-surface-variant);
    font-size: 18px;
    pointer-events: none;
    display: flex;
    align-items: center;
  }
  .search-box input {
    width: 100%;
    padding: var(--spacing-xs) var(--spacing-sm);
    padding-left: calc(var(--spacing-sm) + 22px);
    border-radius: var(--rounded-md);
    border: 1px solid var(--color-outline-variant);
    background: var(--color-surface-container-lowest);
    color: var(--color-on-surface);
    font-family: var(--font-family);
    font-size: 13px;
    outline: none;
    transition: var(--transition-fast);
  }
  .search-box input:focus {
    border-color: var(--color-outline);
  }
  .filters-box {
    display: flex;
    gap: var(--spacing-sm);
    align-items: center;
    flex-wrap: wrap;
  }
  .filter-select {
    padding: var(--spacing-xs) var(--spacing-sm);
    border-radius: var(--rounded-md);
    border: 1px solid var(--color-outline-variant);
    background: var(--color-surface-container-lowest);
    color: var(--color-on-surface);
    font-family: var(--font-family);
    font-size: 13px;
    outline: none;
    cursor: pointer;
    transition: var(--transition-fast);
  }
  .filter-select:focus {
    border-color: var(--color-outline);
  }
  .clear-filters-btn {
    background: none;
    border: 1px solid var(--color-outline-variant);
    color: var(--color-on-surface-variant);
    padding: var(--spacing-xs) var(--spacing-sm);
    border-radius: var(--rounded-md);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: var(--transition-fast);
  }
  .clear-filters-btn:hover {
    background: var(--color-surface-container-low);
    color: var(--color-primary);
  }
  .pagination-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--spacing-md);
    background: var(--color-surface-container-lowest);
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-md);
    box-shadow: var(--shadow-soft);
    font-size: 13px;
    color: var(--color-on-surface-variant);
    flex-wrap: wrap;
    gap: var(--spacing-md);
  }
  .pagination-controls {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
  }
  .page-size-select {
    padding: 4px var(--spacing-sm);
    border-radius: var(--rounded-md);
    border: 1px solid var(--color-outline-variant);
    background: var(--color-surface-container-lowest);
    color: var(--color-on-surface-variant);
    font-size: 12px;
    margin-right: var(--spacing-sm);
    cursor: pointer;
    outline: none;
  }
  .page-btn {
    background: var(--color-surface-container-lowest);
    border: 1px solid var(--color-outline-variant);
    color: var(--color-on-surface-variant);
    padding: 6px 12px;
    border-radius: var(--rounded-md);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: var(--transition-fast);
  }
  .page-btn:hover:not(:disabled) {
    background: var(--color-surface-container-low);
    color: var(--color-primary);
  }
  .page-btn.active {
    background: var(--color-primary);
    color: var(--color-on-primary);
    border-color: var(--color-primary);
  }
  .page-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .leads-container {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
  }

  .list-header {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
  }

  .header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .list-header h2 {
    font-size: 20px;
    font-weight: 700;
    color: var(--color-on-surface);
  }

  .subtitle {
    font-size: 13px;
    color: var(--color-on-surface-variant);
  }

  .add-lead-btn {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
    background: var(--color-primary);
    color: var(--color-on-primary);
    padding: var(--spacing-sm) var(--spacing-md);
    border-radius: var(--rounded-md);
    font-size: 13px;
    font-weight: 600;
    transition: var(--transition-fast);
  }

  .add-lead-btn:hover {
    background: var(--color-primary-container);
    color: var(--color-on-primary-container);
  }

  .add-lead-btn .material-icons {
    font-size: 16px;
  }

  .table-wrapper {
    background: var(--color-surface-container-lowest);
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-md);
    overflow: hidden;
    box-shadow: var(--shadow-soft);
  }

  .leads-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    text-align: left;
  }

  .leads-table th {
    background: var(--color-surface-container-low);
    color: var(--color-on-surface-variant);
    font-weight: 700;
    text-transform: uppercase;
    font-size: 10px;
    letter-spacing: 0.05em;
    padding: var(--spacing-md) var(--spacing-lg);
    border-bottom: 1px solid var(--color-outline-variant);
  }

  .leads-table td {
    padding: var(--spacing-md) var(--spacing-lg);
    color: var(--color-on-surface);
    border-bottom: 1px solid var(--color-outline-variant);
  }

  .leads-table tbody tr {
    cursor: pointer;
    transition: var(--transition-fast);
    outline: none;
  }

  .leads-table tbody tr:focus {
    background: var(--color-surface-container-low);
    box-shadow: inset 3px 0 0 var(--color-success);
  }

  .leads-table tbody tr:hover {
    background: var(--color-surface-container-low);
  }

  .leads-table tbody tr:last-child td {
    border-bottom: none;
  }

  .lead-name-cell {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
  }

  .avatar-sm {
    width: 28px;
    height: 28px;
    border-radius: var(--rounded-full);
    background: var(--color-surface-dim);
    color: var(--color-on-surface);
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 700;
    font-size: 10px;
    border: 1px solid var(--color-outline-variant);
  }

  .lead-name-cell strong {
    font-weight: 600;
    color: var(--color-on-surface);
  }

  .status-badge {
    display: inline-block;
    padding: 2px 8px;
    border-radius: var(--rounded-full);
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
  }

  .status-badge.new {
    background: #e0f2fe;
    color: #0369a1;
  }

  .status-badge.contacted {
    background: #fef3c7;
    color: #b45309;
  }

  .status-badge.qualified {
    background: rgba(16, 185, 129, 0.1);
    color: var(--color-success);
  }

  .status-badge.lost {
    background: rgba(244, 63, 94, 0.1);
    color: var(--color-danger);
  }

  .status-badge.converted {
    background: #e0e7ff;
    color: #4338ca;
  }

  .value-cell {
    font-weight: 600;
    color: var(--color-success);
  }

  .empty-state {
    text-align: center;
    color: var(--color-on-surface-variant);
    padding-block: var(--spacing-xl);
    font-style: italic;
  }

  /* Modal Styles */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(15, 23, 42, 0.15);
    backdrop-filter: blur(2px);
    z-index: 2000;
  }

  .modal-container {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 500px;
    max-width: 90%;
    background: var(--color-surface-container-lowest);
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-lg);
    box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.1), 0 8px 10px -6px rgba(0, 0, 0, 0.1);
    z-index: 2001;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--spacing-md) var(--spacing-lg);
    border-bottom: 1px solid var(--color-outline-variant);
  }

  .modal-header h3 {
    font-size: 16px;
    font-weight: 700;
    color: var(--color-on-surface);
  }

  .close-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-on-surface-variant);
    border-radius: var(--rounded-full);
    padding: var(--spacing-xs);
    transition: var(--transition-fast);
  }

  .close-btn:hover {
    background: var(--color-surface-container-low);
    color: var(--color-on-surface);
  }

  .modal-form {
    padding: var(--spacing-lg);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
  }

  .form-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: var(--spacing-md);
  }

  @media (max-width: 480px) {
    .form-grid {
      grid-template-columns: 1fr;
    }
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
  }

  .form-group.full-width {
    grid-column: 1 / -1;
  }

  .form-group label {
    font-size: 12px;
    font-weight: 600;
    color: var(--color-on-surface-variant);
  }

  .form-group input, 
  .form-group textarea {
    padding: var(--spacing-sm);
    border-radius: var(--rounded-md);
    border: 1px solid var(--color-outline-variant);
    background: var(--color-surface-container-lowest);
    color: var(--color-on-surface);
    font-family: var(--font-family);
    font-size: 13px;
    outline: none;
    transition: var(--transition-fast);
  }

  .form-group input:focus, 
  .form-group textarea:focus {
    border-color: var(--color-outline);
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--spacing-sm);
    margin-top: var(--spacing-sm);
  }

  .cancel-btn {
    background: none;
    border: 1px solid var(--color-outline-variant);
    color: var(--color-on-surface-variant);
    padding: var(--spacing-sm) var(--spacing-md);
    border-radius: var(--rounded-md);
    font-size: 13px;
    font-weight: 600;
    transition: var(--transition-fast);
  }

  .cancel-btn:hover {
    background: var(--color-surface-container-low);
    color: var(--color-on-surface);
  }

  .save-btn {
    background: var(--color-secondary);
    color: var(--color-on-secondary);
    padding: var(--spacing-sm) var(--spacing-md);
    border-radius: var(--rounded-md);
    font-size: 13px;
    font-weight: 600;
    transition: var(--transition-fast);
  }

  .save-btn:hover:not(:disabled) {
    background: var(--color-on-secondary-container);
  }

  .save-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

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
</style>
