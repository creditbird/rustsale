/// <reference types="bun-types" />
import { describe, expect, test as it, mock, beforeEach, afterEach } from 'bun:test';
import {
  isTauriEnvironment,
  sanitizeImageFilename,
  saveImageToFile,
  type ImageExportDeps
} from '../imageExport';

describe('imageExport utility', () => {
  const originalWindow = globalThis.window;

  beforeEach(() => {
    mock.restore();
  });

  afterEach(() => {
    // restore window properties if modified
  });

  describe('sanitizeImageFilename', () => {
    it('returns provided valid filename', () => {
      expect(sanitizeImageFilename('my_picture.png')).toBe('my_picture.png');
      expect(sanitizeImageFilename('photo.jpg')).toBe('photo.jpg');
      expect(sanitizeImageFilename('graphic.webp')).toBe('graphic.webp');
    });

    it('appends .png if no extension provided in name and no URL extension available', () => {
      expect(sanitizeImageFilename('my_picture')).toBe('my_picture.png');
    });

    it('derives extension from fallbackUrl when name lacks one', () => {
      expect(sanitizeImageFilename('custom_name', 'https://cdn.example.com/photo.webp')).toBe('custom_name.webp');
      expect(sanitizeImageFilename('custom_name', 'https://cdn.example.com/photo.jpg?token=123')).toBe('custom_name.jpg');
      expect(sanitizeImageFilename('custom_name', 'https://cdn.example.com/photo.tiff')).toBe('custom_name.tiff');
    });

    it('sanitizes illegal path characters from filename', () => {
      expect(sanitizeImageFilename('my:pic/name?.jpg')).toBe('my_pic_name_.jpg');
    });

    it('extracts filename from URL when name is missing', () => {
      expect(sanitizeImageFilename(null, 'https://cdn.example.com/assets/banner.jpg')).toBe('banner.jpg');
      expect(sanitizeImageFilename('', 'https://cdn.example.com/images/icon.png?v=2')).toBe('icon.png');
      expect(sanitizeImageFilename('', 'https://cdn.example.com/images/scan.tiff')).toBe('scan.tiff');
    });

    it('falls back to timestamp filename if neither name nor url has valid filename', () => {
      const fallback = sanitizeImageFilename(null, 'https://cdn.example.com/raw-stream');
      expect(fallback).toMatch(/^image_\d{8}_\d{6}\.png$/);
    });
  });

  describe('isTauriEnvironment', () => {
    it('returns true when __TAURI_INTERNALS__ exists on window', () => {
      const prev = (globalThis as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
      (globalThis as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
      expect(isTauriEnvironment()).toBe(true);
      if (prev !== undefined) {
        (globalThis as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = prev;
      } else {
        delete (globalThis as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
      }
    });
  });

  describe('saveImageToFile in Tauri environment', () => {
    it('invokes save_image_file command and shows success toast on valid save path', async () => {
      const invokeMock = mock((cmd: string, args?: Record<string, unknown>) => {
        if (cmd === 'save_image_file') {
          return Promise.resolve('/home/user/Pictures/sample.png');
        }
        return Promise.reject(new Error(`Unexpected command: ${cmd}`));
      });
      const toastSuccessMock = mock((_msg: string) => {});
      const toastErrorMock = mock((_msg: string) => {});

      const deps: ImageExportDeps = {
        isTauri: () => true,
        invokeCommand: invokeMock as unknown as ImageExportDeps['invokeCommand'],
        showToastSuccess: toastSuccessMock,
        showToastError: toastErrorMock
      };

      const result = await saveImageToFile('https://example.com/sample.png', 'sample.png', deps);

      expect(invokeMock).toHaveBeenCalledTimes(1);
      expect(invokeMock).toHaveBeenCalledWith('save_image_file', {
        defaultFilename: 'sample.png',
        url: 'https://example.com/sample.png'
      });
      expect(toastSuccessMock).toHaveBeenCalledTimes(1);
      expect(toastErrorMock).not.toHaveBeenCalled();
      expect(result).toBe('/home/user/Pictures/sample.png');
    });

    it('handles cancellation gracefully when save dialog returns null', async () => {
      const invokeMock = mock((_cmd: string) => Promise.resolve(null));
      const toastSuccessMock = mock((_msg: string) => {});
      const toastErrorMock = mock((_msg: string) => {});

      const deps: ImageExportDeps = {
        isTauri: () => true,
        invokeCommand: invokeMock as unknown as ImageExportDeps['invokeCommand'],
        showToastSuccess: toastSuccessMock,
        showToastError: toastErrorMock
      };

      const result = await saveImageToFile('https://example.com/sample.png', 'sample.png', deps);

      expect(invokeMock).toHaveBeenCalledTimes(1);
      expect(toastSuccessMock).not.toHaveBeenCalled();
      expect(toastErrorMock).not.toHaveBeenCalled();
      expect(result).toBeNull();
    });

    it('shows error toast and rethrows when invoke fails', async () => {
      const invokeMock = mock((_cmd: string) => Promise.reject(new Error('Disk full')));
      const toastSuccessMock = mock((_msg: string) => {});
      const toastErrorMock = mock((_msg: string) => {});

      const deps: ImageExportDeps = {
        isTauri: () => true,
        invokeCommand: invokeMock as unknown as ImageExportDeps['invokeCommand'],
        showToastSuccess: toastSuccessMock,
        showToastError: toastErrorMock
      };

      expect(saveImageToFile('https://example.com/sample.png', 'sample.png', deps)).rejects.toThrow('Disk full');
    });
  });

  describe('saveImageToFile in Browser environment fallback', () => {
    it('downloads data URL via anchor tag', async () => {
      const toastSuccessMock = mock((_msg: string) => {});
      const deps: ImageExportDeps = {
        isTauri: () => false,
        showToastSuccess: toastSuccessMock
      };

      const dataUrl = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==';
      const result = await saveImageToFile(dataUrl, 'pixel.png', deps);

      expect(result).toBe('pixel.png');
      expect(toastSuccessMock).toHaveBeenCalledTimes(1);
    });

    it('fetches remote URL as blob and creates object URL', async () => {
      const toastSuccessMock = mock((_msg: string) => {});
      const mockBlob = new Blob(['fake image'], { type: 'image/png' });
      const fetchMock = mock((_input: RequestInfo | URL) =>
        Promise.resolve({
          ok: true,
          status: 200,
          blob: () => Promise.resolve(mockBlob)
        } as unknown as Response)
      );
      const createObjectUrlMock = mock((_blob: Blob) => 'blob:http://localhost/1234');
      const revokeObjectUrlMock = mock((_url: string) => {});

      const deps: ImageExportDeps = {
        isTauri: () => false,
        fetchFn: fetchMock as unknown as ImageExportDeps['fetchFn'],
        createObjectUrl: createObjectUrlMock,
        revokeObjectUrl: revokeObjectUrlMock,
        showToastSuccess: toastSuccessMock
      };

      const result = await saveImageToFile('https://cdn.example.com/remote.png', 'remote.png', deps);

      expect(fetchMock).toHaveBeenCalledTimes(1);
      expect(createObjectUrlMock).toHaveBeenCalledTimes(1);
      expect(toastSuccessMock).toHaveBeenCalledTimes(1);
      expect(result).toBe('remote.png');
    });
  });
});
