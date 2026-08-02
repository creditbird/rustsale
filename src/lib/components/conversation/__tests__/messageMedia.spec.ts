import { describe, it, expect } from 'bun:test';
import { safeConversationUrl, formatMediaDuration } from '../messageMedia';

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

  describe('formatMediaDuration', () => {
    it('formats seconds into mm:ss', () => {
      expect(formatMediaDuration(0)).toBe('00:00');
      expect(formatMediaDuration(65)).toBe('01:05');
      expect(formatMediaDuration(3600)).toBe('60:00');
    });
  });
});
