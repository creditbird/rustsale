<script lang="ts">
  import { onMount } from 'svelte';
  import { toast } from 'svelte-sonner';
  import * as crmService from '$lib/services/crmService';
  import type { Deal, Lead } from '$lib/services/crmService';
  import * as Card from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import {
    GripVertical, Inbox, AlertCircle, Calendar, Plus, Trash2, X,
    Building, DollarSign, Search, UserCheck, TrendingUp, Layers, CheckCircle2,
    ChevronDown, User
  } from 'lucide-svelte';
  import { m, formatCurrency, formatCompactCurrency, formatDate } from '$lib/i18n';

  let deals = $state<Deal[]>([]);
  let leads = $state<Lead[]>([]);
  let errorMsg = $state<string | null>(null);
  let activeDragId = $state<number | null>(null);
  let dragOverColumn = $state<string | null>(null);

  // Search & Filter State
  let searchQuery = $state('');

  // Create Deal Modal State
  let showCreateModal = $state(false);
  let selectedLead = $state<Lead | null>(null);
  let leadSearchQuery = $state('');
  let isLeadDropdownOpen = $state(false);

  let newTitle = $state('');
  let newValue = $state(10000000);
  let newStage = $state('Prospecting');
  let creatingDeal = $state(false);
  let createErrorMsg = $state<string | null>(null);

  const stageConfig: Record<string, { label: string; color: string; bgLight: string; border: string; barColor: string; prob: string }> = {
    get 'Prospecting'() { return { label: m.pipeline_stage_prospecting(), color: 'text-blue-500', bgLight: 'bg-blue-500/10', border: 'border-blue-500/30', barColor: 'bg-blue-500', prob: '20%' }; },
    get 'Demo'() { return { label: m.pipeline_stage_demo(), color: 'text-purple-500', bgLight: 'bg-purple-500/10', border: 'border-purple-500/30', barColor: 'bg-purple-500', prob: '40%' }; },
    get 'Proposal'() { return { label: m.pipeline_stage_proposal(), color: 'text-amber-500', bgLight: 'bg-amber-500/10', border: 'border-amber-500/30', barColor: 'bg-amber-500', prob: '60%' }; },
    get 'Negotiation'() { return { label: m.pipeline_stage_negotiation(), color: 'text-indigo-500', bgLight: 'bg-indigo-500/10', border: 'border-indigo-500/30', barColor: 'bg-indigo-500', prob: '80%' }; },
    get 'Won'() { return { label: m.pipeline_stage_won(), color: 'text-emerald-500', bgLight: 'bg-emerald-500/10', border: 'border-emerald-500/30', barColor: 'bg-emerald-500', prob: '100%' }; }
  };

  const stages = ['Prospecting', 'Demo', 'Proposal', 'Negotiation', 'Won'];

  const presetValues = [5000000, 10000000, 25000000, 50000000, 100000000];

  async function loadDeals() {
    try {
      deals = await crmService.getDeals();
      errorMsg = null;
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    }
  }

  async function loadLeads() {
    try {
      leads = await crmService.getLeads();
    } catch (err) {
      console.error('Failed to load leads for deal creation:', err);
    }
  }

  async function openCreateModal(defaultStage = 'Prospecting', preselectedLead?: Lead) {
    createErrorMsg = null;
    await loadLeads();
    newStage = defaultStage;
    leadSearchQuery = '';
    isLeadDropdownOpen = false;

    if (preselectedLead) {
      selectLead(preselectedLead);
    } else if (leads.length > 0) {
      selectLead(leads[0]);
    } else {
      selectedLead = null;
      newTitle = '';
    }

    newValue = 10000000;
    showCreateModal = true;
  }

  function clearSelectedLead() {
    selectedLead = null;
    leadSearchQuery = '';
    isLeadDropdownOpen = true;
  }

  function selectLead(lead: Lead) {
    selectedLead = lead;
    leadSearchQuery = `${lead.name} (${lead.company})`;
    isLeadDropdownOpen = false;
    // Auto-generate title if empty or default
    if (!newTitle || newTitle.startsWith('Deal -') || newTitle.startsWith('Contract -')) {
      newTitle = `Deal - ${lead.company || lead.name}`;
    }
  }

  let filteredModalLeads = $derived(
    leads.filter(l => {
      const q = leadSearchQuery.toLowerCase().trim();
      if (!q) return true;
      return (
        l.name.toLowerCase().includes(q) ||
        l.company.toLowerCase().includes(q) ||
        (l.email && l.email.toLowerCase().includes(q)) ||
        (l.phone && l.phone.includes(q))
      );
    })
  );

  let filteredDeals = $derived(
    deals.filter(d => {
      if (!searchQuery.trim()) return true;
      const q = searchQuery.toLowerCase().trim();
      const lead = leads.find(l => l.id === d.lead_id);
      return (
        d.title.toLowerCase().includes(q) ||
        d.stage.toLowerCase().includes(q) ||
        (lead && (lead.name.toLowerCase().includes(q) || lead.company.toLowerCase().includes(q)))
      );
    })
  );

  // Pipeline metrics
  let totalPipelineValue = $derived(deals.reduce((sum, d) => sum + (d.value || 0), 0));
  let wonDealsValue = $derived(deals.filter(d => d.stage === 'Won').reduce((sum, d) => sum + (d.value || 0), 0));
  let avgDealSize = $derived(deals.length > 0 ? totalPipelineValue / deals.length : 0);

  async function handleCreateDeal(e: Event) {
    e.preventDefault();
    if (!selectedLead || !selectedLead.id) {
      createErrorMsg = 'Please select an associated lead.';
      return;
    }
    if (!newTitle.trim()) {
      createErrorMsg = 'Please enter a deal title.';
      return;
    }

    creatingDeal = true;
    createErrorMsg = null;
    try {
      const createdTitle = newTitle.trim();
      await crmService.createDeal(selectedLead.id, createdTitle, Number(newValue) || 0, newStage);
      showCreateModal = false;
      toast.success(m.kanban_deal_created_toast(), {
        description: `Created opportunity "${createdTitle}" for ${selectedLead.name}.`
      });
      await loadDeals();
    } catch (err) {
      createErrorMsg = err instanceof Error ? err.message : String(err);
      toast.error(m.common_error(), {
        description: createErrorMsg
      });
    } finally {
      creatingDeal = false;
    }
  }

  // Delete Deal Modal State
  let showDeleteModal = $state(false);
  let dealToDelete = $state<{ id: number; title: string } | null>(null);
  let deletingDeal = $state(false);

  function promptDeleteDeal(id: number, title: string) {
    dealToDelete = { id, title };
    showDeleteModal = true;
  }

  async function confirmDeleteDeal() {
    if (!dealToDelete) return;
    deletingDeal = true;
    const deletedTitle = dealToDelete.title;
    try {
      await crmService.deleteDeal(dealToDelete.id);
      showDeleteModal = false;
      dealToDelete = null;
      toast.success(m.kanban_deal_deleted_toast(), {
        description: `Deleted opportunity "${deletedTitle}".`
      });
      await loadDeals();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
      toast.error(m.common_error(), {
        description: errorMsg
      });
    } finally {
      deletingDeal = false;
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

  onMount(() => {
    loadDeals();
    loadLeads();
  });
</script>

<div class="space-y-4 h-full flex flex-col">
  <!-- Top Metrics Overview Bar -->
  <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
    <div class="p-3.5 rounded-xl border border-border bg-card/70 flex items-center gap-3 shadow-2xs">
      <div class="p-2.5 rounded-lg bg-primary/10 text-primary">
        <DollarSign class="w-4 h-4" />
      </div>
      <div>
        <p class="text-[11px] text-muted-foreground font-medium">{m.pipeline_total_value()}</p>
        <p class="text-base font-extrabold text-foreground tracking-tight">{formatCurrency(totalPipelineValue, 'VND')}</p>
      </div>
    </div>

    <div class="p-3.5 rounded-xl border border-border bg-card/70 flex items-center gap-3 shadow-2xs">
      <div class="p-2.5 rounded-lg bg-blue-500/10 text-blue-500">
        <Layers class="w-4 h-4" />
      </div>
      <div>
        <p class="text-[11px] text-muted-foreground font-medium">{m.dashboard_deals_overview()}</p>
        <p class="text-base font-extrabold text-foreground tracking-tight">{m.pipeline_deals_count({ count: deals.length })}</p>
      </div>
    </div>

    <div class="p-3.5 rounded-xl border border-border bg-card/70 flex items-center gap-3 shadow-2xs">
      <div class="p-2.5 rounded-lg bg-emerald-500/10 text-emerald-500">
        <TrendingUp class="w-4 h-4" />
      </div>
      <div>
        <p class="text-[11px] text-muted-foreground font-medium">{m.pipeline_won_value()}</p>
        <p class="text-base font-extrabold text-emerald-600 tracking-tight">{formatCurrency(wonDealsValue, 'VND')}</p>
      </div>
    </div>

    <div class="p-3.5 rounded-xl border border-border bg-card/70 flex items-center gap-3 shadow-2xs">
      <div class="p-2.5 rounded-lg bg-indigo-500/10 text-indigo-500">
        <UserCheck class="w-4 h-4" />
      </div>
      <div>
        <p class="text-[11px] text-muted-foreground font-medium">{m.pipeline_avg_deal()}</p>
        <p class="text-base font-extrabold text-foreground tracking-tight">{formatCurrency(Math.round(avgDealSize), 'VND')}</p>
      </div>
    </div>
  </div>

  <!-- Header & Toolbar -->
  <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 bg-card/40 p-3 rounded-xl border border-border">
    <div class="relative flex-1 max-w-sm">
      <Search class="w-4 h-4 absolute left-3 top-2.5 text-muted-foreground" />
      <Input
        bind:value={searchQuery}
        placeholder={m.kanban_search_placeholder()}
        class="pl-9 h-9 text-xs bg-background"
      />
    </div>

    <div class="flex items-center gap-2 self-end sm:self-auto">
      <Button onclick={() => openCreateModal('Prospecting')} size="sm" class="gap-1.5 font-semibold shadow-xs bg-primary hover:bg-primary/90 text-primary-foreground">
        <Plus class="w-4 h-4" />
        <span>{m.kanban_add_deal()}</span>
      </Button>
    </div>
  </div>

  {#if errorMsg}
    <div class="p-3 rounded-lg border border-destructive/30 bg-destructive/10 text-destructive text-xs flex items-center gap-2">
      <AlertCircle class="w-4 h-4 shrink-0" />
      <span>{errorMsg}</span>
    </div>
  {/if}

  <!-- Kanban Column Grid -->
  <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-4 flex-1 overflow-x-auto min-h-0 pb-2">
    {#each stages as stage}
      {@const cfg = stageConfig[stage]}
      {@const stageDeals = filteredDeals.filter(d => d.stage === stage)}
      {@const totalValue = stageDeals.reduce((sum, d) => sum + (d.value || 0), 0)}
      {@const isOver = dragOverColumn === stage}

      <div
        class="rounded-xl border border-border bg-card/50 flex flex-col gap-2.5 min-w-[210px] transition-all duration-150 overflow-hidden {isOver ? 'bg-accent/40 border-primary/40 shadow-md ring-1 ring-primary/20' : ''}"
        ondragover={(e) => handleDragOver(e, stage)}
        ondragleave={(e) => handleDragLeave(e, stage)}
        ondrop={(e) => handleDrop(e, stage)}
        role="region"
        aria-label="{cfg.label} stage column"
      >
        <!-- Column Top Indicator Bar -->
        <div class="h-1.5 w-full {cfg.barColor}"></div>

        <!-- Column Header -->
        <div class="px-3 pt-1 pb-2 flex flex-col gap-1 border-b border-border/60">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-1.5">
              <span class="text-xs font-bold uppercase tracking-wider text-foreground">{cfg.label}</span>
              <span class="text-[10px] font-semibold text-muted-foreground">({cfg.prob})</span>
            </div>
            <div class="flex items-center gap-1">
              <Badge variant="secondary" class="text-[10px] px-1.5 py-0 font-bold">{stageDeals.length}</Badge>
              <button
                type="button"
                onclick={() => openCreateModal(stage)}
                class="p-1 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                title="{m.kanban_add_deal()} - {cfg.label}"
              >
                <Plus class="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
          <div class="flex items-center justify-between mt-0.5">
            <span class="text-xs font-extrabold text-emerald-600">{formatCurrency(totalValue, 'VND')}</span>
          </div>
        </div>

        <!-- Deal Cards List -->
        <div class="flex-1 overflow-y-auto space-y-2.5 px-2.5 pb-2.5" role="list">
          {#if stageDeals.length === 0}
            <div class="h-32 border border-dashed border-border/70 rounded-xl flex flex-col items-center justify-center text-muted-foreground/60 p-4 text-center bg-muted/20">
              <Inbox class="w-6 h-6 stroke-1 mb-1 text-muted-foreground/40" />
              <span class="text-[11px] font-medium text-muted-foreground">{m.kanban_empty_column()}</span>
            </div>
          {:else}
            {#each stageDeals as deal}
              {@const isDragging = activeDragId === deal.id}
              {@const lead = leads.find(l => l.id === deal.lead_id)}
              <Card.Root
                class="group cursor-grab active:cursor-grabbing hover:border-primary/40 hover:shadow-md transition-all duration-150 select-none relative bg-card border-border/80 rounded-xl overflow-hidden {isDragging ? 'opacity-40 scale-95' : ''}"
                draggable="true"
                ondragstart={(e: DragEvent) => handleDragStart(e, deal.id!)}
                ondragend={handleDragEnd}
                role="listitem"
              >
                <Card.Content class="p-3 space-y-2">
                  <div class="flex items-start justify-between gap-1.5">
                    <div class="space-y-1 min-w-0">
                      <h4 class="text-xs font-bold text-foreground leading-snug break-words group-hover:text-primary transition-colors">{deal.title}</h4>
                      {#if lead}
                        <div class="flex items-center gap-1.5 text-[11px] text-muted-foreground font-medium truncate">
                          <div class="w-4 h-4 rounded-full bg-primary/10 text-primary flex items-center justify-center text-[9px] font-bold shrink-0">
                            {lead.name.charAt(0).toUpperCase()}
                          </div>
                          <span class="truncate font-semibold text-foreground/80">{lead.name}</span>
                          {#if lead.company}
                            <span class="text-[10px] text-muted-foreground shrink-0">• {lead.company}</span>
                          {/if}
                        </div>
                      {/if}
                    </div>
                    <div class="flex items-center gap-0.5 shrink-0 opacity-80 group-hover:opacity-100 transition-opacity">
                      <button
                        type="button"
                        onclick={() => promptDeleteDeal(deal.id!, deal.title)}
                        class="opacity-0 group-hover:opacity-100 p-1 text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-all rounded-md"
                        title={m.common_delete()}
                      >
                        <Trash2 class="w-3.5 h-3.5" />
                      </button>
                      <GripVertical class="w-3.5 h-3.5 text-muted-foreground/40 shrink-0 cursor-grab" />
                    </div>
                  </div>

                  <div class="flex items-center justify-between pt-2 border-t border-border/40 text-[11px]">
                    <span class="font-extrabold text-emerald-600 text-xs flex items-center gap-0.5">
                      {formatCurrency(deal.value, 'VND')}
                    </span>
                    {#if deal.created_at}
                      <span class="text-muted-foreground text-[10px] flex items-center gap-1 font-medium">
                        <Calendar class="w-3 h-3 text-muted-foreground/60" />
                        {formatDate(deal.created_at, { month: 'numeric', day: 'numeric' })}
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

<!-- Modern Redesigned Create New Deal Modal -->
{#if showCreateModal}
  <div class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-lg overflow-hidden animate-in fade-in zoom-in-95 duration-150">

      <!-- Modal Header -->
      <div class="px-6 py-4 border-b border-border flex items-center justify-between bg-muted/40">
        <div class="flex items-center gap-2.5">
          <div class="w-8 h-8 rounded-xl bg-primary/10 text-primary flex items-center justify-center shadow-2xs">
            <Plus class="w-4 h-4" />
          </div>
          <div>
            <h3 class="font-bold text-base text-foreground">{m.kanban_create_deal_title()}</h3>
            <p class="text-xs text-muted-foreground">{m.kanban_create_deal_subtitle()}</p>
          </div>
        </div>
        <button
          type="button"
          onclick={() => showCreateModal = false}
          class="text-muted-foreground hover:text-foreground p-1.5 rounded-lg hover:bg-accent transition-colors"
          aria-label={m.common_close()}
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <form onsubmit={handleCreateDeal} class="p-6 space-y-5">
        {#if createErrorMsg}
          <div class="p-3 rounded-xl bg-destructive/10 border border-destructive/20 text-destructive text-xs flex items-center gap-2">
            <AlertCircle class="w-4 h-4 shrink-0" />
            <span>{createErrorMsg}</span>
          </div>
        {/if}

        <!-- 1. Searchable Lead Selector -->
        <div class="space-y-1.5 relative">
          <label for="lead-search-input" class="text-xs font-bold text-foreground flex items-center gap-1.5">
            <User class="w-3.5 h-3.5 text-primary" />
            {m.kanban_select_lead_label()} <span class="text-destructive">*</span>
          </label>

          <div class="relative">
            <Search class="w-4 h-4 absolute left-3 top-2.5 text-muted-foreground" />
            <input
              id="lead-search-input"
              type="text"
              bind:value={leadSearchQuery}
              onfocus={() => isLeadDropdownOpen = true}
              oninput={() => isLeadDropdownOpen = true}
              placeholder={m.kanban_lead_search_placeholder()}
              class="w-full h-10 pl-9 pr-8 rounded-xl border border-input bg-background text-xs text-foreground focus:outline-none focus:ring-2 focus:ring-primary/40 font-medium"
            />
            <button
              type="button"
              onclick={() => isLeadDropdownOpen = !isLeadDropdownOpen}
              class="absolute right-2.5 top-2.5 text-muted-foreground hover:text-foreground"
            >
              <ChevronDown class="w-4 h-4" />
            </button>
          </div>

          <!-- Dropdown Search Results List -->
          {#if isLeadDropdownOpen}
            <div class="absolute left-0 right-0 top-full mt-1 z-20 max-h-48 overflow-y-auto bg-popover border border-border rounded-xl shadow-xl py-1 divide-y divide-border/40">
              {#if filteredModalLeads.length === 0}
                <div class="px-4 py-3 text-xs text-muted-foreground text-center">
                  {m.kanban_no_leads_found()}
                </div>
              {:else}
                {#each filteredModalLeads as lead}
                  <button
                    type="button"
                    onclick={() => selectLead(lead)}
                    class="w-full px-3.5 py-2 text-left hover:bg-accent/70 transition-colors flex items-center justify-between gap-2 {selectedLead?.id === lead.id ? 'bg-primary/10 text-primary font-bold' : ''}"
                  >
                    <div class="min-w-0">
                      <p class="text-xs font-semibold text-foreground truncate">{lead.name}</p>
                      <p class="text-[10px] text-muted-foreground truncate">{lead.company} {lead.phone ? `• ${lead.phone}` : ''}</p>
                    </div>
                    {#if selectedLead?.id === lead.id}
                      <CheckCircle2 class="w-4 h-4 text-primary shrink-0" />
                    {/if}
                  </button>
                {/each}
              {/if}
            </div>
          {/if}
        </div>

        <!-- Selected Lead Banner Card -->
        {#if selectedLead}
          <div class="p-3 rounded-xl bg-primary/5 border border-primary/20 flex items-center justify-between">
            <div class="flex items-center gap-2.5">
              <div class="w-8 h-8 rounded-full bg-primary text-primary-foreground font-bold flex items-center justify-center text-xs shadow-2xs">
                {selectedLead.name.charAt(0).toUpperCase()}
              </div>
              <div>
                <p class="text-xs font-bold text-foreground">{selectedLead.name}</p>
                <p class="text-[11px] text-muted-foreground">{selectedLead.company} • {selectedLead.email}</p>
              </div>
            </div>
            <div class="flex items-center gap-2">
              <Badge variant="outline" class="text-[10px] bg-background">{m.common_info()}</Badge>
              <button
                type="button"
                onclick={clearSelectedLead}
                class="px-2 py-1 rounded-lg bg-destructive/10 hover:bg-destructive/20 text-destructive transition-colors text-[11px] font-semibold flex items-center gap-1 border border-destructive/20"
                title={m.kanban_deselect_lead()}
              >
                <X class="w-3.5 h-3.5" />
                <span>{m.kanban_deselect_lead()}</span>
              </button>
            </div>
          </div>
        {/if}

        <!-- 2. Deal Title Input -->
        <div class="space-y-1.5">
          <label for="deal-title-input" class="text-xs font-bold text-foreground flex items-center gap-1.5">
            <Building class="w-3.5 h-3.5 text-primary" />
            {m.kanban_deal_title_label()} <span class="text-destructive">*</span>
          </label>
          <Input
            id="deal-title-input"
            bind:value={newTitle}
            placeholder={m.kanban_deal_title_placeholder()}
            class="h-10 text-xs rounded-xl"
            required
          />
        </div>

        <!-- 3. Value & Stage Row -->
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div class="space-y-1.5">
            <label for="deal-value-input" class="text-xs font-bold text-foreground flex items-center gap-1.5">
              <DollarSign class="w-3.5 h-3.5 text-emerald-600" />
              {m.kanban_deal_value_label()} (₫) <span class="text-destructive">*</span>
            </label>
            <Input
              id="deal-value-input"
              type="number"
              bind:value={newValue}
              placeholder="10000000"
              class="h-10 text-xs rounded-xl font-bold text-emerald-600"
              min="0"
              step="500000"
              required
            />

            <!-- Quick Value Presets (VND) -->
            <div class="flex items-center gap-1 pt-1 flex-wrap">
              {#each presetValues as preset}
                <button
                  type="button"
                  onclick={() => newValue = preset}
                  class="text-[10px] font-semibold px-2 py-0.5 rounded-lg border border-border hover:border-emerald-500 hover:bg-emerald-500/10 hover:text-emerald-600 transition-colors"
                >
                  {formatCompactCurrency(preset, 'VND')}
                </button>
              {/each}
            </div>
          </div>

          <div class="space-y-1.5">
            <label for="deal-stage-select" class="text-xs font-bold text-foreground flex items-center gap-1.5">
              <Layers class="w-3.5 h-3.5 text-primary" />
              {m.kanban_initial_stage_label()}
            </label>
            <select
              id="deal-stage-select"
              bind:value={newStage}
              class="w-full h-10 px-3 rounded-xl border border-input bg-background text-xs font-semibold text-foreground focus:outline-none focus:ring-2 focus:ring-primary/40"
            >
              {#each stages as stage}
                <option value={stage}>{stageConfig[stage].label} ({stageConfig[stage].prob})</option>
              {/each}
            </select>
          </div>
        </div>

        <!-- Footer Action Buttons -->
        <div class="pt-4 border-t border-border flex items-center justify-end gap-3">
          <Button
            type="button"
            variant="outline"
            size="default"
            onclick={() => showCreateModal = false}
            disabled={creatingDeal}
            class="rounded-xl h-10 px-5 text-xs font-semibold"
          >
            {m.common_cancel()}
          </Button>
          <Button
            type="submit"
            size="default"
            disabled={creatingDeal || !selectedLead}
            class="rounded-xl h-10 px-6 text-xs font-bold bg-primary text-primary-foreground hover:bg-primary/90 shadow-md"
          >
            {creatingDeal ? m.kanban_creating_btn() : m.kanban_create_btn()}
          </Button>
        </div>
      </form>
    </div>
  </div>
{/if}

<!-- Custom Styled Delete Deal Confirmation Modal -->
{#if showDeleteModal && dealToDelete}
  <div class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-md overflow-hidden animate-in fade-in zoom-in-95 duration-150">
      <div class="p-6 text-center space-y-4">
        <div class="w-12 h-12 rounded-full bg-destructive/10 text-destructive flex items-center justify-center mx-auto shadow-2xs">
          <AlertCircle class="w-6 h-6" />
        </div>

        <div class="space-y-1.5">
          <h3 class="font-bold text-base text-foreground">{m.kanban_delete_deal_title()}</h3>
          <p class="text-xs text-muted-foreground leading-relaxed">
            {m.kanban_delete_deal_prompt({ title: dealToDelete.title })}
          </p>
        </div>

        <div class="pt-2 flex items-center justify-center gap-3">
          <Button
            type="button"
            variant="outline"
            size="default"
            onclick={() => { showDeleteModal = false; dealToDelete = null; }}
            disabled={deletingDeal}
            class="rounded-xl h-10 px-5 text-xs font-semibold"
          >
            {m.common_cancel()}
          </Button>
          <Button
            type="button"
            variant="destructive"
            size="default"
            onclick={confirmDeleteDeal}
            disabled={deletingDeal}
            class="rounded-xl h-10 px-6 text-xs font-bold shadow-md bg-destructive text-destructive-foreground hover:bg-destructive/90"
          >
            {deletingDeal ? m.kanban_deleting_btn() : m.kanban_delete_btn()}
          </Button>
        </div>
      </div>
    </div>
  </div>
{/if}
