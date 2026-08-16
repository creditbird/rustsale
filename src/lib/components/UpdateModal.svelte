<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import { Progress } from '$lib/components/ui/progress';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import { updaterService, type UpdaterState } from '$lib/services/updaterService';
  import {
    Sparkles,
    DownloadCloud,
    RotateCw,
    CheckCircle2,
    AlertCircle,
    ArrowUpRight,
    HardDrive,
    Calendar,
    RefreshCw
  } from 'lucide-svelte';
  import { onDestroy, onMount } from 'svelte';

  let updaterState = $state<UpdaterState>(updaterService.getState());

  let unsubscribe: (() => void) | null = null;

  onMount(() => {
    unsubscribe = updaterService.subscribe((s) => {
      updaterState = s;
    });
  });

  onDestroy(() => {
    if (unsubscribe) {
      unsubscribe();
    }
  });

  function handleOpenChange(open: boolean) {
    if (!open && updaterState.status !== 'downloading') {
      updaterService.closeModal();
    }
  }

  function formatBytes(bytes: number): string {
    if (!bytes || bytes <= 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }

  function formatDate(dateStr: string | null): string {
    if (!dateStr) return '';
    try {
      return new Date(dateStr).toLocaleDateString('en-US', {
        year: 'numeric',
        month: 'short',
        day: 'numeric'
      });
    } catch {
      return dateStr;
    }
  }
</script>

<Dialog.Root open={updaterState.isModalOpen} onOpenChange={handleOpenChange}>
  <Dialog.Content class="sm:max-w-[540px] p-0 overflow-hidden border-border/80 shadow-2xl">
    <!-- Header Banner -->
    <div class="relative bg-linear-to-br from-primary/15 via-primary/5 to-background p-6 border-b border-border/60">
      <div class="flex items-start gap-4">
        <div class="size-12 rounded-2xl bg-primary text-primary-foreground flex items-center justify-center shadow-md shadow-primary/20 shrink-0">
          <Sparkles class="size-6" />
        </div>
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-2">
            <Dialog.Title class="text-lg font-bold tracking-tight text-foreground">
              New Software Update
            </Dialog.Title>
            {#if updaterState.availableVersion}
              <Badge variant="default" class="bg-primary/90 text-[11px] font-semibold">
                v{updaterState.availableVersion}
              </Badge>
            {/if}
          </div>
          <Dialog.Description class="text-xs text-muted-foreground mt-1">
            RustSale CRM & Outreach Pro • Distributed via Cloudflare R2
          </Dialog.Description>
        </div>
      </div>
    </div>

    <div class="p-6 flex flex-col gap-5">
      <!-- Version Transition Info Card -->
      <div class="grid grid-cols-2 gap-3 p-3.5 rounded-xl bg-muted/40 border border-border/60 text-xs">
        <div class="flex flex-col gap-1">
          <span class="text-muted-foreground font-medium">Current Version</span>
          <span class="font-semibold text-foreground flex items-center gap-1.5">
            <span class="size-2 rounded-full bg-muted-foreground/40"></span>
            v{updaterState.currentVersion}
          </span>
        </div>
        <div class="flex flex-col gap-1 border-l border-border/60 pl-3">
          <span class="text-muted-foreground font-medium">Latest Version</span>
          <span class="font-semibold text-emerald-600 dark:text-emerald-400 flex items-center gap-1.5">
            <span class="size-2 rounded-full bg-emerald-500 animate-pulse"></span>
            v{updaterState.availableVersion || '---'}
          </span>
        </div>
      </div>

      <!-- Release Date & Meta (if available) -->
      {#if updaterState.releaseDate}
        <div class="flex items-center gap-4 text-xs text-muted-foreground px-1">
          <span class="flex items-center gap-1.5">
            <Calendar class="size-3.5" /> {formatDate(updaterState.releaseDate)}
          </span>
          <span class="flex items-center gap-1.5">
            <HardDrive class="size-3.5" /> Ed25519 Verified
          </span>
        </div>
      {/if}

      <!-- Release Notes -->
      <div class="flex flex-col gap-2">
        <span class="text-xs font-semibold text-foreground uppercase tracking-wider">
          Release Notes
        </span>
        <div class="rounded-xl border border-border/70 bg-background/50 overflow-hidden">
          <ScrollArea class="h-36 p-3 text-xs leading-relaxed text-muted-foreground">
            {#if updaterState.releaseNotes}
              <div class="whitespace-pre-line text-foreground/90 font-normal">
                {updaterState.releaseNotes}
              </div>
            {:else}
              <div class="italic text-muted-foreground">No detailed release notes provided.</div>
            {/if}
          </ScrollArea>
        </div>
      </div>

      <!-- Download & Progress Section -->
      {#if updaterState.status === 'downloading'}
        <div class="flex flex-col gap-2.5 p-4 rounded-xl bg-primary/5 border border-primary/20">
          <div class="flex items-center justify-between text-xs">
            <span class="font-medium text-foreground flex items-center gap-2">
              <RotateCw class="size-3.5 animate-spin text-primary" />
              Downloading update...
            </span>
            <span class="font-semibold text-primary">
              {updaterState.progress.percentage}%
            </span>
          </div>
          <Progress value={updaterState.progress.percentage} max={100} class="h-2 bg-primary/20" />
          <div class="flex justify-between text-[11px] text-muted-foreground">
            <span>Downloaded: {formatBytes(updaterState.progress.downloaded)}</span>
            {#if updaterState.progress.total > 0}
              <span>Total: {formatBytes(updaterState.progress.total)}</span>
            {/if}
          </div>
        </div>
      {:else}
        {#if updaterState.status === 'downloaded'}
          <div class="flex items-center gap-3 p-3.5 rounded-xl bg-emerald-500/10 border border-emerald-500/20 text-xs text-emerald-700 dark:text-emerald-300">
            <CheckCircle2 class="size-5 text-emerald-600 shrink-0" />
            <div>
              <span class="font-semibold">Update downloaded successfully!</span>
              <p class="text-[11px] opacity-90 mt-0.5">Restart the application to complete installing the new version.</p>
            </div>
          </div>
        {:else}
          {#if updaterState.status === 'error'}
            <div class="flex items-start gap-3 p-3.5 rounded-xl bg-destructive/10 border border-destructive/20 text-xs text-destructive">
              <AlertCircle class="size-5 shrink-0 mt-0.5" />
              <div class="flex-1">
                <span class="font-semibold">Update failed</span>
                <p class="text-[11px] opacity-90 mt-0.5">{updaterState.errorMessage || 'Please check your network connection.'}</p>
              </div>
            </div>
          {/if}
        {/if}
      {/if}
    </div>

    <!-- Dialog Actions Footer -->
    <div class="p-4 bg-muted/20 border-t border-border/60 flex items-center justify-end gap-2.5">
      {#if updaterState.status === 'downloaded'}
        <Button variant="outline" size="sm" onclick={() => updaterService.closeModal()}>
          Later
        </Button>
        <Button
          size="sm"
          class="bg-emerald-600 hover:bg-emerald-700 text-white font-semibold gap-2 shadow-sm"
          onclick={() => updaterService.relaunch()}
        >
          <RefreshCw class="size-4" />
          Restart Now
        </Button>
      {:else}
        {#if updaterState.status === 'downloading'}
          <Button variant="outline" size="sm" disabled>
            <RotateCw class="size-3.5 animate-spin mr-2" /> Updating...
          </Button>
        {:else}
          <Button
            variant="ghost"
            size="sm"
            onclick={() => updaterService.closeModal()}
          >
            Later
          </Button>
          <Button
            size="sm"
            class="bg-primary hover:bg-primary/90 text-primary-foreground font-semibold gap-2 shadow-sm shadow-primary/20"
            onclick={() => updaterService.downloadAndInstall()}
          >
            <DownloadCloud class="size-4" />
            Update Now
          </Button>
        {/if}
      {/if}
    </div>
  </Dialog.Content>
</Dialog.Root>
