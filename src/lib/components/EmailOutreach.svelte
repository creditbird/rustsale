<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getLeads, getCategories, getCampaignLeads, type Lead, type Category } from '$lib/services/crmService';

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

  // Reactive states
  let smtpServers = $state<SmtpServer[]>([]);
  let templates = $state<EmailTemplate[]>([]);
  let leads = $state<Lead[]>([]);
  let categories = $state<Category[]>([]);

  let selectedSmtpId = $state<number | null>(null);
  let selectedTemplateId = $state<number | null>(null);
  let selectedLeadIds = $state<number[]>([]);

  // Category and Segmentation filters
  let selectedCategoryId = $state<number | null>(null);
  let segmentationFilter = $state('all');
  let filterTemplateId = $state<number | null>(null);
  let leadSearchQuery = $state('');

  // SMTP Server Modal states
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

  // Email Template Modal states
  let isTemplateModalOpen = $state(false);
  let editingTemplateId = $state<number | null>(null);
  let templateName = $state('');
  let templateSubject = $state('');
  let templateContent = $state('');
  let templateError = $state<string | null>(null);
  let activeInputField = $state<'subject' | 'content'>('content');

  // Campaign execution states
  let isCampaignRunning = $state(false);
  let campaignProgress = $state(0);
  let campaignStatus = $state('Idle');
  let campaignLogs = $state<string[]>([]);
  let campaignConsoleElement: HTMLDivElement | null = null;

  // Tauri event listeners
  let campaignUnlistens: UnlistenFn[] = [];

  let isButtonDisabled = $derived(
    isCampaignRunning || !selectedSmtpId || !selectedTemplateId || selectedLeadIds.length === 0
  );

  let filteredLeads = $derived(
    leads.filter(lead =>
      (lead.name || '').toLowerCase().includes(leadSearchQuery.toLowerCase()) ||
      (lead.company || '').toLowerCase().includes(leadSearchQuery.toLowerCase()) ||
      (lead.email || '').toLowerCase().includes(leadSearchQuery.toLowerCase()) ||
      (lead.phone || '').includes(leadSearchQuery)
    )
  );

  let isAllLeadsSelected = $derived(
    filteredLeads.length > 0 &&
    filteredLeads.every(lead => lead.id && selectedLeadIds.includes(lead.id))
  );

  let selectedSmtpServer = $derived(
    smtpServers.find(s => s.id === selectedSmtpId) || null
  );

  let selectedTemplate = $derived(
    templates.find(t => t.id === selectedTemplateId) || null
  );

  async function loadSmtpServers(): Promise<void> {
    try {
      const res = await invoke<SmtpServer[]>('get_smtp_servers');
      smtpServers = [...res];
      if (smtpServers.length > 0 && selectedSmtpId === null) {
        selectedSmtpId = smtpServers[0].id || null;
      }
    } catch (e) {
      console.error('Failed to load SMTP servers:', e);
    }
  }

  async function loadTemplates(): Promise<void> {
    try {
      const res = await invoke<EmailTemplate[]>('get_email_templates');
      templates = [...res];
      if (templates.length > 0 && selectedTemplateId === null) {
        selectedTemplateId = templates[0].id || null;
      }
    } catch (e) {
      console.error('Failed to load email templates:', e);
    }
  }

  async function loadLeadsData(): Promise<void> {
    try {
      leads = await getLeads();
      selectedLeadIds = leads.filter(l => l.id != null).map(l => l.id as number);
    } catch (e) {
      console.error('Error loading leads:', e);
    }
  }

  async function loadCategories(): Promise<void> {
    try {
      categories = await getCategories();
    } catch (e) {
      console.error('Error loading categories:', e);
    }
  }

  $effect(() => {
    if (selectedCategoryId !== null && selectedCategoryId !== undefined && (selectedCategoryId as any) !== 'null' && (selectedCategoryId as any) !== '') {
      getCampaignLeads(Number(selectedCategoryId), segmentationFilter, filterTemplateId)
        .then(res => {
          leads = res;
          selectedLeadIds = leads.filter(l => l.id != null).map(l => l.id as number);
        })
        .catch(err => {
          console.error('Error fetching campaign leads:', err);
          leads = [];
        });
    }
  });

  onMount(async () => {
    loadCategories();
    await loadSmtpServers();
    await loadTemplates();
    await loadLeadsData();
  });

  function cleanupCampaignListeners() {
    campaignUnlistens.forEach(u => u());
    campaignUnlistens = [];
  }

  // SMTP Server Actions
  function openAddSmtpModal() {
    editingSmtpId = null;
    smtpName = '';
    smtpHost = '';
    smtpPort = 587;
    smtpUsername = '';
    smtpPassword = '';
    smtpFromEmail = '';
    smtpFromName = '';
    smtpSecurity = 'STARTTLS';
    smtpTestResult = null;
    smtpError = null;
    isSmtpModalOpen = true;
  }

  function openEditSmtpModal(server: SmtpServer) {
    editingSmtpId = server.id || null;
    smtpName = server.name;
    smtpHost = server.host;
    smtpPort = server.port;
    smtpUsername = server.username;
    smtpPassword = server.password;
    smtpFromEmail = server.from_email;
    smtpFromName = server.from_name;
    smtpSecurity = server.security;
    smtpTestResult = null;
    smtpError = null;
    isSmtpModalOpen = true;
  }

  function closeSmtpModal() {
    isSmtpModalOpen = false;
    smtpError = null;
    smtpTestResult = null;
  }

  async function handleTestSmtpConnection() {
    smtpTesting = true;
    smtpTestResult = null;
    smtpError = null;
    try {
      const res = await invoke<string>('test_smtp_connection', {
        host: smtpHost,
        port: Number(smtpPort)
      });
      smtpTestResult = { success: true, message: res };
    } catch (e) {
      smtpTestResult = { success: false, message: String(e) };
    } finally {
      smtpTesting = false;
    }
  }

  async function handleSaveSmtpServer() {
    if (!smtpName.trim()) {
      smtpError = 'Server Name is required.';
      console.log('HANDLE SAVE RETURNED EARLY AT NAME:', smtpError);
      return;
    }
    if (!smtpHost.trim()) {
      smtpError = 'SMTP Host is required.';
      console.log('HANDLE SAVE RETURNED EARLY AT HOST:', smtpError);
      return;
    }

    try {
      const serverPayload: SmtpServer = {
        id: editingSmtpId || undefined,
        name: smtpName.trim(),
        host: smtpHost.trim(),
        port: Number(smtpPort),
        username: smtpUsername.trim(),
        password: smtpPassword,
        from_email: smtpFromEmail.trim(),
        from_name: smtpFromName.trim(),
        security: smtpSecurity,
        created_at: new Date().toISOString()
      };

      const savedId = await invoke<number>('save_smtp_server', { server: serverPayload });
      selectedSmtpId = savedId || editingSmtpId || null;
      await loadSmtpServers();
      closeSmtpModal();
    } catch (e) {
      console.error('handleSaveSmtpServer CAUGHT ERROR:', e);
      smtpError = String(e);
    }
  }

  async function handleDeleteSmtpServer(id: number) {
    if (!confirm('Are you sure you want to delete this SMTP server configuration?')) return;
    try {
      await invoke('delete_smtp_server', { id });
      if (selectedSmtpId === id) selectedSmtpId = null;
      await loadSmtpServers();
    } catch (e) {
      alert(`Error deleting SMTP server: ${e}`);
    }
  }

  // Template Actions
  function openAddTemplateModal() {
    editingTemplateId = null;
    templateName = '';
    templateSubject = '';
    templateContent = '';
    templateError = null;
    activeInputField = 'content';
    isTemplateModalOpen = true;
  }

  function openEditTemplateModal(tpl: EmailTemplate) {
    editingTemplateId = tpl.id || null;
    templateName = tpl.name;
    templateSubject = tpl.subject;
    templateContent = tpl.content;
    templateError = null;
    activeInputField = 'content';
    isTemplateModalOpen = true;
  }

  function closeTemplateModal() {
    isTemplateModalOpen = false;
  }

  function insertVariableBadge(variableTag: string) {
    if (activeInputField === 'subject') {
      templateSubject += variableTag;
    } else {
      templateContent += variableTag;
    }
  }

  async function handleSaveTemplate() {
    if (!templateName.trim()) {
      templateError = 'Template Name is required.';
      return;
    }
    if (!templateSubject.trim()) {
      templateError = 'Subject line is required.';
      return;
    }

    try {
      if (editingTemplateId) {
        await invoke('update_email_template', {
          id: editingTemplateId,
          name: templateName.trim(),
          subject: templateSubject.trim(),
          content: templateContent
        });
      } else {
        const savedId = await invoke<number>('create_email_template', {
          name: templateName.trim(),
          subject: templateSubject.trim(),
          content: templateContent
        });
        selectedTemplateId = savedId;
      }
      await loadTemplates();
      closeTemplateModal();
    } catch (e) {
      templateError = String(e);
    }
  }

  async function handleDeleteTemplate(id: number) {
    if (!confirm('Are you sure you want to delete this email template?')) return;
    try {
      await invoke('delete_email_template', { id });
      if (selectedTemplateId === id) selectedTemplateId = null;
      await loadTemplates();
    } catch (e) {
      alert(`Error deleting template: ${e}`);
    }
  }

  // Lead selection functions
  function toggleLeadSelection(leadId: number) {
    if (selectedLeadIds.includes(leadId)) {
      selectedLeadIds = selectedLeadIds.filter(id => id !== leadId);
    } else {
      selectedLeadIds = [...selectedLeadIds, leadId];
    }
  }

  function toggleSelectAll() {
    if (isAllLeadsSelected) {
      const filteredIds = new Set(filteredLeads.map(l => l.id));
      selectedLeadIds = selectedLeadIds.filter(id => !filteredIds.has(id));
    } else {
      const newIds = new Set(selectedLeadIds);
      filteredLeads.forEach(l => {
        if (l.id != null) newIds.add(l.id);
      });
      selectedLeadIds = Array.from(newIds);
    }
  }

  function appendCampaignLog(msg: string) {
    const timestamp = new Date().toLocaleTimeString();
    campaignLogs = [...campaignLogs, `[${timestamp}] ${msg}`];
    console.log('CAMPAIGN LOGS IN SVELTE IS NOW:', campaignLogs);
    setTimeout(() => {
      if (campaignConsoleElement) {
        campaignConsoleElement.scrollTop = campaignConsoleElement.scrollHeight;
      }
    }, 50);
  }

  async function startCampaign() {
    if (!selectedSmtpId) {
      alert('Please select an active SMTP server configuration first.');
      return;
    }
    if (!selectedTemplateId) {
      alert('Please select an email template first.');
      return;
    }
    if (selectedLeadIds.length === 0) {
      alert('Please select at least one lead to receive the email outreach.');
      return;
    }

    isCampaignRunning = true;
    campaignProgress = 0;
    campaignStatus = 'Starting email campaign...';
    campaignLogs = [];
    appendCampaignLog(`Starting email outreach campaign to ${selectedLeadIds.length} leads...`);

    cleanupCampaignListeners();

    try {
      const [progressUnlisten, logUnlisten, completeUnlisten] = await Promise.all([
        listen<{ progress: number; status: string; message: string }>(
          'email-outreach-progress',
          (event) => {
            campaignProgress = event.payload.progress;
            campaignStatus = event.payload.message;
            if (event.payload.status === 'failed') {
              isCampaignRunning = false;
              cleanupCampaignListeners();
            }
          }
        ),
        listen<{ message: string }>('email-outreach-log', (event) => {
          appendCampaignLog(event.payload.message);
        }),
        listen<{ total_sent: number; message: string }>(
          'email-outreach-complete',
          (event) => {
            isCampaignRunning = false;
            campaignProgress = 100;
            campaignStatus = event.payload.message;
            appendCampaignLog(`[SUCCESS] ${event.payload.message}`);
            cleanupCampaignListeners();
          }
        )
      ]);
      campaignUnlistens.push(progressUnlisten, logUnlisten, completeUnlisten);

      await invoke('send_email_outreach', {
        smtp_id: selectedSmtpId,
        template_id: selectedTemplateId,
        lead_ids: selectedLeadIds
      });
      appendCampaignLog('Email outreach process invoked successfully.');
    } catch (e) {
      console.error('Error starting email campaign:', e);
      isCampaignRunning = false;
      campaignStatus = 'Failed';
      appendCampaignLog(`Error starting email campaign: ${e}`);
    }
  }
</script>

<div class="container animate-fade-in">
  <header class="header">
    <div class="title-section">
      <span class="material-icons outreach-icon">mark_email_unread</span>
      <div>
        <h1>Email Outreach Manager</h1>
        <p class="subtitle">Configure SMTP servers, design custom email templates, and launch email campaigns</p>
      </div>
    </div>
  </header>

  <div class="layout-grid">
    <!-- Left Column: SMTP Configuration & Email Templates -->
    <div class="left-col">
      <!-- SMTP Configuration Card -->
      <div class="card section-card">
        <div class="card-header">
          <h2>SMTP Configuration</h2>
          <button type="button" class="action-btn primary-action-btn" onclick={openAddSmtpModal}>
            <span class="material-icons">add</span>
            Add SMTP Server
          </button>
        </div>

        <div class="card-body">
          {#if smtpServers.length === 0}
            <div class="empty-state">
              <span class="material-icons">dns</span>
              <p>No SMTP servers configured yet. Click "Add SMTP Server" to create one.</p>
            </div>
          {:else}
            <div class="item-list">
              {#each smtpServers as server (server.id)}
                <div
                  class="item-card"
                  class:selected={selectedSmtpId === server.id}
                  onclick={() => selectedSmtpId = server.id || null}
                  role="button"
                  tabindex="0"
                  onkeydown={(e) => e.key === 'Enter' && (selectedSmtpId = server.id || null)}
                >
                  <div class="item-main">
                    <div class="item-title-row">
                      <span class="item-name">{server.name}</span>
                      <span class="badge security-badge">{server.security}</span>
                    </div>
                    <div class="item-meta">
                      <span><strong>Host:</strong> {server.host}:{server.port}</span>
                      <span><strong>From:</strong> {server.from_name ? `${server.from_name} <${server.from_email}>` : server.from_email}</span>
                    </div>
                  </div>
                  <div class="item-actions" onclick={(e) => e.stopPropagation()} role="group" aria-label="Server options">
                    <button
                      type="button"
                      class="icon-btn"
                      title="Edit Server"
                      onclick={() => openEditSmtpModal(server)}
                    >
                      <span class="material-icons">edit</span>
                    </button>
                    <button
                      type="button"
                      class="icon-btn danger"
                      title="Delete Server"
                      onclick={() => server.id && handleDeleteSmtpServer(server.id)}
                    >
                      <span class="material-icons">delete</span>
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>

      <!-- Email Templates Card -->
      <div class="card section-card">
        <div class="card-header">
          <h2>Email Templates</h2>
          <button type="button" class="action-btn primary-action-btn" onclick={openAddTemplateModal}>
            <span class="material-icons">add</span>
            Create Template
          </button>
        </div>

        <div class="card-body">
          {#if templates.length === 0}
            <div class="empty-state">
              <span class="material-icons">description</span>
              <p>No email templates found. Click "Create Template" to get started.</p>
            </div>
          {:else}
            <div class="item-list">
              {#each templates as tpl (tpl.id)}
                <div
                  class="item-card"
                  class:selected={selectedTemplateId === tpl.id}
                  onclick={() => selectedTemplateId = tpl.id || null}
                  role="button"
                  tabindex="0"
                  onkeydown={(e) => e.key === 'Enter' && (selectedTemplateId = tpl.id || null)}
                >
                  <div class="item-main">
                    <div class="item-title-row">
                      <span class="item-name">{tpl.name}</span>
                    </div>
                    <div class="item-subject">
                      <strong>Subject:</strong> {tpl.subject}
                    </div>
                    <div class="item-preview">
                      {tpl.content.slice(0, 100)}{tpl.content.length > 100 ? '...' : ''}
                    </div>
                  </div>
                  <div class="item-actions" onclick={(e) => e.stopPropagation()} role="group" aria-label="Template options">
                    <button
                      type="button"
                      class="icon-btn"
                      title="Edit Template"
                      onclick={() => openEditTemplateModal(tpl)}
                    >
                      <span class="material-icons">edit</span>
                    </button>
                    <button
                      type="button"
                      class="icon-btn danger"
                      title="Delete Template"
                      onclick={() => tpl.id && handleDeleteTemplate(tpl.id)}
                    >
                      <span class="material-icons">delete</span>
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    </div>

    <!-- Right Column: Lead Selection & Live Console -->
    <div class="right-col">
      <!-- Target Lead Selection Card -->
      <div class="card section-card">
        <div class="card-header">
          <h2>Target Lead Selection</h2>
          <span class="selected-count-badge">
            {selectedLeadIds.length} / {leads.length} Leads Selected
          </span>
        </div>

        <div class="card-body">
          <!-- Filters Toolbar -->
          <div class="filters-toolbar">
            <div class="filter-group">
              <label for="category-select">Category:</label>
              <select id="category-select" bind:value={selectedCategoryId}>
                <option value={null}>All Categories</option>
                {#each categories as cat (cat.id)}
                  <option value={cat.id}>{cat.name}</option>
                {/each}
              </select>
            </div>

            <div class="filter-group">
              <label for="segmentation-select">Filter:</label>
              <select id="segmentation-select" bind:value={segmentationFilter}>
                <option value="all">All Leads</option>
                <option value="not_reached">Not Reached</option>
                <option value="reached">Reached</option>
              </select>
            </div>

            <div class="search-group">
              <span class="material-icons search-icon">search</span>
              <input
                type="text"
                placeholder="Search leads..."
                bind:value={leadSearchQuery}
              />
            </div>
          </div>

          <!-- Leads Table -->
          <div class="table-container">
            <table class="leads-table">
              <thead>
                <tr>
                  <th style="width: 40px; text-align: center;">
                    <input
                      type="checkbox"
                      aria-label="Select all leads"
                      checked={isAllLeadsSelected}
                      onclick={toggleSelectAll}
                    />
                  </th>
                  <th>Name</th>
                  <th>Company</th>
                  <th>Email</th>
                  <th>Phone</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {#if filteredLeads.length === 0}
                  <tr>
                    <td colspan="6" class="empty-table-cell">No leads match the selected criteria.</td>
                  </tr>
                {:else}
                  {#each filteredLeads as lead (lead.id)}
                    <tr class:selected-row={lead.id && selectedLeadIds.includes(lead.id)}>
                      <td style="text-align: center;">
                        <input
                          type="checkbox"
                          aria-label={`Select ${lead.name}`}
                          checked={lead.id ? selectedLeadIds.includes(lead.id) : false}
                          onclick={() => lead.id && toggleLeadSelection(lead.id)}
                        />
                      </td>
                      <td class="cell-primary">{lead.name}</td>
                      <td>{lead.company}</td>
                      <td>{lead.email || '-'}</td>
                      <td>{lead.phone || '-'}</td>
                      <td><span class="status-badge">{lead.status}</span></td>
                    </tr>
                  {/each}
                {/if}
              </tbody>
            </table>
          </div>
        </div>
      </div>

      <!-- Live Execution Console Card -->
      <div class="card section-card console-card">
        <div class="card-header">
          <h2>Live Execution Console</h2>
          <button
            type="button"
            class="action-btn campaign-btn"
            disabled={(isCampaignRunning || !selectedSmtpId || !selectedTemplateId || selectedLeadIds.length === 0) || undefined}
            onclick={startCampaign}
          >
            <span class="material-icons" class:spinning={isCampaignRunning}>send</span>
            {isCampaignRunning ? 'Running Campaign...' : 'Start Email Campaign'}
          </button>
        </div>

        <div class="card-body">
          <div class="campaign-summary-bar">
            <div class="summary-item">
              <span class="summary-label">Selected SMTP:</span>
              <span class="summary-value">{selectedSmtpServer ? selectedSmtpServer.name : 'None'}</span>
            </div>
            <div class="summary-item">
              <span class="summary-label">Selected Template:</span>
              <span class="summary-value">{selectedTemplate ? selectedTemplate.name : 'None'}</span>
            </div>
            <div class="summary-item">
              <span class="summary-label">Status:</span>
              <span class="summary-value status-text">{campaignStatus}</span>
            </div>
          </div>

          <!-- Progress Bar -->
          <div class="progress-bar-container">
            <div class="progress-bar-fill" style="width: {campaignProgress}%;"></div>
            <span class="progress-text">{Math.round(campaignProgress)}%</span>
          </div>

          <!-- Log Terminal Output -->
          <div class="terminal-container" bind:this={campaignConsoleElement}>
            {#if campaignLogs.length === 0}
              <div class="terminal-placeholder">
                <span class="material-icons">terminal</span>
                <span>Console output ready. Click "Start Email Campaign" to begin.</span>
              </div>
            {:else}
              {#each campaignLogs as logLine}
                <div class="log-line">{logLine}</div>
              {/each}
            {/if}
          </div>
        </div>
      </div>
    </div>
  </div>
</div>

<!-- SMTP Server Modal -->
{#if isSmtpModalOpen}
  <div class="modal-backdrop animate-fade-in" onclick={(e) => e.target === e.currentTarget && closeSmtpModal()} role="dialog" aria-modal="true" aria-labelledby="smtp-modal-heading">
    <div class="modal-card animate-scale-up" role="document">
      <div class="modal-header">
        <h3 id="smtp-modal-heading">{editingSmtpId ? 'Edit SMTP Server' : 'Add SMTP Server'}</h3>
        <button type="button" class="close-btn" onclick={closeSmtpModal}>
          <span class="material-icons">close</span>
        </button>
      </div>

        <div class="modal-body form-grid">
          {#if smtpError}
            <div class="modal-error-banner">{smtpError}</div>
          {/if}

          <div class="form-group full-width">
            <label for="smtp-name">Server Name</label>
            <input
              id="smtp-name"
              type="text"
              placeholder="e.g. Primary Gmail SMTP"
              bind:value={smtpName}
            />
          </div>

          <div class="form-group half-width">
            <label for="smtp-host">SMTP Host</label>
            <input
              id="smtp-host"
              type="text"
              placeholder="smtp.gmail.com"
              bind:value={smtpHost}
            />
          </div>

          <div class="form-group half-width">
            <label for="smtp-port">SMTP Port</label>
            <input
              id="smtp-port"
              type="number"
              placeholder="587"
              bind:value={smtpPort}
            />
          </div>

          <div class="form-group half-width">
            <label for="smtp-username">Username</label>
            <input
              id="smtp-username"
              type="text"
              placeholder="user@gmail.com"
              bind:value={smtpUsername}
            />
          </div>

          <div class="form-group half-width">
            <label for="smtp-password">Password</label>
            <input
              id="smtp-password"
              type="password"
              placeholder="••••••••"
              bind:value={smtpPassword}
            />
          </div>

          <div class="form-group half-width">
            <label for="smtp-from-email">From Email</label>
            <input
              id="smtp-from-email"
              type="email"
              placeholder="sender@domain.com"
              bind:value={smtpFromEmail}
            />
          </div>

          <div class="form-group half-width">
            <label for="smtp-from-name">From Name</label>
            <input
              id="smtp-from-name"
              type="text"
              placeholder="Sales Team"
              bind:value={smtpFromName}
            />
          </div>

          <div class="form-group full-width">
            <label for="smtp-security">Security Protocol</label>
            <select id="smtp-security" bind:value={smtpSecurity}>
              <option value="STARTTLS">STARTTLS (Port 587 / 25)</option>
              <option value="SSL_TLS">SSL / TLS (Port 465)</option>
              <option value="NONE">None (Unencrypted)</option>
            </select>
          </div>

          {#if smtpTestResult}
            <div class="test-result-banner" class:success={smtpTestResult.success} class:failure={!smtpTestResult.success}>
              {smtpTestResult.message}
            </div>
          {/if}
        </div>

        <div class="modal-footer">
          <button
            type="button"
            class="secondary-btn"
            disabled={smtpTesting}
            onclick={handleTestSmtpConnection}
          >
            <span class="material-icons" class:spinning={smtpTesting}>sync_alt</span>
            {smtpTesting ? 'Testing...' : 'Test Connection'}
          </button>

          <button type="button" class="secondary-btn" onclick={closeSmtpModal}>Cancel</button>

          <button type="button" class="primary-btn" onclick={handleSaveSmtpServer}>Save Server</button>
        </div>
    </div>
  </div>
{/if}

<!-- Email Template Modal -->
{#if isTemplateModalOpen}
  <div class="modal-backdrop animate-fade-in" onclick={(e) => e.target === e.currentTarget && closeTemplateModal()} role="dialog" aria-modal="true" aria-labelledby="tpl-modal-heading">
    <div class="modal-card animate-scale-up" role="document">
      <div class="modal-header">
        <h3 id="tpl-modal-heading">{editingTemplateId ? 'Edit Email Template' : 'Create Email Template'}</h3>
        <button type="button" class="close-btn" onclick={closeTemplateModal}>
          <span class="material-icons">close</span>
        </button>
      </div>

        <div class="modal-body form-grid">
          {#if templateError}
            <div class="modal-error-banner">{templateError}</div>
          {/if}

          <div class="form-group full-width">
            <label for="template-name">Template Name</label>
            <input
              id="template-name"
              type="text"
              placeholder="e.g. Introduction Sequence #1"
              bind:value={templateName}
            />
          </div>

          <div class="form-group full-width">
            <label for="template-subject">Subject Line</label>
            <input
              id="template-subject"
              type="text"
              placeholder={'e.g. Proposal for {{company}}'}
              bind:value={templateSubject}
              onfocus={() => activeInputField = 'subject'}
            />
          </div>

          <!-- Variable Badges -->
          <div class="form-group full-width">
            <div class="badge-label-row">
              <span class="helper-text">Click to insert variable into active field ({activeInputField}):</span>
              <div class="variable-badges">
                <button
                  type="button"
                  class="var-badge"
                  onclick={() => insertVariableBadge('{{name}}')}
                >
                  + {'{{name}}'}
                </button>
                <button
                  type="button"
                  class="var-badge"
                  onclick={() => insertVariableBadge('{{company}}')}
                >
                  + {'{{company}}'}
                </button>
                <button
                  type="button"
                  class="var-badge"
                  onclick={() => insertVariableBadge('{{email}}')}
                >
                  + {'{{email}}'}
                </button>
                <button
                  type="button"
                  class="var-badge"
                  onclick={() => insertVariableBadge('{{phone}}')}
                >
                  + {'{{phone}}'}
                </button>
              </div>
            </div>
          </div>

          <div class="form-group full-width">
            <label for="template-content">Email Content</label>
            <textarea
              id="template-content"
              rows="6"
              placeholder={'Hello {{name}}, I would like to offer...'}
              bind:value={templateContent}
              onfocus={() => activeInputField = 'content'}
            ></textarea>
          </div>
        </div>

        <div class="modal-footer">
          <button type="button" class="secondary-btn" onclick={closeTemplateModal}>Cancel</button>
          <button type="button" class="primary-btn" onclick={handleSaveTemplate}>Save Template</button>
        </div>
    </div>
  </div>
{/if}

<style>
  :global(:root) {
    --spacing-xs: 4px;
    --spacing-sm: 8px;
    --spacing-md: 16px;
    --spacing-lg: 24px;
    --color-surface-container-lowest: rgba(15, 23, 42, 0.6);
    --color-surface-container-low: rgba(30, 41, 59, 0.7);
    --color-surface-container: rgba(30, 41, 59, 0.9);
    --color-on-surface: #f8fafc;
    --color-on-surface-variant: #94a3b8;
    --color-outline: rgba(255, 255, 255, 0.1);
    --color-primary: #3b82f6;
    --color-success: #10b981;
    --color-danger: #ef4444;
    --rounded-md: 8px;
    --rounded-lg: 12px;
    --rounded-full: 9999px;
    --transition-fast: all 0.15s ease-in-out;
  }

  .container {
    padding: var(--spacing-lg);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-lg);
    color: var(--color-on-surface);
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .title-section {
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
  }

  .outreach-icon {
    font-size: 36px;
    color: var(--color-primary);
    background: rgba(59, 130, 246, 0.12);
    padding: var(--spacing-sm);
    border-radius: var(--rounded-lg);
    border: 1px solid rgba(59, 130, 246, 0.2);
  }

  h1 {
    font-size: 24px;
    font-weight: 700;
    margin: 0;
  }

  .subtitle {
    font-size: 13px;
    color: var(--color-on-surface-variant);
    margin: 2px 0 0 0;
  }

  .layout-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--spacing-lg);
  }

  @media (max-width: 1024px) {
    .layout-grid {
      grid-template-columns: 1fr;
    }
  }

  .left-col, .right-col {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-lg);
  }

  /* Cards & Glassmorphism */
  .card {
    background: var(--color-surface-container-lowest);
    backdrop-filter: blur(12px);
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-lg);
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .card-header {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-outline);
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: var(--color-surface-container-low);
  }

  .card-header h2 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
  }

  .card-body {
    padding: var(--spacing-md);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--spacing-xs);
    font-size: 13px;
    font-weight: 600;
    padding: 6px 12px;
    border-radius: var(--rounded-md);
    border: none;
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .primary-action-btn {
    background: var(--color-primary);
    color: #ffffff;
  }

  .primary-action-btn:hover {
    background: #2563eb;
  }

  .campaign-btn {
    background: var(--color-success);
    color: #ffffff;
    padding: 8px 16px;
    font-size: 14px;
  }

  .campaign-btn:hover:not(:disabled) {
    background: #059669;
  }

  .campaign-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* List & Items */
  .empty-state {
    text-align: center;
    padding: var(--spacing-lg);
    color: var(--color-on-surface-variant);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--spacing-xs);
  }

  .empty-state .material-icons {
    font-size: 32px;
    opacity: 0.5;
  }

  .item-list {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
  }

  .item-card {
    background: var(--color-surface-container-low);
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-md);
    padding: var(--spacing-md);
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .item-card:hover {
    border-color: var(--color-primary);
    background: var(--color-surface-container);
  }

  .item-card.selected {
    border-color: var(--color-primary);
    box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.3);
    background: rgba(59, 130, 246, 0.08);
  }

  .item-main {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
    flex: 1;
  }

  .item-title-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
  }

  .item-name {
    font-weight: 600;
    font-size: 14px;
  }

  .badge {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 6px;
    border-radius: var(--rounded-full);
    text-transform: uppercase;
  }

  .security-badge {
    background: rgba(59, 130, 246, 0.2);
    color: #60a5fa;
    border: 1px solid rgba(59, 130, 246, 0.3);
  }

  .item-meta {
    font-size: 12px;
    color: var(--color-on-surface-variant);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .item-subject {
    font-size: 12px;
    color: var(--color-on-surface);
  }

  .item-preview {
    font-size: 11px;
    color: var(--color-on-surface-variant);
    font-style: italic;
  }

  .item-actions {
    display: flex;
    gap: var(--spacing-xs);
  }

  .icon-btn {
    background: transparent;
    border: none;
    color: var(--color-on-surface-variant);
    padding: 4px;
    border-radius: 4px;
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .icon-btn:hover {
    color: var(--color-on-surface);
    background: rgba(255, 255, 255, 0.1);
  }

  .icon-btn.danger:hover {
    color: var(--color-danger);
    background: rgba(239, 68, 68, 0.1);
  }

  /* Filters Toolbar */
  .filters-toolbar {
    display: flex;
    gap: var(--spacing-md);
    flex-wrap: wrap;
    align-items: center;
  }

  .filter-group {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
    font-size: 13px;
  }

  .filter-group select {
    background: var(--color-surface-container-low);
    color: var(--color-on-surface);
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-md);
    padding: 4px 8px;
    font-size: 13px;
    outline: none;
  }

  .search-group {
    display: flex;
    align-items: center;
    background: var(--color-surface-container-low);
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-md);
    padding: 4px 8px;
    flex: 1;
    min-width: 160px;
  }

  .search-icon {
    font-size: 18px;
    color: var(--color-on-surface-variant);
    margin-right: 4px;
  }

  .search-group input {
    background: transparent;
    border: none;
    color: var(--color-on-surface);
    font-size: 13px;
    outline: none;
    width: 100%;
  }

  .selected-count-badge {
    font-size: 12px;
    background: rgba(16, 185, 129, 0.15);
    color: #34d399;
    padding: 2px 8px;
    border-radius: var(--rounded-full);
    font-weight: 500;
  }

  /* Table */
  .table-container {
    overflow-x: auto;
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-md);
    max-height: 280px;
    overflow-y: auto;
  }

  .leads-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    text-align: left;
  }

  .leads-table th {
    background: var(--color-surface-container-low);
    color: var(--color-on-surface-variant);
    padding: 8px 12px;
    font-weight: 600;
    position: sticky;
    top: 0;
    z-index: 1;
    border-bottom: 1px solid var(--color-outline);
  }

  .leads-table td {
    padding: 8px 12px;
    border-bottom: 1px solid var(--color-outline);
  }

  .leads-table tr:hover {
    background: rgba(255, 255, 255, 0.03);
  }

  .leads-table tr.selected-row {
    background: rgba(59, 130, 246, 0.08);
  }

  .cell-primary {
    font-weight: 600;
  }

  .status-badge {
    font-size: 11px;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.08);
  }

  .empty-table-cell {
    text-align: center;
    color: var(--color-on-surface-variant);
    padding: var(--spacing-lg);
  }

  /* Console & Logs */
  .campaign-summary-bar {
    display: flex;
    gap: var(--spacing-md);
    flex-wrap: wrap;
    background: var(--color-surface-container-low);
    padding: var(--spacing-sm) var(--spacing-md);
    border-radius: var(--rounded-md);
    font-size: 13px;
  }

  .summary-item {
    display: flex;
    gap: 4px;
    align-items: center;
  }

  .summary-label {
    color: var(--color-on-surface-variant);
  }

  .summary-value {
    font-weight: 600;
  }

  .progress-bar-container {
    height: 18px;
    background: var(--color-surface-container-low);
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-full);
    overflow: hidden;
    position: relative;
  }

  .progress-bar-fill {
    height: 100%;
    background: linear-gradient(90deg, #3b82f6, #10b981);
    transition: width 0.3s ease;
  }

  .progress-text {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    font-size: 10px;
    font-weight: 700;
    color: #ffffff;
    text-shadow: 0 1px 2px rgba(0,0,0,0.6);
  }

  .terminal-container {
    background: #090d16;
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-md);
    padding: var(--spacing-md);
    font-family: 'Fira Code', monospace, monospace;
    font-size: 12px;
    color: #10b981;
    min-height: 120px;
    max-height: 200px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .terminal-placeholder {
    color: var(--color-on-surface-variant);
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
    height: 100%;
    justify-content: center;
    margin-top: 20px;
  }

  .log-line {
    white-space: pre-wrap;
    word-break: break-all;
  }

  /* Modals */
  .modal-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.65);
    backdrop-filter: blur(4px);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 1000;
  }

  .modal-card {
    background: #1e293b;
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-lg);
    width: 90%;
    max-width: 560px;
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .modal-header {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-outline);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .modal-header h3 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }

  .close-btn {
    background: transparent;
    border: none;
    color: var(--color-on-surface-variant);
    cursor: pointer;
  }

  .modal-body {
    padding: var(--spacing-md);
    max-height: 70vh;
    overflow-y: auto;
  }

  .form-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--spacing-md);
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
  }

  .form-group.full-width {
    grid-column: span 2;
  }

  .form-group.half-width {
    grid-column: span 1;
  }

  .form-group label {
    font-size: 12px;
    font-weight: 600;
    color: var(--color-on-surface-variant);
  }

  .form-group input, .form-group select, .form-group textarea {
    background: var(--color-surface-container-lowest);
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-md);
    padding: 8px 12px;
    color: var(--color-on-surface);
    font-size: 13px;
    outline: none;
  }

  .form-group input:focus, .form-group select:focus, .form-group textarea:focus {
    border-color: var(--color-primary);
  }

  .badge-label-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .helper-text {
    font-size: 11px;
    color: var(--color-on-surface-variant);
  }

  .variable-badges {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .var-badge {
    background: rgba(59, 130, 246, 0.15);
    color: #60a5fa;
    border: 1px solid rgba(59, 130, 246, 0.3);
    border-radius: var(--rounded-full);
    padding: 2px 8px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .var-badge:hover {
    background: rgba(59, 130, 246, 0.3);
  }

  .modal-error-banner {
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid var(--color-danger);
    color: #fca5a5;
    padding: var(--spacing-sm) var(--spacing-md);
    border-radius: var(--rounded-md);
    font-size: 12px;
    grid-column: span 2;
  }

  .test-result-banner {
    grid-column: span 2;
    padding: var(--spacing-sm) var(--spacing-md);
    border-radius: var(--rounded-md);
    font-size: 12px;
  }

  .test-result-banner.success {
    background: rgba(16, 185, 129, 0.15);
    border: 1px solid var(--color-success);
    color: #6ee7b7;
  }

  .test-result-banner.failure {
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid var(--color-danger);
    color: #fca5a5;
  }

  .modal-footer {
    padding: var(--spacing-md);
    border-top: 1px solid var(--color-outline);
    display: flex;
    justify-content: flex-end;
    gap: var(--spacing-sm);
  }

  .primary-btn {
    background: var(--color-primary);
    color: #ffffff;
    border: none;
    padding: 8px 16px;
    border-radius: var(--rounded-md);
    font-weight: 600;
    font-size: 13px;
    cursor: pointer;
  }

  .primary-btn:hover {
    background: #2563eb;
  }

  .secondary-btn {
    background: transparent;
    color: var(--color-on-surface);
    border: 1px solid var(--color-outline);
    padding: 8px 16px;
    border-radius: var(--rounded-md);
    font-weight: 600;
    font-size: 13px;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .secondary-btn:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .spinning {
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .animate-fade-in {
    animation: fadeIn 0.2s ease-out forwards;
  }

  .animate-scale-up {
    animation: scaleUp 0.18s cubic-bezier(0.34, 1.56, 0.64, 1) forwards;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes scaleUp {
    from { opacity: 0; transform: scale(0.95); }
    to { opacity: 1; transform: scale(1); }
  }
</style>
