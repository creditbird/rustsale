<script lang="ts">
  import {
    X,
    ChevronLeft,
    ChevronRight,
    ZoomIn,
    ZoomOut,
    RotateCcw,
    Download,
    Loader2,
    Copy,
    Check
  } from 'lucide-svelte';
  import { saveImageToFile } from '$lib/utils/imageExport';
  import { toast } from 'svelte-sonner';

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
  let panX = $state(0);
  let panY = $state(0);
  let isDragging = $state(false);
  let isSaving = $state(false);
  let contextMenuOpen = $state(false);
  let contextMenuPos = $state({ x: 0, y: 0 });
  let copied = $state(false);

  let dragStartX = 0;
  let dragStartY = 0;
  let startPanX = 0;
  let startPanY = 0;
  let hasDragged = false;
  let lastOpen = false;

  $effect(() => {
    if (open && !lastOpen) {
      currentIndex = Math.max(0, Math.min(initialIndex, images.length - 1));
      zoomScale = 1.0;
      panX = 0;
      panY = 0;
      isDragging = false;
      contextMenuOpen = false;
      copied = false;
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
    panX = 0;
    panY = 0;
    isDragging = false;
    contextMenuOpen = false;
    onIndexChange(newIdx);
  }

  function nextImage() {
    if (totalImages <= 1) return;
    const newIdx = (currentIndex + 1) % totalImages;
    currentIndex = newIdx;
    zoomScale = 1.0;
    panX = 0;
    panY = 0;
    isDragging = false;
    contextMenuOpen = false;
    onIndexChange(newIdx);
  }

  function selectThumbnail(idx: number) {
    if (idx < 0 || idx >= totalImages) return;
    currentIndex = idx;
    zoomScale = 1.0;
    panX = 0;
    panY = 0;
    isDragging = false;
    contextMenuOpen = false;
    onIndexChange(idx);
  }

  function zoomIn() {
    zoomScale = Math.min(5.0, Number((zoomScale + 0.25).toFixed(2)));
  }

  function zoomOut() {
    const nextScale = Math.max(0.5, Number((zoomScale - 0.25).toFixed(2)));
    zoomScale = nextScale;
    if (nextScale <= 1.0) {
      panX = 0;
      panY = 0;
    }
  }

  function resetZoom() {
    zoomScale = 1.0;
    panX = 0;
    panY = 0;
    isDragging = false;
  }

  function handlePointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    if (contextMenuOpen) {
      closeContextMenu();
    }
    if (zoomScale <= 1.0) return;
    isDragging = true;
    hasDragged = false;
    dragStartX = e.clientX;
    dragStartY = e.clientY;
    startPanX = panX;
    startPanY = panY;

    if (e.currentTarget && 'setPointerCapture' in e.currentTarget) {
      try {
        (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      } catch {
        // ignore
      }
    }
  }

  function handlePointerMove(e: PointerEvent) {
    if (!isDragging) return;
    const dx = e.clientX - dragStartX;
    const dy = e.clientY - dragStartY;
    if (Math.hypot(dx, dy) > 4) {
      hasDragged = true;
    }
    panX = startPanX + dx;
    panY = startPanY + dy;
  }

  function handlePointerUp(e: PointerEvent) {
    if (!isDragging) return;
    isDragging = false;
    if (e.currentTarget && 'releasePointerCapture' in e.currentTarget) {
      try {
        (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
      } catch {
        // ignore
      }
    }
  }

  function handlePointerCancel(e: PointerEvent) {
    if (!isDragging) return;
    isDragging = false;
    if (e.currentTarget && 'releasePointerCapture' in e.currentTarget) {
      try {
        (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
      } catch {
        // ignore
      }
    }
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    if (e.deltaY < 0) {
      zoomIn();
    } else if (e.deltaY > 0) {
      zoomOut();
    }
  }

  function handleDoubleClick(e: MouseEvent) {
    e.stopPropagation();
    if (zoomScale > 1.0) {
      resetZoom();
    } else {
      zoomScale = 2.0;
      panX = 0;
      panY = 0;
    }
  }

  async function downloadImage() {
    if (!currentImage?.url || isSaving) return;
    isSaving = true;
    try {
      const fallbackName =
        currentImage.alt && currentImage.alt.trim() && currentImage.alt !== 'Image'
          ? currentImage.alt.trim()
          : undefined;
      await saveImageToFile(currentImage.url, fallbackName);
    } catch (err) {
      console.error('Failed to save image:', err);
    } finally {
      isSaving = false;
      closeContextMenu();
    }
  }

  async function copyImageUrl() {
    if (!currentImage?.url) return;
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(currentImage.url);
        toast.success('Đã sao chép liên kết ảnh');
        copied = true;
        setTimeout(() => (copied = false), 2000);
      } else {
        const textarea = document.createElement('textarea');
        textarea.value = currentImage.url;
        textarea.style.position = 'fixed';
        textarea.style.opacity = '0';
        document.body.appendChild(textarea);
        textarea.focus();
        textarea.select();
        const successful = document.execCommand('copy');
        document.body.removeChild(textarea);
        if (successful) {
          toast.success('Đã sao chép liên kết ảnh');
          copied = true;
          setTimeout(() => (copied = false), 2000);
        } else {
          toast.error('Không thể sao chép liên kết');
        }
      }
    } catch {
      toast.error('Không thể sao chép liên kết');
    } finally {
      closeContextMenu();
    }
  }

  function handleContextMenu(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    const menuWidth = 220;
    const menuHeight = 260;
    const winWidth = typeof window !== 'undefined' ? window.innerWidth : 800;
    const winHeight = typeof window !== 'undefined' ? window.innerHeight : 600;
    const x = Math.min(e.clientX, winWidth - menuWidth - 10);
    const y = Math.min(e.clientY, winHeight - menuHeight - 10);
    contextMenuPos = { x: Math.max(10, x), y: Math.max(10, y) };
    contextMenuOpen = true;
  }

  function closeContextMenu() {
    contextMenuOpen = false;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      if (contextMenuOpen) {
        closeContextMenu();
      } else {
        onClose();
      }
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
      e.preventDefault();
      void downloadImage();
    } else if (e.key === 's' || e.key === 'S') {
      e.preventDefault();
      void downloadImage();
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      closeContextMenu();
      prevImage();
    } else if (e.key === 'ArrowRight') {
      e.preventDefault();
      closeContextMenu();
      nextImage();
    } else if (e.key === '+' || e.key === '=') {
      e.preventDefault();
      zoomIn();
    } else if (e.key === '-' || e.key === '_') {
      e.preventDefault();
      zoomOut();
    } else if (e.key === '0') {
      e.preventDefault();
      resetZoom();
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (contextMenuOpen) {
      closeContextMenu();
      return;
    }
    if (hasDragged) return;
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
    tabindex="-1"
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
          title="Zoom out (-)"
          data-testid="gallery-zoom-out-btn"
          onclick={zoomOut}
        >
          <ZoomOut class="size-4" />
        </button>

        <span class="text-xs font-mono px-2 py-1 bg-white/10 rounded-md text-white/80 select-none min-w-[3.5rem] text-center" data-testid="gallery-zoom-level">
          {Math.round(zoomScale * 100)}%
        </span>

        <button
          type="button"
          class="p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors text-xs font-mono"
          title="Reset zoom (0)"
          data-testid="gallery-zoom-reset-btn"
          onclick={resetZoom}
        >
          <RotateCcw class="size-4" />
        </button>

        <button
          type="button"
          class="p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors"
          title="Zoom in (+)"
          data-testid="gallery-zoom-in-btn"
          onclick={zoomIn}
        >
          <ZoomIn class="size-4" />
        </button>

        <div class="h-4 w-px bg-white/20 mx-1"></div>

        <button
          type="button"
          class="p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          title="Lưu ảnh về máy (Ctrl+S)"
          disabled={isSaving}
          data-testid="gallery-download-btn"
          onclick={downloadImage}
        >
          {#if isSaving}
            <Loader2 class="size-4 animate-spin" />
          {:else}
            <Download class="size-4" />
          {/if}
        </button>

        <button
          type="button"
          class="p-2 rounded-lg bg-white/15 hover:bg-white/30 text-white transition-colors ml-2"
          title="Close (Esc)"
          data-testid="gallery-close-btn"
          onclick={onClose}
        >
          <X class="size-5" />
        </button>
      </div>
    </div>

    <!-- Main image display area -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="relative flex-1 flex items-center justify-center overflow-hidden p-4 select-none {isDragging ? 'cursor-grabbing' : zoomScale > 1 ? 'cursor-grab' : ''}"
      onclick={handleBackdropClick}
      onwheel={handleWheel}
      onpointerdown={handlePointerDown}
      onpointermove={handlePointerMove}
      onpointerup={handlePointerUp}
      onpointercancel={handlePointerCancel}
      oncontextmenu={handleContextMenu}
    >
      <!-- Prev Button -->
      {#if totalImages > 1}
        <button
          type="button"
          class="absolute left-6 z-10 p-3 rounded-full bg-black/50 hover:bg-black/80 border border-white/20 text-white transition-all hover:scale-105"
          title="Previous image (Left arrow)"
          data-testid="gallery-prev-btn"
          onpointerdown={(e) => e.stopPropagation()}
          onclick={(e) => { e.stopPropagation(); prevImage(); }}
        >
          <ChevronLeft class="size-6" />
        </button>
      {/if}

      <!-- Main Image -->
      <img
        src={currentImage.url}
        alt={currentImage.alt || 'Image preview'}
        class="max-h-[82vh] max-w-[90vw] object-contain rounded-lg shadow-2xl select-none {isDragging ? 'cursor-grabbing duration-0' : zoomScale > 1 ? 'cursor-grab transition-transform duration-150' : 'cursor-zoom-in transition-transform duration-150'}"
        style="transform: translate({panX}px, {panY}px) scale({zoomScale}); touch-action: none;"
        data-testid="gallery-main-image"
        draggable="false"
        ondragstart={(e) => e.preventDefault()}
        ondblclick={handleDoubleClick}
        oncontextmenu={handleContextMenu}
      />

      <!-- Next Button -->
      {#if totalImages > 1}
        <button
          type="button"
          class="absolute right-6 z-10 p-3 rounded-full bg-black/50 hover:bg-black/80 border border-white/20 text-white transition-all hover:scale-105"
          title="Next image (Right arrow)"
          data-testid="gallery-next-btn"
          onpointerdown={(e) => e.stopPropagation()}
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

    <!-- Custom Right-Click Context Menu -->
    {#if contextMenuOpen}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="fixed z-[60] min-w-[200px] overflow-hidden rounded-xl border border-white/15 bg-zinc-900/95 p-1.5 text-zinc-100 shadow-2xl backdrop-blur-xl animate-in fade-in zoom-in-95 duration-100"
        style="left: {contextMenuPos.x}px; top: {contextMenuPos.y}px;"
        data-testid="gallery-context-menu"
        onclick={(e) => e.stopPropagation()}
      >
        <button
          type="button"
          class="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium hover:bg-white/15 transition-colors text-left"
          data-testid="context-menu-save-btn"
          onclick={downloadImage}
        >
          <Download class="size-4 text-primary" />
          <span>Lưu ảnh về máy...</span>
          <span class="ml-auto text-[10px] text-white/50 font-mono">Ctrl+S</span>
        </button>

        <button
          type="button"
          class="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium hover:bg-white/15 transition-colors text-left"
          data-testid="context-menu-copy-url-btn"
          onclick={copyImageUrl}
        >
          {#if copied}
            <Check class="size-4 text-emerald-400" />
            <span class="text-emerald-400">Đã sao chép URL</span>
          {:else}
            <Copy class="size-4 text-white/70" />
            <span>Sao chép URL ảnh</span>
          {/if}
        </button>

        <div class="h-px bg-white/10 my-1"></div>

        <button
          type="button"
          class="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium hover:bg-white/15 transition-colors text-left"
          data-testid="context-menu-zoom-in-btn"
          onclick={() => { zoomIn(); closeContextMenu(); }}
        >
          <ZoomIn class="size-4 text-white/70" />
          <span>Phóng to</span>
          <span class="ml-auto text-[10px] text-white/50 font-mono">+</span>
        </button>

        <button
          type="button"
          class="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium hover:bg-white/15 transition-colors text-left"
          data-testid="context-menu-zoom-out-btn"
          onclick={() => { zoomOut(); closeContextMenu(); }}
        >
          <ZoomOut class="size-4 text-white/70" />
          <span>Thu nhỏ</span>
          <span class="ml-auto text-[10px] text-white/50 font-mono">-</span>
        </button>

        <button
          type="button"
          class="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium hover:bg-white/15 transition-colors text-left"
          data-testid="context-menu-zoom-reset-btn"
          onclick={() => { resetZoom(); closeContextMenu(); }}
        >
          <RotateCcw class="size-4 text-white/70" />
          <span>Đặt lại kích thước</span>
          <span class="ml-auto text-[10px] text-white/50 font-mono">0</span>
        </button>

        <div class="h-px bg-white/10 my-1"></div>

        <button
          type="button"
          class="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium text-red-300 hover:bg-red-500/20 transition-colors text-left"
          data-testid="context-menu-close-btn"
          onclick={() => { closeContextMenu(); onClose(); }}
        >
          <X class="size-4 text-red-400" />
          <span>Đóng xem ảnh</span>
          <span class="ml-auto text-[10px] text-white/50 font-mono">Esc</span>
        </button>
      </div>
    {/if}
  </div>
{/if}
