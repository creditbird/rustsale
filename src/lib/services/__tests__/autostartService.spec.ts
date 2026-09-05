import { beforeEach, describe, expect, mock, test } from 'bun:test';

let mockIsEnabledState = false;
let mockShouldThrow = false;
let enableCallCount = 0;
let disableCallCount = 0;

mock.module('@tauri-apps/plugin-autostart', () => ({
  isEnabled: async () => {
    if (mockShouldThrow) throw new Error('Plugin error or non-desktop environment');
    return mockIsEnabledState;
  },
  enable: async () => {
    if (mockShouldThrow) throw new Error('Permission denied');
    enableCallCount += 1;
    mockIsEnabledState = true;
  },
  disable: async () => {
    if (mockShouldThrow) throw new Error('Permission denied');
    disableCallCount += 1;
    mockIsEnabledState = false;
  }
}));

import { isAutostartEnabled, setAutostart } from '../autostartService';

describe('autostartService', () => {
  beforeEach(() => {
    mockIsEnabledState = false;
    mockShouldThrow = false;
    enableCallCount = 0;
    disableCallCount = 0;
  });

  test('isAutostartEnabled returns true when plugin reports enabled', async () => {
    mockIsEnabledState = true;
    const enabled = await isAutostartEnabled();
    expect(enabled).toBe(true);
  });

  test('isAutostartEnabled returns false when plugin reports disabled', async () => {
    mockIsEnabledState = false;
    const enabled = await isAutostartEnabled();
    expect(enabled).toBe(false);
  });

  test('isAutostartEnabled gracefully returns false on plugin error or non-desktop environment', async () => {
    mockShouldThrow = true;
    const enabled = await isAutostartEnabled();
    expect(enabled).toBe(false);
  });

  test('setAutostart(true) invokes enable()', async () => {
    await setAutostart(true);
    expect(enableCallCount).toBe(1);
    expect(disableCallCount).toBe(0);
    expect(mockIsEnabledState).toBe(true);
  });

  test('setAutostart(false) invokes disable()', async () => {
    mockIsEnabledState = true;
    await setAutostart(false);
    expect(disableCallCount).toBe(1);
    expect(enableCallCount).toBe(0);
    expect(mockIsEnabledState).toBe(false);
  });

  test('setAutostart throws when plugin fails', async () => {
    mockShouldThrow = true;
    await expect(setAutostart(true)).rejects.toThrow('Permission denied');
    await expect(setAutostart(false)).rejects.toThrow('Permission denied');
  });
});
