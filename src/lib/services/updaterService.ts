import { toast } from 'svelte-sonner';

export type UpdateStatus =
  | 'idle'
  | 'checking'
  | 'available'
  | 'up-to-date'
  | 'downloading'
  | 'downloaded'
  | 'error';

export interface UpdateProgress {
  downloaded: number;
  total: number;
  percentage: number;
}

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  body?: string;
  date?: string;
}

export interface UpdaterState {
  status: UpdateStatus;
  currentVersion: string;
  availableVersion: string | null;
  releaseNotes: string | null;
  releaseDate: string | null;
  progress: UpdateProgress;
  errorMessage: string | null;
  isModalOpen: boolean;
}

export function isTauri(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof (window as any).__TAURI_INTERNALS__?.invoke === 'function' &&
    typeof (window as any).__TAURI_INTERNALS__?.transformCallback === 'function'
  );
}

export interface UpdateAdapter {
  version: string;
  currentVersion?: string;
  body?: string;
  date?: string;
  available: boolean;
  downloadAndInstall: (onEvent?: (event: any) => void) => Promise<void>;
}

class UpdaterStore {
  private state: UpdaterState = {
    status: 'idle',
    currentVersion: '0.1.0',
    availableVersion: null,
    releaseNotes: null,
    releaseDate: null,
    progress: {
      downloaded: 0,
      total: 0,
      percentage: 0,
    },
    errorMessage: null,
    isModalOpen: false,
  };

  private listeners = new Set<(state: UpdaterState) => void>();
  private activeUpdateHandle: UpdateAdapter | null = null;
  private mockAdapter: UpdateAdapter | null = null;

  constructor() {
    this.initVersion();
  }

  private async initVersion() {
    if (isTauri()) {
      try {
        const { getVersion } = await import('@tauri-apps/api/app');
        const v = await getVersion();
        if (v) {
          this.state.currentVersion = v;
          this.notify();
        }
      } catch (err) {
        console.warn('[Updater] Failed to get app version from Tauri:', err);
      }
    }
  }

  public getState(): UpdaterState {
    return { ...this.state, progress: { ...this.state.progress } };
  }

  public subscribe(listener: (state: UpdaterState) => void): () => void {
    this.listeners.add(listener);
    listener(this.getState());
    return () => {
      this.listeners.delete(listener);
    };
  }

  private notify() {
    const s = this.getState();
    for (const listener of this.listeners) {
      listener(s);
    }
  }

  public openModal() {
    this.state.isModalOpen = true;
    this.notify();
  }

  public closeModal() {
    this.state.isModalOpen = false;
    this.notify();
  }

  public setMockAdapter(adapter: UpdateAdapter | null) {
    this.mockAdapter = adapter;
  }

  public async checkForUpdate(options: { silent?: boolean } = {}): Promise<UpdateInfo | null> {
    const { silent = false } = options;
    this.state.status = 'checking';
    this.state.errorMessage = null;
    this.notify();

    // Use mock adapter if provided (for testing or dev simulation)
    if (this.mockAdapter) {
      if (this.mockAdapter.available) {
        this.activeUpdateHandle = this.mockAdapter;
        this.state.status = 'available';
        this.state.availableVersion = this.mockAdapter.version;
        this.state.releaseNotes = this.mockAdapter.body || 'Bản cập nhật mới với nhiều cải tiến và vá lỗi.';
        this.state.releaseDate = this.mockAdapter.date || new Date().toISOString();
        this.state.isModalOpen = true;
        this.notify();
        if (silent) {
          toast.success(`Đã có phiên bản mới v${this.mockAdapter.version}!`, {
            action: {
              label: 'Xem chi tiết',
              onClick: () => this.openModal(),
            },
          });
        }
        return {
          version: this.mockAdapter.version,
          currentVersion: this.mockAdapter.currentVersion || this.state.currentVersion,
          body: this.mockAdapter.body,
          date: this.mockAdapter.date,
        };
      } else {
        this.state.status = 'up-to-date';
        this.state.availableVersion = null;
        this.notify();
        if (!silent) {
          toast.success('Bạn đang sử dụng phiên bản mới nhất.');
        }
        return null;
      }
    }

    if (!isTauri()) {
      // Running in standard browser or dev without Tauri native bindings
      this.state.status = 'up-to-date';
      this.notify();
      if (!silent) {
        toast.info('Bạn đang sử dụng phiên bản mới nhất (Môi trường Web/Dev).');
      }
      return null;
    }

    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const update = await check();

      if (update && update.available) {
        this.activeUpdateHandle = update;
        this.state.status = 'available';
        this.state.availableVersion = update.version;
        this.state.releaseNotes = update.body || 'Bản cập nhật mới với nhiều cải tiến và vá lỗi.';
        this.state.releaseDate = update.date || new Date().toISOString();
        this.state.isModalOpen = true;
        this.notify();

        if (silent) {
          toast.success(`Đã có phiên bản mới v${update.version}!`, {
            action: {
              label: 'Xem chi tiết',
              onClick: () => this.openModal(),
            },
          });
        }
        return {
          version: update.version,
          currentVersion: update.currentVersion || this.state.currentVersion,
          body: update.body,
          date: update.date,
        };
      } else {
        this.state.status = 'up-to-date';
        this.state.availableVersion = null;
        this.notify();
        if (!silent) {
          toast.success('Bạn đang sử dụng phiên bản mới nhất.');
        }
        return null;
      }
    } catch (err: any) {
      const errorMsg = err?.message || String(err) || 'Không thể kiểm tra bản cập nhật.';
      console.error('[Updater] Check failed:', err);
      this.state.status = 'error';
      this.state.errorMessage = errorMsg;
      this.notify();

      if (!silent) {
        toast.error(`Lỗi kiểm tra cập nhật: ${errorMsg}`);
      }
      return null;
    }
  }

  public async downloadAndInstall(): Promise<void> {
    if (!this.activeUpdateHandle) {
      // Fallback if simulate
      if (!isTauri()) {
        this.state.status = 'downloading';
        this.state.progress = { downloaded: 50, total: 100, percentage: 50 };
        this.notify();
        await new Promise((r) => setTimeout(r, 50));
        this.state.status = 'downloaded';
        this.state.progress = { downloaded: 100, total: 100, percentage: 100 };
        this.notify();
        return;
      }
      toast.error('Không tìm thấy thông tin bản cập nhật để tải về.');
      return;
    }

    this.state.status = 'downloading';
    this.state.errorMessage = null;
    this.state.progress = { downloaded: 0, total: 0, percentage: 0 };
    this.notify();

    let downloadedBytes = 0;
    let contentLength = 0;

    try {
      await this.activeUpdateHandle.downloadAndInstall((event: any) => {
        if (!event) return;
        switch (event.event) {
          case 'Started':
            contentLength = event.data?.contentLength || 0;
            this.state.progress.total = contentLength;
            this.state.progress.downloaded = 0;
            this.state.progress.percentage = 0;
            this.notify();
            break;
          case 'Progress':
            downloadedBytes += event.data?.chunkLength || 0;
            this.state.progress.downloaded = downloadedBytes;
            if (contentLength > 0) {
              this.state.progress.percentage = Math.min(
                100,
                Math.round((downloadedBytes / contentLength) * 100)
              );
            }
            this.notify();
            break;
          case 'Finished':
            this.state.progress.percentage = 100;
            this.notify();
            break;
        }
      });

      this.state.status = 'downloaded';
      this.notify();
      toast.success('Đã tải xong bản cập nhật! Khởi động lại ứng dụng để áp dụng.');
    } catch (err: any) {
      const errorMsg = err?.message || String(err) || 'Lỗi trong quá trình tải bản cập nhật.';
      console.error('[Updater] Download & install failed:', err);

      // Smart Fallback for Linux (Arch, CachyOS, Fedora, openSUSE, portable user installs)
      const isLinux = typeof navigator !== 'undefined' && /linux/i.test(navigator.userAgent || '');
      const isPackageInstallFailure =
        errorMsg.includes('Failed to install package') ||
        errorMsg.includes('dpkg') ||
        errorMsg.includes('command not found') ||
        errorMsg.includes('permission denied');

      if (isTauri() && (isLinux || isPackageInstallFailure) && this.state.availableVersion) {
        console.log('[Updater] Native installer failed or non-Debian Linux detected. Triggering Smart In-App Portable Updater fallback...');
        try {
          this.state.status = 'downloading';
          this.state.errorMessage = null;
          this.state.progress = { downloaded: 50, total: 100, percentage: 50 };
          this.notify();

          const { invoke } = await import('@tauri-apps/api/core');
          await invoke('install_portable_linux_update', {
            version: this.state.availableVersion,
          });

          this.state.status = 'downloaded';
          this.state.progress = { downloaded: 100, total: 100, percentage: 100 };
          this.notify();
          toast.success('Đã tự động cập nhật bản mới thành công! Khởi động lại ứng dụng để áp dụng.');
          return;
        } catch (fallbackErr: any) {
          const fallbackMsg = fallbackErr?.message || String(fallbackErr) || errorMsg;
          console.error('[Updater] Portable fallback failed:', fallbackErr);
          this.state.status = 'error';
          this.state.errorMessage = fallbackMsg;
          this.notify();
          toast.error(`Cập nhật thất bại: ${fallbackMsg}`);
          return;
        }
      }

      this.state.status = 'error';
      this.state.errorMessage = errorMsg;
      this.notify();
      toast.error(`Cập nhật thất bại: ${errorMsg}`);
    }
  }

  public async relaunch(): Promise<void> {
    if (isTauri()) {
      try {
        const { relaunch } = await import('@tauri-apps/plugin-process');
        await relaunch();
        return;
      } catch (err) {
        console.warn('[Updater] plugin-process relaunch failed:', err);
      }
    }
    toast.info('Vui lòng khởi động lại ứng dụng thủ công.');
  }

  public setMockState(partial: Partial<UpdaterState>) {
    this.state = { ...this.state, ...partial };
    this.notify();
  }
}

export const updaterService = new UpdaterStore();
