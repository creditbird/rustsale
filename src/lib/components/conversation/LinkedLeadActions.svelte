<script lang="ts">
  import { onMount } from 'svelte';
  import {
    linkConversationIdentity,
    relinkConversationLead,
    unlinkConversationIdentity,
    unlinkConversationLead,
    type InboxItem
  } from '$lib/services/conversationService';
  import * as crmService from '$lib/services/crmService';
  import type { Lead, Category } from '$lib/services/crmService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import * as Dialog from '$lib/components/ui/dialog';
  import { RefreshCw, Unlink, Search, AlertCircle, Tag, Plus, Check } from 'lucide-svelte';

  let {
    item,
    onRelinked,
    onUnlinked
  }: {
    item: InboxItem;
    onRelinked?: (leadId: number) => void;
    onUnlinked?: () => void;
  } = $props();

  let isRelinkModalOpen = $state(false);
  let isUnlinkModalOpen = $state(false);
  let isCategoryModalOpen = $state(false);

  // Relink state
  let leads = $state<Lead[]>([]);
  let isLoadingLeads = $state(false);
  let leadSearchText = $state('');
  let selectedLeadId = $state<number | null>(null);
  let isRelinking = $state(false);
  let relinkError = $state<string | null>(null);

  // Unlink state
  let isUnlinking = $state(false);
  let unlinkError = $state<string | null>(null);

  // Category state
  let assignedCategories = $state<Category[]>([]);
  let allCategories = $state<Category[]>([]);
  let isLoadingCategories = $state(false);
  let categorySearchText = $state('');
  let newCategoryName = $state('');
  let isCreatingCategory = $state(false);
  let categoryError = $state<string | null>(null);

  const leadCategoriesCache = new Map<number, Category[]>();
  const leadCategoriesInFlight = new Map<number, Promise<Category[]>>();
  const categoryFetchGeneration = new Map<number, number>();
  const categoryPendingForced = new Map<number, boolean>();
  let visibleLeadRequestEpoch = 0;

  $effect(() => {
    const leadId = item.leadId;
    isCategoryModalOpen = false;
    categoryError = null;
    if (leadId) {
      void loadLeadCategories(leadId);
    } else {
      assignedCategories = [];
    }
  });

  onMount(() => {
    return crmService.onLeadsChanged(() => {
      leadCategoriesCache.clear();
      if (item.leadId) {
        void loadLeadCategories(item.leadId, true);
      }
    });
  });

  async function loadLeadCategories(leadId: number, forceRefresh = false): Promise<Category[]> {
    const epoch = ++visibleLeadRequestEpoch;
    if (leadCategoriesCache.has(leadId)) {
      if (item.leadId === leadId) {
        assignedCategories = leadCategoriesCache.get(leadId)!;
      }
      if (!forceRefresh) return leadCategoriesCache.get(leadId)!;
    }

    if (leadCategoriesInFlight.has(leadId)) {
      if (forceRefresh) {
        categoryPendingForced.set(leadId, true);
      }
      try {
        const res = await leadCategoriesInFlight.get(leadId)!;
        if (item.leadId === leadId && epoch === visibleLeadRequestEpoch) {
          assignedCategories = res;
        }
      } catch {}
      if (categoryPendingForced.get(leadId)) {
        categoryPendingForced.delete(leadId);
        return loadLeadCategories(leadId, true);
      }
      return leadCategoriesCache.get(leadId) ?? [];
    }

    const generation = (categoryFetchGeneration.get(leadId) ?? 0) + 1;
    categoryFetchGeneration.set(leadId, generation);

    const promise = (async () => {
      try {
        const res = await crmService.getCategoriesForLead(leadId);
        if (categoryFetchGeneration.get(leadId) === generation) {
          leadCategoriesCache.set(leadId, res);
        }
        if (item.leadId === leadId && epoch === visibleLeadRequestEpoch) {
          assignedCategories = res;
        }
        return res;
      } catch (err) {
        console.error('Failed to load lead categories', err);
        if (!leadCategoriesCache.has(leadId) && item.leadId === leadId && epoch === visibleLeadRequestEpoch) {
          assignedCategories = [];
        }
        return [];
      }
    })();

    leadCategoriesInFlight.set(leadId, promise);
    let result: Category[] = [];
    try {
      result = await promise;
    } finally {
      if (leadCategoriesInFlight.get(leadId) === promise) {
        leadCategoriesInFlight.delete(leadId);
      }
    }

    if (categoryPendingForced.get(leadId)) {
      categoryPendingForced.delete(leadId);
      return loadLeadCategories(leadId, true);
    }

    return result;
  }

  let filteredLeads = $derived.by(() => {
    const q = leadSearchText.trim().toLowerCase();
    if (!q) return leads;
    return leads.filter(
      (l) =>
        l.name.toLowerCase().includes(q) ||
        l.company.toLowerCase().includes(q) ||
        (l.email && l.email.toLowerCase().includes(q)) ||
        (l.phone && l.phone.includes(q))
    );
  });

  let filteredCategories = $derived.by(() => {
    const q = categorySearchText.trim().toLowerCase();
    if (!q) return allCategories;
    return allCategories.filter((c) => c.name.toLowerCase().includes(q));
  });

  let assignedCategoryIds = $derived(
    new Set(assignedCategories.map((c) => c.id).filter((id): id is number => id !== undefined))
  );

  async function openCategoryModal() {
    categoryError = null;
    categorySearchText = '';
    newCategoryName = '';
    isCategoryModalOpen = true;
    isLoadingCategories = true;
    const targetLeadId = item.leadId;
    try {
      const [all, assigned] = await Promise.all([
        crmService.getCategories(),
        targetLeadId ? loadLeadCategories(targetLeadId, true) : Promise.resolve([])
      ]);
      if (item.leadId === targetLeadId) {
        allCategories = all;
        assignedCategories = assigned;
      }
    } catch (err) {
      if (item.leadId === targetLeadId) {
        categoryError = err instanceof Error ? err.message : String(err);
      }
    } finally {
      if (item.leadId === targetLeadId) {
        isLoadingCategories = false;
      }
    }
  }

  async function toggleCategory(category: Category) {
    if (!item.leadId || category.id === undefined) return;
    categoryError = null;
    const catId = category.id;
    const isAssigned = assignedCategoryIds.has(catId);
    try {
      if (isAssigned) {
        await crmService.removeLeadsFromCategory([item.leadId], catId);
        assignedCategories = assignedCategories.filter((c) => c.id !== catId);
      } else {
        await crmService.addLeadsToCategory([item.leadId], catId);
        assignedCategories = [...assignedCategories, category];
      }
      leadCategoriesCache.set(item.leadId, assignedCategories);
    } catch (err) {
      categoryError = err instanceof Error ? err.message : String(err);
    }
  }

  async function handleCreateAndAssignCategory() {
    const name = newCategoryName.trim();
    if (!name || !item.leadId || isCreatingCategory) return;
    isCreatingCategory = true;
    categoryError = null;
    try {
      const catId = await crmService.createCategory(name);
      await crmService.addLeadsToCategory([item.leadId], catId);
      const newCat: Category = { id: catId, name };
      allCategories = [...allCategories, newCat];
      assignedCategories = [...assignedCategories, newCat];
      leadCategoriesCache.set(item.leadId, assignedCategories);
      newCategoryName = '';
    } catch (err) {
      categoryError = err instanceof Error ? err.message : String(err);
    } finally {
      isCreatingCategory = false;
    }
  }

  async function openRelinkModal() {
    relinkError = null;
    selectedLeadId = null;
    leadSearchText = '';
    isRelinkModalOpen = true;
    isLoadingLeads = true;
    try {
      leads = await crmService.getLeads();
    } catch (err) {
      relinkError = err instanceof Error ? err.message : String(err);
    } finally {
      isLoadingLeads = false;
    }
  }

  async function handleConfirmRelink() {
    if (!selectedLeadId) return;
    isRelinking = true;
    relinkError = null;
    try {
      if (item.leadId) {
        await relinkConversationLead(item.leadId, selectedLeadId);
      } else if (item.identityId) {
        await linkConversationIdentity(item.identityId, selectedLeadId);
      } else {
        return;
      }
      isRelinkModalOpen = false;
      onRelinked?.(selectedLeadId);
    } catch (err) {
      relinkError = err instanceof Error ? err.message : String(err);
    } finally {
      isRelinking = false;
    }
  }

  async function handleConfirmUnlink() {
    if (!item.identityId && !item.leadId) return;
    isUnlinking = true;
    unlinkError = null;
    try {
      if (item.leadId) {
        await unlinkConversationLead(item.leadId);
      } else if (item.identityId) {
        await unlinkConversationIdentity(item.identityId);
      }
      isUnlinkModalOpen = false;
      onUnlinked?.();
    } catch (err) {
      unlinkError = err instanceof Error ? err.message : String(err);
    } finally {
      isUnlinking = false;
    }
  }
</script>

<div class="flex items-center gap-2 flex-wrap">
  {#if item.leadId && assignedCategories.length > 0}
    {#each assignedCategories as cat (cat.id)}
      <Badge variant="secondary" class="flex items-center gap-1 text-xs px-2 py-0.5 font-normal">
        <Tag class="size-3 text-muted-foreground" />
        <span>{cat.name}</span>
      </Badge>
    {/each}
  {/if}

  {#if item.leadId}
    <Button
      type="button"
      variant="outline"
      size="sm"
      onclick={openCategoryModal}
      class="flex items-center gap-1.5 text-xs"
    >
      <Tag class="size-3.5" data-icon="inline-start" />
      <span>{assignedCategories.length > 0 ? 'Category' : '+ Category'}</span>
    </Button>
  {/if}

  <Button
    type="button"
    variant="outline"
    size="sm"
    disabled={!item.identityId && !item.leadId}
    onclick={openRelinkModal}
    class="flex items-center gap-1.5 text-xs"
  >
    <RefreshCw class="size-3.5" data-icon="inline-start" />
    <span>Change Lead</span>
  </Button>

  <Button
    type="button"
    variant="outline"
    size="sm"
    disabled={!item.identityId && !item.leadId}
    onclick={() => (isUnlinkModalOpen = true)}
    class="flex items-center gap-1.5 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive"
  >
    <Unlink class="size-3.5 text-destructive" data-icon="inline-start" />
    <span>Unlink</span>
  </Button>
</div>

<!-- Modal 1: Change Linked Lead -->
<Dialog.Root bind:open={isRelinkModalOpen}>
  <Dialog.Content class="sm:max-w-[480px]">
    <Dialog.Header>
      <Dialog.Title>Change Linked Lead</Dialog.Title>
      <Dialog.Description>
        Select a new lead in CRM to replace for this conversation.
      </Dialog.Description>
    </Dialog.Header>

    <div class="flex flex-col gap-3 py-3">
      {#if relinkError}
        <div class="text-xs text-destructive bg-destructive/10 p-2.5 rounded-md flex items-center gap-1.5" role="alert">
          <AlertCircle class="size-4 shrink-0" />
          <span>{relinkError}</span>
        </div>
      {/if}

      <div class="relative">
        <Search class="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
        <Input
          bind:value={leadSearchText}
          placeholder="Search by name, email, phone..."
          class="pl-8 text-sm"
        />
      </div>

      <div class="max-h-[220px] overflow-y-auto border rounded-md p-1 flex flex-col gap-1">
        {#if isLoadingLeads}
          <div class="text-xs text-center text-muted-foreground py-4">Loading leads list...</div>
        {:else if filteredLeads.length === 0}
          <div class="text-xs text-center text-muted-foreground py-4">No matching leads found.</div>
        {:else}
          {#each filteredLeads as lead (lead.id)}
            <button
              type="button"
              onclick={() => (selectedLeadId = lead.id ?? null)}
              class="w-full text-left px-3 py-2 rounded-md text-sm transition-colors flex items-center justify-between {selectedLeadId === lead.id ? 'bg-accent text-accent-foreground font-medium' : 'hover:bg-muted/50'}"
            >
              <div>
                <div class="font-medium">{lead.name}</div>
                <div class="text-xs text-muted-foreground">{lead.company || 'Individual'} {lead.phone ? `• ${lead.phone}` : ''}</div>
              </div>
              {#if selectedLeadId === lead.id}
                <span class="text-xs font-semibold text-primary">Selected</span>
              {/if}
            </button>
          {/each}
        {/if}
      </div>
    </div>

    <Dialog.Footer>
      <Button variant="outline" onclick={() => (isRelinkModalOpen = false)}>Cancel</Button>
      <Button
        disabled={!selectedLeadId || isRelinking}
        onclick={handleConfirmRelink}
      >
        {isRelinking ? 'Updating...' : 'Confirm Change'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Modal 2: Confirm Unlink -->
<Dialog.Root bind:open={isUnlinkModalOpen}>
  <Dialog.Content class="sm:max-w-[420px]">
    <Dialog.Header>
      <Dialog.Title>Confirm Unlink</Dialog.Title>
      <Dialog.Description>
        Are you sure you want to unlink this conversation from lead <strong>{item.title}</strong>? The conversation will return to the Unlinked (Unknown) category.
      </Dialog.Description>
    </Dialog.Header>

    {#if unlinkError}
      <div class="text-xs text-destructive bg-destructive/10 p-2.5 rounded-md flex items-center gap-1.5 my-2" role="alert">
        <AlertCircle class="size-4 shrink-0" />
        <span>{unlinkError}</span>
      </div>
    {/if}

    <Dialog.Footer class="mt-4">
      <Button variant="outline" onclick={() => (isUnlinkModalOpen = false)}>Cancel</Button>
      <Button
        variant="destructive"
        disabled={isUnlinking}
        onclick={handleConfirmUnlink}
      >
        {isUnlinking ? 'Unlinking...' : 'Confirm Unlink'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Modal 3: Manage Category -->
<Dialog.Root bind:open={isCategoryModalOpen}>
  <Dialog.Content class="sm:max-w-[440px]">
    <Dialog.Header>
      <Dialog.Title>Manage Lead Categories</Dialog.Title>
      <Dialog.Description>
        Select or add categories for lead <strong>{item.title}</strong> for outreach campaigns.
      </Dialog.Description>
    </Dialog.Header>

    <div class="flex flex-col gap-3 py-2">
      {#if categoryError}
        <div class="text-xs text-destructive bg-destructive/10 p-2.5 rounded-md flex items-center gap-1.5" role="alert">
          <AlertCircle class="size-4 shrink-0" />
          <span>{categoryError}</span>
        </div>
      {/if}

      <div class="relative">
        <Search class="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
        <Input
          bind:value={categorySearchText}
          placeholder="Search categories..."
          class="pl-8 text-sm"
        />
      </div>

      <div class="max-h-[200px] overflow-y-auto border rounded-md p-1 flex flex-col gap-1">
        {#if isLoadingCategories}
          <div class="text-xs text-center text-muted-foreground py-4">Loading categories...</div>
        {:else if filteredCategories.length === 0}
          <div class="text-xs text-center text-muted-foreground py-4">No matching categories.</div>
        {:else}
          {#each filteredCategories as category (category.id)}
            {@const isAssigned = category.id !== undefined && assignedCategoryIds.has(category.id)}
            <button
              type="button"
              onclick={() => toggleCategory(category)}
              class="w-full text-left px-3 py-2 rounded-md text-sm transition-colors flex items-center justify-between {isAssigned ? 'bg-accent text-accent-foreground font-medium' : 'hover:bg-muted/50'}"
            >
              <div class="flex items-center gap-2">
                <Tag class="size-3.5 text-muted-foreground" />
                <span>{category.name}</span>
              </div>
              {#if isAssigned}
                <div class="flex items-center gap-1 text-xs text-primary font-semibold">
                  <Check class="size-4" />
                  <span>Selected</span>
                </div>
              {/if}
            </button>
          {/each}
        {/if}
      </div>

      <!-- Add Category Inline -->
      <div class="flex items-center gap-2 pt-2 border-t border-border">
        <Input
          bind:value={newCategoryName}
          placeholder="New category name..."
          class="text-sm flex-1"
          onkeydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              handleCreateAndAssignCategory();
            }
          }}
        />
        <Button
          type="button"
          size="sm"
          disabled={!newCategoryName.trim() || isCreatingCategory}
          onclick={handleCreateAndAssignCategory}
          class="flex items-center gap-1 shrink-0"
        >
          <Plus class="size-4" />
          <span>Create</span>
        </Button>
      </div>
    </div>

    <Dialog.Footer>
      <Button variant="outline" onclick={() => (isCategoryModalOpen = false)}>Close</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
