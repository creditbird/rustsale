import { invoke, isTauri } from '@tauri-apps/api/core';
import { toast } from 'svelte-sonner';

export interface ImageExportDeps {
  isTauri?: () => boolean;
  invokeCommand?: <T = unknown>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
  fetchFn?: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
  createObjectUrl?: (blob: Blob) => string;
  revokeObjectUrl?: (url: string) => void;
  showToastSuccess?: (msg: string) => void;
  showToastError?: (msg: string) => void;
}

export function isTauriEnvironment(): boolean {
  return (
    typeof window !== 'undefined' &&
    ('__TAURI__' in window ||
      '__TAURI_INTERNALS__' in window ||
      (typeof isTauri === 'function' && isTauri()))
  );
}

export const VALID_IMAGE_EXT_REGEX = /\.(png|jpe?g|webp|gif|bmp|svg|ico|avif|tiff?)$/i;

export function sanitizeImageFilename(name?: string | null, fallbackUrl?: string | null): string {
  const now = new Date();
  const yyyy = now.getFullYear();
  const mm = String(now.getMonth() + 1).padStart(2, '0');
  const dd = String(now.getDate()).padStart(2, '0');
  const hh = String(now.getHours()).padStart(2, '0');
  const min = String(now.getMinutes()).padStart(2, '0');
  const ss = String(now.getSeconds()).padStart(2, '0');
  const defaultFallback = `image_${yyyy}${mm}${dd}_${hh}${min}${ss}.png`;

  let urlExt: string | null = null;
  if (fallbackUrl && fallbackUrl.trim()) {
    try {
      const parsed = new URL(fallbackUrl.trim());
      const segments = parsed.pathname.split('/');
      const last = segments[segments.length - 1];
      if (last) {
        const cleanLast = last.split('?')[0];
        const match = cleanLast.match(VALID_IMAGE_EXT_REGEX);
        if (match) {
          urlExt = match[0].toLowerCase();
        }
      }
    } catch {
      // not a standard URL, continue
    }
  }

  if (name && name.trim()) {
    const clean = name.trim().replace(/[\\/:*?"<>|]/g, '_');
    if (VALID_IMAGE_EXT_REGEX.test(clean)) {
      return clean;
    }
    if (urlExt) {
      return `${clean}${urlExt}`;
    }
    return `${clean}.png`;
  }

  if (fallbackUrl && fallbackUrl.trim()) {
    try {
      const parsed = new URL(fallbackUrl.trim());
      const segments = parsed.pathname.split('/');
      const last = segments[segments.length - 1];
      if (last) {
        const cleanLast = last.split('?')[0];
        if (VALID_IMAGE_EXT_REGEX.test(cleanLast)) {
          return cleanLast.replace(/[\\/:*?"<>|]/g, '_');
        }
      }
    } catch {
      // not a standard URL, continue to fallback
    }
  }

  return defaultFallback;
}

/**
 * Saves an image to the local filesystem via Tauri's native Save Dialog,
 * or downloads via browser blob URL fallback when in a standard web environment.
 *
 * Returns the saved path (in Tauri) or filename (in browser), or null if cancelled.
 */
export async function saveImageToFile(
  imageUrl: string,
  suggestedFilename?: string | null,
  deps?: ImageExportDeps
): Promise<string | null> {
  const isTauriFn = deps?.isTauri ?? isTauriEnvironment;
  const invokeFn = deps?.invokeCommand ?? invoke;
  const toastSuccess = deps?.showToastSuccess ?? toast.success;
  const toastError = deps?.showToastError ?? toast.error;
  const filename = sanitizeImageFilename(suggestedFilename, imageUrl);

  if (isTauriFn()) {
    try {
      const savedPath = await invokeFn<string | null>('save_image_file', {
        defaultFilename: filename,
        url: imageUrl
      });

      if (savedPath) {
        toastSuccess('Đã lưu ảnh thành công');
        return savedPath;
      }
      return null;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      toastError(`Không thể lưu ảnh: ${msg}`);
      throw err;
    }
  }

  // Browser / Web fallback
  if (typeof document !== 'undefined') {
    const fetchCall = deps?.fetchFn ?? fetch;
    const createUrl = deps?.createObjectUrl ?? URL.createObjectURL;
    const revokeUrl = deps?.revokeObjectUrl ?? URL.revokeObjectURL;

    try {
      if (imageUrl.startsWith('data:') || imageUrl.startsWith('blob:')) {
        const link = document.createElement('a');
        link.href = imageUrl;
        link.download = filename;
        link.target = '_blank';
        document.body.appendChild(link);
        link.click();
        document.body.removeChild(link);
        toastSuccess('Đã tải ảnh về');
        return filename;
      }

      const res = await fetchCall(imageUrl);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const blob = await res.blob();
      const objUrl = createUrl(blob);
      const link = document.createElement('a');
      link.href = objUrl;
      link.download = filename;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      setTimeout(() => revokeUrl(objUrl), 1000);
      toastSuccess('Đã tải ảnh về');
      return filename;
    } catch {
      // Fallback to direct anchor navigation if fetch was blocked by CORS
      const link = document.createElement('a');
      link.href = imageUrl;
      link.download = filename;
      link.target = '_blank';
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      toastSuccess('Đang mở/tải ảnh');
      return filename;
    }
  }

  return filename;
}
