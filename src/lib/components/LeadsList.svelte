<script lang="ts">
  import type { Lead } from '$lib/services/crmService';
  import LeadDetails from './LeadDetails.svelte';

  let { leads = [] } = $props<{ leads: Lead[] }>();
  let selectedLeadId = $state<number | null>(null);
</script>

<div class="leads-container">
  <div class="list-header">
    <h2>Leads & Contacts</h2>
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
            <tr onclick={() => { if (lead.id !== undefined) selectedLeadId = lead.id; }}>
              <td>
                <div class="lead-name-cell">
                  <div class="avatar-sm">
                    {lead.name.split(' ').map(n => n[0]).join('').toUpperCase().substring(0, 2)}
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

  .list-header h2 {
    font-size: 20px;
    font-weight: 700;
    color: var(--color-on-surface);
  }

  .subtitle {
    font-size: 13px;
    color: var(--color-on-surface-variant);
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
</style>
