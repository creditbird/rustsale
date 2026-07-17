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

  <div class="table-wrapper">
    <table class="leads-table">
      <thead>
        <tr>
          <th>Name</th>
          <th>Company</th>
          <th>Status</th>
          <th>Value</th>
          <th>Owner</th>
        </tr>
      </thead>
      <tbody>
        {#if leads.length === 0}
          <tr>
            <td colspan="5" class="empty-state">No leads available.</td>
          </tr>
        {:else}
          {#each leads as lead}
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
                  <strong>{lead.name}</strong>
                </div>
              </td>
              <td>{lead.company}</td>
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

  {#if selectedLeadId !== null}
    <LeadDetails leadId={selectedLeadId} onClose={() => selectedLeadId = null} />
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
