<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import QRCode from 'qrcode';
  import {
    cancelWhatsAppQrLogin,
    disconnectWhatsAppAccount,
    listManagedWhatsAppAccounts,
    startWhatsAppQrLogin,
    type ConnectedWhatsAppAccount,
    type WhatsAppLoginErrorPayload,
    type WhatsAppLoginSuccessPayload,
    type WhatsAppQrCodePayload,
    EVENT_WHATSAPP_LOGIN_ERROR,
    EVENT_WHATSAPP_LOGIN_SUCCESS,
    EVENT_WHATSAPP_QR_CODE
  } from '$lib/services/whatsappService';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import * as Dialog from '$lib/components/ui/dialog';
  import {
    AlertCircle,
    CheckCircle2,
    Clock3,
    LoaderCircle,
    MessageCircleMore,
    Phone,
    QrCode,
    RefreshCw,
    Trash2,
    Wifi,
    WifiOff
  } from 'lucide-svelte';

  let accounts = $state<ConnectedWhatsAppAccount[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let actionError = $state<string | null>(null);
  let disconnecting = $state(new Set<number>());

  // Disconnect Confirmation Dialog State
  let deleteConfirmOpen = $state(false);
  let accountToDisconnect = $state<ConnectedWhatsAppAccount | null>(null);

  // QR Pairing Modal State
  let qrOpen = $state(false);
  let activeAttemptId = $state<string | null>(null);
  let qrCodeDataUrl = $state<string | null>(null);
  let qrSequence = $state<number>(0);
  let qrTimerSeconds = $state<number>(0);
  let qrStarting = $state(false);
  let qrStatus = $state('Initializing WhatsApp session...');
  let qrError = $state<string | null>(null);
  let qrSuccessUser = $state<WhatsAppLoginSuccessPayload | null>(null);

  let qrUnlistens: UnlistenFn[] = [];
  let countdownTimer: ReturnType<typeof setInterval> | null = null;
  let autoCloseTimeout: ReturnType<typeof setTimeout> | null = null;

  function accountName(account: ConnectedWhatsAppAccount): string {
    return (
      account.display_name?.trim() ||
      account.displayName?.trim() ||
      account.phone_number?.trim() ||
      account.phoneNumber?.trim() ||
      account.provider_account_key?.trim() ||
      account.providerAccountKey?.trim() ||
      'WhatsApp Account'
    );
  }

  function accountIdentifier(account: ConnectedWhatsAppAccount): string {
    return (
      account.phone_number?.trim() ||
      account.phoneNumber?.trim() ||
      account.provider_account_key?.trim() ||
      account.providerAccountKey?.trim() ||
      `ID: ${account.id}`
    );
  }

  async function loadAccounts(silent = false) {
    if (!silent) {
      loading = true;
      loadError = null;
    }
    try {
      const result = await listManagedWhatsAppAccounts();
      accounts = Array.isArray(result) ? result : [];
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

  function clearTimers() {
    if (countdownTimer) {
      clearInterval(countdownTimer);
      countdownTimer = null;
    }
    if (autoCloseTimeout) {
      clearTimeout(autoCloseTimeout);
      autoCloseTimeout = null;
    }
  }

  function cleanupQrListeners() {
    clearTimers();
    for (const unlisten of qrUnlistens) {
      try {
        unlisten();
      } catch {
        // ignore unlisten cleanup errors
      }
    }
    qrUnlistens = [];
  }

  export function triggerQrLogin() {
    void beginQrLogin();
  }

  async function generateQrImage(raw: string): Promise<string> {
    if (raw.startsWith('data:image/') || raw.startsWith('http://') || raw.startsWith('https://')) {
      return raw;
    }
    if (raw.startsWith('<svg')) {
      return `data:image/svg+xml;utf8,${encodeURIComponent(raw)}`;
    }
    try {
      return await QRCode.toDataURL(raw, {
        margin: 2,
        width: 256,
        color: {
          dark: '#000000',
          light: '#ffffff'
        }
      });
    } catch {
      return raw;
    }
  }

  async function beginQrLogin() {
    if (qrStarting) return;
    cleanupQrListeners();

    const attemptId = crypto.randomUUID();
    activeAttemptId = attemptId;
    qrOpen = true;
    qrCodeDataUrl = null;
    qrSequence = 0;
    qrTimerSeconds = 0;
    qrError = null;
    qrSuccessUser = null;
    qrStatus = 'Connecting to WhatsApp network...';
    qrStarting = true;

    try {
      const registerQrListener = (unlisten: UnlistenFn): boolean => {
        if (!isMounted || !qrOpen || activeAttemptId !== attemptId) {
          try {
            unlisten();
          } catch {
            // ignore cleanup errors
          }
          return false;
        }
        qrUnlistens.push(unlisten);
        return true;
      };

      const unlistenQr = await listen<WhatsAppQrCodePayload>(EVENT_WHATSAPP_QR_CODE, async (event) => {
        const payload = event.payload;
        const payloadAttemptId = payload.attemptId || payload.attempt_id;
        if (payloadAttemptId && payloadAttemptId !== attemptId) {
          return;
        }

        const raw = payload.qrRaw || payload.qr_raw || '';
        const expiresIn = payload.expiresInSec ?? payload.expires_in_sec ?? 30;
        const seq = payload.sequence ?? 0;

        qrSequence = seq;
        qrTimerSeconds = expiresIn;
        qrCodeDataUrl = await generateQrImage(raw);
        qrStatus = 'Scan this QR code with WhatsApp on your phone (Linked Devices).';
        qrStarting = false;

        if (countdownTimer) {
          clearInterval(countdownTimer);
        }
        countdownTimer = setInterval(() => {
          if (qrTimerSeconds > 0) {
            qrTimerSeconds -= 1;
          } else if (countdownTimer) {
            clearInterval(countdownTimer);
            countdownTimer = null;
          }
        }, 1000);
      });
      if (!registerQrListener(unlistenQr)) return;

      const unlistenSuccess = await listen<WhatsAppLoginSuccessPayload>(EVENT_WHATSAPP_LOGIN_SUCCESS, async (event) => {
        const payload = event.payload;
        const payloadAttemptId = payload.attemptId || payload.attempt_id;
        if (payloadAttemptId && payloadAttemptId !== attemptId) {
          return;
        }

        clearTimers();
        qrSuccessUser = payload;
        qrStatus = 'WhatsApp account paired successfully!';
        qrStarting = false;

        await loadAccounts(true);

        autoCloseTimeout = setTimeout(() => {
          closeQrDialog();
        }, 1500);
      });
      if (!registerQrListener(unlistenSuccess)) return;

      const unlistenError = await listen<WhatsAppLoginErrorPayload>(EVENT_WHATSAPP_LOGIN_ERROR, (event) => {
        const payload = event.payload;
        const payloadAttemptId = payload.attemptId || payload.attempt_id;
        if (payloadAttemptId && payloadAttemptId !== attemptId) {
          return;
        }

        clearTimers();
        qrError = payload.error || 'Failed to complete WhatsApp pairing.';
        qrStatus = qrError;
        qrStarting = false;
      });
      if (!registerQrListener(unlistenError)) return;

      if (!isMounted || !qrOpen || activeAttemptId !== attemptId) return;
      await startWhatsAppQrLogin(attemptId);
    } catch (error) {
      if (!isMounted || activeAttemptId !== attemptId) return;
      clearTimers();
      qrError = error instanceof Error ? error.message : String(error);
      qrStatus = qrError;
      qrStarting = false;
    }
  }

  let isMounted = true;

  function closeQrDialog() {
    qrOpen = false;
    const attemptToCancel = activeAttemptId;
    activeAttemptId = null;
    cleanupQrListeners();

    if (attemptToCancel) {
      cancelWhatsAppQrLogin(attemptToCancel).catch(() => {
        // ignore cancel errors
      });
    }
  }

  function promptDisconnect(account: ConnectedWhatsAppAccount) {
    accountToDisconnect = account;
    deleteConfirmOpen = true;
  }

  async function executeDisconnect() {
    if (!accountToDisconnect) return;
    const target = accountToDisconnect;
    deleteConfirmOpen = false;
    accountToDisconnect = null;

    actionError = null;
    const nextDisconnecting = new Set(disconnecting);
    nextDisconnecting.add(target.id);
    disconnecting = nextDisconnecting;
    try {
      await disconnectWhatsAppAccount(target.id);
      await loadAccounts();
    } catch (error) {
      actionError = error instanceof Error ? error.message : String(error);
    } finally {
      const doneDisconnecting = new Set(disconnecting);
      doneDisconnecting.delete(target.id);
      disconnecting = doneDisconnecting;
    }
  }

  onMount(() => {
    isMounted = true;
    void loadAccounts();
    const pollInterval = setInterval(() => {
      void loadAccounts(true);
    }, 10_000);

    return () => {
      isMounted = false;
      clearInterval(pollInterval);
      cleanupQrListeners();
    };
  });

  onDestroy(() => {
    isMounted = false;
    cleanupQrListeners();
  });
</script>

<div class="flex flex-col gap-4">
  <!-- Header -->
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div>
      <h3 class="text-base font-semibold text-foreground">WhatsApp accounts</h3>
      <p class="text-xs text-muted-foreground">
        Manage connected WhatsApp accounts for customer conversations and outreach campaigns.
      </p>
    </div>
    <div class="flex items-center gap-2">
      <Button variant="outline" size="sm" onclick={() => void loadAccounts()} disabled={loading}>
        <RefreshCw class="size-4 {loading ? 'animate-spin' : ''}" data-icon="inline-start" />
        Refresh
      </Button>
      <Button size="sm" onclick={() => beginQrLogin()}>
        <QrCode class="size-4" data-icon="inline-start" />
        Add WhatsApp account
      </Button>
    </div>
  </div>

  <!-- Alerts -->
  {#if loadError || actionError}
    <div
      role="alert"
      class="flex items-start gap-2 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive"
    >
      <AlertCircle class="mt-0.5 size-4 shrink-0" />
      <span>{loadError || actionError}</span>
    </div>
  {/if}

  <!-- Accounts List or Empty State -->
  {#if loading && accounts.length === 0}
    <Card.Root>
      <Card.Content class="flex items-center justify-center gap-2 py-10 text-sm text-muted-foreground">
        <LoaderCircle class="size-4 animate-spin" /> Loading WhatsApp accounts...
      </Card.Content>
    </Card.Root>
  {:else if accounts.length === 0}
    <Card.Root class="border-dashed">
      <Card.Content class="flex flex-col items-center gap-3 py-10 text-center">
        <div class="rounded-full bg-emerald-500/10 p-3 text-emerald-600">
          <MessageCircleMore class="size-6" />
        </div>
        <div>
          <p class="text-sm font-semibold">No WhatsApp account connected</p>
          <p class="text-xs text-muted-foreground">
            Add an account here to enable WhatsApp messaging and automated outreach campaigns.
          </p>
        </div>
        <Button size="sm" onclick={() => beginQrLogin()}>
          <QrCode class="size-4" data-icon="inline-start" />
          Add WhatsApp account
        </Button>
      </Card.Content>
    </Card.Root>
  {:else}
    <div class="grid gap-3 xl:grid-cols-2">
      {#each accounts as account (account.id)}
        <Card.Root>
          <Card.Header class="gap-3 border-b border-border/70 pb-4">
            <div class="flex items-start justify-between gap-3">
              <div class="flex min-w-0 items-center gap-3">
                <div
                  class="flex size-10 shrink-0 items-center justify-center rounded-full bg-emerald-500/10 font-bold text-emerald-700"
                >
                  {accountName(account).slice(0, 1).toUpperCase()}
                </div>
                <div class="min-w-0">
                  <Card.Title class="truncate text-sm font-semibold">{accountName(account)}</Card.Title>
                  <Card.Description class="truncate font-mono text-[11px] text-muted-foreground">
                    {accountIdentifier(account)}
                  </Card.Description>
                </div>
              </div>
              <div class="flex items-center gap-1 text-emerald-600" title="Connected">
                <Wifi class="size-5 shrink-0" />
              </div>
            </div>
            <div class="flex flex-wrap gap-2">
              <Badge variant="default" class="bg-emerald-600 hover:bg-emerald-600">Connected</Badge>
              <Badge variant="outline" class="gap-1 text-xs">
                <Phone class="size-3" />
                {account.phone_number || account.phoneNumber || 'Linked'}
              </Badge>
            </div>
          </Card.Header>
          <Card.Content class="flex flex-col gap-4 pt-4">
            <div class="min-h-6 text-xs text-muted-foreground">
              <p class="flex items-center gap-2 text-emerald-700">
                <CheckCircle2 class="size-3.5 shrink-0" />
                Realtime session is operational.
              </p>
            </div>
            <div class="flex flex-wrap justify-end gap-2">
              <Button
                variant="ghost"
                size="sm"
                aria-label={`Disconnect ${accountName(account)}`}
                class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                onclick={() => promptDisconnect(account)}
                disabled={disconnecting.has(account.id)}
              >
                <Trash2 class="size-4" />
                Disconnect
              </Button>
            </div>
          </Card.Content>
        </Card.Root>
      {/each}
    </div>
  {/if}
</div>

<!-- QR Pairing Dialog -->
<Dialog.Root bind:open={qrOpen} onOpenChange={(open) => !open && closeQrDialog()}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Connect WhatsApp account</Dialog.Title>
      <Dialog.Description>
        Scan the QR code with WhatsApp on your phone: open Settings &gt; Linked Devices &gt; Link a Device.
      </Dialog.Description>
    </Dialog.Header>

    <div class="flex min-h-72 flex-col items-center justify-center gap-4 py-4 text-center">
      {#if qrError}
        <div class="flex flex-col items-center gap-3">
          <div class="rounded-full bg-destructive/10 p-3 text-destructive">
            <AlertCircle class="size-8" />
          </div>
          <p role="alert" class="max-w-sm text-sm font-medium text-destructive">{qrError}</p>
          <Button variant="outline" size="sm" onclick={() => beginQrLogin()}>Try again</Button>
        </div>
      {:else if qrSuccessUser}
        <div class="flex flex-col items-center gap-3">
          <div class="rounded-full bg-emerald-500/10 p-3 text-emerald-600">
            <CheckCircle2 class="size-8" />
          </div>
          <p class="text-sm font-semibold text-foreground">
            Connected as {qrSuccessUser.pushName || qrSuccessUser.push_name || qrSuccessUser.phoneNumber || qrSuccessUser.phone_number || 'WhatsApp User'}
          </p>
          <p class="text-xs text-muted-foreground">{qrStatus}</p>
        </div>
      {:else if qrCodeDataUrl}
        <div class="flex flex-col items-center gap-3">
          <img
            src={qrCodeDataUrl}
            alt="WhatsApp QR Code"
            class="size-56 rounded-xl border bg-white p-2 shadow-sm"
          />
          <div class="flex items-center gap-2">
            {#if qrSequence > 0}
              <Badge variant="secondary" class="font-mono text-[10px]">
                Code #{qrSequence}
              </Badge>
            {/if}
            {#if qrTimerSeconds > 0}
              <Badge variant="outline" class="gap-1 font-mono text-[10px]">
                <Clock3 class="size-3" />
                {qrTimerSeconds}s
              </Badge>
            {/if}
          </div>
          <p class="text-xs text-muted-foreground">{qrStatus}</p>
        </div>
      {:else}
        <div class="flex flex-col items-center gap-3">
          <LoaderCircle class="size-10 animate-spin text-emerald-600" />
          <p class="text-xs text-muted-foreground">{qrStatus}</p>
        </div>
      {/if}
    </div>

    <Dialog.Footer>
      <Button variant="outline" onclick={closeQrDialog} disabled={qrStarting && !qrError}>
        Cancel
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Disconnect Confirmation Dialog -->
<Dialog.Root bind:open={deleteConfirmOpen} onOpenChange={(open) => !open && (accountToDisconnect = null)}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Disconnect WhatsApp account</Dialog.Title>
      <Dialog.Description>
        Are you sure you want to disconnect {accountToDisconnect ? accountName(accountToDisconnect) : 'this account'}?
        Incoming and outgoing WhatsApp messages will be disabled until you reconnect.
      </Dialog.Description>
    </Dialog.Header>
    <Dialog.Footer class="gap-2 sm:justify-end">
      <Button variant="outline" onclick={() => (deleteConfirmOpen = false)}>
        Cancel
      </Button>
      <Button
        variant="destructive"
        onclick={() => void executeDisconnect()}
        disabled={accountToDisconnect ? disconnecting.has(accountToDisconnect.id) : false}
      >
        Disconnect
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
