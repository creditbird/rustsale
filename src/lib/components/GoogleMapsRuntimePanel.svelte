<script lang="ts">
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import type { GoogleMapsRuntimeStatus } from '$lib/services/googleMapsScraperService';
  import { Download, RefreshCw, ShieldCheck } from 'lucide-svelte';

  interface Props {
    runtime: GoogleMapsRuntimeStatus | null;
    preparing?: boolean;
    error?: string | null;
    onPrepare: () => void | Promise<unknown>;
  }

  let { runtime, preparing = false, error = null, onPrepare }: Props = $props();
</script>

<Card.Root class="py-0" data-testid="google-maps-runtime-panel">
  <Card.Content class="p-2 flex items-center justify-between gap-3">
    <div class="flex min-w-0 items-center gap-2">
      <div class="rounded-md bg-muted p-1.5 text-muted-foreground">
        <ShieldCheck class="size-4" />
      </div>
      <div class="flex min-w-0 flex-col gap-0.5">
        <div class="flex flex-wrap items-center gap-2">
          <span class="text-xs font-semibold text-foreground">CloakBrowser Runtime</span>
          {#if runtime?.installed}
            <Badge variant="secondary">Ready</Badge>
          {:else if error}
            <Badge variant="destructive">Download failed</Badge>
          {:else}
            <Badge variant="outline">First-use download</Badge>
          {/if}
        </div>
        {#if preparing}
          <p class="truncate text-[11px] text-muted-foreground" aria-live="polite">
            Downloading and verifying the official CloakBrowser runtime...
          </p>
        {:else if runtime?.installed}
          <p class="truncate text-[11px] text-muted-foreground">
            CloakBrowser {runtime.version ?? runtime.bundledVersion ?? 'runtime'} ready
            {runtime.platform ? ` on ${runtime.platform}` : ''}.
          </p>
        {:else if error}
          <p class="truncate text-[11px] text-destructive" role="alert" title={error}>{error}</p>
        {:else}
          <p class="truncate text-[11px] text-muted-foreground">
            Official runtime downloads directly on first use and stays in local cache.
          </p>
        {/if}
      </div>
    </div>

    <Button
      variant={runtime?.installed ? 'outline' : 'default'}
      size="xs"
      onclick={onPrepare}
      disabled={preparing}
      aria-busy={preparing}
      class="shrink-0"
    >
      {#if preparing}
        <RefreshCw class="size-4 animate-spin" /> Downloading CloakBrowser
      {:else}
        <Download class="size-4" />
        {error ? 'Retry CloakBrowser Download' : runtime?.installed ? 'Verify CloakBrowser' : 'Download CloakBrowser'}
      {/if}
    </Button>
  </Card.Content>
</Card.Root>
