import { describe, expect, it } from 'bun:test';
import type { ConversationQuickTemplate } from '$lib/services/conversationService';
import {
  findSlashToken,
  rankQuickTemplates,
  replaceSlashToken
} from '../quickTemplateState';

describe('quickTemplateState', () => {
  describe('findSlashToken', () => {
    it('detects slash token at end of typed text', () => {
      expect(findSlashToken('Xin /gia', 8)).toEqual({ start: 4, end: 8, query: 'gia' });
    });

    it('returns null for URL patterns', () => {
      expect(findSlashToken('https://a/b', 11)).toBeNull();
    });

    it('detects slash token at index 0', () => {
      expect(findSlashToken('/tu-van', 7)).toEqual({ start: 0, end: 7, query: 'tu-van' });
    });

    it('detects slash token after newline', () => {
      expect(findSlashToken('\n/chao', 6)).toEqual({ start: 1, end: 6, query: 'chao' });
    });

    it('detects slash token with trailing text when caret is right after token query', () => {
      expect(findSlashToken('Xin /gia nhé', 8)).toEqual({ start: 4, end: 8, query: 'gia' });
    });

    it('returns null if caret is after whitespace following the slash token', () => {
      expect(findSlashToken('Xin /gia nhé', 10)).toBeNull();
    });

    it('returns null if slash is preceded by non-whitespace character', () => {
      expect(findSlashToken('abc/def', 7)).toBeNull();
    });

    it('detects empty query right after slash', () => {
      expect(findSlashToken('Xin /', 5)).toEqual({ start: 4, end: 5, query: '' });
    });
  });

  describe('replaceSlashToken', () => {
    it('replaces slash token with body and calculates new caret position', () => {
      expect(
        replaceSlashToken(
          'Xin /gia nhé',
          { start: 4, end: 8, query: 'gia' },
          'Bảng giá'
        )
      ).toEqual({ value: 'Xin Bảng giá nhé', caret: 12 });
    });

    it('replaces slash token at start of value', () => {
      expect(
        replaceSlashToken(
          '/tu-van hello',
          { start: 0, end: 7, query: 'tu-van' },
          'Tư vấn 24/7'
        )
      ).toEqual({ value: 'Tư vấn 24/7 hello', caret: 11 });
    });

    it('replaces slash token at end of value', () => {
      expect(
        replaceSlashToken(
          'Chào /chao',
          { start: 5, end: 10, query: 'chao' },
          'bạn!'
        )
      ).toEqual({ value: 'Chào bạn!', caret: 9 });
    });
  });

  describe('rankQuickTemplates', () => {
    const mockTemplates: ConversationQuickTemplate[] = [
      {
        id: 1,
        shortcut: 'gia',
        title: 'Bảng giá sản phẩm',
        bodyText: 'Dưới đây là bảng giá mới nhất...',
        category: 'Bán hàng',
        isActive: true,
        usageCount: 10,
        lastUsedAt: '2026-08-01T00:00:00Z',
        createdAt: '2026-07-01T00:00:00Z',
        updatedAt: '2026-08-01T00:00:00Z'
      },
      {
        id: 2,
        shortcut: 'giagiam',
        title: 'Bảng giá giảm giá',
        bodyText: 'Chương trình giảm giá tháng 8...',
        category: null,
        isActive: true,
        usageCount: 25,
        lastUsedAt: null,
        createdAt: '2026-07-01T00:00:00Z',
        updatedAt: '2026-07-01T00:00:00Z'
      },
      {
        id: 3,
        shortcut: 'chao',
        title: 'Lời chào khách hàng gia',
        bodyText: 'Xin chào anh chị!',
        category: 'CSKH',
        isActive: true,
        usageCount: 5,
        lastUsedAt: null,
        createdAt: '2026-07-01T00:00:00Z',
        updatedAt: '2026-07-01T00:00:00Z'
      },
      {
        id: 4,
        shortcut: 'tam-byebye',
        title: 'Tạm biệt',
        bodyText: 'Cảm ơn anh gia đã nhắn!',
        category: null,
        isActive: false, // Inactive! Should be filtered out
        usageCount: 100,
        lastUsedAt: null,
        createdAt: '2026-07-01T00:00:00Z',
        updatedAt: '2026-07-01T00:00:00Z'
      }
    ];

    it('filters out inactive templates', () => {
      const results = rankQuickTemplates(mockTemplates, '');
      expect(results.some((t) => t.id === 4)).toBe(false);
      expect(results.length).toBe(3);
    });

    it('ranks exact shortcut match first, followed by prefix match, then content match', () => {
      const results = rankQuickTemplates(mockTemplates, 'gia');
      // ID 1: exact shortcut match ("gia")
      // ID 2: prefix shortcut match ("giagiam")
      // ID 3: title contains "gia"
      expect(results[0].id).toBe(1);
      expect(results[1].id).toBe(2);
      expect(results[2].id).toBe(3);
    });

    it('sorts empty query by usageCount descending', () => {
      const results = rankQuickTemplates(mockTemplates, '');
      expect(results[0].id).toBe(2); // usageCount = 25
      expect(results[1].id).toBe(1); // usageCount = 10
      expect(results[2].id).toBe(3); // usageCount = 5
    });
  });
});
