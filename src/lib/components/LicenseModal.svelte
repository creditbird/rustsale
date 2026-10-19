<script lang="ts">
  import { onMount } from 'svelte';
  import {
    getHardwareInfo,
    getCurrentUser,
    loginWithEmail,
    registerWithEmail,
    logout,
    verifyDeviceAndSubscription,
    getUserDevices,
    revokeDevice,
    createPaymentOrder,
    checkPaymentStatus,
    getSubscriptionPlans,
    type HardwareInfo,
    type UserDevice,
    type DeviceVerificationResult,
    type PaymentTransaction,
    type SubscriptionPlan
  } from '$lib/services/licenseService';

  import {
    ShieldCheck,
    ShieldAlert,
    Laptop,
    Key,
    User,
    LogOut,
    QrCode,
    Sparkles,
    CheckCircle2,
    XCircle,
    RefreshCw,
    Copy,
    Check,
    Clock
  } from 'lucide-svelte';

  interface Props {
    open: boolean;
    preventClose?: boolean;
    onClose?: () => void;
    onVerified?: () => void;
  }

  let { open = $bindable(false), preventClose = false, onClose, onVerified }: Props = $props();

  let activeTab = $state<'auth' | 'status' | 'devices' | 'upgrade'>('status');
  let isLoading = $state(false);
  let errorMsg = $state<string | null>(null);
  let successMsg = $state<string | null>(null);

  // User & License State
  let user = $state<any>(null);
  let hwInfo = $state<HardwareInfo | null>(null);
  let verification = $state<DeviceVerificationResult | null>(null);
  let devices = $state<UserDevice[]>([]);

  // Auth Inputs
  let authEmail = $state('');
  let authPassword = $state('');
  let isRegisterMode = $state(false);

  // Upgrade & Payment State
  let selectedPlan = $state<string>('pro');
  let availablePlans = $state<SubscriptionPlan[]>([
    {
      id: 'pro',
      name: 'PRO Plan',
      price: 199000,
      duration_days: 30,
      max_devices: 3,
      description: 'Includes: 3 Devices + Full Scraper & Zalo/Email Outreach features.',
      badge_text: 'PRO Plan',
      is_active: true,
      sort_order: 1
    },
    {
      id: 'enterprise',
      name: 'ENTERPRISE Plan',
      price: 499000,
      duration_days: 30,
      max_devices: 10,
      description: 'Includes: 10 Devices + Priority Scraper speed & 24/7 Support.',
      badge_text: 'ENTERPRISE Plan',
      is_active: true,
      sort_order: 2
    }
  ]);
  let currentTransaction = $state<PaymentTransaction | null>(null);
  let pollTimer = $state<ReturnType<typeof setInterval> | null>(null);
  let copiedHwid = $state(false);
  let copiedDes = $state(false);

  onMount(() => {
    initLicenseState();
    loadPlans();
    return () => {
      stopPaymentPolling();
    };
  });

  async function loadPlans() {
    try {
      const fetched = await getSubscriptionPlans();
      if (fetched && fetched.length > 0) {
        availablePlans = fetched;
        if (!availablePlans.some(p => p.id === selectedPlan)) {
          selectedPlan = availablePlans[0].id;
        }
      }
    } catch (e) {
      console.warn('Could not load subscription plans from Supabase:', e);
    }
  }

  function getErrorMessage(err: unknown): string {
    if (err instanceof Error) return err.message;
    if (typeof err === 'object' && err !== null) {
      if ('message' in err && typeof (err as any).message === 'string') {
        return (err as any).message;
      }
      if ('error_description' in err && typeof (err as any).error_description === 'string') {
        return (err as any).error_description;
      }
    }
    if (typeof err === 'string') return err;
    return 'An unknown error occurred.';
  }

  async function initLicenseState() {
    isLoading = true;
    errorMsg = null;
    try {
      hwInfo = await getHardwareInfo();
      user = await getCurrentUser();

      if (!user) {
        activeTab = 'auth';
      } else {
        await refreshVerification();
      }
    } catch (err) {
      errorMsg = getErrorMessage(err);
    } finally {
      isLoading = false;
    }
  }

  async function refreshVerification() {
    isLoading = true;
    errorMsg = null;
    try {
      verification = await verifyDeviceAndSubscription();
      devices = await getUserDevices();

      if (verification.allowed && onVerified) {
        onVerified();
      } else if (!verification.allowed) {
        if (verification.reason === 'unauthenticated') {
          activeTab = 'auth';
        } else if (verification.reason === 'subscription_expired' || verification.reason === 'device_limit_exceeded') {
          activeTab = 'upgrade';
        }
      }
    } catch (err) {
      errorMsg = getErrorMessage(err);
    } finally {
      isLoading = false;
    }
  }

  async function handleAuth() {
    if (!authEmail || !authPassword) {
      errorMsg = 'Please enter both Email and Password.';
      return;
    }
    isLoading = true;
    errorMsg = null;
    try {
      if (isRegisterMode) {
        user = await registerWithEmail(authEmail, authPassword);
        successMsg = 'Registration successful! Please sign in.';
        isRegisterMode = false;
      } else {
        user = await loginWithEmail(authEmail, authPassword);
        successMsg = 'Signed in successfully!';
        await refreshVerification();
        activeTab = 'status';
      }
    } catch (err) {
      errorMsg = getErrorMessage(err);
    } finally {
      isLoading = false;
    }
  }

  async function handleLogout() {
    await logout();
    user = null;
    verification = null;
    devices = [];
    activeTab = 'auth';
  }

  async function handleRevokeDevice(deviceId: string) {
    if (!confirm('Are you sure you want to revoke and deactivate this device?')) return;
    isLoading = true;
    try {
      const ok = await revokeDevice(deviceId);
      if (ok) {
        successMsg = 'Device revoked successfully.';
        await refreshVerification();
      } else {
        errorMsg = 'Failed to revoke device.';
      }
    } finally {
      isLoading = false;
    }
  }

  async function handleCreatePayment() {
    isLoading = true;
    errorMsg = null;
    try {
      const targetPlan = availablePlans.find(p => p.id === selectedPlan);
      const price = targetPlan ? targetPlan.price : undefined;
      const duration = targetPlan ? targetPlan.duration_days : 30;
      currentTransaction = await createPaymentOrder(selectedPlan, price, duration);
      startPaymentPolling(currentTransaction.order_code);
    } catch (err) {
      errorMsg = getErrorMessage(err);
    } finally {
      isLoading = false;
    }
  }

  function startPaymentPolling(orderCode: number) {
    if (pollTimer) clearInterval(pollTimer);
    pollTimer = setInterval(async () => {
      const tx = await checkPaymentStatus(orderCode);
      if (tx && tx.status === 'completed') {
        if (pollTimer) clearInterval(pollTimer);
        pollTimer = null;
        successMsg = 'Payment successful! Your subscription and license validity have been renewed.';
        currentTransaction = null;
        await refreshVerification();
        activeTab = 'status';
      }
    }, 3000);
  }

  function stopPaymentPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  function resetPaymentState() {
    stopPaymentPolling();
    currentTransaction = null;
  }

  function handleSelectPlan(planId: string) {
    selectedPlan = planId;
    if (currentTransaction && currentTransaction.plan_id !== planId) {
      resetPaymentState();
    }
  }

  function copyHwid() {
    if (hwInfo?.hardware_id) {
      navigator.clipboard.writeText(hwInfo.hardware_id);
      copiedHwid = true;
      setTimeout(() => (copiedHwid = false), 2000);
    }
  }

  function handleClose() {
    if (preventClose) return;
    stopPaymentPolling();
    open = false;
    if (onClose) onClose();
  }
</script>

{#if open}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-xs p-4 animate-in fade-in duration-200">
    <div class="relative w-full max-w-2xl bg-card border border-border rounded-2xl shadow-2xl overflow-hidden flex flex-col text-card-foreground max-h-[90vh]">
      
      <!-- Header -->
      <div class="px-6 py-4 border-b border-border flex items-center justify-between bg-muted/30">
        <div class="flex items-center gap-3">
          <div class="p-2 rounded-xl bg-emerald-500/10 text-emerald-600 border border-emerald-500/20">
            <ShieldCheck class="w-6 h-6" />
          </div>
          <div>
            <h2 class="text-lg font-bold text-foreground tracking-tight">License & HWID Management</h2>
            <p class="text-xs text-muted-foreground">RustSale V2 Hardware ID & License Manager</p>
          </div>
        </div>

        {#if !preventClose}
          <button
            onclick={handleClose}
            class="text-muted-foreground hover:text-foreground p-2 rounded-lg hover:bg-muted transition-colors"
          >
            ✕
          </button>
        {:else}
          <div class="px-2.5 py-1 rounded-md bg-amber-500/10 text-amber-600 border border-amber-500/20 text-xs font-semibold">
            Activation Required
          </div>
        {/if}
      </div>

      <!-- Navigation Tabs -->
      <div class="flex border-b border-border bg-muted/20 px-6 gap-2 pt-2">
        <button
          onclick={() => (activeTab = 'status')}
          class="px-4 py-2 text-xs font-semibold rounded-t-lg transition-colors flex items-center gap-2 border-b-2 {activeTab === 'status' ? 'border-emerald-600 text-emerald-600 bg-card shadow-2xs font-bold' : 'border-transparent text-muted-foreground hover:text-foreground'}"
        >
          <ShieldCheck class="w-4 h-4" /> Status
        </button>
        <button
          onclick={() => (activeTab = 'devices')}
          class="px-4 py-2 text-xs font-semibold rounded-t-lg transition-colors flex items-center gap-2 border-b-2 {activeTab === 'devices' ? 'border-emerald-600 text-emerald-600 bg-card shadow-2xs font-bold' : 'border-transparent text-muted-foreground hover:text-foreground'}"
        >
          <Laptop class="w-4 h-4" /> Devices ({devices.length})
        </button>
        <button
          onclick={() => (activeTab = 'upgrade')}
          class="px-4 py-2 text-xs font-semibold rounded-t-lg transition-colors flex items-center gap-2 border-b-2 {activeTab === 'upgrade' ? 'border-emerald-600 text-emerald-600 bg-card shadow-2xs font-bold' : 'border-transparent text-muted-foreground hover:text-foreground'}"
        >
          <Sparkles class="w-4 h-4 text-amber-500" /> Upgrade / Payment
        </button>
        <button
          onclick={() => (activeTab = 'auth')}
          class="px-4 py-2 text-xs font-semibold rounded-t-lg transition-colors flex items-center gap-2 border-b-2 {activeTab === 'auth' ? 'border-emerald-600 text-emerald-600 bg-card shadow-2xs font-bold' : 'border-transparent text-muted-foreground hover:text-foreground'}"
        >
          <User class="w-4 h-4" /> Account
        </button>

      </div>

      <!-- Content Area -->
      <div class="p-6 overflow-y-auto space-y-4 flex-1 bg-card">

        {#if preventClose}
          <div class="p-3.5 bg-amber-500/10 border border-amber-500/20 text-amber-700 dark:text-amber-300 text-xs rounded-xl flex items-center gap-3 shadow-2xs">
            <ShieldAlert class="w-5 h-5 shrink-0 text-amber-500" />
            <div>
              <div class="font-bold text-amber-800 dark:text-amber-200 text-xs">Login & License Activation Required</div>
              <div class="text-[11px] text-amber-700/80 dark:text-amber-300/80">Please sign in or renew your device subscription to continue accessing RustSale.</div>
            </div>
          </div>
        {/if}

        {#if errorMsg}
          <div class="p-3 bg-destructive/10 border border-destructive/20 text-destructive text-xs rounded-xl flex items-center gap-2">
            <ShieldAlert class="w-4 h-4 shrink-0" />
            <span>{errorMsg}</span>
          </div>
        {/if}

        {#if successMsg}
          <div class="p-3 bg-emerald-500/10 border border-emerald-500/20 text-emerald-600 dark:text-emerald-300 text-xs rounded-xl flex items-center gap-2">
            <CheckCircle2 class="w-4 h-4 shrink-0" />
            <span>{successMsg}</span>
          </div>
        {/if}

        <!-- TAB: STATUS -->
        {#if activeTab === 'status'}
          <div class="space-y-4">
            <!-- Hardware Card -->
            <div class="p-4 rounded-xl bg-muted/40 border border-border space-y-3">
              <div class="flex items-center justify-between">
                <span class="text-xs font-medium text-muted-foreground">Hardware ID (HWID):</span>
                <button
                  onclick={copyHwid}
                  class="text-xs text-emerald-600 hover:text-emerald-700 flex items-center gap-1 bg-emerald-500/10 px-2 py-1 rounded-md border border-emerald-500/20 transition-colors"
                >
                  {#if copiedHwid}
                    <Check class="w-3 h-3 text-emerald-600" /> Copied
                  {:else}
                    <Copy class="w-3 h-3" /> Copy HWID
                  {/if}
                </button>
              </div>
              <div class="font-mono text-sm font-bold text-emerald-700 dark:text-emerald-400 bg-background px-3 py-2 rounded-lg border border-border shadow-2xs">
                {hwInfo?.hardware_id || 'Loading HWID...'}
              </div>
              <div class="flex text-xs text-muted-foreground gap-4">
                <span>Hostname: <strong class="text-foreground font-semibold">{hwInfo?.hostname}</strong></span>
                <span>OS: <strong class="text-foreground font-semibold">{hwInfo?.os_info}</strong></span>
              </div>
            </div>

            <!-- License Status Banner -->
            {#if verification}
              {#if verification.allowed}
                <div class="p-4 rounded-xl bg-emerald-500/10 border border-emerald-500/20 space-y-2 text-emerald-900 dark:text-emerald-100">
                  <div class="flex items-center gap-2 text-emerald-600 dark:text-emerald-400 font-bold text-sm">
                    <CheckCircle2 class="w-5 h-5" />
                    <span>Valid License - Access Granted</span>
                  </div>
                  <div class="grid grid-cols-2 gap-2 text-xs text-muted-foreground pt-2 border-t border-emerald-500/10">
                    <div>Subscription Plan: <span class="font-bold text-foreground uppercase">{verification.plan_id}</span></div>
                    <div>Valid Until: <span class="font-bold text-amber-600 dark:text-amber-400">{new Date(verification.expires_at || '').toLocaleDateString('en-US')}</span></div>
                    <div>Allowed Devices: <span class="font-bold text-foreground">{verification.max_devices} devices</span></div>
                  </div>
                </div>
              {:else}
                <div class="p-4 rounded-xl bg-amber-500/10 border border-amber-500/20 space-y-2">
                  <div class="flex items-center gap-2 text-amber-600 dark:text-amber-400 font-bold text-sm">
                    <ShieldAlert class="w-5 h-5" />
                    <span>
                      {#if verification.reason === 'subscription_expired'}
                        Subscription plan has expired
                      {:else if verification.reason === 'device_limit_exceeded'}
                        Device limit exceeded ({verification.current_devices}/{verification.max_devices} devices)
                      {:else}
                        Unverified license
                      {/if}
                    </span>
                  </div>
                  <p class="text-xs text-muted-foreground">
                    Please renew your subscription or revoke old devices in the "Devices" tab.
                  </p>
                </div>
              {/if}
            {/if}

            <div class="flex justify-end pt-2">
              <button
                onclick={refreshVerification}
                disabled={isLoading}
                class="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 text-white font-medium text-xs rounded-xl flex items-center gap-2 transition-all disabled:opacity-50 shadow-2xs"
              >
                <RefreshCw class="w-3.5 h-3.5 {isLoading ? 'animate-spin' : ''}" /> Re-check
              </button>
            </div>
          </div>
        {/if}

        <!-- TAB: DEVICES -->
        {#if activeTab === 'devices'}
          <div class="space-y-3">
            <p class="text-xs text-muted-foreground">List of devices currently linked to this account:</p>
            {#each devices as dev (dev.id)}
              <div class="p-3 bg-muted/30 border border-border rounded-xl flex items-center justify-between">
                <div class="flex items-center gap-3">
                  <Laptop class="w-5 h-5 text-emerald-600 shrink-0" />
                  <div>
                    <div class="text-xs font-bold text-foreground flex items-center gap-2">
                      {dev.device_name}
                      {#if hwInfo && dev.hardware_id === hwInfo.hardware_id}
                        <span class="text-[10px] bg-emerald-500/10 text-emerald-600 px-2 py-0.5 rounded-md border border-emerald-500/20 font-semibold">Current Device</span>
                      {/if}
                    </div>
                    <div class="font-mono text-[11px] text-muted-foreground">{dev.hardware_id}</div>
                    <div class="text-[10px] text-muted-foreground/80">Last Active: {new Date(dev.last_active).toLocaleString('en-US')}</div>
                  </div>
                </div>

                <button
                  onclick={() => handleRevokeDevice(dev.id)}
                  disabled={isLoading}
                  class="px-3 py-1.5 text-xs text-destructive bg-destructive/10 hover:bg-destructive/20 border border-destructive/20 rounded-lg transition-colors font-medium"
                >
                  Revoke Device
                </button>
              </div>
            {:else}
              <div class="text-center py-6 text-muted-foreground text-xs">No devices registered yet.</div>
            {/each}
          </div>
        {/if}

        <!-- TAB: UPGRADE / VIETQR -->
        {#if activeTab === 'upgrade'}
          <div class="space-y-4">
            <div class="grid grid-cols-2 gap-3">
              {#each availablePlans as plan (plan.id)}
                <button
                  type="button"
                  onclick={() => handleSelectPlan(plan.id)}
                  class="p-4 rounded-xl border text-left transition-all relative overflow-hidden {selectedPlan === plan.id ? 'border-emerald-500 bg-emerald-500/5 text-foreground ring-1 ring-emerald-500' : 'border-border bg-card text-muted-foreground hover:border-border/80'}"
                >
                  <div class="text-sm font-bold text-emerald-600 dark:text-emerald-400">{plan.name}</div>
                  <div class="text-lg font-black text-foreground py-1">{plan.price.toLocaleString('en-US')} VND <span class="text-xs text-muted-foreground font-normal">/ {plan.duration_days} days</span></div>
                  <div class="text-xs text-muted-foreground">{plan.description}</div>
                </button>
              {/each}
            </div>

            {#if !currentTransaction}
              {@const targetPlan = availablePlans.find(p => p.id === selectedPlan)}
              <button
                type="button"
                onclick={handleCreatePayment}
                disabled={isLoading}
                class="w-full py-3 bg-emerald-600 hover:bg-emerald-500 text-white font-bold text-xs rounded-xl shadow-md flex items-center justify-center gap-2 transition-all"
              >
                <QrCode class="w-4 h-4" /> Generate Payment Order ({targetPlan?.name || 'Selected Plan'} - {targetPlan?.price.toLocaleString('en-US')} VND)
              </button>
            {:else}
              {@const currentPlanObj = availablePlans.find(p => p.id === currentTransaction?.plan_id)}
              <!-- QR Payment Display Modal -->
              <div class="p-4 bg-muted/30 border border-border rounded-xl space-y-3 text-center">
                <div class="text-xs font-semibold text-foreground">
                  Scan QR code to pay <span class="text-emerald-600 dark:text-emerald-400 font-bold">({currentPlanObj?.name || currentTransaction.plan_id.toUpperCase()})</span>:
                </div>
                <div class="flex justify-center p-2 bg-white rounded-xl inline-block border border-border shadow-2xs">
                  <img src={currentTransaction.qr_code_url} alt="Payment QR" class="w-56 h-56 object-contain" />
                </div>
                <div class="text-xs text-muted-foreground space-y-1">
                  <div>Order Code: <strong class="text-amber-600 dark:text-amber-400 font-mono">{currentTransaction.order_code}</strong></div>
                  <div>Amount: <strong class="text-emerald-600 dark:text-emerald-400 font-bold">{currentTransaction.amount.toLocaleString()} VND</strong></div>
                  <div class="flex items-center justify-center gap-1 pt-1">
                    <span>Transfer Memo:</span>
                    <strong class="text-blue-600 dark:text-blue-400 font-mono font-bold">RS SEVQR TKPRS {currentTransaction.order_code}</strong>
                    <button
                      type="button"
                      onclick={() => {
                        if (currentTransaction?.order_code) {
                          navigator.clipboard.writeText(`RS SEVQR TKPRS ${currentTransaction.order_code}`);
                          copiedDes = true;
                          setTimeout(() => (copiedDes = false), 2000);
                        }
                      }}
                      class="p-1 hover:bg-muted rounded text-xs transition-colors"
                      title="Copy transfer memo"
                    >
                      {#if copiedDes}
                        <Check class="w-3.5 h-3.5 text-emerald-500" />
                      {:else}
                        <Copy class="w-3.5 h-3.5" />
                      {/if}
                    </button>
                  </div>
                </div>
                <div class="flex items-center justify-center gap-2 text-xs text-emerald-600 animate-pulse pt-2 font-medium">
                  <RefreshCw class="w-3.5 h-3.5 animate-spin" /> Waiting for automatic payment confirmation...
                </div>

                <div class="pt-2 border-t border-border/50 flex justify-center">
                  <button
                    type="button"
                    onclick={resetPaymentState}
                    class="text-xs text-destructive hover:bg-destructive/10 px-3 py-1.5 rounded-lg border border-destructive/20 transition-colors font-medium flex items-center gap-1"
                  >
                    ✕ Cancel this order & choose another plan
                  </button>
                </div>
              </div>
            {/if}
          </div>
        {/if}

        <!-- TAB: AUTH -->
        {#if activeTab === 'auth'}
          <div class="space-y-4">
            {#if user}
              <div class="p-4 bg-muted/30 border border-border rounded-xl flex items-center justify-between">
                <div>
                  <div class="text-xs text-muted-foreground">Signed in as:</div>
                  <div class="text-sm font-bold text-foreground">{user.email}</div>
                </div>
                <button
                  type="button"
                  onclick={handleLogout}
                  class="px-3 py-1.5 bg-destructive/10 hover:bg-destructive/20 text-destructive border border-destructive/20 rounded-lg text-xs flex items-center gap-1 font-medium"
                >
                  <LogOut class="w-3.5 h-3.5" /> Sign out
                </button>
              </div>
            {:else}
              <form onsubmit={(e) => { e.preventDefault(); handleAuth(); }} class="space-y-3">
                <h3 class="text-sm font-bold text-foreground">{isRegisterMode ? 'Register New Account' : 'Account Sign In'}</h3>
                <div>
                  <label for="license-auth-email" class="text-xs text-muted-foreground block mb-1">Email</label>
                  <input
                    id="license-auth-email"
                    type="email"
                    bind:value={authEmail}
                    placeholder="name@example.com"
                    class="w-full bg-background border border-input rounded-xl px-3 py-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-emerald-500"
                  />
                </div>
                <div>
                  <label for="license-auth-password" class="text-xs text-muted-foreground block mb-1">Password</label>
                  <input
                    id="license-auth-password"
                    type="password"
                    bind:value={authPassword}
                    placeholder="••••••••"
                    class="w-full bg-background border border-input rounded-xl px-3 py-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-emerald-500"
                  />
                </div>
                <button
                  type="submit"
                  disabled={isLoading}
                  class="w-full py-2.5 bg-emerald-600 hover:bg-emerald-500 text-white font-bold text-xs rounded-xl transition-all shadow-2xs"
                >
                  {isRegisterMode ? 'Register' : 'Sign In'}
                </button>

                <div class="text-center pt-2">
                  <button
                    type="button"
                    onclick={() => (isRegisterMode = !isRegisterMode)}
                    class="text-xs text-emerald-600 hover:underline font-medium"
                  >
                    {isRegisterMode ? 'Already have an account? Sign in' : "Don't have an account? Register now"}
                  </button>
                </div>
              </form>
            {/if}
          </div>
        {/if}

      </div>
    </div>
  </div>
{/if}
