import { Clipboard, Storage, signal } from "nsh";
import { Util } from "rustsale/native";
import { startCore } from "./core";
import { notify } from "./toast";
import {
  formatLicenseDate,
  formatVnd,
  licenseInitials,
  transferMemoFor,
} from "./licenseFormat";

export { formatLicenseDate, formatVnd, licenseInitials, transferMemoFor };

export interface HardwareInfo {
  hardware_id: string;
  hostname: string;
  os_info: string;
}

export interface LicenseUser {
  id: string;
  email: string;
}

export interface LicenseSession {
  access_token: string;
  refresh_token?: string;
  user: LicenseUser;
}

export interface UserDevice {
  id: string;
  user_id: string;
  hardware_id: string;
  device_name: string;
  os_info: string;
  status: "active" | "blocked";
  last_active: string;
  created_at: string;
}

export type VerificationReason =
  | "device_registered"
  | "existing_device"
  | "subscription_expired"
  | "device_limit_exceeded"
  | "device_blocked"
  | "unauthenticated"
  | "unconfigured"
  | "error";

export interface DeviceVerificationResult {
  allowed: boolean;
  reason: VerificationReason;
  device_id?: string;
  plan_id?: string;
  expires_at?: string;
  max_devices?: number;
  current_devices?: number;
}

export interface SubscriptionPlan {
  id: string;
  name: string;
  price: number;
  duration_days: number;
  max_devices: number;
  description: string;
  badge_text?: string;
  is_active: boolean;
  sort_order: number;
}

export interface PaymentTransaction {
  id: string;
  order_code: number;
  user_id: string;
  amount: number;
  provider: "payos" | "sepay" | "vietqr";
  plan_id: string;
  duration_days: number;
  status: "pending" | "completed" | "failed" | "canceled";
  payment_url?: string;
  qr_code_url?: string;
  created_at: string;
}

export interface SupabaseEndpointConfig {
  url: string;
  anonKey: string;
}

export type LicenseTab = "status" | "devices" | "upgrade" | "auth";

const SUPABASE_URL_KEY = "rustsale_supabase_url";
const SUPABASE_ANON_KEY = "rustsale_supabase_anon_key";
const SESSION_KEY = "rustsale_license_session";
const VIETQR_BANK_ID_KEY = "rustsale_vietqr_bank_id";
const VIETQR_ACCOUNT_NO_KEY = "rustsale_vietqr_account_no";
const VIETQR_ACCOUNT_NAME_KEY = "rustsale_vietqr_account_name";

const DEFAULT_VIETQR_BANK_ID = "ICB";
const DEFAULT_VIETQR_ACCOUNT_NO = "101874355801";
const DEFAULT_VIETQR_ACCOUNT_NAME = "TRAN TRONG TRI";

export const PAYMENT_POLL_INTERVAL_MS = 3000;

export const DEFAULT_PLANS: SubscriptionPlan[] = [
  {
    id: "pro",
    name: "PRO Plan",
    price: 199000,
    duration_days: 30,
    max_devices: 3,
    description: "Bao gồm 3 thiết bị + đầy đủ tính năng Scraper & Zalo/Email Outreach.",
    badge_text: "PRO Plan",
    is_active: true,
    sort_order: 1,
  },
  {
    id: "enterprise",
    name: "ENTERPRISE Plan",
    price: 499000,
    duration_days: 30,
    max_devices: 10,
    description: "Bao gồm 10 thiết bị + tốc độ Scraper ưu tiên & hỗ trợ 24/7.",
    badge_text: "ENTERPRISE Plan",
    is_active: true,
    sort_order: 2,
  },
];

// Reactive license state shared between Shell (sidebar + startup gate) and LicenseModal.
export const licenseModalOpen = signal(false);
export const licensePreventClose = signal(false);
export const licenseVerified = signal(false);
export const licenseConfigured = signal(false);
export const licenseLoading = signal(false);
export const licenseError = signal<string | null>(null);
export const licenseSuccess = signal<string | null>(null);
export const licenseTab = signal<LicenseTab>("status");

export const licenseUser = signal<LicenseUser | null>(null);
export const licenseHwInfo = signal<HardwareInfo | null>(null);
export const licenseVerification = signal<DeviceVerificationResult | null>(null);
export const licenseDevices = signal<UserDevice[]>([]);
export const licensePlans = signal<SubscriptionPlan[]>(DEFAULT_PLANS);
export const licenseSelectedPlan = signal<string>("pro");
export const licenseTransaction = signal<PaymentTransaction | null>(null);
export const licensePolling = signal(false);
export const licenseCopiedHwid = signal(false);
export const licenseCopiedMemo = signal(false);

let pollTimer: ReturnType<typeof setInterval> | null = null;
let pollInFlight = false;
let pollEpoch = 0;
let copiedHwidTimer: ReturnType<typeof setTimeout> | null = null;
let copiedMemoTimer: ReturnType<typeof setTimeout> | null = null;

function extractErrorMessage(err: unknown, fallback: string): string {
  if (err instanceof Error && err.message.trim() !== "") return err.message;
  if (typeof err === "object" && err !== null) {
    const obj = err as Record<string, unknown>;
    for (const key of ["error_description", "msg", "message", "error"]) {
      const v = obj[key];
      if (typeof v === "string" && v.trim() !== "") return v;
    }
  }
  if (typeof err === "string" && err.trim() !== "") return err;
  return fallback;
}

/** Resolves Supabase URL + anon key from Storage override or build-time `Util.supabaseConfig()`. */
export async function getLicenseConfig(): Promise<SupabaseEndpointConfig | null> {
  try {
    const [storedUrl, storedKey] = await Promise.all([
      Storage.getItem(SUPABASE_URL_KEY),
      Storage.getItem(SUPABASE_ANON_KEY),
    ]);
    if (
      storedUrl &&
      storedUrl.trim().startsWith("https://") &&
      storedKey &&
      storedKey.trim() !== ""
    ) {
      return {
        url: storedUrl.trim().replace(/\/+$/, ""),
        anonKey: storedKey.trim(),
      };
    }
  } catch {
    // Fall through to build-time config.
  }

  try {
    const built = (await Util.supabaseConfig()) as { url?: string; anon_key?: string } | null;
    if (
      built &&
      typeof built.url === "string" &&
      built.url.startsWith("https://") &&
      typeof built.anon_key === "string" &&
      built.anon_key !== ""
    ) {
      return {
        url: built.url.replace(/\/+$/, ""),
        anonKey: built.anon_key,
      };
    }
  } catch {
    // Dev builds without build-time Supabase config return null.
  }

  return null;
}

async function readStoredSession(): Promise<LicenseSession | null> {
  try {
    const raw = await Storage.getItem(SESSION_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<LicenseSession>;
    if (
      typeof parsed.access_token === "string" &&
      parsed.access_token !== "" &&
      parsed.user &&
      typeof parsed.user.id === "string" &&
      typeof parsed.user.email === "string"
    ) {
      return {
        access_token: parsed.access_token,
        refresh_token: typeof parsed.refresh_token === "string" ? parsed.refresh_token : undefined,
        user: { id: parsed.user.id, email: parsed.user.email },
      };
    }
  } catch {
    // Corrupt session JSON is treated as signed out.
  }
  return null;
}

async function writeStoredSession(session: LicenseSession | null): Promise<void> {
  try {
    if (session === null) {
      await Storage.removeItem(SESSION_KEY);
    } else {
      await Storage.setItem(SESSION_KEY, JSON.stringify(session));
    }
  } catch {
    // Ignore storage write failures to avoid crashing UI.
  }
}

interface HttpResult<T> {
  ok: boolean;
  status: number;
  data: T | null;
  error: string | null;
}

async function supabaseFetch<T>(
  cfg: SupabaseEndpointConfig,
  path: string,
  options: {
    method?: string;
    body?: unknown;
    accessToken?: string | null;
    extraHeaders?: Record<string, string>;
  } = {},
): Promise<HttpResult<T>> {
  const headers: Record<string, string> = {
    apikey: cfg.anonKey,
    Authorization: `Bearer ${options.accessToken || cfg.anonKey}`,
    "Content-Type": "application/json",
    ...(options.extraHeaders ?? {}),
  };

  const res = await fetch(`${cfg.url}${path}`, {
    method: options.method ?? "GET",
    headers,
    body: options.body !== undefined ? JSON.stringify(options.body) : undefined,
  });

  const text = await res.text();
  let parsed: unknown = null;
  if (text.trim() !== "") {
    try {
      parsed = JSON.parse(text);
    } catch {
      parsed = text;
    }
  }

  if (!res.ok) {
    return {
      ok: false,
      status: res.status,
      data: null,
      error: extractErrorMessage(parsed, `HTTP ${res.status}`),
    };
  }

  return {
    ok: true,
    status: res.status,
    data: parsed as T,
    error: null,
  };
}

/** Reads the device hardware ID from `Util.hardwareId()`. */
export async function loadHardwareInfo(): Promise<HardwareInfo> {
  await startCore();
  const hw = (await Util.hardwareId()) as HardwareInfo;
  licenseHwInfo.set(hw);
  return hw;
}

async function fetchCurrentSessionUser(
  cfg: SupabaseEndpointConfig,
): Promise<{ session: LicenseSession; user: LicenseUser } | null> {
  const session = await readStoredSession();
  if (!session) return null;

  const res = await supabaseFetch<{ id?: string; email?: string }>(cfg, "/auth/v1/user", {
    method: "GET",
    accessToken: session.access_token,
  });
  if (!res.ok || !res.data) {
    if (res.status === 401 || res.status === 403) {
      await writeStoredSession(null);
      return null;
    }
    throw new Error(res.error ?? "Không thể xác thực phiên đăng nhập.");
  }

  const user: LicenseUser = {
    id: res.data.id ?? session.user.id,
    email: res.data.email ?? session.user.email,
  };
  return { session, user };
}

async function verifyDeviceFallback(
  cfg: SupabaseEndpointConfig,
  session: LicenseSession,
  userId: string,
  hwInfo: HardwareInfo,
): Promise<DeviceVerificationResult> {
  const subRes = await supabaseFetch<
    { plan_id?: string; max_devices?: number; expires_at?: string }[]
  >(cfg, `/rest/v1/subscriptions?user_id=eq.${encodeURIComponent(userId)}&select=*`, {
    accessToken: session.access_token,
  });
  const sub = Array.isArray(subRes.data) ? subRes.data[0] : null;
  const maxDevices = sub?.max_devices ?? 1;
  const expiresAt = sub?.expires_at ?? new Date(Date.now() + 7 * 86_400_000).toISOString();
  if (new Date(expiresAt).getTime() < Date.now()) {
    return {
      allowed: false,
      reason: "subscription_expired",
      expires_at: expiresAt,
      plan_id: sub?.plan_id,
      max_devices: maxDevices,
    };
  }

  const devRes = await supabaseFetch<UserDevice[]>(
    cfg,
    `/rest/v1/user_devices?user_id=eq.${encodeURIComponent(userId)}&select=*`,
    { accessToken: session.access_token },
  );
  const devices = Array.isArray(devRes.data) ? devRes.data : [];
  const existing = devices.find((d) => d.hardware_id === hwInfo.hardware_id);
  if (existing) {
    if (existing.status === "blocked") {
      return { allowed: false, reason: "device_blocked" };
    }
    return {
      allowed: true,
      reason: "existing_device",
      device_id: existing.id,
      plan_id: sub?.plan_id,
      expires_at: expiresAt,
      max_devices: maxDevices,
    };
  }

  const activeCount = devices.filter((d) => d.status === "active").length;
  if (activeCount >= maxDevices) {
    return {
      allowed: false,
      reason: "device_limit_exceeded",
      current_devices: activeCount,
      max_devices: maxDevices,
      plan_id: sub?.plan_id,
    };
  }

  const regRes = await supabaseFetch<UserDevice[] | UserDevice>(cfg, "/rest/v1/user_devices", {
    method: "POST",
    accessToken: session.access_token,
    extraHeaders: { Prefer: "return=representation" },
    body: {
      user_id: userId,
      hardware_id: hwInfo.hardware_id,
      device_name: hwInfo.hostname,
      os_info: hwInfo.os_info,
      status: "active",
    },
  });
  const created = Array.isArray(regRes.data) ? regRes.data[0] : regRes.data;
  return {
    allowed: true,
    reason: "device_registered",
    device_id: created?.id,
    plan_id: sub?.plan_id,
    expires_at: expiresAt,
    max_devices: maxDevices,
  };
}

async function verifyDeviceWithServer(
  cfg: SupabaseEndpointConfig,
  session: LicenseSession,
  user: LicenseUser,
  hwInfo: HardwareInfo,
): Promise<DeviceVerificationResult> {
  const rpcRes = await supabaseFetch<DeviceVerificationResult>(
    cfg,
    "/rest/v1/rpc/check_or_register_device",
    {
      method: "POST",
      accessToken: session.access_token,
      body: {
        p_hardware_id: hwInfo.hardware_id,
        p_device_name: hwInfo.hostname,
        p_os_info: hwInfo.os_info,
      },
    },
  );
  if (rpcRes.ok && rpcRes.data && typeof rpcRes.data.allowed === "boolean") {
    return rpcRes.data;
  }
  return verifyDeviceFallback(cfg, session, user.id, hwInfo);
}

async function fetchUserDevices(
  cfg: SupabaseEndpointConfig,
  session: LicenseSession,
  userId: string,
): Promise<UserDevice[]> {
  const res = await supabaseFetch<UserDevice[]>(
    cfg,
    `/rest/v1/user_devices?user_id=eq.${encodeURIComponent(userId)}&select=*&order=created_at.desc`,
    { accessToken: session.access_token },
  );
  return res.ok && Array.isArray(res.data) ? res.data : [];
}

/** Loads active subscription plans from Supabase, keeping default fallback plans if unavailable. */
export async function loadSubscriptionPlans(): Promise<SubscriptionPlan[]> {
  const cfg = await getLicenseConfig();
  if (!cfg) {
    licensePlans.set(DEFAULT_PLANS);
    return DEFAULT_PLANS;
  }
  try {
    const session = await readStoredSession();
    const res = await supabaseFetch<SubscriptionPlan[]>(
      cfg,
      "/rest/v1/subscription_plans?is_active=eq.true&select=*&order=sort_order.asc",
      { accessToken: session?.access_token },
    );
    if (res.ok && Array.isArray(res.data) && res.data.length > 0) {
      licensePlans.set(res.data);
      if (!res.data.some((p) => p.id === licenseSelectedPlan())) {
        licenseSelectedPlan.set(res.data[0].id);
      }
      return res.data;
    }
  } catch {
    // Keep fallback plans.
  }
  licensePlans.set(DEFAULT_PLANS);
  return DEFAULT_PLANS;
}

function applyVerificationOutcome(result: DeviceVerificationResult): void {
  // A transient failure (network/server) must not lock out a device that was already
  // verified this session: keep the last good verification and only show the error.
  if (result.reason === "error" && licenseVerified()) return;
  licenseVerification.set(result);
  if (result.allowed) {
    licenseVerified.set(true);
    licensePreventClose.set(false);
  } else {
    licenseVerified.set(false);
    if (licenseConfigured()) {
      licensePreventClose.set(true);
    }
    if (result.reason === "unauthenticated") {
      licenseTab.set("auth");
    } else if (
      result.reason === "subscription_expired" ||
      result.reason === "device_limit_exceeded"
    ) {
      licenseTab.set("upgrade");
    }
  }
}

/** Refreshes user session, hardware verification and registered device list. */
export async function refreshLicenseVerification(): Promise<DeviceVerificationResult | null> {
  licenseLoading.set(true);
  licenseError.set(null);
  try {
    const hw = licenseHwInfo() ?? (await loadHardwareInfo());
    const cfg = await getLicenseConfig();
    if (!cfg) {
      licenseConfigured.set(false);
      licenseVerification.set({ allowed: false, reason: "unconfigured" });
      return null;
    }
    licenseConfigured.set(true);

    const auth = await fetchCurrentSessionUser(cfg);
    if (!auth) {
      licenseUser.set(null);
      licenseDevices.set([]);
      const unauth: DeviceVerificationResult = { allowed: false, reason: "unauthenticated" };
      applyVerificationOutcome(unauth);
      return unauth;
    }

    licenseUser.set(auth.user);
    const [verification, devices] = await Promise.all([
      verifyDeviceWithServer(cfg, auth.session, auth.user, hw),
      fetchUserDevices(cfg, auth.session, auth.user.id),
    ]);
    licenseDevices.set(devices);
    applyVerificationOutcome(verification);
    return verification;
  } catch (err) {
    licenseError.set(extractErrorMessage(err, "Không thể kiểm tra trạng thái bản quyền."));
    const failed: DeviceVerificationResult = { allowed: false, reason: "error" };
    applyVerificationOutcome(failed);
    return failed;
  } finally {
    licenseLoading.set(false);
  }
}

/**
 * Startup license gate (UI-46 / BG-14):
 * Runs automatically when a license server is configured (release build `Util.supabaseConfig()` or
 * Storage override). Unconfigured dev/test builds leave the gate off and make zero network calls.
 */
export async function runStartupLicenseGate(): Promise<void> {
  const cfg = await getLicenseConfig();
  if (!cfg) {
    licenseConfigured.set(false);
    return;
  }
  licenseConfigured.set(true);
  await loadSubscriptionPlans();
  const res = await refreshLicenseVerification();
  if (!res || !res.allowed) {
    licensePreventClose.set(true);
    licenseModalOpen.set(true);
  }
}

/** Opens the License modal and initializes HWID, plans and verification state. */
export function openLicenseModal(): void {
  licenseError.set(null);
  licenseSuccess.set(null);
  licensePreventClose.set(licenseConfigured() && !licenseVerified());
  licenseModalOpen.set(true);
  void (async () => {
    await loadSubscriptionPlans();
    await refreshLicenseVerification();
  })();
}

/** Closes the License modal unless the startup gate currently requires activation. */
export function closeLicenseModal(): void {
  if (licensePreventClose()) return;
  stopPaymentPolling();
  licenseModalOpen.set(false);
}

/** Signs in with email + password against Supabase Auth and verifies this device. */
export async function signInLicense(email: string, password: string): Promise<void> {
  const trimmedEmail = email.trim();
  if (trimmedEmail === "" || password === "") {
    licenseError.set("Vui lòng nhập đầy đủ Email và Mật khẩu.");
    return;
  }
  const cfg = await getLicenseConfig();
  if (!cfg) {
    licenseError.set("Chưa cấu hình máy chủ bản quyền.");
    return;
  }
  licenseLoading.set(true);
  licenseError.set(null);
  licenseSuccess.set(null);
  try {
    const res = await supabaseFetch<{
      access_token?: string;
      refresh_token?: string;
      user?: { id?: string; email?: string };
    }>(cfg, "/auth/v1/token?grant_type=password", {
      method: "POST",
      body: { email: trimmedEmail, password },
    });
    if (!res.ok || !res.data?.access_token || !res.data?.user?.id) {
      throw new Error(res.error ?? "Đăng nhập thất bại. Vui lòng kiểm tra lại thông tin.");
    }
    const user: LicenseUser = {
      id: res.data.user.id,
      email: res.data.user.email ?? trimmedEmail,
    };
    await writeStoredSession({
      access_token: res.data.access_token,
      refresh_token: res.data.refresh_token,
      user,
    });
    licenseUser.set(user);
    licenseSuccess.set("Đăng nhập thành công!");
    const verification = await refreshLicenseVerification();
    if (verification?.allowed) {
      licenseTab.set("status");
    }
  } catch (err) {
    licenseError.set(extractErrorMessage(err, "Đăng nhập thất bại."));
  } finally {
    licenseLoading.set(false);
  }
}

/** Registers a new account via Supabase Auth. Returns true when registration succeeded. */
export async function registerLicense(email: string, password: string): Promise<boolean> {
  const trimmedEmail = email.trim();
  if (trimmedEmail === "" || password === "") {
    licenseError.set("Vui lòng nhập đầy đủ Email và Mật khẩu.");
    return false;
  }
  const cfg = await getLicenseConfig();
  if (!cfg) {
    licenseError.set("Chưa cấu hình máy chủ bản quyền.");
    return false;
  }
  licenseLoading.set(true);
  licenseError.set(null);
  licenseSuccess.set(null);
  try {
    const res = await supabaseFetch<{ user?: { id?: string; email?: string }; id?: string }>(
      cfg,
      "/auth/v1/signup",
      {
        method: "POST",
        body: { email: trimmedEmail, password },
      },
    );
    if (!res.ok) {
      throw new Error(res.error ?? "Đăng ký thất bại.");
    }
    licenseSuccess.set("Đăng ký thành công! Vui lòng đăng nhập.");
    return true;
  } catch (err) {
    licenseError.set(extractErrorMessage(err, "Đăng ký thất bại."));
    return false;
  } finally {
    licenseLoading.set(false);
  }
}

/** Signs out the current account, clears stored session and re-enables the gate when configured. */
export async function logoutLicense(): Promise<void> {
  stopPaymentPolling();
  licenseTransaction.set(null);
  licenseLoading.set(true);
  licenseError.set(null);
  licenseSuccess.set(null);
  try {
    const cfg = await getLicenseConfig();
    const session = await readStoredSession();
    if (cfg && session) {
      await supabaseFetch(cfg, "/auth/v1/logout", {
        method: "POST",
        accessToken: session.access_token,
      }).catch(() => undefined);
    }
  } finally {
    await writeStoredSession(null);
    licenseUser.set(null);
    licenseVerification.set({ allowed: false, reason: "unauthenticated" });
    licenseDevices.set([]);
    licenseVerified.set(false);
    if (licenseConfigured()) {
      licensePreventClose.set(true);
    }
    licenseTab.set("auth");
    licenseLoading.set(false);
  }
}

/** Revokes a registered device by ID and reloads verification + device list. */
export async function revokeLicenseDevice(deviceId: string): Promise<void> {
  const cfg = await getLicenseConfig();
  const session = await readStoredSession();
  if (!cfg || !session) {
    licenseError.set("Bạn cần đăng nhập trước khi thu hồi thiết bị.");
    return;
  }
  licenseLoading.set(true);
  licenseError.set(null);
  licenseSuccess.set(null);
  try {
    const res = await supabaseFetch(
      cfg,
      `/rest/v1/user_devices?id=eq.${encodeURIComponent(deviceId)}`,
      {
        method: "DELETE",
        accessToken: session.access_token,
      },
    );
    if (!res.ok) {
      throw new Error(res.error ?? "Không thể thu hồi thiết bị.");
    }
    licenseSuccess.set("Đã thu hồi thiết bị thành công.");
    await refreshLicenseVerification();
  } catch (err) {
    licenseError.set(extractErrorMessage(err, "Không thể thu hồi thiết bị."));
  } finally {
    licenseLoading.set(false);
  }
}

/** Selects a subscription plan; cancels any active payment order for a different plan. */
export function selectLicensePlan(planId: string): void {
  licenseSelectedPlan.set(planId);
  const tx = licenseTransaction();
  if (tx && tx.plan_id !== planId) {
    resetLicensePayment();
  }
}

/** Stops the active payment status polling interval. */
export function stopPaymentPolling(): void {
  pollEpoch += 1;
  pollInFlight = false;
  if (pollTimer !== null) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
  licensePolling.set(false);
}

/** Cancels the current pending payment order and stops polling. */
export function resetLicensePayment(): void {
  stopPaymentPolling();
  licenseTransaction.set(null);
}

async function pollPaymentOnce(
  cfg: SupabaseEndpointConfig,
  orderCode: number,
  epoch: number,
): Promise<void> {
  if (pollInFlight || epoch !== pollEpoch) return;
  pollInFlight = true;
  try {
    const session = await readStoredSession();
    if (!session || epoch !== pollEpoch) return;
    const res = await supabaseFetch<PaymentTransaction[] | PaymentTransaction>(
      cfg,
      `/rest/v1/payment_transactions?order_code=eq.${encodeURIComponent(String(orderCode))}&select=*`,
      { accessToken: session.access_token },
    );
    if (!res.ok || !res.data || epoch !== pollEpoch) return;
    const tx = Array.isArray(res.data) ? res.data[0] : res.data;
    if (!tx) return;

    if (tx.status === "completed") {
      stopPaymentPolling();
      licenseTransaction.set(null);
      licenseSuccess.set("Thanh toán thành công! Gói bản quyền của bạn đã được gia hạn.");
      await refreshLicenseVerification();
      licenseTab.set("status");
    } else if (tx.status === "failed" || tx.status === "canceled") {
      stopPaymentPolling();
      licenseTransaction.set(null);
      licenseError.set("Đơn thanh toán đã bị huỷ hoặc thất bại. Vui lòng tạo đơn mới.");
    }
  } catch {
    // Transient network errors keep polling until cancelled or completed.
  } finally {
    pollInFlight = false;
  }
}

function startPaymentPolling(cfg: SupabaseEndpointConfig, orderCode: number): void {
  stopPaymentPolling();
  const epoch = pollEpoch;
  licensePolling.set(true);
  pollTimer = setInterval(() => {
    void pollPaymentOnce(cfg, orderCode, epoch);
  }, PAYMENT_POLL_INTERVAL_MS);
}

/** Creates a VietQR / SePay payment order for the selected plan and begins status polling. */
export async function createLicensePayment(): Promise<void> {
  const cfg = await getLicenseConfig();
  const session = await readStoredSession();
  const user = licenseUser();
  if (!cfg) {
    licenseError.set("Chưa cấu hình máy chủ bản quyền.");
    return;
  }
  if (!session || !user) {
    licenseError.set("Vui lòng đăng nhập trước khi tạo đơn thanh toán.");
    licenseTab.set("auth");
    return;
  }

  const planId = licenseSelectedPlan();
  const targetPlan = licensePlans().find((p) => p.id === planId);
  const amount = targetPlan ? targetPlan.price : planId === "enterprise" ? 499000 : 199000;
  const durationDays = targetPlan ? targetPlan.duration_days : 30;

  licenseLoading.set(true);
  licenseError.set(null);
  licenseSuccess.set(null);

  try {
    const [storedBank, storedAcc, storedName] = await Promise.all([
      Storage.getItem(VIETQR_BANK_ID_KEY).catch(() => null),
      Storage.getItem(VIETQR_ACCOUNT_NO_KEY).catch(() => null),
      Storage.getItem(VIETQR_ACCOUNT_NAME_KEY).catch(() => null),
    ]);
    const bankId = storedBank?.trim() || DEFAULT_VIETQR_BANK_ID;
    const accountNo = storedAcc?.trim() || DEFAULT_VIETQR_ACCOUNT_NO;
    const accountName = storedName?.trim() || DEFAULT_VIETQR_ACCOUNT_NAME;
    const orderCode = Math.max(100000000, Number(String(Date.now()).slice(-9)));
    const addInfo = transferMemoFor(orderCode);
    const qrCodeUrl =
      `https://img.vietqr.io/image/${bankId}-${accountNo}-compact2.png` +
      `?amount=${amount}&addInfo=${encodeURIComponent(addInfo)}&accountName=${encodeURIComponent(accountName)}`;

    const res = await supabaseFetch<PaymentTransaction[] | PaymentTransaction>(
      cfg,
      "/rest/v1/payment_transactions",
      {
        method: "POST",
        accessToken: session.access_token,
        extraHeaders: { Prefer: "return=representation" },
        body: {
          order_code: orderCode,
          user_id: user.id,
          amount,
          provider: "sepay",
          plan_id: planId,
          duration_days: durationDays,
          status: "pending",
          qr_code_url: qrCodeUrl,
        },
      },
    );
    if (!res.ok || !res.data) {
      throw new Error(res.error ?? "Không thể tạo đơn thanh toán.");
    }
    const created = Array.isArray(res.data) ? res.data[0] : res.data;
    if (!created) {
      throw new Error("Máy chủ không trả về thông tin đơn thanh toán.");
    }
    licenseTransaction.set(created);
    startPaymentPolling(cfg, created.order_code);
  } catch (err) {
    licenseError.set(extractErrorMessage(err, "Không thể tạo đơn thanh toán."));
  } finally {
    licenseLoading.set(false);
  }
}

/** Copies the device hardware ID to the system clipboard. */
export async function copyLicenseHwid(): Promise<void> {
  const hwid = licenseHwInfo()?.hardware_id;
  if (!hwid) return;
  try {
    await Clipboard.writeText(hwid);
    licenseCopiedHwid.set(true);
    notify("Đã sao chép HWID", "success");
    if (copiedHwidTimer !== null) clearTimeout(copiedHwidTimer);
    copiedHwidTimer = setTimeout(() => {
      licenseCopiedHwid.set(false);
      copiedHwidTimer = null;
    }, 2000);
  } catch {
    notify("Không sao chép được HWID", "danger");
  }
}

/** Copies the VietQR transfer memo to the system clipboard. */
export async function copyPaymentMemo(orderCode: number): Promise<void> {
  try {
    await Clipboard.writeText(transferMemoFor(orderCode));
    licenseCopiedMemo.set(true);
    notify("Đã sao chép nội dung chuyển khoản", "success");
    if (copiedMemoTimer !== null) clearTimeout(copiedMemoTimer);
    copiedMemoTimer = setTimeout(() => {
      licenseCopiedMemo.set(false);
      copiedMemoTimer = null;
    }, 2000);
  } catch {
    notify("Không sao chép được nội dung chuyển khoản", "danger");
  }
}

/** Releases active license timers when the shell unmounts. */
export function cleanupLicenseTimers(): void {
  stopPaymentPolling();
  if (copiedHwidTimer !== null) {
    clearTimeout(copiedHwidTimer);
    copiedHwidTimer = null;
  }
  if (copiedMemoTimer !== null) {
    clearTimeout(copiedMemoTimer);
    copiedMemoTimer = null;
  }
}
