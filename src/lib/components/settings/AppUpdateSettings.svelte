<script lang="ts">
  import * as Card from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import { updaterService, type UpdaterState } from '$lib/services/updaterService';
  import {
    RefreshCw,
    CheckCircle2,
    Sparkles,
    ShieldCheck,
    Cloud,
    HardDrive,
    AlertCircle,
    DownloadCloud
  } from 'lucide-svelte';
  import { onMount, onDestroy } from 'svelte';

  let updaterState = $state<UpdaterState>(updaterService.getState());
  let lastCheckedTime = $state<string | null>(null);

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

  async function handleManualCheck() {
    lastCheckedTime = new Date().toLocaleTimeString('vi-VN', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    });
    await updaterService.checkForUpdate({ silent: false });
  }
</script>

<div class="flex flex-col gap-5">
  <!-- Main Update Card -->
  <Card.Root class="overflow-hidden border-border/80">
    <Card.Header class="bg-muted/10 border-b border-border/60 pb-4">
      <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div class="flex items-start gap-3">
          <div class="rounded-xl bg-primary/10 p-2.5 text-primary shrink-0">
            <Sparkles class="size-5" />
          </div>
          <div>
            <Card.Title class="text-base font-bold text-foreground">
              Cập nhật phần mềm (Auto-Update)
            </Card.Title>
            <Card.Description class="text-xs text-muted-foreground mt-0.5">
              Hệ thống cập nhật bảo mật qua Cloudflare R2 CDN và mã hóa Ed25519.
            </Card.Description>
          </div>
        </div>

        <div class="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            class="h-9 gap-2 font-medium"
            disabled={updaterState.status === 'checking' || updaterState.status === 'downloading'}
            onclick={handleManualCheck}
          >
            <RefreshCw class="size-3.5 {updaterState.status === 'checking' ? 'animate-spin text-primary' : ''}" />
            {updaterState.status === 'checking' ? 'Đang kiểm tra...' : 'Kiểm tra cập nhật'}
          </Button>
        </div>
      </div>
    </Card.Header>

    <Card.Content class="p-6 flex flex-col gap-5">
      <!-- Status & Version Details -->
      <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div class="p-4 rounded-xl border border-border/70 bg-background/60 flex flex-col gap-1.5">
          <span class="text-xs text-muted-foreground font-medium">Phiên bản hiện tại</span>
          <div class="flex items-center gap-2">
            <span class="text-lg font-bold text-foreground">v{updaterState.currentVersion}</span>
            <Badge variant="secondary" class="text-[10px] font-semibold">Bản cài đặt</Badge>
          </div>
        </div>

        <div class="p-4 rounded-xl border border-border/70 bg-background/60 flex flex-col gap-1.5">
          <span class="text-xs text-muted-foreground font-medium">Kênh phân phối</span>
          <div class="flex items-center gap-2">
            <span class="text-sm font-semibold text-foreground flex items-center gap-1.5">
              <Cloud class="size-4 text-sky-500" /> Cloudflare R2
            </span>
            <Badge variant="outline" class="text-[10px] text-emerald-600 bg-emerald-500/10 border-emerald-500/20">
              Live
            </Badge>
          </div>
        </div>

        <div class="p-4 rounded-xl border border-border/70 bg-background/60 flex flex-col gap-1.5">
          <span class="text-xs text-muted-foreground font-medium">Trạng thái cập nhật</span>
          <div class="flex items-center gap-2">
            {#if updaterState.status === 'available'}
              <span class="text-sm font-bold text-primary flex items-center gap-1.5">
                <span class="size-2 rounded-full bg-primary animate-ping"></span>
                Có bản v{updaterState.availableVersion}
              </span>
            {:else if updaterState.status === 'checking'}
              <span class="text-sm text-muted-foreground flex items-center gap-1.5">
                <RefreshCw class="size-3.5 animate-spin" /> Đang kiểm tra...
              </span>
            {:else if updaterState.status === 'error'}
              <span class="text-sm font-medium text-destructive flex items-center gap-1.5">
                <AlertCircle class="size-4" /> Lỗi kết nối
              </span>
            {:else}
              <span class="text-sm font-semibold text-emerald-600 dark:text-emerald-400 flex items-center gap-1.5">
                <CheckCircle2 class="size-4 text-emerald-600" /> Mới nhất
              </span>
            {/if}
          </div>
          {#if lastCheckedTime}
            <span class="text-[10px] text-muted-foreground">Kiểm tra lúc: {lastCheckedTime}</span>
          {/if}
        </div>
      </div>

      <!-- Alert if update is available -->
      {#if updaterState.status === 'available'}
        <div class="p-4 rounded-xl bg-primary/10 border border-primary/20 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div class="flex items-start gap-3">
            <Sparkles class="size-5 text-primary shrink-0 mt-0.5" />
            <div>
              <span class="text-sm font-bold text-foreground">
                Đã có bản cập nhật mới v{updaterState.availableVersion}
              </span>
              <p class="text-xs text-muted-foreground mt-0.5 line-clamp-1">
                {updaterState.releaseNotes || 'Nhấn cập nhật để tải và cài đặt phiên bản mới nhất.'}
              </p>
            </div>
          </div>
          <Button
            size="sm"
            class="bg-primary text-primary-foreground font-semibold gap-2 shrink-0 shadow-sm"
            onclick={() => updaterService.openModal()}
          >
            <DownloadCloud class="size-4" />
            Xem & Cài đặt
          </Button>
        </div>
      {/if}
    </Card.Content>
  </Card.Root>

  <!-- Security & Infrastructure Info -->
  <Card.Root class="overflow-hidden border-border/80">
    <Card.Header class="bg-muted/10 border-b border-border/60 pb-3">
      <div class="flex items-center gap-2.5">
        <ShieldCheck class="size-4 text-emerald-600" />
        <Card.Title class="text-sm font-semibold text-foreground">Bảo mật & Toàn vẹn dữ liệu</Card.Title>
      </div>
    </Card.Header>
    <Card.Content class="p-5 grid grid-cols-1 sm:grid-cols-2 gap-4 text-xs text-muted-foreground">
      <div class="flex items-start gap-3 p-3 rounded-lg bg-muted/20 border border-border/60">
        <ShieldCheck class="size-4 text-emerald-600 shrink-0 mt-0.5" />
        <div>
          <span class="font-semibold text-foreground">Chữ ký số Ed25519</span>
          <p class="mt-0.5 text-[11px] leading-relaxed">
            Mọi bản cập nhật đều được kiểm tra chữ ký mã hóa trước khi cài đặt, đảm bảo chống giả mạo mã nguồn tuyệt đối.
          </p>
        </div>
      </div>

      <div class="flex items-start gap-3 p-3 rounded-lg bg-muted/20 border border-border/60">
        <HardDrive class="size-4 text-sky-600 shrink-0 mt-0.5" />
        <div>
          <span class="font-semibold text-foreground">Bảo toàn SQLite Local DB</span>
          <p class="mt-0.5 text-[11px] leading-relaxed">
            Dữ liệu CRM, chiến dịch và cấu hình nằm trong thư mục người dùng riêng biệt, không bị ảnh hưởng khi nâng cấp app.
          </p>
        </div>
      </div>
    </Card.Content>
  </Card.Root>
</div>
