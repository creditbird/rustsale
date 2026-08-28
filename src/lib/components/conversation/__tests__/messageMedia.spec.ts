import { describe, it, expect } from 'bun:test';
import {
  attachmentLabel,
  extractGalleryImages,
  extractStickerUrl,
  formatBytes,
  formatMediaDuration,
  isEncryptedWhatsAppMediaUrl,
  safeConversationUrl,
  safeImagePreviewDataUrl
} from '../messageMedia';

describe('messageMedia', () => {
  describe('isEncryptedWhatsAppMediaUrl', () => {
    it('detects WhatsApp encrypted CDN URLs and path patterns', () => {
      expect(isEncryptedWhatsAppMediaUrl('https://mmg.whatsapp.net/o1/v/t24/f2/enc.bin')).toBe(true);
      expect(isEncryptedWhatsAppMediaUrl('/o1/v/t24/f2/AQOmJA8CDN...')).toBe(true);
      expect(isEncryptedWhatsAppMediaUrl('/v/t62/f2/encrypted.enc')).toBe(true);
      expect(isEncryptedWhatsAppMediaUrl('/mms/image/encrypted')).toBe(true);
      expect(isEncryptedWhatsAppMediaUrl('https://cdn.example.com/file.enc')).toBe(true);
      expect(isEncryptedWhatsAppMediaUrl('https://cdn.example.com/file.enc?token=123')).toBe(true);
    });

    it('returns false for standard unencrypted images and data URLs', () => {
      expect(isEncryptedWhatsAppMediaUrl('https://cdn.example.com/image.jpg')).toBe(false);
      expect(isEncryptedWhatsAppMediaUrl('data:image/png;base64,iVBORw0KGgo=')).toBe(false);
      expect(isEncryptedWhatsAppMediaUrl('asset://localhost/path/to/img.png')).toBe(false);
      expect(isEncryptedWhatsAppMediaUrl(null)).toBe(false);
      expect(isEncryptedWhatsAppMediaUrl(undefined)).toBe(false);
    });
  });

  describe('safeConversationUrl', () => {
    it('accepts valid HTTP/HTTPS URLs', () => {
      expect(safeConversationUrl('https://cdn.example.com/image.jpg')).toBe(
        'https://cdn.example.com/image.jpg'
      );
      expect(safeConversationUrl('http://cdn.example.com/file.pdf')).toBe(
        'http://cdn.example.com/file.pdf'
      );
    });

    it('rejects encrypted WhatsApp media URLs', () => {
      expect(safeConversationUrl('https://mmg.whatsapp.net/o1/v/t24/enc.bin')).toBeNull();
      expect(safeConversationUrl('/o1/v/t24/f2/enc')).toBeNull();
      expect(safeConversationUrl('https://cdn.example.com/file.enc')).toBeNull();
    });

    it('accepts valid raster data image and asset URLs', () => {
      expect(safeConversationUrl('data:image/png;base64,iVBORw0KGgo=')).toBe(
        'data:image/png;base64,iVBORw0KGgo='
      );
      expect(safeConversationUrl('asset://localhost/path/to/img.png')).toBe(
        'asset://localhost/path/to/img.png'
      );
    });

    it('rejects unsafe URL protocols', () => {
      expect(safeConversationUrl('javascript:alert(1)')).toBeNull();
      expect(safeConversationUrl('file:///etc/passwd')).toBeNull();
      expect(safeConversationUrl('../secret.txt')).toBeNull();
      expect(safeConversationUrl('data:text/html;base64,PHNjcmlwdD4=')).toBeNull();
      expect(safeConversationUrl('data:image/svg+xml;base64,PHN2Zz4=')).toBeNull();
    });
  });

  describe('safeImagePreviewDataUrl', () => {
    it('accepts only bounded raster image data URLs', () => {
      expect(safeImagePreviewDataUrl('data:image/png;base64,iVBORw0KGgo=')).toBe(
        'data:image/png;base64,iVBORw0KGgo='
      );
      expect(safeImagePreviewDataUrl('data:image/jpeg;base64,/9j/4AAQ')).toBe(
        'data:image/jpeg;base64,/9j/4AAQ'
      );
      expect(safeImagePreviewDataUrl('data:image/svg+xml;base64,PHN2Zz4=')).toBeNull();
      expect(safeImagePreviewDataUrl('data:text/html;base64,PHNjcmlwdD4=')).toBeNull();
      expect(safeImagePreviewDataUrl('file:///tmp/preview.png')).toBeNull();
    });
  });

  it('formats bytes and attachment labels', () => {
    expect(formatBytes(1024)).toBe('1.0 KB');
    expect(formatBytes(5 * 1024 * 1024)).toBe('5.0 MB');
    expect(attachmentLabel('video')).toBe('Video');
    expect(attachmentLabel('unknown')).toBe('Attachment');
  });

  describe('extractStickerUrl', () => {
    it('extracts URL from attachment remoteUrl or thumbnailUrl', () => {
      expect(extractStickerUrl({ remoteUrl: 'https://cdn.example.com/sticker.png' })).toBe(
        'https://cdn.example.com/sticker.png'
      );
      expect(extractStickerUrl({ thumbnailUrl: 'https://cdn.example.com/thumb.png' })).toBe(
        'https://cdn.example.com/thumb.png'
      );
    });

    it('extracts URL from attachment metadata_json when remoteUrl is missing', () => {
      expect(
        extractStickerUrl({
          remoteUrl: null,
          metadata_json: JSON.stringify({ stickerSpriteUrl: 'https://stc-laban.zdn.vn/sprite.png' })
        })
      ).toBe('https://stc-laban.zdn.vn/sprite.png');
    });

    it('extracts URL from various content property variants (stickerSpriteUrl, stickerWebpUrl, etc.)', () => {
      expect(extractStickerUrl(null, { stickerSpriteUrl: 'https://stc-laban.zdn.vn/sprite.png' })).toBe(
        'https://stc-laban.zdn.vn/sprite.png'
      );
      expect(extractStickerUrl(null, { sticker_webp_url: 'https://stc-laban.zdn.vn/sample.webp' })).toBe(
        'https://stc-laban.zdn.vn/sample.webp'
      );
      expect(extractStickerUrl(null, { thumbUrl: 'https://stc-laban.zdn.vn/thumb.png' })).toBe(
        'https://stc-laban.zdn.vn/thumb.png'
      );
    });

    it('extracts URL from nested content object structures', () => {
      expect(
        extractStickerUrl(null, { sticker: { url: 'https://stc-laban.zdn.vn/nested.png' } })
      ).toBe('https://stc-laban.zdn.vn/nested.png');
      expect(
        extractStickerUrl(null, { data: { spriteUrl: 'https://stc-laban.zdn.vn/data.png' } })
      ).toBe('https://stc-laban.zdn.vn/data.png');
    });
  });

  describe('formatMediaDuration', () => {
    it('formats seconds into mm:ss', () => {
      expect(formatMediaDuration(0)).toBe('00:00');
      expect(formatMediaDuration(65)).toBe('01:05');
      expect(formatMediaDuration(3600)).toBe('60:00');
    });
  });

  describe('extractGalleryImages', () => {
    it('extracts standard image URLs and ignores un-decrypted WhatsApp encrypted URLs', () => {
      const messages = [
        {
          attachments: [
            {
              mediaType: 'image',
              remoteUrl: 'https://example.com/photo1.jpg',
              thumbnailUrl: null,
              fileName: 'photo1.jpg'
            },
            {
              mediaType: 'image',
              remoteUrl: 'https://mmg.whatsapp.net/v/encrypted.enc',
              thumbnailUrl: null,
              fileName: 'wa_encrypted.jpg'
            },
            {
              mediaType: 'image',
              remoteUrl: 'https://mmg.whatsapp.net/v/encrypted2.enc',
              thumbnailUrl: 'data:image/jpeg;base64,/9j/4AAQSkZJRg==',
              fileName: 'wa_decrypted.jpg'
            },
            {
              mediaType: 'image',
              remoteUrl: 'https://mmg.whatsapp.net/v/encrypted3.enc',
              thumbnailUrl: 'https://mmg.whatsapp.net/o1/v/t24/encrypted_thumb',
              fileName: 'wa_encrypted_thumb.jpg'
            }
          ],
          occurredAt: '2026-08-27T10:00:00Z'
        }
      ];

      const result = extractGalleryImages(messages);
      expect(result.length).toBe(2);
      expect(result[0].url).toBe('https://example.com/photo1.jpg');
      expect(result[1].url).toBe('data:image/jpeg;base64,/9j/4AAQSkZJRg==');
    });
  });
});
