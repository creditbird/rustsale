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
    listManagedZaloAccounts,
    type ManagedZaloAccount
  } from '$lib/services/accountSettingsService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import * as Card from '$lib/components/ui/card';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Select from '$lib/components/ui/select';
  import { Progress } from '$lib/components/ui/progress';
  import { MessageSquare, Plus, Trash2, Edit, Send, Upload, Image, X, FileImage, Settings2 } from 'lucide-svelte';

  interface OutreachTemplate {
    id?: number;
    name: string;
    content: string;
    image_url?: string | null;
    created_at?: string;
  }

  let sessions = $state<ManagedZaloAccount[]>([]);
  let selectedImeis = $state<string[]>([]);

  let templates = $state<OutreachTemplate[]>([]);
  let selectedTemplateId = $state<number | null>(null);

  const LEAD_PAGE_SIZE = 50;

  let leads = $state<Lead[]>([]);
  let totalLeads = $state(0);
  let currentLeadPage = $state(1);
  let pageCursors = $state<(number | null)[]>([null]);
  let nextLeadCursor = $state<number | null>(null);
  let isLoadingLeads = $state(false);
  let categories = $state<Category[]>([]);
  let selectedCategoryId = $state<number | null>(null);
  let selectedLeadIds = $state<number[]>([]);
  let selectAllMatching = $state(true);
  let excludedLeadIds = $state<number[]>([]);

  let segmentationFilter = $state('all');
  let filterTemplateId = $state<number | null>(null);

  let leadSearchQuery = $state('');
  let filterVerifiedZalo = $state(false);
  let filterUnreached = $state(false);
  let filterMobileOnly = $state(false);

  let isTemplateModalOpen = $state(false);
  let editingTemplateId = $state<number | null>(null);
  let templateName = $state('');
  let templateContent = $state('');
  let templateImageUrl = $state('');
  let templateError = $state<string | null>(null);
  let fileInputRef = $state<HTMLInputElement | null>(null);
  let templateTextareaRef = $state<HTMLTextAreaElement | null>(null);

  function insertTagAtCursor(tag: string) {
    const textToInsert = `[${tag}]`;
    if (!templateTextareaRef) {
      templateContent += textToInsert;
      return;
    }

    const start = templateTextareaRef.selectionStart ?? templateContent.length;
    const end = templateTextareaRef.selectionEnd ?? templateContent.length;

    const before = templateContent.substring(0, start);
    const after = templateContent.substring(end);

    templateContent = before + textToInsert + after;

    setTimeout(() => {
      if (templateTextareaRef) {
        templateTextareaRef.focus();
        const newPos = start + textToInsert.length;
        templateTextareaRef.setSelectionRange(newPos, newPos);
      }
    }, 0);
  }

  let isDeleteTemplateModalOpen = $state(false);
  let templateToDelete = $state<OutreachTemplate | null>(null);
  let isDeletingTemplate = $state(false);

  let isCampaignRunning = $state(false);
  let campaignProgress = $state(0);
  let campaignStatus = $state('Idle');
  let campaignLogs = $state<string[]>([]);
  let consoleElement: HTMLDivElement | null = null;

  let enableBurstLimit = $state(true);
  let burstMaxMessages = $state(5);
  let burstDelaySeconds = $state(30);
  let interMessageDelaySeconds = $state(3);

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

  function getSessionDisplayName(session: ManagedZaloAccount): string {
    return session.displayName?.trim() || `Zalo (${session.accountKey.slice(0, 8)}...)`;
  }

  function getSessionAvatar(session: ManagedZaloAccount): string | null {
    return session.avatar;
  }

  async function loadSessions(): Promise<boolean> {
    try {
      const result = await listManagedZaloAccounts();
      if (isDisposed) return false;
      sessions = result;
      if (sessions.length > 0 && selectedImeis.length === 0) {
        selectedImeis = [sessions[0].accountKey];
      }
      return true;
    } catch (e) {
      console.error('Error loading Zalo sessions:', e);
      return false;
    }
  }

  async function loadTemplates(): Promise<boolean> {
    try {
      const result = await invoke<OutreachTemplate[]>('get_outreach_templates');
      if (isDisposed) return false;
      templates = result;
      if (templates.length > 0 && selectedTemplateId === null) {
        selectedTemplateId = templates[0].id || null;
      }
      return true;
    } catch (e) {
      console.error('Error loading outreach templates:', e);
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
      console.error('Error loading categories:', e);
      return false;
    }
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
      channel: 'zalo',
      search: leadSearchQuery.trim(),
      filterVerifiedZalo,
      filterUnreached,
      filterMobileOnly
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
      console.error('Error loading outreach lead page:', err);
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
    activeTab = 'outreach',
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
      if (activeTab === 'outreach') {
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
    const [categoriesLoaded, sessionsLoaded, templatesLoaded] = await Promise.all([
      loadCategories(),
      loadSessions(),
      loadTemplates()
    ]);
    if (isDisposed) return false;
    resetLeadPagination();
    const leadsLoaded = await reloadLeads({ page: 1, cursor: null, resetSelection: true });
    if (isDisposed) return false;
    const succeeded = categoriesLoaded && sessionsLoaded && templatesLoaded && leadsLoaded;

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
      channel: 'zalo',
      search: leadSearchQuery.trim(),
      filterVerifiedZalo,
      filterUnreached,
      filterMobileOnly
    });
    const shouldReload = untrack(
      () => activeTab === 'outreach' && hasLoaded && filterKey !== loadedLeadFilterKey
    );
    if (!shouldReload) return;

    if (filterReloadTimer) clearTimeout(filterReloadTimer);
    isLoadingLeads = true;
    filterReloadTimer = setTimeout(() => {
      filterReloadTimer = null;
      if (activeTab !== 'outreach') {
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
    const isActive = currentTab === 'outreach';
    const wasInSettings = untrack(() => lastTab === 'settings');
    lastTab = currentTab;

    const cacheState = untrack(() => ({ hasLoaded, isStale }));

    if (isActive) {
      if (!cacheState.hasLoaded) {
        void refreshData();
      } else if (cacheState.isStale) {
        void refreshCrmData();
      } else if (wasInSettings) {
        void loadSessions();
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
      if (activeTab === 'outreach') {
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

  function triggerFileInput() {
    fileInputRef?.click();
  }

  function handleFileSelect(e: Event) {
    const input = e.target as HTMLInputElement;
    if (input.files && input.files[0]) {
      const file = input.files[0];
      const reader = new FileReader();
      reader.onload = (event) => {
        if (event.target?.result) {
          templateImageUrl = event.target.result as string;
        }
      };
      reader.readAsDataURL(file);
    }
  }

  function removeImageAttachment() {
    templateImageUrl = '';
    if (fileInputRef) fileInputRef.value = '';
  }

  function openCreateTemplate() {
    editingTemplateId = null;
    templateName = '';
    templateContent = '';
    templateImageUrl = '';
    templateError = null;
    if (fileInputRef) fileInputRef.value = '';
    isTemplateModalOpen = true;
  }

  function openEditTemplate(template: OutreachTemplate) {
    editingTemplateId = template.id || null;
    templateName = template.name;
    templateContent = template.content;
    templateImageUrl = template.image_url || '';
    templateError = null;
    isTemplateModalOpen = true;
  }

  async function saveTemplate() {
    if (!templateName.trim() || !templateContent.trim()) {
      templateError = 'Name and content are required.';
      return;
    }
    try {
      const imageUrlParam = templateImageUrl.trim() ? templateImageUrl.trim() : null;
      if (editingTemplateId !== null) {
        await invoke('update_outreach_template', {
          id: editingTemplateId,
          name: templateName,
          content: templateContent,
          imageUrl: imageUrlParam,
        });
      } else {
        await invoke('create_outreach_template', {
          name: templateName,
          content: templateContent,
          imageUrl: imageUrlParam,
        });
      }
      isTemplateModalOpen = false;
      await loadTemplates();
    } catch (e) { templateError = String(e); }
  }

  function promptDeleteTemplate(template: OutreachTemplate) {
    templateToDelete = template;
    isDeleteTemplateModalOpen = true;
  }

  async function confirmDeleteTemplate() {
    if (!templateToDelete || !templateToDelete.id) return;
    isDeletingTemplate = true;
    try {
      await invoke('delete_outreach_template', { id: templateToDelete.id });
      if (selectedTemplateId === templateToDelete.id) selectedTemplateId = null;
      await loadTemplates();
      isDeleteTemplateModalOpen = false;
      templateToDelete = null;
    } catch (e) {
      alert(`Error deleting template: ${e}`);
    } finally {
      isDeletingTemplate = false;
    }
  }

  function toggleLead(id: number) {
    if (selectAllMatching) {
      if (excludedLeadSet.has(id)) excludedLeadIds = excludedLeadIds.filter(x => x !== id);
      else excludedLeadIds = [...excludedLeadIds, id];
      return;
    }

    if (selectedLeadSet.has(id)) selectedLeadIds = selectedLeadIds.filter(x => x !== id);
    else selectedLeadIds = [...selectedLeadIds, id];
  }

  function toggleSelectAllLeads() {
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
    setTimeout(() => {
      if (consoleElement) consoleElement.scrollTop = consoleElement.scrollHeight;
    }, 50);
  }

  async function startCampaign() {
    if (selectedImeis.length === 0 || !selectedTemplateId || selectedLeadCount === 0) return;

    isCampaignRunning = true;
    campaignProgress = 0;
    campaignStatus = 'Starting Zalo outreach campaign...';
    campaignLogs = [];

    cleanupCampaignListeners();

    try {
      const campaignLeadIds = await resolveSelectedLeadIds();
      if (campaignLeadIds.length === 0) {
        isCampaignRunning = false;
        campaignStatus = 'No matching leads selected.';
        return;
      }
      appendCampaignLog(`Starting Zalo Campaign to ${campaignLeadIds.length} leads using ${selectedImeis.length} session(s)...`);

      const progressUnlisten = await listen<{ progress: number; status: string; message: string }>('outreach-progress', (event) => {
        campaignProgress = event.payload.progress;
        campaignStatus = event.payload.message;
        if (event.payload.status === 'failed') isCampaignRunning = false;
      });
      campaignUnlistens.push(progressUnlisten);

      const logUnlisten = await listen<{ message: string }>('outreach-log', (event) => {
        appendCampaignLog(event.payload.message);
      });
      campaignUnlistens.push(logUnlisten);

      const completeUnlisten = await listen<{ total_sent: number; message: string }>('outreach-complete', (event) => {
        isCampaignRunning = false;
        campaignProgress = 100;
        campaignStatus = event.payload.message;
        appendCampaignLog(`[COMPLETE] ${event.payload.message}`);
        void refreshCrmData();
      });
      campaignUnlistens.push(completeUnlisten);

      await invoke('start_zalo_outreach', {
        imeis: selectedImeis,
        templateId: selectedTemplateId,
        leadIds: campaignLeadIds,
        enableBurstLimit,
        burstMaxMessages: Number(burstMaxMessages) || 5,
        burstDelaySeconds: Number(burstDelaySeconds) || 30,
        interMessageDelaySeconds: Number(interMessageDelaySeconds) || 3,
      });
    } catch (e) {
      isCampaignRunning = false;
      appendCampaignLog(`Error: ${e}`);
    }
  }
</script>

<div class="flex flex-col gap-4">
  <!-- Top Header Bar -->
  <div class="flex items-center justify-between">
    <div class="flex flex-col">
      <h2 class="text-xl font-bold text-foreground tracking-tight">Zalo Outreach Manager</h2>
      <p class="text-xs text-muted-foreground mt-0.5">Select connected accounts, customize templates, and launch campaign flows</p>
    </div>
  </div>

  <!-- Main Grid -->
  <div class="grid grid-cols-1 lg:grid-cols-12 gap-5">
    <!-- Left Column: Sessions & Templates -->
    <div class="lg:col-span-5 flex flex-col gap-4">
      <!-- Managed Accounts Card -->
      <Card.Root>
        <Card.Header class="pb-3 border-b border-border flex flex-row items-center justify-between">
          <Card.Title class="text-sm font-bold flex items-center gap-2">
            <MessageSquare class="size-4 text-emerald-600" /> Available Zalo Accounts
          </Card.Title>
          <Button variant="outline" size="sm" class="h-7 text-[11px] px-2 gap-1" onclick={onOpenSettings}>
            <Settings2 class="size-3" /> Manage accounts in Settings
          </Button>
        </Card.Header>
        <Card.Content class="p-3 flex flex-col gap-2 text-xs max-h-56 overflow-y-auto">
          {#if sessions.length === 0}
            <div class="flex flex-col items-center gap-2 py-4 text-center text-muted-foreground">
              <span class="italic">No Zalo account is available.</span>
              <Button variant="link" size="sm" onclick={onOpenSettings}>Open account settings</Button>
            </div>
          {:else}
            {#each sessions as s (s.accountKey)}
              {@const displayName = getSessionDisplayName(s)}
              {@const avatarUrl = getSessionAvatar(s)}
              <div class="flex items-center justify-between p-2 rounded-lg border border-border bg-card hover:bg-accent/40 transition-colors">
                <div class="flex items-center gap-2.5 truncate">
                  {#if avatarUrl}
                    <img src={avatarUrl} alt={displayName} class="size-8 rounded-full object-cover border border-emerald-500/30 shrink-0" />
                  {:else}
                    <div class="size-8 rounded-full bg-emerald-500/10 text-emerald-600 font-bold text-xs flex items-center justify-center border border-emerald-500/20 shrink-0">
                      {displayName[0].toUpperCase()}
                    </div>
                  {/if}
                  <div class="truncate flex flex-col gap-0.5">
                    <div class="font-semibold text-foreground truncate flex items-center gap-1.5">
                      {displayName}
                      <Badge variant="outline" class="text-[9px] px-1 py-0 capitalize">{s.authStatus}</Badge>
                      <Badge variant="outline" class="text-[9px] px-1 py-0 capitalize">{s.runtimeStatus}</Badge>
                    </div>
                    <div class="text-[10px] text-muted-foreground truncate">IMEI: {s.accountKey.slice(0, 16)}...</div>
                  </div>
                </div>
              </div>
            {/each}
          {/if}
        </Card.Content>
      </Card.Root>

      <!-- Templates Card -->
      <Card.Root>
        <Card.Header class="pb-3 border-b border-border flex flex-row items-center justify-between">
          <Card.Title class="text-sm font-bold flex items-center gap-2">
            <Edit class="size-4 text-emerald-600" /> Outreach Templates
          </Card.Title>
          <Button size="sm" variant="outline" class="h-7 text-[11px] px-2 gap-1" onclick={openCreateTemplate}>
            <Plus class="size-3" /> New Template
          </Button>
        </Card.Header>
        <Card.Content class="p-3 flex flex-col gap-2 text-xs max-h-64 overflow-y-auto">
          {#if templates.length === 0}
            <div class="text-muted-foreground text-center py-4 italic">No templates created.</div>
          {:else}
            {#each templates as t, index (t.id ?? index)}
              <div
                class="p-2.5 rounded-lg border cursor-pointer transition-colors flex items-center justify-between {selectedTemplateId === t.id ? 'border-primary bg-primary/5' : 'border-border bg-card hover:bg-accent/40'}"
                onclick={() => { if (t.id) selectedTemplateId = t.id; }}
                role="button"
                tabindex="0"
                onkeydown={(e) => e.key === 'Enter' && t.id && (selectedTemplateId = t.id)}
              >
                <div class="truncate flex items-center gap-2 min-w-0">
                  {#if t.image_url}
                    <img src={t.image_url} alt="Attachment" class="size-8 rounded object-cover border border-border shrink-0" onerror={(e: Event) => ((e.currentTarget as HTMLElement).style.display = 'none')} />
                  {/if}
                  <div class="truncate flex flex-col gap-0.5 min-w-0">
                    <div class="font-semibold text-foreground flex items-center gap-1.5">
                      <span class="truncate">{t.name}</span>
                      {#if t.image_url}
                        <Badge variant="secondary" class="text-[9px] px-1 py-0 h-4 shrink-0 font-normal">📷 Image</Badge>
                      {/if}
                    </div>
                    <div class="text-[11px] text-muted-foreground truncate">{t.content}</div>
                  </div>
                </div>
                <div class="flex items-center gap-1 shrink-0">
                  <Button variant="ghost" size="sm" class="size-6 p-0" onclick={(e) => { e.stopPropagation(); openEditTemplate(t); }}>
                    <Edit class="size-3" />
                  </Button>
                  <Button variant="ghost" size="sm" class="size-6 p-0 text-destructive" onclick={(e) => { e.stopPropagation(); promptDeleteTemplate(t); }}>
                    <Trash2 class="size-3" />
                  </Button>
                </div>
              </div>
            {/each}
          {/if}
        </Card.Content>
      </Card.Root>
    </div>

    <!-- Right Column: Campaign Flow & Terminal -->
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
          <span class="font-semibold text-foreground">Step 1: Select Active Zalo Session(s)</span>
          <div class="flex flex-wrap gap-2 pt-1">
            {#each sessions as s (s.accountKey)}
              {@const isSelected = selectedImeis.includes(s.accountKey)}
              {@const displayName = getSessionDisplayName(s)}
              {@const avatarUrl = getSessionAvatar(s)}
              <label 
                class="flex items-center gap-2.5 p-2 rounded-lg border border-border bg-card hover:bg-accent/40 cursor-pointer transition-colors {isSelected ? 'border-primary bg-primary/5 font-semibold' : ''}"
              >
                <Checkbox 
                  checked={isSelected}
                  disabled={isCampaignRunning}
                  onCheckedChange={(checked) => {
                    if (checked) {
                      if (!selectedImeis.includes(s.accountKey)) selectedImeis = [...selectedImeis, s.accountKey];
                    } else {
                      selectedImeis = selectedImeis.filter(i => i !== s.accountKey);
                    }
                  }}
                />
                {#if avatarUrl}
                  <img src={avatarUrl} alt={displayName} class="size-6 rounded-full object-cover border border-emerald-500/30 shrink-0" />
                {:else}
                  <div class="size-6 rounded-full bg-emerald-500/10 text-emerald-600 font-bold text-[10px] flex items-center justify-center border border-emerald-500/20 shrink-0">
                    {displayName[0].toUpperCase()}
                  </div>
                {/if}
                <span class="font-medium text-foreground">{displayName}</span>
              </label>
            {/each}
          </div>
        </div>

        <!-- Target Category -->
        <div class="flex flex-col gap-1">
          <label for="category-select" class="font-semibold text-foreground">Target Category</label>
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

        <!-- Campaign Segmentation Filters -->
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
              <span class="text-[10px] text-muted-foreground font-normal">(Bỏ qua số chặn / không Zalo)</span>
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
            <label for="filter-template" class="font-semibold text-foreground">Filter by Success Template</label>
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
          <label for="campaign-template" class="font-semibold text-foreground">Step 2: Select Template</label>
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
              <Input type="text" placeholder="Filter leads..." bind:value={leadSearchQuery} class="h-7 w-32 text-[11px]" />
              <Button variant="ghost" size="sm" class="h-7 text-[10px] px-2" onclick={toggleSelectAllLeads}>
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
              <Button
                variant={filterVerifiedZalo ? "default" : "outline"}
                size="sm"
                onclick={() => filterVerifiedZalo = !filterVerifiedZalo}
                class="h-6 text-[10px] px-2"
              >
                Verified Zalo
              </Button>
              <Button
                variant={filterUnreached ? "default" : "outline"}
                size="sm"
                onclick={() => filterUnreached = !filterUnreached}
                class="h-6 text-[10px] px-2"
              >
                Unreached
              </Button>
              <Button
                variant={filterMobileOnly ? "default" : "outline"}
                size="sm"
                onclick={() => filterMobileOnly = !filterMobileOnly}
                class="h-6 text-[10px] px-2"
              >
                Mobile Only
              </Button>
            </div>
          </div>

          <div class="max-h-48 overflow-y-auto p-2 rounded-md border border-border bg-muted/20 flex flex-col gap-1">
            {#each leads as lead (lead.id)}
              {#if lead.id}
                {@const isSelected = selectAllMatching ? !excludedLeadSet.has(lead.id) : selectedLeadSet.has(lead.id)}
                <div 
                  class="flex items-center justify-between p-1.5 rounded-md hover:bg-muted/50 cursor-pointer transition-colors text-xs {isSelected ? 'bg-primary/5 font-semibold' : ''}"
                  onclick={() => lead.id && toggleLead(lead.id)}
                  role="button"
                  tabindex="0"
                  onkeydown={(e) => e.key === 'Enter' && lead.id && toggleLead(lead.id)}
                >
                  <div class="flex items-center gap-2 truncate">
                    <Checkbox 
                      checked={isSelected}
                      disabled={isCampaignRunning}
                    />
                    <span class="font-medium text-foreground truncate">{lead.name}</span>
                    {#if lead.has_zalo === 1}
                      <Badge variant="secondary" class="text-[9px] px-1 py-0 bg-emerald-500/10 text-emerald-600 border-emerald-500/20">Zalo Verified</Badge>
                    {:else if lead.zalo_status === 'BLOCKED_STRANGER'}
                      <Badge variant="outline" class="text-[9px] px-1 py-0 bg-amber-500/10 text-amber-600 border-amber-500/20">Chặn người lạ</Badge>
                    {:else if lead.zalo_status === 'VERIFIED_NOT_EXIST' || lead.zalo_status === 'NOT_FOUND'}
                      <Badge variant="outline" class="text-[9px] px-1 py-0 bg-muted text-muted-foreground border-border">Không có Zalo</Badge>
                    {/if}
                  </div>
                  <div class="flex items-center gap-2 text-[11px] text-muted-foreground shrink-0">
                    <span>{lead.phone}</span>
                    {#if lead.phone && (lead.phone.startsWith('09') || lead.phone.startsWith('03') || lead.phone.startsWith('07') || lead.phone.startsWith('08') || lead.phone.startsWith('05'))}
                      <Badge variant="outline" class="text-[9px] px-1 py-0 bg-primary/10 text-primary border-primary/20">Mobile</Badge>
                    {/if}
                    <span class="truncate max-w-[120px]">{lead.company}</span>
                  </div>
                </div>
              {/if}
            {/each}
          </div>
        </div>

        <!-- Burst Limit Controls -->
        <div class="flex flex-col gap-2.5 p-3 rounded-lg border border-border bg-muted/20">
          <div class="flex items-center justify-between">
            <label for="enable-burst-limit" class="flex items-center gap-2 font-semibold text-foreground cursor-pointer text-xs">
              <Checkbox
                id="enable-burst-limit"
                checked={enableBurstLimit}
                disabled={isCampaignRunning}
                onCheckedChange={(checked) => enableBurstLimit = !!checked}
              />
              <span>Kích hoạt Burst Limit (Chống block Zalo)</span>
            </label>
            {#if enableBurstLimit}
              <Badge variant="outline" class="text-[9px] px-1.5 py-0.5 border-emerald-500/30 text-emerald-600 bg-emerald-500/10 font-semibold">Đã bật bảo vệ</Badge>
            {:else}
              <Badge variant="outline" class="text-[9px] px-1.5 py-0.5 text-muted-foreground border-border">Đã tắt</Badge>
            {/if}
          </div>

          {#if enableBurstLimit}
            <div class="grid grid-cols-3 gap-2 pt-1 border-t border-border/50 text-xs">
              <div class="flex flex-col gap-1">
                <label for="burst-max-msg" class="text-[11px] text-muted-foreground font-medium">Messages / burst</label>
                <Input
                  id="burst-max-msg"
                  type="number"
                  min="1"
                  disabled={isCampaignRunning}
                  bind:value={burstMaxMessages}
                  class="h-8 text-xs bg-background"
                />
              </div>
              <div class="flex flex-col gap-1">
                <label for="burst-delay-sec" class="text-[11px] text-muted-foreground font-medium">Burst delay (s)</label>
                <Input
                  id="burst-delay-sec"
                  type="number"
                  min="1"
                  disabled={isCampaignRunning}
                  bind:value={burstDelaySeconds}
                  class="h-8 text-xs bg-background"
                />
              </div>
              <div class="flex flex-col gap-1">
                <label for="inter-msg-delay-sec" class="text-[11px] text-muted-foreground font-medium">Inter-msg delay (s)</label>
                <Input
                  id="inter-msg-delay-sec"
                  type="number"
                  min="0"
                  disabled={isCampaignRunning}
                  bind:value={interMessageDelaySeconds}
                  class="h-8 text-xs bg-background"
                />
              </div>
            </div>
          {/if}
        </div>

        <!-- Action Button & Progress -->
        <div class="flex flex-col gap-3 pt-2 border-t border-border">
          <div class="flex items-center justify-between text-xs font-semibold">
            <span class="text-foreground">{campaignStatus}</span>
            <span class="text-emerald-600">{Math.round(campaignProgress)}%</span>
          </div>
          <Progress value={campaignProgress} class="h-2" />

          <Button
            size="sm"
            onclick={startCampaign}
            disabled={isCampaignRunning || isLoadingLeads || selectedImeis.length === 0 || !selectedTemplateId || selectedLeadCount === 0}
            class="w-full gap-2 bg-emerald-600 hover:bg-emerald-700 text-white"
          >
            <Send class="size-4" /> {isCampaignRunning ? 'Running Zalo Campaign...' : 'Launch Zalo Campaign'}
          </Button>
        </div>

        <!-- Terminal Output -->
        <div class="min-h-[160px] max-h-[220px] p-3 rounded-lg bg-slate-950 text-slate-100 font-mono text-[11px] overflow-y-auto flex flex-col gap-1 border border-slate-800" bind:this={consoleElement}>
          {#if campaignLogs.length === 0}
            <div class="text-slate-500 italic">Outreach console logs will stream here dynamically...</div>
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

<!-- Template Modal Dialog -->
<Dialog.Root bind:open={isTemplateModalOpen}>
  <Dialog.Content class="sm:max-w-2xl w-full">
    <Dialog.Header>
      <Dialog.Title class="text-base font-bold">{editingTemplateId ? 'Edit Outreach Template' : 'Create Outreach Template'}</Dialog.Title>
    </Dialog.Header>
    <div class="flex flex-col gap-4 py-2 text-xs">
      {#if templateError}
        <div class="p-2.5 rounded-lg bg-destructive/10 text-destructive text-xs font-medium">{templateError}</div>
      {/if}

      <div class="flex flex-col gap-1.5">
        <label for="tpl-name" class="font-semibold text-foreground">Template Name</label>
        <Input id="tpl-name" bind:value={templateName} placeholder="e.g. Welcome Intro / Promotion Outreach" class="text-xs h-9" />
      </div>

      <div class="flex flex-col gap-1.5">
        <div class="flex items-center justify-between">
          <label for="tpl-content" class="font-semibold text-foreground">Message Content</label>
          <div class="flex items-center gap-1.5">
            <span class="text-[11px] text-muted-foreground font-medium">Quick Tags:</span>
            <Button
              variant="outline"
              size="sm"
              class="h-6 px-2 text-[10px] font-mono hover:bg-primary/10 hover:text-primary transition-colors cursor-pointer"
              onclick={() => insertTagAtCursor('name')}
            >
              + [name]
            </Button>
            <Button
              variant="outline"
              size="sm"
              class="h-6 px-2 text-[10px] font-mono hover:bg-primary/10 hover:text-primary transition-colors cursor-pointer"
              onclick={() => insertTagAtCursor('company')}
            >
              + [company]
            </Button>
            <Button
              variant="outline"
              size="sm"
              class="h-6 px-2 text-[10px] font-mono hover:bg-primary/10 hover:text-primary transition-colors cursor-pointer"
              onclick={() => insertTagAtCursor('phone')}
            >
              + [phone]
            </Button>
          </div>
        </div>
        <textarea
          id="tpl-content"
          bind:this={templateTextareaRef}
          rows="7"
          bind:value={templateContent}
          placeholder="Write template content here... e.g. Chào [name], bên mình chuyên cung cấp giải pháp..."
          class="w-full rounded-md border border-input bg-background p-3 text-xs leading-relaxed focus:outline-hidden focus:ring-1 focus:ring-ring min-h-[160px] resize-y"
        ></textarea>
      </div>

      <div class="flex flex-col gap-1.5">
        <div class="flex items-center justify-between">
          <label for="tpl-image" class="font-semibold text-foreground">Image Attachment</label>
          <span class="text-[10px] text-muted-foreground font-normal">Optional</span>
        </div>

        <input
          type="file"
          accept="image/*"
          bind:this={fileInputRef}
          onchange={handleFileSelect}
          class="hidden"
        />

        {#if templateImageUrl.trim()}
          <div class="flex items-center justify-between gap-3 p-3 rounded-lg border border-border bg-card shadow-xs">
            <div class="flex items-center gap-3 min-w-0">
              <img
                src={templateImageUrl}
                alt="Template attachment preview"
                class="size-16 rounded-md object-cover border border-border shrink-0 shadow-xs"
                onerror={(e: Event) => ((e.currentTarget as HTMLElement).style.display = 'none')}
              />
              <div class="flex flex-col gap-1 truncate text-xs">
                <span class="font-semibold text-foreground truncate flex items-center gap-1.5">
                  <FileImage class="size-4 text-emerald-600 shrink-0" />
                  Image attached successfully
                </span>
                <span class="text-[11px] text-muted-foreground truncate max-w-[400px]">
                  {templateImageUrl.startsWith('data:image/') ? 'Uploaded local image file' : templateImageUrl}
                </span>
              </div>
            </div>
            <Button variant="ghost" size="sm" class="size-8 p-0 text-destructive hover:bg-destructive/10 shrink-0" onclick={removeImageAttachment}>
              <X class="size-4" />
            </Button>
          </div>
        {:else}
          <div class="flex flex-col gap-2.5 p-4 rounded-lg border border-dashed border-border bg-accent/20 text-center items-center justify-center">
            <Button type="button" variant="outline" size="sm" class="h-8 text-xs gap-2 bg-background shadow-xs px-4" onclick={triggerFileInput}>
              <Upload class="size-3.5 text-emerald-600" /> Upload Image from Computer
            </Button>
            <div class="text-[11px] text-muted-foreground">or enter image URL / local file path below</div>
            <Input id="tpl-image" bind:value={templateImageUrl} placeholder="https://example.com/photo.jpg or /path/to/image.jpg" class="text-xs h-8 bg-background max-w-lg" />
          </div>
        {/if}
      </div>
    </div>
    <Dialog.Footer class="gap-2 pt-2 border-t border-border">
      <Button variant="outline" size="sm" onclick={() => isTemplateModalOpen = false}>Cancel</Button>
      <Button size="sm" onclick={saveTemplate} class="px-5">Save Template</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Delete Template Confirmation Dialog -->
<Dialog.Root bind:open={isDeleteTemplateModalOpen}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Delete Outreach Template</Dialog.Title>
    </Dialog.Header>
    <div class="py-2 text-xs text-muted-foreground leading-relaxed">
      Are you sure you want to delete template <strong class="text-foreground">{templateToDelete?.name}</strong>?
      This action cannot be undone.
    </div>
    <Dialog.Footer class="gap-2">
      <Button variant="outline" size="sm" onclick={() => isDeleteTemplateModalOpen = false} disabled={isDeletingTemplate}>
        Cancel
      </Button>
      <Button variant="destructive" size="sm" onclick={confirmDeleteTemplate} disabled={isDeletingTemplate}>
        {isDeletingTemplate ? 'Deleting...' : 'Delete Template'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
