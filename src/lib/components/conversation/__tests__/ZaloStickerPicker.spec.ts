import { afterEach, describe, expect, it, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import type { ZaloStickerReference } from '$lib/services/conversationService';
import ZaloStickerPicker from '../ZaloStickerPicker.svelte';

const sticker = (stickerId: number): ZaloStickerReference => ({
  stickerId,
  cateId: 9,
  stickerType: 1,
  url: `https://cdn.example.com/${stickerId}.png`
});

describe('ZaloStickerPicker', () => {
  afterEach(cleanup);

  it('debounces and trims live searches', async () => {
    const search = mock(async () => [sticker(1)]);
    render(ZaloStickerPicker, { conversationId: 31, search });
    await waitFor(() => expect(search).toHaveBeenCalledWith(31, '', 40));
    search.mockClear();

    await fireEvent.input(screen.getByTestId('sticker-search-input'), {
      target: { value: '  chào  ' }
    });
    await new Promise((resolve) => setTimeout(resolve, 350));

    expect(search).toHaveBeenCalledTimes(1);
    expect(search).toHaveBeenCalledWith(31, 'chào', 40);
  });

  it('ignores stale responses from older searches', async () => {
    let resolveOld: (items: ZaloStickerReference[]) => void = () => {};
    const search = mock((_: number, query: string) => {
      if (query === 'old') {
        return new Promise<ZaloStickerReference[]>((resolve) => {
          resolveOld = resolve;
        });
      }
      return Promise.resolve(query === 'new' ? [sticker(2)] : []);
    });
    render(ZaloStickerPicker, { conversationId: 31, search });
    await waitFor(() => expect(search).toHaveBeenCalled());

    const input = screen.getByTestId('sticker-search-input');
    await fireEvent.input(input, { target: { value: 'old' } });
    await new Promise((resolve) => setTimeout(resolve, 320));
    await fireEvent.input(input, { target: { value: 'new' } });
    await new Promise((resolve) => setTimeout(resolve, 320));
    await waitFor(() => expect(screen.getByAltText('Sticker 2')).toBeTruthy());

    resolveOld([sticker(1)]);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(screen.getByAltText('Sticker 2')).toBeTruthy();
    expect(screen.queryByAltText('Sticker 1')).toBeNull();
  });
});
