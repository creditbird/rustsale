<script lang="ts">
  import { tick } from 'svelte';
  import { linkConversationIdentity, type InboxItem } from '$lib/services/conversationService';
  import * as crmService from '$lib/services/crmService';
  import type { Lead } from '$lib/services/crmService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Link, UserPlus, Search, AlertCircle } from 'lucide-svelte';

  let {
    item,
    onLinked
  }: {
    item: InboxItem;
    onLinked?: (leadId: number) => void;
  } = $props();

  let isLinkModalOpen = $state(false);
  let isCreateModalOpen = $state(false);

  // Link lead state
  let leads = $state<Lead[]>([]);
  let isLoadingLeads = $state(false);
  let leadSearchText = $state('');
  let selectedLeadId = $state<number | null>(null);
  let isLinking = $state(false);
  let linkError = $state<string | null>(null);

  // Create lead state
  let newLeadName = $state('');
  let newLeadCompany = $state('');
  let newLeadEmail = $state('');
  let newLeadPhone = $state('');
  let isCreating = $state(false);
  let createError = $state<string | null>(null);
  let createValidationError = $state<string | null>(null);

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

  async function openLinkModal() {
    linkError = null;
    selectedLeadId = null;
    leadSearchText = '';
    isLinkModalOpen = true;
    isLoadingLeads = true;
    try {
      leads = await crmService.getLeads();
    } catch (err) {
      linkError = err instanceof Error ? err.message : String(err);
    } finally {
      isLoadingLeads = false;
    }
  }

  async function handleConfirmLink() {
    if (!selectedLeadId || !item.identityId) return;
    isLinking = true;
    linkError = null;
    try {
      await linkConversationIdentity(item.identityId, selectedLeadId);
      isLinkModalOpen = false;
      onLinked?.(selectedLeadId);
    } catch (err) {
      linkError = err instanceof Error ? err.message : String(err);
    } finally {
      isLinking = false;
    }
  }

  async function openCreateModal() {
    createError = null;
    createValidationError = null;
    newLeadName = item.title || item.participants?.[0]?.displayName || '';
    newLeadCompany = '';
    newLeadEmail = '';
    newLeadPhone = '';
    isCreateModalOpen = true;
    await tick();
  }

  async function handleConfirmCreate() {
    createValidationError = null;
    createError = null;
    const trimmedName = newLeadName.trim();
    if (!trimmedName) {
      createValidationError = 'Lead name cannot be empty';
      return;
    }
    if (!item.identityId) return;

    isCreating = true;
    try {
      const channelName = item.channels?.[0]?.channel ?? 'conversation';
      const leadId = await crmService.addLead(
        trimmedName,
        newLeadCompany.trim() || 'Individual',
        'New',
        0,
        'Self',
        newLeadEmail.trim(),
        newLeadPhone.trim(),
        `Created from ${channelName}`
      );
      await linkConversationIdentity(item.identityId, leadId);
      isCreateModalOpen = false;
      onLinked?.(leadId);
    } catch (err) {
      createError = err instanceof Error ? err.message : String(err);
    } finally {
      isCreating = false;
    }
  }
</script>

<div class="flex items-center gap-2">
  <Button
    type="button"
    variant="outline"
    size="sm"
    disabled={!item.identityId}
    onclick={openLinkModal}
    class="flex items-center gap-1.5 text-xs"
  >
    <Link class="size-3.5" />
    <span>Link Lead</span>
  </Button>

  <Button
    type="button"
    variant="default"
    size="sm"
    disabled={!item.identityId}
    onclick={openCreateModal}
    class="flex items-center gap-1.5 text-xs"
  >
    <UserPlus class="size-3.5" />
    <span>Create Lead</span>
  </Button>
</div>

<!-- Modal 1: Link Existing Lead -->
<Dialog.Root bind:open={isLinkModalOpen}>
  <Dialog.Content class="sm:max-w-[480px]">
    <Dialog.Header>
      <Dialog.Title>Link Existing Lead</Dialog.Title>
      <Dialog.Description>
        Search and select a lead in CRM to assign to this conversation.
      </Dialog.Description>
    </Dialog.Header>

    <div class="flex flex-col gap-3 py-3">
      {#if linkError}
        <div class="text-xs text-destructive bg-destructive/10 p-2.5 rounded-md flex items-center gap-1.5" role="alert">
          <AlertCircle class="size-4 shrink-0" />
          <span>{linkError}</span>
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

      <div class="max-h-[220px] overflow-y-auto border rounded-md p-1 space-y-1">
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
      <Button variant="outline" onclick={() => (isLinkModalOpen = false)}>Cancel</Button>
      <Button
        disabled={!selectedLeadId || isLinking}
        onclick={handleConfirmLink}
      >
        {isLinking ? 'Linking...' : 'Confirm Link'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Modal 2: Create New Lead -->
<Dialog.Root bind:open={isCreateModalOpen}>
  <Dialog.Content class="sm:max-w-[440px]">
    <Dialog.Header>
      <Dialog.Title>Create New Lead</Dialog.Title>
      <Dialog.Description>
        Create new lead info in CRM and link with this conversation.
      </Dialog.Description>
    </Dialog.Header>

    <div class="flex flex-col gap-3 py-3">
      {#if createValidationError || createError}
        <div class="text-xs text-destructive bg-destructive/10 p-2.5 rounded-md flex items-center gap-1.5" role="alert">
          <AlertCircle class="size-4 shrink-0" />
          <span>{createValidationError || createError}</span>
        </div>
      {/if}

      <div class="space-y-3">
        <div>
          <label for="lead-name-input" class="text-sm font-medium text-foreground block mb-1">
            Full Name *
          </label>
          <Input id="lead-name-input" aria-label="Full Name" bind:value={newLeadName} placeholder="Enter lead name..." />
        </div>

        <div>
          <label for="lead-company-input" class="text-sm font-medium text-foreground block mb-1">
            Company
          </label>
          <Input id="lead-company-input" aria-label="Company" bind:value={newLeadCompany} placeholder="Individual" />
        </div>

        <div>
          <label for="lead-email-input" class="text-sm font-medium text-foreground block mb-1">
            Email
          </label>
          <Input id="lead-email-input" aria-label="Email" type="email" bind:value={newLeadEmail} placeholder="email@example.com" />
        </div>

        <div>
          <label for="lead-phone-input" class="text-sm font-medium text-foreground block mb-1">
            Phone Number
          </label>
          <Input id="lead-phone-input" aria-label="Phone Number" type="tel" bind:value={newLeadPhone} placeholder="0901234567" />
        </div>
      </div>
    </div>

    <Dialog.Footer>
      <Button variant="outline" onclick={() => (isCreateModalOpen = false)}>Cancel</Button>
      <Button disabled={isCreating} onclick={handleConfirmCreate}>
        {isCreating ? 'Creating...' : 'Create & Link'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
