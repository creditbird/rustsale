/// <reference types="bun-types" />
import { afterEach, describe, expect, test as it, mock } from 'bun:test';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import ImageGalleryModal from '../ImageGalleryModal.svelte';

describe('ImageGalleryModal Component', () => {
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
});
