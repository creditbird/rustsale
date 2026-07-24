<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getLeads, getCategories, getCampaignLeads, type Lead, type Category } from '$lib/services/crmService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import * as Card from '$lib/components/ui/card';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Progress } from '$lib/components/ui/progress';
  import { Mail, Plus, Server, Edit, Trash2, Send, Terminal, FileText, RefreshCw } from 'lucide-svelte';

  interface SmtpServer {
    id?: number;
    name: string;
    host: string;
    port: number;
    username: string;
    password: string;
    from_email: string;
    from_name: string;
    security: string;
    created_at?: string;
  }

  interface EmailTemplate {
    id?: number;
    name: string;
    subject: string;
    content: string;
    created_at?: string;
  }

  let smtpServers = $state<SmtpServer[]>([]);
  let templates = $state<EmailTemplate[]>([]);
  let leads = $state<Lead[]>([]);
  let categories = $state<Category[]>([]);

  let selectedSmtpId = $state<number | null>(null);
  let selectedSmtpIds = $state<number[]>([]);
  let selectedTemplateId = $state<number | null>(null);
  let selectedLeadIds = $state<number[]>([]);

  let selectedCategoryId = $state<number | null>(null);
  let segmentationFilter = $state('all');
  let filterTemplateId = $state<number | null>(null);
  let leadSearchQuery = $state('');

  let isSmtpModalOpen = $state(false);
  let editingSmtpId = $state<number | null>(null);
  let smtpName = $state('');
  let smtpHost = $state('');
  let smtpPort = $state(587);
  let smtpUsername = $state('');
  let smtpPassword = $state('');
  let smtpFromEmail = $state('');
  let smtpFromName = $state('');
  let smtpSecurity = $state('STARTTLS');
  let smtpTesting = $state(false);
  let smtpTestResult = $state<{ success: boolean; message: string } | null>(null);
  let smtpError = $state<string | null>(null);

  let isTemplateModalOpen = $state(false);
  let editingTemplateId = $state<number | null>(null);
  let templateName = $state('');
  let templateSubject = $state('');
  let templateContent = $state('');
  let templateError = $state<string | null>(null);
  let activeInputField = $state<'subject' | 'content'>('content');

  let isCampaignRunning = $state(false);
  let campaignProgress = $state(0);
  let campaignStatus = $state('Idle');
  let campaignLogs = $state<string[]>([]);
  let campaignConsoleElement: HTMLDivElement | null = null;

  let campaignUnlistens: UnlistenFn[] = [];

  let filteredLeads = $derived(
    leads.filter(lead =>
      (lead.name || '').toLowerCase().includes(leadSearchQuery.toLowerCase()) ||
      (lead.company || '').toLowerCase().includes(leadSearchQuery.toLowerCase()) ||
      (lead.email || '').toLowerCase().includes(leadSearchQuery.toLowerCase())
    )
  );

  let isAllLeadsSelected = $derived(
    filteredLeads.length > 0 &&
    filteredLeads.every(lead => lead.id && selectedLeadIds.includes(lead.id))
  );

  async function loadSmtpServers(): Promise<void> {
    try {
      const res = await invoke<SmtpServer[]>('get_smtp_servers');
      smtpServers = [...res];
      if (smtpServers.length > 0) {
        if (selectedSmtpId === null && smtpServers[0].id) selectedSmtpId = smtpServers[0].id;
        if (selectedSmtpIds.length === 0 && smtpServers[0].id) selectedSmtpIds = [smtpServers[0].id];
      }
    } catch (e) { console.error(e); }
  }

  async function loadTemplates(): Promise<void> {
    try {
      const res = await invoke<EmailTemplate[]>('get_email_templates');
      templates = [...res];
      if (templates.length > 0 && selectedTemplateId === null) selectedTemplateId = templates[0].id || null;
    } catch (e) { console.error(e); }
  }

  async function loadLeadsData(): Promise<void> {
    try {
      leads = await getLeads();
      selectedLeadIds = leads.filter(l => l.id != null).map(l => l.id as number);
    } catch (e) { console.error(e); }
  }

  async function loadCategories(): Promise<void> {
    try { categories = await getCategories(); } catch (e) { console.error(e); }
  }

  function selectValidEmail() {
    const emailLeads = filteredLeads.filter(l => l.email && l.email.includes('@'));
    const newIds = new Set(selectedLeadIds);
    emailLeads.forEach(l => { if (l.id != null) newIds.add(l.id); });
    selectedLeadIds = Array.from(newIds);
  }

  function selectUnreached() {
    const unreached = filteredLeads.filter(l => !(l as any).email_reached);
    const newIds = new Set(selectedLeadIds);
    unreached.forEach(l => { if (l.id != null) newIds.add(l.id); });
    selectedLeadIds = Array.from(newIds);
  }

  $effect(() => {
    if (selectedCategoryId !== null && selectedCategoryId !== undefined) {
      getCampaignLeads(Number(selectedCategoryId), segmentationFilter, filterTemplateId)
        .then(res => {
          leads = res;
          selectedLeadIds = leads.filter(l => l.id != null).map(l => l.id as number);
        })
        .catch(err => { console.error(err); leads = []; });
    }
  });

  onMount(async () => {
    loadCategories();
    await loadSmtpServers();
    await loadTemplates();
    await loadLeadsData();
  });

  function cleanupCampaignListeners() { campaignUnlistens.forEach(u => u()); campaignUnlistens = []; }

  function openAddSmtpModal() {
    editingSmtpId = null;
    smtpName = ''; smtpHost = ''; smtpPort = 587; smtpUsername = ''; smtpPassword = '';
    smtpFromEmail = ''; smtpFromName = ''; smtpSecurity = 'STARTTLS'; smtpTestResult = null; smtpError = null;
    isSmtpModalOpen = true;
  }

  function openEditSmtpModal(server: SmtpServer) {
    editingSmtpId = server.id || null;
    smtpName = server.name; smtpHost = server.host; smtpPort = server.port;
    smtpUsername = server.username; smtpPassword = server.password;
    smtpFromEmail = server.from_email; smtpFromName = server.from_name; smtpSecurity = server.security;
    smtpTestResult = null; smtpError = null;
    isSmtpModalOpen = true;
  }

  async function handleTestSmtpConnection() {
    smtpTesting = true; smtpTestResult = null; smtpError = null;
    try {
      const res = await invoke<string>('test_smtp_connection', { host: smtpHost, port: Number(smtpPort) });
      smtpTestResult = { success: true, message: res };
    } catch (e) {
      smtpTestResult = { success: false, message: String(e) };
    } finally { smtpTesting = false; }
  }

  async function handleSaveSmtpServer() {
    if (!smtpName.trim() || !smtpHost.trim()) { smtpError = 'Name and Host are required.'; return; }
    try {
      const serverPayload: SmtpServer = {
        id: editingSmtpId || undefined,
        name: smtpName.trim(), host: smtpHost.trim(), port: Number(smtpPort),
        username: smtpUsername.trim(), password: smtpPassword,
        from_email: smtpFromEmail.trim(), from_name: smtpFromName.trim(),
        security: smtpSecurity, created_at: new Date().toISOString()
      };
      const savedId = await invoke<number>('save_smtp_server', { server: serverPayload });
      selectedSmtpId = savedId || editingSmtpId || null;
      await loadSmtpServers();
      isSmtpModalOpen = false;
    } catch (e) { smtpError = String(e); }
  }

  async function handleDeleteSmtpServer(id: number) {
    if (!confirm('Delete SMTP server?')) return;
    try {
      await invoke('delete_smtp_server', { id });
      if (selectedSmtpId === id) selectedSmtpId = null;
      await loadSmtpServers();
    } catch (e) { alert(String(e)); }
  }

  function openAddTemplateModal() {
    editingTemplateId = null; templateName = ''; templateSubject = ''; templateContent = ''; templateError = null;
    activeInputField = 'content';
    isTemplateModalOpen = true;
  }

  function openEditTemplateModal(tpl: EmailTemplate) {
    editingTemplateId = tpl.id || null; templateName = tpl.name; templateSubject = tpl.subject; templateContent = tpl.content; templateError = null;
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

  async function handleDeleteTemplate(id: number) {
    if (!confirm('Delete template?')) return;
    try {
      await invoke('delete_email_template', { id });
      if (selectedTemplateId === id) selectedTemplateId = null;
      await loadTemplates();
    } catch (e) { alert(String(e)); }
  }

  function toggleLead(leadId: number) {
    if (selectedLeadIds.includes(leadId)) selectedLeadIds = selectedLeadIds.filter(id => id !== leadId);
    else selectedLeadIds = [...selectedLeadIds, leadId];
  }

  function toggleSelectAll() {
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
    setTimeout(() => { if (campaignConsoleElement) campaignConsoleElement.scrollTop = campaignConsoleElement.scrollHeight; }, 50);
  }

  async function startCampaign() {
    const activeSmtpId = selectedSmtpIds[0] || selectedSmtpId;
    if (!activeSmtpId || !selectedTemplateId || selectedLeadIds.length === 0) return;

    isCampaignRunning = true; campaignProgress = 0; campaignStatus = 'Starting email campaign...'; campaignLogs = [];
    appendCampaignLog(`Starting email campaign to ${selectedLeadIds.length} leads...`);

    cleanupCampaignListeners();

    try {
      const [progressUnlisten, logUnlisten, completeUnlisten] = await Promise.all([
        listen<{ progress: number; status: string; message: string }>('email-outreach-progress', (e) => {
          campaignProgress = e.payload.progress; campaignStatus = e.payload.message;
          if (e.payload.status === 'failed') isCampaignRunning = false;
        }),
        listen<{ message: string }>('email-outreach-log', (e) => appendCampaignLog(e.payload.message)),
        listen<{ total_sent: number; message: string }>('email-outreach-complete', (e) => {
          isCampaignRunning = false; campaignProgress = 100; campaignStatus = e.payload.message;
          appendCampaignLog(`[COMPLETE] ${e.payload.message}`);
        })
      ]);
      campaignUnlistens.push(progressUnlisten, logUnlisten, completeUnlisten);

      await invoke('send_email_outreach', { smtp_id: activeSmtpId, template_id: selectedTemplateId, lead_ids: selectedLeadIds });
    } catch (e) { isCampaignRunning = false; appendCampaignLog(`Error: ${e}`); }
  }
</script>

<div class="flex flex-col gap-4">
  <!-- Header Bar -->
  <div class="flex items-center justify-between">
    <div class="flex flex-col">
      <h2 class="text-xl font-bold text-foreground tracking-tight">Email Outreach Manager</h2>
      <p class="text-xs text-muted-foreground mt-0.5">Configure SMTP servers, HTML templates, and targeted email broadcasts</p>
    </div>
  </div>

  <!-- Main Grid -->
  <div class="grid grid-cols-1 lg:grid-cols-12 gap-5">
    <!-- Left Column: SMTP & Templates -->
    <div class="lg:col-span-5 flex flex-col gap-4">
      <!-- SMTP Servers Card -->
      <Card.Root>
        <Card.Header class="pb-3 border-b border-border flex flex-row items-center justify-between">
          <Card.Title class="text-sm font-bold flex items-center gap-2">
            <Server class="size-4 text-emerald-600" /> SMTP Configurations
          </Card.Title>
          <Button size="sm" class="h-7 text-[11px] px-2 gap-1 bg-emerald-600 hover:bg-emerald-700 text-white" onclick={openAddSmtpModal}>
            <Plus class="size-3" /> Add Server
          </Button>
        </Card.Header>
        <Card.Content class="p-3 flex flex-col gap-2 text-xs max-h-56 overflow-y-auto">
          {#if smtpServers.length === 0}
            <div class="text-muted-foreground text-center py-4 italic">No SMTP servers added yet.</div>
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
                  <div class="text-[10px] text-muted-foreground truncate">{server.host}:{server.port} • {server.from_email}</div>
                </div>
                <div class="flex items-center gap-1 shrink-0">
                  <Button variant="ghost" size="sm" class="size-6 p-0" onclick={(e) => { e.stopPropagation(); openEditSmtpModal(server); }}>
                    <Edit class="size-3" />
                  </Button>
                  <Button variant="ghost" size="sm" class="size-6 p-0 text-destructive" onclick={(e) => { e.stopPropagation(); server.id && handleDeleteSmtpServer(server.id); }}>
                    <Trash2 class="size-3" />
                  </Button>
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
                  <Button variant="ghost" size="sm" class="size-6 p-0 text-destructive" onclick={(e) => { e.stopPropagation(); tpl.id && handleDeleteTemplate(tpl.id); }}>
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
                {#if s.id}
                  <label class="flex items-center gap-1.5 p-1.5 rounded border border-border bg-card hover:bg-accent/30 cursor-pointer text-xs">
                    <input type="checkbox" value={s.id} bind:group={selectedSmtpIds} disabled={isCampaignRunning} class="rounded border-input text-primary" />
                    <span class="font-medium text-foreground">{s.name} ({s.host})</span>
                  </label>
                {/if}
              {/each}
            {/if}
          </div>
        </div>

        <!-- Target Category -->
        <div class="flex flex-col gap-1">
          <label for="category-select-email" class="font-semibold text-foreground">Target Category</label>
          <select id="category-select-email" bind:value={selectedCategoryId} class="w-full h-9 rounded-md border border-input bg-background px-3 text-xs" disabled={isCampaignRunning}>
            <option value={null}>-- Select Category --</option>
            {#each categories as cat (cat.id)}
              <option value={cat.id}>{cat.name}</option>
            {/each}
          </select>
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
            <select id="filter-template-email" bind:value={filterTemplateId} class="w-full h-8 rounded-md border border-input bg-background px-3 text-xs" disabled={isCampaignRunning}>
              <option value={null}>-- Choose Template --</option>
              {#each templates as t (t.id)}
                <option value={t.id}>{t.name}</option>
              {/each}
            </select>
          </div>
        {/if}

        <!-- Step 2: Select Template -->
        <div class="flex flex-col gap-1">
          <label for="campaign-template-email" class="font-semibold text-foreground">Step 2: Select Template</label>
          <select id="campaign-template-email" bind:value={selectedTemplateId} class="w-full h-9 rounded-md border border-input bg-background px-3 text-xs" disabled={isCampaignRunning}>
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
              <Input type="text" placeholder="Search lead..." bind:value={leadSearchQuery} class="h-7 w-32 text-[11px]" />
              <Button variant="ghost" size="sm" class="h-7 text-[10px] px-2" onclick={toggleSelectAll}>
                {isAllLeadsSelected ? 'Deselect All' : 'Select All'}
              </Button>
            </div>
          </div>

          <!-- Quick Helpers -->
          <div class="flex items-center justify-end gap-1.5">
            <Button variant="outline" size="sm" onclick={selectValidEmail} disabled={filteredLeads.filter(l => l.email && l.email.includes('@')).length === 0} class="h-6 text-[10px] px-2">
              Valid Email Only
            </Button>
            <Button variant="outline" size="sm" onclick={selectUnreached} disabled={filteredLeads.length === 0} class="h-6 text-[10px] px-2">
              Unreached Only
            </Button>
          </div>

          <div class="max-h-48 overflow-y-auto p-2 rounded-md border border-border bg-muted/20 flex flex-col gap-1">
            {#each filteredLeads as lead (lead.id)}
              {#if lead.id}
                <label class="flex items-center justify-between p-1.5 rounded hover:bg-muted/50 cursor-pointer text-xs">
                  <div class="flex items-center gap-2 truncate">
                    <input type="checkbox" checked={selectedLeadIds.includes(lead.id)} onchange={() => lead.id && toggleLead(lead.id)} disabled={isCampaignRunning} class="rounded border-input text-primary shrink-0" />
                    <span class="font-medium text-foreground truncate">{lead.name} ({lead.company || 'N/A'})</span>
                  </div>
                  <span class="text-[11px] text-muted-foreground shrink-0">{lead.email || 'No email'}</span>
                </label>
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
            disabled={isCampaignRunning || (selectedSmtpIds.length === 0 && !selectedSmtpId) || !selectedTemplateId || selectedLeadIds.length === 0}
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

<!-- SMTP Dialog -->
<Dialog.Root bind:open={isSmtpModalOpen}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{editingSmtpId ? 'Edit SMTP Configuration' : 'Add SMTP Configuration'}</Dialog.Title>
    </Dialog.Header>
    <div class="flex flex-col gap-3 py-2 text-xs">
      {#if smtpError}
        <div class="p-2 rounded bg-destructive/10 text-destructive text-xs">{smtpError}</div>
      {/if}
      <div class="flex flex-col gap-1">
        <label for="smtp-name" class="font-semibold text-foreground">Server Name *</label>
        <Input id="smtp-name" bind:value={smtpName} placeholder="e.g. Work SendGrid" class="text-xs" />
      </div>
      <div class="grid grid-cols-3 gap-2">
        <div class="col-span-2 flex flex-col gap-1">
          <label for="smtp-host" class="font-semibold text-foreground">Host *</label>
          <Input id="smtp-host" bind:value={smtpHost} placeholder="smtp.gmail.com" class="text-xs" />
        </div>
        <div class="flex flex-col gap-1">
          <label for="smtp-port" class="font-semibold text-foreground">Port</label>
          <Input id="smtp-port" type="number" bind:value={smtpPort} class="text-xs" />
        </div>
      </div>
      <div class="grid grid-cols-2 gap-2">
        <div class="flex flex-col gap-1">
          <label for="smtp-username" class="font-semibold text-foreground">Username</label>
          <Input id="smtp-username" bind:value={smtpUsername} placeholder="Username" class="text-xs" />
        </div>
        <div class="flex flex-col gap-1">
          <label for="smtp-password" class="font-semibold text-foreground">Password</label>
          <Input id="smtp-password" type="password" bind:value={smtpPassword} placeholder="Password" class="text-xs" />
        </div>
      </div>
      <div class="grid grid-cols-2 gap-2">
        <div class="flex flex-col gap-1">
          <label for="smtp-from-name" class="font-semibold text-foreground">From Name</label>
          <Input id="smtp-from-name" bind:value={smtpFromName} placeholder="Sales Team" class="text-xs" />
        </div>
        <div class="flex flex-col gap-1">
          <label for="smtp-from-email" class="font-semibold text-foreground">From Email</label>
          <Input id="smtp-from-email" bind:value={smtpFromEmail} placeholder="sales@company.com" class="text-xs" />
        </div>
      </div>

      <!-- Test Connection Button -->
      <div class="pt-2 border-t border-border flex items-center justify-between">
        <Button variant="outline" size="sm" onclick={handleTestSmtpConnection} disabled={smtpTesting || !smtpHost.trim()} class="h-8 text-xs gap-1">
          <RefreshCw class="size-3 {smtpTesting ? 'animate-spin' : ''}" /> Test Connection
        </Button>
        {#if smtpTestResult}
          <span class="text-[11px] font-medium {smtpTestResult.success ? 'text-emerald-600' : 'text-rose-500'}">
            {smtpTestResult.message}
          </span>
        {/if}
      </div>
    </div>
    <Dialog.Footer class="gap-2">
      <Button variant="outline" size="sm" onclick={() => isSmtpModalOpen = false}>Cancel</Button>
      <Button size="sm" onclick={handleSaveSmtpServer}>Save Server</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Email Template Dialog -->
<Dialog.Root bind:open={isTemplateModalOpen}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{editingTemplateId ? 'Edit Email Template' : 'Create Email Template'}</Dialog.Title>
    </Dialog.Header>
    <div class="flex flex-col gap-3 py-2 text-xs">
      {#if templateError}
        <div class="p-2 rounded bg-destructive/10 text-destructive text-xs">{templateError}</div>
      {/if}
      <div class="flex flex-col gap-1">
        <label for="tpl-name-email" class="font-semibold text-foreground">Template Name *</label>
        <Input id="tpl-name-email" bind:value={templateName} placeholder="e.g. Intro Sequence" class="text-xs" />
      </div>
      <div class="flex flex-col gap-1">
        <label for="tpl-subject-email" class="font-semibold text-foreground">Subject Line *</label>
        <Input id="tpl-subject-email" bind:value={templateSubject} placeholder="e.g. Partnership Proposal" class="text-xs" onfocus={() => activeInputField = 'subject'} />
      </div>

      <!-- Variable Insertion Badges -->
      <div class="flex flex-col gap-1">
        <span class="text-[11px] text-muted-foreground">Click to insert variable into active field ({activeInputField}):</span>
        <div class="flex flex-wrap gap-1">
          <Badge variant="outline" class="cursor-pointer hover:bg-accent text-[10px]" onclick={() => insertVariableBadge('{{name}}')}>+ {'{{name}}'}</Badge>
          <Badge variant="outline" class="cursor-pointer hover:bg-accent text-[10px]" onclick={() => insertVariableBadge('{{company}}')}>+ {'{{company}}'}</Badge>
          <Badge variant="outline" class="cursor-pointer hover:bg-accent text-[10px]" onclick={() => insertVariableBadge('{{email}}')}>+ {'{{email}}'}</Badge>
          <Badge variant="outline" class="cursor-pointer hover:bg-accent text-[10px]" onclick={() => insertVariableBadge('{{phone}}')}>+ {'{{phone}}'}</Badge>
        </div>
      </div>

      <div class="flex flex-col gap-1">
        <label for="tpl-content-email" class="font-semibold text-foreground">Content</label>
        <textarea id="tpl-content-email" rows="6" bind:value={templateContent} placeholder="Hi {{name}}..." class="w-full rounded-md border border-input bg-background p-2 text-xs" onfocus={() => activeInputField = 'content'}></textarea>
      </div>
    </div>
    <Dialog.Footer class="gap-2">
      <Button variant="outline" size="sm" onclick={() => isTemplateModalOpen = false}>Cancel</Button>
      <Button size="sm" onclick={handleSaveTemplate}>Save Server</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
