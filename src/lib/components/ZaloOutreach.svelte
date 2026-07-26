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
  import * as Select from '$lib/components/ui/select';
  import { Progress } from '$lib/components/ui/progress';
  import { MessageSquare, Plus, RefreshCw, Trash2, Edit, QrCode, Send, CheckCircle, AlertCircle, User, Upload, Image, X, FileImage } from 'lucide-svelte';

  interface ZaloSession {
    imei: string;
    user_agent: string;
    cookies_json: string;
    language: string;
    display_name?: string | null;
    avatar?: string | null;
  }

  interface ZaloSessionStatus {
    imei: string;
    is_valid: boolean;
    display_name?: string;
    avatar?: string;
    error_message?: string;
  }

  interface OutreachTemplate {
    id?: number;
    name: string;
    content: string;
    image_url?: string | null;
    created_at?: string;
  }

  let sessions = $state<ZaloSession[]>([]);
  let sessionStatuses = $state<Record<string, { valid: boolean | null; checking: boolean; displayName?: string; avatar?: string; error?: string }>>({});
  let selectedImeis = $state<string[]>([]);
  let isVerifyingAll = $state(false);

  let templates = $state<OutreachTemplate[]>([]);
  let selectedTemplateId = $state<number | null>(null);

  let leads = $state<Lead[]>([]);
  let categories = $state<Category[]>([]);
  let selectedCategoryId = $state<number | null>(null);
  let selectedLeadIds = $state<number[]>([]);

  let segmentationFilter = $state('all');
  let filterTemplateId = $state<number | null>(null);

  let leadSearchQuery = $state('');
  let filterVerifiedZalo = $state(false);
  let filterUnreached = $state(false);
  let filterMobileOnly = $state(false);

  let isQrModalOpen = $state(false);
  let qrImage = $state<string | null>(null);
  let qrUser = $state<{ avatar: string; display_name: string } | null>(null);
  let qrStatus = $state('Initializing Zalo QR code...');
  let qrError = $state<string | null>(null);

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

  let isDeleteSessionModalOpen = $state(false);
  let sessionToDelete = $state<ZaloSession | null>(null);
  let isDeletingSession = $state(false);

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

  let qrUnlistens: UnlistenFn[] = [];
  let campaignUnlistens: UnlistenFn[] = [];

  let filteredLeads = $derived(
    leads.filter(lead => {
      const q = leadSearchQuery.toLowerCase().trim();
      if (q) {
        const matchesName = (lead.name || '').toLowerCase().includes(q);
        const matchesCompany = (lead.company || '').toLowerCase().includes(q);
        const matchesPhone = (lead.phone || '').toLowerCase().includes(q);
        if (!matchesName && !matchesCompany && !matchesPhone) return false;
      }
      if (filterVerifiedZalo && lead.has_zalo !== 1) return false;
      if (filterUnreached && lead.zalo_reached === 1) return false;
      if (filterMobileOnly && !lead.phone) return false;
      return true;
    })
  );

  let isAllLeadsSelected = $derived(
    filteredLeads.length > 0 && 
    filteredLeads.every(lead => lead.id && selectedLeadIds.includes(lead.id))
  );

  function getSessionDisplayName(s: { imei: string; display_name?: string | null }): string {
    const status = sessionStatuses[s.imei];
    if (status?.displayName && status.displayName.trim()) {
      return status.displayName;
    }
    if (s.display_name && s.display_name.trim()) {
      return s.display_name;
    }
    return `Zalo (${s.imei.slice(0, 8)}...)`;
  }

  function getSessionAvatar(s: { imei: string; avatar?: string | null }): string | null | undefined {
    const status = sessionStatuses[s.imei];
    return status?.avatar || s.avatar;
  }

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
    if (segmentationFilter === 'template_success' && !filterTemplateId && templates.length > 0) {
      filterTemplateId = templates[0].id || null;
    }
  });

  async function reloadLeads() {
    if (selectedCategoryId !== null) {
      if (segmentationFilter === 'template_success' && !filterTemplateId) {
        leads = [];
        selectedLeadIds = [];
        return;
      }
      try {
        const res = await getCampaignLeads(selectedCategoryId, segmentationFilter, filterTemplateId, 'zalo');
        leads = res;
        selectedLeadIds = leads.map(l => l.id).filter((id): id is number => id != null);
      } catch (err) {
        console.error('Error fetching campaign leads:', err);
        leads = [];
        selectedLeadIds = [];
      }
    } else {
      leads = [];
      selectedLeadIds = [];
    }
  }

  $effect(() => {
    // track dependencies
    const cat = selectedCategoryId;
    const seg = segmentationFilter;
    const tmpl = filterTemplateId;
    reloadLeads();
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
      await loadSessions();
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
      await loadSessions();
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
        qrStatus = `Scanned by ${event.payload.display_name}! Confirm on phone...`;
      });
      qrUnlistens.push(scannedUnlisten);

      const successUnlisten = await listen<{ imei: string }>('zalo-qr-success', async (event) => {
        qrStatus = 'Login successful!';
        setTimeout(async () => {
          isQrModalOpen = false;
          cleanupQrListeners();
          await loadSessions();
          verifySession(event.payload.imei);
        }, 1000);
      });
      qrUnlistens.push(successUnlisten);

      const errorUnlisten = await listen<{ message: string }>('zalo-qr-error', (event) => {
        qrError = event.payload.message;
        qrStatus = event.payload.message;
      });
      qrUnlistens.push(errorUnlisten);

      await invoke('start_zalo_qr_login');
    } catch (e) {
      const message = String(e);
      qrError = message;
      qrStatus = message;
    }
  }

  function closeQrModal() { isQrModalOpen = false; cleanupQrListeners(); }

  function promptDeleteSession(session: ZaloSession) {
    sessionToDelete = session;
    isDeleteSessionModalOpen = true;
  }

  async function confirmDeleteSession() {
    if (!sessionToDelete) return;
    isDeletingSession = true;
    try {
      await invoke('delete_zalo_session', { imei: sessionToDelete.imei });
      selectedImeis = selectedImeis.filter(i => i !== sessionToDelete?.imei);
      await loadSessions();
      isDeleteSessionModalOpen = false;
      sessionToDelete = null;
    } catch (e) {
      alert(`Error deleting session: ${e}`);
    } finally {
      isDeletingSession = false;
    }
  }

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
    if (selectedLeadIds.includes(id)) selectedLeadIds = selectedLeadIds.filter(x => x !== id);
    else selectedLeadIds = [...selectedLeadIds, id];
  }

  function toggleSelectAllLeads() {
    if (isAllLeadsSelected) {
      const filteredIds = new Set(filteredLeads.map(l => l.id));
      selectedLeadIds = selectedLeadIds.filter(id => !filteredIds.has(id));
    } else {
      const newIds = new Set(selectedLeadIds);
      filteredLeads.forEach(l => { if (l.id != null) newIds.add(l.id); });
      selectedLeadIds = Array.from(newIds);
    }
  }

  function appendCampaignLog(msg: string) {
    const timestamp = new Date().toLocaleTimeString();
    campaignLogs = [...campaignLogs, `[${timestamp}] ${msg}`];
    setTimeout(() => {
      if (consoleElement) consoleElement.scrollTop = consoleElement.scrollHeight;
    }, 50);
  }

  async function startCampaign() {
    if (selectedImeis.length === 0 || !selectedTemplateId || selectedLeadIds.length === 0) return;

    isCampaignRunning = true;
    campaignProgress = 0;
    campaignStatus = 'Starting Zalo outreach campaign...';
    campaignLogs = [];
    appendCampaignLog(`Starting Zalo Campaign to ${selectedLeadIds.length} leads using ${selectedImeis.length} session(s)...`);

    cleanupCampaignListeners();

    try {
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
        reloadLeads();
      });
      campaignUnlistens.push(completeUnlisten);

      await invoke('start_zalo_outreach', {
        imeis: selectedImeis,
        templateId: selectedTemplateId,
        leadIds: selectedLeadIds,
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
      <p class="text-xs text-muted-foreground mt-0.5">Authenticate sessions, customize templates, and launch campaign flows</p>
    </div>
  </div>

  <!-- Main Grid -->
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
              {@const status = sessionStatuses[s.imei]}
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
                      {#if status?.valid === true}
                        <Badge variant="outline" class="text-[9px] px-1 py-0 border-emerald-500/30 text-emerald-600 bg-emerald-500/10">Active</Badge>
                      {:else if status?.valid === false}
                        <Badge variant="outline" class="text-[9px] px-1 py-0 border-rose-500/30 text-rose-600 bg-rose-500/10">Expired</Badge>
                      {/if}
                    </div>
                    <div class="text-[10px] text-muted-foreground truncate">IMEI: {s.imei.slice(0, 16)}...</div>
                  </div>
                </div>
                <div class="flex items-center gap-1 shrink-0">
                  <Button variant="ghost" size="sm" class="size-6 p-0" onclick={() => verifySession(s.imei)}>
                    <RefreshCw class="size-3 {status?.checking ? 'animate-spin' : ''}" />
                  </Button>
                  <Button variant="ghost" size="sm" class="size-6 p-0 text-destructive" onclick={() => promptDeleteSession(s)}>
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
            {#each sessions as s (s.imei)}
              {@const isSelected = selectedImeis.includes(s.imei)}
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
                      if (!selectedImeis.includes(s.imei)) selectedImeis = [...selectedImeis, s.imei];
                    } else {
                      selectedImeis = selectedImeis.filter(i => i !== s.imei);
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
            <span class="font-semibold text-foreground">Step 3: Select Leads ({selectedLeadIds.length} chosen)</span>
            <div class="flex items-center gap-2">
              <Input type="text" placeholder="Filter leads..." bind:value={leadSearchQuery} class="h-7 w-32 text-[11px]" />
              <Button variant="ghost" size="sm" class="h-7 text-[10px] px-2" onclick={toggleSelectAllLeads}>
                {isAllLeadsSelected ? 'Deselect All' : 'Select All'}
              </Button>
            </div>
          </div>

          <!-- Quick Helpers -->
          <div class="flex items-center justify-end gap-1.5">
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

          <div class="max-h-48 overflow-y-auto p-2 rounded-md border border-border bg-muted/20 flex flex-col gap-1">
            {#each filteredLeads as lead (lead.id)}
              {#if lead.id}
                {@const isSelected = selectedLeadIds.includes(lead.id)}
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
            disabled={isCampaignRunning || selectedImeis.length === 0 || !selectedTemplateId || selectedLeadIds.length === 0}
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

<!-- Delete Session Confirmation Dialog -->
<Dialog.Root bind:open={isDeleteSessionModalOpen}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Logout & Delete Zalo Session</Dialog.Title>
    </Dialog.Header>
    <div class="py-2 text-xs text-muted-foreground leading-relaxed">
      Are you sure you want to log out and delete the session for
      <strong class="text-foreground">{sessionToDelete ? getSessionDisplayName(sessionToDelete) : ''}</strong>?
      This action will remove saved credentials from your system.
    </div>
    <Dialog.Footer class="gap-2">
      <Button variant="outline" size="sm" onclick={() => isDeleteSessionModalOpen = false} disabled={isDeletingSession}>
        Cancel
      </Button>
      <Button variant="destructive" size="sm" onclick={confirmDeleteSession} disabled={isDeletingSession}>
        {isDeletingSession ? 'Deleting...' : 'Delete Session'}
      </Button>
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
