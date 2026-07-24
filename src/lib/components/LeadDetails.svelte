<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import * as crmService from '$lib/services/crmService';
  import type { Lead, Activity } from '$lib/services/crmService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import * as Select from '$lib/components/ui/select';
  import * as Dialog from '$lib/components/ui/dialog';
  import { 
    X, Edit2, Trash2, Save, Phone, Mail, Users, CheckCircle2, XCircle, 
    AlertCircle, Calendar, MessageSquare, Clock, MapPin, Building
  } from 'lucide-svelte';

  let { leadId, onClose, onRefresh } = $props<{ leadId: number; onClose: () => void; onRefresh?: () => void }>();

  let lead = $state<Lead | null>(null);
  let activities = $state<Activity[]>([]);
  let loading = $state(false);
  let errorMsg = $state<string | null>(null);

  // Edit mode states
  let isEditing = $state(false);
  let editName = $state('');
  let editCompany = $state('');
  let editStatus = $state('');
  let editValue = $state(0);
  let editOwner = $state('');
  let editEmail = $state('');
  let editPhone = $state('');
  let editAbout = $state('');
  let editAddress = $state('');
  let savingLead = $state(false);
  let deletingLead = $state(false);

  function startEditing() {
    if (!lead) return;
    editName = lead.name;
    editCompany = lead.company;
    editStatus = lead.status;
    editValue = lead.value;
    editOwner = lead.owner || '';
    editEmail = lead.email;
    editPhone = lead.phone || '';
    editAbout = lead.about || '';
    editAddress = lead.address || '';
    isEditing = true;
  }

  async function handleSaveLead(e: Event) {
    e.preventDefault();
    if (!lead || !editName.trim() || !editCompany.trim() || !editEmail.trim()) return;

    savingLead = true;
    errorMsg = null;
    try {
      await crmService.updateLead(
        leadId,
        editName.trim(),
        editCompany.trim(),
        editStatus,
        editValue,
        editOwner.trim(),
        editEmail.trim(),
        editPhone.trim(),
        editAbout.trim() || null,
        editAddress.trim() || null
      );
      isEditing = false;
      await loadLeadDetails(leadId);
      if (onRefresh) onRefresh();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      savingLead = false;
    }
  }

  let isDeleteLeadModalOpen = $state(false);

  function promptDeleteLead() {
    isDeleteLeadModalOpen = true;
  }

  async function confirmDeleteLead() {
    if (!lead) return;
    deletingLead = true;
    errorMsg = null;
    try {
      await crmService.deleteLead(leadId);
      isDeleteLeadModalOpen = false;
      if (onRefresh) onRefresh();
      onClose();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      deletingLead = false;
    }
  }

  // Form states
  let activityType = $state('Call');
  let activityContent = $state('');
  let logging = $state(false);

  $effect(() => {
    if (leadId) loadLeadDetails(leadId);
  });

  async function loadLeadDetails(id: number) {
    loading = true;
    errorMsg = null;
    try {
      const [leadData, activitiesData] = await Promise.all([
        crmService.getLeadDetails(id),
        crmService.getActivities(id)
      ]);
      if (id !== leadId) return;
      lead = leadData;
      activities = activitiesData.sort(
        (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );
    } catch (err) {
      if (id === leadId) {
        errorMsg = err instanceof Error ? err.message : String(err);
      }
    } finally {
      if (id === leadId) loading = false;
    }
  }

  async function handleLogActivity(e: Event) {
    e.preventDefault();
    if (!activityContent.trim()) return;

    logging = true;
    errorMsg = null;
    const currentLeadId = leadId;
    try {
      const performer = lead?.owner || 'Self';
      await crmService.addActivity(currentLeadId, activityType, activityContent.trim(), performer);
      if (currentLeadId !== leadId) return;
      activityContent = '';
      const activitiesData = await crmService.getActivities(currentLeadId);
      if (currentLeadId !== leadId) return;
      activities = activitiesData.sort(
        (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );
    } catch (err) {
      if (currentLeadId === leadId) {
        errorMsg = err instanceof Error ? err.message : String(err);
      }
    } finally {
      if (currentLeadId === leadId) logging = false;
    }
  }

  function formatDateTime(dateStr: string) {
    if (!dateStr) return '';
    try {
      const date = new Date(dateStr);
      if (isNaN(date.getTime())) return dateStr;
      return date.toLocaleString('en-US', {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit'
      });
    } catch (e) {
      return dateStr;
    }
  }
</script>

<!-- Backdrop overlay -->
<div 
  class="fixed inset-0 z-50 bg-background/80 backdrop-blur-xs" 
  transition:fade={{ duration: 150 }} 
  onclick={onClose}
  role="presentation"
></div>

<!-- Drawer container -->
<aside 
  class="fixed top-0 right-0 bottom-0 z-50 w-full max-w-2xl bg-card border-l border-border shadow-2xl flex flex-col overflow-hidden text-card-foreground"
  transition:fly={{ x: 640, duration: 250 }}
>
  <!-- Drawer Header -->
  <header class="h-14 px-6 border-b border-border flex items-center justify-between shrink-0 bg-muted/20">
    <div class="flex items-center gap-2">
      <h3 class="text-sm font-bold text-foreground tracking-wide">Lead Profile Details</h3>
      {#if lead}
        <Badge variant="outline" class="text-[10px] uppercase font-mono">{lead.status}</Badge>
      {/if}
    </div>
    <Button variant="ghost" size="sm" onclick={onClose} class="h-8 w-8 p-0 text-muted-foreground hover:text-foreground">
      <X class="w-4 h-4" />
    </Button>
  </header>

  {#if loading && !lead}
    <div class="flex-1 flex flex-col items-center justify-center p-6 text-muted-foreground text-xs gap-2">
      <div class="w-5 h-5 rounded-full border-2 border-primary border-t-transparent animate-spin"></div>
      <p>Loading lead details...</p>
    </div>
  {:else if errorMsg && !lead}
    <div class="flex-1 flex flex-col items-center justify-center p-6 text-destructive text-xs gap-2">
      <AlertCircle class="w-6 h-6" />
      <p>{errorMsg}</p>
    </div>
  {:else if lead}
    <div class="flex-1 overflow-y-auto p-6 space-y-6">
      {#if errorMsg}
        <div class="p-3 rounded-lg bg-destructive/10 border border-destructive/30 text-destructive text-xs flex items-center gap-2">
          <AlertCircle class="w-4 h-4 shrink-0" />
          <span>{errorMsg}</span>
        </div>
      {/if}

      <!-- Profile Summary Card -->
      <div class="p-4 rounded-xl border border-border bg-card shadow-xs relative space-y-3">
        <div class="flex items-start gap-4">
          <div class="w-14 h-14 rounded-full bg-emerald-500/10 text-emerald-600 font-bold text-base flex items-center justify-center border border-emerald-500/20 shrink-0 shadow-2xs">
            {lead.name.split(' ').map((n: string) => n[0]).join('').toUpperCase().substring(0, 2)}
          </div>
          <div class="flex-1 min-w-0 pr-10">
            <h4 class="text-lg font-bold text-foreground truncate">{lead.name}</h4>
            <p class="text-xs text-muted-foreground font-medium">{lead.company}</p>
            <div class="flex flex-wrap gap-2 mt-2.5">
              <Badge variant="default" class="text-[10px] py-0.5 px-2.5">{lead.status}</Badge>
              {#if lead.has_zalo === 1}
                <Badge variant="outline" class="gap-1 border-sky-500/30 text-sky-600 bg-sky-500/10 text-[10px] py-0.5 px-2">
                  <CheckCircle2 class="w-3 h-3 text-sky-500" /> Zalo Verified
                </Badge>
              {:else if lead.has_zalo === 0}
                <Badge variant="outline" class="gap-1 border-slate-300 text-slate-500 bg-slate-100 text-[10px] py-0.5 px-2">
                  <XCircle class="w-3 h-3 text-slate-400" /> No Zalo
                </Badge>
              {/if}
            </div>
          </div>
          {#if !isEditing}
            <Button variant="outline" size="sm" onclick={startEditing} class="h-8 gap-1.5 absolute top-4 right-4 text-xs shadow-2xs">
              <Edit2 class="w-3.5 h-3.5" />
              Edit
            </Button>
          {/if}
        </div>
      </div>

      {#if isEditing}
        <!-- Edit Lead Form (2-column responsive layout) -->
        <form onsubmit={handleSaveLead} class="space-y-4 text-xs border border-border bg-card p-5 rounded-xl shadow-xs">
          <div class="flex items-center justify-between border-b border-border pb-3">
            <h5 class="text-xs font-bold text-foreground uppercase tracking-wider">Edit Lead Information</h5>
            <span class="text-[10px] text-muted-foreground">* Required fields</span>
          </div>

          <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div class="space-y-1">
              <label for="edit-name" class="font-medium text-foreground">Name *</label>
              <Input id="edit-name" bind:value={editName} required class="h-9 text-xs" />
            </div>
            <div class="space-y-1">
              <label for="edit-company" class="font-medium text-foreground">Company *</label>
              <Input id="edit-company" bind:value={editCompany} required class="h-9 text-xs" />
            </div>

            <div class="space-y-1">
              <label for="edit-email" class="font-medium text-foreground">Email *</label>
              <Input id="edit-email" type="email" bind:value={editEmail} required class="h-9 text-xs" />
            </div>
            <div class="space-y-1">
              <label for="edit-phone" class="font-medium text-foreground">Phone</label>
              <Input id="edit-phone" bind:value={editPhone} class="h-9 text-xs" />
            </div>

            <div class="space-y-1">
              <label for="edit-status" class="font-medium text-foreground">Status</label>
              <select
                id="edit-status"
                bind:value={editStatus}
                class="w-full h-9 text-xs border border-input bg-background rounded-md px-3 py-1 text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring cursor-pointer"
              >
                <option value="New">New</option>
                <option value="Contacted">Contacted</option>
                <option value="Qualified">Qualified</option>
                <option value="Lost">Lost</option>
                <option value="Converted">Converted (Contact)</option>
              </select>
            </div>
            <div class="space-y-1">
              <label for="edit-value" class="font-medium text-foreground">Est. Value ($)</label>
              <Input id="edit-value" type="number" bind:value={editValue} min="0" class="h-9 text-xs" />
            </div>

            <div class="space-y-1 sm:col-span-2">
              <label for="edit-owner" class="font-medium text-foreground">Owner</label>
              <Input id="edit-owner" bind:value={editOwner} class="h-9 text-xs" />
            </div>

            <div class="space-y-1 sm:col-span-2">
              <label for="edit-address" class="font-medium text-foreground">Address</label>
              <Input id="edit-address" bind:value={editAddress} class="h-9 text-xs" placeholder="Full street address..." />
            </div>

            <div class="space-y-1 sm:col-span-2">
              <label for="edit-about" class="font-medium text-foreground">About / Notes</label>
              <textarea id="edit-about" bind:value={editAbout} rows="3" class="w-full p-3 rounded-md border border-input bg-background text-xs leading-relaxed focus:outline-hidden focus:ring-1 focus:ring-ring resize-y"></textarea>
            </div>
          </div>

          <div class="flex items-center justify-between pt-4 border-t border-border">
            <Button type="button" variant="destructive" size="sm" onclick={promptDeleteLead} disabled={deletingLead} class="gap-1.5 h-9 px-4">
              <Trash2 class="w-3.5 h-3.5" />
              {deletingLead ? 'Deleting...' : 'Delete Lead'}
            </Button>
            <div class="flex items-center gap-2">
              <Button type="button" variant="outline" size="sm" onclick={() => isEditing = false} class="h-9 px-4">Cancel</Button>
              <Button type="submit" size="sm" disabled={savingLead} class="gap-1.5 h-9 px-5">
                <Save class="w-3.5 h-3.5" />
                {savingLead ? 'Saving...' : 'Save Changes'}
              </Button>
            </div>
          </div>
        </form>
      {:else}
        <!-- Contact Details -->
        <div class="space-y-3">
          <h5 class="text-xs font-bold text-muted-foreground uppercase tracking-wider">Contact Overview</h5>
          <div class="rounded-xl border border-border bg-card p-4 space-y-3 text-xs shadow-xs">
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 pb-3 border-b border-border/60">
              <div>
                <span class="text-muted-foreground flex items-center gap-1.5 mb-1 font-medium"><Mail class="w-3.5 h-3.5 text-primary" /> Email</span>
                <a href="mailto:{lead.email}" class="text-primary font-semibold hover:underline break-all">{lead.email}</a>
              </div>
              <div>
                <span class="text-muted-foreground flex items-center gap-1.5 mb-1 font-medium"><Phone class="w-3.5 h-3.5 text-primary" /> Phone</span>
                <span class="text-foreground font-semibold">{lead.phone || '—'}</span>
              </div>
            </div>

            <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 pb-3 border-b border-border/60">
              <div>
                <span class="text-muted-foreground flex items-center gap-1.5 mb-1 font-medium"><Users class="w-3.5 h-3.5 text-primary" /> Owner</span>
                <span class="text-foreground font-semibold">{lead.owner || '—'}</span>
              </div>
              <div>
                <span class="text-muted-foreground flex items-center gap-1.5 mb-1 font-medium"><Building class="w-3.5 h-3.5 text-primary" /> Est. Value</span>
                <span class="text-emerald-600 font-bold text-sm">${(lead.value || 0).toLocaleString()}</span>
              </div>
            </div>

            <div class="pb-1">
              <span class="text-muted-foreground flex items-center gap-1.5 mb-1 font-medium"><MapPin class="w-3.5 h-3.5 text-primary" /> Address</span>
              <span class="text-foreground font-medium leading-relaxed break-words block bg-muted/30 p-2.5 rounded-lg border border-border/50">{lead.address || '—'}</span>
            </div>

            {#if lead.about}
              <div class="pt-2">
                <span class="text-muted-foreground block mb-1 font-medium">About</span>
                <p class="p-3 rounded-lg bg-muted/30 border border-border/60 text-foreground leading-relaxed break-words">{lead.about}</p>
              </div>
            {/if}
          </div>
        </div>

        <!-- Log Activity Form -->
        <div class="space-y-3 pt-2">
          <h5 class="text-xs font-bold text-muted-foreground uppercase tracking-wider">Log Activity</h5>
          <form onsubmit={handleLogActivity} class="space-y-3 text-xs rounded-xl border border-border bg-card p-4 shadow-xs">
            <select
              bind:value={activityType}
              class="w-full h-9 text-xs border border-input bg-background rounded-md px-3 py-1 text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring cursor-pointer"
            >
              <option value="Call">Call</option>
              <option value="Email">Email</option>
              <option value="Meeting">Meeting</option>
            </select>
            <textarea
              bind:value={activityContent}
              placeholder="What did you discuss? Enter notes here..."
              rows="3"
              required
              class="w-full p-3 rounded-md border border-input bg-background text-foreground placeholder:text-muted-foreground text-xs leading-relaxed focus:outline-hidden focus:ring-1 focus:ring-ring resize-y"
            ></textarea>
            <Button type="submit" size="sm" class="w-full h-9 text-xs font-medium" disabled={logging || !activityContent.trim()}>
              {logging ? 'Logging...' : 'Log Activity'}
            </Button>
          </form>
        </div>

        <!-- Activity Timeline -->
        <div class="space-y-3 pt-2">
          <h5 class="text-xs font-bold text-muted-foreground uppercase tracking-wider">Activity History</h5>
          {#if activities.length === 0}
            <p class="text-xs text-muted-foreground italic p-4 rounded-xl border border-border bg-card text-center">No activities logged yet.</p>
          {:else}
            <div class="space-y-3 relative before:absolute before:left-3 before:top-2 before:bottom-2 before:w-0.5 before:bg-border">
              {#each activities as activity}
                <div class="flex items-start gap-3 relative">
                  <div class="w-6.5 h-6.5 rounded-full bg-background border border-border flex items-center justify-center text-primary z-10 text-[10px] shrink-0">
                    <Clock class="w-3 h-3 text-muted-foreground" />
                  </div>
                  <div class="flex-1 p-3 rounded-xl border border-border/70 bg-card text-xs space-y-1.5 shadow-2xs">
                    <div class="flex items-center justify-between font-semibold">
                      <span class="text-foreground">{activity.activity_type}</span>
                      <span class="text-[10px] text-muted-foreground font-normal">{formatDateTime(activity.created_at)}</span>
                    </div>
                    <p class="text-muted-foreground leading-relaxed break-words">{activity.content}</p>
                    <span class="text-[10px] text-muted-foreground/70 block">Logged by {activity.performed_by}</span>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</aside>

<!-- Delete Lead Confirmation Dialog -->
<Dialog.Root bind:open={isDeleteLeadModalOpen}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Delete Lead</Dialog.Title>
    </Dialog.Header>
    <div class="py-2 text-xs text-muted-foreground leading-relaxed">
      Are you sure you want to delete lead <strong class="text-foreground">{lead?.name}</strong>?
      This action will remove the lead and all associated activity records permanently.
    </div>
    <Dialog.Footer class="gap-2">
      <Button variant="outline" size="sm" onclick={() => isDeleteLeadModalOpen = false} disabled={deletingLead}>Cancel</Button>
      <Button variant="destructive" size="sm" onclick={confirmDeleteLead} disabled={deletingLead}>
        {deletingLead ? 'Deleting...' : 'Delete Lead'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
