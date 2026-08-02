import { describe, it, expect, mock, beforeEach } from 'bun:test';
import { openFilePicker, MAX_CLIPBOARD_IMAGE_BYTES } from '../conversationPlatform';

mock.module('@tauri-apps/plugin-dialog', () => ({
  open: mock(() => Promise.resolve(null)),
}));

mock.module('@tauri-apps/api/webview', () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: mock(() => Promise.resolve(() => {})),
  }),
}));

import { open } from '@tauri-apps/plugin-dialog';

describe('conversationPlatform', () => {
  beforeEach(() => {
    (open as any).mockReset();
  });

  describe('openFilePicker', () => {
    it('returns array of paths when multiple files selected', async () => {
      (open as any).mockImplementation(() => Promise.resolve(['/path/1.png', '/path/2.png']));

      const result = await openFilePicker({ multiple: true });
      expect(result).toEqual(['/path/1.png', '/path/2.png']);
      expect(open).toHaveBeenCalledWith({
        multiple: true,
        filters: undefined,
      });
    });

    it('normalizes single selection to array', async () => {
      (open as any).mockImplementation(() => Promise.resolve('/path/1.png'));

      const result = await openFilePicker({ multiple: false });
      expect(result).toEqual(['/path/1.png']);
    });

    it('returns empty array when user cancels dialog', async () => {
      (open as any).mockImplementation(() => Promise.resolve(null));

      const result = await openFilePicker();
      expect(result).toEqual([]);
    });
  });

  describe('MAX_CLIPBOARD_IMAGE_BYTES', () => {
    it('is set to 10 MiB', () => {
      expect(MAX_CLIPBOARD_IMAGE_BYTES).toBe(10 * 1024 * 1024);
    });
  });
});
