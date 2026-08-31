import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { m } from '$lib/i18n';

export interface NotificationSettings {
  enabled: boolean;
  sound_enabled: boolean;
  privacy_mode: boolean;
  mute_groups: boolean;
}

export interface TrayConfig {
  close_to_tray: boolean;
  dnd_enabled: boolean;
}

let activeConversationId: number | null = null;
let isWindowFocused = true;
let unlistenNotificationSent: UnlistenFn | null = null;
let unlistenTrayMarkAllRead: UnlistenFn | null = null;
let unlistenTrayDndChanged: UnlistenFn | null = null;

let onNavigateCallback: ((conversationId: number, leadId?: number) => void) | null = null;
let onMarkAllReadCallback: (() => void) | null = null;
let onDndChangedCallback: ((enabled: boolean) => void) | null = null;

export function formatNotificationPreviewText(
  rawText: string,
  contentType: string,
  privacyMode: boolean
): string {
  if (privacyMode) {
    try {
      return m.notif_preview_masked();
    } catch {
      return 'Bạn có tin nhắn mới';
    }
  }
  switch (contentType) {
    case 'image':
      try {
        return m.notif_preview_image();
      } catch {
        return '[Hình ảnh]';
      }
    case 'sticker':
      try {
        return m.notif_preview_sticker();
      } catch {
        return '[Nhãn dán]';
      }
    case 'file':
      try {
        return m.notif_preview_file();
      } catch {
        return '[Tệp đính kèm]';
      }
    case 'audio':
      return '[Audio]';
    case 'video':
      return '[Video]';
    default: {
      const trimmed = rawText.trim();
      if (!trimmed) {
        try {
          return m.notif_preview_masked();
        } catch {
          return 'Bạn có tin nhắn mới';
        }
      }
      if (trimmed.length > 120) {
        return trimmed.slice(0, 117) + '...';
      }
      return trimmed;
    }
  }
}

export function getActiveFocusState(): { activeConversationId: number | null; isFocused: boolean } {
  return { activeConversationId, isFocused: isWindowFocused };
}

let lastSyncedFocus: { convId: number | null; focused: boolean } | null = null;
let focusSyncSeq = 0;

export async function setConversationFocus(
  convId: number | null,
  focused?: boolean
): Promise<void> {
  activeConversationId = convId;
  if (typeof focused === 'boolean') {
    isWindowFocused = focused;
  }

  const targetConvId = activeConversationId;
  const targetFocused = isWindowFocused;

  if (
    lastSyncedFocus &&
    lastSyncedFocus.convId === targetConvId &&
    lastSyncedFocus.focused === targetFocused
  ) {
    return;
  }

  const seq = ++focusSyncSeq;

  try {
    const accepted = await invoke<boolean>('set_active_conversation_focus', {
      activeConversationId: targetConvId,
      isFocused: targetFocused,
      seq
    });
    if (accepted !== false && seq === focusSyncSeq) {
      lastSyncedFocus = { convId: targetConvId, focused: targetFocused };
    }
  } catch (err) {
    if (seq === focusSyncSeq) {
      lastSyncedFocus = null;
    }
    console.warn('Failed to sync conversation focus to backend:', err);
    throw err;
  }
}

export async function subscribeTrayDndChanged(callback: (enabled: boolean) => void): Promise<UnlistenFn> {
  return listen<boolean>('tray-dnd-changed', (event) => {
    callback(event.payload);
  });
}

export async function getNotificationSettings(): Promise<NotificationSettings> {
  try {
    const res = await invoke<NotificationSettings>('get_notification_settings');
    if (res && typeof res.enabled === 'boolean') {
      return res;
    }
  } catch (err) {
    console.warn('Failed to get notification settings, falling back to defaults:', err);
  }
  return {
    enabled: true,
    sound_enabled: true,
    privacy_mode: false,
    mute_groups: false
  };
}

export async function updateNotificationSettings(settings: NotificationSettings): Promise<void> {
  await invoke('update_notification_settings', { settings });
}

export async function testDesktopNotification(): Promise<void> {
  await invoke('test_desktop_notification');
}

export async function getTrayState(): Promise<TrayConfig> {
  try {
    const res = await invoke<TrayConfig>('get_tray_state');
    if (res && typeof res.close_to_tray === 'boolean') {
      return res;
    }
  } catch (err) {
    console.warn('Failed to get tray state:', err);
  }
  return {
    close_to_tray: true,
    dnd_enabled: false
  };
}

export async function setCloseToTray(enabled: boolean): Promise<void> {
  await invoke('set_close_to_tray', { enabled });
}

export async function setTrayDnd(enabled: boolean): Promise<void> {
  await invoke('set_tray_dnd', { enabled });
}

let sharedAudioCtx: AudioContext | null = null;

function getSharedAudioContext(): AudioContext | null {
  if (typeof window === 'undefined') return null;
  try {
    if (!sharedAudioCtx || sharedAudioCtx.state === 'closed') {
      const AudioContextClass =
        window.AudioContext ||
        (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
      if (!AudioContextClass) return null;
      sharedAudioCtx = new AudioContextClass();
    }
    if (sharedAudioCtx.state === 'suspended') {
      sharedAudioCtx.resume().catch(() => {});
    }
    return sharedAudioCtx;
  } catch (err) {
    console.debug('Notification chime audio context initialization error:', err);
    return null;
  }
}

export function _resetSharedAudioContextForTesting(): void {
  sharedAudioCtx = null;
}

export function _getSharedAudioContextForTesting(): AudioContext | null {
  return getSharedAudioContext();
}

export function playNotificationChime(): void {
  try {
    const ctx = getSharedAudioContext();
    if (!ctx) return;

    const now = ctx.currentTime;

    // Create oscillator 1 (587.33 Hz - D5)
    const osc1 = ctx.createOscillator();
    const gain1 = ctx.createGain();
    osc1.type = 'sine';
    osc1.frequency.setValueAtTime(587.33, now);
    gain1.gain.setValueAtTime(0, now);
    gain1.gain.linearRampToValueAtTime(0.18, now + 0.02);
    gain1.gain.exponentialRampToValueAtTime(0.001, now + 0.22);
    osc1.connect(gain1);
    gain1.connect(ctx.destination);
    osc1.start(now);
    osc1.stop(now + 0.25);

    // Create oscillator 2 (880.00 Hz - A5)
    const osc2 = ctx.createOscillator();
    const gain2 = ctx.createGain();
    osc2.type = 'sine';
    osc2.frequency.setValueAtTime(880.00, now + 0.08);
    gain2.gain.setValueAtTime(0, now + 0.08);
    gain2.gain.linearRampToValueAtTime(0.15, now + 0.1);
    gain2.gain.exponentialRampToValueAtTime(0.001, now + 0.35);
    osc2.connect(gain2);
    gain2.connect(ctx.destination);
    osc2.start(now + 0.08);
    osc2.stop(now + 0.38);
  } catch (err) {
    console.debug('Notification chime audio error:', err);
  }
}

export async function initDesktopNotificationBridge(options?: {
  onNavigate?: (conversationId: number, leadId?: number) => void;
  onMarkAllRead?: () => void;
  onDndChanged?: (enabled: boolean) => void;
}): Promise<() => void> {
  if (options?.onNavigate) {
    onNavigateCallback = options.onNavigate;
  }
  if (options?.onMarkAllRead) {
    onMarkAllReadCallback = options.onMarkAllRead;
  }
  if (options?.onDndChanged) {
    onDndChangedCallback = options.onDndChanged;
  }

  const handleFocus = () => {
    isWindowFocused = true;
    void setConversationFocus(activeConversationId, true).catch(() => {});
  };

  const handleBlur = () => {
    isWindowFocused = false;
    void setConversationFocus(activeConversationId, false).catch(() => {});
  };

  if (typeof window !== 'undefined') {
    window.addEventListener('focus', handleFocus);
    window.addEventListener('blur', handleBlur);
  }

  try {
    unlistenNotificationSent = await listen<{ conversationId?: number; leadId?: number; sound?: boolean; test?: boolean }>(
      'desktop-notification-sent',
      (event) => {
        if (event.payload?.sound) {
          playNotificationChime();
        }
      }
    );

    unlistenTrayMarkAllRead = await listen('tray-mark-all-read', () => {
      onMarkAllReadCallback?.();
    });

    unlistenTrayDndChanged = await listen<boolean>('tray-dnd-changed', (event) => {
      onDndChangedCallback?.(event.payload);
    });
  } catch (err) {
    console.warn('Failed to register notification listeners in frontend:', err);
  }

  return () => {
    if (typeof window !== 'undefined') {
      window.removeEventListener('focus', handleFocus);
      window.removeEventListener('blur', handleBlur);
    }
    if (unlistenNotificationSent) {
      unlistenNotificationSent();
      unlistenNotificationSent = null;
    }
    if (unlistenTrayMarkAllRead) {
      unlistenTrayMarkAllRead();
      unlistenTrayMarkAllRead = null;
    }
    if (unlistenTrayDndChanged) {
      unlistenTrayDndChanged();
      unlistenTrayDndChanged = null;
    }
    onNavigateCallback = null;
    onMarkAllReadCallback = null;
    onDndChangedCallback = null;
  };
}
