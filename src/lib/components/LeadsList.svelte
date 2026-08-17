<script lang="ts">
  import { untrack } from 'svelte';
  import type { Lead } from '$lib/services/crmService';
  import * as crmService from '$lib/services/crmService';
  import LeadDetails from './LeadDetails.svelte';
  import * as Table from '$lib/components/ui/table';
  import { Input } from '$lib/components/ui/input';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import * as Select from '$lib/components/ui/select';
  import { Search, Plus, FolderPlus, CheckCircle2, XCircle, X, ChevronLeft, ChevronRight, ExternalLink, Download } from 'lucide-svelte';
  import { toast } from 'svelte-sonner';
  import { exportLeadsToCsv } from '$lib/utils/leadsExportCsv';
  import { m, formatCurrency } from '$lib/i18n';

  let { leads = [], entityType = 'leads', onRefresh, onOpenConversation } = $props<{
    leads: Lead[];
    entityType?: 'leads' | 'contacts';
    onRefresh?: () => void;
    onOpenConversation?: (lead: { id: number; phone?: string; name?: string; zalo_uid?: string }) => void;
  }>();
  let selectedLeadId = $state<number | null>(null);

  // Multiselect state
  let selectedRows = $state<Set<number>>(new Set());

  // Entity label derivations
  let entityLabel = $derived(entityType === 'contacts' ? m.leads_entity_contacts() : m.leads_entity_leads());
  let entityLabelPlural = $derived(entityType === 'contacts' ? m.leads_entity_contacts() : m.leads_entity_leads());
  let entityLabelCap = $derived(entityType === 'contacts' ? 'Contact' : 'Lead');
  let entityLabelPluralCap = $derived(entityType === 'contacts' ? 'Contacts' : 'Leads');
  
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
  let filterStatus = $state('all');
  let filterOwner = $state('all');
  let filterValueRange = $state('all');
  let filterZaloStatus = $state('all');
  let filterOutreachStatus = $state('all');
  let currentPage = $state(1);
  let pageSize = $state(10);

  // Derived variables
  let uniqueStatuses = $derived(Array.from(new Set(leads.map((l: Lead) => l.status))) as string[]);
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
      
      if (filterStatus && filterStatus !== 'all' && lead.status !== filterStatus) return false;
      if (filterOwner && filterOwner !== 'all' && lead.owner !== filterOwner) return false;
      
      if (filterValueRange && filterValueRange !== 'all') {
        const val = lead.value || 0;
        if (filterValueRange === 'under_10m' && val >= 10000000) return false;
        if (filterValueRange === '10m_to_50m' && (val < 10000000 || val > 50000000)) return false;
        if (filterValueRange === 'above_50m' && val <= 50000000) return false;
      }

      if (filterZaloStatus && filterZaloStatus !== 'all') {
        if (filterZaloStatus === 'verified' && lead.has_zalo !== 1) return false;
        if (filterZaloStatus === 'no_zalo' && lead.has_zalo !== 0) return false;
        if (filterZaloStatus === 'unchecked' && lead.has_zalo !== null && lead.has_zalo !== undefined) return false;
      }

      if (filterOutreachStatus && filterOutreachStatus !== 'all') {
        if (filterOutreachStatus === 'reached' && lead.zalo_reached !== 1) return false;
        if (filterOutreachStatus === 'not_reached' && lead.zalo_reached === 1) return false;
      }
      
      return true;
    })
  );

  let totalPages = $derived(Math.max(1, Math.ceil(filteredLeads.length / pageSize)));

  let paginatedLeads = $derived(
    filteredLeads.slice((currentPage - 1) * pageSize, currentPage * pageSize)
  );

  // Pagination bounds info
  let totalEntries = $derived(filteredLeads.length);
  let showingStart = $derived(totalEntries === 0 ? 0 : (currentPage - 1) * pageSize + 1);
  let showingEnd = $derived(Math.min(currentPage * pageSize, totalEntries));

  // Selection computed helpers
  let isCurrentPageAllSelected = $derived(
    paginatedLeads.length > 0 && paginatedLeads.every((l: Lead) => l.id !== undefined && selectedRows.has(l.id))
  );

  let isCurrentPagePartiallySelected = $derived(
    paginatedLeads.some((l: Lead) => l.id !== undefined && selectedRows.has(l.id)) && !isCurrentPageAllSelected
  );

  let isAllFilteredSelected = $derived(
    filteredLeads.length > 0 && filteredLeads.every((l: Lead) => l.id !== undefined && selectedRows.has(l.id))
  );

  function selectAllFiltered() {
    filteredLeads.forEach((l: Lead) => {
      if (l.id !== undefined) selectedRows.add(l.id);
    });
    selectedRows = new Set(selectedRows);
  }

  function clearSelection() {
    selectedRows.clear();
    selectedRows = new Set(selectedRows);
  }

  // Reset pagination to page 1 and clear selected rows on search / filter / pageSize updates
  let prevFilterKey = $state('');
  $effect(() => {
    // Read reactive filter values and page size
    const currentKey = `${searchQuery}|${filterStatus}|${filterOwner}|${filterValueRange}|${filterZaloStatus}|${filterOutreachStatus}|${pageSize}`;
    untrack(() => {
      if (prevFilterKey !== currentKey) {
        if (prevFilterKey !== '') {
          currentPage = 1;
          selectedRows = new Set();
        }
        prevFilterKey = currentKey;
      }
    });
  });

  // Clamp currentPage when totalPages changes
  $effect(() => {
    if (currentPage > totalPages) {
      currentPage = Math.max(1, totalPages);
    }
  });

  async function handleAddLead(e: Event) {
    e.preventDefault();
    const trimmedName = name.trim();
    const trimmedCompany = company.trim();
    const trimmedEmail = email.trim();
    const trimmedPhone = phone.trim();
    const trimmedAbout = about.trim();

    if (!trimmedName || !trimmedCompany || !trimmedEmail) {
      modalErrorMsg = m.errors_required_field();
      return;
    }

    saving = true;
    modalErrorMsg = null;
    try {
      await crmService.addLead(
        trimmedName,
        trimmedCompany,
        entityType === 'contacts' ? 'Converted' : 'New',
        Number(value) || 0,
        owner.trim() || 'Self',
        trimmedEmail,
        trimmedPhone,
        trimmedAbout || null
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
      assignSuccessMsg = m.common_success();
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

  async function handleExportSelected() {
    if (selectedRows.size === 0) return;
    const leadsToExport = leads.filter((l: Lead) => l.id !== undefined && selectedRows.has(l.id));
    if (leadsToExport.length === 0) return;

    try {
      const savedPath = await exportLeadsToCsv(leadsToExport, entityType);
      if (!savedPath) {
        return;
      }
      const displayName = savedPath.split(/[\/\\]/).pop() || savedPath;
      toast.success(m.leads_export_success({ count: leadsToExport.length, entity: entityLabelPlural, destination: displayName }));
    } catch (err) {
      console.error('Failed to export leads', err);
      toast.error(m.leads_export_failed());
    }
  }
</script>

<div class="flex flex-col gap-4">
  <!-- Top Action Header -->
  <div class="flex items-center justify-between">
    <div>
      <h2 class="text-xl font-bold text-foreground tracking-tight">
        {entityType === 'contacts' ? m.tab_contacts_title() : m.leads_title()}
      </h2>
      <p class="text-xs text-muted-foreground mt-0.5">
        {entityType === 'contacts' ? m.tab_contacts_subtitle() : m.tab_leads_subtitle()}
      </p>
    </div>
    <div class="flex items-center gap-2">
      {#if selectedRows.size > 0}
        <Button variant="outline" size="sm" onclick={handleExportSelected} class="gap-2 border-emerald-500/40 text-emerald-600 hover:bg-emerald-500/10 dark:border-emerald-500/30 dark:text-emerald-400">
          <Download class="size-4" />
          {m.leads_export_csv({ count: selectedRows.size })}
        </Button>
        <Button variant="outline" size="sm" onclick={openCategoryModal} class="gap-2 border-emerald-500/40 text-emerald-600 hover:bg-emerald-500/10 dark:border-emerald-500/30 dark:text-emerald-400">
          <FolderPlus class="size-4" />
          {m.leads_assign_category({ count: selectedRows.size })}
        </Button>
      {/if}
      <Button size="sm" onclick={() => { showAddModal = true; modalErrorMsg = null; }} class="gap-2 bg-primary text-primary-foreground hover:bg-primary/90">
        <Plus class="size-4" />
        {entityType === 'contacts' ? m.leads_add_contact() : m.leads_new_lead()}
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
        placeholder={m.leads_search_placeholder()}
        bind:value={searchQuery}
        class="pl-9 text-xs h-9"
      />
    </div>

    <!-- Filter Dropdowns -->
    <div class="flex flex-wrap items-center gap-2">
      {#if uniqueStatuses.length > 1}
        <Select.Root type="single" bind:value={filterStatus}>
          <Select.Trigger class="h-9 text-xs min-w-[120px]">
            {filterStatus === 'all' || !filterStatus ? m.leads_filter_status() : filterStatus}
          </Select.Trigger>
          <Select.Content>
            <Select.Item value="all">{m.leads_filter_status()}</Select.Item>
            {#each uniqueStatuses as status (status)}
              <Select.Item value={status}>{status}</Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
      {/if}

      <Select.Root type="single" bind:value={filterOwner}>
        <Select.Trigger class="h-9 text-xs min-w-[120px]">
          {filterOwner === 'all' || !filterOwner ? m.leads_filter_all_owners() : filterOwner}
        </Select.Trigger>
        <Select.Content>
          <Select.Item value="all">{m.leads_filter_all_owners()}</Select.Item>
          {#each uniqueOwners as owner (owner)}
            <Select.Item value={owner}>{owner}</Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>

      <Select.Root type="single" bind:value={filterValueRange}>
        <Select.Trigger class="h-9 text-xs min-w-[130px]">
          {filterValueRange === 'under_10m' ? m.leads_filter_under_10m() : filterValueRange === '10m_to_50m' ? m.leads_filter_10m_50m() : filterValueRange === 'above_50m' ? m.leads_filter_above_50m() : m.leads_filter_all_values()}
        </Select.Trigger>
        <Select.Content>
          <Select.Item value="all">{m.leads_filter_all_values()}</Select.Item>
          <Select.Item value="under_10m">{m.leads_filter_under_10m()}</Select.Item>
          <Select.Item value="10m_to_50m">{m.leads_filter_10m_50m()}</Select.Item>
          <Select.Item value="above_50m">{m.leads_filter_above_50m()}</Select.Item>
        </Select.Content>
      </Select.Root>

      <Select.Root type="single" bind:value={filterZaloStatus}>
        <Select.Trigger class="h-9 text-xs min-w-[130px]">
          {filterZaloStatus === 'verified' ? m.leads_filter_zalo_verified() : filterZaloStatus === 'no_zalo' ? m.leads_filter_zalo_none() : filterZaloStatus === 'unchecked' ? m.leads_filter_zalo_unchecked() : m.leads_filter_zalo_all()}
        </Select.Trigger>
        <Select.Content>
          <Select.Item value="all">{m.leads_filter_zalo_all()}</Select.Item>
          <Select.Item value="verified">{m.leads_filter_zalo_verified()}</Select.Item>
          <Select.Item value="no_zalo">{m.leads_filter_zalo_none()}</Select.Item>
          <Select.Item value="unchecked">{m.leads_filter_zalo_unchecked()}</Select.Item>
        </Select.Content>
      </Select.Root>

      {#if searchQuery || (filterStatus && filterStatus !== 'all') || (filterOwner && filterOwner !== 'all') || (filterValueRange && filterValueRange !== 'all') || (filterZaloStatus && filterZaloStatus !== 'all') || (filterOutreachStatus && filterOutreachStatus !== 'all')}
        <Button
          variant="ghost"
          size="sm"
          onclick={() => { searchQuery = ''; filterStatus = 'all'; filterOwner = 'all'; filterValueRange = 'all'; filterZaloStatus = 'all'; filterOutreachStatus = 'all'; }}
          class="h-9 px-2 text-xs gap-1 text-muted-foreground hover:text-foreground"
        >
          <X class="size-3.5" /> {m.common_clear_filters()}
        </Button>
      {/if}
    </div>
  </div>

  <!-- Selection Banner -->
  {#if selectedRows.size > 0}
    <div class="px-4 py-2 bg-emerald-500/10 border border-emerald-500/20 rounded-lg flex items-center justify-between text-xs text-emerald-700 dark:text-emerald-300 font-medium">
      <div class="flex items-center gap-2">
        <span>
          {#if isAllFilteredSelected}
            {m.leads_all_selected_matching({ count: filteredLeads.length, entity: entityLabelPlural })}
          {:else}
            {m.leads_selected_count({ count: selectedRows.size, entity: entityLabel })}
          {/if}
        </span>
        {#if !isAllFilteredSelected}
          <button
            type="button"
            onclick={selectAllFiltered}
            class="underline font-bold hover:text-emerald-800 dark:hover:text-emerald-100 transition-colors ml-1 cursor-pointer"
          >
            {m.leads_select_all_matching({ count: filteredLeads.length, entity: entityLabelPlural })}
          </button>
        {/if}
      </div>
      <button
        type="button"
        onclick={clearSelection}
        class="text-xs text-emerald-600 dark:text-emerald-400 hover:underline cursor-pointer"
      >
        {m.leads_clear_selection()}
      </button>
    </div>
  {/if}

  <!-- Data Table -->
  <div class="rounded-md border border-border bg-card overflow-hidden">
    <Table.Root>
      <Table.Header class="bg-muted/40">
        <Table.Row>
          <Table.Head class="w-10 text-center">
            <div class="flex items-center justify-center">
              <Checkbox 
                checked={isCurrentPageAllSelected}
                indeterminate={isCurrentPagePartiallySelected}
                onCheckedChange={(checked) => toggleAllRows(Boolean(checked))}
                aria-label={`Select all ${entityLabelPlural} on current page`}
              />
            </div>
          </Table.Head>
          <Table.Head>{m.leads_table_name()}</Table.Head>
          <Table.Head>{m.leads_table_company()}</Table.Head>
          <Table.Head>{m.leads_table_phone()}</Table.Head>
          <Table.Head>{m.leads_table_email()}</Table.Head>
          <Table.Head>{m.leads_table_zalo()}</Table.Head>
          <Table.Head>{m.common_status()}</Table.Head>
          <Table.Head class="text-right">{m.leads_table_value()}</Table.Head>
          <Table.Head>{m.leads_table_owner()}</Table.Head>
        </Table.Row>
      </Table.Header>
      <Table.Body>
        {#if paginatedLeads.length === 0}
          <Table.Row>
            <Table.Cell colspan={9} class="text-center py-8 text-xs text-muted-foreground">
              {m.common_no_data()}
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
                    <CheckCircle2 class="size-3 text-sky-500" /> {m.leads_filter_zalo_verified()}
                  </Badge>
                {:else if lead.has_zalo === 0}
                  <Badge variant="outline" class="gap-1 border-slate-300 text-slate-500 bg-slate-100 text-[10px] py-0">
                    <XCircle class="size-3 text-slate-400" /> {m.leads_filter_zalo_none()}
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
                {formatCurrency(lead.value, 'VND')}
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
      <span>{m.common_showing({ start: showingStart, end: showingEnd, total: totalEntries })}</span>
      
      <div class="flex items-center gap-2">
        <Select.Root
          type="single"
          value={String(pageSize)}
          onValueChange={(val) => val && (pageSize = Number(val))}
        >
          <Select.Trigger class="h-8 text-xs min-w-[90px]">
            {pageSize} / page
          </Select.Trigger>
          <Select.Content>
            <Select.Item value="5">5 / page</Select.Item>
            <Select.Item value="10">10 / page</Select.Item>
            <Select.Item value="25">25 / page</Select.Item>
          </Select.Content>
        </Select.Root>

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
    {@const selectedLead = leads.find((l: Lead) => l.id === selectedLeadId)}
    <LeadDetails
      leadId={selectedLeadId}
      initialLead={selectedLead}
      onClose={() => selectedLeadId = null}
      onRefresh={onRefresh}
      onOpenConversation={(leadPayload) => {
        if (onOpenConversation) onOpenConversation(leadPayload);
        selectedLeadId = null;
      }}
    />
  {/if}
</div>

<!-- Add Lead / Contact Modal -->
{#if showAddModal}
  <div class="fixed inset-0 z-50 bg-background/80 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="w-full max-w-lg rounded-xl border border-border bg-card p-6 shadow-lg flex flex-col gap-4">
      <div class="flex items-center justify-between border-b border-border pb-3">
        <h3 class="text-base font-bold text-foreground">{m.leads_modal_add_title({ entity: entityLabelCap })}</h3>
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
            <label for="lead-name" class="font-medium text-foreground">
              {m.leads_modal_name_label()} <span class="text-destructive">*</span>
            </label>
            <Input id="lead-name" bind:value={name} required placeholder="Full Name" />
          </div>
          <div class="flex flex-col gap-1">
            <label for="lead-company" class="font-medium text-foreground">
              {m.leads_modal_company_label()} <span class="text-destructive">*</span>
            </label>
            <Input id="lead-company" bind:value={company} required placeholder="Company Name" />
          </div>
          <div class="flex flex-col gap-1">
            <label for="lead-email" class="font-medium text-foreground">
              {m.leads_modal_email_label()} <span class="text-destructive">*</span>
            </label>
            <Input id="lead-email" type="email" bind:value={email} required placeholder="email@example.com" />
          </div>
          <div class="flex flex-col gap-1">
            <label for="lead-phone" class="font-medium text-foreground">{m.leads_modal_phone_label()}</label>
            <Input id="lead-phone" bind:value={phone} placeholder="Phone number" />
          </div>
          <div class="flex flex-col gap-1">
            <label for="lead-value" class="font-medium text-foreground">{m.leads_modal_value_label()}</label>
            <Input id="lead-value" type="number" bind:value={value} min="0" />
          </div>
          <div class="flex flex-col gap-1">
            <label for="lead-owner" class="font-medium text-foreground">{m.leads_modal_owner_label()}</label>
            <Input id="lead-owner" bind:value={owner} placeholder="Owner Name" />
          </div>
        </div>

        <div class="flex flex-col gap-1">
          <label for="lead-about" class="font-medium text-foreground">{m.leads_modal_about_label()}</label>
          <textarea
            id="lead-about"
            bind:value={about}
            rows="3"
            placeholder={`${entityLabelCap} background & details...`}
            class="w-full p-2.5 rounded-md border border-input bg-background text-xs text-foreground focus:outline-hidden"
          ></textarea>
        </div>

        <div class="flex items-center justify-end gap-2 pt-2 border-t border-border">
          <Button type="button" variant="outline" size="sm" onclick={() => showAddModal = false}>{m.common_cancel()}</Button>
          <Button type="submit" size="sm" disabled={saving}>
            {saving ? m.common_loading() : m.leads_modal_save_btn({ entity: entityLabelCap })}
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
        <h3 class="text-sm font-bold text-foreground">{m.leads_modal_assign_title({ count: selectedRows.size, entity: entityLabelCap })}</h3>
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
        <p class="text-muted-foreground">{m.leads_modal_assign_title({ count: selectedRows.size, entity: entityLabel })}</p>
        <Select.Root
          type="single"
          value={selectedCategoryId != null ? String(selectedCategoryId) : ""}
          onValueChange={(val) => selectedCategoryId = val ? Number(val) : null}
        >
          <Select.Trigger class="w-full h-9 text-xs">
            {categories.find(c => String(c.id) === String(selectedCategoryId))?.name || m.leads_modal_select_category()}
          </Select.Trigger>
          <Select.Content>
            {#each categories as cat (cat.id)}
              <Select.Item value={String(cat.id)}>{cat.name}</Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
      </div>

      <div class="flex items-center justify-end gap-2 pt-2">
        <Button variant="outline" size="sm" onclick={() => showCategoryModal = false} disabled={assigningCategory}>{m.common_cancel()}</Button>
        <Button size="sm" onclick={handleAssignCategory} disabled={assigningCategory || categories.length === 0}>
          {assigningCategory ? m.common_loading() : m.leads_modal_assign_btn({ entity: entityLabel })}
        </Button>
      </div>
    </div>
  </div>
{/if}
