/// <reference types="bun-types" />
import { describe, it, expect, mock, beforeEach, afterEach } from 'bun:test';
import { render, fireEvent, cleanup, waitFor } from '@testing-library/svelte';
import ImageMessageContent from '../ImageMessageContent.svelte';
import type { ConversationAttachment } from '$lib/services/conversationService';

const downloadMock = mock((id: number) => {
  return Promise.resolve({
    id,
    mediaType: 'image',
    mimeType: 'image/jpeg',
    fileName: 'downloaded.jpg',
    sizeBytes: 50000,
    remoteUrl: 'https://mmg.whatsapp.net/v/downloaded.enc',
    thumbnailUrl: 'data:image/jpeg;base64,/9j/4AAQSkZJRg==',
    width: 800,
    height: 600,
    durationSeconds: null,
  } as ConversationAttachment);
});

mock.module('$lib/services/conversationService', () => ({
  downloadConversationAttachment: downloadMock,
}));

describe('ImageMessageContent Component', () => {
  beforeEach(() => {
    downloadMock.mockClear();
    cleanup();
  });

  afterEach(() => {
    cleanup();
  });

  it('renders image directly when thumbnailUrl data URL is present', () => {
    const attachment: ConversationAttachment = {
      id: 101,
      mediaType: 'image',
      mimeType: 'image/jpeg',
      fileName: 'photo.jpg',
      sizeBytes: 12345,
      remoteUrl: 'https://mmg.whatsapp.net/v/photo.enc',
      thumbnailUrl: 'data:image/jpeg;base64,/9j/4AAQSkZJRg==',
      width: 400,
      height: 300,
      durationSeconds: null,
    };

    const { getByTestId, queryByTestId } = render(ImageMessageContent, {
      props: { attachment }
    });

    expect(getByTestId('image-message-content')).not.toBeNull();
    expect(queryByTestId('image-message-fallback')).toBeNull();
  });

  it('renders image directly when standard non-encrypted remoteUrl is present', () => {
    const attachment: ConversationAttachment = {
      id: 102,
      mediaType: 'image',
      mimeType: 'image/jpeg',
      fileName: 'banner.jpg',
      sizeBytes: 24000,
      remoteUrl: 'https://example.com/banner.jpg',
      thumbnailUrl: null,
      width: 800,
      height: 600,
      durationSeconds: null,
    };

    const { getByTestId, queryByTestId } = render(ImageMessageContent, {
      props: { attachment }
    });

    expect(getByTestId('image-message-content')).not.toBeNull();
    expect(queryByTestId('image-message-fallback')).toBeNull();
  });

  it('renders fallback and auto-downloads WhatsApp encrypted image without thumbnail', async () => {
    const attachment: ConversationAttachment = {
      id: 103,
      mediaType: 'image',
      mimeType: 'image/jpeg',
      fileName: 'encrypted_wa.jpg',
      sizeBytes: 197632,
      remoteUrl: 'https://mmg.whatsapp.net/v/encrypted_wa.enc',
      thumbnailUrl: null,
      width: null,
      height: null,
      durationSeconds: null,
    };

    const { getByTestId } = render(ImageMessageContent, {
      props: { attachment }
    });

    // Auto-trigger on mount
    await waitFor(() => {
      expect(downloadMock).toHaveBeenCalledWith(103);
    });

    // Once downloaded, renders the image container
    await waitFor(() => {
      expect(getByTestId('image-message-content')).not.toBeNull();
    });
  });

  it('does not render an encrypted WhatsApp CDN thumbnail directly', async () => {
    const attachment: ConversationAttachment = {
      id: 107,
      mediaType: 'image',
      mimeType: 'image/jpeg',
      fileName: 'encrypted_thumbnail.jpg',
      sizeBytes: 197632,
      remoteUrl: null,
      thumbnailUrl: 'https://mmg.whatsapp.net/v/encrypted_thumbnail.enc',
      width: null,
      height: null,
      durationSeconds: null,
    };

    const { getByTestId } = render(ImageMessageContent, {
      props: { attachment }
    });

    await waitFor(() => {
      expect(downloadMock).toHaveBeenCalledWith(107);
      expect(getByTestId('image-message-content')).not.toBeNull();
    });
  });

  it('shows a low-resolution thumbnail immediately then resolves encrypted WhatsApp media', async () => {
    const lowResolutionThumbnail = 'data:image/jpeg;base64,TE9X';
    const attachment: ConversationAttachment = {
      id: 108,
      mediaType: 'image',
      mimeType: 'image/jpeg',
      fileName: 'upgrade_thumbnail.jpg',
      sizeBytes: 197632,
      remoteUrl: 'https://mmg.whatsapp.net/v/upgrade_thumbnail.enc',
      thumbnailUrl: lowResolutionThumbnail,
      width: null,
      height: null,
      durationSeconds: null,
    };

    const { getByTestId } = render(ImageMessageContent, {
      props: { attachment }
    });

    expect((getByTestId('image-message-content').querySelector('img') as HTMLImageElement).src)
      .toContain(lowResolutionThumbnail);
    await waitFor(() => {
      expect(downloadMock).toHaveBeenCalledWith(108);
      expect((getByTestId('image-message-content').querySelector('img') as HTMLImageElement).src)
        .toContain('data:image/jpeg;base64,/9j/4AAQSkZJRg==');
    });
  });

  it('handles click on fallback card to trigger download and open callback', async () => {
    const onImageClick = mock();
    const attachment: ConversationAttachment = {
      id: 104,
      mediaType: 'image',
      mimeType: 'image/jpeg',
      fileName: 'click_dl.jpg',
      sizeBytes: 50000,
      remoteUrl: 'https://mmg.whatsapp.net/v/click_dl.enc',
      thumbnailUrl: null,
      width: null,
      height: null,
      durationSeconds: null,
    };

    const { getByTestId } = render(ImageMessageContent, {
      props: { attachment, onImageClick }
    });

    const fallback = getByTestId('image-message-fallback');
    await fireEvent.click(fallback);

    await waitFor(() => {
      expect(downloadMock).toHaveBeenCalledWith(104);
      expect(onImageClick).toHaveBeenCalledWith('data:image/jpeg;base64,/9j/4AAQSkZJRg==');
    });
  });

  it('displays permanent missing key error without retry prompt and blocks clicking', async () => {
    downloadMock.mockImplementationOnce(() =>
      Promise.reject({
        code: 'media_metadata_unavailable',
        message: 'Không thể khôi phục ảnh lịch sử. Vui lòng yêu cầu người gửi gửi lại ảnh.'
      })
    );

    const attachment: ConversationAttachment = {
      id: 105,
      mediaType: 'image',
      mimeType: 'image/jpeg',
      fileName: 'old_photo.jpg',
      sizeBytes: 197632,
      remoteUrl: 'https://mmg.whatsapp.net/v/old_photo.enc',
      thumbnailUrl: null,
      width: null,
      height: null,
      durationSeconds: null,
    };

    const { getByTestId, findByText, queryByText } = render(ImageMessageContent, {
      props: { attachment }
    });

    await waitFor(() => {
      expect(downloadMock).toHaveBeenCalledWith(105);
    });

    const errorMsg = await findByText(/Không thể khôi phục ảnh lịch sử/);
    expect(errorMsg).not.toBeNull();
    expect(queryByText(/Thử lại/)).toBeNull();

    // Clicking fallback card when not retryable does not trigger another download
    downloadMock.mockClear();
    const fallback = getByTestId('image-message-fallback');
    await fireEvent.click(fallback);
    expect(downloadMock).not.toHaveBeenCalled();
  });

  it('keeps manual retry available for a structured transient provider failure', async () => {
    downloadMock.mockImplementationOnce(() =>
      Promise.reject({
        code: 'provider_failure',
        message: 'Kết nối WhatsApp tạm thời gián đoạn'
      })
    );

    const attachment: ConversationAttachment = {
      id: 106,
      mediaType: 'image',
      mimeType: 'image/jpeg',
      fileName: 'retryable.jpg',
      sizeBytes: 20000,
      remoteUrl: 'https://mmg.whatsapp.net/v/retryable.enc',
      thumbnailUrl: null,
      width: null,
      height: null,
      durationSeconds: null,
    };

    const { getByTestId, findByText } = render(ImageMessageContent, {
      props: { attachment }
    });

    await findByText(/Kết nối WhatsApp tạm thời gián đoạn/);
    expect(await findByText(/Thử lại/)).not.toBeNull();

    downloadMock.mockClear();
    await fireEvent.click(getByTestId('image-message-fallback'));
    await waitFor(() => {
      expect(downloadMock).toHaveBeenCalledWith(106);
      expect(getByTestId('image-message-content')).not.toBeNull();
    });
  });
});
