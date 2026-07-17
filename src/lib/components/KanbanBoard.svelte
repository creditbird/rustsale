<script lang="ts">
  import { onMount } from 'svelte';
  import * as crmService from '$lib/services/crmService';
  import type { Deal } from '$lib/services/crmService';

  let deals = $state<Deal[]>([]);
  let errorMsg = $state<string | null>(null);
  let activeDragId = $state<number | null>(null);
  let dragOverColumn = $state<string | null>(null);

  const stages = ['Prospecting', 'Demo', 'Proposal', 'Negotiation', 'Won'];

  async function loadDeals() {
    try {
      deals = await crmService.getDeals();
      errorMsg = null;
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    }
  }

  function handleDragStart(e: DragEvent, id: number) {
    activeDragId = id;
    if (e.dataTransfer) {
      e.dataTransfer.setData('text/plain', id.toString());
      e.dataTransfer.effectAllowed = 'move';
    }
  }

  function handleDragEnd() {
    activeDragId = null;
    dragOverColumn = null;
  }

  function handleDragOver(e: DragEvent, stage: string) {
    e.preventDefault();
    if (dragOverColumn !== stage) {
      dragOverColumn = stage;
    }
  }

  function handleDragLeave(e: DragEvent, stage: string) {
    if (dragOverColumn === stage) {
      dragOverColumn = null;
    }
  }

  async function handleDrop(e: DragEvent, targetStage: string) {
    e.preventDefault();
    dragOverColumn = null;

    const dealIdStr = e.dataTransfer?.getData('text/plain') || (activeDragId !== null ? activeDragId.toString() : '');
    if (dealIdStr) {
      const dealId = parseInt(dealIdStr);
      if (!isNaN(dealId)) {
        try {
          await crmService.updateDealStage(dealId, targetStage);
          await loadDeals();
        } catch (err) {
          errorMsg = err instanceof Error ? err.message : String(err);
        }
      }
    }
    activeDragId = null;
  }

  onMount(loadDeals);
</script>

<div class="kanban-container">
  <div class="kanban-header">
    <h2>Deal Pipeline</h2>
    <p class="subtitle">Drag and drop deal cards to change their stage in real-time.</p>
  </div>

  {#if errorMsg}
    <div class="error-banner">
      <span class="material-icons">error_outline</span>
      <span>{errorMsg}</span>
    </div>
  {/if}

  <div class="kanban-grid">
    {#each stages as stage}
      {@const stageDeals = deals.filter(d => d.stage === stage)}
      {@const totalValue = stageDeals.reduce((sum, d) => sum + (d.value || 0), 0)}

      <div 
        class="kanban-column"
        class:drag-over={dragOverColumn === stage}
        ondragover={(e) => handleDragOver(e, stage)}
        ondragleave={(e) => handleDragLeave(e, stage)}
        ondrop={(e) => handleDrop(e, stage)}
        role="region"
        aria-label="{stage} stage column"
      >
        <div class="column-header">
          <h3>{stage}</h3>
          <div class="column-badge">
            <span class="deal-count">{stageDeals.length}</span>
            <span class="deal-value">${totalValue.toLocaleString()}</span>
          </div>
        </div>

        <div class="card-list" class:empty={stageDeals.length === 0} role="list" aria-label="{stage} deals">
          {#if stageDeals.length === 0}
            <div class="column-empty-state">
              <span class="material-icons empty-icon">inbox</span>
              <p>Drag deals here</p>
            </div>
          {:else}
            {#each stageDeals as deal}
              <div 
                class="deal-card"
                class:dragging={activeDragId === deal.id}
                draggable="true"
                ondragstart={(e) => handleDragStart(e, deal.id!)}
                ondragend={handleDragEnd}
                role="listitem"
              >
                <div class="deal-card-header">
                  <h4>{deal.title}</h4>
                  <span class="material-icons drag-handle">drag_indicator</span>
                </div>
                <div class="deal-card-footer">
                  <span class="deal-value-text">${deal.value.toLocaleString()}</span>
                  {#if deal.created_at}
                    <span class="deal-date">{new Date(deal.created_at).toLocaleDateString('en-US', { month: 'short', day: 'numeric' })}</span>
                  {/if}
                </div>
              </div>
            {/each}
          {/if}
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
  .kanban-container {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
    block-size: 100%;
    overflow: hidden;
  }

  .kanban-header {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
  }

  .kanban-header h2 {
    font-size: 20px;
    font-weight: 700;
    color: var(--color-on-surface);
  }

  .subtitle {
    font-size: 13px;
    color: var(--color-on-surface-variant);
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
  }

  .kanban-grid {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: var(--spacing-md);
    flex: 1;
    overflow-x: auto;
    padding-bottom: var(--spacing-md);
  }

  .kanban-column {
    background: var(--color-surface-container-low);
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-md);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
    padding: var(--spacing-md);
    min-inline-size: 200px;
    transition: background-color var(--transition-fast), border-color var(--transition-fast);
  }

  .kanban-column.drag-over {
    background: var(--color-surface-container);
    border-color: var(--color-outline);
  }

  .column-header {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
    border-bottom: 1px solid var(--color-outline-variant);
    padding-bottom: var(--spacing-sm);
  }

  .column-header h3 {
    font-size: 13px;
    font-weight: 700;
    color: var(--color-on-surface);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .column-badge {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 11px;
    color: var(--color-on-surface-variant);
  }

  .deal-count {
    background: var(--color-surface-container-highest);
    padding: 1px 6px;
    border-radius: var(--rounded-sm);
    font-weight: 700;
  }

  .deal-value {
    font-weight: 600;
    color: var(--color-success);
  }

  .card-list {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
    flex: 1;
    overflow-y: auto;
  }

  .card-list.empty {
    justify-content: center;
  }

  .column-empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: var(--color-on-surface-variant);
    opacity: 0.5;
    padding-block: var(--spacing-lg);
    gap: var(--spacing-xs);
  }

  .empty-icon {
    font-size: 24px;
  }

  .column-empty-state p {
    font-size: 11px;
    font-style: italic;
  }

  .deal-card {
    background: var(--color-surface-container-lowest);
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-md);
    padding: var(--spacing-md);
    box-shadow: var(--shadow-soft);
    cursor: grab;
    transition: transform var(--transition-fast), box-shadow var(--transition-fast), opacity var(--transition-fast);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
  }

  .deal-card:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 6px rgba(15, 23, 42, 0.05);
    border-color: var(--color-outline);
  }

  .deal-card:active {
    cursor: grabbing;
  }

  .deal-card.dragging {
    opacity: 0.4;
    transform: scale(0.98);
  }

  .deal-card-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: var(--spacing-xs);
  }

  .deal-card h4 {
    font-size: 13px;
    font-weight: 600;
    color: var(--color-on-surface);
    line-height: 1.4;
  }

  .drag-handle {
    font-size: 16px;
    color: var(--color-on-surface-variant);
    opacity: 0.4;
    cursor: grab;
  }

  .deal-card-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 11px;
    margin-top: var(--spacing-xs);
  }

  .deal-value-text {
    color: var(--color-success);
    font-weight: 700;
  }

  .deal-date {
    color: var(--color-on-surface-variant);
  }
</style>
