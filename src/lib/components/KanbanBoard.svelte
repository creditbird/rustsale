<script lang="ts">
  import { onMount } from 'svelte';
  import * as crmService from '$lib/services/crmService';
  import type { Deal } from '$lib/services/crmService';
  import * as Card from '$lib/components/ui/card';
  import { Badge } from '$lib/components/ui/badge';
  import { GripVertical, Inbox, AlertCircle, Calendar, DollarSign } from 'lucide-svelte';

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

<div class="space-y-4 h-full flex flex-col">
  <div class="flex items-center justify-between">
    <div>
      <h2 class="text-xl font-bold text-foreground tracking-tight">Deal Pipeline</h2>
      <p class="text-xs text-muted-foreground mt-0.5">Drag and drop deal cards across stages to update status in real-time</p>
    </div>
  </div>

  {#if errorMsg}
    <div class="p-3 rounded-lg border border-destructive/30 bg-destructive/10 text-destructive text-xs flex items-center gap-2">
      <AlertCircle class="w-4 h-4 shrink-0" />
      <span>{errorMsg}</span>
    </div>
  {/if}

  <!-- Kanban Column Grid -->
  <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-4 flex-1 overflow-x-auto min-h-0">
    {#each stages as stage}
      {@const stageDeals = deals.filter(d => d.stage === stage)}
      {@const totalValue = stageDeals.reduce((sum, d) => sum + (d.value || 0), 0)}
      {@const isOver = dragOverColumn === stage}

      <div 
        class="rounded-xl border border-border bg-card/60 p-3 flex flex-col gap-3 min-w-[200px] transition-all duration-150 {isOver ? 'bg-accent/40 border-primary/40 shadow-md ring-1 ring-primary/20' : ''}"
        ondragover={(e) => handleDragOver(e, stage)}
        ondragleave={(e) => handleDragLeave(e, stage)}
        ondrop={(e) => handleDrop(e, stage)}
        role="region"
        aria-label="{stage} stage column"
      >
        <!-- Column Header -->
        <div class="border-b border-border pb-2.5 flex flex-col gap-1">
          <div class="flex items-center justify-between">
            <h3 class="text-xs font-bold uppercase tracking-wider text-foreground">{stage}</h3>
            <Badge variant="secondary" class="text-[10px] px-2 py-0 font-bold">{stageDeals.length}</Badge>
          </div>
          <span class="text-[11px] font-bold text-emerald-600">${totalValue.toLocaleString()}</span>
        </div>

        <!-- Deal Cards List -->
        <div class="flex-1 overflow-y-auto space-y-2.5 pr-0.5" role="list">
          {#if stageDeals.length === 0}
            <div class="h-32 border border-dashed border-border rounded-lg flex flex-col items-center justify-center text-muted-foreground/60 p-4 text-center">
              <Inbox class="w-6 h-6 stroke-1 mb-1" />
              <span class="text-[11px] italic">Drag deals here</span>
            </div>
          {:else}
            {#each stageDeals as deal}
              {@const isDragging = activeDragId === deal.id}
              <Card.Root 
                class="cursor-grab active:cursor-grabbing hover:border-border transition-all duration-150 shadow-2xs select-none {isDragging ? 'opacity-40 scale-95' : ''}"
                draggable="true"
                ondragstart={(e: DragEvent) => handleDragStart(e, deal.id!)}
                ondragend={handleDragEnd}
                role="listitem"
              >
                <Card.Content class="p-3 space-y-2">
                  <div class="flex items-start justify-between gap-2">
                    <h4 class="text-xs font-semibold text-foreground leading-snug">{deal.title}</h4>
                    <GripVertical class="w-3.5 h-3.5 text-muted-foreground/50 shrink-0 mt-0.5" />
                  </div>
                  <div class="flex items-center justify-between pt-1 border-t border-border/40 text-[11px]">
                    <span class="font-bold text-emerald-600 flex items-center gap-0.5">
                      ${deal.value.toLocaleString()}
                    </span>
                    {#if deal.created_at}
                      <span class="text-muted-foreground text-[10px] flex items-center gap-1">
                        <Calendar class="w-3 h-3 text-muted-foreground/60" />
                        {new Date(deal.created_at).toLocaleDateString('en-US', { month: 'short', day: 'numeric' })}
                      </span>
                    {/if}
                  </div>
                </Card.Content>
              </Card.Root>
            {/each}
          {/if}
        </div>
      </div>
    {/each}
  </div>
</div>
