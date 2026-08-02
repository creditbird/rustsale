import { describe, it, expect, mock, beforeEach, afterEach } from 'bun:test';
import { render, fireEvent, cleanup } from '@testing-library/svelte';
import AttachmentPreviewQueue from '../AttachmentPreviewQueue.svelte';
import type { ComposerAttachmentItem } from '../composerDraftState';

describe('AttachmentPreviewQueue', () => {
  beforeEach(() => {
    cleanup();
  });

  afterEach(() => {
    cleanup();
  });
  const sampleItems: ComposerAttachmentItem[] = [
    {
      localId: '1',
      stagedAttachmentId: 'staged-1',
      fileName: 'document.pdf',
      mimeType: 'application/pdf',
      sizeBytes: 2048,
      attachmentType: 'file',
      previewDataUrl: null,
      status: 'ready',
      errorMessage: null,
    },
    {
      localId: '2',
      stagedAttachmentId: null,
      fileName: 'photo.png',
      mimeType: 'image/png',
      sizeBytes: 1048576,
      attachmentType: 'image',
      previewDataUrl: 'data:image/png;base64,abc',
      status: 'staging',
      errorMessage: null,
    },
  ];

  it('renders nothing when items list is empty', () => {
    const { queryByTestId } = render(AttachmentPreviewQueue, { props: { items: [] } });
    expect(queryByTestId('attachment-preview-queue')).toBeNull();
  });

  it('renders list of items with proper status badges', () => {
    const { getAllByTestId, getByTestId, getByText } = render(AttachmentPreviewQueue, {
      props: { items: sampleItems },
    });

    const items = getAllByTestId('attachment-item');
    expect(items.length).toBe(2);
    expect(getByText('document.pdf')).not.toBeNull();
    expect(getByText('photo.png')).not.toBeNull();
    expect(getByTestId('status-staging')).not.toBeNull();
  });

  it('calls onRemove when remove button is clicked', async () => {
    const onRemove = mock();
    const { getAllByTestId } = render(AttachmentPreviewQueue, {
      props: {
        items: sampleItems,
        onRemove,
      },
    });

    const removeBtns = getAllByTestId('remove-attachment-btn');
    await fireEvent.click(removeBtns[0]);

    expect(onRemove).toHaveBeenCalledWith(sampleItems[0]);
  });
});
