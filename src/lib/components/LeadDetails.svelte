<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import * as crmService from '$lib/services/crmService';
  import type { Lead, Activity } from '$lib/services/crmService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
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

  async function handleDeleteLead() {
    if (!lead) return;
    const confirmDelete = confirm(`Are you sure you want to delete ${lead.name}?`);
    if (!confirmDelete) return;

    deletingLead = true;
    errorMsg = null;
    try {
      await crmService.deleteLead(leadId);
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
  class="fixed top-0 right-0 bottom-0 z-50 w-full max-w-md bg-card border-l border-border shadow-xl flex flex-col overflow-hidden text-card-foreground"
  transition:fly={{ x: 400, duration: 250 }}
>
  <!-- Drawer Header -->
  <header class="h-14 px-5 border-b border-border flex items-center justify-between shrink-0">
    <h3 class="text-sm font-bold text-foreground">Lead Profile Details</h3>
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
    <div class="flex-1 overflow-y-auto p-5 space-y-6">
      {#if errorMsg}
        <div class="p-3 rounded bg-destructive/10 border border-destructive/30 text-destructive text-xs flex items-center gap-2">
          <AlertCircle class="w-4 h-4 shrink-0" />
          <span>{errorMsg}</span>
        </div>
      {/if}

      <!-- Profile Summary Card -->
      <div class="p-4 rounded-xl border border-border bg-muted/20 relative space-y-3">
        <div class="flex items-start gap-3">
          <div class="w-12 h-12 rounded-full bg-emerald-500/10 text-emerald-600 font-bold text-sm flex items-center justify-center border border-emerald-500/20 shrink-0">
            {lead.name.split(' ').map((n: string) => n[0]).join('').toUpperCase().substring(0, 2)}
          </div>
          <div class="flex-1 min-w-0 pr-8">
            <h4 class="text-base font-bold text-foreground truncate">{lead.name}</h4>
            <p class="text-xs text-muted-foreground truncate">{lead.company}</p>
            <div class="flex flex-wrap gap-1.5 mt-2">
              <Badge variant="default" class="text-[10px] py-0">{lead.status}</Badge>
              {#if lead.has_zalo === 1}
                <Badge variant="outline" class="gap-1 border-sky-500/30 text-sky-600 bg-sky-500/10 text-[10px] py-0">
                  <CheckCircle2 class="w-3 h-3 text-sky-500" /> Zalo Verified
                </Badge>
              {:else if lead.has_zalo === 0}
                <Badge variant="outline" class="gap-1 border-slate-300 text-slate-500 bg-slate-100 text-[10px] py-0">
                  <XCircle class="w-3 h-3 text-slate-400" /> No Zalo
                </Badge>
              {/if}
            </div>
          </div>
          {#if !isEditing}
            <Button variant="ghost" size="sm" onclick={startEditing} class="h-8 w-8 p-0 absolute top-3 right-3 text-muted-foreground hover:text-foreground">
              <Edit2 class="w-4 h-4" />
            </Button>
          {/if}
        </div>
      </div>

      {#if isEditing}
        <!-- Edit Lead Form -->
        <form onsubmit={handleSaveLead} class="space-y-3 text-xs border-t border-border pt-4">
          <h5 class="text-xs font-bold text-muted-foreground uppercase tracking-wider">Edit Information</h5>
          <div class="space-y-1">
            <label for="edit-name" class="font-medium text-foreground">Name *</label>
            <Input id="edit-name" bind:value={editName} required />
          </div>
          <div class="space-y-1">
            <label for="edit-company" class="font-medium text-foreground">Company *</label>
            <Input id="edit-company" bind:value={editCompany} required />
          </div>
          <div class="space-y-1">
            <label for="edit-status" class="font-medium text-foreground">Status</label>
            <select id="edit-status" bind:value={editStatus} class="w-full h-9 px-3 rounded-md border border-input bg-background">
              <option value="New">New</option>
              <option value="Contacted">Contacted</option>
              <option value="Qualified">Qualified</option>
              <option value="Lost">Lost</option>
              <option value="Converted">Converted (Contact)</option>
            </select>
          </div>
          <div class="space-y-1">
            <label for="edit-value" class="font-medium text-foreground">Est. Value ($)</label>
            <Input id="edit-value" type="number" bind:value={editValue} min="0" />
          </div>
          <div class="space-y-1">
            <label for="edit-owner" class="font-medium text-foreground">Owner</label>
            <Input id="edit-owner" bind:value={editOwner} />
          </div>
          <div class="space-y-1">
            <label for="edit-email" class="font-medium text-foreground">Email *</label>
            <Input id="edit-email" type="email" bind:value={editEmail} required />
          </div>
          <div class="space-y-1">
            <label for="edit-phone" class="font-medium text-foreground">Phone</label>
            <Input id="edit-phone" bind:value={editPhone} />
          </div>
          <div class="space-y-1">
            <label for="edit-address" class="font-medium text-foreground">Address</label>
            <Input id="edit-address" bind:value={editAddress} />
          </div>
          <div class="space-y-1">
            <label for="edit-about" class="font-medium text-foreground">About</label>
            <textarea id="edit-about" bind:value={editAbout} rows="3" class="w-full p-2.5 rounded border border-input bg-background"></textarea>
          </div>

          <div class="flex items-center justify-between pt-3 border-t border-border">
            <Button type="button" variant="destructive" size="sm" onclick={handleDeleteLead} disabled={deletingLead} class="gap-1.5">
              <Trash2 class="w-3.5 h-3.5" />
              {deletingLead ? 'Deleting...' : 'Delete'}
            </Button>
            <div class="flex items-center gap-2">
              <Button type="button" variant="outline" size="sm" onclick={() => isEditing = false}>Cancel</Button>
              <Button type="submit" size="sm" disabled={savingLead} class="gap-1.5">
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
          <div class="space-y-2 text-xs">
            <div class="flex justify-between py-1.5 border-b border-border/60">
              <span class="text-muted-foreground flex items-center gap-1.5"><Mail class="w-3.5 h-3.5" /> Email</span>
              <a href="mailto:{lead.email}" class="text-primary font-medium hover:underline truncate max-w-[200px]">{lead.email}</a>
            </div>
            <div class="flex justify-between py-1.5 border-b border-border/60">
              <span class="text-muted-foreground flex items-center gap-1.5"><Phone class="w-3.5 h-3.5" /> Phone</span>
              <span class="text-foreground font-medium">{lead.phone || '—'}</span>
            </div>
            <div class="flex justify-between py-1.5 border-b border-border/60">
              <span class="text-muted-foreground flex items-center gap-1.5"><MapPin class="w-3.5 h-3.5" /> Address</span>
              <span class="text-foreground font-medium truncate max-w-[200px]">{lead.address || '—'}</span>
            </div>
            <div class="flex justify-between py-1.5 border-b border-border/60">
              <span class="text-muted-foreground flex items-center gap-1.5"><Users class="w-3.5 h-3.5" /> Owner</span>
              <span class="text-foreground font-medium">{lead.owner || '—'}</span>
            </div>
            <div class="flex justify-between py-1.5 border-b border-border/60">
              <span class="text-muted-foreground flex items-center gap-1.5"><Building class="w-3.5 h-3.5" /> Est. Value</span>
              <span class="text-emerald-600 font-bold">${(lead.value || 0).toLocaleString()}</span>
            </div>
            {#if lead.about}
              <div class="pt-2">
                <span class="text-muted-foreground block mb-1">About</span>
                <p class="p-2.5 rounded bg-muted/30 border border-border/60 text-foreground leading-relaxed">{lead.about}</p>
              </div>
            {/if}
          </div>
        </div>

        <!-- Log Activity Form -->
        <div class="space-y-3 pt-2">
          <h5 class="text-xs font-bold text-muted-foreground uppercase tracking-wider">Log Activity</h5>
          <form onsubmit={handleLogActivity} class="space-y-2.5 text-xs">
            <select bind:value={activityType} class="w-full h-8 px-2.5 rounded-md border border-input bg-background text-foreground">
              <option value="Call">Call</option>
              <option value="Email">Email</option>
              <option value="Meeting">Meeting</option>
            </select>
            <textarea
              bind:value={activityContent}
              placeholder="What did you discuss? Enter notes here..."
              rows="3"
              required
              class="w-full p-2.5 rounded-md border border-input bg-background text-foreground placeholder:text-muted-foreground"
            ></textarea>
            <Button type="submit" size="sm" class="w-full h-8 text-xs" disabled={logging || !activityContent.trim()}>
              {logging ? 'Logging...' : 'Log Activity'}
            </Button>
          </form>
        </div>

        <!-- Activity Timeline -->
        <div class="space-y-3 pt-2">
          <h5 class="text-xs font-bold text-muted-foreground uppercase tracking-wider">Activity History</h5>
          {#if activities.length === 0}
            <p class="text-xs text-muted-foreground italic">No activities logged yet.</p>
          {:else}
            <div class="space-y-3 relative before:absolute before:left-3 before:top-2 before:bottom-2 before:w-0.5 before:bg-border">
              {#each activities as activity}
                <div class="flex items-start gap-3 relative">
                  <div class="w-6.5 h-6.5 rounded-full bg-background border border-border flex items-center justify-center text-primary z-10 text-[10px]">
                    <Clock class="w-3 h-3 text-muted-foreground" />
                  </div>
                  <div class="flex-1 p-2.5 rounded-lg border border-border/70 bg-muted/20 text-xs space-y-1">
                    <div class="flex items-center justify-between font-semibold">
                      <span class="text-foreground">{activity.activity_type}</span>
                      <span class="text-[10px] text-muted-foreground font-normal">{formatDateTime(activity.created_at)}</span>
                    </div>
                    <p class="text-muted-foreground leading-relaxed">{activity.content}</p>
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
