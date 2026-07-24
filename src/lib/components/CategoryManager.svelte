<script lang="ts">
  import { onMount } from 'svelte';
  import * as crmService from '$lib/services/crmService';
  import type { Category, Lead } from '$lib/services/crmService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import * as Card from '$lib/components/ui/card';
  import * as Table from '$lib/components/ui/table';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Select from '$lib/components/ui/select';
  import { Folder, Plus, Trash2, Search, FolderMinus, AlertCircle, CheckCircle2 } from 'lucide-svelte';

  let categories = $state<Category[]>([]);
  let activeCategoryId = $state<number | null>(null);
  let newCategoryName = $state('');
  
  let categoryLeads = $state<Lead[]>([]);
  let allLeads = $state<Lead[]>([]);
  let addLeadsSearch = $state('');

  let categoryLeadsSearch = $state('');
  let filterCategoryStatus = $state('');
  let filterCategoryZalo = $state('');

  let selectedToRemove = $state<Set<number>>(new Set());
  let selectedToAdd = $state<Set<number>>(new Set());

  let loading = $state(false);
  let errorMsg = $state<string | null>(null);
  let successMsg = $state<string | null>(null);

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
      categoryLeadsSearch = '';
      filterCategoryStatus = '';
      filterCategoryZalo = '';
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

  let uniqueCategoryStatuses = $derived(
    Array.from(new Set(categoryLeads.map(l => l.status).filter(Boolean))) as string[]
  );

  let filteredCategoryLeads = $derived(
    categoryLeads.filter(lead => {
      if (categoryLeadsSearch.trim()) {
        const query = categoryLeadsSearch.toLowerCase().trim();
        const matchesName = (lead.name || '').toLowerCase().includes(query);
        const matchesCompany = (lead.company || '').toLowerCase().includes(query);
        const matchesPhone = (lead.phone || '').includes(query);
        const matchesEmail = (lead.email || '').toLowerCase().includes(query);
        if (!matchesName && !matchesCompany && !matchesPhone && !matchesEmail) {
          return false;
        }
      }

      if (filterCategoryStatus && lead.status !== filterCategoryStatus) return false;

      if (filterCategoryZalo) {
        if (filterCategoryZalo === 'verified' && lead.has_zalo !== 1) return false;
        if (filterCategoryZalo === 'no_zalo' && lead.has_zalo !== 0) return false;
      }

      return true;
    })
  );

  let availableLeads = $derived(
    allLeads.filter(lead => 
      !categoryLeads.some(cl => cl.id === lead.id) &&
      (addLeadsSearch ? (lead.name || '').toLowerCase().includes(addLeadsSearch.toLowerCase()) || 
       (lead.company || '').toLowerCase().includes(addLeadsSearch.toLowerCase()) ||
       (lead.phone || '').includes(addLeadsSearch) : true)
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

  function toggleRemoveAll(checked: boolean) {
    if (checked) {
      filteredCategoryLeads.forEach(l => { if (l.id) selectedToRemove.add(l.id); });
    } else {
      filteredCategoryLeads.forEach(l => { if (l.id) selectedToRemove.delete(l.id); });
    }
    selectedToRemove = new Set(selectedToRemove);
  }

  function toggleAddSelection(id: number) {
    if (selectedToAdd.has(id)) selectedToAdd.delete(id);
    else selectedToAdd.add(id);
    selectedToAdd = new Set(selectedToAdd);
  }

  function toggleAddAll(checked: boolean) {
    if (checked) {
      availableLeads.forEach(l => { if (l.id) selectedToAdd.add(l.id); });
    } else {
      selectedToAdd.clear();
    }
    selectedToAdd = new Set(selectedToAdd);
  }
</script>

<div class="flex flex-col gap-4">
  {#if errorMsg}
    <div class="p-3 rounded-lg border border-destructive/30 bg-destructive/10 text-destructive text-xs flex items-center gap-2">
      <AlertCircle class="size-4 shrink-0" />
      <span>{errorMsg}</span>
    </div>
  {/if}
  {#if successMsg}
    <div class="p-3 rounded-lg border border-emerald-500/30 bg-emerald-500/10 text-emerald-600 text-xs flex items-center gap-2">
      <CheckCircle2 class="size-4 shrink-0" />
      <span>{successMsg}</span>
    </div>
  {/if}

  <div class="flex gap-4 h-[calc(100vh-140px)] overflow-hidden">
    <!-- LEFT PANEL: Category Sidebar -->
    <aside class="w-72 rounded-xl border border-border bg-card flex flex-col shrink-0 overflow-hidden shadow-2xs">
      <div class="h-12 px-4 border-b border-border flex items-center justify-between font-bold text-xs uppercase tracking-wider text-foreground">
        <span>Categories</span>
        <Badge variant="secondary" class="text-[10px]">{categories.length}</Badge>
      </div>

      <div class="p-3 border-b border-border flex gap-2">
        <Input 
          type="text" 
          bind:value={newCategoryName} 
          placeholder="New Category..." 
          class="h-8 text-xs flex-1"
          onkeydown={(e) => { if (e.key === 'Enter') handleCreateCategory(); }}
        />
        <Button size="sm" class="size-8 p-0 shrink-0" onclick={handleCreateCategory}>
          <Plus class="size-4" />
        </Button>
      </div>

      <div class="flex-1 overflow-y-auto p-2 flex flex-col gap-1">
        {#each categories as category (category.id)}
          {@const isActive = activeCategoryId === category.id}
          <div class="group flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-all {isActive ? 'bg-primary/10 text-primary font-semibold' : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground'}">
            <button type="button" class="flex-1 flex items-center gap-2.5 text-left truncate cursor-pointer" onclick={() => activeCategoryId = category.id!}>
              <Folder class="size-4 shrink-0 {isActive ? 'text-primary' : 'text-muted-foreground'}" />
              <span class="truncate">{category.name}</span>
            </button>
            <Button variant="ghost" size="sm" onclick={() => handleDeleteCategory(category.id!)} class="size-6 p-0 opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-destructive transition-opacity">
              <Trash2 class="size-3.5" />
            </Button>
          </div>
        {/each}
        {#if categories.length === 0 && !loading}
          <div class="p-4 text-center text-xs text-muted-foreground">No categories yet.</div>
        {/if}
      </div>
    </aside>

    <!-- RIGHT PANEL: Category Details -->
    <main class="flex-1 flex flex-col gap-4 overflow-hidden min-w-0">
      {#if activeCategoryId && activeCategory}
        <div class="p-4 rounded-xl border border-border bg-card flex items-center justify-between shrink-0 shadow-2xs">
          <div>
            <h2 class="text-base font-bold text-foreground">{activeCategory.name}</h2>
            <p class="text-xs text-muted-foreground">{categoryLeads.length} lead(s) categorized</p>
          </div>
        </div>

        <div class="flex-1 overflow-y-auto flex flex-col gap-4 pr-1">
          <!-- Leads in Category Table -->
          <div class="rounded-xl border border-border bg-card overflow-hidden shadow-2xs flex flex-col gap-3">
            <div class="h-11 px-4 border-b border-border bg-muted/30 flex items-center justify-between">
              <div class="flex items-center gap-2">
                <h3 class="text-xs font-bold text-foreground uppercase tracking-wider">Leads in Category</h3>
                <Badge variant="secondary" class="text-[10px]">{filteredCategoryLeads.length} / {categoryLeads.length}</Badge>
              </div>
              {#if selectedToRemove.size > 0}
                <Button variant="destructive" size="sm" class="h-7 text-xs gap-1.5" onclick={handleRemoveLeads}>
                  <FolderMinus class="size-3.5" />
                  Remove Selected ({selectedToRemove.size})
                </Button>
              {/if}
            </div>

            <!-- Toolbar -->
            <div class="px-4 flex items-center gap-3">
              <div class="relative flex-1">
                <Search class="size-3.5 absolute left-2.5 top-2.5 text-muted-foreground" />
                <Input type="text" bind:value={categoryLeadsSearch} placeholder="Search lead name, company, phone..." class="h-8 pl-8 text-xs" />
              </div>
              <Select.Root type="single" bind:value={filterCategoryStatus}>
                <Select.Trigger class="h-8 text-xs min-w-[130px]">
                  {filterCategoryStatus || 'All Statuses'}
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="">All Statuses</Select.Item>
                  {#each uniqueCategoryStatuses as st (st)}
                    <Select.Item value={st}>{st}</Select.Item>
                  {/each}
                </Select.Content>
              </Select.Root>
            </div>

            <Table.Root>
              <Table.Header class="bg-muted/20">
                <Table.Row>
                  <Table.Head class="w-10 text-center">
                    <div class="flex items-center justify-center">
                      <Checkbox 
                        checked={filteredCategoryLeads.length > 0 && filteredCategoryLeads.every(l => l.id && selectedToRemove.has(l.id))}
                        onCheckedChange={(checked) => toggleRemoveAll(Boolean(checked))}
                      />
                    </div>
                  </Table.Head>
                  <Table.Head>Name</Table.Head>
                  <Table.Head>Company</Table.Head>
                  <Table.Head>Phone</Table.Head>
                  <Table.Head>Status</Table.Head>
                </Table.Row>
              </Table.Header>
              <Table.Body>
                {#each filteredCategoryLeads as lead (lead.id)}
                  {@const isSelected = lead.id ? selectedToRemove.has(lead.id) : false}
                  <Table.Row 
                    class="hover:bg-muted/40 cursor-pointer transition-colors {isSelected ? 'bg-primary/5' : ''}"
                    onclick={() => lead.id && toggleRemoveSelection(lead.id)}
                  >
                    <Table.Cell class="text-center">
                      <div class="flex items-center justify-center" onclick={(e) => e.stopPropagation()} role="none">
                        <Checkbox 
                          checked={isSelected}
                          onCheckedChange={() => lead.id && toggleRemoveSelection(lead.id)}
                        />
                      </div>
                    </Table.Cell>
                    <Table.Cell class="font-semibold text-xs text-foreground">{lead.name}</Table.Cell>
                    <Table.Cell class="text-xs text-muted-foreground">{lead.company}</Table.Cell>
                    <Table.Cell class="text-xs text-muted-foreground">{lead.phone || '—'}</Table.Cell>
                    <Table.Cell>
                      <Badge variant="secondary" class="text-[10px] py-0">{lead.status}</Badge>
                    </Table.Cell>
                  </Table.Row>
                {/each}
                {#if filteredCategoryLeads.length === 0}
                  <Table.Row>
                    <Table.Cell colspan={5} class="text-center py-6 text-xs text-muted-foreground">
                      No leads in this category.
                    </Table.Cell>
                  </Table.Row>
                {/if}
              </Table.Body>
            </Table.Root>
          </div>

          <!-- Available Leads to Add Table -->
          <div class="rounded-xl border border-border bg-card overflow-hidden shadow-2xs flex flex-col gap-3">
            <div class="h-11 px-4 border-b border-border bg-muted/30 flex items-center justify-between">
              <h3 class="text-xs font-bold text-foreground uppercase tracking-wider">Available Leads to Add</h3>
              {#if selectedToAdd.size > 0}
                <Button size="sm" class="h-7 text-xs gap-1.5 bg-emerald-600 hover:bg-emerald-700 text-white" onclick={handleAddLeads}>
                  <Plus class="size-3.5" />
                  Add Selected ({selectedToAdd.size})
                </Button>
              {/if}
            </div>

            <div class="px-4">
              <div class="relative">
                <Search class="size-3.5 absolute left-2.5 top-2.5 text-muted-foreground" />
                <Input type="text" bind:value={addLeadsSearch} placeholder="Search leads to add..." class="h-8 pl-8 text-xs" />
              </div>
            </div>

            <Table.Root>
              <Table.Header class="bg-muted/20">
                <Table.Row>
                  <Table.Head class="w-10 text-center">
                    <div class="flex items-center justify-center">
                      <Checkbox 
                        checked={availableLeads.length > 0 && selectedToAdd.size === availableLeads.length}
                        onCheckedChange={(checked) => toggleAddAll(Boolean(checked))}
                      />
                    </div>
                  </Table.Head>
                  <Table.Head>Name</Table.Head>
                  <Table.Head>Company</Table.Head>
                  <Table.Head>Status</Table.Head>
                </Table.Row>
              </Table.Header>
              <Table.Body>
                {#each availableLeads as lead (lead.id)}
                  {@const isSelected = lead.id ? selectedToAdd.has(lead.id) : false}
                  <Table.Row 
                    class="hover:bg-muted/40 cursor-pointer transition-colors {isSelected ? 'bg-primary/5' : ''}"
                    onclick={() => lead.id && toggleAddSelection(lead.id)}
                  >
                    <Table.Cell class="text-center">
                      <div class="flex items-center justify-center" onclick={(e) => e.stopPropagation()} role="none">
                        <Checkbox 
                          checked={isSelected}
                          onCheckedChange={() => lead.id && toggleAddSelection(lead.id)}
                        />
                      </div>
                    </Table.Cell>
                    <Table.Cell class="font-semibold text-xs text-foreground">{lead.name}</Table.Cell>
                    <Table.Cell class="text-xs text-muted-foreground">{lead.company}</Table.Cell>
                    <Table.Cell>
                      <Badge variant="outline" class="text-[10px] py-0">{lead.status}</Badge>
                    </Table.Cell>
                  </Table.Row>
                {/each}
                {#if availableLeads.length === 0}
                  <Table.Row>
                    <Table.Cell colspan={4} class="text-center py-6 text-xs text-muted-foreground">
                      No available leads to add.
                    </Table.Cell>
                  </Table.Row>
                {/if}
              </Table.Body>
            </Table.Root>
          </div>
        </div>
      {:else}
        <div class="flex-1 rounded-xl border border-border bg-card flex flex-col items-center justify-center p-8 text-center text-muted-foreground">
          <Folder class="size-12 stroke-1 text-muted-foreground/50 mb-3" />
          <h3 class="text-sm font-bold text-foreground">Select a Category</h3>
          <p class="text-xs mt-1">Choose a category from the left sidebar to manage its leads</p>
        </div>
      {/if}
    </main>
  </div>
</div>
