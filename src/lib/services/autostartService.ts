import { enable, isEnabled, disable } from '@tauri-apps/plugin-autostart';

/**
 * Checks whether the application is configured to launch on system startup.
 * Returns false if running outside the Tauri desktop runtime or if checking fails.
 */
export async function isAutostartEnabled(): Promise<boolean> {
  try {
    return await isEnabled();
  } catch (err) {
    console.warn('[autostartService] Failed to check autostart status:', err);
    return false;
  }
}

/**
 * Enables or disables autostart on system boot for the current platform (Linux/Windows/macOS).
 */
export async function setAutostart(enableApp: boolean): Promise<void> {
  try {
    if (enableApp) {
      await enable();
    } else {
      await disable();
    }
  } catch (err) {
    console.error('[autostartService] Failed to set autostart status:', err);
    throw err;
  }
}
