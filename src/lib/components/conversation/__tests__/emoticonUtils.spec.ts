import { describe, expect, it } from 'bun:test';
import { parseEmoticons } from '../emoticonUtils';

describe('emoticonUtils', () => {
  it('converts standalone emoticons into emojis', () => {
    expect(parseEmoticons(":')")).toBe('🥲');
    expect(parseEmoticons(':)')).toBe('😊');
    expect(parseEmoticons('<3')).toBe('❤️');
  });

  it('converts multiple space-separated emoticons', () => {
    expect(parseEmoticons(":') :) :)")).toBe('🥲 😊 😊');
  });

  it('converts emoticons embedded in text sentence', () => {
    expect(parseEmoticons("Hok lẽ lôi gemini ra xài luôn trời :')")).toBe(
      'Hok lẽ lôi gemini ra xài luôn trời 🥲'
    );
    expect(parseEmoticons('Xin chào :) rất vui được gặp <3')).toBe(
      'Xin chào 😊 rất vui được gặp ❤️'
    );
  });

  it('does not replace emoticons inside URLs or words without boundaries', () => {
    expect(parseEmoticons('http://example.com/page:)')).toBe(
      'http://example.com/page:)'
    );
  });

  it('returns empty or whitespace string untouched', () => {
    expect(parseEmoticons('')).toBe('');
    expect(parseEmoticons('   ')).toBe('   ');
  });
});
