import { describe, it, expect } from 'bun:test';
import {
  composerDraftKey,
  ComposerDraftCache,
  insertTextAtSelection,
  type ComposerAttachmentItem,
} from '../composerDraftState';

describe('composerDraftState', () => {
  it('generates predictable key', () => {
    expect(composerDraftKey(31, 4)).toBe('31:4');
  });

  it('manages draft updates per conversation key', () => {
    const cache = new ComposerDraftCache();
    cache.update('31:4', { text: 'Hello' });
    cache.update('32:4', { text: 'World' });

    expect(cache.get('31:4').text).toBe('Hello');
    expect(cache.get('32:4').text).toBe('World');
  });

  it('inserts text at selection start and end', () => {
    const res = insertTextAtSelection('hello world', ' beautiful', 5, 5);
    expect(res).toEqual({
      value: 'hello beautiful world',
      caret: 15,
    });
  });

  it('replaces selection range', () => {
    const res = insertTextAtSelection('hello world', '😀', 0, 5);
    expect(res).toEqual({
      value: '😀 world',
      caret: 2,
    });
  });

  it('takes snapshot and clears sent content without losing new typed text', () => {
    const cache = new ComposerDraftCache();
    const item: ComposerAttachmentItem = {
      localId: 'loc-1',
      stagedAttachmentId: 'staged-1',
      fileName: 'test.jpg',
      mimeType: 'image/jpeg',
      sizeBytes: 100,
      attachmentType: 'image',
      previewDataUrl: null,
      status: 'ready',
      errorMessage: null,
    };
    cache.update('31:4', { text: 'First line', attachments: [item] });

    const snapshot = cache.takeSnapshot('31:4');
    expect(snapshot).toEqual({
      text: 'First line',
      stagedAttachmentIds: ['staged-1'],
      mentions: []
    });

    // User types more text after send starts
    cache.update('31:4', { text: 'First line and second line' });

    cache.clearSnapshot('31:4', snapshot);
    expect(cache.get('31:4').text).toBe('First line and second line');
    expect(cache.get('31:4').attachments).toEqual([]);
  });

  it('preserves mentions in draft state and snapshot', () => {
    const cache = new ComposerDraftCache();
    const mention = { identityId: 1, displayText: '@Alice', startUtf16: 0, lengthUtf16: 6 };
    cache.update('10:1', { text: '@Alice hi', mentions: [mention] });

    expect(cache.get('10:1').mentions).toEqual([mention]);
    const snapshot = cache.takeSnapshot('10:1');
    expect(snapshot.mentions).toEqual([mention]);

    cache.clearSnapshot('10:1', snapshot);
    expect(cache.get('10:1').text).toBe('');
    expect(cache.get('10:1').mentions).toEqual([]);
  });
});
