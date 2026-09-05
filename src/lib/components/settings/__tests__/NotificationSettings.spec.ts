import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';
import { setLocale } from '$lib/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import NotificationSettings from '../NotificationSettings.svelte';
import * as desktopNotificationService from '$lib/services/desktopNotificationService';

let mockNotificationSettings = {
  enabled: true,
  sound_enabled: true,
  privacy_mode: false,
  mute_groups: false
};

let mockTrayState = {
  close_to_tray: true,
  dnd_enabled: false
};

let mockAutostartEnabled = false;
let shouldRejectUpdate = false;
let testNotificationCalled = false;

mock.module('$lib/services/autostartService', () => ({
  isAutostartEnabled: async () => mockAutostartEnabled,
  setAutostart: async (val: boolean) => {
    if (shouldRejectUpdate) throw new Error('Autostart update failed');
    mockAutostartEnabled = val;
  }
}));

mock.module('$lib/services/desktopNotificationService', () => ({
  getNotificationSettings: async () => ({ ...mockNotificationSettings }),
  updateNotificationSettings: async (settings: any) => {
    if (shouldRejectUpdate) throw new Error('Update failed');
    mockNotificationSettings = { ...settings };
  },
  getTrayState: async () => ({ ...mockTrayState }),
  setCloseToTray: async (val: boolean) => {
    if (shouldRejectUpdate) throw new Error('Update failed');
    mockTrayState.close_to_tray = val;
  },
  setTrayDnd: async (val: boolean) => {
    if (shouldRejectUpdate) throw new Error('Update failed');
    mockTrayState.dnd_enabled = val;
  },
  testDesktopNotification: async () => {
    if (shouldRejectUpdate) throw new Error('Test failed');
    testNotificationCalled = true;
  }
}));

describe('NotificationSettings component (E4 Runtime Multi-Language Verification)', () => {
  beforeEach(() => {
    cleanup();
    mockNotificationSettings = {
      enabled: true,
      sound_enabled: true,
      privacy_mode: false,
      mute_groups: false
    };
    mockTrayState = {
      close_to_tray: true,
      dnd_enabled: false
    };
    mockAutostartEnabled = false;
    shouldRejectUpdate = false;
    testNotificationCalled = false;
  });

  afterEach(() => {
    cleanup();
  });

  test('E4-vi: renders fully localized Vietnamese UI elements and badges', async () => {
    setLocale('vi');
    render(NotificationSettings);

    expect(await screen.findByText('Thông báo & Khay hệ thống')).toBeTruthy();
    expect(await screen.findByText('Bật thông báo trên màn hình (Desktop Notifications)')).toBeTruthy();
    expect(await screen.findByText('Thu nhỏ xuống khay hệ thống khi đóng (Close to Tray)')).toBeTruthy();
    expect(await screen.findByText('Khởi động cùng hệ thống (Launch at Startup)')).toBeTruthy();
    expect(await screen.findByText('Chế độ riêng tư (Privacy Mode)')).toBeTruthy();
    expect(await screen.findByText('Âm thanh thông báo (Sound Chime)')).toBeTruthy();
    expect(await screen.findByText('Tắt thông báo nhóm chat')).toBeTruthy();
    expect(await screen.findByText('Hoạt động')).toBeTruthy();
    expect(await screen.findByText('Chạy ngầm')).toBeTruthy();

    const btn = await screen.findByRole('button', { name: /Gửi thông báo thử nghiệm/i });
    expect(btn).toBeTruthy();
    await fireEvent.click(btn);
    expect(testNotificationCalled).toBe(true);
  });

  test('E4-en: renders fully localized English UI elements and badges', async () => {
    setLocale('en');
    render(NotificationSettings);

    expect(await screen.findByText('Notifications & System Tray')).toBeTruthy();
    expect(await screen.findByText('Enable Desktop Notifications')).toBeTruthy();
    expect(await screen.findByText('Minimize to System Tray on Close')).toBeTruthy();
    expect(await screen.findByText('Launch at System Startup')).toBeTruthy();
    expect(await screen.findByText('Privacy Mode')).toBeTruthy();
    expect(await screen.findByText('Notification Sound (Chime)')).toBeTruthy();
    expect(await screen.findByText('Mute Group Chats')).toBeTruthy();
    expect(await screen.findByText('Active')).toBeTruthy();
    expect(await screen.findByText('Background')).toBeTruthy();

    const btn = await screen.findByRole('button', { name: /Test Notification/i });
    expect(btn).toBeTruthy();
    await fireEvent.click(btn);
    expect(testNotificationCalled).toBe(true);
  });

  test('E4-dnd: renders DND active alert with localized disable button when DND is on', async () => {
    mockTrayState.dnd_enabled = true;
    setLocale('vi');
    render(NotificationSettings);

    expect(await screen.findByText(/Chế độ Không làm phiền \(DND\) đang được bật/i)).toBeTruthy();
    const disableDndBtn = await screen.findByRole('button', { name: /Tắt DND/i });
    expect(disableDndBtn).toBeTruthy();

    await fireEvent.click(disableDndBtn);
    expect(mockTrayState.dnd_enabled).toBe(false);
  });

  test('E4-toggle: toggling settings updates state and invokes backend commands', async () => {
    setLocale('vi');
    render(NotificationSettings);

    const switches = await screen.findAllByRole('switch');
    expect(switches.length).toBe(6);

    // Toggle Desktop Notifications (first switch)
    await fireEvent.click(switches[0]);
    expect(mockNotificationSettings.enabled).toBe(false);

    // Toggle Close to Tray (second switch)
    await fireEvent.click(switches[1]);
    expect(mockTrayState.close_to_tray).toBe(false);

    // Toggle Launch at System Startup (third switch)
    await fireEvent.click(switches[2]);
    expect(mockAutostartEnabled).toBe(true);
  });
});
