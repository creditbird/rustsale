export function isEncryptedWhatsAppMediaUrl(value: string | null | undefined): boolean {
  if (!value) return false;
  const normalized = value.trim().toLowerCase();
  return (
    normalized.includes('mmg.whatsapp.net') ||
    normalized.startsWith('/o1/') ||
    normalized.startsWith('/v/t62') ||
    normalized.startsWith('/mms/') ||
    normalized.endsWith('.enc') ||
    normalized.includes('.enc?')
  );
}

export function safeConversationUrl(url: string | null | undefined): string | null {
  if (!url) return null;
  const trimmed = url.trim();
  if (isEncryptedWhatsAppMediaUrl(trimmed)) {
    return null;
  }
  if (
    trimmed.startsWith('https://') ||
    trimmed.startsWith('http://') ||
    trimmed.startsWith('asset://')
  ) {
    return trimmed;
  }
  if (trimmed.startsWith('data:image/')) {
    return safeImagePreviewDataUrl(trimmed);
  }
  return null;
}

export async function openExternalUrl(url: string | null | undefined): Promise<void> {
  if (!url) return;
  const safe = safeConversationUrl(url);
  if (!safe) return;
  if (!safe.startsWith('http://') && !safe.startsWith('https://')) return;
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
  if (url.startsWith('data:image/')) return true;
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

export function safeImagePreviewDataUrl(url: string | null | undefined): string | null {
  if (!url) return null;
  const trimmed = url.trim();
  if (!trimmed.startsWith('data:image/')) return null;
  const commaIdx = trimmed.indexOf(',');
  if (commaIdx === -1) return null;
  const header = trimmed.slice(0, commaIdx).toLowerCase();
  if (!header.endsWith(';base64')) return null;
  if (header.includes('svg') || header.includes('html') || header.includes('xml')) return null;

  const payloadLength = trimmed.length - commaIdx - 1;
  const approxBytes = Math.floor((payloadLength * 3) / 4);
  if (approxBytes > MAX_IMAGE_PREVIEW_BYTES || approxBytes === 0) return null;

  return trimmed;
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
      return 'Image';
    case 'video':
      return 'Video';
    case 'audio':
    case 'voice':
      return 'Audio';
    case 'sticker':
      return 'Sticker';
    case 'file':
      return 'File';
    default:
      return 'Attachment';
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
  attachments?: Array<{
    mediaType: string;
    remoteUrl?: string | null;
    thumbnailUrl?: string | null;
    fileName?: string | null;
  }>;
  contentType?: string;
  content?: Record<string, unknown> | null;
  textContent?: string | null;
  occurredAt?: string;
}>): ExtractedGalleryImage[] {
  const images: ExtractedGalleryImage[] = [];
  const seenUrls = new Set<string>();

  for (const msg of messages) {
    if (msg.attachments && msg.attachments.length > 0) {
      for (const att of msg.attachments) {
        if (att.mediaType === 'image') {
          const safeRemote = !isEncryptedWhatsAppMediaUrl(att.remoteUrl) ? safeConversationUrl(att.remoteUrl) : null;
          const safeThumb = !isEncryptedWhatsAppMediaUrl(att.thumbnailUrl) ? safeConversationUrl(att.thumbnailUrl) : null;
          const effectiveUrl = safeThumb || safeRemote;
          if (effectiveUrl && !seenUrls.has(effectiveUrl)) {
            seenUrls.add(effectiveUrl);
            images.push({
              url: effectiveUrl,
              alt: att.fileName ?? 'Image',
              caption: msg.textContent || att.fileName || 'Image',
              timestamp: msg.occurredAt
            });
          }
        }
      }
    }

    if (msg.contentType === 'image' || (msg.contentType && msg.contentType.startsWith('image/'))) {
      const legacyUrl = extractLegacyImageUrl(msg.content);
      if (legacyUrl && !isEncryptedWhatsAppMediaUrl(legacyUrl) && !seenUrls.has(legacyUrl)) {
        seenUrls.add(legacyUrl);
        images.push({
          url: legacyUrl,
          alt: msg.textContent || 'Image',
          caption: msg.textContent || 'Image',
          timestamp: msg.occurredAt
        });
      }
    }
  }

  return images;
}
