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

export function onDragDropEvent(onDrop: (paths: string[]) => void): () => void {
  const webview = getCurrentWebview();
  let unlisten: (() => void) | null = null;

  webview.onDragDropEvent((event) => {
    if (event.payload.type === 'drop') {
      onDrop(event.payload.paths);
    }
  }).then((fn) => {
    unlisten = fn;
  }).catch((err) => {
    console.error('Failed to attach drag-drop listener:', err);
  });

  return () => {
    if (unlisten) {
      unlisten();
    }
  };
}

export const MAX_CLIPBOARD_IMAGE_BYTES = 10 * 1024 * 1024; // 10 MiB

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
