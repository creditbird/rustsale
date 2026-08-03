import { describe, expect, it } from 'bun:test';
import {
  utf16Length,
  sliceUtf16,
  findMentionToken,
  applyTextEditToMentions,
  insertMention,
  toMentionInputs,
  type ComposerMentionRange
} from '../mentionState';

describe('mentionState', () => {
  describe('utf16Length & sliceUtf16', () => {
    it('calculates UTF-16 length correctly for standard text and unicode/emojis', () => {
      expect(utf16Length('Hello')).toBe(5);
      expect(utf16Length('Xin chào 😀')).toBe(11); // 😀 is 2 UTF-16 code units
    });

    it('slices text based on UTF-16 offsets', () => {
      const str = 'Hello 😀 World';
      expect(sliceUtf16(str, 0, 5)).toBe('Hello');
      expect(sliceUtf16(str, 6, 2)).toBe('😀');
      expect(sliceUtf16(str, 9, 5)).toBe('World');
    });
  });

  describe('findMentionToken', () => {
    it('returns null when no @ is present', () => {
      expect(findMentionToken('Hello world', 5)).toBeNull();
    });

    it('detects @ at index 0', () => {
      const token = findMentionToken('@john', 5);
      expect(token).toEqual({
        start: 0,
        end: 5,
        query: 'john'
      });
    });

    it('detects @ preceded by whitespace', () => {
      const token = findMentionToken('Hello @alice', 12);
      expect(token).toEqual({
        start: 6,
        end: 12,
        query: 'alice'
      });
    });

    it('ignores @ embedded in email addresses or words without leading space', () => {
      expect(findMentionToken('user@domain.com', 11)).toBeNull();
      expect(findMentionToken('abc@def', 7)).toBeNull();
    });

    it('returns null if query contains whitespace or newlines', () => {
      expect(findMentionToken('@john doe', 9)).toBeNull();
      expect(findMentionToken('@john\n', 6)).toBeNull();
    });
  });

  describe('applyTextEditToMentions', () => {
    const initialMentions: ComposerMentionRange[] = [
      { identityId: 101, displayText: '@John', startUtf16: 6, lengthUtf16: 5 },
      { identityId: 102, displayText: '@Alice', startUtf16: 17, lengthUtf16: 6 }
    ];

    it('shifts mention start when insertion happens before mention', () => {
      // Insert 5 chars at index 0: "ADDED "
      const updated = applyTextEditToMentions(
        'Hello @John test @Alice',
        'ADDED Hello @John test @Alice',
        { start: 0, deleteCount: 0, insertedText: 'ADDED ' },
        initialMentions
      );
      expect(updated).toEqual([
        { identityId: 101, displayText: '@John', startUtf16: 12, lengthUtf16: 5 },
        { identityId: 102, displayText: '@Alice', startUtf16: 23, lengthUtf16: 6 }
      ]);
    });

    it('removes mention if edit overlaps mention range', () => {
      // Delete 3 chars inside @John at index 7
      const updated = applyTextEditToMentions(
        'Hello @John test @Alice',
        'Hello @ test @Alice',
        { start: 7, deleteCount: 3, insertedText: '' },
        initialMentions
      );
      expect(updated).toEqual([
        { identityId: 102, displayText: '@Alice', startUtf16: 14, lengthUtf16: 6 }
      ]);
    });

    it('leaves mentions unchanged if edit is after them', () => {
      const updated = applyTextEditToMentions(
        'Hello @John test @Alice',
        'Hello @John test @Alice!!!',
        { start: 23, deleteCount: 0, insertedText: '!!!' },
        initialMentions
      );
      expect(updated).toEqual(initialMentions);
    });
  });

  describe('insertMention', () => {
    it('replaces @token with @displayName and space, adding mention range', () => {
      const value = 'Xin chào @jo!';
      const token = { start: 9, end: 12, query: 'jo' };
      const candidate = { identityId: 201, displayName: 'John Doe' };

      const result = insertMention(value, token, candidate, []);

      expect(result.value).toBe('Xin chào @John Doe !');
      expect(result.caret).toBe(19);
      expect(result.mentions).toEqual([
        { identityId: 201, displayText: '@John Doe', startUtf16: 9, lengthUtf16: 9 }
      ]);
    });

    it('shifts existing mentions after inserted mention', () => {
      const existingMentions: ComposerMentionRange[] = [
        { identityId: 301, displayText: '@Bob', startUtf16: 20, lengthUtf16: 4 }
      ];
      const value = 'Xin chào @a va @Bob';
      const token = { start: 9, end: 11, query: 'a' };
      const candidate = { identityId: 302, displayName: 'Alice' };

      const result = insertMention(value, token, candidate, existingMentions);

      expect(result.value).toBe('Xin chào @Alice  va @Bob');
      expect(result.mentions).toHaveLength(2);
      expect(result.mentions[0]).toEqual({
        identityId: 302,
        displayText: '@Alice',
        startUtf16: 9,
        lengthUtf16: 6
      });
      // @Bob started at 20, delta is inserted "@Alice " (7 chars) minus "@a" (2 chars) = +5 shift => 25
      expect(result.mentions[1].startUtf16).toBe(25);
    });
  });

  describe('toMentionInputs', () => {
    it('converts ComposerMentionRange list to ConversationMentionInput list sorted by startUtf16', () => {
      const mentions: ComposerMentionRange[] = [
        { identityId: 2, displayText: '@Bob', startUtf16: 15, lengthUtf16: 4 },
        { identityId: 1, displayText: '@Alice', startUtf16: 0, lengthUtf16: 6 }
      ];
      const inputs = toMentionInputs(mentions);
      expect(inputs).toEqual([
        { identityId: 1, displayText: '@Alice', startUtf16: 0, lengthUtf16: 6 },
        { identityId: 2, displayText: '@Bob', startUtf16: 15, lengthUtf16: 4 }
      ]);
    });
  });
});
