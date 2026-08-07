<script lang="ts">
  import { untrack } from 'svelte';
  import { X, ChevronLeft, ChevronRight, ZoomIn, ZoomOut, RotateCcw, Download } from 'lucide-svelte';

  export interface GalleryImage {
    url: string;
    alt?: string;
    caption?: string;
    title?: string;
    timestamp?: string;
  }

  let {
    open = false,
    images = [],
    initialIndex = 0,
    onClose = () => {},
    onIndexChange = () => {}
  }: {
    open?: boolean;
    images?: GalleryImage[];
    initialIndex?: number;
    onClose?: () => void;
    onIndexChange?: (index: number) => void;
  } = $props();

  let currentIndex = $state(0);
  let zoomScale = $state(1.0);
  let lastOpen = false;

  $effect(() => {
    if (open && !lastOpen) {
      currentIndex = Math.max(0, Math.min(initialIndex, images.length - 1));
      zoomScale = 1.0;
    }
    lastOpen = open;
  });

  let currentImage = $derived(images[currentIndex] ?? null);
  let totalImages = $derived(images.length);

  function prevImage() {
    if (totalImages <= 1) return;
    const newIdx = (currentIndex - 1 + totalImages) % totalImages;
    currentIndex = newIdx;
    zoomScale = 1.0;
    onIndexChange(newIdx);
  }

  function nextImage() {
    if (totalImages <= 1) return;
    const newIdx = (currentIndex + 1) % totalImages;
    currentIndex = newIdx;
    zoomScale = 1.0;
    onIndexChange(newIdx);
  }

  function selectThumbnail(idx: number) {
    if (idx < 0 || idx >= totalImages) return;
    currentIndex = idx;
    zoomScale = 1.0;
    onIndexChange(idx);
  }

  function zoomIn() {
    zoomScale = Math.min(3.0, Number((zoomScale + 0.25).toFixed(2)));
  }

  function zoomOut() {
    zoomScale = Math.max(0.5, Number((zoomScale - 0.25).toFixed(2)));
  }

  function resetZoom() {
    zoomScale = 1.0;
  }

  function downloadImage() {
    if (!currentImage?.url) return;
    const a = document.createElement('a');
    a.href = currentImage.url;
    a.download = currentImage.alt || `image-${currentIndex + 1}.png`;
    a.target = '_blank';
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      prevImage();
    } else if (e.key === 'ArrowRight') {
      e.preventDefault();
      nextImage();
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open && currentImage}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 flex flex-col bg-black/90 text-white backdrop-blur-md transition-opacity animate-in fade-in duration-200"
    data-testid="image-gallery-modal"
    role="dialog"
    aria-modal="true"
    aria-label="Image Gallery Modal"
    onclick={handleBackdropClick}
  >
    <!-- Header bar -->
    <div class="flex items-center justify-between px-6 py-4 bg-black/40 border-b border-white/10 shrink-0 select-none">
      <div class="flex items-center gap-3">
        {#if totalImages > 1}
          <span class="text-xs font-semibold px-2.5 py-1 rounded-full bg-white/15 text-white/90">
            {currentIndex + 1} / {totalImages}
          </span>
        {/if}
        {#if currentImage.caption || currentImage.title}
          <span class="text-sm font-medium text-white/90 truncate max-w-md">
            {currentImage.caption || currentImage.title}
          </span>
        {/if}
      </div>

      <!-- Controls -->
      <div class="flex items-center gap-2">
        <button
          type="button"
          class="p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors"
          title="Thu nhỏ (-)"
          data-testid="gallery-zoom-out-btn"
          onclick={zoomOut}
        >
          <ZoomOut class="size-4" />
        </button>

        <button
          type="button"
          class="p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors text-xs font-mono"
          title="Đặt lại kích thước"
          data-testid="gallery-zoom-reset-btn"
          onclick={resetZoom}
        >
          <RotateCcw class="size-4" />
        </button>

        <button
          type="button"
          class="p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors"
          title="Phóng to (+)"
          data-testid="gallery-zoom-in-btn"
          onclick={zoomIn}
        >
          <ZoomIn class="size-4" />
        </button>

        <div class="h-4 w-px bg-white/20 mx-1"></div>

        <button
          type="button"
          class="p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors"
          title="Tải ảnh về"
          data-testid="gallery-download-btn"
          onclick={downloadImage}
        >
          <Download class="size-4" />
        </button>

        <button
          type="button"
          class="p-2 rounded-lg bg-white/15 hover:bg-white/30 text-white transition-colors ml-2"
          title="Đóng (Esc)"
          data-testid="gallery-close-btn"
          onclick={onClose}
        >
          <X class="size-5" />
        </button>
      </div>
    </div>

    <!-- Main image display area -->
    <div class="relative flex-1 flex items-center justify-center overflow-hidden p-4 select-none" onclick={handleBackdropClick}>
      <!-- Prev Button -->
      {#if totalImages > 1}
        <button
          type="button"
          class="absolute left-6 z-10 p-3 rounded-full bg-black/50 hover:bg-black/80 border border-white/20 text-white transition-all hover:scale-105"
          title="Ảnh trước (Mũi tên Trái)"
          data-testid="gallery-prev-btn"
          onclick={(e) => { e.stopPropagation(); prevImage(); }}
        >
          <ChevronLeft class="size-6" />
        </button>
      {/if}

      <!-- Main Image -->
      <img
        src={currentImage.url}
        alt={currentImage.alt || 'Hình ảnh preview'}
        class="max-h-[82vh] max-w-[90vw] object-contain transition-transform duration-150 rounded-lg shadow-2xl"
        style="transform: scale({zoomScale});"
        data-testid="gallery-main-image"
      />

      <!-- Next Button -->
      {#if totalImages > 1}
        <button
          type="button"
          class="absolute right-6 z-10 p-3 rounded-full bg-black/50 hover:bg-black/80 border border-white/20 text-white transition-all hover:scale-105"
          title="Ảnh tiếp theo (Mũi tên Phải)"
          data-testid="gallery-next-btn"
          onclick={(e) => { e.stopPropagation(); nextImage(); }}
        >
          <ChevronRight class="size-6" />
        </button>
      {/if}
    </div>

    <!-- Thumbnails strip -->
    {#if totalImages > 1}
      <div class="px-6 py-3 bg-black/60 border-t border-white/10 flex items-center justify-center gap-2 overflow-x-auto shrink-0 select-none">
        {#each images as img, idx (img.url + idx)}
          <button
            type="button"
            class="relative size-12 rounded-lg overflow-hidden border-2 transition-all shrink-0 {idx === currentIndex ? 'border-primary ring-2 ring-primary/50 scale-105' : 'border-white/20 opacity-60 hover:opacity-100'}"
            data-testid="gallery-thumbnail"
            onclick={() => selectThumbnail(idx)}
          >
            <img src={img.url} alt={img.alt || `Thumb ${idx + 1}`} class="size-full object-cover" />
          </button>
        {/each}
      </div>
    {/if}
  </div>
{/if}
