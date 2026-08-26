import { open } from '@tauri-apps/plugin-dialog';
import { getCurrentWebview } from '@tauri-apps/api/webview';

export interface FileFilter {
  name: string;
  extensions: string[];
}

export interface OpenFilePickerOptions {
  multiple?: boolean;
  filters?: FileFilter[];
}

export async function openFilePicker(options: OpenFilePickerOptions = {}): Promise<string[]> {
  const result = await open({
    multiple: options.multiple ?? true,
    filters: options.filters,
  });

  if (!result) {
    return [];
  }
  if (Array.isArray(result)) {
    return result;
  }
  return [result];
}

export function onDragDropEvent(
  onDrop: (paths: string[]) => void,
  onActiveChange: (active: boolean) => void = () => {}
): () => void {
  let unlisten: (() => void) | null = null;
  let disposed = false;

  try {
    const webview = getCurrentWebview();
    webview.onDragDropEvent((event) => {
      if (disposed) return;
      if (event.payload.type === 'enter' || event.payload.type === 'over') {
        onActiveChange(true);
      } else if (event.payload.type === 'drop') {
        onActiveChange(false);
        onDrop(event.payload.paths);
      } else if (event.payload.type === 'leave') {
        onActiveChange(false);
      }
    }).then((fn) => {
      if (disposed) {
        fn();
      } else {
        unlisten = fn;
      }
    }).catch(() => {
      onActiveChange(false);
    });
  } catch {
    onActiveChange(false);
  }

  return () => {
    disposed = true;
    onActiveChange(false);
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
  };
}

export const MAX_CLIPBOARD_IMAGE_BYTES = 10 * 1024 * 1024;

export async function readClipboardImage(): Promise<{ fileName: string; mimeType: string; base64Data: string } | null> {
  if (typeof navigator === 'undefined' || !navigator.clipboard) {
    return null;
  }

  const items = await navigator.clipboard.read().catch(() => []);
  for (const item of items) {
    const imageType = item.types.find((t) => t.startsWith('image/'));
    if (imageType) {
      const blob = await item.getType(imageType);
      if (blob.size > MAX_CLIPBOARD_IMAGE_BYTES) {
        throw new Error(`Clipboard image size (${blob.size} bytes) exceeds limit of ${MAX_CLIPBOARD_IMAGE_BYTES} bytes`);
      }
      const arrayBuffer = await blob.arrayBuffer();
      const bytes = new Uint8Array(arrayBuffer);
      let binary = '';
      for (let i = 0; i < bytes.length; i++) {
        binary += String.fromCharCode(bytes[i]);
      }
      const base64Data = btoa(binary);
      const ext = imageType.split('/')[1] || 'png';
      const fileName = `clipboard_${Date.now()}.${ext}`;
      return {
        fileName,
        mimeType: imageType,
        base64Data,
      };
    }
  }

  return null;
}
