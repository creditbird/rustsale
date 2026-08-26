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
  formatNotificationPreviewText
} from '../desktopNotificationService';

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (cmd: string, args?: Record<string, unknown>) => {
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

  test('retrieves and updates tray settings', async () => {
    const tray = await getTrayState();
    expect(tray.close_to_tray).toBe(true);
    expect(tray.dnd_enabled).toBe(false);

    await expect(setCloseToTray(false)).resolves.toBeUndefined();
    await expect(setTrayDnd(true)).resolves.toBeUndefined();
  });
});
