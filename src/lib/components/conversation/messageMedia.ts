export function safeConversationUrl(url: string | null | undefined): string | null {
  if (!url) return null;
  const trimmed = url.trim();
  if (
    trimmed.startsWith('https://') ||
    trimmed.startsWith('http://')
  ) {
    return trimmed;
  }
  return null;
}

const MAX_IMAGE_PREVIEW_BYTES = 10 * 1024 * 1024;
const RASTER_IMAGE_DATA_URL = /^data:image\/(?:png|jpe?g|webp|gif);base64,([a-z0-9+/]+={0,2})$/i;

export function safeImagePreviewDataUrl(url: string | null | undefined): string | null {
  if (!url) return null;
  const trimmed = url.trim();
  const match = RASTER_IMAGE_DATA_URL.exec(trimmed);
  if (!match) return null;

  const payload = match[1];
  const padding = payload.endsWith('==') ? 2 : payload.endsWith('=') ? 1 : 0;
  const decodedBytes = Math.floor((payload.length * 3) / 4) - padding;
  return decodedBytes <= MAX_IMAGE_PREVIEW_BYTES ? trimmed : null;
}

export function formatBytes(bytes: number | null | undefined): string {
  if (!bytes || bytes <= 0) return '0 B';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

export function attachmentLabel(mediaType: string | null | undefined): string {
  switch (mediaType) {
    case 'image':
      return 'Hình ảnh';
    case 'video':
      return 'Video';
    case 'audio':
    case 'voice':
      return 'Âm thanh';
    case 'sticker':
      return 'Sticker';
    case 'file':
      return 'Tệp';
    default:
      return 'Tệp đính kèm';
  }
}

export function formatMediaDuration(seconds: number | null | undefined): string {
  if (!seconds || seconds <= 0) return '00:00';
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  const mm = String(mins).padStart(2, '0');
  const ss = String(secs).padStart(2, '0');
  return `${mm}:${ss}`;
}
