import { describe, it, expect } from 'bun:test';
import {
  attachmentLabel,
  extractStickerUrl,
  formatBytes,
  formatMediaDuration,
  safeConversationUrl,
  safeImagePreviewDataUrl
} from '../messageMedia';

describe('messageMedia', () => {
  describe('safeConversationUrl', () => {
    it('accepts valid HTTP/HTTPS URLs', () => {
      expect(safeConversationUrl('https://cdn.example.com/image.jpg')).toBe(
        'https://cdn.example.com/image.jpg'
      );
      expect(safeConversationUrl('http://cdn.example.com/file.pdf')).toBe(
        'http://cdn.example.com/file.pdf'
      );
    });

    it('rejects unsafe URL protocols', () => {
      expect(safeConversationUrl('javascript:alert(1)')).toBeNull();
      expect(safeConversationUrl('file:///etc/passwd')).toBeNull();
      expect(safeConversationUrl('../secret.txt')).toBeNull();
      expect(safeConversationUrl('data:text/html;base64,PHNjcmlwdD4=')).toBeNull();
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
});
