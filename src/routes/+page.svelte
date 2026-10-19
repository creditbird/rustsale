<script lang="ts">
  import { onMount, tick } from 'svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import Dashboard from '$lib/components/Dashboard.svelte';
  import LeadsList from '$lib/components/LeadsList.svelte';
  import KanbanBoard from '$lib/components/KanbanBoard.svelte';
  import GoogleMapsScraper from '$lib/components/GoogleMapsScraper.svelte';
  import ZaloOutreach from '$lib/components/ZaloOutreach.svelte';
  import WhatsAppOutreach from '$lib/components/WhatsAppOutreach.svelte';
  import EmailOutreach from '$lib/components/EmailOutreach.svelte';
  import CategoryManager from '$lib/components/CategoryManager.svelte';
  import ConversationWorkspace from '$lib/components/conversation/ConversationWorkspace.svelte';
  import FacebookHub from '$lib/components/facebook/FacebookHub.svelte';
  import CampaignCenter from '$lib/components/campaign/CampaignCenter.svelte';
  import AIApprovalQueue from '$lib/components/campaign/AIApprovalQueue.svelte';
  import TemplateCenter from '$lib/components/template/TemplateCenter.svelte';
  import Settings from '$lib/components/Settings.svelte';
  import LicenseModal from '$lib/components/LicenseModal.svelte';
  import { verifyDeviceAndSubscription } from '$lib/services/licenseService';
  import { initDesktopNotificationBridge } from '$lib/services/desktopNotificationService';
  import * as crmService from '$lib/services/crmService';
  import type { Lead } from '$lib/services/crmService';
  import { AlertCircle, Search, Bell } from 'lucide-svelte';

  let activeTab = $state('dashboard');
  let globalSearchQuery = $state('');
  let showLicenseModal = $state(false);
  let preventLicenseClose = $state(false);
  let isLicenseVerified = $state(false);
  let targetLeadForConversation = $state<{ leadId?: number; phone?: string; name?: string; zalo_uid?: string } | null>(null);

  let leads = $state<Lead[]>([]);
  let activeLeads = $derived(leads.filter((l) => l.status !== 'Converted'));
  let convertedContacts = $derived(leads.filter((l) => l.status === 'Converted'));
  let errorMsg = $state<string | null>(null);

  function handleOpenConversationForLead(lead: { id: number; phone?: string; name?: string; zalo_uid?: string }) {
    targetLeadForConversation = { leadId: lead.id, phone: lead.phone, name: lead.name, zalo_uid: lead.zalo_uid };
    activeTab = 'conversation';
  }

  import { m } from '$lib/i18n';

  const tabTitles: Record<string, { title: string; subtitle: string }> = {
    get dashboard() { return { title: m.tab_dashboard_title(), subtitle: m.tab_dashboard_subtitle() }; },
    get scraper() { return { title: m.tab_scraper_title(), subtitle: m.tab_scraper_subtitle() }; },
    get leads() { return { title: m.tab_leads_title(), subtitle: m.tab_leads_subtitle() }; },
    get categories() { return { title: m.tab_categories_title(), subtitle: m.tab_categories_subtitle() }; },
    get conversation() { return { title: m.tab_conversation_title(), subtitle: m.tab_conversation_subtitle() }; },
    get campaign_center() { return { title: m.tab_campaign_center_title(), subtitle: m.tab_campaign_center_subtitle() }; },
    template_center: {
      title: 'Thư Viện Template Mẫu',
      subtitle: 'Quản lý kho nội dung, kịch bản mẫu đa kênh và phân tích hiệu suất'
    },
    get ai_approval() { return { title: m.tab_ai_approval_title(), subtitle: m.tab_ai_approval_subtitle() }; },
    get facebook_hub() { return { title: m.tab_facebook_hub_title(), subtitle: m.tab_facebook_hub_subtitle() }; },
    get outreach() { return { title: m.tab_outreach_title(), subtitle: m.tab_outreach_subtitle() }; },
    get whatsapp_outreach() { return { title: m.tab_whatsapp_outreach_title(), subtitle: m.tab_whatsapp_outreach_subtitle() }; },
    get email_outreach() { return { title: m.tab_email_outreach_title(), subtitle: m.tab_email_outreach_subtitle() }; },
    get pipeline() { return { title: m.tab_pipeline_title(), subtitle: m.tab_pipeline_subtitle() }; },
    get contacts() { return { title: m.tab_contacts_title(), subtitle: m.tab_contacts_subtitle() }; },
    get settings() { return { title: m.tab_settings_title(), subtitle: m.tab_settings_subtitle() }; }
  };

  let isStale = $state(false);
  let hasLoaded = $state(false);

  let reloadTimer: ReturnType<typeof setTimeout> | null = null;
  function debouncedLoadLeads() {
    if (reloadTimer) clearTimeout(reloadTimer);
    reloadTimer = setTimeout(() => {
      reloadTimer = null;
      if (activeTab === 'leads' || activeTab === 'contacts') {
        loadLeads();
      } else {
        isStale = true;
      }
    }, 300);
  }

  async function loadLeads() {
    try {
      leads = await crmService.getLeads();
      errorMsg = null;
      hasLoaded = true;
      isStale = false;
    } catch (err) {
      hasLoaded = false;
      errorMsg = err instanceof Error ? err.message : String(err);
    }
  }

  $effect(() => {
    if (activeTab === 'leads' || activeTab === 'contacts') {
      if (isStale || !hasLoaded) {
        isStale = false;
        loadLeads();
      }
    } else {
      if (reloadTimer) {
        clearTimeout(reloadTimer);
        reloadTimer = null;
        isStale = true;
      }
    }
  });

  function handleOpenLicenseModal() {
    preventLicenseClose = !isLicenseVerified;
    showLicenseModal = true;
  }

  onMount(() => {
    let unmounted = false;
    const unsubBridge = crmService.initScraperEventBridge();
    let unsubNotifBridge: (() => void) | null = null;

    initDesktopNotificationBridge({
      onNavigate: (conversationId, leadId) => {
        targetLeadForConversation = leadId ? { leadId } : null;
        activeTab = 'conversation';
      }
    }).then((cleanup) => {
      if (unmounted) {
        cleanup();
      } else {
        unsubNotifBridge = cleanup;
      }
    });

    const unsub = crmService.onLeadsChanged(() => {
      if (activeTab === 'leads' || activeTab === 'contacts') {
        debouncedLoadLeads();
      } else {
        isStale = true;
      }
    });

    // Option 1: Gate check on app startup
    verifyDeviceAndSubscription()
      .then((res) => {
        if (!res.allowed) {
          preventLicenseClose = true;
          showLicenseModal = true;
          isLicenseVerified = false;
        } else {
          isLicenseVerified = true;
          preventLicenseClose = false;
        }
      })
      .catch((err) => {
        console.error('License check on startup failed:', err);
        preventLicenseClose = true;
        showLicenseModal = true;
        isLicenseVerified = false;
      });

    return () => {
      unmounted = true;
      unsub();
      unsubBridge();
      if (unsubNotifBridge) unsubNotifBridge();
      if (reloadTimer) clearTimeout(reloadTimer);
    };
  });

  let settingsRef = $state<{ openQrLogin?: () => void; openWhatsAppQrLogin?: () => void } | null>(null);

  function handleOpenQrLogin(accountKey?: string) {
    activeTab = 'settings';
    tick().then(() => {
      if (accountKey && accountKey.toLowerCase().includes('whatsapp')) {
        settingsRef?.openWhatsAppQrLogin?.();
      } else {
        settingsRef?.openQrLogin?.();
      }
    });
  }
</script>

<div class="flex w-screen h-screen overflow-hidden bg-background text-foreground antialiased font-sans">
  <!-- Left Sidebar Navigation -->
  <Sidebar bind:activeTab onOpenLicense={handleOpenLicenseModal} />

  <!-- Main View Container -->
  <main class="flex-1 flex flex-col h-full overflow-hidden bg-muted/20">
    <!-- Top Header Bar -->
    <header class="h-16 px-6 border-b border-border bg-background flex items-center justify-between shrink-0 shadow-2xs">
      <div>
        <h1 class="text-base font-bold text-foreground tracking-tight">
          {tabTitles[activeTab]?.title || 'RustSale Pro'}
        </h1>
        <p class="text-xs text-muted-foreground">
          {tabTitles[activeTab]?.subtitle || 'Enterprise CRM Workspace'}
        </p>
      </div>

      <div class="flex items-center gap-3">
        <!-- Quick Search -->
        <div class="relative hidden md:block w-64">
          <Search class="w-4 h-4 absolute left-3 top-2.5 text-muted-foreground" />
          <input
            type="text"
            bind:value={globalSearchQuery}
            onkeydown={(e) => {
              if (e.key === 'Enter' && globalSearchQuery.trim() && activeTab !== 'leads' && activeTab !== 'contacts') {
                activeTab = 'leads';
              }
            }}
            placeholder="Quick search..."
            class="w-full h-9 pl-9 pr-4 rounded-md border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring transition-all"
          />
        </div>

        <!-- Notifications -->
        <button
          type="button"
          onclick={() => (activeTab = 'settings')}
          class="w-9 h-9 rounded-md border border-input bg-background flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-accent transition-colors relative"
          title="Notifications"
        >
          <Bell class="w-4 h-4" />
          <span class="w-2 h-2 rounded-full bg-emerald-500 absolute top-2 right-2"></span>
        </button>
      </div>
    </header>

    <!-- Main Scrollable Content Area -->
    <div class={activeTab === 'conversation'
      ? 'flex-1 min-h-0 overflow-hidden'
      : 'flex-1 overflow-y-auto p-6 flex flex-col gap-6'}>
      {#if errorMsg}
        <div class="p-4 rounded-lg border border-destructive/30 bg-destructive/10 text-destructive text-xs flex items-center gap-3 animate-fade-in">
          <AlertCircle class="w-4 h-4 shrink-0" />
          <span class="font-medium">{errorMsg}</span>
        </div>
      {/if}

      <div class={activeTab === 'conversation' ? 'h-full' : 'hidden'}>
        <ConversationWorkspace {activeTab} targetLead={targetLeadForConversation} onOpenSettings={() => (activeTab = 'settings')} onOpenQrLogin={handleOpenQrLogin} />
      </div>

      {#if activeTab === 'facebook_hub'}
        <FacebookHub {activeTab} onOpenSettings={() => (activeTab = 'settings')} />
      {:else if activeTab === 'campaign_center'}
        <CampaignCenter {activeTab} />
      {:else if activeTab === 'template_center'}
        <TemplateCenter {activeTab} />
      {:else if activeTab === 'ai_approval'}
        <AIApprovalQueue {activeTab} />
      {:else if activeTab === 'dashboard'}
        <Dashboard {activeTab} />
      {:else if activeTab === 'scraper'}
        <GoogleMapsScraper />
      {:else if activeTab === 'leads'}
        <LeadsList entityType="leads" leads={activeLeads} bind:searchQuery={globalSearchQuery} onRefresh={loadLeads} onOpenConversation={handleOpenConversationForLead} />
      {:else if activeTab === 'contacts'}
        <LeadsList entityType="contacts" leads={convertedContacts} bind:searchQuery={globalSearchQuery} onRefresh={loadLeads} onOpenConversation={handleOpenConversationForLead} />
      {:else if activeTab === 'categories'}
        <CategoryManager {activeTab} />
      {:else if activeTab === 'outreach'}
        <ZaloOutreach {activeTab} onOpenSettings={() => (activeTab = 'settings')} />
      {:else if activeTab === 'whatsapp_outreach'}
        <WhatsAppOutreach {activeTab} onOpenSettings={() => (activeTab = 'settings')} />
      {:else if activeTab === 'email_outreach'}
        <EmailOutreach {activeTab} onOpenSettings={() => (activeTab = 'settings')} />
      {:else if activeTab === 'pipeline'}
        <KanbanBoard />
      {:else if activeTab === 'settings'}
        <Settings bind:this={settingsRef} onNavigateToHub={() => (activeTab = 'facebook_hub')} />
      {/if}
    </div>
  </main>

  <!-- License & HWID Management Modal -->
  <LicenseModal
    bind:open={showLicenseModal}
    preventClose={preventLicenseClose}
    onVerified={() => {
      isLicenseVerified = true;
      preventLicenseClose = false;
      showLicenseModal = false;
    }}
  />
</div>

