<script lang="ts">
  import type { Lead } from '$lib/services/crmService';
  import * as crmService from '$lib/services/crmService';
  import LeadDetails from './LeadDetails.svelte';
  import * as Table from '$lib/components/ui/table';
  import { Input } from '$lib/components/ui/input';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Search, Plus, FolderPlus, CheckCircle2, XCircle, X, ChevronLeft, ChevronRight, ExternalLink } from 'lucide-svelte';

  let { leads = [], onRefresh } = $props<{ leads: Lead[]; onRefresh?: () => void }>();
  let selectedLeadId = $state<number | null>(null);

  // Multiselect state
  let selectedRows = $state<Set<number>>(new Set());
  
  // Category assign modal states
  let showCategoryModal = $state(false);
  let categories = $state<crmService.Category[]>([]);
  let selectedCategoryId = $state<number | null>(null);
  let assigningCategory = $state(false);
  let assignErrorMsg = $state<string | null>(null);
  let assignSuccessMsg = $state<string | null>(null);

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
  let filterValueRange = $state('');
  let filterZaloStatus = $state('');
  let filterOutreachStatus = $state('');
  let currentPage = $state(1);
  let pageSize = $state(10);

  // Derived variables
  let uniqueStatuses = $derived(Array.from(new Set(leads.map((l: Lead) => l.status))));
  let uniqueOwners = $derived(Array.from(new Set(leads.map((l: Lead) => l.owner).filter(Boolean))) as string[]);

  let filteredLeads = $derived(
    leads.filter((lead: Lead) => {
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
      
      if (filterStatus && lead.status !== filterStatus) return false;
      if (filterOwner && lead.owner !== filterOwner) return false;
      
      if (filterValueRange) {
        const val = lead.value || 0;
        if (filterValueRange === 'under_10k' && val >= 10000) return false;
        if (filterValueRange === '10k_to_50k' && (val < 10000 || val > 50000)) return false;
        if (filterValueRange === 'above_50k' && val <= 50000) return false;
      }

      if (filterZaloStatus) {
        if (filterZaloStatus === 'verified' && lead.has_zalo !== 1) return false;
        if (filterZaloStatus === 'no_zalo' && lead.has_zalo !== 0) return false;
        if (filterZaloStatus === 'unchecked' && lead.has_zalo !== null && lead.has_zalo !== undefined) return false;
      }

      if (filterOutreachStatus) {
        if (filterOutreachStatus === 'reached' && lead.zalo_reached !== 1) return false;
        if (filterOutreachStatus === 'not_reached' && lead.zalo_reached === 1) return false;
      }
      
      return true;
    })
  );

  $effect(() => {
    const _s = searchQuery;
    const _st = filterStatus;
    const _o = filterOwner;
    const _v = filterValueRange;
    const _z = filterZaloStatus;
    const _or = filterOutreachStatus;
    const _p = pageSize;
    currentPage = 1;
    selectedRows = new Set();
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
      name = '';
      company = '';
      value = 0;
      owner = 'Self';
      email = '';
      phone = '';
      about = '';
      showAddModal = false;
      if (onRefresh) onRefresh();
    } catch (err) {
      modalErrorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      saving = false;
    }
  }

  async function openCategoryModal() {
    if (selectedRows.size === 0) return;
    try {
      categories = await crmService.getCategories();
      selectedCategoryId = categories.length > 0 ? categories[0].id! : null;
      assignErrorMsg = null;
      assignSuccessMsg = null;
      showCategoryModal = true;
    } catch (err) {
      console.error('Failed to load categories', err);
    }
  }

  async function handleAssignCategory() {
    if (!selectedCategoryId || selectedRows.size === 0) return;
    assigningCategory = true;
    assignErrorMsg = null;
    try {
      await crmService.addLeadsToCategory(Array.from(selectedRows), selectedCategoryId);
      assignSuccessMsg = 'Successfully assigned leads to category.';
      setTimeout(() => {
        showCategoryModal = false;
        selectedRows.clear();
        selectedRows = new Set(selectedRows);
        if (onRefresh) onRefresh();
      }, 1200);
    } catch (err) {
      assignErrorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      assigningCategory = false;
    }
  }

  function toggleRowSelection(id: number) {
    if (selectedRows.has(id)) selectedRows.delete(id);
    else selectedRows.add(id);
    selectedRows = new Set(selectedRows);
  }

  function toggleAllRows(checked: boolean) {
    if (checked) {
      paginatedLeads.forEach((l: Lead) => { if (l.id) selectedRows.add(l.id); });
    } else {
      paginatedLeads.forEach((l: Lead) => { if (l.id) selectedRows.delete(l.id); });
    }
    selectedRows = new Set(selectedRows);
  }
</script>

<div class="flex flex-col gap-4">
  <!-- Top Action Header -->
  <div class="flex items-center justify-between">
    <div>
      <h2 class="text-xl font-bold text-foreground tracking-tight">Leads & Contacts</h2>
      <p class="text-xs text-muted-foreground mt-0.5">Click any row to view lead profile & timeline, or click checkbox to select</p>
    </div>
    <div class="flex items-center gap-2">
      {#if selectedRows.size > 0}
        <Button variant="outline" size="sm" onclick={openCategoryModal} class="gap-2 border-emerald-500/40 text-emerald-600 hover:bg-emerald-500/10">
          <FolderPlus class="size-4" />
          Assign to Category ({selectedRows.size})
        </Button>
      {/if}
      <Button size="sm" onclick={() => { showAddModal = true; modalErrorMsg = null; }} class="gap-2 bg-primary text-primary-foreground hover:bg-primary/90">
        <Plus class="size-4" />
        Add Lead
      </Button>
    </div>
  </div>

  <!-- Search & Filter Controls -->
  <div class="p-3 rounded-lg border border-border bg-card flex flex-wrap items-center justify-between gap-3">
    <!-- Search Bar -->
    <div class="relative flex-1 min-w-[220px] max-w-md">
      <Search class="size-4 absolute left-3 top-2.5 text-muted-foreground" />
      <Input
        type="text"
        placeholder="Search name, company, email, owner..."
        bind:value={searchQuery}
        class="pl-9 text-xs h-9"
      />
    </div>

    <!-- Filter Dropdowns -->
    <div class="flex flex-wrap items-center gap-2">
      {#if uniqueStatuses.length > 1}
        <select bind:value={filterStatus} class="h-9 px-3 text-xs rounded-md border border-input bg-background text-foreground focus:outline-hidden cursor-pointer">
          <option value="">All Statuses</option>
          {#each uniqueStatuses as status (status)}
            <option value={status}>{status}</option>
          {/each}
        </select>
      {/if}

      <select bind:value={filterOwner} class="h-9 px-3 text-xs rounded-md border border-input bg-background text-foreground focus:outline-hidden cursor-pointer">
        <option value="">All Owners</option>
        {#each uniqueOwners as owner (owner)}
          <option value={owner}>{owner}</option>
        {/each}
      </select>

      <select bind:value={filterValueRange} class="h-9 px-3 text-xs rounded-md border border-input bg-background text-foreground focus:outline-hidden cursor-pointer">
        <option value="">All Values</option>
        <option value="under_10k">Under $10k</option>
        <option value="10k_to_50k">$10k - $50k</option>
        <option value="above_50k">Above $50k</option>
      </select>

      <select bind:value={filterZaloStatus} class="h-9 px-3 text-xs rounded-md border border-input bg-background text-foreground focus:outline-hidden cursor-pointer">
        <option value="">All Zalo Statuses</option>
        <option value="verified">Verified Zalo</option>
        <option value="no_zalo">No Zalo</option>
        <option value="unchecked">Unchecked</option>
      </select>

      {#if searchQuery || filterStatus || filterOwner || filterValueRange || filterZaloStatus || filterOutreachStatus}
        <Button
          variant="ghost"
          size="sm"
          onclick={() => { searchQuery = ''; filterStatus = ''; filterOwner = ''; filterValueRange = ''; filterZaloStatus = ''; filterOutreachStatus = ''; }}
          class="h-9 px-2 text-xs gap-1 text-muted-foreground hover:text-foreground"
        >
          <X class="size-3.5" /> Clear
        </Button>
      {/if}
    </div>
  </div>

  <!-- Data Table -->
  <div class="rounded-md border border-border bg-card overflow-hidden">
    <Table.Root>
      <Table.Header class="bg-muted/40">
        <Table.Row>
          <Table.Head class="w-10 text-center">
            <div class="flex items-center justify-center">
              <Checkbox 
                checked={paginatedLeads.length > 0 && paginatedLeads.every((l: Lead) => l.id && selectedRows.has(l.id))}
                onCheckedChange={(checked) => toggleAllRows(Boolean(checked))}
              />
            </div>
          </Table.Head>
          <Table.Head>Name</Table.Head>
          <Table.Head>Company</Table.Head>
          <Table.Head>Phone</Table.Head>
          <Table.Head>Email</Table.Head>
          <Table.Head>Zalo</Table.Head>
          <Table.Head>Status</Table.Head>
          <Table.Head class="text-right">Value</Table.Head>
          <Table.Head>Owner</Table.Head>
        </Table.Row>
      </Table.Header>
      <Table.Body>
        {#if paginatedLeads.length === 0}
          <Table.Row>
            <Table.Cell colspan={9} class="text-center py-8 text-xs text-muted-foreground">
              No matching lead records found.
            </Table.Cell>
          </Table.Row>
        {:else}
          {#each paginatedLeads as lead (lead.id)}
            {@const isSelected = lead.id ? selectedRows.has(lead.id) : false}
            <Table.Row 
              class="cursor-pointer hover:bg-muted/50 transition-colors {isSelected ? 'bg-primary/5' : ''}"
              onclick={() => { if (lead.id !== undefined) selectedLeadId = lead.id; }}
            >
              <Table.Cell class="text-center" onclick={(e: Event) => e.stopPropagation()}>
                <div class="flex items-center justify-center">
                  <Checkbox 
                    checked={isSelected}
                    onCheckedChange={() => lead.id && toggleRowSelection(lead.id)}
                  />
                </div>
              </Table.Cell>
              <Table.Cell class="font-medium text-foreground">
                <div class="flex items-center gap-2.5">
                  <div class="size-7 rounded-full bg-emerald-500/10 text-emerald-600 font-bold text-xs flex items-center justify-center border border-emerald-500/20">
                    {lead.name.split(' ').map((n: string) => n[0]).join('').toUpperCase().substring(0, 2)}
                  </div>
                  <span class="font-semibold text-xs text-foreground">{lead.name}</span>
                </div>
              </Table.Cell>
              <Table.Cell class="text-xs text-muted-foreground">{lead.company}</Table.Cell>
              <Table.Cell class="text-xs text-muted-foreground">{lead.phone || '—'}</Table.Cell>
              <Table.Cell class="text-xs text-muted-foreground">{lead.email || '—'}</Table.Cell>
              <Table.Cell>
                {#if lead.has_zalo === 1}
                  <Badge variant="outline" class="gap-1 border-sky-500/30 text-sky-600 bg-sky-500/10 text-[10px] py-0">
                    <CheckCircle2 class="size-3 text-sky-500" /> Verified
                  </Badge>
                {:else if lead.has_zalo === 0}
                  <Badge variant="outline" class="gap-1 border-slate-300 text-slate-500 bg-slate-100 text-[10px] py-0">
                    <XCircle class="size-3 text-slate-400" /> No Zalo
                  </Badge>
                {:else}
                  <span class="text-xs text-muted-foreground/50">—</span>
                {/if}
              </Table.Cell>
              <Table.Cell>
                <Badge variant={lead.status === 'Converted' ? 'default' : 'secondary'} class="text-[10px] py-0">
                  {lead.status}
                </Badge>
              </Table.Cell>
              <Table.Cell class="text-right font-bold text-xs text-foreground">
                ${(lead.value || 0).toLocaleString()}
              </Table.Cell>
              <Table.Cell class="text-xs text-muted-foreground">{lead.owner || '—'}</Table.Cell>
            </Table.Row>
          {/each}
        {/if}
      </Table.Body>
    </Table.Root>
  </div>

  <!-- Pagination Controls -->
  {#if totalEntries > 0}
    <div class="flex items-center justify-between px-2 py-1 text-xs text-muted-foreground">
      <span>Showing {showingStart} to {showingEnd} of {totalEntries} entries</span>
      
      <div class="flex items-center gap-2">
        <select bind:value={pageSize} class="h-8 px-2 text-xs rounded border border-input bg-background cursor-pointer">
          <option value={5}>5 / page</option>
          <option value={10}>10 / page</option>
          <option value={25}>25 / page</option>
        </select>

        <Button variant="outline" size="sm" class="size-8 p-0" disabled={currentPage === 1} onclick={() => currentPage--}>
          <ChevronLeft class="size-4" />
        </Button>
        <span class="font-medium text-foreground px-1">{currentPage} / {totalPages}</span>
        <Button variant="outline" size="sm" class="size-8 p-0" disabled={currentPage === totalPages} onclick={() => currentPage++}>
          <ChevronRight class="size-4" />
        </Button>
      </div>
    </div>
  {/if}

  <!-- Lead Details Modal/Drawer -->
  {#if selectedLeadId !== null}
    <LeadDetails leadId={selectedLeadId} onClose={() => selectedLeadId = null} onRefresh={onRefresh} />
  {/if}
</div>

<!-- Add Lead Modal -->
{#if showAddModal}
  <div class="fixed inset-0 z-50 bg-background/80 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="w-full max-w-lg rounded-xl border border-border bg-card p-6 shadow-lg flex flex-col gap-4">
      <div class="flex items-center justify-between border-b border-border pb-3">
        <h3 class="text-base font-bold text-foreground">Create New Lead</h3>
        <Button variant="ghost" size="sm" onclick={() => showAddModal = false} class="size-8 p-0">
          <X class="size-4" />
        </Button>
      </div>

      {#if modalErrorMsg}
        <div class="p-3 rounded bg-destructive/10 border border-destructive/30 text-destructive text-xs">
          {modalErrorMsg}
        </div>
      {/if}

      <form onsubmit={handleAddLead} class="flex flex-col gap-4 text-xs">
        <div class="grid grid-cols-2 gap-3">
          <div class="flex flex-col gap-1">
            <label for="lead-name" class="font-medium text-foreground">Name *</label>
            <Input id="lead-name" bind:value={name} required placeholder="Full Name" />
          </div>
          <div class="flex flex-col gap-1">
            <label for="lead-company" class="font-medium text-foreground">Company *</label>
            <Input id="lead-company" bind:value={company} required placeholder="Company Name" />
          </div>
          <div class="flex flex-col gap-1">
            <label for="lead-email" class="font-medium text-foreground">Email *</label>
            <Input id="lead-email" type="email" bind:value={email} required placeholder="email@example.com" />
          </div>
          <div class="flex flex-col gap-1">
            <label for="lead-phone" class="font-medium text-foreground">Phone</label>
            <Input id="lead-phone" bind:value={phone} placeholder="Phone number" />
          </div>
          <div class="flex flex-col gap-1">
            <label for="lead-value" class="font-medium text-foreground">Value ($)</label>
            <Input id="lead-value" type="number" bind:value={value} min="0" />
          </div>
          <div class="flex flex-col gap-1">
            <label for="lead-owner" class="font-medium text-foreground">Owner</label>
            <Input id="lead-owner" bind:value={owner} placeholder="Owner Name" />
          </div>
        </div>

        <div class="flex flex-col gap-1">
          <label for="lead-about" class="font-medium text-foreground">Description</label>
          <textarea 
            id="lead-about" 
            bind:value={about} 
            rows="3" 
            placeholder="Lead background & details..." 
            class="w-full p-2.5 rounded-md border border-input bg-background text-xs text-foreground focus:outline-hidden"
          ></textarea>
        </div>

        <div class="flex items-center justify-end gap-2 pt-2 border-t border-border">
          <Button type="button" variant="outline" size="sm" onclick={() => showAddModal = false}>Cancel</Button>
          <Button type="submit" size="sm" disabled={saving}>
            {saving ? 'Saving...' : 'Save Lead'}
          </Button>
        </div>
      </form>
    </div>
  </div>
{/if}

<!-- Assign Category Modal -->
{#if showCategoryModal}
  <div class="fixed inset-0 z-50 bg-background/80 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="w-full max-w-sm rounded-xl border border-border bg-card p-5 shadow-lg flex flex-col gap-4">
      <div class="flex items-center justify-between border-b border-border pb-3">
        <h3 class="text-sm font-bold text-foreground">Assign to Category</h3>
        <Button variant="ghost" size="sm" onclick={() => showCategoryModal = false} class="size-7 p-0">
          <X class="size-4" />
        </Button>
      </div>

      {#if assignSuccessMsg}
        <div class="p-2.5 rounded bg-emerald-500/10 border border-emerald-500/30 text-emerald-600 text-xs">
          {assignSuccessMsg}
        </div>
      {/if}

      <div class="flex flex-col gap-3 text-xs">
        <p class="text-muted-foreground">Assign {selectedRows.size} selected lead(s) to:</p>
        <select bind:value={selectedCategoryId} class="w-full h-9 px-3 rounded-md border border-input bg-background text-foreground cursor-pointer">
          {#each categories as cat (cat.id)}
            <option value={cat.id}>{cat.name}</option>
          {/each}
        </select>
      </div>

      <div class="flex items-center justify-end gap-2 pt-2">
        <Button variant="outline" size="sm" onclick={() => showCategoryModal = false} disabled={assigningCategory}>Cancel</Button>
        <Button size="sm" onclick={handleAssignCategory} disabled={assigningCategory || categories.length === 0}>
          {assigningCategory ? 'Assigning...' : 'Assign Leads'}
        </Button>
      </div>
    </div>
  </div>
{/if}
