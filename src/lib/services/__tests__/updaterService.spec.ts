import { describe, test, expect, beforeEach } from 'bun:test';
import { updaterService, type UpdaterState, type UpdateAdapter } from '../updaterService';

describe('updaterService', () => {
  beforeEach(() => {
    updaterService.setMockAdapter(null);
    updaterService.setMockState({
      status: 'idle',
      currentVersion: '0.1.0',
      availableVersion: null,
      releaseNotes: null,
      releaseDate: null,
      progress: { downloaded: 0, total: 0, percentage: 0 },
      errorMessage: null,
      isModalOpen: false,
    });
  });

  test('initial state has idle status and version', () => {
    const state = updaterService.getState();
    expect(state.status).toBe('idle');
    expect(state.currentVersion).toBe('0.1.0');
    expect(state.availableVersion).toBeNull();
    expect(state.isModalOpen).toBe(false);
  });

  test('openModal and closeModal transitions isModalOpen', () => {
    updaterService.openModal();
    expect(updaterService.getState().isModalOpen).toBe(true);

    updaterService.closeModal();
    expect(updaterService.getState().isModalOpen).toBe(false);
  });

  test('subscribe receives state changes', () => {
    let captured: UpdaterState | null = null;
    const unsub = updaterService.subscribe((s) => {
      captured = s;
    });

    updaterService.setMockState({ status: 'available', availableVersion: '0.2.0' });
    expect((captured as UpdaterState | null)?.status).toBe('available');
    expect((captured as UpdaterState | null)?.availableVersion).toBe('0.2.0');

    unsub();
  });

  test('checkForUpdate in web fallback mode marks as up-to-date', async () => {
    const res = await updaterService.checkForUpdate({ silent: true });
    expect(res).toBeNull();
    expect(updaterService.getState().status).toBe('up-to-date');
  });

  test('checkForUpdate detects available version with mock adapter', async () => {
    const mockAdapter: UpdateAdapter = {
      version: '0.2.0',
      currentVersion: '0.1.0',
      body: 'Bug fixes and performance improvements',
      date: '2026-08-16T12:00:00Z',
      available: true,
      downloadAndInstall: async (onEvent) => {
        onEvent?.({ event: 'Started', data: { contentLength: 1000 } });
        onEvent?.({ event: 'Progress', data: { chunkLength: 500 } });
        onEvent?.({ event: 'Finished' });
      },
    };

    updaterService.setMockAdapter(mockAdapter);
    const updateInfo = await updaterService.checkForUpdate({ silent: true });

    expect(updateInfo).not.toBeNull();
    expect(updateInfo?.version).toBe('0.2.0');
    expect(updaterService.getState().status).toBe('available');
    expect(updaterService.getState().availableVersion).toBe('0.2.0');
    expect(updaterService.getState().isModalOpen).toBe(true);

    // Now test downloading with progress
    await updaterService.downloadAndInstall();
    expect(updaterService.getState().status).toBe('downloaded');
    expect(updaterService.getState().progress.percentage).toBe(100);
  });

  test('downloadAndInstall captures errors gracefully', async () => {
    const failingAdapter: UpdateAdapter = {
      version: '0.3.0',
      available: true,
      downloadAndInstall: async () => {
        throw new Error('Network timeout while fetching binary');
      },
    };

    updaterService.setMockAdapter(failingAdapter);
    await updaterService.checkForUpdate({ silent: true });
    await updaterService.downloadAndInstall();

    const state = updaterService.getState();
    expect(state.status).toBe('error');
    expect(state.errorMessage).toContain('Network timeout');
  });
});
