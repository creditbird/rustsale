import { afterEach, beforeEach, describe, expect, mock, spyOn, test } from 'bun:test';
import { m, setLocale } from '$lib/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import NotificationSettings from '../NotificationSettings.svelte';
import * as desktopNotificationService from '$lib/services/desktopNotificationService';
import { toast } from 'svelte-sonner';

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
  },
  subscribeTrayDndChanged: () => () => {}
}));

describe('NotificationSettings component (E2 Component Mock Verification)', () => {
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

  test('E2-vi: renders fully localized Vietnamese UI elements and badges', async () => {
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

  test('E2-en: renders fully localized English UI elements and badges', async () => {
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

  test('E2-dnd: renders DND active alert with localized disable button when DND is on', async () => {
    mockTrayState.dnd_enabled = true;
    setLocale('vi');
    render(NotificationSettings);

    expect(await screen.findByText(/Chế độ Không làm phiền \(DND\) đang được bật/i)).toBeTruthy();
    const disableDndBtn = await screen.findByRole('button', { name: /Tắt DND/i });
    expect(disableDndBtn).toBeTruthy();

    await fireEvent.click(disableDndBtn);
    expect(mockTrayState.dnd_enabled).toBe(false);
  });

  test('E2-toggle: toggling all 6 settings switches updates state and invokes backend commands', async () => {
    setLocale('vi');
    render(NotificationSettings);

    const switches = await screen.findAllByRole('switch');
    expect(switches.length).toBe(6);

    // 0. Toggle Desktop Notifications
    await fireEvent.click(switches[0]);
    expect(mockNotificationSettings.enabled).toBe(false);

    // 1. Toggle Close to Tray
    await fireEvent.click(switches[1]);
    expect(mockTrayState.close_to_tray).toBe(false);

    // 2. Toggle Launch at System Startup
    await fireEvent.click(switches[2]);
    expect(mockAutostartEnabled).toBe(true);

    // 3. Toggle Notification Sound
    await fireEvent.click(switches[3]);
    expect(mockNotificationSettings.sound_enabled).toBe(false);

    // 4. Toggle Privacy Mode
    await fireEvent.click(switches[4]);
    expect(mockNotificationSettings.privacy_mode).toBe(true);

    // 5. Toggle Mute Group Chats
    await fireEvent.click(switches[5]);
    expect(mockNotificationSettings.mute_groups).toBe(true);
  });

  test('E2-rollback: reverts autostart switch and shows error toast when backend call fails', async () => {
    setLocale('vi');
    let toastErrorMessage = '';
    const toastErrorSpy = spyOn(toast, 'error').mockImplementation((msg: any) => {
      toastErrorMessage = String(msg);
      return '' as any;
    });

    render(NotificationSettings);

    const switches = await screen.findAllByRole('switch');
    const autostartSwitch = switches[2];
    expect(autostartSwitch.getAttribute('aria-checked')).toBe('false');

    shouldRejectUpdate = true;

    // Toggle Autostart (switch 2) when backend rejects
    await fireEvent.click(autostartSwitch);

    // Verify UI state is rolled back to false
    await waitFor(() => {
      expect(autostartSwitch.getAttribute('aria-checked')).toBe('false');
    });

    // Verify error toast was shown with the localized error message
    expect(toastErrorMessage).toBe(m.settings_notif_err_update_autostart());

    toastErrorSpy.mockRestore();
  });
});
