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

export async function openExternalUrl(url: string | null | undefined): Promise<void> {
  if (!url) return;
  const safe = safeConversationUrl(url);
  if (!safe) return;
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(safe);
  } catch {
    if (typeof window !== 'undefined') {
      window.open(safe, '_blank', 'noopener,noreferrer');
    }
  }
}

export function isImageUrl(url: string | null | undefined): boolean {
  if (!url) return false;
  const clean = url.split('?')[0].split('#')[0].toLowerCase();
  if (/\.(png|jpe?g|gif|webp|svg|bmp)$/i.test(clean)) return true;
  if (clean.includes('zdn.vn/gr/jpg/') || clean.includes('/photo-stal-') || clean.includes('/photo-')) return true;
  return false;
}

export function isVideoUrl(url: string | null | undefined): boolean {
  if (!url) return false;
  const clean = url.split('?')[0].split('#')[0].toLowerCase();
  if (/\.(mp4|webm|ogg|mov|m4v|mkv|avi|3gp|flv)$/i.test(clean)) return true;
  if (
    clean.includes('/video-stal-') ||
    clean.includes('/video-') ||
    clean.includes('video.zdn.vn') ||
    clean.includes('zalo-video')
  ) {
    return true;
  }
  return false;
}

export function extractStickerUrl(
  attachment?: { remoteUrl?: string | null; thumbnailUrl?: string | null; metadata_json?: string | null } | null,
  content?: Record<string, unknown> | null
): string | null {
  const fromAtt = safeConversationUrl(attachment?.remoteUrl) ?? safeConversationUrl(attachment?.thumbnailUrl);
  if (fromAtt) return fromAtt;

  if (attachment && 'metadata_json' in attachment && typeof (attachment as Record<string, unknown>).metadata_json === 'string') {
    try {
      const parsed = JSON.parse((attachment as Record<string, unknown>).metadata_json as string);
      const urlFromMeta = extractStickerUrlFromObject(parsed);
      if (urlFromMeta) return urlFromMeta;
    } catch {
      // Ignore invalid metadata_json
    }
  }

  if (!content) return null;
  return extractStickerUrlFromObject(content);
}

function extractStickerUrlFromObject(obj: Record<string, unknown> | null | undefined, depth = 0): string | null {
  if (!obj || typeof obj !== 'object' || depth > 5) return null;

  const candidateKeys = [
    'url',
    'spriteUrl',
    'sprite_url',
    'stickerSpriteUrl',
    'sticker_sprite_url',
    'staticUrl',
    'static_url',
    'webpUrl',
    'webp_url',
    'stickerWebpUrl',
    'sticker_webp_url',
    'webUrl',
    'web_url',
    'stickerUrl',
    'sticker_url',
    'thumbUrl',
    'thumb_url',
    'thumbnailUrl',
    'thumbnail_url',
    'hdUrl',
    'hd_url',
    'previewUrl',
    'preview_url',
    'fullUrl',
    'full_url',
    'mediaUrl',
    'media_url',
    'imageUrl',
    'image_url',
    'src',
    'href',
    'path'
  ];

  for (const key of candidateKeys) {
    if (typeof obj[key] === 'string') {
      const safe = safeConversationUrl(obj[key] as string);
      if (safe) return safe;
    }
  }

  const subObjectKeys = ['sticker', 'data', 'payload', 'params', 'media', 'attachment', 'content'];
  for (const subKey of subObjectKeys) {
    if (obj[subKey] && typeof obj[subKey] === 'object' && !Array.isArray(obj[subKey])) {
      const nestedUrl = extractStickerUrlFromObject(obj[subKey] as Record<string, unknown>, depth + 1);
      if (nestedUrl) return nestedUrl;
    }
  }

  for (const key of Object.keys(obj)) {
    if (candidateKeys.includes(key) || subObjectKeys.includes(key)) continue;
    const val = obj[key];
    if (typeof val === 'string') {
      const safe = safeConversationUrl(val);
      if (safe) return safe;
    } else if (val && typeof val === 'object' && !Array.isArray(val)) {
      const nestedUrl = extractStickerUrlFromObject(val as Record<string, unknown>, depth + 1);
      if (nestedUrl) return nestedUrl;
    }
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
