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
  initDesktopNotificationBridge
} from '../desktopNotificationService';

let mockInvokeFailure = false;

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (cmd: string, args?: Record<string, unknown>) => {
    if (mockInvokeFailure && cmd === 'set_active_conversation_focus') {
      throw new Error('Simulated_IPC_Focus_Failure');
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
    if (cmd === 'set_close_to_tray' || cmd === 'set_tray_dnd' || cmd === 'set_active_conversation_focus') {
      return undefined;
    }
    return undefined;
  }
}));

mock.module('@tauri-apps/api/event', () => ({
  listen: async (_eventName: string, _callback: (event: unknown) => void) => {
    return () => {};
  }
}));

describe('desktopNotificationService (Multi-Language & Focus Engine)', () => {
  beforeEach(() => {
    setLocale('vi');
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

  test('initializes desktop notification bridge and unregisters cleanly', async () => {
    let dndState: boolean | null = null;
    let markAllReadCalled = false;
    let navigatedId: number | null = null;

    const cleanup = await initDesktopNotificationBridge({
      onNavigate: (convId) => {
        navigatedId = convId;
      },
      onMarkAllRead: () => {
        markAllReadCalled = true;
      },
      onDndChanged: (enabled) => {
        dndState = enabled;
      }
    });

    expect(typeof cleanup).toBe('function');
    cleanup();
  });

  test('executes playNotificationChime safely without exceptions', () => {
    // Mock WebAudio AudioContext
    const mockDestination = {};
    const mockGainNode = {
      gain: {
        setValueAtTime: () => {},
        linearRampToValueAtTime: () => {},
        exponentialRampToValueAtTime: () => {}
      },
      connect: () => {}
    };
    const mockOscillatorNode = {
      type: 'sine',
      frequency: {
        setValueAtTime: () => {}
      },
      connect: () => {},
      start: () => {},
      stop: () => {}
    };

    class MockAudioContext {
      currentTime = 0;
      state = 'running';
      destination = mockDestination;
      createGain() {
        return mockGainNode;
      }
      createOscillator() {
        return mockOscillatorNode;
      }
      resume() {
        return Promise.resolve();
      }
    }

    (globalThis as unknown as { AudioContext: typeof MockAudioContext }).AudioContext = MockAudioContext;

    expect(() => playNotificationChime()).not.toThrow();
  });
});
