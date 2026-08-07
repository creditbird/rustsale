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

export function extractLegacyImageUrl(content: Record<string, unknown> | null | undefined): string | null {
  if (!content) return null;
  const url =
    typeof content.url === 'string'
      ? content.url
      : typeof content.src === 'string'
        ? content.src
        : typeof content.href === 'string'
          ? content.href
          : typeof content.path === 'string'
            ? content.path
            : null;
  return safeConversationUrl(url);
}

export interface ExtractedGalleryImage {
  url: string;
  alt?: string;
  caption?: string;
  timestamp?: string;
}

export function extractGalleryImages(messages: Array<{
  attachments?: Array<{ mediaType: string; remoteUrl?: string | null; fileName?: string | null }>;
  contentType?: string;
  content?: Record<string, unknown> | null;
  textContent?: string | null;
  occurredAt?: string;
}>): ExtractedGalleryImage[] {
  const images: ExtractedGalleryImage[] = [];

  for (const msg of messages) {
    if (msg.attachments && msg.attachments.length > 0) {
      for (const att of msg.attachments) {
        if (att.mediaType === 'image') {
          const safeUrl = safeConversationUrl(att.remoteUrl);
          if (safeUrl) {
            images.push({
              url: safeUrl,
              alt: att.fileName ?? 'Hình ảnh',
              caption: msg.textContent || att.fileName || 'Hình ảnh',
              timestamp: msg.occurredAt
            });
          }
        }
      }
    }

    if (msg.contentType === 'image' || (msg.contentType && msg.contentType.startsWith('image/'))) {
      const legacyUrl = extractLegacyImageUrl(msg.content);
      if (legacyUrl && !images.some((img) => img.url === legacyUrl)) {
        images.push({
          url: legacyUrl,
          alt: msg.textContent || 'Hình ảnh',
          caption: msg.textContent || 'Hình ảnh',
          timestamp: msg.occurredAt
        });
      }
    }
  }

  return images;
}
