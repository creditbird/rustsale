<script lang="ts">
  import { onMount } from 'svelte';
  import {
    listManagedWhatsAppAccounts,
    listWhatsAppCampaigns,
    getWhatsAppCampaign,
    getWhatsAppCampaignRecipients,
    createWhatsAppCampaign,
    populateWhatsAppCampaignRecipients,
    startWhatsAppCampaign,
    pauseWhatsAppCampaign,
    resumeWhatsAppCampaign,
    cancelWhatsAppCampaign,
    renderSpintaxTemplate,
    normalizeWhatsAppPhone,
    type ConnectedWhatsAppAccount,
    type WhatsAppCampaignRecord,
    type CampaignRecipientRecord,
    type CampaignRecipientInput
  } from '$lib/services/whatsappService';
  import {
    getCategories,
    getLeads,
    type Category,
    type Lead
  } from '$lib/services/crmService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import * as Card from '$lib/components/ui/card';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Select from '$lib/components/ui/select';
  import { Progress } from '$lib/components/ui/progress';
  import {
    MessageCircleMore,
    Plus,
    RefreshCw,
    Play,
    Pause,
    RotateCcw,
    XCircle,
    Eye,
    Settings2,
    Search,
    Clock,
    ShieldAlert,
    Send,
    CheckCircle2,
    AlertTriangle,
    Sliders,
    Moon,
    Users,
    Shuffle,
    Sparkles
  } from 'lucide-svelte';
  import { m } from '$lib/i18n';

  let {
    activeTab = 'whatsapp_outreach',
    onOpenSettings = () => {}
  }: {
    activeTab?: string;
    onOpenSettings?: () => void;
  } = $props();

  // Primary data state
  let accounts = $state<ConnectedWhatsAppAccount[]>([]);
  let campaigns = $state<WhatsAppCampaignRecord[]>([]);
  let categories = $state<Category[]>([]);
  let crmLeads = $state<Lead[]>([]);

  let isLoading = $state(false);
  let statusFilter = $state('all');
  let searchQuery = $state('');

  // Create Campaign Modal State
  let isCreateModalOpen = $state(false);
  let isSubmittingCampaign = $state(false);
  let createError = $state<string | null>(null);

  let newCampaignName = $state('');
  let selectedAccountIds = $state<number[]>([]);
  let templateContent = $state('{Xin chào|Chào bạn|Hello} [name], chúng tôi gửi ưu đãi dành riêng cho [company]!');
  let templateTextareaRef = $state<HTMLTextAreaElement | null>(null);
  let previewSeed = $state(0);

  // Anti-Ban & Rate Limits
  let minDelaySecs = $state(8);
  let maxDelaySecs = $state(20);
  let burstLimit = $state(25);
  let burstPauseSecs = $state(60);
  let quietHoursEnabled = $state(true);
  let quietHoursStart = $state('22:00');
  let quietHoursEnd = $state('07:30');
  let timezone = $state('Asia/Ho_Chi_Minh');

  // Recipient targeting
  let recipientMode = $state<'crm' | 'manual'>('crm');
  let selectedCategoryId = $state<number | null>(null);
  let selectedLeadIds = $state<number[]>([]);
  let selectAllCategoryLeads = $state(true);
  let manualPhonesRaw = $state('');

  // Details Modal State
  let isDetailsModalOpen = $state(false);
  let selectedCampaign = $state<WhatsAppCampaignRecord | null>(null);
  let campaignRecipients = $state<CampaignRecipientRecord[]>([]);
  let isLoadingRecipients = $state(false);

  // Derived Metrics
  let totalCampaigns = $derived(campaigns.length);
  let runningCampaigns = $derived(
    campaigns.filter((c) => c.status === 'running' || c.status === 'queued').length
  );
  let totalMessagesSent = $derived(
    campaigns.reduce((sum, c) => sum + (c.sent_count || 0), 0)
  );
  let totalDelivered = $derived(
    campaigns.reduce((sum, c) => sum + (c.delivered_count || 0), 0)
  );
  let totalRead = $derived(
    campaigns.reduce((sum, c) => sum + (c.read_count || 0), 0)
  );
  let totalSkipped = $derived(
    campaigns.reduce((sum, c) => sum + (c.skipped_count || 0), 0)
  );
  let deliveryReadRate = $derived(
    totalMessagesSent > 0
      ? Math.min(100, Math.round(((totalDelivered + totalRead) / totalMessagesSent) * 100))
      : 0
  );

  // Filtered Campaigns
  let filteredCampaigns = $derived(
    campaigns.filter((c) => {
      const matchesStatus =
        statusFilter === 'all' || c.status.toLowerCase() === statusFilter.toLowerCase();
      const matchesSearch =
        !searchQuery.trim() ||
        c.name.toLowerCase().includes(searchQuery.trim().toLowerCase());
      return matchesStatus && matchesSearch;
    })
  );

  // Spintax Live Preview
  const sampleLeadVars = {
    name: 'Nguyễn Văn A',
    company: 'TechCorp Việt Nam',
    phone: '+84901234567',
    email: 'a.nguyen@example.com',
    notes: 'Khách hàng VIP'
  };

  let renderedPreview = $derived.by(() => {
    // previewSeed is a trigger dependency for re-randomizing
    void previewSeed;
    return renderSpintaxTemplate(templateContent, sampleLeadVars);
  });

  // Manual Recipients Parsed & Normalized Preview
  let parsedManualRecipients = $derived.by(() => {
    if (recipientMode !== 'manual' || !manualPhonesRaw.trim()) return [];
    const tokens = manualPhonesRaw
      .split(/[\n,;]+/)
      .map((t) => t.trim())
      .filter((t) => t.length > 0);

    const validPhones: Array<{ raw: string; e164: string }> = [];
    const seen = new Set<string>();

    for (const raw of tokens) {
      const e164 = normalizeWhatsAppPhone(raw);
      if (e164 && !seen.has(e164)) {
        seen.add(e164);
        validPhones.push({ raw, e164 });
      }
    }
    return validPhones;
  });

  // CRM Filtered Leads
  let availableCrmLeads = $derived.by(() => {
    return crmLeads.filter((lead) => {
      if (selectedCategoryId !== null && lead.category_id !== selectedCategoryId) {
        return false;
      }
      return Boolean(lead.phone && lead.phone.trim().length > 0);
    });
  });

  let effectiveRecipientCount = $derived.by(() => {
    if (recipientMode === 'manual') {
      return parsedManualRecipients.length;
    }
    if (selectAllCategoryLeads) {
      return availableCrmLeads.length;
    }
    return selectedLeadIds.length;
  });

  // Initial Data Loading
  async function loadData() {
    isLoading = true;
    try {
      const [accs, camps, cats, lds] = await Promise.all([
        listManagedWhatsAppAccounts().catch((e) => {
          console.warn('Failed to load WhatsApp accounts:', e);
          return [];
        }),
        listWhatsAppCampaigns().catch((e) => {
          console.warn('Failed to load WhatsApp campaigns:', e);
          return [];
        }),
        getCategories().catch((e) => {
          console.warn('Failed to load categories:', e);
          return [];
        }),
        getLeads().catch((e) => {
          console.warn('Failed to load CRM leads:', e);
          return [];
        })
      ]);
      accounts = accs;
      campaigns = camps;
      categories = cats;
      crmLeads = lds;

      if (accounts.length > 0 && selectedAccountIds.length === 0) {
        selectedAccountIds = [accounts[0].id];
      }
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    loadData();
  });

  // Campaign Actions
  async function handleStartCampaign(campaignId: number) {
    try {
      await startWhatsAppCampaign(campaignId);
      await refreshCampaigns();
    } catch (e) {
      console.error('Failed to start campaign:', e);
    }
  }

  async function handlePauseCampaign(campaignId: number) {
    try {
      await pauseWhatsAppCampaign(campaignId);
      await refreshCampaigns();
    } catch (e) {
      console.error('Failed to pause campaign:', e);
    }
  }

  async function handleResumeCampaign(campaignId: number) {
    try {
      await resumeWhatsAppCampaign(campaignId);
      await refreshCampaigns();
    } catch (e) {
      console.error('Failed to resume campaign:', e);
    }
  }

  async function handleCancelCampaign(campaignId: number) {
    try {
      await cancelWhatsAppCampaign(campaignId);
      await refreshCampaigns();
    } catch (e) {
      console.error('Failed to cancel campaign:', e);
    }
  }

  async function refreshCampaigns() {
    try {
      campaigns = await listWhatsAppCampaigns();
    } catch (e) {
      console.error('Failed to refresh campaigns:', e);
    }
  }

  // Details Modal
  async function openCampaignDetails(campaign: WhatsAppCampaignRecord) {
    selectedCampaign = campaign;
    isDetailsModalOpen = true;
    isLoadingRecipients = true;
    try {
      const [updatedCampaign, recs] = await Promise.all([
        getWhatsAppCampaign(campaign.id).catch(() => campaign),
        getWhatsAppCampaignRecipients(campaign.id, 100, 0).catch(() => [])
      ]);
      selectedCampaign = updatedCampaign;
      campaignRecipients = recs;
    } finally {
      isLoadingRecipients = false;
    }
  }

  // Create Campaign Modal Helpers
  function openCreateModal() {
    newCampaignName = '';
    createError = null;
    if (accounts.length > 0 && selectedAccountIds.length === 0) {
      selectedAccountIds = [accounts[0].id];
    }
    isCreateModalOpen = true;
  }

  function insertTag(tag: string) {
    const token = `[${tag}]`;
    if (!templateTextareaRef) {
      templateContent += token;
      return;
    }
    const start = templateTextareaRef.selectionStart ?? templateContent.length;
    const end = templateTextareaRef.selectionEnd ?? templateContent.length;
    const before = templateContent.substring(0, start);
    const after = templateContent.substring(end);
    templateContent = before + token + after;

    setTimeout(() => {
      if (templateTextareaRef) {
        templateTextareaRef.focus();
        const pos = start + token.length;
        templateTextareaRef.setSelectionRange(pos, pos);
      }
    }, 0);
  }

  function toggleAccountSelection(id: number) {
    if (selectedAccountIds.includes(id)) {
      selectedAccountIds = selectedAccountIds.filter((x) => x !== id);
    } else {
      selectedAccountIds = [...selectedAccountIds, id];
    }
  }

  function toggleLeadSelection(id: number) {
    if (selectedLeadIds.includes(id)) {
      selectedLeadIds = selectedLeadIds.filter((x) => x !== id);
    } else {
      selectedLeadIds = [...selectedLeadIds, id];
    }
  }

  function shufflePreview() {
    previewSeed += 1;
  }

  async function handleCreateCampaignSubmit() {
    createError = null;

    if (!newCampaignName.trim()) {
      createError = m.whatsapp_outreach_error_no_name();
      return;
    }
    if (selectedAccountIds.length === 0) {
      createError = m.whatsapp_outreach_error_no_accounts();
      return;
    }
    if (!templateContent.trim()) {
      createError = m.whatsapp_outreach_error_no_template();
      return;
    }
    if (effectiveRecipientCount === 0) {
      createError = m.whatsapp_outreach_error_no_recipients();
      return;
    }

    isSubmittingCampaign = true;
    try {
      // 1. Create Campaign
      const campaignId = await createWhatsAppCampaign({
        name: newCampaignName.trim(),
        assigned_account_ids: selectedAccountIds,
        rate_limit_min_delay_secs: minDelaySecs,
        rate_limit_max_delay_secs: maxDelaySecs,
        burst_limit_per_batch: burstLimit,
        burst_pause_secs: burstPauseSecs,
        quiet_hours_enabled: quietHoursEnabled,
        quiet_hours_start: quietHoursStart,
        quiet_hours_end: quietHoursEnd,
        timezone: timezone
      });

      // 2. Build Recipients List
      const recipients: CampaignRecipientInput[] = [];

      if (recipientMode === 'manual') {
        for (const item of parsedManualRecipients) {
          recipients.push({
            recipient_phone_raw: item.raw,
            template: templateContent,
            lead_vars: {
              phone: item.e164
            }
          });
        }
      } else {
        const targetLeads = selectAllCategoryLeads
          ? availableCrmLeads
          : availableCrmLeads.filter((l) => l.id != null && selectedLeadIds.includes(l.id));

        for (const lead of targetLeads) {
          if (!lead.phone) continue;
          recipients.push({
            lead_id: lead.id ?? null,
            recipient_phone_raw: lead.phone,
            template: templateContent,
            lead_vars: {
              name: lead.name || '',
              phone: lead.phone || '',
              company: lead.company || '',
              email: lead.email || '',
              notes: lead.about || ''
            }
          });
        }
      }

      // 3. Populate Recipients
      await populateWhatsAppCampaignRecipients(campaignId, recipients, templateContent);

      // 4. Refresh & Close
      isCreateModalOpen = false;
      await refreshCampaigns();
    } catch (e) {
      console.error('Error creating WhatsApp campaign:', e);
      createError = String(e);
    } finally {
      isSubmittingCampaign = false;
    }
  }

  function getStatusBadgeClass(status: string): string {
    switch (status.toLowerCase()) {
      case 'running':
        return 'bg-emerald-500/15 text-emerald-600 border-emerald-500/30 dark:text-emerald-400';
      case 'queued':
        return 'bg-purple-500/15 text-purple-600 border-purple-500/30 dark:text-purple-400';
      case 'paused':
        return 'bg-amber-500/15 text-amber-600 border-amber-500/30 dark:text-amber-400';
      case 'completed':
        return 'bg-blue-500/15 text-blue-600 border-blue-500/30 dark:text-blue-400';
      case 'failed':
        return 'bg-rose-500/15 text-rose-600 border-rose-500/30 dark:text-rose-400';
      case 'cancelled':
        return 'bg-slate-500/15 text-slate-600 border-slate-500/30 dark:text-slate-400';
      default:
        return 'bg-muted text-muted-foreground border-border';
    }
  }

  function getRecipientStatusBadgeClass(status: string): string {
    switch (status.toLowerCase()) {
      case 'delivered':
      case 'read':
        return 'bg-emerald-500/15 text-emerald-600 border-emerald-500/30 dark:text-emerald-400';
      case 'sent':
        return 'bg-sky-500/15 text-sky-600 border-sky-500/30 dark:text-sky-400';
      case 'running':
        return 'bg-indigo-500/15 text-indigo-600 border-indigo-500/30 dark:text-indigo-400';
      case 'skipped':
        return 'bg-amber-500/15 text-amber-600 border-amber-500/30 dark:text-amber-400';
      case 'failed':
      case 'blocked':
        return 'bg-rose-500/15 text-rose-600 border-rose-500/30 dark:text-rose-400';
      default:
        return 'bg-muted text-muted-foreground border-border';
    }
  }

  function getAccountDisplayName(account: ConnectedWhatsAppAccount): string {
    return account.display_name?.trim() || account.phone_number?.trim() || account.provider_account_key.slice(0, 10);
  }
</script>

<div class="flex flex-col gap-6">
  <!-- Top Header Bar -->
  <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
    <div class="flex flex-col">
      <h2 class="text-xl font-bold text-foreground tracking-tight flex items-center gap-2">
        <MessageCircleMore class="size-6 text-emerald-600 dark:text-emerald-400" />
        {m.whatsapp_outreach_title()}
      </h2>
      <p class="text-xs text-muted-foreground mt-0.5">
        {m.whatsapp_outreach_subtitle()}
      </p>
    </div>

    <div class="flex items-center gap-2">
      <Button
        variant="outline"
        size="sm"
        class="h-9 gap-1.5 text-xs"
        onclick={loadData}
        disabled={isLoading}
      >
        <RefreshCw class="size-3.5 {isLoading ? 'animate-spin' : ''}" />
        {m.whatsapp_outreach_refresh()}
      </Button>

      <Button
        variant="outline"
        size="sm"
        class="h-9 gap-1.5 text-xs text-muted-foreground hover:text-foreground"
        onclick={onOpenSettings}
      >
        <Settings2 class="size-3.5" />
        {m.whatsapp_outreach_manage_accounts()}
      </Button>

      <Button
        variant="default"
        size="sm"
        class="h-9 gap-1.5 text-xs bg-emerald-600 hover:bg-emerald-700 text-white shadow-xs font-semibold"
        onclick={openCreateModal}
      >
        <Plus class="size-4" />
        {m.whatsapp_outreach_new_campaign()}
      </Button>
    </div>
  </div>

  <!-- Metrics Bar -->
  <div class="grid grid-cols-2 lg:grid-cols-5 gap-3.5">
    <Card.Root class="border-border bg-card/60 backdrop-blur-xs">
      <Card.Content class="p-4 flex flex-col gap-1">
        <span class="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
          {m.whatsapp_outreach_metrics_total_campaigns()}
        </span>
        <div class="flex items-baseline justify-between">
          <span class="text-2xl font-bold text-foreground font-mono">{totalCampaigns}</span>
          <MessageCircleMore class="size-4 text-emerald-600/70" />
        </div>
      </Card.Content>
    </Card.Root>

    <Card.Root class="border-border bg-card/60 backdrop-blur-xs">
      <Card.Content class="p-4 flex flex-col gap-1">
        <span class="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
          {m.whatsapp_outreach_metrics_running_campaigns()}
        </span>
        <div class="flex items-baseline justify-between">
          <span class="text-2xl font-bold text-purple-600 font-mono">{runningCampaigns}</span>
          <Clock class="size-4 text-purple-600/70" />
        </div>
      </Card.Content>
    </Card.Root>

    <Card.Root class="border-border bg-card/60 backdrop-blur-xs">
      <Card.Content class="p-4 flex flex-col gap-1">
        <span class="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
          {m.whatsapp_outreach_metrics_total_sent()}
        </span>
        <div class="flex items-baseline justify-between">
          <span class="text-2xl font-bold text-foreground font-mono">{totalMessagesSent}</span>
          <Send class="size-4 text-sky-600/70" />
        </div>
      </Card.Content>
    </Card.Root>

    <Card.Root class="border-border bg-card/60 backdrop-blur-xs">
      <Card.Content class="p-4 flex flex-col gap-1">
        <span class="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
          {m.whatsapp_outreach_metrics_delivery_read_rate()}
        </span>
        <div class="flex items-baseline justify-between">
          <span class="text-2xl font-bold text-emerald-600 font-mono">{deliveryReadRate}%</span>
          <CheckCircle2 class="size-4 text-emerald-600/70" />
        </div>
        <span class="text-[10px] text-muted-foreground">
          {totalDelivered} delivered / {totalRead} read
        </span>
      </Card.Content>
    </Card.Root>

    <Card.Root class="border-border bg-card/60 backdrop-blur-xs col-span-2 lg:col-span-1">
      <Card.Content class="p-4 flex flex-col gap-1">
        <span class="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
          {m.whatsapp_outreach_metrics_skipped()}
        </span>
        <div class="flex items-baseline justify-between">
          <span class="text-2xl font-bold text-amber-600 font-mono">{totalSkipped}</span>
          <ShieldAlert class="size-4 text-amber-600/70" />
        </div>
      </Card.Content>
    </Card.Root>
  </div>

  <!-- Filter & Search Toolbar -->
  <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 border-b border-border pb-4">
    <div class="flex items-center gap-1.5 overflow-x-auto pb-1 sm:pb-0">
      {#each [
        { id: 'all', label: m.whatsapp_outreach_filter_all() },
        { id: 'running', label: m.whatsapp_outreach_filter_running() },
        { id: 'queued', label: m.whatsapp_outreach_filter_queued() },
        { id: 'paused', label: m.whatsapp_outreach_filter_paused() },
        { id: 'completed', label: m.whatsapp_outreach_filter_completed() },
        { id: 'draft', label: m.whatsapp_outreach_filter_draft() },
        { id: 'failed', label: m.whatsapp_outreach_filter_failed() },
        { id: 'cancelled', label: m.whatsapp_outreach_filter_cancelled() }
      ] as tab (tab.id)}
        <button
          type="button"
          class="px-3 py-1.5 rounded-full text-xs font-medium transition-colors cursor-pointer {statusFilter === tab.id
            ? 'bg-primary text-primary-foreground font-semibold shadow-2xs'
            : 'text-muted-foreground hover:bg-accent hover:text-foreground'}"
          onclick={() => (statusFilter = tab.id)}
        >
          {tab.label}
          {#if tab.id !== 'all'}
            <span class="ml-1 text-[10px] opacity-75">
              ({campaigns.filter((c) => c.status.toLowerCase() === tab.id).length})
            </span>
          {/if}
        </button>
      {/each}
    </div>

    <div class="relative w-full sm:w-64">
      <Search class="size-3.5 absolute left-3 top-2.5 text-muted-foreground" />
      <Input
        type="text"
        placeholder={m.whatsapp_outreach_search_placeholder()}
        class="h-8 pl-8 text-xs bg-background"
        bind:value={searchQuery}
      />
    </div>
  </div>

  <!-- Campaign Cards List -->
  {#if filteredCampaigns.length === 0}
    <div class="flex flex-col items-center justify-center py-16 px-4 text-center border border-dashed border-border rounded-xl bg-card/40">
      <div class="size-12 rounded-full bg-emerald-500/10 text-emerald-600 flex items-center justify-center mb-3">
        <MessageCircleMore class="size-6" />
      </div>
      <h3 class="text-sm font-semibold text-foreground">
        {m.whatsapp_outreach_no_campaigns()}
      </h3>
      <p class="text-xs text-muted-foreground mt-1 max-w-sm">
        Launch multi-account WhatsApp campaigns with anti-ban delay pacing and automatic suppression list filtering.
      </p>
      <Button
        variant="default"
        size="sm"
        class="mt-4 gap-1.5 text-xs bg-emerald-600 hover:bg-emerald-700 text-white"
        onclick={openCreateModal}
      >
        <Plus class="size-3.5" />
        {m.whatsapp_outreach_new_campaign()}
      </Button>
    </div>
  {:else}
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
      {#each filteredCampaigns as c (c.id)}
        {@const progressPercent = c.total_recipients > 0 ? Math.min(100, Math.round((c.sent_count / c.total_recipients) * 100)) : 0}
        <Card.Root class="border-border bg-card hover:border-emerald-500/40 transition-all flex flex-col justify-between shadow-2xs">
          <Card.Header class="p-4 pb-2 border-b border-border/60 flex flex-row items-start justify-between gap-2">
            <div class="flex flex-col gap-1 min-w-0">
              <span class="font-bold text-sm text-foreground truncate" title={c.name}>
                {c.name}
              </span>
              <span class="text-[10px] text-muted-foreground font-mono">
                ID: #{c.id} • {new Date(c.created_at).toLocaleDateString()}
              </span>
            </div>
            <Badge variant="outline" class="text-[10px] uppercase font-semibold shrink-0 {getStatusBadgeClass(c.status)}">
              {c.status}
            </Badge>
          </Card.Header>

          <Card.Content class="p-4 flex flex-col gap-3 text-xs">
            <!-- Progress Bar -->
            <div class="flex flex-col gap-1.5">
              <div class="flex justify-between text-[11px] font-medium text-foreground">
                <span>{m.whatsapp_outreach_progress()}: {c.sent_count} / {c.total_recipients}</span>
                <span class="font-mono">{progressPercent}%</span>
              </div>
              <Progress value={progressPercent} class="h-2" />
            </div>

            <!-- Stats breakdown pills -->
            <div class="grid grid-cols-4 gap-1 text-[10px] text-center pt-1">
              <div class="p-1 rounded bg-muted/60 flex flex-col">
                <span class="text-muted-foreground">Sent</span>
                <span class="font-bold text-foreground font-mono">{c.sent_count}</span>
              </div>
              <div class="p-1 rounded bg-emerald-500/10 text-emerald-600 flex flex-col">
                <span>Delivered</span>
                <span class="font-bold font-mono">{c.delivered_count}</span>
              </div>
              <div class="p-1 rounded bg-teal-500/10 text-teal-600 flex flex-col">
                <span>Read</span>
                <span class="font-bold font-mono">{c.read_count}</span>
              </div>
              <div class="p-1 rounded bg-rose-500/10 text-rose-600 flex flex-col">
                <span>Failed</span>
                <span class="font-bold font-mono">{c.failed_count}</span>
              </div>
            </div>

            <!-- Anti-Ban & Quiet Hours Indicators -->
            <div class="flex flex-col gap-1 text-[11px] text-muted-foreground pt-1 border-t border-border/40">
              <div class="flex items-center justify-between">
                <span class="flex items-center gap-1">
                  <Sliders class="size-3 text-emerald-600" />
                  {c.rate_limit_min_delay_secs}s-{c.rate_limit_max_delay_secs}s delay
                </span>
                <span class="font-mono text-[10px]">
                  Batch: {c.burst_limit_per_batch} / {c.burst_pause_secs}s
                </span>
              </div>

              {#if c.quiet_hours_enabled}
                <div class="flex items-center gap-1 text-indigo-500 text-[10px]">
                  <Moon class="size-3" />
                  Quiet Hours: {c.quiet_hours_start} - {c.quiet_hours_end} ({c.timezone})
                </div>
              {/if}
            </div>
          </Card.Content>

          <Card.Footer class="p-3 bg-muted/20 border-t border-border/60 flex items-center justify-between gap-1.5">
            <Button
              variant="ghost"
              size="sm"
              class="h-7 text-xs px-2 gap-1 text-muted-foreground hover:text-foreground"
              onclick={() => openCampaignDetails(c)}
            >
              <Eye class="size-3.5" />
              {m.whatsapp_outreach_btn_view_details()}
            </Button>

            <div class="flex items-center gap-1">
              {#if c.status.toLowerCase() === 'draft' || c.status.toLowerCase() === 'queued'}
                <Button
                  variant="outline"
                  size="sm"
                  class="h-7 text-xs px-2 gap-1 text-emerald-600 border-emerald-500/30 hover:bg-emerald-500/10"
                  onclick={() => handleStartCampaign(c.id)}
                >
                  <Play class="size-3 fill-emerald-600" />
                  {m.whatsapp_outreach_btn_start()}
                </Button>
              {:else if c.status.toLowerCase() === 'running'}
                <Button
                  variant="outline"
                  size="sm"
                  class="h-7 text-xs px-2 gap-1 text-amber-600 border-amber-500/30 hover:bg-amber-500/10"
                  onclick={() => handlePauseCampaign(c.id)}
                >
                  <Pause class="size-3" />
                  {m.whatsapp_outreach_btn_pause()}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  class="h-7 text-xs px-2 text-destructive hover:bg-destructive/10"
                  onclick={() => handleCancelCampaign(c.id)}
                >
                  <XCircle class="size-3.5" />
                </Button>
              {:else if c.status.toLowerCase() === 'paused'}
                <Button
                  variant="outline"
                  size="sm"
                  class="h-7 text-xs px-2 gap-1 text-emerald-600 border-emerald-500/30 hover:bg-emerald-500/10"
                  onclick={() => handleResumeCampaign(c.id)}
                >
                  <RotateCcw class="size-3" />
                  {m.whatsapp_outreach_btn_resume()}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  class="h-7 text-xs px-2 text-destructive hover:bg-destructive/10"
                  onclick={() => handleCancelCampaign(c.id)}
                >
                  <XCircle class="size-3.5" />
                </Button>
              {/if}
            </div>
          </Card.Footer>
        </Card.Root>
      {/each}
    </div>
  {/if}
</div>

<!-- CREATE CAMPAIGN DIALOG -->
<Dialog.Root bind:open={isCreateModalOpen}>
  <Dialog.Content class="max-w-3xl max-h-[90vh] overflow-y-auto p-6">
    <Dialog.Header class="pb-3 border-b border-border">
      <Dialog.Title class="text-base font-bold flex items-center gap-2">
        <MessageCircleMore class="size-5 text-emerald-600" />
        {m.whatsapp_outreach_create_title()}
      </Dialog.Title>
      <Dialog.Description class="text-xs text-muted-foreground">
        {m.whatsapp_outreach_create_subtitle()}
      </Dialog.Description>
    </Dialog.Header>

    {#if createError}
      <div class="p-3 rounded-lg border border-destructive/30 bg-destructive/10 text-destructive text-xs flex items-center gap-2">
        <AlertTriangle class="size-4 shrink-0" />
        <span>{createError}</span>
      </div>
    {/if}

    <div class="flex flex-col gap-4 py-2 text-xs">
      <!-- 1. Campaign Name -->
      <div class="flex flex-col gap-1">
        <label for="campaign-name-input" class="font-semibold text-foreground">
          {m.whatsapp_outreach_field_name()} <span class="text-destructive">*</span>
        </label>
        <Input
          id="campaign-name-input"
          type="text"
          placeholder={m.whatsapp_outreach_field_name_placeholder()}
          bind:value={newCampaignName}
          class="h-9 text-xs"
        />
      </div>

      <!-- 2. Account Selector -->
      <div class="flex flex-col gap-1.5">
        <div class="flex items-center justify-between">
          <span class="font-semibold text-foreground">
            {m.whatsapp_outreach_field_accounts()} <span class="text-destructive">*</span>
          </span>
          <span class="text-[10px] text-muted-foreground">
            {selectedAccountIds.length} account(s) selected
          </span>
        </div>

        {#if accounts.length === 0}
          <div class="p-3 rounded-lg border border-border bg-muted/30 text-muted-foreground flex items-center justify-between">
            <span>{m.whatsapp_outreach_field_no_accounts()}</span>
            <Button variant="link" size="sm" class="h-6 text-xs text-primary" onclick={onOpenSettings}>
              Open Settings
            </Button>
          </div>
        {:else}
          <div class="grid grid-cols-1 sm:grid-cols-2 gap-2 max-h-36 overflow-y-auto p-1 border border-border rounded-lg bg-card/50">
            {#each accounts as acc (acc.id)}
              {@const isSelected = selectedAccountIds.includes(acc.id)}
              {@const displayName = getAccountDisplayName(acc)}
              <div
                class="flex items-center gap-2.5 p-2 rounded-md border cursor-pointer transition-colors {isSelected
                  ? 'border-emerald-500 bg-emerald-500/10 font-semibold'
                  : 'border-border bg-card hover:bg-accent/40'}"
                onclick={() => toggleAccountSelection(acc.id)}
                role="button"
                tabindex="0"
                onkeydown={(e) => e.key === 'Enter' && toggleAccountSelection(acc.id)}
              >
                <Checkbox checked={isSelected} onCheckedChange={() => toggleAccountSelection(acc.id)} />
                <div class="size-6 rounded-full bg-emerald-500/15 text-emerald-600 font-bold text-[10px] flex items-center justify-center shrink-0">
                  {displayName[0].toUpperCase()}
                </div>
                <div class="flex flex-col min-w-0">
                  <span class="text-xs truncate text-foreground">{displayName}</span>
                  <span class="text-[10px] text-muted-foreground font-mono truncate">{acc.phone_number || acc.provider_account_key}</span>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <!-- 3. Message Template & Spintax -->
      <div class="flex flex-col gap-1.5">
        <div class="flex items-center justify-between">
          <label for="template-editor" class="font-semibold text-foreground">
            {m.whatsapp_outreach_field_template()} <span class="text-destructive">*</span>
          </label>
          <span class="text-[10px] text-muted-foreground">
            Supports Spintax syntax: <code>{'{Hello|Hi|Greetings}'}</code>
          </span>
        </div>

        <textarea
          id="template-editor"
          bind:this={templateTextareaRef}
          bind:value={templateContent}
          rows="3"
          placeholder={m.whatsapp_outreach_field_template_placeholder()}
          class="w-full p-2.5 rounded-md border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring font-sans"
        ></textarea>

        <!-- Variable buttons -->
        <div class="flex flex-wrap items-center gap-1.5 pt-1">
          <span class="text-[11px] font-medium text-muted-foreground">{m.whatsapp_outreach_insert_token()}:</span>
          {#each ['name', 'phone', 'company', 'email', 'notes'] as tag (tag)}
            <button
              type="button"
              class="h-6 px-2 text-[11px] rounded bg-muted hover:bg-accent text-foreground border border-border transition-colors cursor-pointer"
              onclick={() => insertTag(tag)}
            >
              + [{tag}]
            </button>
          {/each}
        </div>

        <!-- Live Spintax Preview Box -->
        <div class="p-3 rounded-lg border border-emerald-500/30 bg-emerald-500/5 flex flex-col gap-1.5 mt-1">
          <div class="flex items-center justify-between text-[11px] font-semibold text-emerald-700 dark:text-emerald-400">
            <span class="flex items-center gap-1.5">
              <Sparkles class="size-3.5" />
              {m.whatsapp_outreach_spintax_preview()}
            </span>
            <Button
              variant="ghost"
              size="sm"
              class="h-6 text-[10px] px-2 gap-1 text-emerald-700 dark:text-emerald-400 hover:bg-emerald-500/10"
              onclick={shufflePreview}
            >
              <Shuffle class="size-3" />
              {m.whatsapp_outreach_reroll_preview()}
            </Button>
          </div>
          <div class="p-2 rounded bg-background border border-emerald-500/20 text-xs text-foreground italic whitespace-pre-wrap">
            {renderedPreview}
          </div>
        </div>
      </div>

      <!-- 4. Anti-Ban Rate Limit & Quiet Hours -->
      <div class="flex flex-col gap-2 p-3 rounded-lg border border-border bg-card/40">
        <span class="font-semibold text-foreground flex items-center gap-1.5">
          <Sliders class="size-4 text-emerald-600" />
          {m.whatsapp_outreach_rate_limits_title()}
        </span>

        <div class="grid grid-cols-2 sm:grid-cols-4 gap-2.5 pt-1">
          <div class="flex flex-col gap-1">
            <label for="min-delay-input" class="text-[11px] text-muted-foreground">{m.whatsapp_outreach_min_delay()}</label>
            <Input id="min-delay-input" type="number" min="1" max="120" bind:value={minDelaySecs} class="h-8 text-xs font-mono" />
          </div>
          <div class="flex flex-col gap-1">
            <label for="max-delay-input" class="text-[11px] text-muted-foreground">{m.whatsapp_outreach_max_delay()}</label>
            <Input id="max-delay-input" type="number" min="1" max="300" bind:value={maxDelaySecs} class="h-8 text-xs font-mono" />
          </div>
          <div class="flex flex-col gap-1">
            <label for="burst-limit-input" class="text-[11px] text-muted-foreground">{m.whatsapp_outreach_burst_limit()}</label>
            <Input id="burst-limit-input" type="number" min="1" max="100" bind:value={burstLimit} class="h-8 text-xs font-mono" />
          </div>
          <div class="flex flex-col gap-1">
            <label for="burst-pause-input" class="text-[11px] text-muted-foreground">{m.whatsapp_outreach_burst_pause()}</label>
            <Input id="burst-pause-input" type="number" min="5" max="600" bind:value={burstPauseSecs} class="h-8 text-xs font-mono" />
          </div>
        </div>

        <!-- Quiet Hours Section -->
        <div class="flex flex-col gap-2 pt-2 border-t border-border/50">
          <label class="flex items-center gap-2 cursor-pointer text-xs font-medium text-foreground">
            <Checkbox checked={quietHoursEnabled} onCheckedChange={(v) => (quietHoursEnabled = Boolean(v))} />
            <span>{m.whatsapp_outreach_quiet_hours_toggle()}</span>
          </label>

          {#if quietHoursEnabled}
            <div class="grid grid-cols-1 sm:grid-cols-3 gap-2.5 pl-6 pt-1">
              <div class="flex flex-col gap-1">
                <label for="quiet-start-input" class="text-[11px] text-muted-foreground">{m.whatsapp_outreach_quiet_hours_start()}</label>
                <Input id="quiet-start-input" type="text" placeholder="22:00" bind:value={quietHoursStart} class="h-8 text-xs font-mono" />
              </div>
              <div class="flex flex-col gap-1">
                <label for="quiet-end-input" class="text-[11px] text-muted-foreground">{m.whatsapp_outreach_quiet_hours_end()}</label>
                <Input id="quiet-end-input" type="text" placeholder="07:30" bind:value={quietHoursEnd} class="h-8 text-xs font-mono" />
              </div>
              <div class="flex flex-col gap-1">
                <label for="quiet-tz-input" class="text-[11px] text-muted-foreground">{m.whatsapp_outreach_quiet_hours_tz()}</label>
                <Input id="quiet-tz-input" type="text" bind:value={timezone} class="h-8 text-xs font-mono" />
              </div>
            </div>
          {/if}
        </div>
      </div>

      <!-- 5. Target Recipients -->
      <div class="flex flex-col gap-2 p-3 rounded-lg border border-border bg-card/40">
        <div class="flex items-center justify-between">
          <span class="font-semibold text-foreground flex items-center gap-1.5">
            <Users class="size-4 text-emerald-600" />
            {m.whatsapp_outreach_recipients_title()}
          </span>
          <Badge variant="outline" class="text-xs font-mono font-bold text-emerald-600">
            {effectiveRecipientCount} {m.whatsapp_outreach_recipients_count()}
          </Badge>
        </div>

        <!-- Mode Toggle Tabs -->
        <div class="flex items-center gap-2 pt-1">
          <button
            type="button"
            class="px-3 py-1.5 rounded-md text-xs font-medium transition-colors cursor-pointer {recipientMode === 'crm'
              ? 'bg-primary text-primary-foreground font-semibold shadow-xs'
              : 'bg-muted text-muted-foreground hover:text-foreground'}"
            onclick={() => (recipientMode = 'crm')}
          >
            {m.whatsapp_outreach_recipient_mode_crm()}
          </button>
          <button
            type="button"
            class="px-3 py-1.5 rounded-md text-xs font-medium transition-colors cursor-pointer {recipientMode === 'manual'
              ? 'bg-primary text-primary-foreground font-semibold shadow-xs'
              : 'bg-muted text-muted-foreground hover:text-foreground'}"
            onclick={() => (recipientMode = 'manual')}
          >
            {m.whatsapp_outreach_recipient_mode_manual()}
          </button>
        </div>

        {#if recipientMode === 'crm'}
          <div class="flex flex-col gap-2 pt-2">
            <!-- Category Filter -->
            <div class="flex items-center gap-2">
              <label for="crm-category-select" class="text-xs font-medium text-muted-foreground shrink-0">{m.whatsapp_outreach_select_category()}:</label>
              <Select.Root
                type="single"
                value={selectedCategoryId != null ? String(selectedCategoryId) : ""}
                onValueChange={(v) => (selectedCategoryId = v ? Number(v) : null)}
              >
                <Select.Trigger id="crm-category-select" class="w-full h-8 text-xs">
                  {categories.find((c) => String(c.id) === String(selectedCategoryId))?.name || m.whatsapp_outreach_all_categories()}
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="">{m.whatsapp_outreach_all_categories()}</Select.Item>
                  {#each categories as cat (cat.id)}
                    <Select.Item value={String(cat.id)}>{cat.name}</Select.Item>
                  {/each}
                </Select.Content>
              </Select.Root>
            </div>

            <!-- Category Selection Info -->
            <div class="p-2 rounded bg-muted/40 border border-border flex items-center justify-between text-xs">
              <span class="text-muted-foreground">
                Found {availableCrmLeads.length} valid phone numbers in CRM matching this segment.
              </span>
              <label class="flex items-center gap-1.5 cursor-pointer font-medium text-foreground">
                <Checkbox checked={selectAllCategoryLeads} onCheckedChange={(v) => (selectAllCategoryLeads = Boolean(v))} />
                <span>Target all matching leads</span>
              </label>
            </div>
          </div>
        {:else}
          <div class="flex flex-col gap-2 pt-2">
            <textarea
              bind:value={manualPhonesRaw}
              rows="3"
              placeholder={m.whatsapp_outreach_manual_placeholder()}
              class="w-full p-2.5 rounded-md border border-input bg-background text-xs font-mono text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            ></textarea>

            {#if parsedManualRecipients.length > 0}
              <div class="flex flex-col gap-1 p-2 rounded bg-emerald-500/5 border border-emerald-500/20 text-xs">
                <span class="font-semibold text-emerald-700 dark:text-emerald-400">
                  {m.whatsapp_outreach_normalized_preview()} ({parsedManualRecipients.length}):
                </span>
                <div class="flex flex-wrap gap-1 max-h-20 overflow-y-auto">
                  {#each parsedManualRecipients.slice(0, 10) as item (item.e164)}
                    <Badge variant="outline" class="text-[10px] font-mono border-emerald-500/30">
                      {item.e164}
                    </Badge>
                  {/each}
                  {#if parsedManualRecipients.length > 10}
                    <span class="text-[10px] text-muted-foreground self-center">
                      +{parsedManualRecipients.length - 10} more...
                    </span>
                  {/if}
                </div>
              </div>
            {/if}
          </div>
        {/if}
      </div>
    </div>

    <Dialog.Footer class="pt-4 border-t border-border flex items-center justify-end gap-2">
      <Button
        variant="ghost"
        size="sm"
        class="h-9 text-xs"
        onclick={() => (isCreateModalOpen = false)}
        disabled={isSubmittingCampaign}
      >
        {m.whatsapp_outreach_btn_cancel_dialog()}
      </Button>
      <Button
        variant="default"
        size="sm"
        class="h-9 text-xs bg-emerald-600 hover:bg-emerald-700 text-white font-semibold gap-1.5"
        onclick={handleCreateCampaignSubmit}
        disabled={isSubmittingCampaign}
      >
        {#if isSubmittingCampaign}
          <RefreshCw class="size-3.5 animate-spin" />
        {:else}
          <Plus class="size-3.5" />
        {/if}
        {m.whatsapp_outreach_btn_create_submit()}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- CAMPAIGN DETAILS / RECIPIENT LEDGER DIALOG -->
<Dialog.Root bind:open={isDetailsModalOpen}>
  <Dialog.Content class="max-w-4xl max-h-[90vh] overflow-y-auto p-6">
    <Dialog.Header class="pb-3 border-b border-border flex flex-row items-center justify-between">
      <div class="flex flex-col gap-0.5">
        <Dialog.Title class="text-base font-bold flex items-center gap-2">
          <MessageCircleMore class="size-5 text-emerald-600" />
          {m.whatsapp_outreach_details_title()}
        </Dialog.Title>
        {#if selectedCampaign}
          <Dialog.Description class="text-xs text-muted-foreground">
            Campaign: <strong class="text-foreground">{selectedCampaign.name}</strong> • Status:
            <Badge variant="outline" class="ml-1 text-[9px] uppercase font-semibold {getStatusBadgeClass(selectedCampaign.status)}">
              {selectedCampaign.status}
            </Badge>
          </Dialog.Description>
        {/if}
      </div>

      {#if selectedCampaign}
        <Button
          variant="outline"
          size="sm"
          class="h-7 text-xs px-2 gap-1"
          onclick={() => selectedCampaign && openCampaignDetails(selectedCampaign)}
          disabled={isLoadingRecipients}
        >
          <RefreshCw class="size-3 {isLoadingRecipients ? 'animate-spin' : ''}" />
          {m.whatsapp_outreach_refresh()}
        </Button>
      {/if}
    </Dialog.Header>

    <div class="flex flex-col gap-4 py-3 text-xs">
      {#if isLoadingRecipients}
        <div class="py-12 flex flex-col items-center justify-center gap-2 text-muted-foreground">
          <RefreshCw class="size-5 animate-spin text-emerald-600" />
          <span>Loading recipient ledger...</span>
        </div>
      {:else if campaignRecipients.length === 0}
        <div class="py-12 text-center text-muted-foreground italic">
          {m.whatsapp_outreach_no_recipients()}
        </div>
      {:else}
        <div class="border border-border rounded-lg overflow-hidden max-h-96 overflow-y-auto">
          <table class="w-full text-left text-xs border-collapse">
            <thead class="bg-muted/50 text-[11px] text-muted-foreground font-semibold uppercase tracking-wider sticky top-0 border-b border-border">
              <tr>
                <th class="p-2.5">{m.whatsapp_outreach_recipient_col_phone()}</th>
                <th class="p-2.5">{m.whatsapp_outreach_recipient_col_status()}</th>
                <th class="p-2.5">{m.whatsapp_outreach_recipient_col_skip()}</th>
                <th class="p-2.5">{m.whatsapp_outreach_recipient_col_message()}</th>
                <th class="p-2.5 text-center">{m.whatsapp_outreach_recipient_col_attempts()}</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border/60">
              {#each campaignRecipients as r (r.id)}
                <tr class="hover:bg-accent/30 transition-colors">
                  <td class="p-2.5 font-mono font-medium text-foreground">
                    {r.recipient_phone_e164 || r.recipient_phone_raw}
                  </td>
                  <td class="p-2.5">
                    <Badge variant="outline" class="text-[10px] uppercase font-semibold {getRecipientStatusBadgeClass(r.status)}">
                      {r.status}
                    </Badge>
                  </td>
                  <td class="p-2.5 text-muted-foreground text-[11px]">
                    {r.skip_reason || r.last_error || '—'}
                  </td>
                  <td class="p-2.5 text-muted-foreground max-w-xs truncate" title={r.rendered_text || ''}>
                    {r.rendered_text || '—'}
                  </td>
                  <td class="p-2.5 text-center font-mono font-semibold text-foreground">
                    {r.attempt_count}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>

    <Dialog.Footer class="pt-3 border-t border-border flex justify-end">
      <Button
        variant="outline"
        size="sm"
        class="h-8 text-xs"
        onclick={() => (isDetailsModalOpen = false)}
      >
        {m.whatsapp_outreach_btn_cancel_dialog()}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
