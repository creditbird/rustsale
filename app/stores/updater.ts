import { appInfo, signal } from "nsh";
import {
  checkForUpdate,
  downloadUpdate,
  installAndRestart,
  shellInfo,
  type AppUpdate,
  type UpdateProgress,
} from "nsh/updater";
import { Host } from "rustsale/native";
import { notify } from "./toast";
import {
  computeUpdateProgress,
  formatBytes,
  type UpdateProgressState,
} from "./updaterFormat";

export { computeUpdateProgress, formatBytes, type UpdateProgressState };

export type UpdateStatus =
  | "idle"
  | "checking"
  | "available"
  | "up-to-date"
  | "downloading"
  | "downloaded"
  | "error";

export interface UpdaterState {
  status: UpdateStatus;
  currentVersion: string;
  availableVersion: string | null;
  releaseNotes: string | null;
  releaseUrl: string | null;
  canInstall: boolean;
  progress: UpdateProgressState;
  errorMessage: string | null;
  modalOpen: boolean;
}

function resolveCurrentVersion(): string {
  try {
    const shell = shellInfo();
    if (typeof shell?.version === "string" && shell.version !== "") return shell.version;
  } catch {
    // Headless or stubbed updater module may omit shellInfo.
  }
  try {
    const info = appInfo();
    if (typeof info?.version === "string" && info.version !== "") return info.version;
  } catch {
    // Fallback below.
  }
  return "0.1.0";
}

function errorText(err: unknown, fallback: string): string {
  if (err instanceof Error && err.message.trim() !== "") return err.message;
  if (typeof err === "string" && err.trim() !== "") return err;
  return fallback;
}

const ZERO_PROGRESS: UpdateProgressState = { received: 0, total: 0, percentage: 0 };

/** Shared updater state used by Sidebar badge, UpdateModal, SettingsScreen and the core updater bridge. */
export const updaterState = signal<UpdaterState>({
  status: "idle",
  currentVersion: resolveCurrentVersion(),
  availableVersion: null,
  releaseNotes: null,
  releaseUrl: null,
  canInstall: true,
  progress: { ...ZERO_PROGRESS },
  errorMessage: null,
  modalOpen: false,
});

/** Opens the update modal. */
export function openUpdateModal(): void {
  updaterState.update((s) => ({ ...s, modalOpen: true }));
}

/** Closes the update modal unless a download is currently in progress. */
export function closeUpdateModal(): void {
  if (updaterState().status === "downloading") return;
  updaterState.update((s) => ({ ...s, modalOpen: false }));
}

function applyProgress(p: UpdateProgress | null | undefined): void {
  const progress = computeUpdateProgress(p?.received, p?.total);
  updaterState.update((s) => ({
    ...s,
    progress,
  }));
}

/**
 * Checks for a newer version through `nsh/updater`.
 * Updates `updaterState` so the sidebar badge and Settings stay in sync.
 */
export async function checkForAppUpdate(
  options: { silent?: boolean; openOnAvailable?: boolean } = {},
): Promise<AppUpdate | null> {
  const { silent = false, openOnAvailable = false } = options;
  const current = updaterState().status;
  if (current === "checking" || current === "downloading") return null;

  updaterState.update((s) => ({
    ...s,
    status: "checking",
    errorMessage: null,
  }));

  try {
    const found = (await checkForUpdate()) as AppUpdate | null;
    if (found && found.version) {
      updaterState.update((s) => ({
        ...s,
        status: "available",
        availableVersion: found.version,
        releaseNotes: found.notes ?? "",
        releaseUrl: found.url ?? null,
        canInstall: found.canInstall ?? true,
        errorMessage: null,
        progress: { ...ZERO_PROGRESS },
        modalOpen: openOnAvailable ? true : s.modalOpen,
      }));
      if (silent) {
        notify(`Có bản cập nhật mới v${found.version}`, "info");
      }
      return found;
    }

    updaterState.update((s) => ({
      ...s,
      status: "up-to-date",
      availableVersion: null,
      releaseNotes: null,
      releaseUrl: null,
      errorMessage: null,
      progress: { ...ZERO_PROGRESS },
    }));
    if (!silent) {
      notify("Bạn đang dùng bản mới nhất.", "success");
    }
    return null;
  } catch (err) {
    const message = errorText(err, "Không kiểm tra được cập nhật.");
    updaterState.update((s) => ({
      ...s,
      status: "error",
      errorMessage: message,
    }));
    if (!silent) {
      notify("Không kiểm tra được cập nhật.", "danger");
    }
    return null;
  }
}

/** Downloads the update found by the last check, streaming byte progress into `updaterState`. */
export async function downloadAndInstallUpdate(): Promise<void> {
  const s = updaterState();
  if (s.status === "downloading") return;
  if (!s.availableVersion) {
    notify("Không có bản cập nhật để tải.", "warning");
    return;
  }
  if (!s.canInstall) {
    updaterState.update((prev) => ({
      ...prev,
      status: "error",
      errorMessage: "Phiên bản cài đặt hiện tại không hỗ trợ tự động cập nhật.",
    }));
    return;
  }

  updaterState.update((prev) => ({
    ...prev,
    status: "downloading",
    errorMessage: null,
    progress: { ...ZERO_PROGRESS },
  }));

  try {
    await downloadUpdate((p) => {
      applyProgress(p);
    });
    updaterState.update((prev) => {
      const doneBytes = prev.progress.total > 0 ? prev.progress.total : prev.progress.received;
      return {
        ...prev,
        status: "downloaded",
        progress: {
          received: doneBytes,
          total: doneBytes,
          percentage: 100,
        },
        errorMessage: null,
      };
    });
    notify("Tải bản cập nhật thành công! Khởi động lại ứng dụng để áp dụng.", "success");
  } catch (err) {
    const message = errorText(err, "Lỗi khi tải gói cập nhật.");
    updaterState.update((prev) => ({
      ...prev,
      status: "error",
      errorMessage: message,
    }));
    notify("Tải bản cập nhật thất bại.", "danger");
  }
}

/** Replaces the installed app with the downloaded update and restarts it. */
export async function relaunchToInstallUpdate(): Promise<void> {
  try {
    await installAndRestart();
  } catch (err) {
    const message = errorText(err, "Không thể khởi động lại để cài đặt bản cập nhật.");
    updaterState.update((prev) => ({
      ...prev,
      status: "error",
      errorMessage: message,
    }));
    notify("Không thể khởi động lại để cài đặt.", "danger");
  }
}

/** Answers a core updater step (`Host.onUpdaterRequest`) with `nsh/updater` and syncs UI state. */
export async function answerCoreUpdaterRequest(id: number, op: string): Promise<void> {
  try {
    if (op === "check") {
      const found = (await checkForUpdate()) as AppUpdate | null;
      if (found?.version) {
        updaterState.update((s) => ({
          ...s,
          status: "available",
          availableVersion: found.version,
          releaseNotes: found.notes ?? "",
          releaseUrl: found.url ?? null,
          canInstall: found.canInstall ?? true,
          errorMessage: null,
        }));
      } else {
        updaterState.update((s) => ({
          ...s,
          status: "up-to-date",
          availableVersion: null,
          releaseNotes: null,
          releaseUrl: null,
          errorMessage: null,
        }));
      }
      Host.answerUpdater(
        id,
        found?.version ? { version: found.version, notes: found.notes ?? "" } : null,
        null,
      );
    } else if (op === "download") {
      updaterState.update((s) => ({
        ...s,
        status: "downloading",
        errorMessage: null,
        progress: { ...ZERO_PROGRESS },
      }));
      await downloadUpdate((p) => {
        applyProgress(p);
      });
      updaterState.update((s) => {
        const doneBytes = s.progress.total > 0 ? s.progress.total : s.progress.received;
        return {
          ...s,
          status: "downloaded",
          progress: { received: doneBytes, total: doneBytes, percentage: 100 },
          errorMessage: null,
        };
      });
      Host.answerUpdater(id, null, null);
    } else if (op === "restart") {
      await installAndRestart();
      Host.answerUpdater(id, null, null);
    } else {
      Host.answerUpdater(id, null, `unknown updater step ${op}`);
    }
  } catch (err) {
    const message = errorText(err, "updater step failed");
    // A failed core-driven download must not leave the UI stuck in "downloading"
    // (it disables Settings buttons and blocks closing the modal).
    updaterState.update((s) =>
      s.status === "downloading" ? { ...s, status: "error", errorMessage: message } : s,
    );
    Host.answerUpdater(id, null, message);
  }
}
