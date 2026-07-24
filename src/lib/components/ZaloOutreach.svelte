<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getLeads, getCategories, getCampaignLeads, type Lead, type Category } from '$lib/services/crmService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import * as Card from '$lib/components/ui/card';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Progress } from '$lib/components/ui/progress';
  import { MessageSquare, Plus, RefreshCw, Trash2, Edit, QrCode, Send, CheckCircle, AlertCircle, User } from 'lucide-svelte';

  interface ZaloSession {
    imei: string;
    user_agent: string;
    cookies_json: string;
    language: string;
  }

  interface ZaloSessionStatus {
    imei: string;
    is_valid: boolean;
    display_name?: string;
    avatar?: string;
    error_message?: string;
  }

  interface SessionStatusInfo {
    checking: boolean;
    valid: boolean | null;
    displayName?: string;
    avatar?: string;
    error?: string;
  }

  interface OutreachTemplate {
    id?: number;
    name: string;
    content: string;
    created_at?: string;
  }

  let sessions = $state<ZaloSession[]>([]);
  let templates = $state<OutreachTemplate[]>([]);
  let leads = $state<Lead[]>([]);
  let leadSearchQuery = $state('');

  let sessionStatuses = $state<Record<string, SessionStatusInfo>>({});
  let isVerifyingAll = $state(false);

  let selectedImeis = $state<string[]>([]);
  let selectedTemplateId = $state<number | null>(null);
  let selectedLeadIds = $state<number[]>([]);

  let categories = $state<Category[]>([]);
  let selectedCategoryId = $state<number | null>(null);
  let segmentationFilter = $state('all');
  let filterTemplateId = $state<number | null>(null);

  let isQrModalOpen = $state(false);
  let qrImage = $state<string | null>(null);
  let qrUser = $state<{ avatar: string; display_name: string } | null>(null);
  let qrStatus = $state('Initializing login...');
  let qrError = $state<string | null>(null);

  let isTemplateModalOpen = $state(false);
  let editingTemplateId = $state<number | null>(null);
  let templateName = $state('');
  let templateContent = $state('');
  let templateError = $state<string | null>(null);

  let isCampaignRunning = $state(false);
  let campaignProgress = $state(0);
  let campaignStatus = $state('Idle');
  let campaignLogs = $state<string[]>([]);
  let campaignConsoleElement: HTMLDivElement | null = null;

  let qrUnlistens: UnlistenFn[] = [];
  let campaignUnlistens: UnlistenFn[] = [];

  let filteredLeads = $derived(
    leads.filter(lead => 
      (lead.name || '').toLowerCase().includes(leadSearchQuery.toLowerCase()) ||
      (lead.company || '').toLowerCase().includes(leadSearchQuery.toLowerCase()) ||
      (lead.phone || '').includes(leadSearchQuery)
    )
  );

  let isAllLeadsSelected = $derived(
    filteredLeads.length > 0 && 
    filteredLeads.every(lead => lead.id && selectedLeadIds.includes(lead.id))
  );

  async function loadSessions(): Promise<boolean> {
    try {
      sessions = await invoke<ZaloSession[]>('get_zalo_sessions');
      if (sessions.length > 0 && selectedImeis.length === 0) {
        selectedImeis = [sessions[0].imei];
      }
      return true;
    } catch (e) {
      console.error('Error loading Zalo sessions:', e);
      return false;
    }
  }

  async function loadTemplates() {
    try {
      templates = await invoke<OutreachTemplate[]>('get_outreach_templates');
      if (templates.length > 0 && selectedTemplateId === null) {
        selectedTemplateId = templates[0].id || null;
      }
    } catch (e) {
      console.error('Error loading outreach templates:', e);
    }
  }

  async function loadLeadsData() {
    try {
      leads = await getLeads();
    } catch (e) {
      console.error('Error loading leads:', e);
    }
  }

  async function loadCategories() {
    try {
      categories = await getCategories();
    } catch (e) {
      console.error('Error loading categories:', e);
    }
  }

  $effect(() => {
    if (selectedCategoryId !== null) {
      getCampaignLeads(selectedCategoryId, segmentationFilter, filterTemplateId)
        .then(res => { leads = res; })
        .catch(err => { console.error('Error fetching campaign leads', err); leads = []; });
    } else {
      leads = [];
    }
  });

  async function verifySession(imei: string) {
    sessionStatuses[imei] = { ...sessionStatuses[imei], checking: true, valid: null };
    try {
      const res = await invoke<ZaloSessionStatus>('verify_zalo_session', { imei });
      sessionStatuses[imei] = {
        checking: false,
        valid: res ? res.is_valid : false,
        displayName: res?.display_name,
        avatar: res?.avatar,
        error: res?.error_message
      };
    } catch (e) {
      sessionStatuses[imei] = { checking: false, valid: false, error: String(e) };
    }
  }

  async function verifyAllSessions() {
    if (sessions.length === 0) return;
    isVerifyingAll = true;
    for (const s of sessions) {
      sessionStatuses[s.imei] = { ...sessionStatuses[s.imei], checking: true };
    }
    try {
      const results = await invoke<ZaloSessionStatus[]>('verify_all_zalo_sessions') || [];
      if (Array.isArray(results)) {
        for (const res of results) {
          sessionStatuses[res.imei] = {
            checking: false,
            valid: res.is_valid,
            displayName: res.display_name,
            avatar: res.avatar,
            error: res.error_message
          };
        }
      }
    } catch (e) {
      console.error('Error verifying all sessions:', e);
    } finally {
      isVerifyingAll = false;
    }
  }

  onMount(async () => {
    loadCategories();
    await loadSessions();
    verifyAllSessions();
    loadTemplates();
    loadLeadsData();
  });

  onDestroy(() => {
    cleanupQrListeners();
    cleanupCampaignListeners();
  });

  function cleanupQrListeners() { qrUnlistens.forEach(u => u()); qrUnlistens = []; }
  function cleanupCampaignListeners() { campaignUnlistens.forEach(u => u()); campaignUnlistens = []; }

  async function openQrLogin() {
    isQrModalOpen = true;
    qrImage = null;
    qrUser = null;
    qrStatus = 'Connecting to Zalo...';
    qrError = null;
    cleanupQrListeners();

    try {
      const codeUnlisten = await listen<{ image_base64: string }>('zalo-qr-code', (event) => {
        qrImage = event.payload.image_base64;
        qrStatus = 'Scan the QR code with your Zalo app to login.';
      });
      qrUnlistens.push(codeUnlisten);

      const scannedUnlisten = await listen<{ avatar: string; display_name: string }>('zalo-qr-scanned', (event) => {
        qrUser = { avatar: event.payload.avatar, display_name: event.payload.display_name };
        qrStatus = 'QR Code Scanned! Confirm on phone.';
      });
      qrUnlistens.push(scannedUnlisten);

      const successUnlisten = await listen<{ imei: string }>('zalo-qr-success', async () => {
        qrStatus = 'Login successful!';
        await loadSessions();
        closeQrModal();
      });
      qrUnlistens.push(successUnlisten);

      await invoke('start_zalo_qr_login');
    } catch (e) {
      qrError = `Failed: ${e}`;
    }
  }

  function closeQrModal() { isQrModalOpen = false; cleanupQrListeners(); }

  async function deleteSession(imei: string) {
    if (!confirm('Logout and delete session?')) return;
    try {
      await invoke('delete_zalo_session', { imei });
      selectedImeis = selectedImeis.filter(i => i !== imei);
      await loadSessions();
    } catch (e) { alert(`Error: ${e}`); }
  }

  function openCreateTemplate() {
    editingTemplateId = null;
    templateName = '';
    templateContent = '';
    templateError = null;
    isTemplateModalOpen = true;
  }

  function openEditTemplate(template: OutreachTemplate) {
    editingTemplateId = template.id || null;
    templateName = template.name;
    templateContent = template.content;
    templateError = null;
    isTemplateModalOpen = true;
  }

  async function saveTemplate() {
    if (!templateName.trim() || !templateContent.trim()) {
      templateError = 'Name and content are required.';
      return;
    }
    try {
      if (editingTemplateId !== null) {
        await invoke('update_outreach_template', { id: editingTemplateId, name: templateName, content: templateContent });
      } else {
        await invoke('create_outreach_template', { name: templateName, content: templateContent });
      }
      isTemplateModalOpen = false;
      await loadTemplates();
    } catch (e) { templateError = String(e); }
  }

  async function deleteTemplate(id: number) {
    if (!confirm('Delete template?')) return;
    try {
      await invoke('delete_outreach_template', { id });
      if (selectedTemplateId === id) selectedTemplateId = null;
      await loadTemplates();
    } catch (e) { alert(String(e)); }
  }

  function toggleLead(id: number) {
    if (selectedLeadIds.includes(id)) selectedLeadIds = selectedLeadIds.filter(x => x !== id);
    else selectedLeadIds = [...selectedLeadIds, id];
  }

  function toggleSelectAll() {
    const ids = filteredLeads.map(l => l.id).filter((id): id is number => !!id);
    if (ids.every(id => selectedLeadIds.includes(id))) {
      selectedLeadIds = selectedLeadIds.filter(id => !ids.includes(id));
    } else {
      const rest = selectedLeadIds.filter(id => !ids.includes(id));
      selectedLeadIds = [...rest, ...ids];
    }
  }

  function selectVerifiedZalo() {
    const verifiedIds = filteredLeads.filter(l => l.has_zalo === 1).map(l => l.id).filter((id): id is number => !!id);
    const otherSelected = selectedLeadIds.filter(id => !verifiedIds.includes(id));
    selectedLeadIds = [...otherSelected, ...verifiedIds];
  }

  function selectUnreached() {
    const unreachedIds = filteredLeads.filter(l => l.zalo_reached === 0 || l.zalo_reached === null || l.zalo_reached === undefined).map(l => l.id).filter((id): id is number => !!id);
    const otherSelected = selectedLeadIds.filter(id => !unreachedIds.includes(id));
    selectedLeadIds = [...otherSelected, ...unreachedIds];
  }

  function isVnMobile(phone: string | null | undefined): boolean {
    if (!phone) return false;
    const match = phone.match(/[^0-9\s+\-().,\[\]]/);
    const phonePart = match && match.index !== undefined ? phone.substring(0, match.index) : phone;
    const digits = phonePart.replace(/\D/g, '');
    let normalized = digits;
    if (digits.startsWith('0084')) normalized = '0' + digits.slice(4);
    else if (digits.startsWith('84')) normalized = '0' + digits.slice(2);
    if (normalized.length !== 10) return false;
    return ['03', '05', '07', '08', '09'].includes(normalized.slice(0, 2));
  }

  function selectMobileOnly() {
    const mobileIds = filteredLeads.filter(l => isVnMobile(l.phone)).map(l => l.id).filter((id): id is number => !!id);
    const otherSelected = selectedLeadIds.filter(id => !mobileIds.includes(id));
    selectedLeadIds = [...otherSelected, ...mobileIds];
  }

  function appendCampaignLog(msg: string) {
    const timestamp = new Date().toLocaleTimeString();
    campaignLogs = [...campaignLogs, `[${timestamp}] ${msg}`];
    setTimeout(() => { if (campaignConsoleElement) campaignConsoleElement.scrollTop = campaignConsoleElement.scrollHeight; }, 50);
  }

  async function startCampaign() {
    if (selectedImeis.length === 0 || selectedCategoryId === null || selectedTemplateId === null || selectedLeadIds.length === 0) return;

    isCampaignRunning = true;
    campaignProgress = 0;
    campaignStatus = 'Starting Zalo campaign...';
    campaignLogs = [];
    appendCampaignLog(`Starting Zalo campaign to ${selectedLeadIds.length} leads...`);

    cleanupCampaignListeners();

    try {
      const progressUnlisten = await listen<{ progress: number; status: string; message: string }>('campaign-progress', (event) => {
        campaignProgress = event.payload.progress;
        campaignStatus = event.payload.message;
        if (event.payload.status === 'failed') isCampaignRunning = false;
      });
      campaignUnlistens.push(progressUnlisten);

      const logUnlisten = await listen<{ message: string }>('campaign-log', (e) => appendCampaignLog(e.payload.message));
      campaignUnlistens.push(logUnlisten);

      const completeUnlisten = await listen<{ total_sent: number; message: string }>('campaign-complete', (e) => {
        isCampaignRunning = false;
        campaignProgress = 100;
        campaignStatus = e.payload.message;
        appendCampaignLog(`[COMPLETE] ${e.payload.message}`);
      });
      campaignUnlistens.push(completeUnlisten);

      await invoke('send_zalo_campaign', {
        categoryId: selectedCategoryId,
        sessionImeis: selectedImeis,
        templateId: selectedTemplateId
      });
    } catch (e) {
      isCampaignRunning = false;
      appendCampaignLog(`Error: ${e}`);
    }
  }
</script>

<div class="flex flex-col gap-4">
  <!-- Header Bar -->
  <div class="flex items-center justify-between">
    <div class="flex flex-col">
      <h2 class="text-xl font-bold text-foreground tracking-tight">Zalo Outreach Manager</h2>
      <p class="text-xs text-muted-foreground mt-0.5">Authenticate sessions, customize templates, and launch campaign flows</p>
    </div>
  </div>

  <!-- Layout Grid -->
  <div class="grid grid-cols-1 lg:grid-cols-12 gap-5">
    <!-- Left Column: Sessions & Templates -->
    <div class="lg:col-span-5 flex flex-col gap-4">
      <!-- Sessions Card -->
      <Card.Root>
        <Card.Header class="pb-3 border-b border-border flex flex-row items-center justify-between">
          <Card.Title class="text-sm font-bold flex items-center gap-2">
            <MessageSquare class="size-4 text-emerald-600" /> Zalo Sessions
          </Card.Title>
          <div class="flex items-center gap-1.5">
            <Button variant="outline" size="sm" class="h-7 text-[11px] px-2 gap-1" onclick={verifyAllSessions} disabled={isVerifyingAll}>
              <RefreshCw class="size-3 {isVerifyingAll ? 'animate-spin' : ''}" /> Check All
            </Button>
            <Button size="sm" class="h-7 text-[11px] px-2 gap-1 bg-emerald-600 hover:bg-emerald-700 text-white" onclick={openQrLogin}>
              <QrCode class="size-3" /> Add Session
            </Button>
          </div>
        </Card.Header>
        <Card.Content class="p-3 flex flex-col gap-2 text-xs max-h-56 overflow-y-auto">
          {#if sessions.length === 0}
            <div class="text-muted-foreground text-center py-4 italic">No Zalo sessions logged in.</div>
          {:else}
            {#each sessions as s (s.imei)}
              <div class="flex items-center justify-between p-2 rounded-lg border border-border bg-card hover:bg-accent/40 transition-colors">
                <div class="flex items-center gap-2 truncate">
                  {#if sessionStatuses[s.imei]?.valid === true}
                    <CheckCircle class="size-4 text-emerald-500 shrink-0" />
                  {:else if sessionStatuses[s.imei]?.valid === false}
                    <AlertCircle class="size-4 text-rose-500 shrink-0" />
                  {:else}
                    <User class="size-4 text-muted-foreground shrink-0" />
                  {/if}
                  <div class="truncate flex flex-col">
                    <div class="font-semibold text-foreground truncate">{sessionStatuses[s.imei]?.displayName || `IMEI: ${s.imei}`}</div>
                    <div class="text-[10px] text-muted-foreground truncate">{s.user_agent}</div>
                  </div>
                </div>
                <div class="flex items-center gap-1">
                  <Button variant="ghost" size="sm" class="size-6 p-0" onclick={() => verifySession(s.imei)}>
                    <RefreshCw class="size-3 {sessionStatuses[s.imei]?.checking ? 'animate-spin' : ''}" />
                  </Button>
                  <Button variant="ghost" size="sm" class="size-6 p-0 text-destructive" onclick={() => deleteSession(s.imei)}>
                    <Trash2 class="size-3" />
                  </Button>
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
                <div class="truncate flex flex-col gap-0.5">
                  <div class="font-semibold text-foreground">{t.name}</div>
                  <div class="text-[11px] text-muted-foreground truncate">{t.content}</div>
                </div>
                <div class="flex items-center gap-1 shrink-0">
                  <Button variant="ghost" size="sm" class="size-6 p-0" onclick={(e) => { e.stopPropagation(); openEditTemplate(t); }}>
                    <Edit class="size-3" />
                  </Button>
                  <Button variant="ghost" size="sm" class="size-6 p-0 text-destructive" onclick={(e) => { e.stopPropagation(); t.id && deleteTemplate(t.id); }}>
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
            {#each sessions as s (s.imei)}
              {@const isSelected = selectedImeis.includes(s.imei)}
              <label 
                class="flex items-center gap-2 p-2 rounded-lg border border-border bg-card hover:bg-accent/40 cursor-pointer transition-colors {isSelected ? 'border-primary bg-primary/5 font-semibold' : ''}"
              >
                <Checkbox 
                  checked={isSelected}
                  disabled={isCampaignRunning}
                  onCheckedChange={(checked) => {
                    if (checked) {
                      if (!selectedImeis.includes(s.imei)) selectedImeis = [...selectedImeis, s.imei];
                    } else {
                      selectedImeis = selectedImeis.filter(i => i !== s.imei);
                    }
                  }}
                />
                <span class="font-medium text-foreground">IMEI: {s.imei}</span>
              </label>
            {/each}
          </div>
        </div>

        <!-- Target Category -->
        <div class="flex flex-col gap-1">
          <label for="category-select" class="font-semibold text-foreground">Target Category</label>
          <select id="category-select" bind:value={selectedCategoryId} class="w-full h-9 rounded-md border border-input bg-background px-3 text-xs cursor-pointer" disabled={isCampaignRunning}>
            <option value={null}>-- Select Category --</option>
            {#each categories as cat (cat.id)}
              <option value={cat.id}>{cat.name}</option>
            {/each}
          </select>
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
            <select id="filter-template" bind:value={filterTemplateId} class="w-full h-8 rounded-md border border-input bg-background px-3 text-xs cursor-pointer" disabled={isCampaignRunning}>
              <option value={null}>-- Choose Template --</option>
              {#each templates as t (t.id)}
                <option value={t.id}>{t.name}</option>
              {/each}
            </select>
          </div>
        {/if}

        <!-- Step 2: Select Template -->
        <div class="flex flex-col gap-1">
          <label for="campaign-template" class="font-semibold text-foreground">Step 2: Select Template</label>
          <select id="campaign-template" bind:value={selectedTemplateId} class="w-full h-9 rounded-md border border-input bg-background px-3 text-xs cursor-pointer" disabled={isCampaignRunning}>
            <option value={null}>-- Choose Template --</option>
            {#each templates as t (t.id)}
              <option value={t.id}>{t.name}</option>
            {/each}
          </select>
        </div>

        <!-- Step 3: Select Leads Box -->
        <div class="flex flex-col gap-2">
          <div class="flex items-center justify-between text-xs">
            <span class="font-semibold text-foreground">Step 3: Select Leads ({selectedLeadIds.length} chosen)</span>
            <div class="flex items-center gap-2">
              <Input type="text" placeholder="Filter leads..." bind:value={leadSearchQuery} class="h-7 w-32 text-[11px]" />
              <Button variant="ghost" size="sm" class="h-7 text-[10px] px-2" onclick={toggleSelectAll}>
                {isAllLeadsSelected ? 'Deselect All' : 'Select All'}
              </Button>
            </div>
          </div>

          <!-- Quick Selection Helpers -->
          <div class="flex items-center justify-end gap-1.5">
            <Button variant="outline" size="sm" onclick={selectVerifiedZalo} disabled={filteredLeads.filter(l => l.has_zalo === 1).length === 0} class="h-6 text-[10px] px-2">
              Verified Zalo
            </Button>
            <Button variant="outline" size="sm" onclick={selectUnreached} disabled={filteredLeads.filter(l => l.zalo_reached === 0 || l.zalo_reached == null).length === 0} class="h-6 text-[10px] px-2">
              Unreached
            </Button>
            <Button variant="outline" size="sm" onclick={selectMobileOnly} disabled={filteredLeads.filter(l => isVnMobile(l.phone)).length === 0} class="h-6 text-[10px] px-2">
              Mobile Only
            </Button>
          </div>

          <div class="max-h-48 overflow-y-auto p-2 rounded-md border border-border bg-muted/20 flex flex-col gap-1">
            {#each filteredLeads as lead (lead.id)}
              {#if lead.id}
                {@const isSelected = selectedLeadIds.includes(lead.id)}
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
                    <span class="font-medium text-foreground truncate">{lead.name}</span>
                    {#if lead.has_zalo === 1}
                      <Badge variant="secondary" class="text-[9px] px-1 py-0 bg-sky-100 text-sky-700 dark:bg-sky-950 dark:text-sky-300">Zalo Verified</Badge>
                    {:else if lead.has_zalo === 0}
                      <Badge variant="outline" class="text-[9px] px-1 py-0 text-slate-400">No Zalo</Badge>
                    {/if}
                  </div>
                  <div class="flex items-center gap-2 shrink-0 text-[11px] text-muted-foreground">
                    <span>{lead.phone || 'No phone'}</span>
                    {#if !lead.phone || lead.phone.trim() === ''}
                      <Badge variant="destructive" class="text-[9px] px-1 py-0">Missing</Badge>
                    {:else if isVnMobile(lead.phone)}
                      <Badge variant="default" class="text-[9px] px-1 py-0 bg-emerald-600">Mobile</Badge>
                    {:else}
                      <Badge variant="outline" class="text-[9px] px-1 py-0">Landline</Badge>
                    {/if}
                    <span>• {lead.company || 'N/A'}</span>
                  </div>
                </div>
              {/if}
            {/each}
          </div>
        </div>

        <!-- Progress & Execution Button -->
        <div class="flex flex-col gap-3 pt-2 border-t border-border">
          <div class="flex items-center justify-between text-xs font-semibold">
            <span class="text-foreground">{campaignStatus}</span>
            <span class="text-emerald-600">{Math.round(campaignProgress)}%</span>
          </div>
          <Progress value={campaignProgress} class="h-2" />

          <Button
            size="sm"
            onclick={startCampaign}
            disabled={isCampaignRunning || selectedImeis.length === 0 || selectedCategoryId === null || selectedTemplateId === null || selectedLeadIds.length === 0}
            class="w-full gap-2 bg-emerald-600 hover:bg-emerald-700 text-white"
          >
            <Send class="size-4" /> {isCampaignRunning ? 'Running Campaign...' : 'Launch Zalo Campaign'}
          </Button>
        </div>

        <!-- Terminal Logs -->
        <div class="min-h-[160px] max-h-[220px] p-3 rounded-lg bg-slate-950 text-slate-100 font-mono text-[11px] overflow-y-auto flex flex-col gap-1 border border-slate-800" bind:this={campaignConsoleElement}>
          {#if campaignLogs.length === 0}
            <div class="text-slate-500 italic">Outreach console logs will show here dynamically...</div>
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

<!-- QR Login Dialog -->
<Dialog.Root bind:open={isQrModalOpen}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Zalo QR Login</Dialog.Title>
    </Dialog.Header>
    <div class="flex flex-col items-center justify-center p-6 gap-4">
      {#if qrImage}
        <img src={qrImage.startsWith('data:image/') ? qrImage : `data:image/png;base64,${qrImage}`} alt="Zalo QR Code" class="size-56 rounded-lg border border-border shadow-xs" />
      {:else if qrUser}
        <div class="text-center flex flex-col gap-2">
          <img src={qrUser.avatar} alt="User Avatar" class="size-16 rounded-full mx-auto border border-border" />
          <h3 class="font-bold text-foreground">{qrUser.display_name}</h3>
        </div>
      {:else}
        <div class="size-12 rounded-full border-2 border-primary border-t-transparent animate-spin"></div>
      {/if}
      <p class="text-xs text-muted-foreground font-medium text-center">{qrStatus}</p>
    </div>
  </Dialog.Content>
</Dialog.Root>

<!-- Template Modal Dialog -->
<Dialog.Root bind:open={isTemplateModalOpen}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{editingTemplateId ? 'Edit Outreach Template' : 'Create Outreach Template'}</Dialog.Title>
    </Dialog.Header>
    <div class="flex flex-col gap-4 py-2 text-xs">
      {#if templateError}
        <div class="p-2 rounded bg-destructive/10 text-destructive text-xs">{templateError}</div>
      {/if}
      <div class="flex flex-col gap-1">
        <label for="tpl-name" class="font-semibold text-foreground">Template Name</label>
        <Input id="tpl-name" bind:value={templateName} placeholder="e.g. Welcome Intro" class="text-xs" />
      </div>
      <div class="flex flex-col gap-1">
        <label for="tpl-content" class="font-semibold text-foreground">Message Content</label>
        <textarea id="tpl-content" rows="5" bind:value={templateContent} placeholder="Write template content..." class="w-full rounded-md border border-input bg-background p-2 text-xs"></textarea>
      </div>
    </div>
    <Dialog.Footer class="gap-2">
      <Button variant="outline" size="sm" onclick={() => isTemplateModalOpen = false}>Cancel</Button>
      <Button size="sm" onclick={saveTemplate}>Save Template</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
