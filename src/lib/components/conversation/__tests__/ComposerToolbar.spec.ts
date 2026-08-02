import { afterEach, describe, expect, it, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import type { ConversationCapabilities } from '$lib/services/conversationService';
import ComposerToolbar from '../ComposerToolbar.svelte';

const richCapabilities: ConversationCapabilities = {
  maxTextLength: 2000,
  maxAttachmentsPerBatch: 10,
  maxFileSizeBytes: 50 * 1024 * 1024,
  supportedImageTypes: ['image/png'],
  supportedVideoTypes: ['video/mp4'],
  supportedAudioTypes: ['audio/mpeg'],
  supportedDocTypes: ['application/pdf'],
  supportsStickers: true,
  supportsQuoteReply: true,
  supportsRecall: true,
  recallWindowSeconds: 86400,
  supportsReactions: true
};

describe('ComposerToolbar', () => {
  afterEach(cleanup);

  it('emits rich actions when capabilities allow them', async () => {
    const onToggleEmoji = mock(() => {});
    const onToggleSticker = mock(() => {});
    const onPickAttachment = mock(() => {});
    render(ComposerToolbar, {
      capabilities: richCapabilities,
      onToggleEmoji,
      onToggleSticker,
      onPickAttachment
    });

    await fireEvent.click(screen.getByTestId('toggle-emoji-btn'));
    await fireEvent.click(screen.getByTestId('toggle-sticker-btn'));
    await fireEvent.click(screen.getByTestId('attach-file-btn'));

    expect(onToggleEmoji).toHaveBeenCalledTimes(1);
    expect(onToggleSticker).toHaveBeenCalledTimes(1);
    expect(onPickAttachment).toHaveBeenCalledTimes(1);
  });

  it('hides sticker and attachment actions when capabilities deny them', () => {
    render(ComposerToolbar, {
      capabilities: {
        ...richCapabilities,
        maxAttachmentsPerBatch: 0,
        supportsStickers: false
      }
    });

    expect(screen.getByTestId('toggle-emoji-btn')).toBeTruthy();
    expect(screen.queryByTestId('toggle-sticker-btn')).toBeNull();
    expect(screen.queryByTestId('attach-file-btn')).toBeNull();
  });
});
