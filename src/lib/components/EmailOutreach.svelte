<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import * as crmService from '$lib/services/crmService';
  import {
    getCategories,
    getOutreachLeadsPage,
    resolveOutreachLeadIds,
    type Category,
    type Lead,
    type OutreachLeadFilters,
    type OutreachLeadsPage
  } from '$lib/services/crmService';
  import {
    listManagedSmtpAccounts,
    type ManagedSmtpAccount
  } from '$lib/services/accountSettingsService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import * as Card from '$lib/components/ui/card';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Select from '$lib/components/ui/select';
  import { Progress } from '$lib/components/ui/progress';
  import { Mail, Plus, Server, Edit, Trash2, Send, FileText, Settings2, RefreshCw } from 'lucide-svelte';

  interface EmailTemplate {
    id?: number;
    name: string;
    subject: string;
    content: string;
    created_at?: string;
  }

  const LEAD_PAGE_SIZE = 50;

  let smtpServers = $state<ManagedSmtpAccount[]>([]);
  let templates = $state<EmailTemplate[]>([]);
  let leads = $state<Lead[]>([]);
  let totalLeads = $state(0);
  let currentLeadPage = $state(1);
  let pageCursors = $state<(number | null)[]>([null]);
  let nextLeadCursor = $state<number | null>(null);
  let isLoadingLeads = $state(false);
  let categories = $state<Category[]>([]);

  let selectedSmtpId = $state<number | null>(null);
  let selectedSmtpIds = $state<number[]>([]);
  let selectedTemplateId = $state<number | null>(null);
  let selectedLeadIds = $state<number[]>([]);
  let selectAllMatching = $state(true);
  let excludedLeadIds = $state<number[]>([]);

  let selectedCategoryId = $state<number | null>(null);
  let segmentationFilter = $state('all');
  let filterTemplateId = $state<number | null>(null);
  let leadSearchQuery = $state('');

  let isTemplateModalOpen = $state(false);
  let editingTemplateId = $state<number | null>(null);
  let templateName = $state('');
  let templateSubject = $state('');
  let templateContent = $state('');
  let templateError = $state<string | null>(null);
  let activeInputField = $state<'subject' | 'content'>('content');

  let isDeleteTemplateModalOpen = $state(false);
  let templateToDeleteId = $state<number | null>(null);

  let isCampaignRunning = $state(false);
  let campaignProgress = $state(0);
  let campaignStatus = $state('Idle');
  let campaignLogs = $state<string[]>([]);
  let campaignConsoleElement: HTMLDivElement | null = null;

  let campaignUnlistens: UnlistenFn[] = [];
  let leadPageCache = new Map<string, OutreachLeadsPage>();
  let leadRequestId = 0;
  let loadedLeadFilterKey = '';
  let isDisposed = false;
  let lastTab: string | null = null;

  let selectedLeadSet = $derived(new Set(selectedLeadIds));
  let excludedLeadSet = $derived(new Set(excludedLeadIds));
  let selectedLeadCount = $derived(
    selectAllMatching
      ? Math.max(0, totalLeads - excludedLeadIds.length)
      : selectedLeadIds.length
  );

  let isAllLeadsSelected = $derived(
    totalLeads > 0 && selectAllMatching && excludedLeadIds.length === 0
  );
  let totalLeadPages = $derived(Math.max(1, Math.ceil(totalLeads / LEAD_PAGE_SIZE)));
  let hasPreviousLeadPage = $derived(currentLeadPage > 1);
  let hasNextLeadPage = $derived(nextLeadCursor !== null);

  async function loadSmtpServers(): Promise<boolean> {
    try {
      const res = await listManagedSmtpAccounts();
      if (isDisposed) return false;
      smtpServers = [...res];
      if (smtpServers.length > 0) {
        if (selectedSmtpId === null && smtpServers[0].id) selectedSmtpId = smtpServers[0].id;
        if (selectedSmtpIds.length === 0 && smtpServers[0].id) selectedSmtpIds = [smtpServers[0].id];
      }
      return true;
    } catch (e) {
      console.error(e);
      return false;
    }
  }

  async function loadTemplates(): Promise<boolean> {
    try {
      const res = await invoke<EmailTemplate[]>('get_email_templates');
      if (isDisposed) return false;
      templates = [...res];
      if (templates.length > 0 && selectedTemplateId === null) selectedTemplateId = templates[0].id || null;
      return true;
    } catch (e) {
      console.error(e);
      return false;
    }
  }

  async function loadCategories(): Promise<boolean> {
    try {
      const result = await getCategories();
      if (isDisposed) return false;
      categories = result;
      return true;
    } catch (e) {
      console.error(e);
      return false;
    }
  }

  function selectValidEmail() {
    const emailLeads = leads.filter(l => l.email && l.email.trim() !== '' && l.email.includes('@'));
    selectAllMatching = false;
    excludedLeadIds = [];
    selectedLeadIds = emailLeads.map(l => l.id).filter((id): id is number => id != null);
  }

  function selectUnreached() {
    const unreached = leads.filter(l => !(l as any).email_reached);
    if (selectAllMatching) return;
    const newIds = new Set(selectedLeadIds);
    unreached.forEach(l => { if (l.id != null) newIds.add(l.id); });
    selectedLeadIds = Array.from(newIds);
  }

  $effect(() => {
    if (segmentationFilter === 'template_success' && !filterTemplateId && templates.length > 0) {
      filterTemplateId = templates[0].id || null;
    }
  });

  function currentLeadFilters(): OutreachLeadFilters {
    return {
      categoryId: selectedCategoryId,
      segmentation: segmentationFilter,
      filterTemplateId,
      channel: 'email',
      search: leadSearchQuery.trim(),
      filterVerifiedZalo: false,
      filterUnreached: false,
      filterMobileOnly: false
    };
  }

  function leadPageCacheKey(filters: OutreachLeadFilters, cursor: number | null): string {
    return `${JSON.stringify(filters)}::${cursor ?? 'first'}`;
  }

  function resetLeadSelection() {
    selectAllMatching = true;
    excludedLeadIds = [];
    selectedLeadIds = [];
  }

  function resetLeadPagination() {
    currentLeadPage = 1;
    pageCursors = [null];
    nextLeadCursor = null;
  }

  function invalidateLeadCache() {
    leadPageCache.clear();
    leadRequestId += 1;
  }

  async function reloadLeads(options: {
    page?: number;
    cursor?: number | null;
    force?: boolean;
    resetSelection?: boolean;
  } = {}): Promise<boolean> {
    const page = options.page ?? currentLeadPage;
    const cursor = options.cursor !== undefined
      ? options.cursor
      : (pageCursors[page - 1] ?? null);
    const filters = currentLeadFilters();
    const filterKey = JSON.stringify(filters);

    if (filters.segmentation === 'template_success' && !filters.filterTemplateId) {
      leads = [];
      totalLeads = 0;
      nextLeadCursor = null;
      loadedLeadFilterKey = filterKey;
      isLoadingLeads = false;
      isStale = false;
      if (options.resetSelection) resetLeadSelection();
      return true;
    }

    const cacheKey = leadPageCacheKey(filters, cursor);
    const requestId = ++leadRequestId;
    isLoadingLeads = true;

    try {
      let result = options.force ? undefined : leadPageCache.get(cacheKey);
      if (!result) {
        result = await getOutreachLeadsPage(filters, cursor, LEAD_PAGE_SIZE);
        if (isDisposed) return false;
        leadPageCache.set(cacheKey, result);
      }
      if (isDisposed || requestId !== leadRequestId) return true;

      leads = result.items;
      totalLeads = result.total;
      nextLeadCursor = result.next_cursor;
      currentLeadPage = page;
      const cursors = pageCursors.slice(0, page);
      cursors[page - 1] = cursor;
      pageCursors = cursors;
      loadedLeadFilterKey = filterKey;
      isStale = false;
      if (options.resetSelection) resetLeadSelection();
      return true;
    } catch (err) {
      console.error('Error loading email outreach lead page:', err);
      if (!isDisposed) isStale = true;
      return false;
    } finally {
      if (!isDisposed && requestId === leadRequestId) isLoadingLeads = false;
    }
  }

  async function showPreviousLeadPage() {
    if (!hasPreviousLeadPage || isLoadingLeads) return;
    const page = currentLeadPage - 1;
    await reloadLeads({ page, cursor: pageCursors[page - 1] ?? null });
  }

  async function showNextLeadPage() {
    if (!hasNextLeadPage || isLoadingLeads || nextLeadCursor === null) return;
    const page = currentLeadPage + 1;
    const cursor = nextLeadCursor;
    const cursors = pageCursors.slice();
    cursors[page - 1] = cursor;
    pageCursors = cursors;
    await reloadLeads({ page, cursor });
  }

  let {
    activeTab = 'email_outreach',
    onOpenSettings = () => {}
  }: {
    activeTab?: string;
    onOpenSettings?: () => void;
  } = $props();
  let hasLoaded = $state(false);
  let isStale = $state(false);
  let reloadTimer: ReturnType<typeof setTimeout> | null = null;
  let filterReloadTimer: ReturnType<typeof setTimeout> | null = null;

  function debouncedReload() {
    if (reloadTimer) clearTimeout(reloadTimer);
    reloadTimer = setTimeout(() => {
      reloadTimer = null;
      if (activeTab === 'email_outreach') {
        void refreshCrmData();
      } else {
        isStale = true;
      }
    }, 300);
  }

  async function refreshCrmData(): Promise<boolean> {
    invalidateLeadCache();
    const succeeded = await reloadLeads({ force: true });
    if (isDisposed) return false;
    isStale = !succeeded;
    return succeeded;
  }

  async function refreshData(): Promise<boolean> {
    const [categoriesLoaded, smtpLoaded, templatesLoaded] = await Promise.all([
      loadCategories(),
      loadSmtpServers(),
      loadTemplates()
    ]);
    if (isDisposed) return false;
    resetLeadPagination();
    const leadsLoaded = await reloadLeads({ page: 1, cursor: null, resetSelection: true });
    if (isDisposed) return false;
    const succeeded = categoriesLoaded && smtpLoaded && templatesLoaded && leadsLoaded;

    hasLoaded = succeeded;
    if (succeeded) {
      isStale = false;
    } else {
      isStale = true;
    }
    return succeeded;
  }

  $effect(() => {
    const filterKey = JSON.stringify({
      categoryId: selectedCategoryId,
      segmentation: segmentationFilter,
      filterTemplateId,
      channel: 'email',
      search: leadSearchQuery.trim(),
      filterVerifiedZalo: false,
      filterUnreached: false,
      filterMobileOnly: false
    });
    const shouldReload = untrack(
      () => activeTab === 'email_outreach' && hasLoaded && filterKey !== loadedLeadFilterKey
    );
    if (!shouldReload) return;

    if (filterReloadTimer) clearTimeout(filterReloadTimer);
    isLoadingLeads = true;
    filterReloadTimer = setTimeout(() => {
      filterReloadTimer = null;
      if (activeTab !== 'email_outreach') {
        isLoadingLeads = false;
        isStale = true;
        return;
      }
      resetLeadPagination();
      void reloadLeads({ page: 1, cursor: null, resetSelection: true });
    }, 250);
  });

  $effect(() => {
    const currentTab = activeTab;
    const isActive = currentTab === 'email_outreach';
    const wasInSettings = untrack(() => lastTab === 'settings');
    lastTab = currentTab;

    const cacheState = untrack(() => ({ hasLoaded, isStale }));

    if (isActive) {
      if (!cacheState.hasLoaded) {
        void refreshData();
      } else if (cacheState.isStale) {
        void refreshCrmData();
      } else if (wasInSettings) {
        void loadSmtpServers();
      }
    } else {
      if (reloadTimer) {
        clearTimeout(reloadTimer);
        reloadTimer = null;
        isStale = true;
      }
      if (filterReloadTimer) {
        clearTimeout(filterReloadTimer);
        filterReloadTimer = null;
        isLoadingLeads = false;
        isStale = true;
      }
    }
  });

  onMount(() => {
    const unsubBridge = crmService.initScraperEventBridge();
    const unsubLeads = crmService.onLeadsChanged(() => {
      if (activeTab === 'email_outreach') {
        debouncedReload();
      } else {
        isStale = true;
      }
    });

    return () => {
      isDisposed = true;
      leadRequestId += 1;
      unsubLeads();
      unsubBridge();
      if (reloadTimer) {
        clearTimeout(reloadTimer);
        reloadTimer = null;
      }
      if (filterReloadTimer) {
        clearTimeout(filterReloadTimer);
        filterReloadTimer = null;
      }
      cleanupCampaignListeners();
    };
  });

  function cleanupCampaignListeners() { campaignUnlistens.forEach(u => u()); campaignUnlistens = []; }

  function openAddTemplateModal() {
    editingTemplateId = null; templateName = ''; templateSubject = ''; templateContent = ''; templateError = null;
    activeInputField = 'content';
    isTemplateModalOpen = true;
  }

  function openEditTemplateModal(tpl: EmailTemplate) {
    editingTemplateId = tpl.id || null;
    templateName = tpl.name || '';
    templateSubject = tpl.subject || '';
    const rawContent = typeof tpl.content === 'string' ? tpl.content : (tpl.content ? String(tpl.content) : '');
    templateContent = rawContent.replace(/\[object Object\]/g, '');
    templateError = null;
    activeInputField = 'content';
    isTemplateModalOpen = true;
  }

  function insertVariableBadge(tag: string) {
    if (activeInputField === 'subject') templateSubject += tag;
    else templateContent += tag;
  }

  async function handleSaveTemplate() {
    if (!templateName.trim() || !templateSubject.trim()) { templateError = 'Name and Subject are required.'; return; }
    try {
      if (editingTemplateId) {
        await invoke('update_email_template', { id: editingTemplateId, name: templateName.trim(), subject: templateSubject.trim(), content: templateContent });
      } else {
        const savedId = await invoke<number>('create_email_template', { name: templateName.trim(), subject: templateSubject.trim(), content: templateContent });
        selectedTemplateId = savedId;
      }
      await loadTemplates();
      isTemplateModalOpen = false;
    } catch (e) { templateError = String(e); }
  }

  function promptDeleteTemplate(id: number) {
    templateToDeleteId = id;
    isDeleteTemplateModalOpen = true;
  }

  async function confirmDeleteTemplate() {
    if (!templateToDeleteId) return;
    try {
      await invoke('delete_email_template', { id: templateToDeleteId });
      if (selectedTemplateId === templateToDeleteId) selectedTemplateId = null;
      await loadTemplates();
      isDeleteTemplateModalOpen = false;
      templateToDeleteId = null;
    } catch (e) { alert(String(e)); }
  }

  function toggleLead(leadId: number) {
    if (selectAllMatching) {
      if (excludedLeadSet.has(leadId)) excludedLeadIds = excludedLeadIds.filter(id => id !== leadId);
      else excludedLeadIds = [...excludedLeadIds, leadId];
      return;
    }

    if (selectedLeadSet.has(leadId)) selectedLeadIds = selectedLeadIds.filter(id => id !== leadId);
    else selectedLeadIds = [...selectedLeadIds, leadId];
  }

  function toggleSelectAll() {
    if (isAllLeadsSelected) {
      selectAllMatching = false;
      excludedLeadIds = [];
      selectedLeadIds = [];
    } else {
      resetLeadSelection();
    }
  }

  async function resolveSelectedLeadIds(): Promise<number[]> {
    if (selectAllMatching) {
      return resolveOutreachLeadIds(currentLeadFilters(), excludedLeadIds);
    }
    return [...selectedLeadIds];
  }

  function appendCampaignLog(msg: string) {
    const timestamp = new Date().toLocaleTimeString();
    campaignLogs = [...campaignLogs, `[${timestamp}] ${msg}`];
    setTimeout(() => { if (campaignConsoleElement) campaignConsoleElement.scrollTop = campaignConsoleElement.scrollHeight; }, 50);
  }

  async function startCampaign() {
    const activeSmtpId = selectedSmtpIds[0] || selectedSmtpId;
    if (!activeSmtpId || !selectedTemplateId || selectedLeadCount === 0) return;

    isCampaignRunning = true; campaignProgress = 0; campaignStatus = 'Starting email campaign...'; campaignLogs = [];

    cleanupCampaignListeners();

    try {
      const campaignLeadIds = await resolveSelectedLeadIds();
      if (campaignLeadIds.length === 0) {
        isCampaignRunning = false;
        campaignStatus = 'No matching leads selected.';
        return;
      }
      appendCampaignLog(`Starting email campaign to ${campaignLeadIds.length} leads...`);

      const [progressUnlisten, logUnlisten, completeUnlisten] = await Promise.all([
        listen<{ progress: number; status: string; message: string }>('email-outreach-progress', (e) => {
          campaignProgress = e.payload.progress; campaignStatus = e.payload.message;
          if (e.payload.status === 'failed') isCampaignRunning = false;
        }),
        listen<{ message: string }>('email-outreach-log', (e) => appendCampaignLog(e.payload.message)),
        listen<{ total_sent: number; message: string }>('email-outreach-complete', (e) => {
          isCampaignRunning = false; campaignProgress = 100; campaignStatus = e.payload.message;
          appendCampaignLog(`[COMPLETE] ${e.payload.message}`);
          void refreshCrmData();
        })
      ]);
      campaignUnlistens.push(progressUnlisten, logUnlisten, completeUnlisten);

      await invoke('send_email_outreach', { smtpId: activeSmtpId, templateId: selectedTemplateId, leadIds: campaignLeadIds });
    } catch (e) { isCampaignRunning = false; appendCampaignLog(`Error: ${e}`); }
  }
</script>

<div class="flex flex-col gap-4">
  <!-- Header Bar -->
  <div class="flex items-center justify-between">
    <div class="flex flex-col">
      <h2 class="text-xl font-bold text-foreground tracking-tight">Email Outreach Manager</h2>
      <p class="text-xs text-muted-foreground mt-0.5">Select configured senders, manage templates, and launch targeted email broadcasts</p>
    </div>
  </div>

  <!-- Main Grid -->
  <div class="grid grid-cols-1 lg:grid-cols-12 gap-5">
    <!-- Left Column: SMTP & Templates -->
    <div class="lg:col-span-5 flex flex-col gap-4">
      <!-- Managed Email Accounts Card -->
      <Card.Root>
        <Card.Header class="pb-3 border-b border-border flex flex-row items-center justify-between">
          <Card.Title class="text-sm font-bold flex items-center gap-2">
            <Server class="size-4 text-emerald-600" /> Available Email Accounts
          </Card.Title>
          <div class="flex items-center gap-1">
            <Button variant="ghost" size="icon" class="size-7" onclick={() => loadSmtpServers()} title="Refresh accounts">
              <RefreshCw class="size-3.5" />
            </Button>
            <Button variant="outline" size="sm" class="h-7 text-[11px] px-2 gap-1" onclick={onOpenSettings}>
              <Settings2 class="size-3" /> Manage accounts in Settings
            </Button>
          </div>
        </Card.Header>
        <Card.Content class="p-3 flex flex-col gap-2 text-xs max-h-56 overflow-y-auto">
          {#if smtpServers.length === 0}
            <div class="flex flex-col items-center gap-2 py-4 text-center text-muted-foreground">
              <span class="italic">No email account is available.</span>
              <Button variant="link" size="sm" onclick={onOpenSettings}>Open account settings</Button>
            </div>
          {:else}
            {#each smtpServers as server (server.id)}
              <div
                class="p-2.5 rounded-lg border cursor-pointer transition-colors flex items-center justify-between {selectedSmtpId === server.id ? 'border-primary bg-primary/5' : 'border-border bg-card hover:bg-accent/40'}"
                onclick={() => selectedSmtpId = server.id || null}
                role="button"
                tabindex="0"
                onkeydown={(e) => e.key === 'Enter' && (selectedSmtpId = server.id || null)}
              >
                <div class="truncate flex flex-col gap-0.5">
                  <div class="font-semibold text-foreground flex items-center gap-1.5">
                    {server.name} <Badge variant="outline" class="text-[9px] px-1 py-0">{server.security}</Badge>
                  </div>
                  <div class="text-[10px] text-muted-foreground truncate">{server.host}:{server.port} • {server.fromEmail}</div>
                </div>
              </div>
            {/each}
          {/if}
        </Card.Content>
      </Card.Root>

      <!-- Email Templates Card -->
      <Card.Root>
        <Card.Header class="pb-3 border-b border-border flex flex-row items-center justify-between">
          <Card.Title class="text-sm font-bold flex items-center gap-2">
            <FileText class="size-4 text-emerald-600" /> Email Templates
          </Card.Title>
          <Button size="sm" variant="outline" class="h-7 text-[11px] px-2 gap-1" onclick={openAddTemplateModal}>
            <Plus class="size-3" /> New Template
          </Button>
        </Card.Header>
        <Card.Content class="p-3 flex flex-col gap-2 text-xs max-h-64 overflow-y-auto">
          {#if templates.length === 0}
            <div class="text-muted-foreground text-center py-4 italic">No templates created.</div>
          {:else}
            {#each templates as tpl (tpl.id)}
              <div
                class="p-2.5 rounded-lg border cursor-pointer transition-colors flex items-center justify-between {selectedTemplateId === tpl.id ? 'border-primary bg-primary/5' : 'border-border bg-card hover:bg-accent/40'}"
                onclick={() => selectedTemplateId = tpl.id || null}
                role="button"
                tabindex="0"
                onkeydown={(e) => e.key === 'Enter' && (selectedTemplateId = tpl.id || null)}
              >
                <div class="truncate flex flex-col gap-0.5">
                  <div class="font-semibold text-foreground">{tpl.name}</div>
                  <div class="text-[11px] text-muted-foreground truncate">Subject: {tpl.subject}</div>
                </div>
                <div class="flex items-center gap-1 shrink-0">
                  <Button variant="ghost" size="sm" class="size-6 p-0" onclick={(e) => { e.stopPropagation(); openEditTemplateModal(tpl); }}>
                    <Edit class="size-3" />
                  </Button>
                  <Button variant="ghost" size="sm" class="size-6 p-0 text-destructive" onclick={(e) => { e.stopPropagation(); tpl.id && promptDeleteTemplate(tpl.id); }}>
                    <Trash2 class="size-3" />
                  </Button>
                </div>
              </div>
            {/each}
          {/if}
        </Card.Content>
      </Card.Root>
    </div>

    <!-- Right Column: Campaign Execution -->
    <Card.Root class="lg:col-span-7 flex flex-col justify-between">
      <Card.Header class="pb-3 border-b border-border flex flex-row items-center justify-between">
        <Card.Title class="text-base font-bold flex items-center gap-2">
          <Send class="size-4 text-emerald-600" /> Campaign Execution
        </Card.Title>
        <Badge variant="outline" class="text-[10px]">Steps 1-3</Badge>
      </Card.Header>
      <Card.Content class="p-5 flex flex-col gap-4 text-xs">
        <!-- Step 1: Select Session(s) -->
        <div class="flex flex-col gap-1.5">
          <span class="font-semibold text-foreground">Step 1: Select Active SMTP Server(s)</span>
          <div class="flex flex-wrap gap-2 pt-1">
            {#if smtpServers.length === 0}
              <span class="text-xs text-muted-foreground">No active SMTP servers configured.</span>
            {:else}
              {#each smtpServers as s (s.id)}
                {@const isSelected = selectedSmtpIds.includes(s.id)}
                <label class="flex items-center gap-2 p-2 rounded-lg border border-border bg-card hover:bg-accent/40 cursor-pointer transition-colors {isSelected ? 'border-primary bg-primary/5 font-semibold' : ''}">
                  <Checkbox
                    checked={isSelected}
                    disabled={isCampaignRunning}
                    onCheckedChange={(checked) => {
                      if (checked) {
                        if (!selectedSmtpIds.includes(s.id)) selectedSmtpIds = [...selectedSmtpIds, s.id];
                      } else {
                        selectedSmtpIds = selectedSmtpIds.filter(id => id !== s.id);
                      }
                    }}
                  />
                  <span class="font-medium text-foreground">{s.name} ({s.host})</span>
                </label>
              {/each}
            {/if}
          </div>
        </div>

        <!-- Target Category -->
        <div class="flex flex-col gap-1">
          <label for="category-select-email" class="font-semibold text-foreground">Target Category</label>
          <Select.Root
            type="single"
            disabled={isCampaignRunning}
            value={selectedCategoryId != null ? String(selectedCategoryId) : ""}
            onValueChange={(val) => selectedCategoryId = val ? Number(val) : null}
          >
            <Select.Trigger class="w-full h-9 text-xs">
              {categories.find(c => String(c.id) === String(selectedCategoryId))?.name || "-- Select Category --"}
            </Select.Trigger>
            <Select.Content>
              {#each categories as cat (cat.id)}
                <Select.Item value={String(cat.id)}>{cat.name}</Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>
        </div>

        <!-- Campaign Segmentation Filter -->
        <div class="flex flex-col gap-1.5">
          <span class="font-semibold text-foreground">Campaign Segmentation Filter</span>
          <div class="flex flex-col gap-1 text-xs">
            <label class="flex items-center gap-2 cursor-pointer">
              <input type="radio" value="all" bind:group={segmentationFilter} disabled={isCampaignRunning} class="text-primary" />
              <span>All Leads in Category</span>
            </label>
            <label class="flex items-center gap-2 cursor-pointer">
              <input type="radio" value="never_sent" bind:group={segmentationFilter} disabled={isCampaignRunning} class="text-primary" />
              <span>Never Sent Successfully</span>
            </label>
            <label class="flex items-center gap-2 cursor-pointer">
              <input type="radio" value="last_failed" bind:group={segmentationFilter} disabled={isCampaignRunning} class="text-primary" />
              <span>Last Attempt Failed</span>
            </label>
            <label class="flex items-center gap-2 cursor-pointer">
              <input type="radio" value="template_success" bind:group={segmentationFilter} disabled={isCampaignRunning} class="text-primary" />
              <span>Successful Template Match</span>
            </label>
          </div>
        </div>

        {#if segmentationFilter === 'template_success'}
          <div class="flex flex-col gap-1 pl-4 border-l-2 border-primary/30">
            <label for="filter-template-email" class="font-semibold text-foreground">Filter by Success Template</label>
            <Select.Root
              type="single"
              disabled={isCampaignRunning}
              value={filterTemplateId != null ? String(filterTemplateId) : ""}
              onValueChange={(val) => filterTemplateId = val ? Number(val) : null}
            >
              <Select.Trigger class="w-full h-8 text-xs">
                {templates.find(t => String(t.id) === String(filterTemplateId))?.name || "-- Choose Template --"}
              </Select.Trigger>
              <Select.Content>
                {#each templates as t (t.id)}
                  <Select.Item value={String(t.id)}>{t.name}</Select.Item>
                {/each}
              </Select.Content>
            </Select.Root>
          </div>
        {/if}

        <!-- Step 2: Select Template -->
        <div class="flex flex-col gap-1">
          <label for="campaign-template-email" class="font-semibold text-foreground">Step 2: Select Template</label>
          <Select.Root
            type="single"
            disabled={isCampaignRunning}
            value={selectedTemplateId != null ? String(selectedTemplateId) : ""}
            onValueChange={(val) => selectedTemplateId = val ? Number(val) : null}
          >
            <Select.Trigger class="w-full h-9 text-xs">
              {templates.find(t => String(t.id) === String(selectedTemplateId))?.name || "-- Choose Template --"}
            </Select.Trigger>
            <Select.Content>
              {#each templates as t (t.id)}
                <Select.Item value={String(t.id)}>{t.name}</Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>
        </div>

        <!-- Step 3: Select Leads Box -->
        <div class="flex flex-col gap-2">
          <div class="flex items-center justify-between text-xs">
            <span class="font-semibold text-foreground">Step 3: Select Leads ({selectedLeadCount} chosen)</span>
            <div class="flex items-center gap-2">
              <Input type="text" placeholder="Search lead..." bind:value={leadSearchQuery} class="h-7 w-32 text-[11px]" />
              <Button variant="ghost" size="sm" class="h-7 text-[10px] px-2" onclick={toggleSelectAll}>
                {isAllLeadsSelected ? 'Deselect All' : 'Select All'}
              </Button>
            </div>
          </div>

          <!-- Quick Helpers -->
          <div class="flex items-center justify-between gap-2">
            <div class="flex items-center gap-1.5 text-[10px] text-muted-foreground">
              <Button variant="outline" size="sm" aria-label="Previous leads page" onclick={showPreviousLeadPage} disabled={!hasPreviousLeadPage || isLoadingLeads} class="h-6 text-[10px] px-2">
                Prev
              </Button>
              <span class="min-w-12 text-center">{currentLeadPage}/{totalLeadPages}</span>
              <Button variant="outline" size="sm" aria-label="Next leads page" onclick={showNextLeadPage} disabled={!hasNextLeadPage || isLoadingLeads} class="h-6 text-[10px] px-2">
                Next
              </Button>
            </div>
            <div class="flex items-center gap-1.5">
              <Button variant="outline" size="sm" onclick={selectValidEmail} disabled={leads.filter(l => l.email && l.email.includes('@')).length === 0} class="h-6 text-[10px] px-2">
                Valid Email Only
              </Button>
              <Button variant="outline" size="sm" onclick={selectUnreached} disabled={leads.length === 0} class="h-6 text-[10px] px-2">
                Unreached Only
              </Button>
            </div>
          </div>

          <div class="max-h-48 overflow-y-auto p-2 rounded-md border border-border bg-muted/20 flex flex-col gap-1">
            {#each leads as lead (lead.id)}
              {#if lead.id}
                {@const isSelected = selectAllMatching ? !excludedLeadSet.has(lead.id) : selectedLeadSet.has(lead.id)}
                <div
                  class="flex items-center justify-between p-1.5 rounded-md hover:bg-muted/50 cursor-pointer transition-colors text-xs {isSelected ? 'bg-primary/5' : ''}"
                  onclick={() => lead.id && toggleLead(lead.id)}
                  role="button"
                  tabindex="0"
                  onkeydown={(e) => e.key === 'Enter' && lead.id && toggleLead(lead.id)}
                >
                  <div class="flex items-center gap-2 truncate">
                    <div onclick={(e) => e.stopPropagation()} role="none">
                      <Checkbox
                        checked={isSelected}
                        disabled={isCampaignRunning}
                        onCheckedChange={() => lead.id && toggleLead(lead.id)}
                      />
                    </div>
                    <span class="font-medium text-foreground truncate">{lead.name} ({lead.company || 'N/A'})</span>
                  </div>
                  <span class="text-[11px] text-muted-foreground shrink-0">{lead.email || 'No email'}</span>
                </div>
              {/if}
            {/each}
          </div>
        </div>

        <!-- Campaign Action & Progress -->
        <div class="flex flex-col gap-3 pt-2 border-t border-border">
          <div class="flex items-center justify-between text-xs font-semibold">
            <span class="text-foreground">{campaignStatus}</span>
            <span class="text-emerald-600">{Math.round(campaignProgress)}%</span>
          </div>
          <Progress value={campaignProgress} class="h-2" />

          <Button
            size="sm"
            onclick={startCampaign}
            disabled={isCampaignRunning || isLoadingLeads || (selectedSmtpIds.length === 0 && !selectedSmtpId) || !selectedTemplateId || selectedLeadCount === 0}
            class="w-full gap-2 bg-emerald-600 hover:bg-emerald-700 text-white"
          >
            <Mail class="size-4" /> {isCampaignRunning ? 'Sending Email Campaign...' : 'Launch Email Campaign'}
          </Button>
        </div>

        <!-- Terminal Output -->
        <div class="min-h-[160px] max-h-[220px] p-3 rounded-lg bg-slate-950 text-slate-100 font-mono text-[11px] overflow-y-auto flex flex-col gap-1 border border-slate-800" bind:this={campaignConsoleElement}>
          {#if campaignLogs.length === 0}
            <div class="text-slate-500 italic">Campaign execution logs will display here...</div>
          {:else}
            {#each campaignLogs as log, index (index)}
              <div class="leading-relaxed border-b border-slate-900/60 pb-0.5">{log}</div>
            {/each}
          {/if}
        </div>
      </Card.Content>
    </Card.Root>
  </div>
</div>

<!-- Email Template Dialog -->
<Dialog.Root bind:open={isTemplateModalOpen}>
  <Dialog.Content class="sm:max-w-4xl max-w-[95vw] w-full max-h-[90vh] flex flex-col overflow-hidden p-6 gap-4">
    <Dialog.Header class="pb-2 border-b border-border">
      <Dialog.Title>{editingTemplateId ? 'Edit Email Template' : 'Create Email Template'}</Dialog.Title>
    </Dialog.Header>

    <div class="flex flex-col gap-4 py-1 text-xs overflow-y-auto pr-1 flex-1">
      {#if templateError}
        <div class="p-3 rounded-xl bg-destructive/10 border border-destructive/30 text-destructive text-xs font-semibold">
          {templateError}
        </div>
      {/if}

      <!-- Top Row: Name and Subject in 2 Columns -->
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div class="flex flex-col gap-1.5">
          <label for="tpl-name-email" class="font-bold text-foreground flex items-center gap-1">
            Template Name <span class="text-rose-500">*</span>
          </label>
          <Input
            id="tpl-name-email"
            bind:value={templateName}
            placeholder="e.g. Intro Sequence - Cold Outreach"
            class="text-xs h-9 rounded-xl font-semibold"
          />
        </div>
        <div class="flex flex-col gap-1.5">
          <label for="tpl-subject-email" class="font-bold text-foreground flex items-center gap-1">
            Subject Line <span class="text-rose-500">*</span>
          </label>
          <Input
            id="tpl-subject-email"
            bind:value={templateSubject}
            placeholder="e.g. Strategic Partnership Proposal for &#123;&#123;company&#125;&#125;"
            class="text-xs h-9 rounded-xl font-semibold"
            onfocus={() => activeInputField = 'subject'}
          />
        </div>
      </div>

      <!-- Variable Insertion Toolbar -->
      <div class="p-3 rounded-xl bg-muted/40 border border-border/70 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3">
        <div class="flex items-center gap-2 flex-wrap">
          <span class="text-[11px] font-bold text-muted-foreground">Chèn biến tự động:</span>
          <div class="flex flex-wrap gap-1.5">
            <Badge role="button" variant="outline" class="cursor-pointer hover:bg-accent text-[11px] font-mono px-2.5 py-1" onclick={() => insertVariableBadge('{{name}}')}>+ {'{{name}}'}</Badge>
            <Badge role="button" variant="outline" class="cursor-pointer hover:bg-accent text-[11px] font-mono px-2.5 py-1" onclick={() => insertVariableBadge('{{company}}')}>+ {'{{company}}'}</Badge>
            <Badge role="button" variant="outline" class="cursor-pointer hover:bg-accent text-[11px] font-mono px-2.5 py-1" onclick={() => insertVariableBadge('{{email}}')}>+ {'{{email}}'}</Badge>
            <Badge role="button" variant="outline" class="cursor-pointer hover:bg-accent text-[11px] font-mono px-2.5 py-1" onclick={() => insertVariableBadge('{{phone}}')}>+ {'{{phone}}'}</Badge>
          </div>
        </div>
        <Badge variant="secondary" class="text-[10px] font-mono">
          Trạng thái: <span class="capitalize font-bold ml-1 text-emerald-600">{activeInputField === 'subject' ? 'Subject' : 'Body'}</span>
        </Badge>
      </div>

      <!-- Content Area with Large Textarea & Live Preview -->
      <div class="flex flex-col gap-2 flex-1">
        <div class="flex items-center justify-between">
          <label for="tpl-content-email" class="font-bold text-foreground">Content</label>
          <span class="text-[11px] text-muted-foreground">Hỗ trợ plain text & HTML format</span>
        </div>

        <textarea
          id="tpl-content-email"
          rows="14"
          bind:value={templateContent}
          placeholder="Hi &#123;&#123;name&#125;&#125;,&#10;&#10;I hope this email finds you well...&#10;&#10;Best regards,&#10;Sales Team"
          class="w-full min-h-[300px] md:min-h-[380px] rounded-xl border border-input bg-background p-3.5 text-xs md:text-sm font-mono leading-relaxed text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring shadow-2xs resize-y"
          onfocus={() => activeInputField = 'content'}
        ></textarea>
      </div>
    </div>

    <Dialog.Footer class="gap-2 pt-3 border-t border-border">
      <Button variant="outline" size="sm" onclick={() => isTemplateModalOpen = false} class="h-9 px-4 rounded-xl font-semibold">
        Cancel
      </Button>
      <Button size="sm" onclick={handleSaveTemplate} class="h-9 px-5 rounded-xl font-bold bg-emerald-600 hover:bg-emerald-700 text-white shadow-sm">
        Save Template
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Delete Template Confirmation Dialog -->
<Dialog.Root bind:open={isDeleteTemplateModalOpen}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Delete Email Template</Dialog.Title>
    </Dialog.Header>
    <div class="py-2 text-xs text-muted-foreground leading-relaxed">
      Are you sure you want to delete this email template? This action cannot be undone.
    </div>
    <Dialog.Footer class="gap-2">
      <Button variant="outline" size="sm" onclick={() => isDeleteTemplateModalOpen = false}>Cancel</Button>
      <Button variant="destructive" size="sm" onclick={confirmDeleteTemplate}>Delete Template</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
