<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import {
    deleteFacebookAccount,
    fetchFacebookPagesFromToken,
    listManagedFacebookAccounts,
    saveManagedFacebookAccount,
    type AccountRuntimeStatus,
    type DiscoveredFacebookPage,
    type ManagedFacebookAccount
  } from '$lib/services/accountSettingsService';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Input } from '$lib/components/ui/input';
  import {
    AlertCircle,
    Check,
    CheckCircle2,
    Clock3,
    ExternalLink,
    KeyRound,
    LoaderCircle,
    MessagesSquare,
    Plus,
    RefreshCw,
    ShieldCheck,
    Trash2,
    Wifi,
    WifiOff
  } from 'lucide-svelte';

  let { onNavigateToHub }: { onNavigateToHub?: () => void } = $props();

  let accounts = $state<ManagedFacebookAccount[]>([]);
  let inFlightLoads = $state(0);
  let loading = $derived(inFlightLoads > 0);
  let isPolling = $state(false);
  let loadRequestId = 0;
  let loadError = $state<string | null>(null);
  let actionError = $state<string | null>(null);
  let deleting = $state(new Set<string>());

  // Connect Modal State
  let connectModalOpen = $state(false);
  let connectTab = $state<'oauth' | 'manual'>('oauth');
  let oauthTokenOrUrl = $state('');
  let oauthStateNonce = $state<string | null>(null);
  let isFetchingPages = $state(false);
  let isSavingPages = $state(false);
  let modalError = $state<string | null>(null);
  let modalSuccess = $state<string | null>(null);
  let discoveredPages = $state<DiscoveredFacebookPage[]>([]);
  let selectedPageIds = $state<Set<string>>(new Set());

  // Manual Tab Form
  let manualPageId = $state('');
  let manualPageName = $state('');
  let manualAccessToken = $state('');
  let manualAvatarUrl = $state('');

  // Delete Confirm Modal State
  let deleteConfirmOpen = $state(false);
  let pageToDelete = $state<ManagedFacebookAccount | null>(null);

  function authVariant(status: string): 'default' | 'secondary' | 'outline' | 'destructive' {
    if (status === 'valid') return 'default';
    if (status === 'expired' || status === 'invalid' || status === 'revoked') return 'destructive';
    return 'secondary';
  }

  function authLabel(status: string): string {
    if (status === 'valid') return 'Token Valid';
    if (status === 'expired') return 'Token Expired';
    if (status === 'invalid') return 'Token Invalid';
    if (status === 'revoked') return 'Token Revoked';
    return 'Token Unknown';
  }

  function runtimeVariant(status: AccountRuntimeStatus): 'default' | 'secondary' | 'outline' | 'destructive' {
    if (status === 'connected') return 'default';
    if (status === 'invalid') return 'destructive';
    return 'outline';
  }

  function runtimeLabel(status: AccountRuntimeStatus): string {
    if (status === 'connected') return 'Runtime Connected';
    if (status === 'connecting') return 'Runtime Connecting';
    if (status === 'degraded') return 'Runtime Degraded';
    if (status === 'invalid') return 'Runtime Invalid';
    return 'Runtime Disconnected';
  }

  export async function loadAccounts(silent = false) {
    if (silent && isPolling) return;
    if (silent) isPolling = true;
    const thisRequestId = ++loadRequestId;
    if (!silent) {
      inFlightLoads++;
      loadError = null;
    }
    try {
      const fetched = await listManagedFacebookAccounts();
      if (thisRequestId === loadRequestId) {
        accounts = fetched;
        loadError = null;
      }
    } catch (error) {
      if (!silent && thisRequestId === loadRequestId) {
        loadError = error instanceof Error ? error.message : String(error);
      }
    } finally {
      if (!silent) {
        inFlightLoads = Math.max(0, inFlightLoads - 1);
      }
      if (silent) {
        isPolling = false;
      }
    }
  }

  function generateCryptoNonce(): string {
    if (typeof crypto !== 'undefined' && typeof crypto.getRandomValues === 'function') {
      const bytes = new Uint8Array(16);
      crypto.getRandomValues(bytes);
      return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
    }
    return Math.random().toString(36).substring(2, 15);
  }

  function openConnectModal() {
    connectTab = 'oauth';
    oauthTokenOrUrl = '';
    oauthStateNonce = null;
    discoveredPages = [];
    selectedPageIds = new Set();
    modalError = null;
    modalSuccess = null;
    manualPageId = '';
    manualPageName = '';
    manualAccessToken = '';
    manualAvatarUrl = '';
    connectModalOpen = true;
  }

  function extractTokenAndValidateState(raw: string): { token: string; error?: string } {
    const trimmed = raw.trim();
    if (!trimmed) return { token: '' };

    // If OAuth flow was initiated in this session, require valid state correlation
    if (oauthStateNonce) {
      const stateMatch = trimmed.match(/[#&?]state=([^&]+)/);
      if (!stateMatch || !stateMatch[1]) {
        return {
          token: '',
          error: 'OAuth callback is missing required state parameter. Please complete authorization through the opened window.'
        };
      }
      const decodedState = decodeURIComponent(stateMatch[1]);
      if (decodedState !== oauthStateNonce) {
        return {
          token: '',
          error: 'OAuth state nonce mismatch. Please re-authenticate to prevent CSRF.'
        };
      }
    }

    const tokenMatch = trimmed.match(/[#&?]access_token=([^&]+)/);
    if (tokenMatch && tokenMatch[1]) {
      return { token: decodeURIComponent(tokenMatch[1]) };
    }

    return { token: trimmed };
  }

  function handleOpenMetaOAuth() {
    const metaAppId = '1075678647572774';
    const redirectUri = 'https://facebook.com/connect/login_success.html';
    const scope = 'pages_show_list,pages_read_engagement,pages_manage_posts,pages_messaging';
    const nonce = generateCryptoNonce();
    oauthStateNonce = nonce;
    const authUrl = `https://www.facebook.com/v26.0/dialog/oauth?client_id=${metaAppId}&redirect_uri=${encodeURIComponent(redirectUri)}&response_type=token&state=${encodeURIComponent(nonce)}&scope=${encodeURIComponent(scope)}`;
    window.open(authUrl, '_blank');
  }

  async function handleDiscoverPages() {
    const { token: cleanToken, error: stateError } = extractTokenAndValidateState(oauthTokenOrUrl);
    if (stateError) {
      modalError = stateError;
      return;
    }
    if (!cleanToken) {
      modalError = 'Please enter or paste an Access Token or Meta Callback URL';
      return;
    }

    modalError = null;
    modalSuccess = null;
    isFetchingPages = true;

    try {
      const pages = await fetchFacebookPagesFromToken(cleanToken);
      if (pages.length === 0) {
        modalError = 'No managed Fanpages found for this account/token.';
      } else {
        discoveredPages = pages;
        // Select all by default
        selectedPageIds = new Set(pages.map((p) => p.id));
        modalSuccess = `Found ${pages.length} Fanpage(s). Select which pages to connect below:`;
      }
    } catch (e) {
      modalError = e instanceof Error ? e.message : String(e);
      discoveredPages = [];
    } finally {
      isFetchingPages = false;
    }
  }

  function toggleSelectPage(pageId: string) {
    const next = new Set(selectedPageIds);
    if (next.has(pageId)) {
      next.delete(pageId);
    } else {
      next.add(pageId);
    }
    selectedPageIds = next;
  }

  async function handleSaveDiscoveredPages() {
    if (selectedPageIds.size === 0) {
      modalError = 'Please select at least one Fanpage to connect.';
      return;
    }

    modalError = null;
    isSavingPages = true;

    try {
      for (const page of discoveredPages) {
        if (selectedPageIds.has(page.id)) {
          await saveManagedFacebookAccount({
            pageId: page.id,
            pageName: page.name,
            avatarUrl: page.avatarUrl,
            accessToken: page.accessToken,
            grantedScopes: page.permissions
          });
        }
      }
      await loadAccounts(false);
      connectModalOpen = false;
    } catch (e) {
      modalError = e instanceof Error ? e.message : String(e);
    } finally {
      isSavingPages = false;
    }
  }

  async function handleSaveManual() {
    const pageId = manualPageId.trim();
    const token = manualAccessToken.trim();
    if (!pageId) {
      modalError = 'Page ID is required';
      return;
    }
    if (!token) {
      modalError = 'Page Access Token is required';
      return;
    }

    modalError = null;
    isSavingPages = true;

    try {
      await saveManagedFacebookAccount({
        pageId,
        pageName: manualPageName.trim() || pageId,
        avatarUrl: manualAvatarUrl.trim() || null,
        accessToken: token,
        grantedScopes: [
          'pages_show_list',
          'pages_read_engagement',
          'pages_manage_posts',
          'pages_messaging'
        ]
      });
      await loadAccounts(false);
      connectModalOpen = false;
    } catch (e) {
      modalError = e instanceof Error ? e.message : String(e);
    } finally {
      isSavingPages = false;
    }
  }

  function promptDelete(account: ManagedFacebookAccount) {
    pageToDelete = account;
    deleteConfirmOpen = true;
  }

  async function confirmDelete() {
    if (!pageToDelete) return;
    const pageId = pageToDelete.pageId;
    deleteConfirmOpen = false;
    deleting = new Set(deleting).add(pageId);
    actionError = null;

    try {
      await deleteFacebookAccount(pageId);
      accounts = accounts.filter((a) => a.pageId !== pageId);
    } catch (error) {
      actionError = error instanceof Error ? error.message : String(error);
    } finally {
      const next = new Set(deleting);
      next.delete(pageId);
      deleting = next;
      pageToDelete = null;
    }
  }

  let runtimePoll: ReturnType<typeof setInterval> | null = null;

  onMount(() => {
    void loadAccounts();
    runtimePoll = setInterval(() => {
      void loadAccounts(true);
    }, 5_000);
    runtimePoll?.unref?.();

    return () => {
      if (runtimePoll) clearInterval(runtimePoll);
    };
  });

  onDestroy(() => {
    if (runtimePoll) clearInterval(runtimePoll);
  });
</script>

<div class="flex flex-col gap-4">
  <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
    <div>
      <h3 class="text-sm font-semibold text-foreground">Facebook & Messenger Pages</h3>
      <p class="text-xs text-muted-foreground">
        Authentication health and 2-way Messenger synchronization are tracked securely via Token Vault.
      </p>
    </div>
    <div class="flex items-center gap-2">
      <Button
        variant="outline"
        size="sm"
        onclick={() => loadAccounts()}
        disabled={loading}
        class="gap-1.5 text-xs shadow-2xs"
      >
        <RefreshCw class="size-3.5 {loading ? 'animate-spin' : ''}" />
        Refresh
      </Button>
      <Button
        size="sm"
        onclick={openConnectModal}
        class="gap-1.5 text-xs bg-blue-600 hover:bg-blue-700 text-white shadow-2xs"
      >
        <Plus class="size-3.5" />
        Connect Fanpage
      </Button>
    </div>
  </div>

  {#if actionError}
    <div class="flex items-start gap-2.5 rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive">
      <AlertCircle class="size-4 shrink-0 mt-0.5" />
      <span>{actionError}</span>
    </div>
  {/if}

  {#if loadError}
    <div class="flex items-start gap-2.5 rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive">
      <AlertCircle class="size-4 shrink-0 mt-0.5" />
      <span>{loadError}</span>
    </div>
  {/if}

  {#if loading && accounts.length === 0}
    <div class="flex items-center justify-center p-12 text-muted-foreground text-xs gap-2">
      <LoaderCircle class="size-4 animate-spin text-primary" />
      Loading connected Facebook accounts...
    </div>
  {:else if accounts.length === 0}
    <Card.Root class="border-dashed">
      <Card.Content class="flex flex-col items-center justify-center py-10 text-center gap-3">
        <div class="size-12 rounded-full bg-blue-600/10 text-blue-600 flex items-center justify-center font-bold text-xl">
          <MessagesSquare class="size-6" />
        </div>
        <div class="max-w-sm">
          <h4 class="text-sm font-semibold text-foreground">No Facebook Pages Connected</h4>
          <p class="text-xs text-muted-foreground mt-1">
            Connect your Facebook Fanpage to enable 2-way Messenger chat in Conversations, automated comment replies, and post scheduling.
          </p>
        </div>
        <Button
          size="sm"
          onclick={openConnectModal}
          class="gap-1.5 text-xs bg-blue-600 hover:bg-blue-700 text-white shadow-2xs mt-2"
        >
          <Plus class="size-3.5" />
          Connect Facebook Page
        </Button>
      </Card.Content>
    </Card.Root>
  {:else}
    <div class="grid grid-cols-1 md:grid-cols-2 gap-3.5">
      {#each accounts as account (account.pageId)}
        <Card.Root class="overflow-hidden border border-border/80 shadow-2xs hover:border-border transition-colors">
          <Card.Header class="pb-3 bg-muted/10 border-b border-border/40">
            <div class="flex items-start justify-between gap-3">
              <div class="flex items-center gap-3 min-w-0">
                {#if account.avatarUrl}
                  <img
                    src={account.avatarUrl}
                    alt={account.pageName || account.pageId}
                    class="size-10 rounded-lg object-cover border border-border shrink-0"
                  />
                {:else}
                  <div class="size-10 rounded-lg bg-blue-600 text-white flex items-center justify-center font-bold text-base shrink-0 shadow-2xs">
                    f
                  </div>
                {/if}
                <div class="min-w-0">
                  <Card.Title class="text-sm truncate">
                    {account.pageName || account.pageId}
                  </Card.Title>
                  <Card.Description class="text-[11px] font-mono truncate text-muted-foreground">
                    ID: {account.pageId}
                  </Card.Description>
                </div>
              </div>

              <div class="flex items-center gap-1.5 shrink-0">
                {#if account.runtimeStatus === 'connected'}
                  <span title="Connected"><Wifi class="size-4 text-emerald-600" /></span>
                {:else}
                  <span title="Disconnected"><WifiOff class="size-4 text-muted-foreground" /></span>
                {/if}
              </div>
            </div>
          </Card.Header>

          <Card.Content class="pt-3 pb-3 flex flex-col gap-3">
            <div class="flex flex-wrap items-center gap-1.5">
              <Badge variant={authVariant(account.tokenStatus)} class="text-[10px] gap-1 px-2 py-0.5">
                {#if account.tokenStatus === 'valid'}
                  <CheckCircle2 class="size-3" />
                {:else}
                  <AlertCircle class="size-3" />
                {/if}
                {authLabel(account.tokenStatus)}
              </Badge>

              <Badge variant={runtimeVariant(account.runtimeStatus)} class="text-[10px] gap-1 px-2 py-0.5">
                {runtimeLabel(account.runtimeStatus)}
              </Badge>
            </div>

            {#if account.grantedScopes && account.grantedScopes.length > 0}
              <div class="flex flex-wrap items-center gap-1">
                {#each account.grantedScopes as scope}
                  <span class="px-1.5 py-0.5 rounded bg-muted text-[9px] font-mono text-muted-foreground">
                    {scope}
                  </span>
                {/each}
              </div>
            {/if}

            {#if account.lastError}
              <p class="text-[11px] text-destructive flex items-center gap-1 bg-destructive/10 p-2 rounded-md">
                <AlertCircle class="size-3 shrink-0" />
                <span class="truncate">{account.lastError}</span>
              </p>
            {/if}

            <div class="flex items-center justify-between pt-1 border-t border-border/50 text-[11px] text-muted-foreground">
              <span class="flex items-center gap-1">
                <ShieldCheck class="size-3 text-emerald-600" />
                AES-256 Keyring Vault
              </span>
              <Button
                variant="ghost"
                size="sm"
                onclick={() => promptDelete(account)}
                disabled={deleting.has(account.pageId)}
                class="h-7 px-2 text-destructive hover:bg-destructive/10 hover:text-destructive text-xs gap-1"
              >
                <Trash2 class="size-3" />
                Disconnect
              </Button>
            </div>
          </Card.Content>
        </Card.Root>
      {/each}
    </div>
  {/if}
</div>

<!-- Connect Fanpage Modal -->
<Dialog.Root bind:open={connectModalOpen}>
  <Dialog.Content class="max-w-lg">
    <Dialog.Header>
      <Dialog.Title class="flex items-center gap-2 text-base">
        <MessagesSquare class="size-5 text-blue-600" />
        Connect Facebook Fanpage
      </Dialog.Title>
      <Dialog.Description class="text-xs text-muted-foreground">
        Link your Meta Fanpage to enable 2-way Messenger synchronization, post scheduling, and comment automations.
      </Dialog.Description>
    </Dialog.Header>

    <!-- Tab Switcher -->
    <div class="flex border-b border-border mb-3 text-xs">
      <button
        type="button"
        onclick={() => {
          connectTab = 'oauth';
          modalError = null;
        }}
        class="flex-1 py-2 font-medium border-b-2 transition-colors {connectTab === 'oauth' ? 'border-blue-600 text-blue-600' : 'border-transparent text-muted-foreground hover:text-foreground'}"
      >
        Login with Facebook (OAuth)
      </button>
      <button
        type="button"
        onclick={() => {
          connectTab = 'manual';
          oauthStateNonce = null;
          modalError = null;
        }}
        class="flex-1 py-2 font-medium border-b-2 transition-colors {connectTab === 'manual' ? 'border-blue-600 text-blue-600' : 'border-transparent text-muted-foreground hover:text-foreground'}"
      >
        Manual Access Token
      </button>
    </div>

    {#if modalError}
      <div class="flex items-start gap-2 rounded-lg border border-destructive/30 bg-destructive/10 p-2.5 text-xs text-destructive mb-2">
        <AlertCircle class="size-4 shrink-0 mt-0.5" />
        <span>{modalError}</span>
      </div>
    {/if}

    {#if modalSuccess}
      <div class="flex items-start gap-2 rounded-lg border border-emerald-500/30 bg-emerald-500/10 p-2.5 text-xs text-emerald-700 dark:text-emerald-400 mb-2">
        <CheckCircle2 class="size-4 shrink-0 mt-0.5" />
        <span>{modalSuccess}</span>
      </div>
    {/if}

    {#if connectTab === 'oauth'}
      <div class="flex flex-col gap-3 py-1 text-xs">
        <div class="rounded-lg bg-muted/40 p-3 border border-border flex flex-col gap-2">
          <div class="flex items-center justify-between">
            <span class="font-semibold text-foreground">Step 1: Authorize on Facebook</span>
            <Button
              type="button"
              size="sm"
              onclick={handleOpenMetaOAuth}
              class="bg-blue-600 hover:bg-blue-700 text-white text-xs h-7 gap-1 px-2.5"
            >
              <ExternalLink class="size-3" />
              Open Meta Login
            </Button>
          </div>
          <p class="text-[11px] text-muted-foreground">
            Clicking the button opens the official Meta login screen in your browser. Grant permissions to the Fanpages you want to manage.
          </p>
        </div>

        <div class="flex flex-col gap-1.5">
          <label for="oauth-token-input" class="text-xs font-medium text-foreground">
            Step 2: Paste Access Token or Redirect URL
          </label>
          <div class="flex gap-2">
            <Input
              id="oauth-token-input"
              type="text"
              bind:value={oauthTokenOrUrl}
              placeholder="Paste Access Token (EAAB...) or redirected URL"
              class="text-xs flex-1 font-mono"
            />
            <Button
              type="button"
              size="sm"
              onclick={handleDiscoverPages}
              disabled={isFetchingPages || !oauthTokenOrUrl.trim()}
              class="text-xs bg-blue-600 hover:bg-blue-700 text-white shrink-0 gap-1.5"
            >
              {#if isFetchingPages}
                <LoaderCircle class="size-3.5 animate-spin" />
                Scanning...
              {:else}
                <RefreshCw class="size-3.5" />
                Fetch Pages
              {/if}
            </Button>
          </div>
        </div>

        {#if discoveredPages.length > 0}
          <div class="flex flex-col gap-2 mt-2">
            <span class="text-xs font-medium text-foreground">Discovered Fanpages:</span>
            <div class="max-h-48 overflow-y-auto flex flex-col gap-2 pr-1">
              {#each discoveredPages as page (page.id)}
                <div
                  class="flex items-center justify-between p-2.5 rounded-lg border border-border bg-card hover:bg-accent/40 cursor-pointer transition-colors"
                  role="button"
                  tabindex="0"
                  onclick={() => toggleSelectPage(page.id)}
                  onkeydown={(e) => { if (e.key === ' ' || e.key === 'Enter') toggleSelectPage(page.id); }}
                >
                  <div class="flex items-center gap-2.5 min-w-0">
                    <div class="size-5 rounded border border-border flex items-center justify-center {selectedPageIds.has(page.id) ? 'bg-blue-600 text-white border-blue-600' : 'bg-background'}">
                      {#if selectedPageIds.has(page.id)}
                        <Check class="size-3.5 stroke-[3]" />
                      {/if}
                    </div>
                    {#if page.avatarUrl}
                      <img src={page.avatarUrl} alt={page.name} class="size-7 rounded object-cover border border-border" />
                    {:else}
                      <div class="size-7 rounded bg-blue-600 text-white flex items-center justify-center font-bold text-xs">f</div>
                    {/if}
                    <div class="min-w-0">
                      <div class="text-xs font-semibold truncate text-foreground">{page.name}</div>
                      <div class="text-[10px] text-muted-foreground font-mono truncate">ID: {page.id} {page.category ? `• ${page.category}` : ''}</div>
                    </div>
                  </div>
                  <Badge variant="outline" class="text-[9px] px-1.5 py-0 shrink-0">Ready</Badge>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      </div>
    {:else}
      <div class="flex flex-col gap-3 py-1 text-xs">
        <div class="flex flex-col gap-1.5">
          <label for="manual-page-id" class="text-xs font-medium text-foreground">
            Page ID <span class="text-destructive">*</span>
          </label>
          <Input
            id="manual-page-id"
            type="text"
            bind:value={manualPageId}
            placeholder="e.g. 102938475610293"
            class="text-xs font-mono"
          />
        </div>

        <div class="flex flex-col gap-1.5">
          <label for="manual-page-name" class="text-xs font-medium text-foreground">
            Page Name (Optional)
          </label>
          <Input
            id="manual-page-name"
            type="text"
            bind:value={manualPageName}
            placeholder="e.g. RustSale Support Fanpage"
            class="text-xs"
          />
        </div>

        <div class="flex flex-col gap-1.5">
          <label for="manual-token" class="text-xs font-medium text-foreground">
            Page Access Token <span class="text-destructive">*</span>
          </label>
          <Input
            id="manual-token"
            type="password"
            bind:value={manualAccessToken}
            placeholder="EAAB..."
            class="text-xs font-mono"
          />
          <span class="text-[10px] text-muted-foreground">
            Token will be securely encrypted using AES-256-GCM in your OS Keyring Token Vault.
          </span>
        </div>
      </div>
    {/if}

    <Dialog.Footer class="gap-2 sm:gap-0 mt-2">
      <Button variant="outline" size="sm" onclick={() => (connectModalOpen = false)}>
        Cancel
      </Button>
      {#if connectTab === 'oauth'}
        <Button
          size="sm"
          onclick={handleSaveDiscoveredPages}
          disabled={isSavingPages || discoveredPages.length === 0 || selectedPageIds.size === 0}
          class="bg-blue-600 hover:bg-blue-700 text-white gap-1.5 text-xs"
        >
          {#if isSavingPages}
            <LoaderCircle class="size-3.5 animate-spin" />
            Connecting...
          {:else}
            <Check class="size-3.5" />
            Connect Selected Pages ({selectedPageIds.size})
          {/if}
        </Button>
      {:else}
        <Button
          size="sm"
          onclick={handleSaveManual}
          disabled={isSavingPages || !manualPageId.trim() || !manualAccessToken.trim()}
          class="bg-blue-600 hover:bg-blue-700 text-white gap-1.5 text-xs"
        >
          {#if isSavingPages}
            <LoaderCircle class="size-3.5 animate-spin" />
            Connecting...
          {:else}
            <Check class="size-3.5" />
            Save & Connect Page
          {/if}
        </Button>
      {/if}
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Delete Confirmation Modal -->
<Dialog.Root bind:open={deleteConfirmOpen}>
  <Dialog.Content class="max-w-sm">
    <Dialog.Header>
      <Dialog.Title class="text-sm font-semibold text-destructive flex items-center gap-2">
        <AlertCircle class="size-4" />
        Disconnect Facebook Page
      </Dialog.Title>
      <Dialog.Description class="text-xs">
        Are you sure you want to disconnect Page <strong>{pageToDelete?.pageName || pageToDelete?.pageId}</strong>?
        This will remove the stored vault token and pause Messenger synchronization. Your conversation history, customer contacts, and CRM records will remain safely preserved.
      </Dialog.Description>
    </Dialog.Header>
    <Dialog.Footer class="gap-2">
      <Button variant="outline" size="sm" onclick={() => (deleteConfirmOpen = false)}>
        Cancel
      </Button>
      <Button variant="destructive" size="sm" onclick={confirmDelete}>
        Confirm Disconnect
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

