<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getLeads, type Lead } from '$lib/services/crmService';

  interface ZaloSession {
    imei: string;
    user_agent: string;
    cookies_json: string;
    language: string;
  }

  interface OutreachTemplate {
    id?: number;
    name: string;
    content: string;
    created_at?: string;
  }

  // Reactive states
  let sessions = $state<ZaloSession[]>([]);
  let templates = $state<OutreachTemplate[]>([]);
  let leads = $state<Lead[]>([]);
  let leadSearchQuery = $state('');

  // Selected campaign parameters
  let selectedImei = $state('');
  let selectedTemplateId = $state<number | null>(null);
  let selectedLeadIds = $state<number[]>([]);

  // QR Modal flow states
  let isQrModalOpen = $state(false);
  let qrImage = $state<string | null>(null);
  let qrUser = $state<{ avatar: string; display_name: string } | null>(null);
  let qrStatus = $state('Initializing login...');
  let qrError = $state<string | null>(null);

  // Template Modal flow states
  let isTemplateModalOpen = $state(false);
  let editingTemplateId = $state<number | null>(null);
  let templateName = $state('');
  let templateContent = $state('');
  let templateError = $state<string | null>(null);

  // Campaign execution states
  let isCampaignRunning = $state(false);
  let campaignProgress = $state(0);
  let campaignStatus = $state('Idle');
  let campaignLogs = $state<string[]>([]);
  let campaignConsoleElement: HTMLDivElement | null = null;

  // Tauri event listeners
  let qrUnlistens: UnlistenFn[] = [];
  let campaignUnlistens: UnlistenFn[] = [];

  // Computed / filtered leads
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

  // Fetch initial data
  async function loadSessions() {
    try {
      sessions = await invoke<ZaloSession[]>('get_zalo_sessions');
      if (sessions.length > 0 && !selectedImei) {
        selectedImei = sessions[0].imei;
      }
    } catch (e) {
      console.error('Error loading Zalo sessions:', e);
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

  onMount(() => {
    loadSessions();
    loadTemplates();
    loadLeadsData();
  });

  onDestroy(() => {
    cleanupQrListeners();
    cleanupCampaignListeners();
  });

  // Helper cleanups
  function cleanupQrListeners() {
    qrUnlistens.forEach(u => u());
    qrUnlistens = [];
  }

  function cleanupCampaignListeners() {
    campaignUnlistens.forEach(u => u());
    campaignUnlistens = [];
  }

  // QR Login Actions
  async function openQrLogin() {
    isQrModalOpen = true;
    qrImage = null;
    qrUser = null;
    qrStatus = 'Connecting to Zalo...';
    qrError = null;

    cleanupQrListeners();

    try {
      // Setup QR event listeners first
      const codeUnlisten = await listen<{ image_base64: string; token: string; code: string }>(
        'zalo-qr-code',
        (event) => {
          qrImage = event.payload.image_base64;
          qrStatus = 'Scan the QR code with your Zalo app to login.';
        }
      );
      qrUnlistens.push(codeUnlisten);

      const scannedUnlisten = await listen<{ avatar: string; display_name: string }>(
        'zalo-qr-scanned',
        (event) => {
          qrUser = {
            avatar: event.payload.avatar,
            display_name: event.payload.display_name
          };
          qrStatus = 'QR Code Scanned! Please confirm on your mobile app.';
        }
      );
      qrUnlistens.push(scannedUnlisten);

      const successUnlisten = await listen<{ imei: string }>('zalo-qr-success', async (event) => {
        qrStatus = 'Login successful!';
        qrImage = null;
        await loadSessions();
        setTimeout(() => {
          closeQrModal();
        }, 1500);
      });
      qrUnlistens.push(successUnlisten);

      const expiredUnlisten = await listen('zalo-qr-expired', () => {
        qrStatus = 'QR Code expired. Retrying...';
        qrImage = null;
      });
      qrUnlistens.push(expiredUnlisten);

      const declinedUnlisten = await listen('zalo-qr-declined', () => {
        qrError = 'Login request declined on phone.';
        qrStatus = 'Declined';
        cleanupQrListeners();
      });
      qrUnlistens.push(declinedUnlisten);

      // Trigger the Tauri start command
      await invoke('start_zalo_qr_login');
    } catch (e) {
      qrError = `Failed to start login: ${e}`;
      qrStatus = 'Error';
    }
  }

  function closeQrModal() {
    isQrModalOpen = false;
    cleanupQrListeners();
  }

  async function deleteSession(imei: string) {
    if (!confirm('Are you sure you want to log out and delete this session?')) return;
    try {
      await invoke('delete_zalo_session', { imei });
      if (selectedImei === imei) {
        selectedImei = '';
      }
      await loadSessions();
    } catch (e) {
      alert(`Failed to delete session: ${e}`);
    }
  }

  // Template CRUD actions
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
      templateError = 'Name and Content are required.';
      return;
    }

    try {
      if (editingTemplateId !== null) {
        await invoke('update_outreach_template', {
          id: editingTemplateId,
          name: templateName,
          content: templateContent
        });
      } else {
        await invoke('create_outreach_template', {
          name: templateName,
          content: templateContent
        });
      }
      isTemplateModalOpen = false;
      await loadTemplates();
    } catch (e) {
      templateError = `Failed to save template: ${e}`;
    }
  }

  async function deleteTemplate(id: number) {
    if (!confirm('Are you sure you want to delete this template?')) return;
    try {
      await invoke('delete_outreach_template', { id });
      if (selectedTemplateId === id) {
        selectedTemplateId = null;
      }
      await loadTemplates();
    } catch (e) {
      alert(`Failed to delete template: ${e}`);
    }
  }

  // Leads checkbox toggle helpers
  function toggleLead(id: number) {
    if (selectedLeadIds.includes(id)) {
      selectedLeadIds = selectedLeadIds.filter(x => x !== id);
    } else {
      selectedLeadIds = [...selectedLeadIds, id];
    }
  }

  function toggleSelectAll() {
    const currentFilteredIds = filteredLeads.map(l => l.id).filter((id): id is number => !!id);
    const allSelected = currentFilteredIds.every(id => selectedLeadIds.includes(id));

    if (allSelected) {
      // Unselect all filtered leads
      selectedLeadIds = selectedLeadIds.filter(id => !currentFilteredIds.includes(id));
    } else {
      // Select all filtered leads (avoid duplicate inserts)
      const otherSelectedIds = selectedLeadIds.filter(id => !currentFilteredIds.includes(id));
      selectedLeadIds = [...otherSelectedIds, ...currentFilteredIds];
    }
  }

  // Campaign Execution
  function appendCampaignLog(msg: string) {
    const timestamp = new Date().toLocaleTimeString();
    campaignLogs = [...campaignLogs, `[${timestamp}] ${msg}`];
    
    setTimeout(() => {
      if (campaignConsoleElement) {
        campaignConsoleElement.scrollTop = campaignConsoleElement.scrollHeight;
      }
    }, 50);
  }

  async function startCampaign() {
    if (!selectedImei) {
      alert('Please select an active Zalo session first.');
      return;
    }
    if (selectedTemplateId === null) {
      alert('Please select an outreach template first.');
      return;
    }
    if (selectedLeadIds.length === 0) {
      alert('Please select at least one lead to receive the outreach.');
      return;
    }

    isCampaignRunning = true;
    campaignProgress = 0;
    campaignStatus = 'Starting...';
    campaignLogs = [];
    appendCampaignLog(`Starting outreach campaign to ${selectedLeadIds.length} leads...`);

    cleanupCampaignListeners();

    try {
      const progressUnlisten = await listen<{ progress: number; status: string; message: string }>(
        'outreach-progress',
        (event) => {
          campaignProgress = event.payload.progress;
          campaignStatus = event.payload.message;
          if (event.payload.status === 'failed') {
            isCampaignRunning = false;
            cleanupCampaignListeners();
          }
        }
      );
      campaignUnlistens.push(progressUnlisten);

      const logUnlisten = await listen<{ message: string }>('outreach-log', (event) => {
        appendCampaignLog(event.payload.message);
      });
      campaignUnlistens.push(logUnlisten);

      const completeUnlisten = await listen<{ total_sent: number; message: string }>('outreach-complete', (event) => {
        isCampaignRunning = false;
        campaignProgress = 100;
        campaignStatus = event.payload.message;
        appendCampaignLog(`[SUCCESS] ${event.payload.message}`);
        cleanupCampaignListeners();
      });
      campaignUnlistens.push(completeUnlisten);

      await invoke('send_zalo_outreach', {
        imei: selectedImei,
        leadIds: selectedLeadIds,
        templateId: selectedTemplateId
      });
      appendCampaignLog('Outreach campaign invoked successfully.');
    } catch (e) {
      isCampaignRunning = false;
      campaignStatus = 'Failed';
      appendCampaignLog(`Error starting outreach campaign: ${e}`);
    }
  }
</script>

<div class="container animate-fade-in">
  <header class="header">
    <div class="title-section">
      <span class="material-icons outreach-icon">campaign</span>
      <div>
        <h1>Zalo Outreach Manager</h1>
        <p class="subtitle">Authenticate sessions, customize outreach templates, and launch campaign flows</p>
      </div>
    </div>
  </header>

  <div class="layout-grid">
    <!-- Left Column: Sessions and Templates -->
    <div class="left-col">
      <!-- Zalo Sessions Card -->
      <div class="card session-card">
        <div class="card-header">
          <h2>Zalo Sessions</h2>
          <button type="button" class="add-session-btn" onclick={openQrLogin}>
            <span class="material-icons">qr_code_scanner</span>
            Add Session
          </button>
        </div>

        <div class="sessions-list">
          {#if sessions.length === 0}
            <div class="empty-state">
              <p>No active Zalo sessions. Click "Add Session" to scan and link your Zalo account.</p>
            </div>
          {:else}
            {#each sessions as s}
              <div class="session-item" class:selected={selectedImei === s.imei}>
                <button type="button" class="session-info" onclick={() => selectedImei = s.imei}>
                  <span class="material-icons status-icon active">check_circle</span>
                  <span class="session-details">
                    <span class="session-name">IMEI: {s.imei}</span>
                    <span class="session-ua">{(s.user_agent || '').substring(0, 40)}...</span>
                  </span>
                </button>
                <button type="button" class="delete-btn" onclick={() => deleteSession(s.imei)} title="Delete Session">
                  <span class="material-icons">delete</span>
                </button>
              </div>
            {/each}
          {/if}
        </div>
      </div>

      <!-- Templates Card -->
      <div class="card template-card">
        <div class="card-header">
          <h2>Outreach Templates</h2>
          <button type="button" class="add-template-btn" onclick={openCreateTemplate}>
            <span class="material-icons">add</span>
            Create Template
          </button>
        </div>

        <div class="templates-list">
          {#if templates.length === 0}
            <div class="empty-state">
              <p>No outreach templates. Create one to standardise your messaging.</p>
            </div>
          {:else}
            {#each templates as t}
              <div class="template-item" class:selected={selectedTemplateId === t.id}>
                <button type="button" class="template-info" onclick={() => { if (t.id) selectedTemplateId = t.id; }}>
                  <span class="template-name">{t.name}</span>
                  <span class="template-preview">{t.content}</span>
                </button>
                <div class="template-actions">
                  <button type="button" class="action-btn" onclick={() => openEditTemplate(t)} title="Edit Template">
                    <span class="material-icons">edit</span>
                  </button>
                  <button type="button" class="action-btn delete" onclick={() => t.id && deleteTemplate(t.id)} title="Delete Template">
                    <span class="material-icons">delete</span>
                  </button>
                </div>
              </div>
            {/each}
          {/if}
        </div>
      </div>
    </div>

    <!-- Right Column: Campaign Execution & Progress -->
    <div class="right-col">
      <div class="card campaign-card">
        <div class="card-header">
          <h2>Campaign Execution</h2>
          <span class="step-indicator">Steps 1-3</span>
        </div>

        <div class="campaign-form">
          <!-- Step 1: Select Session -->
          <div class="input-group">
            <label for="campaign-session">Step 1: Select Active Zalo Session</label>
            <div class="select-wrapper">
              <select id="campaign-session" bind:value={selectedImei} disabled={isCampaignRunning}>
                <option value="" disabled>-- Choose Session --</option>
                {#each sessions as s}
                  <option value={s.imei}>IMEI: {s.imei}</option>
                {/each}
              </select>
            </div>
          </div>

          <!-- Step 2: Select Template -->
          <div class="input-group">
            <label for="campaign-template">Step 2: Select Template</label>
            <div class="select-wrapper">
              <select id="campaign-template" bind:value={selectedTemplateId} disabled={isCampaignRunning}>
                <option value={null} disabled>-- Choose Template --</option>
                {#each templates as t}
                  <option value={t.id}>{t.name}</option>
                {/each}
              </select>
            </div>
          </div>

          <!-- Step 3: Select Leads -->
          <div class="leads-selector-section">
            <div class="leads-header">
              <span class="leads-title">Step 3: Select Leads ({selectedLeadIds.length} chosen)</span>
              <div class="leads-actions">
                <input 
                  type="text" 
                  class="search-bar" 
                  placeholder="Filter leads..." 
                  bind:value={leadSearchQuery} 
                />
                <button 
                  type="button" 
                  class="select-all-btn" 
                  onclick={toggleSelectAll}
                  disabled={filteredLeads.length === 0}
                >
                  {isAllLeadsSelected ? 'Deselect All' : 'Select All'}
                </button>
              </div>
            </div>

            <div class="leads-list-box">
              {#if filteredLeads.length === 0}
                <div class="empty-state">
                  <p>No leads found matching query.</p>
                </div>
              {:else}
                {#each filteredLeads as lead}
                  {#if lead.id}
                    <label class="lead-checkbox-item">
                      <input 
                        type="checkbox" 
                        checked={selectedLeadIds.includes(lead.id)} 
                        onchange={() => lead.id && toggleLead(lead.id)}
                        disabled={isCampaignRunning}
                      />
                      <span class="lead-label-info">
                        <span class="lead-lbl-name">{lead.name}</span>
                        <span class="lead-lbl-details">{lead.phone} • {lead.company}</span>
                      </span>
                    </label>
                  {/if}
                {/each}
              {/if}
            </div>
          </div>

          <button 
            type="button"
            class="start-campaign-btn" 
            onclick={startCampaign} 
            disabled={isCampaignRunning || !selectedImei || selectedTemplateId === null || selectedLeadIds.length === 0}
          >
            {#if isCampaignRunning}
              <span class="spinner"></span>
              Outreach Sending...
            {:else}
              <span class="material-icons">send</span>
              Launch Zalo Campaign
            {/if}
          </button>
        </div>

        <!-- Real-time progress bar and log console -->
        <div class="campaign-progress-section">
          <div class="progress-labels">
            <span class="status-msg">{campaignStatus}</span>
            <span class="percentage">{Math.round(campaignProgress)}%</span>
          </div>
          <div class="progress-bar-container">
            <div class="progress-bar-fill" style="width: {campaignProgress}%" class:animated={isCampaignRunning}></div>
          </div>

          <div class="console-section">
            <div class="console-header">
              <span class="material-icons console-icon">terminal</span>
              <h3>Outreach Console Logs</h3>
              <button type="button" class="clear-btn" onclick={() => campaignLogs = []} disabled={campaignLogs.length === 0}>
                Clear
              </button>
            </div>
            <div class="console-body" bind:this={campaignConsoleElement}>
              {#if campaignLogs.length === 0}
                <div class="console-placeholder">
                  <span class="material-icons">insights</span>
                  <p>Campaign logs will show here dynamically.</p>
                </div>
              {:else}
                {#each campaignLogs as log}
                  <div class="log-line">{log}</div>
                {/each}
              {/if}
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>

  <!-- QR LOGIN MODAL -->
  {#if isQrModalOpen}
    <div class="modal-backdrop">
      <div class="modal-card qr-modal animate-slide-down">
        <div class="modal-header">
          <h2>Zalo QR Login</h2>
          <button type="button" class="modal-close" onclick={closeQrModal}>
            <span class="material-icons">close</span>
          </button>
        </div>

        <div class="modal-body qr-body">
          <div class="qr-container">
            {#if qrImage}
              <img 
                src={qrImage.startsWith('data:image/') ? qrImage : `data:image/png;base64,${qrImage}`} 
                alt="Zalo QR Login Code" 
                class="qr-code-img"
              />
            {:else if qrUser}
              <div class="user-scanned animate-scale-up">
                <img src={qrUser.avatar} alt="User Avatar" class="user-avatar" />
                <h3>{qrUser.display_name}</h3>
              </div>
            {:else}
              <div class="qr-spinner-box">
                <div class="large-spinner"></div>
              </div>
            {/if}
          </div>

          <div class="qr-status-box">
            <p class="qr-status">{qrStatus}</p>
            {#if qrError}
              <p class="qr-error-msg">{qrError}</p>
            {/if}
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- TEMPLATE MODAL -->
  {#if isTemplateModalOpen}
    <div class="modal-backdrop">
      <div class="modal-card template-form-modal animate-slide-down">
        <div class="modal-header">
          <h2>{editingTemplateId ? 'Edit Outreach Template' : 'Create Outreach Template'}</h2>
          <button type="button" class="modal-close" onclick={() => isTemplateModalOpen = false}>
            <span class="material-icons">close</span>
          </button>
        </div>

        <div class="modal-body">
          {#if templateError}
            <div class="modal-error-banner">{templateError}</div>
          {/if}

          <div class="input-group">
            <label for="template-name-input">Template Name</label>
            <input 
              id="template-name-input"
              type="text" 
              bind:value={templateName} 
              placeholder="e.g. Cafe Welcome Promo" 
            />
          </div>

          <div class="input-group">
            <label for="template-content-input">Message Content</label>
            <textarea 
              id="template-content-input"
              rows="6" 
              bind:value={templateContent} 
              placeholder="Write your message here... Use [name] as placeholder."
            ></textarea>
          </div>

          <div class="modal-footer">
            <button type="button" class="secondary-btn" onclick={() => isTemplateModalOpen = false}>
              Cancel
            </button>
            <button type="button" class="primary-btn" onclick={saveTemplate}>
              Save Template
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .container {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-lg);
    block-size: 100%;
  }

  .header {
    background: linear-gradient(135deg, var(--color-surface-container) 0%, var(--color-surface-container-low) 100%);
    padding: var(--spacing-md) var(--spacing-lg);
    border-radius: var(--rounded-lg);
    border: 1px solid var(--color-outline-variant);
    box-shadow: var(--shadow-soft);
  }

  .title-section {
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
  }

  .outreach-icon {
    font-size: 36px;
    color: var(--color-success);
    background: rgba(16, 185, 129, 0.1);
    padding: var(--spacing-sm);
    border-radius: var(--rounded-md);
  }

  .subtitle {
    font-size: 13px;
    color: var(--color-on-surface-variant);
    margin-top: 2px;
  }

  .layout-grid {
    display: grid;
    grid-template-columns: 280px 1fr;
    gap: var(--spacing-lg);
    flex: 1;
    min-block-size: 0;
  }

  @media (max-width: 1024px) {
    .layout-grid {
      grid-template-columns: 1fr;
      overflow-y: auto;
    }
  }

  .left-col {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-lg);
    min-block-size: 0;
  }

  .right-col {
    display: flex;
    flex-direction: column;
    min-block-size: 0;
  }

  .card {
    background: var(--color-surface-container-lowest);
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-xl);
    box-shadow: var(--shadow-soft);
    display: flex;
    flex-direction: column;
    padding: var(--spacing-md);
    min-block-size: 0;
  }

  .session-card {
    flex: 1;
  }

  .template-card {
    flex: 1.2;
  }

  .campaign-card {
    flex: 1;
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: var(--spacing-md);
    border-bottom: 1px solid var(--color-outline-variant);
    padding-bottom: var(--spacing-sm);
  }

  .card-header h2 {
    font-size: 14px;
    font-weight: 700;
  }

  /* Sessions and Templates Lists */
  .sessions-list, .templates-list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
  }

  .empty-state {
    margin: auto;
    text-align: center;
    color: var(--color-neutral-slate);
    font-size: 12px;
    padding: var(--spacing-md);
  }

  .session-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--spacing-sm);
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-md);
    transition: var(--transition-fast);
    background: var(--color-surface-container-lowest);
  }

  .session-item:hover {
    border-color: var(--color-success);
    background: var(--color-surface-container-low);
  }

  .session-item.selected {
    border-color: var(--color-success);
    background: rgba(16, 185, 129, 0.05);
    box-shadow: 0 0 0 2px rgba(16, 185, 129, 0.1);
  }

  .session-item:focus-within,
  .template-item:focus-within,
  .lead-checkbox-item:focus-within {
    border-color: var(--color-success);
    background: var(--color-surface-container-low);
    outline: none;
    box-shadow: 0 0 0 2px rgba(16, 185, 129, 0.15);
  }

  .session-info {
    flex: 1;
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    cursor: pointer;
    background: none;
    border: none;
    text-align: left;
    padding: 0;
    font-family: inherit;
    color: inherit;
  }

  .status-icon {
    font-size: 18px;
    color: var(--color-neutral-slate);
  }

  .status-icon.active {
    color: var(--color-success);
  }

  .session-details {
    display: flex;
    flex-direction: column;
  }

  .session-name {
    font-size: 13px;
    font-weight: 600;
  }

  .session-ua {
    font-size: 11px;
    color: var(--color-neutral-slate);
  }

  .delete-btn {
    color: var(--color-neutral-slate);
    transition: var(--transition-fast);
    display: flex;
    align-items: center;
    padding: var(--spacing-xs);
    border-radius: var(--rounded-sm);
  }

  .delete-btn:hover {
    color: var(--color-danger);
    background: rgba(244, 63, 94, 0.1);
  }

  /* Template Item Styling */
  .template-item {
    display: flex;
    align-items: start;
    justify-content: space-between;
    padding: var(--spacing-sm);
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-md);
    transition: var(--transition-fast);
    cursor: pointer;
  }

  .template-item:hover {
    border-color: var(--color-success);
    background: var(--color-surface-container-low);
  }

  .template-item.selected {
    border-color: var(--color-success);
    background: rgba(16, 185, 129, 0.05);
  }

  .template-info {
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    text-align: left;
    padding: 0;
    font-family: inherit;
    color: inherit;
  }

  .template-name {
    font-size: 13px;
    font-weight: 600;
    display: block;
    margin-bottom: 2px;
  }

  .template-preview {
    font-size: 11px;
    color: var(--color-neutral-slate);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .template-actions {
    display: flex;
    gap: var(--spacing-xs);
  }

  .action-btn {
    color: var(--color-neutral-slate);
    padding: var(--spacing-xs);
    border-radius: var(--rounded-sm);
    display: flex;
    align-items: center;
    transition: var(--transition-fast);
  }

  .action-btn:hover {
    color: var(--color-primary);
    background: var(--color-surface-container);
  }

  .action-btn.delete:hover {
    color: var(--color-danger);
    background: rgba(244, 63, 94, 0.1);
  }

  button:focus-visible {
    outline: 2px solid var(--color-success);
    outline-offset: 2px;
  }

  /* Buttons */
  .add-session-btn, .add-template-btn {
    font-size: 12px;
    font-weight: 600;
    background: var(--color-surface-container-low);
    color: var(--color-primary);
    border: 1px solid var(--color-outline);
    padding: 6px 12px;
    border-radius: var(--rounded-md);
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
    transition: var(--transition-fast);
  }

  .add-session-btn:hover, .add-template-btn:hover {
    background: var(--color-surface-container);
    border-color: var(--color-primary);
  }

  /* Campaign Form & Selection */
  .campaign-form {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--spacing-md);
    margin-bottom: var(--spacing-lg);
  }

  @media (max-width: 768px) {
    .campaign-form {
      grid-template-columns: 1fr;
    }
  }

  .input-group {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
  }

  .input-group label {
    font-size: 12px;
    font-weight: 600;
    color: var(--color-on-surface-variant);
  }

  .select-wrapper {
    position: relative;
  }

  .select-wrapper select {
    width: 100%;
    padding: var(--spacing-sm) var(--spacing-md);
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-md);
    background: var(--color-surface-container-lowest);
    color: var(--color-on-surface);
    font-size: 13px;
    outline: none;
    transition: var(--transition-fast);
  }

  .select-wrapper select:focus {
    border-color: var(--color-success);
  }

  /* Step 3: Leads checkbox selector block */
  .leads-selector-section {
    grid-column: span 2;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
  }

  @media (max-width: 768px) {
    .leads-selector-section {
      grid-column: span 1;
    }
  }

  .leads-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 12px;
    font-weight: 600;
    color: var(--color-on-surface-variant);
  }

  .leads-actions {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
  }

  .search-bar {
    padding: var(--spacing-xs) var(--spacing-sm);
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-sm);
    font-size: 11px;
    outline: none;
    width: 150px;
  }

  .search-bar:focus {
    border-color: var(--color-success);
  }

  .select-all-btn {
    font-size: 11px;
    font-weight: 600;
    background: var(--color-surface-container-low);
    color: var(--color-primary);
    border: 1px solid var(--color-outline);
    padding: var(--spacing-xs) var(--spacing-sm);
    border-radius: var(--rounded-sm);
    transition: var(--transition-fast);
  }

  .select-all-btn:hover:not(:disabled) {
    background: var(--color-surface-container);
  }

  .select-all-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .leads-list-box {
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-md);
    background: var(--color-surface-container-lowest);
    max-block-size: 200px;
    overflow-y: auto;
    padding: var(--spacing-xs);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .lead-checkbox-item {
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
    padding: 6px var(--spacing-sm);
    border-radius: var(--rounded-sm);
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .lead-checkbox-item:hover {
    background: var(--color-surface-container-low);
  }

  .lead-checkbox-item input[type="checkbox"] {
    accent-color: var(--color-success);
    width: 16px;
    height: 16px;
    cursor: pointer;
  }

  .lead-label-info {
    display: flex;
    flex-direction: column;
  }

  .lead-lbl-name {
    font-size: 12px;
    font-weight: 600;
  }

  .lead-lbl-details {
    font-size: 10px;
    color: var(--color-neutral-slate);
  }

  .start-campaign-btn {
    grid-column: span 2;
    background: var(--color-success);
    color: var(--color-on-secondary);
    padding: var(--spacing-md);
    border-radius: var(--rounded-md);
    font-weight: 600;
    font-size: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--spacing-sm);
    transition: var(--transition-fast);
  }

  @media (max-width: 768px) {
    .start-campaign-btn {
      grid-column: span 1;
    }
  }

  .start-campaign-btn:hover:not(:disabled) {
    background: #0d9668;
    box-shadow: 0 4px 6px rgba(16, 185, 129, 0.15);
  }

  .start-campaign-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Progress & Console */
  .campaign-progress-section {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
    flex: 1;
    min-block-size: 0;
  }

  .progress-labels {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: var(--spacing-md);
  }

  .status-msg {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-on-surface);
  }

  .percentage {
    font-size: 14px;
    font-weight: 700;
    color: var(--color-success);
  }

  .progress-bar-container {
    height: 10px;
    background: var(--color-surface-container-highest);
    border-radius: var(--rounded-full);
    overflow: hidden;
    position: relative;
    margin-bottom: var(--spacing-md);
  }

  .progress-bar-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--color-success) 0%, #34d399 100%);
    border-radius: var(--rounded-full);
    transition: width var(--transition-fast) ease-out;
  }

  .progress-bar-fill.animated {
    background-image: linear-gradient(
      45deg,
      rgba(255, 255, 255, 0.15) 25%,
      transparent 25%,
      transparent 50%,
      rgba(255, 255, 255, 0.15) 50%,
      rgba(255, 255, 255, 0.15) 75%,
      transparent 75%,
      transparent
    );
    background-size: 1rem 1rem;
    animation: progress-bar-stripes 1s linear infinite;
  }

  /* Console section */
  .console-section {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-block-size: 200px;
    background: #0f172a;
    border-radius: var(--rounded-md);
    overflow: hidden;
    border: 1px solid #1e293b;
    box-shadow: inset 0 2px 8px rgba(0, 0, 0, 0.3);
  }

  .console-header {
    background: #1e293b;
    padding: var(--spacing-sm) var(--spacing-md);
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    border-bottom: 1px solid #334155;
  }

  .console-icon {
    font-size: 18px;
    color: #38bdf8;
  }

  .console-header h3 {
    font-size: 12px;
    font-weight: 700;
    color: #e2e8f0;
    margin: 0;
    flex: 1;
  }

  .clear-btn {
    font-size: 11px;
    font-weight: 600;
    color: #94a3b8;
    background: #334155;
    padding: var(--spacing-xs) var(--spacing-sm);
    border-radius: var(--rounded-sm);
    transition: var(--transition-fast);
  }

  .clear-btn:hover:not(:disabled) {
    color: #f1f5f9;
    background: #475569;
  }

  .console-body {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-md);
    font-family: Menlo, Monaco, Consolas, "Fira Code", monospace;
    font-size: 11px;
    line-height: 1.5;
    color: #34d399;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
  }

  .console-placeholder {
    margin: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--spacing-sm);
    color: #64748b;
    text-align: center;
  }

  .console-placeholder .material-icons {
    font-size: 32px;
  }

  .log-line {
    word-break: break-all;
    white-space: pre-wrap;
  }

  /* Modals Overlay */
  .modal-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(15, 23, 42, 0.4);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal-card {
    background: var(--color-surface-container-lowest);
    border-radius: var(--rounded-xl);
    border: 1px solid var(--color-outline-variant);
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 10px 10px -5px rgba(0, 0, 0, 0.04);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .qr-modal {
    width: 320px;
  }

  .template-form-modal {
    width: 480px;
  }

  .modal-header {
    background: var(--color-surface-container-low);
    padding: var(--spacing-md);
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--color-outline-variant);
  }

  .modal-header h2 {
    font-size: 14px;
    font-weight: 700;
  }

  .modal-close {
    color: var(--color-neutral-slate);
    display: flex;
    align-items: center;
    padding: var(--spacing-xs);
    border-radius: var(--rounded-sm);
    transition: var(--transition-fast);
  }

  .modal-close:hover {
    color: var(--color-danger);
    background: rgba(244, 63, 94, 0.1);
  }

  .modal-body {
    padding: var(--spacing-lg);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
  }

  /* QR Login Internal styling */
  .qr-body {
    align-items: center;
    text-align: center;
  }

  .qr-container {
    width: 200px;
    height: 200px;
    background: var(--color-surface-container-low);
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-lg);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    position: relative;
  }

  .qr-code-img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .user-scanned {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--spacing-sm);
  }

  .user-avatar {
    width: 80px;
    height: 80px;
    border-radius: var(--rounded-full);
    border: 3px solid var(--color-success);
    box-shadow: var(--shadow-soft);
  }

  .user-scanned h3 {
    font-size: 14px;
    margin: 0;
  }

  .qr-spinner-box {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .large-spinner {
    width: 40px;
    height: 40px;
    border: 4px solid rgba(16, 185, 129, 0.1);
    border-top-color: var(--color-success);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  .qr-status-box {
    margin-top: var(--spacing-sm);
  }

  .qr-status {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-on-surface);
  }

  .qr-error-msg {
    font-size: 12px;
    color: var(--color-danger);
    margin-top: var(--spacing-xs);
  }

  /* Template Form Modal Elements */
  .modal-error-banner {
    background: rgba(244, 63, 94, 0.08);
    border: 1px solid var(--color-danger);
    padding: var(--spacing-sm);
    color: var(--color-danger);
    font-size: 12px;
    border-radius: var(--rounded-md);
  }

  .template-form-modal input, .template-form-modal textarea {
    width: 100%;
    padding: var(--spacing-sm) var(--spacing-md);
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-md);
    background: var(--color-surface-container-lowest);
    color: var(--color-on-surface);
    font-family: inherit;
    font-size: 13px;
    outline: none;
    transition: var(--transition-fast);
  }

  .template-form-modal input:focus, .template-form-modal textarea:focus {
    border-color: var(--color-success);
  }

  .modal-footer {
    display: flex;
    justify-content: end;
    gap: var(--spacing-sm);
    margin-top: var(--spacing-sm);
  }

  .primary-btn {
    background: var(--color-success);
    color: var(--color-on-secondary);
    font-weight: 600;
    font-size: 13px;
    padding: 8px 16px;
    border-radius: var(--rounded-md);
    transition: var(--transition-fast);
  }

  .primary-btn:hover {
    background: #0d9668;
  }

  .secondary-btn {
    background: var(--color-surface-container-low);
    color: var(--color-on-surface-variant);
    border: 1px solid var(--color-outline);
    font-weight: 600;
    font-size: 13px;
    padding: 8px 16px;
    border-radius: var(--rounded-md);
    transition: var(--transition-fast);
  }

  .secondary-btn:hover {
    background: var(--color-surface-container);
  }

  /* Spinners */
  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top-color: #ffffff;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  /* Animations */
  .animate-fade-in {
    animation: fadeIn 0.3s ease-out forwards;
  }

  .animate-slide-down {
    animation: slideDown 0.25s ease-out forwards;
  }

  .animate-scale-up {
    animation: scaleUp 0.2s cubic-bezier(0.34, 1.56, 0.64, 1) forwards;
  }

  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(4px); }
    to { opacity: 1; transform: translateY(0); }
  }

  @keyframes slideDown {
    from { opacity: 0; transform: translateY(-12px); }
    to { opacity: 1; transform: translateY(0); }
  }

  @keyframes scaleUp {
    from { opacity: 0; transform: scale(0.9); }
    to { opacity: 1; transform: scale(1); }
  }
</style>
