import { beforeEach, describe, expect, mock, test } from 'bun:test';
import { setLocale } from '$lib/i18n';
import {
  setConversationFocus,
  getActiveFocusState,
  getNotificationSettings,
  updateNotificationSettings,
  getTrayState,
  setCloseToTray,
  setTrayDnd,
  formatNotificationPreviewText,
  playNotificationChime,
  initDesktopNotificationBridge,
  subscribeTrayDndChanged,
  _resetSharedAudioContextForTesting,
  _getSharedAudioContextForTesting
} from '../desktopNotificationService';

let mockInvokeFailure = false;
let customFocusHandler: ((args: Record<string, unknown>) => Promise<boolean>) | null = null;
let backendFocusState = { convId: null as number | null, isFocused: true, seq: 0 };
const eventListeners: Record<string, ((event: { payload: unknown }) => void)[]> = {};

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (cmd: string, args?: Record<string, unknown>) => {
    if (mockInvokeFailure && cmd === 'set_active_conversation_focus') {
      throw new Error('Simulated_IPC_Focus_Failure');
    }
    if (cmd === 'set_active_conversation_focus') {
      if (customFocusHandler) {
        return customFocusHandler(args || {});
      }
      const seq = (args?.seq as number) || 0;
      if (seq > backendFocusState.seq) {
        backendFocusState = {
          convId: (args?.activeConversationId as number | null) ?? null,
          isFocused: (args?.isFocused as boolean) ?? true,
          seq
        };
        return true;
      }
      return false; // Stale sequence rejected
    }
    if (cmd === 'get_notification_settings') {
      return { enabled: true, sound_enabled: true, privacy_mode: false, mute_groups: false };
    }
    if (cmd === 'update_notification_settings') {
      return undefined;
    }
    if (cmd === 'get_tray_state') {
      return { close_to_tray: true, dnd_enabled: false };
    }
    if (cmd === 'set_close_to_tray' || cmd === 'set_tray_dnd') {
      return undefined;
    }
    return undefined;
  }
}));

mock.module('@tauri-apps/api/event', () => ({
  listen: async (eventName: string, callback: (event: { payload: unknown }) => void) => {
    if (!eventListeners[eventName]) {
      eventListeners[eventName] = [];
    }
    eventListeners[eventName].push(callback);
    return () => {
      eventListeners[eventName] = eventListeners[eventName].filter((cb) => cb !== callback);
    };
  }
}));

describe('desktopNotificationService (Multi-Language & Focus Engine)', () => {
  beforeEach(() => {
    setLocale('vi');
    mockInvokeFailure = false;
    customFocusHandler = null;
    backendFocusState = { convId: null, isFocused: true, seq: 0 };
    _resetSharedAudioContextForTesting();
  });

  test('formats preview text correctly in Vietnamese', () => {
    setLocale('vi');
    expect(formatNotificationPreviewText('Xin chào', 'text', false)).toBe('Xin chào');
    expect(formatNotificationPreviewText('Bí mật', 'text', true)).toBe('Bạn có tin nhắn mới');
    expect(formatNotificationPreviewText('', 'image', false)).toBe('[Hình ảnh]');
    expect(formatNotificationPreviewText('', 'sticker', false)).toBe('[Nhãn dán]');
    expect(formatNotificationPreviewText('', 'file', false)).toBe('[Tệp đính kèm]');
  });

  test('formats preview text correctly in English', () => {
    setLocale('en');
    expect(formatNotificationPreviewText('Hello', 'text', false)).toBe('Hello');
    expect(formatNotificationPreviewText('Secret', 'text', true)).toBe('You have a new message');
    expect(formatNotificationPreviewText('', 'image', false)).toBe('[Image]');
    expect(formatNotificationPreviewText('', 'sticker', false)).toBe('[Sticker]');
    expect(formatNotificationPreviewText('', 'file', false)).toBe('[Attachment]');
  });

  test('tracks conversation focus state', async () => {
    await setConversationFocus(123, true);
    const state = getActiveFocusState();
    expect(state.activeConversationId).toBe(123);
    expect(state.isFocused).toBe(true);

    await setConversationFocus(null, false);
    const updated = getActiveFocusState();
    expect(updated.activeConversationId).toBeNull();
    expect(updated.isFocused).toBe(false);
  });

  test('preserves monotonic backend focus state when earlier IPC execution is delayed behind newer IPC', async () => {
    let releaseCall1: () => void = () => {};
    const call1Deferred = new Promise<void>((resolve) => {
      releaseCall1 = resolve;
    });

    let callCount = 0;
    customFocusHandler = async (args) => {
      callCount++;
      const seq = (args.seq as number) || 0;
      if (callCount === 1) {
        // Force Call 1 to pause at the barrier until released
        await call1Deferred;
      }
      if (seq > backendFocusState.seq) {
        backendFocusState = {
          convId: (args.activeConversationId as number | null) ?? null,
          isFocused: (args.isFocused as boolean) ?? true,
          seq
        };
        return true;
      }
      return false; // Stale sequence rejected
    };

    // Trigger Call 1 for conv 100 (which gets delayed at the barrier)
    const p1 = setConversationFocus(100, true);

    // Trigger Call 2 for conv 200 (which completes immediately)
    const p2 = setConversationFocus(200, true);
    await p2;

    // Backend must currently be at conv 200
    expect(backendFocusState.convId).toBe(200);

    // Now release Call 1 to finish its execution
    releaseCall1();
    await p1;

    // Backend state must NOT have regressed to conv 100
    expect(backendFocusState.convId).toBe(200);
    expect(getActiveFocusState().activeConversationId).toBe(200);
  });

  test('retrieves and updates notification settings', async () => {
    const settings = await getNotificationSettings();
    expect(settings.enabled).toBe(true);
    expect(settings.sound_enabled).toBe(true);

    await expect(
      updateNotificationSettings({
        enabled: false,
        sound_enabled: false,
        privacy_mode: true,
        mute_groups: true
      })
    ).resolves.toBeUndefined();
  });

  test('recovers and allows retry when IPC focus synchronization fails', async () => {
    // Inject transient failure
    mockInvokeFailure = true;
    await expect(setConversationFocus(999, true)).rejects.toThrow('Simulated_IPC_Focus_Failure');

    // Recovery: retry with the exact same state succeeds and is not blocked by stale cache
    mockInvokeFailure = false;
    await expect(setConversationFocus(999, true)).resolves.toBeUndefined();
    const state = getActiveFocusState();
    expect(state.activeConversationId).toBe(999);
    expect(state.isFocused).toBe(true);
  });

  test('manages tray state and toggles dnd/close-to-tray', async () => {
    const trayState = await getTrayState();
    expect(trayState.close_to_tray).toBe(true);
    expect(trayState.dnd_enabled).toBe(false);

    await expect(setCloseToTray(false)).resolves.toBeUndefined();
    await expect(setTrayDnd(true)).resolves.toBeUndefined();
  });

  test('subscribes to tray-dnd-changed events and receives updates', async () => {
    let capturedDnd = false;
    const unsub = await subscribeTrayDndChanged((enabled) => {
      capturedDnd = enabled;
    });

    // Simulate backend tray-dnd-changed emission
    eventListeners['tray-dnd-changed']?.forEach((cb) => cb({ payload: true }));
    expect(capturedDnd).toBe(true);

    eventListeners['tray-dnd-changed']?.forEach((cb) => cb({ payload: false }));
    expect(capturedDnd).toBe(false);

    unsub();
  });

  test('initializes desktop notification bridge and processes events', async () => {
    let dndState = false;
    let markAllReadCalled = false;

    const cleanup = await initDesktopNotificationBridge({
      onMarkAllRead: () => {
        markAllReadCalled = true;
      },
      onDndChanged: (enabled) => {
        dndState = enabled;
      }
    });

    // Fire mock events
    eventListeners['tray-mark-all-read']?.forEach((cb) => cb({ payload: null }));
    expect(markAllReadCalled).toBe(true);

    eventListeners['tray-dnd-changed']?.forEach((cb) => cb({ payload: true }));
    expect(dndState).toBe(true);

    expect(typeof cleanup).toBe('function');
    cleanup();
  });

  test('reuses single AudioContext instance and safely restores environment', () => {
    const originalWindow = (globalThis as unknown as { window?: unknown }).window;
    let instanceCount = 0;
    let resumeCallCount = 0;

    class MockAudioContext {
      currentTime = 0;
      state = 'suspended';
      destination = {};
      constructor() {
        instanceCount++;
      }
      createGain() {
        return {
          gain: {
            setValueAtTime: () => {},
            linearRampToValueAtTime: () => {},
            exponentialRampToValueAtTime: () => {}
          },
          connect: () => {}
        };
      }
      createOscillator() {
        return {
          type: 'sine',
          frequency: { setValueAtTime: () => {} },
          connect: () => {},
          start: () => {},
          stop: () => {}
        };
      }
      resume() {
        resumeCallCount++;
        this.state = 'running';
        return Promise.resolve();
      }
    }

    try {
      const fakeWindow = {
        AudioContext: MockAudioContext
      };
      (globalThis as unknown as { window: unknown }).window = fakeWindow;

      playNotificationChime();
      expect(instanceCount).toBe(1);
      expect(resumeCallCount).toBe(1);

      // Second call reuses same instance and does not instantiate a second AudioContext
      playNotificationChime();
      expect(instanceCount).toBe(1);
    } finally {
      if (originalWindow !== undefined) {
        (globalThis as unknown as { window: unknown }).window = originalWindow;
      } else {
        delete (globalThis as unknown as { window?: unknown }).window;
      }
    }
  });
});
