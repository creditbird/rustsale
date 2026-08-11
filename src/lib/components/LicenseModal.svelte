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
      name: 'Gói PRO',
      price: 199000,
      duration_days: 30,
      max_devices: 3,
      description: 'Gồm: 3 Máy + Toàn bộ tính năng Scraper & Outreach Zalo/Email.',
      badge_text: 'Gói PRO',
      is_active: true,
      sort_order: 1
    },
    {
      id: 'enterprise',
      name: 'Gói ENTERPRISE',
      price: 499000,
      duration_days: 30,
      max_devices: 10,
      description: 'Gồm: 10 Máy + Ưu tiên tốc độ Scraper & Hỗ trợ 24/7.',
      badge_text: 'Gói ENTERPRISE',
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
    return 'Đã xảy ra lỗi không xác định.';
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
      errorMsg = 'Vui lòng nhập Email và Mật khẩu';
      return;
    }
    isLoading = true;
    errorMsg = null;
    try {
      if (isRegisterMode) {
        user = await registerWithEmail(authEmail, authPassword);
        successMsg = 'Đăng ký thành công! Vui lòng đăng nhập.';
        isRegisterMode = false;
      } else {
        user = await loginWithEmail(authEmail, authPassword);
        successMsg = 'Đăng nhập thành công!';
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
    if (!confirm('Bạn có chắc chắn muốn huỷ kích hoạt thiết bị này không?')) return;
    isLoading = true;
    try {
      const ok = await revokeDevice(deviceId);
      if (ok) {
        successMsg = 'Đã huỷ kích hoạt thiết bị.';
        await refreshVerification();
      } else {
        errorMsg = 'Không thể huỷ kích hoạt thiết bị.';
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
        successMsg = 'Thanh toán thành công! Gói cước và thời hạn của bạn đã được gia hạn.';
        currentTransaction = null;
        await refreshVerification();
        activeTab = 'status';
      }
    }, 3000);
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
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
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
            <h2 class="text-lg font-bold text-foreground tracking-tight">Quản lý Bản quyền & Thiết bị</h2>
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
            Bắt buộc Kích hoạt
          </div>
        {/if}
      </div>

      <!-- Navigation Tabs -->
      <div class="flex border-b border-border bg-muted/20 px-6 gap-2 pt-2">
        <button
          onclick={() => (activeTab = 'status')}
          class="px-4 py-2 text-xs font-semibold rounded-t-lg transition-colors flex items-center gap-2 border-b-2 {activeTab === 'status' ? 'border-emerald-600 text-emerald-600 bg-card shadow-2xs font-bold' : 'border-transparent text-muted-foreground hover:text-foreground'}"
        >
          <ShieldCheck class="w-4 h-4" /> Trạng thái
        </button>
        <button
          onclick={() => (activeTab = 'devices')}
          class="px-4 py-2 text-xs font-semibold rounded-t-lg transition-colors flex items-center gap-2 border-b-2 {activeTab === 'devices' ? 'border-emerald-600 text-emerald-600 bg-card shadow-2xs font-bold' : 'border-transparent text-muted-foreground hover:text-foreground'}"
        >
          <Laptop class="w-4 h-4" /> Thiết bị ({devices.length})
        </button>
        <button
          onclick={() => (activeTab = 'upgrade')}
          class="px-4 py-2 text-xs font-semibold rounded-t-lg transition-colors flex items-center gap-2 border-b-2 {activeTab === 'upgrade' ? 'border-emerald-600 text-emerald-600 bg-card shadow-2xs font-bold' : 'border-transparent text-muted-foreground hover:text-foreground'}"
        >
          <Sparkles class="w-4 h-4 text-amber-500" /> Nâng gói / VietQR
        </button>
        <button
          onclick={() => (activeTab = 'auth')}
          class="px-4 py-2 text-xs font-semibold rounded-t-lg transition-colors flex items-center gap-2 border-b-2 {activeTab === 'auth' ? 'border-emerald-600 text-emerald-600 bg-card shadow-2xs font-bold' : 'border-transparent text-muted-foreground hover:text-foreground'}"
        >
          <User class="w-4 h-4" /> Tài khoản
        </button>

      </div>

      <!-- Content Area -->
      <div class="p-6 overflow-y-auto space-y-4 flex-1 bg-card">

        {#if preventClose}
          <div class="p-3.5 bg-amber-500/10 border border-amber-500/20 text-amber-700 dark:text-amber-300 text-xs rounded-xl flex items-center gap-3 shadow-2xs">
            <ShieldAlert class="w-5 h-5 shrink-0 text-amber-500" />
            <div>
              <div class="font-bold text-amber-800 dark:text-amber-200 text-xs">Yêu cầu Đăng nhập & Kích hoạt Bản quyền</div>
              <div class="text-[11px] text-amber-700/80 dark:text-amber-300/80">Vui lòng đăng nhập hoặc gia hạn gói cước thiết bị để tiếp tục truy cập phần mềm RustSale.</div>
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
                <span class="text-xs font-medium text-muted-foreground">Mã phần cứng (HWID):</span>
                <button
                  onclick={copyHwid}
                  class="text-xs text-emerald-600 hover:text-emerald-700 flex items-center gap-1 bg-emerald-500/10 px-2 py-1 rounded-md border border-emerald-500/20 transition-colors"
                >
                  {#if copiedHwid}
                    <Check class="w-3 h-3 text-emerald-600" /> Đã sao chép
                  {:else}
                    <Copy class="w-3 h-3" /> Sao chép HWID
                  {/if}
                </button>
              </div>
              <div class="font-mono text-sm font-bold text-emerald-700 dark:text-emerald-400 bg-background px-3 py-2 rounded-lg border border-border shadow-2xs">
                {hwInfo?.hardware_id || 'Đang tải HWID...'}
              </div>
              <div class="flex text-xs text-muted-foreground gap-4">
                <span>Tên máy: <strong class="text-foreground font-semibold">{hwInfo?.hostname}</strong></span>
                <span>HĐH: <strong class="text-foreground font-semibold">{hwInfo?.os_info}</strong></span>
              </div>
            </div>

            <!-- License Status Banner -->
            {#if verification}
              {#if verification.allowed}
                <div class="p-4 rounded-xl bg-emerald-500/10 border border-emerald-500/20 space-y-2 text-emerald-900 dark:text-emerald-100">
                  <div class="flex items-center gap-2 text-emerald-600 dark:text-emerald-400 font-bold text-sm">
                    <CheckCircle2 class="w-5 h-5" />
                    <span>Bản quyền Hợp lệ - Được phép truy cập</span>
                  </div>
                  <div class="grid grid-cols-2 gap-2 text-xs text-muted-foreground pt-2 border-t border-emerald-500/10">
                    <div>Gói cước: <span class="font-bold text-foreground uppercase">{verification.plan_id}</span></div>
                    <div>Thời hạn đến: <span class="font-bold text-amber-600 dark:text-amber-400">{new Date(verification.expires_at || '').toLocaleDateString('vi-VN')}</span></div>
                    <div>Số thiết bị cho phép: <span class="font-bold text-foreground">{verification.max_devices} máy</span></div>
                  </div>
                </div>
              {:else}
                <div class="p-4 rounded-xl bg-amber-500/10 border border-amber-500/20 space-y-2">
                  <div class="flex items-center gap-2 text-amber-600 dark:text-amber-400 font-bold text-sm">
                    <ShieldAlert class="w-5 h-5" />
                    <span>
                      {#if verification.reason === 'subscription_expired'}
                        Gói cước đã hết hạn
                      {:else if verification.reason === 'device_limit_exceeded'}
                        Vượt quá số lượng thiết bị cho phép ({verification.current_devices}/{verification.max_devices} máy)
                      {:else}
                        Chưa xác thực bản quyền
                      {/if}
                    </span>
                  </div>
                  <p class="text-xs text-muted-foreground">
                    Vui lòng gia hạn gói cước hoặc hủy kích hoạt thiết bị cũ trong mục "Thiết bị".
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
                <RefreshCw class="w-3.5 h-3.5 {isLoading ? 'animate-spin' : ''}" /> Kiểm tra lại
              </button>
            </div>
          </div>
        {/if}

        <!-- TAB: DEVICES -->
        {#if activeTab === 'devices'}
          <div class="space-y-3">
            <p class="text-xs text-muted-foreground">Danh sách các thiết bị đang liên kết với tài khoản này:</p>
            {#each devices as dev (dev.id)}
              <div class="p-3 bg-muted/30 border border-border rounded-xl flex items-center justify-between">
                <div class="flex items-center gap-3">
                  <Laptop class="w-5 h-5 text-emerald-600 shrink-0" />
                  <div>
                    <div class="text-xs font-bold text-foreground flex items-center gap-2">
                      {dev.device_name}
                      {#if hwInfo && dev.hardware_id === hwInfo.hardware_id}
                        <span class="text-[10px] bg-emerald-500/10 text-emerald-600 px-2 py-0.5 rounded-md border border-emerald-500/20 font-semibold">Máy hiện tại</span>
                      {/if}
                    </div>
                    <div class="font-mono text-[11px] text-muted-foreground">{dev.hardware_id}</div>
                    <div class="text-[10px] text-muted-foreground/80">Hoạt động lần cuối: {new Date(dev.last_active).toLocaleString('vi-VN')}</div>
                  </div>
                </div>

                <button
                  onclick={() => handleRevokeDevice(dev.id)}
                  disabled={isLoading}
                  class="px-3 py-1.5 text-xs text-destructive bg-destructive/10 hover:bg-destructive/20 border border-destructive/20 rounded-lg transition-colors font-medium"
                >
                  Huỷ kích hoạt
                </button>
              </div>
            {:else}
              <div class="text-center py-6 text-muted-foreground text-xs">Chưa có thiết bị nào được đăng ký.</div>
            {/each}
          </div>
        {/if}

        <!-- TAB: UPGRADE / VIETQR -->
        {#if activeTab === 'upgrade'}
          <div class="space-y-4">
            <div class="grid grid-cols-2 gap-3">
              {#each availablePlans as plan}
                <button
                  onclick={() => (selectedPlan = plan.id)}
                  class="p-4 rounded-xl border text-left transition-all relative overflow-hidden {selectedPlan === plan.id ? 'border-emerald-500 bg-emerald-500/5 text-foreground ring-1 ring-emerald-500' : 'border-border bg-card text-muted-foreground hover:border-border/80'}"
                >
                  <div class="text-sm font-bold text-emerald-600 dark:text-emerald-400">{plan.name}</div>
                  <div class="text-lg font-black text-foreground py-1">{plan.price.toLocaleString('vi-VN')}đ <span class="text-xs text-muted-foreground font-normal">/ {plan.duration_days} ngày</span></div>
                  <div class="text-xs text-muted-foreground">{plan.description}</div>
                </button>
              {/each}
            </div>

            {#if !currentTransaction}
              <button
                onclick={handleCreatePayment}
                disabled={isLoading}
                class="w-full py-3 bg-emerald-600 hover:bg-emerald-500 text-white font-bold text-xs rounded-xl shadow-md flex items-center justify-center gap-2 transition-all"
              >
                <QrCode class="w-4 h-4" /> Tạo mã chuyển khoản VietQR / PayOS
              </button>
            {:else}
              <!-- QR Payment Display Modal -->
              <div class="p-4 bg-muted/30 border border-border rounded-xl space-y-3 text-center">
                <div class="text-xs font-semibold text-foreground">Quét mã QR bằng ứng dụng Ngân hàng để thanh toán:</div>
                <div class="flex justify-center p-2 bg-white rounded-xl inline-block border border-border shadow-2xs">
                  <img src={currentTransaction.qr_code_url} alt="VietQR" class="w-56 h-56 object-contain" />
                </div>
                <div class="text-xs text-muted-foreground space-y-1">
                  <div>Mã đơn hàng: <strong class="text-amber-600 dark:text-amber-400 font-mono">{currentTransaction.order_code}</strong></div>
                  <div>Số tiền: <strong class="text-emerald-600 dark:text-emerald-400 font-bold">{currentTransaction.amount.toLocaleString()}đ</strong></div>
                  <div class="flex items-center justify-center gap-1 pt-1">
                    <span>Nội dung CK:</span>
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
                      title="Sao chép nội dung chuyển khoản"
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
                  <RefreshCw class="w-3.5 h-3.5 animate-spin" /> Đang chờ hệ thống tự động xác nhận chuyển khoản...
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
                  <div class="text-xs text-muted-foreground">Đã đăng nhập tài khoản:</div>
                  <div class="text-sm font-bold text-foreground">{user.email}</div>
                </div>
                <button
                  onclick={handleLogout}
                  class="px-3 py-1.5 bg-destructive/10 hover:bg-destructive/20 text-destructive border border-destructive/20 rounded-lg text-xs flex items-center gap-1 font-medium"
                >
                  <LogOut class="w-3.5 h-3.5" /> Đăng xuất
                </button>
              </div>
            {:else}
              <form onsubmit={(e) => { e.preventDefault(); handleAuth(); }} class="space-y-3">
                <h3 class="text-sm font-bold text-foreground">{isRegisterMode ? 'Đăng ký tài khoản mới' : 'Đăng nhập tài khoản'}</h3>
                <div>
                  <label class="text-xs text-muted-foreground block mb-1">Email</label>
                  <input
                    type="email"
                    bind:value={authEmail}
                    placeholder="name@example.com"
                    class="w-full bg-background border border-input rounded-xl px-3 py-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-emerald-500"
                  />
                </div>
                <div>
                  <label class="text-xs text-muted-foreground block mb-1">Mật khẩu</label>
                  <input
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
                  {isRegisterMode ? 'Đăng ký' : 'Đăng nhập'}
                </button>

                <div class="text-center pt-2">
                  <button
                    type="button"
                    onclick={() => (isRegisterMode = !isRegisterMode)}
                    class="text-xs text-emerald-600 hover:underline font-medium"
                  >
                    {isRegisterMode ? 'Đã có tài khoản? Đăng nhập' : 'Chưa có tài khoản? Đăng ký ngay'}
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
