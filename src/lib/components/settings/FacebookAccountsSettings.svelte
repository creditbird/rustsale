<script module lang="ts">
  export function isAllowedOAuthHost(hostname: string, configuredSupabaseUrl?: string | null): boolean {
    const host = hostname.toLowerCase().trim();
    if (host === 'localhost' || host === '127.0.0.1') {
      return true;
    }
    if (host === 'trantrongtri.com' || host.endsWith('.trantrongtri.com')) {
      return true;
    }
    if (host === 'supabase.co' || host.endsWith('.supabase.co')) {
      return true;
    }
    if (configuredSupabaseUrl) {
      try {
        const configHost = new URL(configuredSupabaseUrl).hostname.toLowerCase();
        if (host === configHost) {
          return true;
        }
      } catch {
        // ignore malformed configured supabase url
      }
    }
    return false;
  }

  export function isValidOAuthRedirectUri(uri: string, configuredSupabaseUrl?: string | null): boolean {
    if (!uri) return false;
    try {
      const parsed = new URL(uri.trim());
      if (parsed.protocol === 'http:') {
        if (parsed.hostname === 'localhost' || parsed.hostname === '127.0.0.1') {
          return true;
        }
        return false;
      }
      if (parsed.protocol === 'https:') {
        return isAllowedOAuthHost(parsed.hostname, configuredSupabaseUrl);
      }
      return false;
    } catch {
      return false;
    }
  }
</script>

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
    ChevronDown,
    ChevronUp,
    Clock3,
    Copy,
    ExternalLink,
    KeyRound,
    LoaderCircle,
    MessagesSquare,
    Plus,
    RefreshCw,
    Settings2,
    ShieldCheck,
    Trash2,
    Wifi,
    WifiOff
  } from 'lucide-svelte';
  import { openExternalUrl } from '$lib/components/conversation/messageMedia';
  import { getSupabaseConfig } from '$lib/services/supabaseClient';

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
    throw new Error('Web Crypto API is required for secure OAuth state generation');
  }

  let showAdvanced = $state(false);

  function openConnectModal() {
    connectTab = 'oauth';
    oauthTokenOrUrl = '';
    oauthStateNonce = null;
    oauthAuthUrl = null;
    copiedAuthUrl = false;
    showAdvanced = false;
    customAppId =
      (typeof localStorage !== 'undefined' ? localStorage.getItem('rustsale_meta_app_id') : null) ||
      import.meta.env.VITE_FACEBOOK_APP_ID ||
      '';
    customRedirectUri = getDefaultRedirectUri();
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

  function openReconnectModal(account: ManagedFacebookAccount) {
    connectTab = 'manual';
    oauthTokenOrUrl = '';
    oauthStateNonce = null;
    oauthAuthUrl = null;
    copiedAuthUrl = false;
    showAdvanced = false;
    customAppId =
      (typeof localStorage !== 'undefined' ? localStorage.getItem('rustsale_meta_app_id') : null) ||
      import.meta.env.VITE_FACEBOOK_APP_ID ||
      '';
    customRedirectUri = getDefaultRedirectUri();
    discoveredPages = [];
    selectedPageIds = new Set();
    modalError = null;
    modalSuccess = null;
    manualPageId = account.pageId;
    manualPageName = account.pageName || '';
    manualAccessToken = '';
    manualAvatarUrl = account.avatarUrl || '';
    connectModalOpen = true;
  }

  function extractTokenAndValidateState(raw: string): { token: string; error?: string } {
    const trimmed = raw.trim();
    if (!trimmed) return { token: '' };

    if (connectTab === 'oauth') {
      if (!oauthStateNonce) {
        return {
          token: '',
          error: "Please click 'Open Meta Login' to initiate authentication session before pasting."
        };
      }
      const stateMatch = trimmed.match(/[#&?]state=([^&]+)/);
      if (stateMatch && stateMatch[1]) {
        const decodedState = decodeURIComponent(stateMatch[1]);
        if (decodedState !== oauthStateNonce) {
          return {
            token: '',
            error: 'OAuth state nonce mismatch. Please re-authenticate to prevent CSRF.'
          };
        }
      }
    }

    const providerTokenMatch = trimmed.match(/[#&?]provider_token=([^&]+)/);
    if (providerTokenMatch && providerTokenMatch[1]) {
      if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
        try {
          const urlObj = new URL(trimmed);
          const { url: sbUrl } = getSupabaseConfig();
          if (!isValidOAuthRedirectUri(urlObj.href, sbUrl)) {
            return {
              token: '',
              error: 'OAuth callback origin is not recognized as a valid secure domain.'
            };
          }
        } catch {
          // ignore parsing error and proceed with token extraction
        }
      }
      return { token: decodeURIComponent(providerTokenMatch[1]) };
    }

    const tokenMatch = trimmed.match(/[#&?]access_token=([^&]+)/);
    if (tokenMatch && tokenMatch[1]) {
      return { token: decodeURIComponent(tokenMatch[1]) };
    }

    return { token: trimmed };
  }

  function getDefaultRedirectUri(): string {
    const { url } = getSupabaseConfig();
    if (typeof localStorage !== 'undefined') {
      const saved = localStorage.getItem('rustsale_facebook_redirect_uri');
      if (saved && isValidOAuthRedirectUri(saved, url)) return saved;
    }
    if (url && isValidOAuthRedirectUri(url, url)) {
      return `${url.replace(/\/$/, '')}/auth/v1/callback`;
    }
    return 'https://rustsale.trantrongtri.com/oauth/callback.html';
  }

  let customAppId = $state(
    (typeof localStorage !== 'undefined' ? localStorage.getItem('rustsale_meta_app_id') : null) ||
      import.meta.env.VITE_FACEBOOK_APP_ID ||
      ''
  );
  let customRedirectUri = $state(getDefaultRedirectUri());
  let copiedRedirectUri = $state(false);
  let oauthAuthUrl = $state<string | null>(null);
  let copiedAuthUrl = $state(false);

  function handleAppIdChange(val: string) {
    customAppId = val.trim();
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('rustsale_meta_app_id', customAppId);
    }
    oauthAuthUrl = null;
    oauthStateNonce = null;
  }

  function handleRedirectUriChange(val: string) {
    const trimmed = val.trim();
    customRedirectUri = trimmed;
    if (typeof localStorage !== 'undefined') {
      const { url } = getSupabaseConfig();
      if (!trimmed || isValidOAuthRedirectUri(trimmed, url)) {
        localStorage.setItem('rustsale_facebook_redirect_uri', trimmed);
      }
    }
    oauthAuthUrl = null;
    oauthStateNonce = null;
  }

  async function handleCopyRedirectUri() {
    try {
      let succeeded = false;
      if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(customRedirectUri);
        succeeded = true;
      } else if (typeof document !== 'undefined') {
        const textarea = document.createElement('textarea');
        textarea.value = customRedirectUri;
        textarea.style.position = 'fixed';
        textarea.style.opacity = '0';
        document.body.appendChild(textarea);
        textarea.select();
        succeeded = document.execCommand('copy');
        document.body.removeChild(textarea);
      }
      if (succeeded) {
        copiedRedirectUri = true;
        setTimeout(() => {
          copiedRedirectUri = false;
        }, 2500);
      } else {
        modalError = 'Failed to copy Redirect URI to clipboard.';
      }
    } catch {
      modalError = 'Failed to copy Redirect URI to clipboard.';
    }
  }

  function getOrGenerateAuthUrl(): string | null {
    const appId = customAppId.trim() || import.meta.env.VITE_FACEBOOK_APP_ID || '';
    if (!appId) {
      modalError = 'Please enter your Meta App ID above (or set VITE_FACEBOOK_APP_ID in .env), or switch to the "Manual Access Token" tab to connect directly without creating an App.';
      return null;
    }
    const { url } = getSupabaseConfig();
    const redirectUri = customRedirectUri.trim() || getDefaultRedirectUri();
    if (!isValidOAuthRedirectUri(redirectUri, url)) {
      modalError = 'Invalid OAuth Redirect URI. Must be a valid HTTPS URL on an approved domain (e.g. Supabase callback *.supabase.co or *.trantrongtri.com).';
      return null;
    }
    const scope =
      'pages_show_list,pages_read_engagement,pages_manage_posts,pages_messaging,pages_manage_engagement,pages_read_user_content,leads_retrieval';
    if (!oauthStateNonce) {
      oauthStateNonce = generateCryptoNonce();
    }
    const authUrl = `https://www.facebook.com/v26.0/dialog/oauth?client_id=${encodeURIComponent(
      appId
    )}&redirect_uri=${encodeURIComponent(
      redirectUri
    )}&scope=${encodeURIComponent(scope)}&response_type=token&state=${encodeURIComponent(
      oauthStateNonce
    )}`;
    oauthAuthUrl = authUrl;
    return authUrl;
  }

  async function handleOpenMetaOAuth() {
    const authUrl = getOrGenerateAuthUrl();
    if (!authUrl) return;
    modalError = null;
    modalSuccess = 'Opened Meta Login in your default browser. Please complete authorization, then copy the address bar URL or access token and paste below.';
    await openExternalUrl(authUrl);
  }

  async function handleCopyAuthUrl() {
    const authUrl = getOrGenerateAuthUrl();
    if (!authUrl) return;
    try {
      if (typeof navigator !== 'undefined' && navigator.clipboard) {
        await navigator.clipboard.writeText(authUrl);
        copiedAuthUrl = true;
        setTimeout(() => {
          copiedAuthUrl = false;
        }, 2500);
      }
    } catch {
      modalError = 'Failed to copy URL to clipboard.';
    }
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
      const pages = await fetchFacebookPagesFromToken(
        cleanToken,
        customAppId.trim() || undefined
      );
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

  function toggleSelectAllPages() {
    if (selectedPageIds.size === discoveredPages.length) {
      selectedPageIds = new Set();
    } else {
      selectedPageIds = new Set(discoveredPages.map((p) => p.id));
    }
  }

  async function handleSaveDiscoveredPages() {
    if (selectedPageIds.size === 0) {
      modalError = 'Please select at least one Fanpage to connect.';
      modalSuccess = null;
      return;
    }

    modalError = null;
    modalSuccess = null;
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
      modalSuccess = null;
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
              <div class="flex items-center gap-1.5">
                {#if account.tokenStatus !== 'valid' || account.runtimeStatus !== 'connected' || account.lastError}
                  <Button
                    variant="outline"
                    size="sm"
                    onclick={() => openReconnectModal(account)}
                    class="h-7 px-2 text-blue-600 border-blue-200 hover:bg-blue-50 dark:hover:bg-blue-950/30 text-xs gap-1"
                  >
                    <KeyRound class="size-3" />
                    Cấp lại quyền
                  </Button>
                {/if}
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
            </div>
          </Card.Content>
        </Card.Root>
      {/each}
    </div>
  {/if}
</div>

<!-- Connect Fanpage Modal -->
<Dialog.Root bind:open={connectModalOpen}>
  <Dialog.Content class="max-w-lg max-h-[85vh] flex flex-col p-0 overflow-hidden" onInteractOutside={(e) => e.preventDefault()}>
    <Dialog.Header class="px-6 pt-5 pb-3 border-b border-border/50 shrink-0">
      <Dialog.Title class="flex items-center gap-2 text-base">
        <MessagesSquare class="size-5 text-blue-600" />
        Connect Facebook Fanpage
      </Dialog.Title>
      <Dialog.Description class="text-xs text-muted-foreground">
        Link your Meta Fanpage to enable 2-way Messenger synchronization, post scheduling, and comment automations.
      </Dialog.Description>

      <!-- Tab Switcher -->
      <div class="flex border-b border-border mt-3 text-xs">
        <button
          type="button"
          onclick={() => {
            connectTab = 'oauth';
            modalError = null;
            modalSuccess = null;
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
            modalSuccess = null;
          }}
          class="flex-1 py-2 font-medium border-b-2 transition-colors {connectTab === 'manual' ? 'border-blue-600 text-blue-600' : 'border-transparent text-muted-foreground hover:text-foreground'}"
        >
          Manual Access Token
        </button>
      </div>
    </Dialog.Header>

    <div class="flex-1 overflow-y-auto px-6 py-3.5 flex flex-col gap-3 min-h-0">
      {#if modalError}
        <div class="flex items-start gap-2 rounded-lg border border-destructive/30 bg-destructive/10 p-2.5 text-xs text-destructive shrink-0">
          <AlertCircle class="size-4 shrink-0 mt-0.5" />
          <span class="break-words leading-relaxed">{modalError}</span>
        </div>
      {:else if modalSuccess}
        <div class="flex items-start gap-2 rounded-lg border border-emerald-500/30 bg-emerald-500/10 p-2.5 text-xs text-emerald-700 dark:text-emerald-400 shrink-0">
          <CheckCircle2 class="size-4 shrink-0 mt-0.5" />
          <span class="break-words leading-relaxed">{modalSuccess}</span>
        </div>
      {/if}

      {#if connectTab === 'oauth'}
        <div class="flex flex-col gap-3.5 text-xs">
          <!-- Step 1: Authorize -->
          <div class="rounded-xl bg-blue-50/60 dark:bg-blue-950/20 p-3.5 border border-blue-200/60 dark:border-blue-800/40 flex flex-col gap-2.5">
            <div class="flex items-center justify-between gap-2">
              <span class="font-semibold text-foreground flex items-center gap-1.5">
                <span class="size-5 rounded-full bg-blue-600 text-white flex items-center justify-center text-[11px] font-bold">1</span>
                Authorize with Facebook
              </span>
              <div class="flex items-center gap-1.5 shrink-0">
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onclick={handleCopyAuthUrl}
                  class="text-xs h-7 gap-1 px-2 border-border bg-background"
                  title="Copy authorization link to paste into your browser"
                >
                  {#if copiedAuthUrl}
                    <Check class="size-3 text-emerald-600 dark:text-emerald-400" />
                    Copied
                  {:else}
                    <Copy class="size-3" />
                    Copy Link
                  {/if}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  onclick={handleOpenMetaOAuth}
                  class="bg-blue-600 hover:bg-blue-700 text-white text-xs h-7 gap-1.5 px-3 shadow-sm"
                >
                  <ExternalLink class="size-3" />
                  Open Meta Login
                </Button>
              </div>
            </div>
            <p class="text-[11px] text-muted-foreground leading-relaxed">
              Click <strong>Open Meta Login</strong> to log in with your Facebook account and select the Fanpages you want to grant permissions to.
            </p>
          </div>

          <!-- Step 2: Paste Token or URL -->
          <div class="rounded-xl bg-muted/30 p-3.5 border border-border flex flex-col gap-2.5">
            <span class="font-semibold text-foreground flex items-center gap-1.5">
              <span class="size-5 rounded-full bg-slate-700 dark:bg-slate-300 text-white dark:text-slate-900 flex items-center justify-center text-[11px] font-bold">2</span>
              Paste Access Token or Callback URL
            </span>
            <div class="flex gap-2">
              <Input
                id="oauth-token-input"
                type="text"
                bind:value={oauthTokenOrUrl}
                placeholder="Paste Access Token (EAAB...) or redirected address URL"
                class="text-xs flex-1 font-mono bg-background"
              />
              <Button
                type="button"
                size="sm"
                onclick={handleDiscoverPages}
                disabled={isFetchingPages || !oauthTokenOrUrl.trim()}
                class="text-xs bg-blue-600 hover:bg-blue-700 text-white shrink-0 gap-1.5 px-3"
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

          <!-- Discovered Pages -->
          {#if discoveredPages.length > 0}
            <div class="flex flex-col gap-2 p-3 rounded-xl border border-border bg-card">
              <div class="flex items-center justify-between">
                <span class="text-xs font-semibold text-foreground flex items-center gap-1.5">
                  <CheckCircle2 class="size-3.5 text-emerald-600" />
                  Discovered Fanpages ({discoveredPages.length})
                </span>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onclick={toggleSelectAllPages}
                  class="h-6 px-2 text-[11px] text-muted-foreground hover:text-foreground"
                >
                  {selectedPageIds.size === discoveredPages.length ? 'Deselect All' : 'Select All'}
                </Button>
              </div>

              <div class="max-h-48 overflow-y-auto flex flex-col gap-1.5 pr-0.5">
                {#each discoveredPages as page (page.id)}
                  <div
                    class="flex items-center justify-between p-2 rounded-lg border transition-all cursor-pointer {selectedPageIds.has(page.id) ? 'border-blue-500 bg-blue-50/40 dark:bg-blue-950/20 shadow-xs' : 'border-border/70 hover:bg-accent/40'}"
                    role="button"
                    tabindex="0"
                    onclick={() => toggleSelectPage(page.id)}
                    onkeydown={(e) => { if (e.key === ' ' || e.key === 'Enter') toggleSelectPage(page.id); }}
                  >
                    <div class="flex items-center gap-2.5 min-w-0">
                      <div class="size-4.5 rounded border flex items-center justify-center shrink-0 {selectedPageIds.has(page.id) ? 'bg-blue-600 text-white border-blue-600' : 'border-border bg-background'}">
                        {#if selectedPageIds.has(page.id)}
                          <Check class="size-3 stroke-[3]" />
                        {/if}
                      </div>
                      {#if page.avatarUrl}
                        <img src={page.avatarUrl} alt={page.name} class="size-7 rounded-full object-cover border border-border shrink-0" />
                      {:else}
                        <div class="size-7 rounded-full bg-blue-600 text-white flex items-center justify-center font-bold text-xs shrink-0">f</div>
                      {/if}
                      <div class="min-w-0">
                        <div class="text-xs font-semibold truncate text-foreground">{page.name}</div>
                        <div class="text-[10px] text-muted-foreground font-mono truncate">
                          ID: {page.id} {page.category ? `• ${page.category}` : ''}
                        </div>
                      </div>
                    </div>
                    <Badge variant="outline" class="text-[9px] px-1.5 py-0 shrink-0">Ready</Badge>
                  </div>
                {/each}
              </div>
            </div>
          {/if}

          <!-- Advanced Configuration Accordion -->
          <div class="border-t border-border/50 pt-2 flex flex-col gap-2">
            <button
              type="button"
              onclick={() => (showAdvanced = !showAdvanced)}
              class="w-full flex items-center justify-between py-1 px-2 text-[11px] text-muted-foreground hover:text-foreground rounded hover:bg-muted/40 transition-colors"
            >
              <span class="flex items-center gap-1.5 font-medium">
                <Settings2 class="size-3 text-muted-foreground" />
                Advanced Settings (App ID & OAuth Redirect URI)
              </span>
              {#if showAdvanced}
                <ChevronUp class="size-3.5 text-muted-foreground" />
              {:else}
                <ChevronDown class="size-3.5 text-muted-foreground" />
              {/if}
            </button>

            {#if showAdvanced}
              <div class="flex flex-col gap-2.5 p-3 rounded-lg bg-muted/20 border border-border/60">
                <!-- Meta App ID -->
                <div class="flex flex-col gap-1">
                  <div class="flex items-center justify-between">
                    <label for="meta-app-id-input" class="text-[11px] font-medium text-foreground">
                      Meta App ID
                    </label>
                    <a
                      href="https://developers.facebook.com/apps"
                      target="_blank"
                      rel="noopener noreferrer"
                      class="text-[10px] text-blue-600 dark:text-blue-400 hover:underline flex items-center gap-0.5"
                    >
                      developers.facebook.com
                      <ExternalLink class="size-2.5" />
                    </a>
                  </div>
                  <Input
                    id="meta-app-id-input"
                    type="text"
                    bind:value={customAppId}
                    oninput={(e) => handleAppIdChange(e.currentTarget.value)}
                    placeholder="Enter App ID (e.g. 1585399209632312)"
                    class="text-xs font-mono h-8"
                  />
                </div>

                <!-- OAuth Redirect URI -->
                <div class="flex flex-col gap-1">
                  <div class="flex items-center justify-between">
                    <label for="redirect-uri-input" class="text-[11px] font-medium text-foreground">
                      Valid OAuth Redirect URI
                    </label>
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      onclick={handleCopyRedirectUri}
                      class="h-5 px-1.5 text-[10px] text-blue-600 dark:text-blue-400 gap-1 hover:bg-transparent"
                      title="Copy to paste into Meta Developer -> Valid OAuth Redirect URIs"
                    >
                      {#if copiedRedirectUri}
                        <Check class="size-2.5 text-emerald-600 dark:text-emerald-400" />
                        Copied
                      {:else}
                        <Copy class="size-2.5" />
                        Copy Callback URL
                      {/if}
                    </Button>
                  </div>
                  <Input
                    id="redirect-uri-input"
                    type="text"
                    bind:value={customRedirectUri}
                    oninput={(e) => handleRedirectUriChange(e.currentTarget.value)}
                    placeholder="https://<project-ref>.supabase.co/auth/v1/callback"
                    class="text-xs font-mono h-8"
                  />
                  <span class="text-[9px] text-muted-foreground">
                    Must match an exact entry in <strong>Meta Developer ➜ Facebook Login ➜ Valid OAuth Redirect URIs</strong>.
                  </span>
                </div>
              </div>
            {/if}
          </div>
        </div>
      {:else}
        <div class="flex flex-col gap-3 text-xs">
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
    </div>

    <Dialog.Footer class="px-6 py-3 border-t border-border bg-muted/20 flex items-center justify-between shrink-0">
      <Button variant="outline" size="sm" onclick={() => (connectModalOpen = false)}>
        Cancel
      </Button>
      {#if connectTab === 'oauth'}
        <Button
          size="sm"
          onclick={handleSaveDiscoveredPages}
          disabled={isSavingPages || discoveredPages.length === 0 || selectedPageIds.size === 0}
          class="bg-blue-600 hover:bg-blue-700 text-white gap-1.5 text-xs shadow-sm"
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
          class="bg-blue-600 hover:bg-blue-700 text-white gap-1.5 text-xs shadow-sm"
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

