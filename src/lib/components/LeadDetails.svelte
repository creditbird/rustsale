<script lang="ts">
  import { untrack } from 'svelte';
  import { fade, fly } from 'svelte/transition';
  import { toast } from 'svelte-sonner';
  import * as crmService from '$lib/services/crmService';
  import type { Lead, Activity, Deal } from '$lib/services/crmService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import * as Dialog from '$lib/components/ui/dialog';
  import { invoke } from '@tauri-apps/api/core';
  import {
    listManagedZaloAccounts,
    listManagedSmtpAccounts,
    type ManagedZaloAccount,
    type ManagedSmtpAccount
  } from '$lib/services/accountSettingsService';
  import {
    X, Edit2, Trash2, Save, Phone, Mail, Users, CheckCircle2, XCircle,
    AlertCircle, Calendar, Clock, MapPin, Building, Briefcase, Plus,
    DollarSign, Layers, Send, MessageCircle, ExternalLink, RefreshCw, MessageSquare
  } from 'lucide-svelte';
  import * as m from '$lib/paraglide/messages';
  import {
    formatCurrency,
    formatCompactCurrency,
    convertCurrency,
    formatDate,
    formatDateTime,
    currencyState,
    type Currency
  } from '$lib/i18n';

  interface ZaloTemplate {
    id: number;
    name: string;
    content: string;
    image_url?: string | null;
  }

  interface EmailTemplate {
    id: number;
    name: string;
    subject: string;
    content: string;
  }

  let { leadId, initialLead = null, onClose, onRefresh, onOpenConversation } = $props<{
    leadId: number;
    initialLead?: Lead | null;
    onClose: () => void;
    onRefresh?: () => void;
    onOpenConversation?: (lead: { id: number; phone?: string; name?: string; zalo_uid?: string }) => void;
  }>();

  let lead = $state<Lead | null>(untrack(() => initialLead));
  let activities = $state<Activity[]>([]);
  let deals = $state<Deal[]>([]);
  let loading = $state(false);
  let errorMsg = $state<string | null>(null);

  // Edit mode states
  let isEditing = $state(false);
  let editName = $state('');
  let editCompany = $state('');
  let editStatus = $state('');
  let editValue = $state(0);
  let editCurrency = $state<Currency>('VND');
  let editOwner = $state('');
  let editEmail = $state('');
  let editPhone = $state('');
  let editAbout = $state('');
  let editAddress = $state('');
  let savingLead = $state(false);
  let deletingLead = $state(false);

  // Create Deal Modal State for this lead
  let showCreateDealModal = $state(false);
  let newDealTitle = $state('');
  let newDealCurrency = $state<Currency>('VND');
  let newDealValue = $state(10000000);
  let newDealStage = $state('Prospecting');
  let creatingDeal = $state(false);
  let createDealErrorMsg = $state<string | null>(null);

  // Quick Send Zalo Modal State
  let showSendZaloModal = $state(false);
  let zaloAccounts = $state<ManagedZaloAccount[]>([]);
  let selectedZaloAccountKey = $state<string>('');
  let zaloTemplates = $state<ZaloTemplate[]>([]);
  let selectedZaloTemplateId = $state<number | null>(null);
  let sendingZalo = $state(false);
  let sendZaloErrorMsg = $state<string | null>(null);

  // Quick Send Email Modal State
  let showSendEmailModal = $state(false);
  let smtpAccounts = $state<ManagedSmtpAccount[]>([]);
  let selectedSmtpId = $state<number | null>(null);
  let emailTemplates = $state<EmailTemplate[]>([]);
  let selectedEmailTemplateId = $state<number | null>(null);
  let sendingEmail = $state(false);
  let sendEmailErrorMsg = $state<string | null>(null);

  const vndPresets = [5000000, 10000000, 25000000, 50000000, 100000000];
  const usdPresets = [200, 500, 1000, 2500, 5000];

  let dealPresets = $derived(newDealCurrency === 'USD' ? usdPresets : vndPresets);

  function handleDealCurrencyChange(c: Currency) {
    if (newDealCurrency === c) return;
    newDealCurrency = c;
    newDealValue = c === 'USD' ? 500 : 10000000;
  }

  const stages = ['Prospecting', 'Demo', 'Proposal', 'Negotiation', 'Won'] as const;
  const stageBadges: Record<string, { label: () => string; bg: string; text: string }> = {
    'Prospecting': { label: () => `${m.pipeline_stage_prospecting()} (20%)`, bg: 'bg-blue-500/10 border-blue-500/30', text: 'text-blue-600' },
    'Demo': { label: () => `${m.pipeline_stage_demo()} (40%)`, bg: 'bg-purple-500/10 border-purple-500/30', text: 'text-purple-600' },
    'Proposal': { label: () => `${m.pipeline_stage_proposal()} (60%)`, bg: 'bg-amber-500/10 border-amber-500/30', text: 'text-amber-600' },
    'Negotiation': { label: () => `${m.pipeline_stage_negotiation()} (80%)`, bg: 'bg-indigo-500/10 border-indigo-500/30', text: 'text-indigo-600' },
    'Won': { label: () => `${m.pipeline_stage_won()} (100%)`, bg: 'bg-emerald-500/10 border-emerald-500/30', text: 'text-emerald-600' }
  };

  function getLeadStatusLabel(status: string): string {
    switch (status) {
      case 'New': return m.leads_status_new();
      case 'Contacted': return m.leads_status_contacted();
      case 'Qualified': return m.leads_status_qualified();
      case 'Proposal': return m.leads_status_proposal();
      case 'Won': return m.leads_status_won();
      case 'Lost': return m.leads_status_lost();
      case 'Converted': return m.leads_status_converted();
      default: return status;
    }
  }

  function getActivityTypeLabel(type: string): string {
    switch (type) {
      case 'Call': return m.lead_details_activity_call();
      case 'Email': return m.lead_details_activity_email();
      case 'Meeting': return m.lead_details_activity_meeting();
      default: return type;
    }
  }

  function getLeadInitials(name?: string | null): string {
    return (
      (name || '')
        .trim()
        .split(/\s+/)
        .filter(Boolean)
        .map((n: string) => n[0])
        .join('')
        .toUpperCase()
        .substring(0, 2) || '?'
    );
  }

  function hasValidEmail(email?: string | null): boolean {
    return Boolean(email && email !== 'No Email' && email.includes('@'));
  }

  function startEditing() {
    if (!lead) return;
    editName = lead.name;
    editCompany = lead.company;
    editStatus = lead.status;
    editValue = lead.value;
    editCurrency = (lead.currency as Currency) || 'VND';
    editOwner = lead.owner || '';
    editEmail = lead.email === 'No Email' ? '' : (lead.email || '');
    editPhone = lead.phone || '';
    editAbout = lead.about || '';
    editAddress = lead.address || '';
    isEditing = true;
  }

  function handleCurrencySwitch(targetCurrency: Currency) {
    if (editCurrency === targetCurrency) return;
    if (editValue && editValue > 0) {
      if (editCurrency === 'VND' && targetCurrency === 'USD') {
        const converted = convertCurrency(editValue, 'VND', 'USD', currencyState.exchangeRate);
        editValue = Math.round(converted * 100) / 100;
      } else if (editCurrency === 'USD' && targetCurrency === 'VND') {
        const converted = convertCurrency(editValue, 'USD', 'VND', currencyState.exchangeRate);
        editValue = Math.round(converted);
      }
    }
    editCurrency = targetCurrency;
  }

  function openCreateDealModal() {
    createDealErrorMsg = null;
    newDealTitle = `Deal - ${lead?.company || lead?.name || ''}`;
    newDealCurrency = (lead?.currency as Currency) || currencyState.baseCurrency;
    newDealValue = newDealCurrency === 'USD' ? 500 : 10000000;
    newDealStage = 'Prospecting';
    showCreateDealModal = true;
  }

  async function openSendZaloModal() {
    sendZaloErrorMsg = null;
    showSendZaloModal = true;
    try {
      const [accs, tpls] = await Promise.all([
        listManagedZaloAccounts().catch(() => []),
        invoke<ZaloTemplate[]>('get_outreach_templates').catch(() => [])
      ]);
      zaloAccounts = accs || [];
      zaloTemplates = tpls || [];
      const validAcc = zaloAccounts.find((a) => a.authStatus === 'valid') || zaloAccounts[0];
      if (validAcc) {
        selectedZaloAccountKey = validAcc.accountKey;
      }
      if (zaloTemplates.length > 0) {
        selectedZaloTemplateId = zaloTemplates[0].id;
      }
    } catch (err) {
      sendZaloErrorMsg = err instanceof Error ? err.message : String(err);
    }
  }

  async function handleSendZalo(e: Event) {
    e.preventDefault();
    if (!selectedZaloAccountKey) {
      sendZaloErrorMsg = m.lead_details_no_zalo_acc();
      return;
    }
    if (!selectedZaloTemplateId) {
      sendZaloErrorMsg = m.lead_details_no_zalo_tpl();
      return;
    }
    sendingZalo = true;
    sendZaloErrorMsg = null;
    try {
      await invoke('send_zalo_outreach', {
        imei: selectedZaloAccountKey,
        leadIds: [leadId],
        templateId: selectedZaloTemplateId
      });
      const tplName = zaloTemplates.find((t) => t.id === selectedZaloTemplateId)?.name || m.lead_details_fallback_zalo_tpl();
      await crmService.addActivity(
        leadId,
        'Zalo',
        m.lead_details_log_zalo_prefix({ name: lead?.name || m.lead_details_fallback_customer(), template: tplName }),
        'Self'
      );
      showSendZaloModal = false;
      toast.success(m.lead_details_toast_zalo_sent(), {
        description: m.lead_details_toast_zalo_sent_desc({ name: lead?.name || m.lead_details_fallback_customer() })
      });
      await loadLeadDetails(leadId);
      if (onRefresh) onRefresh();
    } catch (err) {
      sendZaloErrorMsg = err instanceof Error ? err.message : String(err);
      toast.error(m.lead_details_toast_zalo_fail(), { description: sendZaloErrorMsg });
    } finally {
      sendingZalo = false;
    }
  }

  async function openSendEmailModal() {
    if (!hasValidEmail(lead?.email)) {
      toast.error(m.lead_details_toast_no_email(), {
        description: m.lead_details_toast_no_email_desc()
      });
      return;
    }
    sendEmailErrorMsg = null;
    showSendEmailModal = true;
    try {
      const [accs, tpls] = await Promise.all([
        listManagedSmtpAccounts().catch(() => []),
        invoke<EmailTemplate[]>('get_email_templates').catch(() => [])
      ]);
      smtpAccounts = accs || [];
      emailTemplates = tpls || [];
      if (smtpAccounts.length > 0) {
        selectedSmtpId = smtpAccounts[0].id;
      }
      if (emailTemplates.length > 0) {
        selectedEmailTemplateId = emailTemplates[0].id;
      }
    } catch (err) {
      sendEmailErrorMsg = err instanceof Error ? err.message : String(err);
    }
  }

  async function handleSendEmail(e: Event) {
    e.preventDefault();
    if (!selectedSmtpId) {
      sendEmailErrorMsg = m.lead_details_no_smtp();
      return;
    }
    if (!selectedEmailTemplateId) {
      sendEmailErrorMsg = m.lead_details_no_email_tpl();
      return;
    }
    sendingEmail = true;
    sendEmailErrorMsg = null;
    try {
      await invoke('send_email_outreach', {
        smtpId: selectedSmtpId,
        templateId: selectedEmailTemplateId,
        leadIds: [leadId]
      });
      const tpl = emailTemplates.find((t) => t.id === selectedEmailTemplateId);
      await crmService.addActivity(
        leadId,
        'Email',
        m.lead_details_log_email_prefix({ email: lead?.email || '', subject: tpl?.subject || tpl?.name || m.lead_details_fallback_outreach_email() }),
        'Self'
      );
      showSendEmailModal = false;
      toast.success(m.lead_details_toast_email_sent(), {
        description: m.lead_details_toast_email_sent_desc({ email: lead?.email || '' })
      });
      await loadLeadDetails(leadId);
      if (onRefresh) onRefresh();
    } catch (err) {
      sendEmailErrorMsg = err instanceof Error ? err.message : String(err);
      toast.error(m.lead_details_toast_email_fail(), { description: sendEmailErrorMsg });
    } finally {
      sendingEmail = false;
    }
  }

  function getZaloMeUrl(phone?: string | null): string | null {
    if (!phone) return null;
    const clean = phone.replace(/[^0-9]/g, '');
    if (!clean) return null;
    const formatted = clean.startsWith('0') ? '84' + clean.slice(1) : clean;
    return `https://zalo.me/${formatted}`;
  }

  function openZaloConversation() {
    showSendZaloModal = false;
    const zaloUid = lead?.zalo_uid || lead?.about?.match(/Zalo UID:\s*(\d+)/i)?.[1] || undefined;
    const payload = {
      id: leadId,
      phone: lead?.phone || editPhone || undefined,
      name: lead?.name || editName || undefined,
      zalo_uid: zaloUid
    };
    if (onOpenConversation) {
      onOpenConversation(payload);
    }
    if (onClose) {
      onClose();
    }
  }

  async function handleCreateDeal(e: Event) {
    e.preventDefault();
    if (!newDealTitle.trim()) {
      createDealErrorMsg = m.lead_details_err_enter_deal_title();
      return;
    }

    creatingDeal = true;
    createDealErrorMsg = null;
    try {
      const createdTitle = newDealTitle.trim();
      await crmService.createDeal(
        leadId,
        createdTitle,
        Number(newDealValue) || 0,
        newDealStage,
        newDealCurrency
      );
      showCreateDealModal = false;
      toast.success(m.lead_details_toast_deal_created(), {
        description: m.lead_details_toast_deal_created_desc({ title: createdTitle, name: lead?.name || m.lead_details_fallback_customer() })
      });
      await loadLeadDetails(leadId);
      if (onRefresh) onRefresh();
    } catch (err) {
      createDealErrorMsg = err instanceof Error ? err.message : String(err);
      toast.error(m.lead_details_toast_deal_create_fail(), {
        description: createDealErrorMsg
      });
    } finally {
      creatingDeal = false;
    }
  }

  // Delete Deal Modal State
  let showDeleteDealModal = $state(false);
  let dealToDelete = $state<{ id: number; title: string } | null>(null);
  let deletingDeal = $state(false);

  function promptDeleteDeal(id: number, title: string) {
    dealToDelete = { id, title };
    showDeleteDealModal = true;
  }

  async function confirmDeleteDeal() {
    if (!dealToDelete) return;
    deletingDeal = true;
    const deletedTitle = dealToDelete.title;
    try {
      await crmService.deleteDeal(dealToDelete.id);
      showDeleteDealModal = false;
      dealToDelete = null;
      toast.success(m.lead_details_toast_deal_deleted(), {
        description: m.lead_details_toast_deal_deleted_desc({ title: deletedTitle })
      });
      await loadLeadDetails(leadId);
      if (onRefresh) onRefresh();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
      toast.error(m.lead_details_toast_deal_delete_fail(), {
        description: errorMsg
      });
    } finally {
      deletingDeal = false;
    }
  }

  async function handleSaveLead(e: Event) {
    e.preventDefault();
    if (!lead || !editName.trim() || !editCompany.trim()) return;

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
        editEmail.trim() || 'No Email',
        editPhone.trim(),
        editAbout.trim() || null,
        editAddress.trim() || null,
        editCurrency
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
      const [leadData, activitiesData, allDealsData] = await Promise.all([
        crmService.getLeadDetails(id),
        crmService.getActivities(id),
        crmService.getDeals()
      ]);
      if (id !== leadId) return;
      lead = leadData;
      activities = activitiesData.sort(
        (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );
      deals = allDealsData.filter(d => d.lead_id === id);
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
      <h3 class="text-sm font-bold text-foreground tracking-wide">{m.lead_details_title()}</h3>
      {#if lead}
        <Badge variant="outline" class="text-[10px] uppercase font-mono">{getLeadStatusLabel(lead.status)}</Badge>
      {/if}
    </div>
    <Button variant="ghost" size="sm" onclick={onClose} class="h-8 w-8 p-0 text-muted-foreground hover:text-foreground">
      <X class="w-4 h-4" />
    </Button>
  </header>

  {#if loading && !lead}
    <div class="flex-1 flex flex-col items-center justify-center p-6 text-muted-foreground text-xs gap-2">
      <div class="w-5 h-5 rounded-full border-2 border-primary border-t-transparent animate-spin"></div>
      <p>{m.lead_details_loading()}</p>
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
      <div class="p-4 rounded-xl border border-border bg-card shadow-xs relative space-y-4">
        <div class="flex items-start gap-4">
          <div class="w-14 h-14 rounded-full bg-emerald-500/10 text-emerald-600 font-bold text-base flex items-center justify-center border border-emerald-500/20 shrink-0 shadow-2xs">
            {getLeadInitials(lead.name)}
          </div>
          <div class="flex-1 min-w-0 pr-12">
            <h4 class="text-lg font-bold text-foreground truncate">{lead.name}</h4>
            <p class="text-xs text-muted-foreground font-medium">{lead.company}</p>
            <div class="flex flex-wrap gap-2 mt-2.5">
              <Badge variant="default" class="text-[10px] py-0.5 px-2.5">{getLeadStatusLabel(lead.status)}</Badge>
              {#if lead.has_zalo === 1}
                <Badge variant="outline" class="gap-1 border-sky-500/30 text-sky-600 bg-sky-500/10 text-[10px] py-0.5 px-2 font-semibold">
                  <CheckCircle2 class="w-3 h-3 text-sky-500" /> {m.lead_details_zalo_verified()}
                </Badge>
              {:else if lead.has_zalo === 0}
                <Badge variant="outline" class="gap-1 border-slate-300 text-slate-500 bg-slate-100 text-[10px] py-0.5 px-2">
                  <XCircle class="w-3 h-3 text-slate-400" /> {m.lead_details_no_zalo()}
                </Badge>
              {/if}
            </div>
          </div>
          {#if !isEditing}
            <Button variant="outline" size="sm" onclick={startEditing} class="h-8 gap-1.5 absolute top-4 right-4 text-xs shadow-2xs">
              <Edit2 class="w-3.5 h-3.5" />
              {m.common_edit()}
            </Button>
          {/if}
        </div>

        <!-- CRM Action Bar: Quick Deal Creation & Outreach Actions -->
        {#if !isEditing}
          <div class="pt-3 border-t border-border/60 flex items-center gap-2 flex-wrap">
            <Button
              size="sm"
              onclick={openCreateDealModal}
              class="gap-1.5 font-bold text-xs bg-emerald-600 hover:bg-emerald-700 text-white shadow-sm h-8 rounded-lg"
            >
              <Briefcase class="w-3.5 h-3.5" />
              <span>{m.lead_details_create_deal_btn()}</span>
            </Button>

            <Button
              size="sm"
              onclick={openSendZaloModal}
              class="gap-1.5 font-bold text-xs border border-sky-500/30 bg-sky-500/10 text-sky-600 hover:bg-sky-500/20 shadow-2xs h-8 rounded-lg"
            >
              <MessageCircle class="w-3.5 h-3.5" />
              <span>{m.lead_details_send_zalo_action()}</span>
            </Button>

            <Button
              size="sm"
              onclick={openSendEmailModal}
              class="gap-1.5 font-bold text-xs border border-border bg-muted/40 hover:bg-muted text-foreground shadow-2xs h-8 rounded-lg"
            >
              <Mail class="w-3.5 h-3.5" />
              <span>{m.lead_details_send_email_action()}</span>
            </Button>
          </div>
        {/if}
      </div>

      <!-- Deals List Section -->
      <div class="space-y-3">
        <div class="flex items-center justify-between">
          <h5 class="text-xs font-bold text-muted-foreground uppercase tracking-wider flex items-center gap-1.5">
            <Briefcase class="w-3.5 h-3.5 text-emerald-600" />
            {m.lead_details_deals_heading({ count: deals.length })}
          </h5>
        </div>

        {#if deals.length === 0}
          <div class="p-4 rounded-xl border border-dashed border-border bg-card/50 text-center py-5">
            <p class="text-xs text-muted-foreground">{m.lead_details_no_deals()}</p>
          </div>
        {:else}
          <div class="space-y-2.5">
            {#each deals as deal (deal.id)}
              {@const badge = stageBadges[deal.stage] || { label: () => deal.stage, bg: 'bg-muted', text: 'text-foreground' }}
              <div class="p-3.5 rounded-xl border border-border bg-card shadow-2xs hover:border-emerald-500/40 transition-colors flex items-center justify-between gap-3">
                <div class="min-w-0 space-y-1">
                  <div class="flex items-center gap-2">
                    <h6 class="text-xs font-bold text-foreground truncate">{deal.title}</h6>
                    <Badge variant="outline" class="text-[10px] font-semibold px-2 py-0.5 {badge.bg} {badge.text}">
                      {badge.label()}
                    </Badge>
                  </div>
                  <div class="flex items-center gap-3 text-[11px]">
                    <span class="font-extrabold text-emerald-600 text-xs">{formatCurrency(deal.value, (deal.currency as Currency) || 'VND')}</span>
                    {#if deal.created_at}
                      <span class="text-muted-foreground text-[10px] flex items-center gap-1 font-medium">
                        <Calendar class="w-3 h-3 text-muted-foreground/60" />
                        {formatDate(deal.created_at, { month: 'numeric', day: 'numeric' })}
                      </span>
                    {/if}
                  </div>
                </div>

                <Button
                  variant="ghost"
                  size="sm"
                  onclick={() => promptDeleteDeal(deal.id!, deal.title)}
                  class="h-7 w-7 p-0 text-muted-foreground hover:text-destructive hover:bg-destructive/10 rounded-lg shrink-0"
                  title={m.lead_details_delete_deal_tooltip()}
                >
                  <Trash2 class="w-3.5 h-3.5" />
                </Button>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      {#if isEditing}
        <!-- Edit Lead Form (2-column responsive layout) -->
        <form onsubmit={handleSaveLead} class="space-y-4 text-xs border border-border bg-card p-5 rounded-xl shadow-xs">
          <div class="flex items-center justify-between border-b border-border pb-3">
            <h5 class="text-xs font-bold text-foreground uppercase tracking-wider">{m.lead_details_edit_title()}</h5>
            <span class="text-[10px] text-muted-foreground">{m.lead_details_required_fields()}</span>
          </div>

          <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div class="space-y-1">
              <label for="edit-name" class="font-medium text-foreground">{m.leads_modal_name_label()} *</label>
              <Input id="edit-name" bind:value={editName} required class="h-9 text-xs" />
            </div>
            <div class="space-y-1">
              <label for="edit-company" class="font-medium text-foreground">{m.leads_modal_company_label()} *</label>
              <Input id="edit-company" bind:value={editCompany} required class="h-9 text-xs" />
            </div>

            <div class="space-y-1">
              <label for="edit-email" class="font-medium text-foreground">{m.leads_modal_email_label()}</label>
              <Input id="edit-email" type="email" bind:value={editEmail} class="h-9 text-xs" />
            </div>
            <div class="space-y-1">
              <label for="edit-phone" class="font-medium text-foreground">{m.leads_modal_phone_label()}</label>
              <Input id="edit-phone" bind:value={editPhone} class="h-9 text-xs" />
            </div>

            <div class="space-y-1">
              <label for="edit-status" class="font-medium text-foreground">{m.common_status()}</label>
              <select
                id="edit-status"
                bind:value={editStatus}
                class="w-full h-9 text-xs border border-input bg-background rounded-md px-3 py-1 text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring cursor-pointer"
              >
                <option value="New">{m.leads_status_new()}</option>
                <option value="Contacted">{m.leads_status_contacted()}</option>
                <option value="Qualified">{m.leads_status_qualified()}</option>
                <option value="Lost">{m.leads_status_lost()}</option>
                <option value="Converted">{m.leads_status_converted()}</option>
              </select>
            </div>
            <div class="space-y-1">
              <label for="edit-value" class="font-medium text-foreground">{m.lead_details_estimated_value()}</label>
              <div class="flex items-center gap-1.5">
                <Input id="edit-value" type="number" bind:value={editValue} min="0" step={editCurrency === 'USD' ? '100' : '500000'} class="h-9 text-xs font-semibold text-emerald-600 flex-1" />
                <div class="flex bg-muted p-0.5 rounded-md border text-[11px] font-medium shrink-0">
                  <button
                    type="button"
                    class="px-2 py-1 rounded transition-colors {editCurrency === 'VND' ? 'bg-background shadow-xs text-foreground font-semibold' : 'text-muted-foreground hover:text-foreground'}"
                    onclick={() => handleCurrencySwitch('VND')}
                  >
                    VND (₫)
                  </button>
                  <button
                    type="button"
                    class="px-2 py-1 rounded transition-colors {editCurrency === 'USD' ? 'bg-background shadow-xs text-foreground font-semibold' : 'text-muted-foreground hover:text-foreground'}"
                    onclick={() => handleCurrencySwitch('USD')}
                  >
                    USD ($)
                  </button>
                </div>
              </div>
            </div>

            <div class="space-y-1 sm:col-span-2">
              <label for="edit-owner" class="font-medium text-foreground">{m.leads_modal_owner_label()}</label>
              <Input id="edit-owner" bind:value={editOwner} class="h-9 text-xs" />
            </div>

            <div class="space-y-1 sm:col-span-2">
              <label for="edit-address" class="font-medium text-foreground">{m.lead_details_address()}</label>
              <Input id="edit-address" bind:value={editAddress} class="h-9 text-xs" placeholder={m.lead_details_address_placeholder()} />
            </div>

            <div class="space-y-1 sm:col-span-2">
              <label for="edit-about" class="font-medium text-foreground">{m.lead_details_description()}</label>
              <textarea id="edit-about" bind:value={editAbout} rows="3" class="w-full p-3 rounded-md border border-input bg-background text-xs leading-relaxed focus:outline-hidden focus:ring-1 focus:ring-ring resize-y"></textarea>
            </div>
          </div>

          <div class="flex items-center justify-between pt-4 border-t border-border">
            <Button type="button" variant="destructive" size="sm" onclick={promptDeleteLead} disabled={deletingLead} class="gap-1.5 h-9 px-4">
              <Trash2 class="w-3.5 h-3.5" />
              {deletingLead ? m.lead_details_deleting() : m.common_delete()}
            </Button>
            <div class="flex items-center gap-2">
              <Button type="button" variant="outline" size="sm" onclick={() => isEditing = false} class="h-9 px-4">{m.common_cancel()}</Button>
              <Button type="submit" size="sm" disabled={savingLead} class="gap-1.5 h-9 px-5">
                <Save class="w-3.5 h-3.5" />
                {savingLead ? m.lead_details_saving() : m.common_save()}
              </Button>
            </div>
          </div>
        </form>
      {:else}
        <!-- Contact Details Overview -->
        <div class="space-y-3">
          <h5 class="text-xs font-bold text-muted-foreground uppercase tracking-wider">{m.lead_details_contact_overview()}</h5>
          <div class="rounded-xl border border-border bg-card p-4 space-y-3 text-xs shadow-xs">
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 pb-3 border-b border-border/60">
              <div>
                <span class="text-muted-foreground flex items-center gap-1.5 mb-1 font-medium"><Mail class="w-3.5 h-3.5 text-primary" /> {m.leads_modal_email_label()}</span>
                {#if hasValidEmail(lead.email)}
                  <a href="mailto:{lead.email}" class="text-primary font-semibold hover:underline break-all">{lead.email}</a>
                {:else}
                  <span class="text-foreground font-semibold">{lead.email && lead.email !== 'No Email' ? lead.email : '—'}</span>
                {/if}
              </div>
              <div>
                <span class="text-muted-foreground flex items-center gap-1.5 mb-1 font-medium"><Phone class="w-3.5 h-3.5 text-primary" /> {m.leads_modal_phone_label()}</span>
                <span class="text-foreground font-semibold">{lead.phone || '—'}</span>
              </div>
            </div>

            <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 pb-3 border-b border-border/60">
              <div>
                <span class="text-muted-foreground flex items-center gap-1.5 mb-1 font-medium"><Users class="w-3.5 h-3.5 text-primary" /> {m.leads_modal_owner_label()}</span>
                <span class="text-foreground font-semibold">{lead.owner || '—'}</span>
              </div>
              <div>
                <span class="text-muted-foreground flex items-center gap-1.5 mb-1 font-medium"><Building class="w-3.5 h-3.5 text-primary" /> {m.lead_details_estimated_value()}</span>
                <span class="text-emerald-600 font-bold text-sm">{formatCurrency(lead.value || 0, (lead.currency as Currency) || 'VND')}</span>
              </div>
            </div>

            <div class="pb-1">
              <span class="text-muted-foreground flex items-center gap-1.5 mb-1 font-medium"><MapPin class="w-3.5 h-3.5 text-primary" /> {m.lead_details_address()}</span>
              <span class="text-foreground font-medium leading-relaxed break-words block bg-muted/30 p-2.5 rounded-lg border border-border/50">{lead.address || '—'}</span>
            </div>

            {#if lead.about}
              <div class="pt-2">
                <span class="text-muted-foreground block mb-1 font-medium">{m.lead_details_description()}</span>
                <p class="p-3 rounded-lg bg-muted/30 border border-border/60 text-foreground leading-relaxed break-words">{lead.about}</p>
              </div>
            {/if}
          </div>
        </div>

        <!-- Log Activity Form -->
        <div class="space-y-3 pt-2">
          <h5 class="text-xs font-bold text-muted-foreground uppercase tracking-wider">{m.lead_details_log_activity()}</h5>
          <form onsubmit={handleLogActivity} class="space-y-3 text-xs rounded-xl border border-border bg-card p-4 shadow-xs">
            <select
              bind:value={activityType}
              class="w-full h-9 text-xs border border-input bg-background rounded-md px-3 py-1 text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring cursor-pointer"
            >
              <option value="Call">{m.lead_details_activity_call()}</option>
              <option value="Email">{m.lead_details_activity_email()}</option>
              <option value="Meeting">{m.lead_details_activity_meeting()}</option>
            </select>
            <textarea
              bind:value={activityContent}
              placeholder={m.lead_details_activity_placeholder()}
              rows="3"
              required
              class="w-full p-3 rounded-md border border-input bg-background text-foreground placeholder:text-muted-foreground text-xs leading-relaxed focus:outline-hidden focus:ring-1 focus:ring-ring resize-y"
            ></textarea>
            <Button type="submit" size="sm" class="w-full h-9 text-xs font-medium" disabled={logging || !activityContent.trim()}>
              {logging ? m.lead_details_activity_logging() : m.lead_details_activity_submit()}
            </Button>
          </form>
        </div>

        <!-- Activity Timeline -->
        <div class="space-y-3 pt-2">
          <h5 class="text-xs font-bold text-muted-foreground uppercase tracking-wider">{m.lead_details_activity_history()}</h5>
          {#if activities.length === 0}
            <p class="text-xs text-muted-foreground italic p-4 rounded-xl border border-border bg-card text-center">{m.lead_details_no_activities()}</p>
          {:else}
            <div class="space-y-3 relative before:absolute before:left-3 before:top-2 before:bottom-2 before:w-0.5 before:bg-border">
              {#each activities as activity (activity.id)}
                <div class="flex items-start gap-3 relative">
                  <div class="w-6.5 h-6.5 rounded-full bg-background border border-border flex items-center justify-center text-primary z-10 text-[10px] shrink-0">
                    <Clock class="w-3 h-3 text-muted-foreground" />
                  </div>
                  <div class="flex-1 p-3 rounded-xl border border-border/70 bg-card text-xs space-y-1.5 shadow-2xs">
                    <div class="flex items-center justify-between font-semibold">
                      <span class="text-foreground">{getActivityTypeLabel(activity.activity_type)}</span>
                      <span class="text-[10px] text-muted-foreground font-normal">{formatDateTime(activity.created_at)}</span>
                    </div>
                    <p class="text-muted-foreground leading-relaxed break-words">{activity.content}</p>
                    <span class="text-[10px] text-muted-foreground/70 block">{m.lead_details_logged_by({ performer: activity.performed_by })}</span>
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

<!-- Create Deal Modal for this Specific Lead -->
{#if showCreateDealModal && lead}
  <div class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-lg overflow-hidden animate-in fade-in zoom-in-95 duration-150">

      <!-- Modal Header -->
      <div class="px-6 py-4 border-b border-border flex items-center justify-between bg-muted/40">
        <div class="flex items-center gap-2.5">
          <div class="w-8 h-8 rounded-xl bg-emerald-500/10 text-emerald-600 flex items-center justify-center shadow-2xs">
            <Briefcase class="w-4 h-4" />
          </div>
          <div>
            <h3 class="font-bold text-base text-foreground">{m.lead_details_create_deal_modal_title()}</h3>
            <p class="text-xs text-muted-foreground">{lead.name} — {lead.company}</p>
          </div>
        </div>
        <button
          type="button"
          onclick={() => showCreateDealModal = false}
          class="text-muted-foreground hover:text-foreground p-1.5 rounded-lg hover:bg-accent transition-colors"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <form onsubmit={handleCreateDeal} class="p-6 space-y-4">
        {#if createDealErrorMsg}
          <div class="p-3 rounded-xl bg-destructive/10 border border-destructive/20 text-destructive text-xs flex items-center gap-2">
            <AlertCircle class="w-4 h-4 shrink-0" />
            <span>{createDealErrorMsg}</span>
          </div>
        {/if}

        <div class="space-y-1.5">
          <label for="lead-deal-title" class="text-xs font-bold text-foreground">{m.lead_details_deal_title()}</label>
          <Input
            id="lead-deal-title"
            bind:value={newDealTitle}
            placeholder="e.g. Deal - {lead.company}"
            class="h-10 text-xs rounded-xl"
            required
          />
        </div>

        <div class="space-y-3">
          <div class="flex items-center justify-between">
            <label for="lead-deal-value" class="text-xs font-bold text-foreground flex items-center gap-1.5">
              <DollarSign class="w-3.5 h-3.5 text-emerald-600" />
              {m.lead_details_deal_value()} ({newDealCurrency === 'USD' ? '$' : '₫'}) <span class="text-destructive">*</span>
            </label>
            <!-- Currency selector buttons -->
            <div class="flex items-center p-0.5 rounded-lg bg-muted border border-border">
              <button
                type="button"
                class="px-2.5 py-1 rounded-md text-[11px] font-bold transition-all cursor-pointer {newDealCurrency === 'VND'
                  ? 'bg-emerald-600 text-white shadow-xs'
                  : 'text-muted-foreground hover:text-foreground'}"
                onclick={() => handleDealCurrencyChange('VND')}
              >
                VND (₫)
              </button>
              <button
                type="button"
                class="px-2.5 py-1 rounded-md text-[11px] font-bold transition-all cursor-pointer {newDealCurrency === 'USD'
                  ? 'bg-emerald-600 text-white shadow-xs'
                  : 'text-muted-foreground hover:text-foreground'}"
                onclick={() => handleDealCurrencyChange('USD')}
              >
                USD ($)
              </button>
            </div>
          </div>

          <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div class="space-y-1.5">
              <Input
                id="lead-deal-value"
                type="number"
                bind:value={newDealValue}
                placeholder={newDealCurrency === 'USD' ? '500' : '10000000'}
                class="h-10 text-xs rounded-xl font-bold text-emerald-600"
                min="0"
                step={newDealCurrency === 'USD' ? '50' : '500000'}
                required
              />

              <!-- Quick Value Presets -->
              <div class="flex items-center gap-1 pt-1 flex-wrap">
                {#each dealPresets as preset (preset)}
                  <button
                    type="button"
                    onclick={() => newDealValue = preset}
                    class="text-[10px] font-semibold px-2 py-0.5 rounded-lg border border-border hover:border-emerald-500 hover:bg-emerald-500/10 hover:text-emerald-600 transition-colors cursor-pointer"
                  >
                    {formatCompactCurrency(preset, newDealCurrency)}
                  </button>
                {/each}
              </div>
            </div>

            <div class="space-y-1.5">
              <label for="lead-deal-stage" class="text-xs font-bold text-foreground">{m.lead_details_initial_stage()}</label>
              <select
                id="lead-deal-stage"
                bind:value={newDealStage}
                class="w-full h-10 px-3 rounded-xl border border-input bg-background text-xs font-semibold text-foreground focus:outline-none focus:ring-2 focus:ring-primary/40"
              >
                {#each stages as stage (stage)}
                  <option value={stage}>{stageBadges[stage]?.label() || stage}</option>
                {/each}
              </select>
            </div>
          </div>
        </div>

        <div class="pt-4 border-t border-border flex items-center justify-end gap-3">
          <Button
            type="button"
            variant="outline"
            size="default"
            onclick={() => showCreateDealModal = false}
            disabled={creatingDeal}
            class="rounded-xl h-10 px-5 text-xs font-semibold"
          >
            {m.common_cancel()}
          </Button>
          <Button
            type="submit"
            size="default"
            disabled={creatingDeal}
            class="rounded-xl h-10 px-6 text-xs font-bold bg-emerald-600 text-white hover:bg-emerald-700 shadow-md"
          >
            {creatingDeal ? m.common_loading() : m.kanban_create_btn()}
          </Button>
        </div>
      </form>
    </div>
  </div>
{/if}

<!-- Delete Lead Confirmation Dialog -->
<Dialog.Root bind:open={isDeleteLeadModalOpen}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{m.lead_details_delete_lead_title()}</Dialog.Title>
    </Dialog.Header>
    <div class="py-2 text-xs text-muted-foreground leading-relaxed">
      {m.lead_details_delete_lead_desc({ name: lead?.name || '' })}
    </div>
    <Dialog.Footer class="gap-2">
      <Button variant="outline" size="sm" onclick={() => isDeleteLeadModalOpen = false} disabled={deletingLead}>{m.common_cancel()}</Button>
      <Button variant="destructive" size="sm" onclick={confirmDeleteLead} disabled={deletingLead}>
        {deletingLead ? m.lead_details_deleting() : m.lead_details_delete_lead_btn()}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Custom Styled Delete Deal Confirmation Modal -->
{#if showDeleteDealModal && dealToDelete}
  <div class="fixed inset-0 z-[60] bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-md overflow-hidden animate-in fade-in zoom-in-95 duration-150">
      <div class="p-6 text-center space-y-4">
        <div class="w-12 h-12 rounded-full bg-destructive/10 text-destructive flex items-center justify-center mx-auto shadow-2xs">
          <AlertCircle class="w-6 h-6" />
        </div>

        <div class="space-y-1.5">
          <h3 class="font-bold text-base text-foreground">{m.lead_details_delete_deal_title()}</h3>
          <p class="text-xs text-muted-foreground leading-relaxed">
            {m.lead_details_delete_deal_desc({ title: dealToDelete.title })}
          </p>
        </div>

        <div class="pt-2 flex items-center justify-center gap-3">
          <Button
            type="button"
            variant="outline"
            size="default"
            onclick={() => { showDeleteDealModal = false; dealToDelete = null; }}
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
            {deletingDeal ? m.lead_details_deleting() : m.lead_details_delete_deal_btn()}
          </Button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Quick Send Zalo Dialog -->
<Dialog.Root bind:open={showSendZaloModal}>
  <Dialog.Content class="sm:max-w-lg">
    <Dialog.Header>
      <Dialog.Title class="flex items-center gap-2 text-sky-600">
        <MessageCircle class="w-5 h-5" />
        {m.lead_details_send_zalo_title({ name: lead?.name || '' })}
      </Dialog.Title>
    </Dialog.Header>

    <form onsubmit={handleSendZalo} class="space-y-4 py-2">
      {#if sendZaloErrorMsg}
        <div class="p-3 rounded-lg bg-destructive/10 border border-destructive/20 text-destructive text-xs flex items-center gap-2">
          <AlertCircle class="w-4 h-4 shrink-0" />
          <span>{sendZaloErrorMsg}</span>
        </div>
      {/if}

      <!-- Select Zalo Account -->
      <div class="space-y-1.5">
        <label for="lead-zalo-account-select" class="text-xs font-semibold text-foreground">{m.lead_details_sender_zalo_acc()}</label>
        {#if zaloAccounts.length === 0}
          <p class="text-xs text-amber-600 bg-amber-500/10 p-2.5 rounded-lg border border-amber-500/20">
            {m.lead_details_no_zalo_acc()}
          </p>
        {:else}
          <select
            id="lead-zalo-account-select"
            bind:value={selectedZaloAccountKey}
            class="w-full h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground focus:outline-hidden focus:ring-1 focus:ring-sky-500"
          >
            {#each zaloAccounts as acc (acc.accountKey)}
              <option value={acc.accountKey}>
                {acc.displayName || acc.accountKey} ({acc.authStatus === 'valid' ? m.common_active() : acc.authStatus})
              </option>
            {/each}
          </select>
        {/if}
      </div>

      <!-- Select Zalo Template -->
      <div class="space-y-1.5">
        <label for="lead-zalo-template-select" class="text-xs font-semibold text-foreground">{m.lead_details_zalo_tpl()}</label>
        {#if zaloTemplates.length === 0}
          <p class="text-xs text-muted-foreground bg-muted/40 p-2.5 rounded-lg border border-border">
            {m.lead_details_no_zalo_tpl()}
          </p>
        {:else}
          <select
            id="lead-zalo-template-select"
            bind:value={selectedZaloTemplateId}
            class="w-full h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground focus:outline-hidden focus:ring-1 focus:ring-sky-500"
          >
            {#each zaloTemplates as tpl (tpl.id)}
              <option value={tpl.id}>{tpl.name}</option>
            {/each}
          </select>

          {#if selectedZaloTemplateId}
            {@const selectedTpl = zaloTemplates.find(t => t.id === selectedZaloTemplateId)}
            {#if selectedTpl}
              <div class="p-3 rounded-lg bg-muted/30 border border-border text-xs space-y-1 text-muted-foreground">
                <span class="font-semibold text-foreground">{m.lead_details_preview_message()}</span>
                <p class="whitespace-pre-wrap">{selectedTpl.content}</p>
              </div>
            {/if}
          {/if}
        {/if}
      </div>

      <!-- Direct Conversation Chat Navigation -->
      <div class="pt-2 border-t border-border flex items-center justify-between">
        <span class="text-xs text-muted-foreground">{m.lead_details_or_direct_chat()}</span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onclick={openZaloConversation}
          class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-sky-500/10 text-sky-600 hover:bg-sky-500/20 text-xs font-medium border border-sky-500/30 transition-colors"
        >
          <MessageSquare class="w-3.5 h-3.5" />
          <span>{m.lead_details_open_chat()}</span>
        </Button>
      </div>

      <Dialog.Footer class="gap-2 pt-2">
        <Button variant="outline" size="sm" type="button" onclick={() => showSendZaloModal = false} disabled={sendingZalo}>
          {m.common_cancel()}
        </Button>
        <Button
          type="submit"
          size="sm"
          disabled={sendingZalo || !selectedZaloAccountKey || !selectedZaloTemplateId}
          class="bg-sky-600 hover:bg-sky-700 text-white font-bold gap-1.5"
        >
          {#if sendingZalo}
            <RefreshCw class="w-3.5 h-3.5 animate-spin" />
            <span>{m.common_loading()}</span>
          {:else}
            <Send class="w-3.5 h-3.5" />
            <span>{m.lead_details_send_zalo_btn()}</span>
          {/if}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>

<!-- Quick Send Email Dialog -->
<Dialog.Root bind:open={showSendEmailModal}>
  <Dialog.Content class="sm:max-w-lg">
    <Dialog.Header>
      <Dialog.Title class="flex items-center gap-2 text-foreground">
        <Mail class="w-5 h-5 text-emerald-600" />
        {m.lead_details_send_email_title({ name: lead?.name || '' })}
      </Dialog.Title>
    </Dialog.Header>

    <form onsubmit={handleSendEmail} class="space-y-4 py-2">
      {#if sendEmailErrorMsg}
        <div class="p-3 rounded-lg bg-destructive/10 border border-destructive/20 text-destructive text-xs flex items-center gap-2">
          <AlertCircle class="w-4 h-4 shrink-0" />
          <span>{sendEmailErrorMsg}</span>
        </div>
      {/if}

      <div class="space-y-1.5">
        <label for="lead-recipient-email-input" class="text-xs font-semibold text-foreground">{m.lead_details_recipient_email()}</label>
        <Input id="lead-recipient-email-input" type="email" value={lead?.email || ''} readonly class="bg-muted/40 text-xs text-muted-foreground" />
      </div>

      <!-- Select SMTP Account -->
      <div class="space-y-1.5">
        <label for="lead-smtp-account-select" class="text-xs font-semibold text-foreground">{m.lead_details_sender_smtp()}</label>
        {#if smtpAccounts.length === 0}
          <p class="text-xs text-amber-600 bg-amber-500/10 p-2.5 rounded-lg border border-amber-500/20">
            {m.lead_details_no_smtp()}
          </p>
        {:else}
          <select
            id="lead-smtp-account-select"
            bind:value={selectedSmtpId}
            class="w-full h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground focus:outline-hidden focus:ring-1 focus:ring-emerald-500"
          >
            {#each smtpAccounts as smtp (smtp.id)}
              <option value={smtp.id}>
                {smtp.name} ({smtp.fromEmail})
              </option>
            {/each}
          </select>
        {/if}
      </div>

      <!-- Select Email Template -->
      <div class="space-y-1.5">
        <label for="lead-email-template-select" class="text-xs font-semibold text-foreground">{m.lead_details_email_tpl()}</label>
        {#if emailTemplates.length === 0}
          <p class="text-xs text-muted-foreground bg-muted/40 p-2.5 rounded-lg border border-border">
            {m.lead_details_no_email_tpl()}
          </p>
        {:else}
          <select
            id="lead-email-template-select"
            bind:value={selectedEmailTemplateId}
            class="w-full h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground focus:outline-hidden focus:ring-1 focus:ring-emerald-500"
          >
            {#each emailTemplates as tpl (tpl.id)}
              <option value={tpl.id}>{tpl.name} - {tpl.subject}</option>
            {/each}
          </select>

          {#if selectedEmailTemplateId}
            {@const selectedTpl = emailTemplates.find(t => t.id === selectedEmailTemplateId)}
            {#if selectedTpl}
              <div class="p-3 rounded-lg bg-muted/30 border border-border text-xs space-y-1 text-muted-foreground">
                <span class="font-semibold text-foreground">{m.lead_details_subject({ subject: selectedTpl.subject })}</span>
                <p class="whitespace-pre-wrap truncate max-h-24 overflow-y-auto">{selectedTpl.content}</p>
              </div>
            {/if}
          {/if}
        {/if}
      </div>

      <!-- Native mailto Fallback -->
      {#if lead && hasValidEmail(lead.email)}
        <div class="pt-2 border-t border-border flex items-center justify-between">
          <span class="text-xs text-muted-foreground">{m.lead_details_or_open_client()}</span>
          <a
            href="mailto:{lead.email}"
            class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-muted/60 text-foreground hover:bg-muted text-xs font-medium border border-border transition-colors"
          >
            <span>{m.lead_details_open_mail_client()}</span>
            <ExternalLink class="w-3 h-3" />
          </a>
        </div>
      {/if}

      <Dialog.Footer class="gap-2 pt-2">
        <Button variant="outline" size="sm" type="button" onclick={() => showSendEmailModal = false} disabled={sendingEmail}>
          {m.common_cancel()}
        </Button>
        <Button
          type="submit"
          size="sm"
          disabled={sendingEmail || !selectedSmtpId || !selectedEmailTemplateId}
          class="bg-emerald-600 hover:bg-emerald-700 text-white font-bold gap-1.5"
        >
          {#if sendingEmail}
            <RefreshCw class="w-3.5 h-3.5 animate-spin" />
            <span>{m.common_loading()}</span>
          {:else}
            <Send class="w-3.5 h-3.5" />
            <span>{m.lead_details_send_email_btn()}</span>
          {/if}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
