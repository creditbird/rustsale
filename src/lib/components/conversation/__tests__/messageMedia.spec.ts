import { describe, it, expect } from 'bun:test';
import {
  attachmentLabel,
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
    expect(attachmentLabel('unknown')).toBe('Tệp đính kèm');
  });

  describe('formatMediaDuration', () => {
    it('formats seconds into mm:ss', () => {
      expect(formatMediaDuration(0)).toBe('00:00');
      expect(formatMediaDuration(65)).toBe('01:05');
      expect(formatMediaDuration(3600)).toBe('60:00');
    });
  });
});
