<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import {
    deleteFacebookAccount,
    listManagedFacebookAccounts,
    type AccountRuntimeStatus,
    type ManagedFacebookAccount
  } from '$lib/services/accountSettingsService';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import * as Dialog from '$lib/components/ui/dialog';
  import {
    AlertCircle,
    CheckCircle2,
    Clock3,
    ExternalLink,
    LoaderCircle,
    MessagesSquare,
    Plus,
    RefreshCw,
    ShieldCheck,
    Trash2,
    Wifi,
    WifiOff
  } from 'lucide-svelte';
  import { m } from '$lib/i18n';

  let { onNavigateToHub }: { onNavigateToHub?: () => void } = $props();

  let accounts = $state<ManagedFacebookAccount[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let actionError = $state<string | null>(null);
  let deleting = $state(new Set<string>());

  let connectModalOpen = $state(false);
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
    if (!silent) {
      loading = true;
      loadError = null;
    }
    try {
      accounts = await listManagedFacebookAccounts();
      loadError = null;
    } catch (error) {
      if (!silent) {
        loadError = error instanceof Error ? error.message : String(error);
      }
    } finally {
      if (!silent) {
        loading = false;
      }
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
    deleting.add(pageId);
    actionError = null;

    try {
      await deleteFacebookAccount(pageId);
      accounts = accounts.filter((a) => a.pageId !== pageId);
    } catch (error) {
      actionError = error instanceof Error ? error.message : String(error);
    } finally {
      deleting.delete(pageId);
      pageToDelete = null;
    }
  }

  let runtimePoll: ReturnType<typeof setInterval> | null = null;

  onMount(() => {
    void loadAccounts();
    runtimePoll = setInterval(() => {
      void loadAccounts(true);
    }, 5_000);

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
        onclick={() => (connectModalOpen = true)}
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
          onclick={() => (connectModalOpen = true)}
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
  <Dialog.Content class="max-w-md">
    <Dialog.Header>
      <Dialog.Title class="flex items-center gap-2">
        <MessagesSquare class="size-5 text-blue-600" />
        Connect Facebook Page
      </Dialog.Title>
      <Dialog.Description class="text-xs">
        Connect your Meta Fanpage to activate 2-way Messenger synchronization and marketing automations.
      </Dialog.Description>
    </Dialog.Header>

    <div class="flex flex-col gap-3 py-2 text-xs">
      <div class="rounded-lg bg-muted/40 p-3.5 border border-border flex flex-col gap-2">
        <span class="font-semibold text-foreground">How connection works:</span>
        <ol class="list-decimal list-inside space-y-1 text-muted-foreground">
          <li>Authenticate via Zero-Secret PKCE OAuth flow with Meta.</li>
          <li>Page Access Tokens are encrypted using AES-256-GCM in OS Keyring.</li>
          <li>Inbound Messenger webhooks and post comments sync directly into Conversations and Facebook Hub.</li>
        </ol>
      </div>
    </div>

    <Dialog.Footer class="gap-2 sm:gap-0">
      <Button variant="outline" size="sm" onclick={() => (connectModalOpen = false)}>
        Close
      </Button>
      {#if onNavigateToHub}
        <Button
          size="sm"
          onclick={() => {
            connectModalOpen = false;
            onNavigateToHub?.();
          }}
          class="bg-blue-600 hover:bg-blue-700 text-white gap-1.5"
        >
          <ExternalLink class="size-3.5" />
          Open Facebook Hub
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
        This will remove the stored vault token and cease Messenger synchronization.
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
