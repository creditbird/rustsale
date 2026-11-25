// Headless UI-45 tests: sidebar update badge, UpdateModal download/install/relaunch flow,
// Settings updater card sync, and pure byte/progress helpers.
// All updater operations are stubbed via `options.modules["nsh/updater"]` — no real network/binary writes.
import { expect, open, test } from "nsh/test";
import { Host } from "rustsale/native";
import { computeUpdateProgress, formatBytes } from "./stores/updaterFormat";

const dir = `/tmp/rustsale-v3-ui-update-${Date.now()}`;

async function openWithUpdater(
  updaterModule: Record<string, {} | null | ((...args: any[]) => unknown)>,
) {
  await Host.start(dir);
  return open("./App.tsx", {
    width: 1280,
    height: 800,
    clock: {},
    modules: {
      "nsh/updater": updaterModule,
    },
  });
}

const has = (app: Awaited<ReturnType<typeof openWithUpdater>>, s: string) =>
  app.texts().some((t) => t.includes(s));

test("background update check shows sidebar badge and opens UpdateModal with release notes", async () => {
  let checkCalls = 0;
  const app = await openWithUpdater({
    checkForUpdate: () => {
      checkCalls += 1;
      return {
        version: "0.2.0",
        notes: "Sửa lỗi đồng bộ Zalo\nCải thiện tốc độ scraper",
        canInstall: true,
      };
    },
  });

  expect(app.shows("id:sidebar-update-badge")).toBe(false);

  // Trigger the 30s non-blocking background update check via manual clock.
  await app.wait(30_100);
  await app.waitFor("id:sidebar-update-badge");
  expect(checkCalls).toBe(1);
  expect(has(app, "v0.2.0")).toBe(true);

  await app.press("id:sidebar-update-badge");
  await app.waitFor("id:update-available-version");
  expect(has(app, "v0.1.0")).toBe(true);
  expect(has(app, "v0.2.0")).toBe(true);
  expect(has(app, "Sửa lỗi đồng bộ Zalo")).toBe(true);
});

test("download finishes into downloaded state, keeps sidebar badge ready, and calls installAndRestart", async () => {
  let downloadCalls = 0;
  let restartCalls = 0;

  const app = await openWithUpdater({
    checkForUpdate: () => ({
      version: "0.2.5",
      notes: "Bản vá ổn định",
      canInstall: true,
    }),
    downloadUpdate: () => {
      downloadCalls += 1;
    },
    installAndRestart: () => {
      restartCalls += 1;
    },
  });

  await app.wait(30_100);
  await app.waitFor("id:sidebar-update-badge");
  await app.press("id:sidebar-update-badge");
  await app.waitFor("id:update-now");

  await app.press("id:update-now");
  await app.waitFor("id:update-downloaded-banner");
  await app.waitFor("id:update-restart");
  expect(downloadCalls).toBe(1);
  expect(has(app, "Tải bản cập nhật thành công!")).toBe(true);

  // Deferring restart closes the modal while keeping the sidebar badge in "Cài lại" state.
  await app.press("id:update-later");
  await app.waitUntil(() => !app.shows("id:update-downloaded-banner"));
  expect(app.shows("id:sidebar-update-badge")).toBe(true);
  expect(has(app, "Cài lại")).toBe(true);

  // Reopen from sidebar badge and trigger installAndRestart.
  await app.press("id:sidebar-update-badge");
  await app.waitFor("id:update-restart");
  await app.press("id:update-restart");
  await app.waitUntil(() => restartCalls === 1);
  expect(restartCalls).toBe(1);
});

test("download failure shows error banner and lets the user close; up-to-date hides badge", async () => {
  const app = await openWithUpdater({
    checkForUpdate: () => ({
      version: "0.3.0",
      notes: "Thử lỗi tải",
      canInstall: true,
    }),
    downloadUpdate: () => {
      throw new Error("Mất kết nối tải gói cập nhật");
    },
  });

  await app.wait(30_100);
  await app.waitFor("id:sidebar-update-badge");
  await app.press("id:sidebar-update-badge");
  await app.waitFor("id:update-now");

  await app.press("id:update-now");
  await app.waitFor("id:update-error-banner");
  expect(has(app, "Mất kết nối tải gói cập nhật")).toBe(true);

  await app.press("id:update-later");
  await app.waitUntil(() => !app.shows("id:update-error-banner"));

  // Up-to-date app stays without a badge and syncs with SettingsScreen.
  const upToDate = await openWithUpdater({
    checkForUpdate: () => null,
  });
  await upToDate.wait(30_100);
  expect(upToDate.shows("id:sidebar-update-badge")).toBe(false);

  await upToDate.press("id:nav-settings");
  await upToDate.waitFor("id:settings-update-check");
  await upToDate.press("id:settings-update-check");
  await upToDate.waitUntil(() => has(upToDate, "Bạn đang dùng bản mới nhất."));
});

test("manual-install update shows release URL notice and disables auto-update CTA", async () => {
  const app = await openWithUpdater({
    checkForUpdate: () => ({
      version: "0.4.0",
      notes: "Phát hành gói thủ công",
      url: "https://releases.example.test/v0.4.0",
      canInstall: false,
    }),
  });

  await app.wait(30_100);
  await app.waitFor("id:sidebar-update-badge");
  await app.press("id:sidebar-update-badge");
  await app.waitFor("id:update-manual-banner");
  expect(has(app, "https://releases.example.test/v0.4.0")).toBe(true);
  expect(app.enabled("id:update-now")).toBe(false);
});

test("formatBytes and computeUpdateProgress format and clamp edge values", () => {
  expect(formatBytes(0)).toBe("0 B");
  expect(formatBytes(-50)).toBe("0 B");
  expect(formatBytes(Number.NaN)).toBe("0 B");
  expect(formatBytes(512)).toBe("512 B");
  expect(formatBytes(1024)).toBe("1 KB");
  expect(formatBytes(1536)).toBe("1.5 KB");
  expect(formatBytes(1024 * 1024 * 12.5)).toBe("12.5 MB");

  expect(computeUpdateProgress(0, 0)).toEqual({ received: 0, total: 0, percentage: 0 });
  expect(computeUpdateProgress(500, 1000)).toEqual({ received: 500, total: 1000, percentage: 50 });
  expect(computeUpdateProgress(1500, 1000)).toEqual({ received: 1500, total: 1000, percentage: 100 });
  expect(computeUpdateProgress(2048, null)).toEqual({ received: 2048, total: 0, percentage: 0 });
});
