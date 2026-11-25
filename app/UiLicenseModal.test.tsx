// Headless UI-46 tests: LicenseModal startup gate, auth (sign in / register / sign out),
// HWID copy via real `Util.hardwareId()`, device list & revoke, plan picker, VietQR payment order
// creation, memo copy, and payment polling.
// All HTTP calls are stubbed via `options.network` — zero real license server or payment calls.
import { expect, open, test, type StubRequest, type StubResponse } from "nsh/test";
import { Host, Util } from "rustsale/native";
import {
  formatLicenseDate,
  formatVnd,
  licenseInitials,
  transferMemoFor,
} from "./stores/licenseFormat";

const dir = `/tmp/rustsale-v3-ui-license-${Date.now()}`;
const BASE_URL = "https://license.example.test";
const TEST_STORAGE: Record<string, string> = {
  rustsale_supabase_url: BASE_URL,
  rustsale_supabase_anon_key: "test-anon-key",
};
const SIGNED_IN_SESSION = JSON.stringify({
  access_token: "tok-test-session",
  refresh_token: "ref-test-session",
  user: { id: "user-1", email: "demo@rustsale.test" },
});
const TINY_PNG =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";

const STUB_PLANS = [
  {
    id: "pro",
    name: "PRO Plan",
    price: 199000,
    duration_days: 30,
    max_devices: 3,
    description: "3 thiết bị + đầy đủ công cụ",
    is_active: true,
    sort_order: 1,
  },
  {
    id: "enterprise",
    name: "ENTERPRISE Plan",
    price: 499000,
    duration_days: 30,
    max_devices: 10,
    description: "10 thiết bị + ưu tiên hỗ trợ",
    is_active: true,
    sort_order: 2,
  },
];

type NetworkMap = Record<string, StubResponse | ((request: StubRequest) => StubResponse | Promise<StubResponse>)>;

async function openWithLicense(options: {
  storage?: Record<string, string>;
  network: NetworkMap;
  clock?: boolean;
}) {
  await Host.start(dir);
  return open("./App.tsx", {
    width: 1280,
    height: 800,
    storage: { ...TEST_STORAGE, ...(options.storage ?? {}) },
    network: options.network,
    clock: options.clock ? {} : undefined,
    modules: {
      "nsh/updater": {
        checkForUpdate: () => null,
      },
    },
  });
}

const has = (app: Awaited<ReturnType<typeof openWithLicense>>, s: string) =>
  app.texts().some((t) => t.includes(s));

test("startup gate blocks dismiss until sign in succeeds, then unlocks and copies real HWID", async () => {
  await Host.start(dir);
  const hw = (await Util.hardwareId()) as { hardware_id: string; hostname: string; os_info: string };
  expect(hw.hardware_id.length).toBeGreaterThan(0);

  let loginCalls = 0;
  const app = await openWithLicense({
    network: {
      [`GET ${BASE_URL}/rest/v1/subscription_plans*`]: { body: STUB_PLANS },
      [`POST ${BASE_URL}/auth/v1/token*`]: (req) => {
        loginCalls += 1;
        const parsed = JSON.parse(req.body ?? "{}") as { email?: string; password?: string };
        expect(parsed.email).toBe("demo@rustsale.test");
        expect(parsed.password).toBe("secret123");
        return {
          body: {
            access_token: "tok-test-login",
            refresh_token: "ref-test-login",
            user: { id: "user-1", email: "demo@rustsale.test" },
          },
        };
      },
      [`GET ${BASE_URL}/auth/v1/user`]: {
        body: { id: "user-1", email: "demo@rustsale.test" },
      },
      [`POST ${BASE_URL}/rest/v1/rpc/check_or_register_device`]: {
        body: {
          allowed: true,
          reason: "device_registered",
          device_id: "dev-current",
          plan_id: "pro",
          expires_at: "2027-01-15T00:00:00.000Z",
          max_devices: 3,
        },
      },
      [`GET ${BASE_URL}/rest/v1/user_devices*`]: {
        body: [
          {
            id: "dev-current",
            user_id: "user-1",
            hardware_id: hw.hardware_id,
            device_name: hw.hostname,
            os_info: hw.os_info,
            status: "active",
            last_active: "2026-11-25T10:00:00.000Z",
            created_at: "2026-11-25T10:00:00.000Z",
          },
        ],
      },
    },
  });

  // Unauthenticated startup opens the modal in locked gate mode on the Auth tab.
  await app.waitFor("id:license-gate-banner");
  await app.waitFor("id:license-gate-badge");
  await app.waitFor("id:license-auth-email");
  expect(app.shows("id:license-close")).toBe(false);

  // Escape cannot bypass the startup gate.
  await app.keyboard.press("Escape");
  await app.wait(50);
  expect(app.shows("id:license-gate-banner")).toBe(true);

  // Empty submit shows inline validation error without calling auth endpoint.
  await app.press("id:license-auth-submit");
  await app.waitFor("id:license-error");
  expect(has(app, "Vui lòng nhập đầy đủ Email và Mật khẩu")).toBe(true);
  expect(loginCalls).toBe(0);

  // Valid sign-in unlocks gate and switches to Status tab.
  await app.type("id:license-auth-email", "demo@rustsale.test");
  await app.type("id:license-auth-password", "secret123");
  await app.press("id:license-auth-submit");

  await app.waitFor("id:license-status-valid");
  await app.waitFor("id:license-close");
  expect(loginCalls).toBe(1);
  expect(app.shows("id:license-gate-banner")).toBe(false);
  expect(has(app, "PRO")).toBe(true);
  expect(has(app, hw.hardware_id)).toBe(true);

  // Copy HWID writes the real hardware ID to clipboard.
  await app.press("id:license-copy-hwid");
  await app.waitUntil(() => app.clipboard() === hw.hardware_id);
  expect(app.clipboard()).toBe(hw.hardware_id);
});

test("devices tab marks current device and revokes an old device with inline confirmation", async () => {
  await Host.start(dir);
  const hw = (await Util.hardwareId()) as { hardware_id: string; hostname: string; os_info: string };

  let devices = [
    {
      id: "dev-current",
      user_id: "user-1",
      hardware_id: hw.hardware_id,
      device_name: "Máy hiện tại",
      os_info: hw.os_info,
      status: "active",
      last_active: "2026-11-25T10:00:00.000Z",
      created_at: "2026-11-25T10:00:00.000Z",
    },
    {
      id: "dev-old",
      user_id: "user-1",
      hardware_id: "HWID-OLD-DEVICE-9999",
      device_name: "Laptop cũ",
      os_info: "macOS",
      status: "active",
      last_active: "2026-10-01T10:00:00.000Z",
      created_at: "2026-10-01T10:00:00.000Z",
    },
  ];
  let deleteCalls = 0;

  const app = await openWithLicense({
    storage: { rustsale_license_session: SIGNED_IN_SESSION },
    network: {
      [`GET ${BASE_URL}/rest/v1/subscription_plans*`]: { body: STUB_PLANS },
      [`GET ${BASE_URL}/auth/v1/user`]: { body: { id: "user-1", email: "demo@rustsale.test" } },
      [`POST ${BASE_URL}/rest/v1/rpc/check_or_register_device`]: {
        body: {
          allowed: true,
          reason: "existing_device",
          device_id: "dev-current",
          plan_id: "pro",
          expires_at: "2027-01-15T00:00:00.000Z",
          max_devices: 3,
        },
      },
      [`GET ${BASE_URL}/rest/v1/user_devices*`]: () => ({ body: devices }),
      [`DELETE ${BASE_URL}/rest/v1/user_devices*`]: (req) => {
        deleteCalls += 1;
        if (req.url.includes("dev-old")) {
          devices = devices.filter((d) => d.id !== "dev-old");
        }
        return { status: 204, body: "" };
      },
    },
  });

  // Valid license does not lock startup; open modal from sidebar footer.
  await app.waitUntil(() => has(app, "PRO • Đã kích hoạt"));
  await app.press("id:sidebar-license-btn");
  await app.waitFor("id:license-tab-devices");
  await app.press("id:license-tab-devices");

  await app.waitFor("id:license-device-dev-current");
  await app.waitFor("id:license-device-dev-old");
  expect(has(app, "Thiết bị hiện tại")).toBe(true);
  expect(has(app, "Laptop cũ")).toBe(true);

  await app.press("id:license-revoke-dev-old");
  await app.waitFor("id:license-confirm-revoke-dev-old");
  await app.press("id:license-confirm-revoke-dev-old");

  await app.waitUntil(() => !app.shows("id:license-device-dev-old"));
  expect(deleteCalls).toBe(1);
  expect(app.shows("id:license-device-dev-current")).toBe(true);
});

test("expired subscription creates VietQR order, copies transfer memo, polls until completed and unlocks gate", async () => {
  await Host.start(dir);
  const hw = (await Util.hardwareId()) as { hardware_id: string; hostname: string; os_info: string };

  let subscriptionActive = false;
  let createdOrderCode = 0;
  let pollCount = 0;

  const app = await openWithLicense({
    clock: true,
    storage: { rustsale_license_session: SIGNED_IN_SESSION },
    network: {
      [`GET ${BASE_URL}/rest/v1/subscription_plans*`]: { body: STUB_PLANS },
      [`GET ${BASE_URL}/auth/v1/user`]: { body: { id: "user-1", email: "demo@rustsale.test" } },
      [`POST ${BASE_URL}/rest/v1/rpc/check_or_register_device`]: () =>
        subscriptionActive
          ? {
              body: {
                allowed: true,
                reason: "existing_device",
                device_id: "dev-current",
                plan_id: "enterprise",
                expires_at: "2027-06-01T00:00:00.000Z",
                max_devices: 10,
              },
            }
          : {
              body: {
                allowed: false,
                reason: "subscription_expired",
                plan_id: "pro",
                expires_at: "2025-01-01T00:00:00.000Z",
                max_devices: 3,
              },
            },
      [`GET ${BASE_URL}/rest/v1/user_devices*`]: {
        body: [
          {
            id: "dev-current",
            user_id: "user-1",
            hardware_id: hw.hardware_id,
            device_name: hw.hostname,
            os_info: hw.os_info,
            status: "active",
            last_active: "2026-11-25T10:00:00.000Z",
            created_at: "2026-11-25T10:00:00.000Z",
          },
        ],
      },
      [`POST ${BASE_URL}/rest/v1/payment_transactions`]: (req) => {
        const body = JSON.parse(req.body ?? "{}") as {
          order_code: number;
          user_id: string;
          amount: number;
          plan_id: string;
          duration_days: number;
          provider: string;
        };
        expect(body.plan_id).toBe("enterprise");
        expect(body.amount).toBe(499000);
        expect(body.provider).toBe("sepay");
        createdOrderCode = body.order_code;
        return {
          status: 201,
          body: [
            {
              id: "tx-1",
              order_code: body.order_code,
              user_id: body.user_id,
              amount: body.amount,
              provider: "sepay",
              plan_id: body.plan_id,
              duration_days: body.duration_days,
              status: "pending",
              qr_code_url: TINY_PNG,
              created_at: "2026-11-25T10:00:00.000Z",
            },
          ],
        };
      },
      [`GET ${BASE_URL}/rest/v1/payment_transactions*`]: () => {
        pollCount += 1;
        if (pollCount >= 2) {
          subscriptionActive = true;
        }
        return {
          body: [
            {
              id: "tx-1",
              order_code: createdOrderCode,
              user_id: "user-1",
              amount: 499000,
              provider: "sepay",
              plan_id: "enterprise",
              duration_days: 30,
              status: pollCount >= 2 ? "completed" : "pending",
              qr_code_url: TINY_PNG,
              created_at: "2026-11-25T10:00:00.000Z",
            },
          ],
        };
      },
    },
  });

  // Expired subscription locks gate and opens directly on the Upgrade tab.
  await app.waitFor("id:license-gate-banner");
  await app.waitFor("id:license-plan-enterprise");
  await app.press("id:license-plan-enterprise");
  await app.press("id:license-create-payment");

  await app.waitFor("id:license-payment-panel");
  await app.waitFor("id:license-payment-qr");
  await app.waitFor("id:license-payment-polling");
  expect(createdOrderCode).toBeGreaterThanOrEqual(100000000);
  expect(has(app, String(createdOrderCode))).toBe(true);

  // Copy transfer memo writes exact SePay memo format.
  await app.press("id:license-copy-memo");
  const expectedMemo = transferMemoFor(createdOrderCode);
  await app.waitUntil(() => app.clipboard() === expectedMemo);
  expect(app.clipboard()).toBe(expectedMemo);

  // 1st poll (3s): still pending.
  await app.wait(3100);
  await app.waitUntil(() => pollCount >= 1);
  expect(app.shows("id:license-payment-panel")).toBe(true);

  // 2nd poll (+3s): completed -> re-verifies subscription, unlocks gate, switches to Status tab.
  await app.wait(3100);
  await app.waitFor("id:license-status-valid");
  await app.waitFor("id:license-close");
  expect(app.shows("id:license-gate-banner")).toBe(false);
  expect(has(app, "Thanh toán thành công")).toBe(true);
  expect(has(app, "ENTERPRISE")).toBe(true);
});

test("cancelling payment stops polling; sign out clears session and register mode submits signup", async () => {
  await Host.start(dir);
  const hw = (await Util.hardwareId()) as { hardware_id: string; hostname: string; os_info: string };

  let pollCount = 0;
  let logoutCalls = 0;
  let signupCalls = 0;

  const app = await openWithLicense({
    clock: true,
    storage: { rustsale_license_session: SIGNED_IN_SESSION },
    network: {
      [`GET ${BASE_URL}/rest/v1/subscription_plans*`]: { body: STUB_PLANS },
      [`GET ${BASE_URL}/auth/v1/user`]: { body: { id: "user-1", email: "demo@rustsale.test" } },
      [`POST ${BASE_URL}/rest/v1/rpc/check_or_register_device`]: {
        body: {
          allowed: false,
          reason: "subscription_expired",
          plan_id: "pro",
          expires_at: "2025-01-01T00:00:00.000Z",
          max_devices: 3,
        },
      },
      [`GET ${BASE_URL}/rest/v1/user_devices*`]: {
        body: [
          {
            id: "dev-current",
            user_id: "user-1",
            hardware_id: hw.hardware_id,
            device_name: hw.hostname,
            os_info: hw.os_info,
            status: "active",
            last_active: "2026-11-25T10:00:00.000Z",
            created_at: "2026-11-25T10:00:00.000Z",
          },
        ],
      },
      [`POST ${BASE_URL}/rest/v1/payment_transactions`]: (req) => {
        const body = JSON.parse(req.body ?? "{}") as { order_code: number; plan_id: string; amount: number };
        return {
          status: 201,
          body: [
            {
              id: "tx-cancel",
              order_code: body.order_code,
              user_id: "user-1",
              amount: body.amount,
              provider: "sepay",
              plan_id: body.plan_id,
              duration_days: 30,
              status: "pending",
              qr_code_url: TINY_PNG,
              created_at: "2026-11-25T10:00:00.000Z",
            },
          ],
        };
      },
      [`GET ${BASE_URL}/rest/v1/payment_transactions*`]: () => {
        pollCount += 1;
        return { body: [] };
      },
      [`POST ${BASE_URL}/auth/v1/logout`]: () => {
        logoutCalls += 1;
        return { status: 204, body: "" };
      },
      [`POST ${BASE_URL}/auth/v1/signup`]: (req) => {
        signupCalls += 1;
        const body = JSON.parse(req.body ?? "{}") as { email?: string };
        expect(body.email).toBe("new@rustsale.test");
        return { body: { user: { id: "user-2", email: "new@rustsale.test" } } };
      },
    },
  });

  await app.waitFor("id:license-create-payment");
  await app.press("id:license-create-payment");
  await app.waitFor("id:license-payment-panel");

  // Cancel immediately before first 3s poll fires -> advancing clock triggers zero poll calls.
  await app.press("id:license-cancel-payment");
  await app.waitUntil(() => !app.shows("id:license-payment-panel"));
  await app.wait(9500);
  expect(pollCount).toBe(0);

  // Sign out clears stored session and keeps gate active.
  await app.press("id:license-tab-auth");
  await app.waitFor("id:license-logout");
  await app.press("id:license-logout");
  await app.waitFor("id:license-auth-email");
  expect(logoutCalls).toBe(1);
  expect(app.storage().rustsale_license_session).toBeUndefined();
  expect(app.shows("id:license-gate-banner")).toBe(true);

  // Switch to register mode and submit signup.
  await app.press("id:license-auth-toggle");
  await app.type("id:license-auth-email", "new@rustsale.test");
  await app.type("id:license-auth-password", "newpass123");
  await app.press("id:license-auth-submit");
  await app.waitFor("id:license-success");
  expect(signupCalls).toBe(1);
  expect(has(app, "Đăng ký thành công")).toBe(true);
});

test("license format helpers format VND, dates, memo and initials safely", () => {
  expect(formatVnd(199000)).toBe("199.000 VND");
  expect(formatVnd(499000)).toBe("499.000 VND");
  expect(formatVnd(Number.NaN)).toBe("0 VND");
  expect(transferMemoFor(123456789)).toBe("RS SEVQR TKPRS 123456789");
  expect(formatLicenseDate(null)).toBe("—");
  expect(formatLicenseDate("invalid-date")).toBe("invalid-date");
  expect(licenseInitials("nguyen.an@example.com")).toBe("NA");
  expect(licenseInitials("admin@example.com")).toBe("AD");
  expect(licenseInitials(null)).toBe("RS");
});
