<script lang="ts">
  import { onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import {
    deleteZaloAccount,
    listManagedZaloAccounts,
    reconnectZaloAccount,
    startZaloQrLogin,
    verifyZaloAccount,
    type AccountRuntimeStatus,
    type ManagedZaloAccount,
    type ZaloAuthStatus
  } from '$lib/services/accountSettingsService';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import * as Dialog from '$lib/components/ui/dialog';
  import {
    AlertCircle,
    CheckCircle2,
    Clock3,
    LoaderCircle,
    QrCode,
    RefreshCw,
    Trash2,
    Wifi,
    WifiOff
  } from 'lucide-svelte';

  let accounts = $state<ManagedZaloAccount[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let actionError = $state<string | null>(null);
  let reconnecting = $state(new Set<string>());
  let verifying = $state(new Set<string>());
  let deleting = $state(new Set<string>());

  let qrOpen = $state(false);
  let qrImage = $state<string | null>(null);
  let qrUser = $state<{ avatar: string; displayName: string } | null>(null);
  let qrStatus = $state('Preparing secure Zalo login...');
  let qrError = $state<string | null>(null);
  let qrStarting = $state(false);
  let qrSuccess = $state(false);
  let qrUnlistens: UnlistenFn[] = [];
  let autoCloseTimeout: ReturnType<typeof setTimeout> | null = null;

  const authLabels: Record<ZaloAuthStatus, string> = {
    unknown: 'Authentication Unknown',
    checking: 'Authentication Checking',
    valid: 'Authentication Valid',
    expired: 'Authentication Expired',
    error: 'Authentication Error'
  };

  const runtimeLabels: Record<AccountRuntimeStatus, string> = {
    not_applicable: 'Runtime Not applicable',
    disconnected: 'Runtime Disconnected',
    connecting: 'Runtime Connecting',
    connected: 'Runtime Connected',
    degraded: 'Runtime Degraded',
    invalid: 'Runtime Invalid'
  };

  function accountName(account: ManagedZaloAccount) {
    return account.displayName?.trim() || account.accountKey;
  }

  function authVariant(status: ZaloAuthStatus): 'default' | 'secondary' | 'outline' | 'destructive' {
    if (status === 'valid') return 'default';
    if (status === 'expired' || status === 'error') return 'destructive';
    return 'secondary';
  }

  function runtimeVariant(status: AccountRuntimeStatus): 'default' | 'secondary' | 'outline' | 'destructive' {
    if (status === 'connected') return 'default';
    if (status === 'invalid') return 'destructive';
    return 'outline';
  }

  async function loadAccounts(silent = false) {
    if (!silent) {
      loading = true;
      loadError = null;
    }
    try {
      accounts = await listManagedZaloAccounts();
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

  function withPending(source: Set<string>, accountKey: string, pending: boolean) {
    const next = new Set(source);
    if (pending) next.add(accountKey);
    else next.delete(accountKey);
    return next;
  }

  async function verifyAccount(account: ManagedZaloAccount) {
    actionError = null;
    verifying = withPending(verifying, account.accountKey, true);
    try {
      await verifyZaloAccount(account.accountKey);
      await loadAccounts();
    } catch (error) {
      actionError = error instanceof Error ? error.message : String(error);
    } finally {
      verifying = withPending(verifying, account.accountKey, false);
    }
  }

  async function reconnectAccount(account: ManagedZaloAccount) {
    actionError = null;
    reconnecting = withPending(reconnecting, account.accountKey, true);
    try {
      await reconnectZaloAccount(account.accountKey);
      await loadAccounts();
    } catch (error) {
      actionError = error instanceof Error ? error.message : String(error);
    } finally {
      reconnecting = withPending(reconnecting, account.accountKey, false);
    }
  }

  async function removeAccount(account: ManagedZaloAccount) {
    actionError = null;
    deleting = withPending(deleting, account.accountKey, true);
    try {
      await deleteZaloAccount(account.accountKey);
      await loadAccounts();
    } catch (error) {
      actionError = error instanceof Error ? error.message : String(error);
    } finally {
      deleting = withPending(deleting, account.accountKey, false);
    }
  }

  function clearTimers() {
    if (autoCloseTimeout) {
      clearTimeout(autoCloseTimeout);
      autoCloseTimeout = null;
    }
  }

  function cleanupQrListeners() {
    clearTimers();
    qrUnlistens.forEach((unlisten) => {
      try {
        unlisten();
      } catch {
        // ignore unlisten cleanup errors
      }
    });
    qrUnlistens = [];
  }

  export function triggerQrLogin() {
    beginQrLogin();
  }

  async function beginQrLogin(targetAccount?: ManagedZaloAccount) {
    if (qrStarting) return;
    cleanupQrListeners();
    qrOpen = true;
    qrImage = null;
    qrUser = null;
    qrError = null;
    qrSuccess = false;
    qrStatus = targetAccount
      ? `Reconnecting ${accountName(targetAccount)}...`
      : 'Connecting to Zalo...';
    qrStarting = true;

    try {
      qrUnlistens.push(
        await listen<{ image_base64: string }>('zalo-qr-code', (event) => {
          qrImage = event.payload.image_base64;
          qrStatus = 'Scan this QR code with the Zalo mobile app.';
          qrStarting = false;
        })
      );
      qrUnlistens.push(
        await listen<{ avatar: string; display_name: string }>('zalo-qr-scanned', (event) => {
          qrUser = {
            avatar: event.payload.avatar,
            displayName: event.payload.display_name
          };
          qrStatus = `Scanned by ${event.payload.display_name}. Confirm the login on your phone.`;
        })
      );
      qrUnlistens.push(
        await listen<{ imei: string }>('zalo-qr-success', async (event) => {
          clearTimers();
          qrSuccess = true;
          qrStarting = false;
          qrStatus = 'Zalo login successful! Realtime connection active.';
          cleanupQrListeners();

          try {
            await reconnectZaloAccount(event.payload.imei);
          } catch (e) {
            console.warn('Zalo post-login reconnect notice:', e);
          }
          await loadAccounts(true);

          autoCloseTimeout = setTimeout(() => {
            closeQrDialog();
          }, 1200);
        })
      );
      qrUnlistens.push(
        await listen<{ message: string }>('zalo-qr-error', (event) => {
          clearTimers();
          qrError = event.payload.message;
          qrStatus = event.payload.message;
          qrStarting = false;
          cleanupQrListeners();
        })
      );
      await startZaloQrLogin();
    } catch (error) {
      clearTimers();
      qrError = error instanceof Error ? error.message : String(error);
      qrStatus = qrError;
      cleanupQrListeners();
    } finally {
      qrStarting = false;
    }
  }

  function closeQrDialog() {
    clearTimers();
    qrOpen = false;
    qrStarting = false;
    qrError = null;
    qrSuccess = false;
    cleanupQrListeners();
  }

  onMount(() => {
    void loadAccounts();
    const runtimePoll = setInterval(() => {
      void loadAccounts(true);
    }, 5_000);

    return () => {
      clearInterval(runtimePoll);
      cleanupQrListeners();
    };
  });
</script>

<div class="flex flex-col gap-4">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div>
      <h3 class="text-base font-semibold text-foreground">Zalo accounts</h3>
      <p class="text-xs text-muted-foreground">Authentication health and realtime listener status are tracked separately.</p>
    </div>
    <div class="flex items-center gap-2">
      <Button variant="outline" size="sm" onclick={() => void loadAccounts()} disabled={loading}>
        <RefreshCw class="size-4 {loading ? 'animate-spin' : ''}" data-icon="inline-start" />
        Refresh
      </Button>
      <Button size="sm" onclick={() => beginQrLogin()}>
        <QrCode class="size-4" data-icon="inline-start" />
        Add Zalo account
      </Button>
    </div>
  </div>

  {#if loadError || actionError}
    <div role="alert" class="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
      {loadError || actionError}
    </div>
  {/if}

  {#if loading && accounts.length === 0}
    <Card.Root>
      <Card.Content class="flex items-center justify-center gap-2 py-10 text-sm text-muted-foreground">
        <LoaderCircle class="size-4 animate-spin" /> Loading Zalo accounts...
      </Card.Content>
    </Card.Root>
  {:else if accounts.length === 0}
    <Card.Root class="border-dashed">
      <Card.Content class="flex flex-col items-center gap-3 py-10 text-center">
        <div class="rounded-full bg-emerald-500/10 p-3 text-emerald-600"><QrCode class="size-6" /></div>
        <div>
          <p class="text-sm font-semibold">No Zalo account configured</p>
          <p class="text-xs text-muted-foreground">Add an account here before using Conversations or Zalo Outreach.</p>
        </div>
        <Button size="sm" onclick={() => beginQrLogin()}>Add Zalo account</Button>
      </Card.Content>
    </Card.Root>
  {:else}
    <div class="grid gap-3 xl:grid-cols-2">
      {#each accounts as account (account.accountKey)}
        <Card.Root>
          <Card.Header class="gap-3 border-b border-border/70 pb-4">
            <div class="flex items-start justify-between gap-3">
              <div class="flex min-w-0 items-center gap-3">
                {#if account.avatar}
                  <img src={account.avatar} alt={accountName(account)} class="size-10 shrink-0 rounded-full border object-cover" />
                {:else}
                  <div class="flex size-10 shrink-0 items-center justify-center rounded-full bg-emerald-500/10 font-semibold text-emerald-700">
                    {accountName(account).slice(0, 1).toUpperCase()}
                  </div>
                {/if}
                <div class="min-w-0">
                  <Card.Title class="truncate text-sm">{accountName(account)}</Card.Title>
                  <Card.Description class="truncate font-mono text-[11px]">{account.accountKey}</Card.Description>
                </div>
              </div>
              {#if account.runtimeStatus === 'connected'}
                <Wifi class="size-5 shrink-0 text-emerald-600" />
              {:else}
                <WifiOff class="size-5 shrink-0 text-muted-foreground" />
              {/if}
            </div>
            <div class="flex flex-wrap gap-2">
              <Badge variant={authVariant(account.authStatus)}>{authLabels[account.authStatus]}</Badge>
              <Badge variant={runtimeVariant(account.runtimeStatus)}>{runtimeLabels[account.runtimeStatus]}</Badge>
            </div>
          </Card.Header>
          <Card.Content class="flex flex-col gap-4 pt-4">
            <div class="min-h-9 text-xs text-muted-foreground">
              {#if account.authError}
                <p class="flex items-start gap-2 text-destructive">
                  <AlertCircle class="mt-0.5 size-3.5 shrink-0" />
                  {account.authError === 'auth_expired_102'
                    ? 'Session expired or active on another device (Error 102). Please re-login.'
                    : account.authError}
                </p>
              {:else if account.lastError}
                <p class="flex items-start gap-2"><AlertCircle class="mt-0.5 size-3.5 shrink-0" /> {account.lastError}</p>
              {:else if account.runtimeStatus === 'connected'}
                <p class="flex items-center gap-2 text-emerald-700"><CheckCircle2 class="size-3.5" /> Realtime listener is operational.</p>
              {:else}
                <p class="flex items-center gap-2"><Clock3 class="size-3.5" /> Waiting for a realtime connection.</p>
              {/if}
            </div>
            <div class="flex flex-wrap justify-end gap-2">
              {#if account.authStatus === 'expired' || account.authStatus === 'error'}
                <Button
                  size="sm"
                  aria-label={`Re-login ${accountName(account)}`}
                  onclick={() => beginQrLogin(account)}
                  disabled={qrStarting}
                >
                  <QrCode class="size-4" data-icon="inline-start" />
                  Re-login
                </Button>
              {/if}
              <Button
                variant="outline"
                size="sm"
                aria-label={`Verify ${accountName(account)}`}
                onclick={() => verifyAccount(account)}
                disabled={verifying.has(account.accountKey)}
              >
                <RefreshCw class="size-4 {verifying.has(account.accountKey) ? 'animate-spin' : ''}" />
                Verify
              </Button>
              {#if account.runtimeStatus !== 'connected'}
                <Button
                  variant="outline"
                  size="sm"
                  aria-label={`Reconnect ${accountName(account)}`}
                  onclick={() => reconnectAccount(account)}
                  disabled={reconnecting.has(account.accountKey)}
                >
                  <Wifi class="size-4" />
                  Reconnect
                </Button>
              {/if}
              <Button
                variant="ghost"
                size="sm"
                aria-label={`Delete ${accountName(account)}`}
                class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                onclick={() => removeAccount(account)}
                disabled={deleting.has(account.accountKey)}
              >
                <Trash2 class="size-4" />
                Delete
              </Button>
            </div>
          </Card.Content>
        </Card.Root>
      {/each}
    </div>
  {/if}
</div>

<Dialog.Root bind:open={qrOpen} onOpenChange={(open) => !open && closeQrDialog()}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Add Zalo account</Dialog.Title>
      <Dialog.Description>Scan once. RustSale will hand realtime ownership to the account supervisor.</Dialog.Description>
    </Dialog.Header>
    <div class="flex min-h-72 flex-col items-center justify-center gap-4 py-4 text-center">
      {#if qrError}
        <div class="flex flex-col items-center gap-3">
          <div class="rounded-full bg-destructive/10 p-3 text-destructive"><AlertCircle class="size-7" /></div>
          <p role="alert" class="max-w-sm text-sm font-medium text-destructive">{qrError}</p>
          <Button variant="outline" size="sm" onclick={() => beginQrLogin()}>Try again</Button>
        </div>
      {:else if qrSuccess}
        <div class="flex flex-col items-center gap-3">
          <div class="rounded-full bg-emerald-500/10 p-3 text-emerald-600">
            <CheckCircle2 class="size-8" />
          </div>
          <p class="text-sm font-semibold text-foreground">
            {qrUser?.displayName ? `Connected as ${qrUser.displayName}` : 'Login Successful'}
          </p>
          <p class="text-xs text-muted-foreground">{qrStatus}</p>
        </div>
      {:else if qrUser}
        <div class="flex flex-col items-center gap-3">
          {#if qrUser.avatar}
            <img src={qrUser.avatar} alt={qrUser.displayName} class="size-16 rounded-full border object-cover" />
          {:else}
            <div class="flex size-16 shrink-0 items-center justify-center rounded-full bg-emerald-500/10 font-semibold text-emerald-700">
              {qrUser.displayName.slice(0, 1).toUpperCase()}
            </div>
          {/if}
          <p class="text-sm font-semibold text-foreground">{qrUser.displayName}</p>
          <p class="text-xs text-muted-foreground">{qrStatus}</p>
        </div>
      {:else if qrImage}
        <img
          src={qrImage.startsWith('data:image/') ? qrImage : `data:image/png;base64,${qrImage}`}
          alt="Zalo QR code"
          class="size-56 rounded-xl border bg-white p-2 shadow-sm"
        />
        <p class="text-xs text-muted-foreground">{qrStatus}</p>
      {:else}
        <LoaderCircle class="size-10 animate-spin text-primary" />
        <p class="text-xs text-muted-foreground">{qrStatus}</p>
      {/if}
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={closeQrDialog} disabled={qrStarting && !qrError}>Close</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
