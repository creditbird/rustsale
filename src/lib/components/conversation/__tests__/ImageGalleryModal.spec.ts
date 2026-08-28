/// <reference types="bun-types" />
import { afterEach, beforeEach, describe, expect, test as it, mock } from 'bun:test';

const saveImageMock = mock(() => Promise.resolve('/path/to/saved.png'));

mock.module('$lib/utils/imageExport', () => ({
  saveImageToFile: saveImageMock,
  isTauriEnvironment: () => true,
  sanitizeImageFilename: (n?: string | null) => n || 'image.png'
}));

import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import ImageGalleryModal from '../ImageGalleryModal.svelte';

describe('ImageGalleryModal Component', () => {
  beforeEach(() => {
    saveImageMock.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  const sampleImages = [
    { url: 'https://example.com/img1.png', alt: 'Image 1', caption: 'First Image' },
    { url: 'https://example.com/img2.png', alt: 'Image 2', caption: 'Second Image' },
    { url: 'https://example.com/img3.png', alt: 'Image 3', caption: 'Third Image' }
  ];

  it('does not render when open is false', () => {
    const { container } = render(ImageGalleryModal, {
      open: false,
      images: sampleImages,
      initialIndex: 0
    });
    expect(screen.queryByTestId('image-gallery-modal')).toBeNull();
  });

  it('renders modal when open is true and displays initial image', () => {
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0
    });

    expect(screen.getByTestId('image-gallery-modal')).not.toBeNull();
    const mainImg = screen.getByTestId('gallery-main-image');
    expect(mainImg.getAttribute('src')).toBe('https://example.com/img1.png');
    expect(screen.getByText('1 / 3')).not.toBeNull();
    expect(screen.getByText('First Image')).not.toBeNull();
  });

  it('navigates to next and previous images using buttons', async () => {
    const onIndexChange = mock();
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0,
      onIndexChange
    });

    const nextBtn = screen.getByTestId('gallery-next-btn');
    await fireEvent.click(nextBtn);

    expect(onIndexChange).toHaveBeenCalledWith(1);
    const mainImg = screen.getByTestId('gallery-main-image');
    expect(mainImg.getAttribute('src')).toBe('https://example.com/img2.png');
    expect(screen.getByText('2 / 3')).not.toBeNull();

    const prevBtn = screen.getByTestId('gallery-prev-btn');
    await fireEvent.click(prevBtn);

    expect(onIndexChange).toHaveBeenCalledWith(0);
    expect(mainImg.getAttribute('src')).toBe('https://example.com/img1.png');
  });

  it('wraps around when navigating past last or before first image', async () => {
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 2
    });

    const nextBtn = screen.getByTestId('gallery-next-btn');
    await fireEvent.click(nextBtn);

    const mainImg = screen.getByTestId('gallery-main-image');
    expect(mainImg.getAttribute('src')).toBe('https://example.com/img1.png');

    const prevBtn = screen.getByTestId('gallery-prev-btn');
    await fireEvent.click(prevBtn);
    expect(mainImg.getAttribute('src')).toBe('https://example.com/img3.png');
  });

  it('handles keyboard navigation (ArrowRight, ArrowLeft, Escape)', async () => {
    const onClose = mock();
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0,
      onClose
    });

    await fireEvent.keyDown(window, { key: 'ArrowRight' });
    const mainImg = screen.getByTestId('gallery-main-image');
    expect(mainImg.getAttribute('src')).toBe('https://example.com/img2.png');

    await fireEvent.keyDown(window, { key: 'ArrowLeft' });
    expect(mainImg.getAttribute('src')).toBe('https://example.com/img1.png');

    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('selects image when thumbnail is clicked', async () => {
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0
    });

    const thumbs = screen.getAllByTestId('gallery-thumbnail');
    expect(thumbs.length).toBe(3);

    await fireEvent.click(thumbs[2]);
    const mainImg = screen.getByTestId('gallery-main-image');
    expect(mainImg.getAttribute('src')).toBe('https://example.com/img3.png');
    expect(screen.getByText('3 / 3')).not.toBeNull();
  });

  it('supports zoom in, zoom out, and reset zoom', async () => {
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0
    });

    const zoomInBtn = screen.getByTestId('gallery-zoom-in-btn');
    const zoomOutBtn = screen.getByTestId('gallery-zoom-out-btn');
    const zoomResetBtn = screen.getByTestId('gallery-zoom-reset-btn');

    const mainImg = screen.getByTestId('gallery-main-image');

    await fireEvent.click(zoomInBtn);
    expect(mainImg.style.transform).toContain('scale(1.25)');

    await fireEvent.click(zoomOutBtn);
    expect(mainImg.style.transform).toContain('scale(1)');

    await fireEvent.click(zoomInBtn);
    await fireEvent.click(zoomInBtn);
    expect(mainImg.style.transform).toContain('scale(1.5)');

    await fireEvent.click(zoomResetBtn);
    expect(mainImg.style.transform).toContain('scale(1)');
  });

  it('calls onClose when close button or backdrop is clicked', async () => {
    const onClose = mock();
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0,
      onClose
    });

    const closeBtn = screen.getByTestId('gallery-close-btn');
    await fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('displays the current zoom percentage', async () => {
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0
    });

    const zoomLevel = screen.getByTestId('gallery-zoom-level');
    expect(zoomLevel.textContent?.trim()).toBe('100%');

    const zoomInBtn = screen.getByTestId('gallery-zoom-in-btn');
    await fireEvent.click(zoomInBtn);
    expect(zoomLevel.textContent?.trim()).toBe('125%');
  });

  it('allows dragging/panning image when zoomed in', async () => {
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0
    });

    const zoomInBtn = screen.getByTestId('gallery-zoom-in-btn');
    await fireEvent.click(zoomInBtn); // zoomScale = 1.25

    const mainImg = screen.getByTestId('gallery-main-image');
    const container = mainImg.parentElement!;

    // Start dragging
    await fireEvent.pointerDown(container, {
      button: 0,
      clientX: 100,
      clientY: 100,
      pointerId: 1
    });

    // Move pointer by dx=40, dy=60
    await fireEvent.pointerMove(container, {
      clientX: 140,
      clientY: 160,
      pointerId: 1
    });

    expect(mainImg.style.transform).toContain('translate(40px, 60px)');
    expect(mainImg.style.transform).toContain('scale(1.25)');

    // Release drag
    await fireEvent.pointerUp(container, {
      clientX: 140,
      clientY: 160,
      pointerId: 1
    });
  });

  it('does not pan when zoomScale is 1.0', async () => {
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0
    });

    const mainImg = screen.getByTestId('gallery-main-image');
    const container = mainImg.parentElement!;

    await fireEvent.pointerDown(container, {
      button: 0,
      clientX: 100,
      clientY: 100,
      pointerId: 1
    });

    await fireEvent.pointerMove(container, {
      clientX: 140,
      clientY: 160,
      pointerId: 1
    });

    expect(mainImg.style.transform).toContain('translate(0px, 0px)');
    expect(mainImg.style.transform).toContain('scale(1)');
  });

  it('toggles zoom on double-click', async () => {
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0
    });

    const mainImg = screen.getByTestId('gallery-main-image');

    // Double click to zoom to 2.0x
    await fireEvent.dblClick(mainImg);
    expect(mainImg.style.transform).toContain('scale(2)');

    // Double click again to reset to 1.0x
    await fireEvent.dblClick(mainImg);
    expect(mainImg.style.transform).toContain('scale(1)');
    expect(mainImg.style.transform).toContain('translate(0px, 0px)');
  });

  it('supports wheel zoom', async () => {
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0
    });

    const mainImg = screen.getByTestId('gallery-main-image');
    const container = mainImg.parentElement!;

    // Wheel up (deltaY < 0) zooms in
    await fireEvent.wheel(container, { deltaY: -100 });
    expect(mainImg.style.transform).toContain('scale(1.25)');

    // Wheel down (deltaY > 0) zooms out
    await fireEvent.wheel(container, { deltaY: 100 });
    expect(mainImg.style.transform).toContain('scale(1)');
  });

  it('supports keyboard zoom controls (+, -, 0)', async () => {
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0
    });

    const mainImg = screen.getByTestId('gallery-main-image');

    // Key + zooms in
    await fireEvent.keyDown(window, { key: '+' });
    expect(mainImg.style.transform).toContain('scale(1.25)');

    // Key - zooms out
    await fireEvent.keyDown(window, { key: '-' });
    expect(mainImg.style.transform).toContain('scale(1)');

    // Key = zooms in
    await fireEvent.keyDown(window, { key: '=' });
    expect(mainImg.style.transform).toContain('scale(1.25)');

    // Key 0 resets zoom
    await fireEvent.keyDown(window, { key: '0' });
    expect(mainImg.style.transform).toContain('scale(1)');
    expect(mainImg.style.transform).toContain('translate(0px, 0px)');
  });

  it('resets pan offset when navigating to another image', async () => {
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0
    });

    const zoomInBtn = screen.getByTestId('gallery-zoom-in-btn');
    await fireEvent.click(zoomInBtn);

    const mainImg = screen.getByTestId('gallery-main-image');
    const container = mainImg.parentElement!;

    await fireEvent.pointerDown(container, { button: 0, clientX: 50, clientY: 50, pointerId: 1 });
    await fireEvent.pointerMove(container, { clientX: 100, clientY: 100, pointerId: 1 });
    await fireEvent.pointerUp(container, { clientX: 100, clientY: 100, pointerId: 1 });

    expect(mainImg.style.transform).toContain('translate(50px, 50px)');

    // Navigate to next
    const nextBtn = screen.getByTestId('gallery-next-btn');
    await fireEvent.click(nextBtn);

    expect(mainImg.style.transform).toContain('translate(0px, 0px)');
    expect(mainImg.style.transform).toContain('scale(1)');
  });

  it('triggers saveImageToFile when download button is clicked', async () => {
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0
    });

    const downloadBtn = screen.getByTestId('gallery-download-btn');
    await fireEvent.click(downloadBtn);

    expect(saveImageMock).toHaveBeenCalledTimes(1);
    expect(saveImageMock).toHaveBeenCalledWith('https://example.com/img1.png', 'Image 1');
  });

  it('triggers saveImageToFile via keyboard shortcut (Ctrl+S or S)', async () => {
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 1
    });

    await fireEvent.keyDown(window, { key: 's', ctrlKey: true });
    expect(saveImageMock).toHaveBeenCalledTimes(1);
    expect(saveImageMock).toHaveBeenCalledWith('https://example.com/img2.png', 'Image 2');
  });

  it('opens context menu on right click on image and allows saving or copying', async () => {
    const clipboardWriteMock = mock(() => Promise.resolve());
    Object.assign(navigator, {
      clipboard: {
        writeText: clipboardWriteMock
      }
    });

    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0
    });

    const mainImg = screen.getByTestId('gallery-main-image');
    expect(screen.queryByTestId('gallery-context-menu')).toBeNull();

    // Right click on image
    await fireEvent.contextMenu(mainImg, { clientX: 200, clientY: 150 });
    const contextMenu = screen.getByTestId('gallery-context-menu');
    expect(contextMenu).not.toBeNull();

    // Click "Lưu ảnh về máy..." in context menu
    const saveBtn = screen.getByTestId('context-menu-save-btn');
    await fireEvent.click(saveBtn);
    expect(saveImageMock).toHaveBeenCalledTimes(1);

    // Reopen context menu
    await fireEvent.contextMenu(mainImg, { clientX: 200, clientY: 150 });
    const copyUrlBtn = screen.getByTestId('context-menu-copy-url-btn');
    await fireEvent.click(copyUrlBtn);
    expect(clipboardWriteMock).toHaveBeenCalledWith('https://example.com/img1.png');
  });

  it('closes context menu with Escape key without closing modal', async () => {
    const onClose = mock();
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0,
      onClose
    });

    const mainImg = screen.getByTestId('gallery-main-image');
    await fireEvent.contextMenu(mainImg, { clientX: 200, clientY: 150 });
    expect(screen.getByTestId('gallery-context-menu')).not.toBeNull();

    // First escape closes context menu
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByTestId('gallery-context-menu')).toBeNull();
    expect(onClose).not.toHaveBeenCalled();

    // Second escape closes modal
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('renders friendly error fallback when image fails to load', async () => {
    render(ImageGalleryModal, {
      open: true,
      images: sampleImages,
      initialIndex: 0
    });

    const mainImg = screen.getByTestId('gallery-main-image');
    expect(screen.queryByTestId('gallery-error-fallback')).toBeNull();

    await fireEvent.error(mainImg);

    expect(screen.getByTestId('gallery-error-fallback')).not.toBeNull();
    expect(screen.getByText('Không thể hiển thị ảnh')).not.toBeNull();
  });
});
