<script lang="ts">
  import { Badge, type BadgeVariant } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Progress } from '$lib/components/ui/progress';
  import type {
    GoogleMapsJobStatus,
    GoogleMapsJobViewState,
    GoogleMapsScrapeJob
  } from '$lib/services/googleMapsScraperService';
  import { AlertTriangle, Pause, Play, RefreshCw, Square, Terminal } from 'lucide-svelte';

  interface Props {
    job: GoogleMapsScrapeJob | null;
    view: GoogleMapsJobViewState | null;
    logs: string[];
    actionPending?: boolean;
    onPause: () => void | Promise<void>;
    onResume: () => void | Promise<void>;
    onCancel: () => void | Promise<void>;
    onRetryFailed: () => void | Promise<void>;
    onRetryEmail: () => void | Promise<void>;
  }

  let {
    job,
    view,
    logs,
    actionPending = false,
    onPause,
    onResume,
    onCancel,
    onRetryFailed,
    onRetryEmail
  }: Props = $props();

  let status = $derived(view?.status ?? job?.status ?? null);
  let percentage = $derived(view?.percentage ?? progressFromJob(job));
  let statusMessage = $derived(
    view?.message || job?.last_error_message || defaultStatusMessage(status)
  );

  function progressFromJob(current: GoogleMapsScrapeJob | null): number {
    if (!current || current.total_units === 0) return 0;
    return Math.min(100, Math.max(0, (current.processed_units / current.total_units) * 100));
  }

  function statusLabel(current: GoogleMapsJobStatus | null): string {
    switch (current) {
      case 'queued': return 'Queued';
      case 'running': return 'Running';
      case 'paused': return 'Paused';
      case 'paused_requires_action': return 'Action Required';
      case 'paused_limit_reached': return 'Limit Reached';
      case 'interrupted': return 'Interrupted';
      case 'partial': return 'Partial';
      case 'completed': return 'Completed';
      case 'failed': return 'Failed';
      case 'cancelled': return 'Cancelled';
      default: return 'Idle';
    }
  }

  function statusVariant(current: GoogleMapsJobStatus | null): BadgeVariant {
    if (current === 'failed' || current === 'cancelled') return 'destructive';
    if (current === 'partial' || current === 'paused_requires_action') return 'secondary';
    if (current === 'running') return 'default';
    return 'outline';
  }

  function defaultStatusMessage(current: GoogleMapsJobStatus | null): string {
    switch (current) {
      case 'queued': return 'Job is queued and ready to start.';
      case 'running': return 'Scraping is running.';
      case 'paused': return 'Paused safely at the latest durable checkpoint.';
      case 'paused_requires_action': return 'Manual browser action is required.';
      case 'paused_limit_reached': return 'The configured safety limit was reached.';
      case 'interrupted': return 'The application stopped before this job finished.';
      case 'partial': return 'Some units or enrichments remain unresolved.';
      case 'completed': return 'Completed successfully.';
      case 'failed': return 'The job failed before completion.';
      case 'cancelled': return 'The job was cancelled.';
      default: return 'Ready';
    }
  }
</script>

<Card.Root class="lg:col-span-5 flex flex-col justify-between" data-testid="google-maps-job-panel">
  <Card.Header class="pb-3 border-b border-border flex flex-row items-center justify-between">
    <Card.Title class="text-base font-bold flex items-center gap-2">
      <Terminal class="size-4 text-emerald-600" /> Execution Logs
    </Card.Title>
    <Badge variant={statusVariant(status)} class="text-[10px]">{statusLabel(status)}</Badge>
  </Card.Header>
  <Card.Content class="p-5 flex-1 flex flex-col gap-4">
    <div class="flex flex-col gap-1.5">
      <div class="flex items-center justify-between gap-3 text-xs">
        <span class="font-medium text-foreground">{statusMessage}</span>
        <span class="font-bold text-emerald-600">{Math.round(percentage)}%</span>
      </div>
      <Progress value={percentage} max={100} class="h-2" />
    </div>

    {#if job}
      <div class="grid grid-cols-3 gap-2 text-center text-[11px]">
        <div class="rounded-md border border-border bg-muted/20 p-2">
          <div class="font-bold text-foreground">{job.businesses_discovered}</div>
          <div class="text-muted-foreground">Discovered</div>
        </div>
        <div class="rounded-md border border-border bg-muted/20 p-2">
          <div class="font-bold text-foreground">{job.businesses_qualified}</div>
          <div class="text-muted-foreground">Qualified</div>
        </div>
        <div class="rounded-md border border-border bg-muted/20 p-2">
          <div class="font-bold text-foreground">{job.leads_created}</div>
          <div class="text-muted-foreground">Leads</div>
        </div>
      </div>
    {/if}

    {#if status === 'paused_requires_action'}
      <div class="flex flex-col gap-2 rounded-lg border border-amber-300 bg-amber-50 p-3 text-xs text-amber-950" role="alert">
        <div class="flex items-center gap-2 font-semibold">
          <AlertTriangle class="size-4" /> Complete the CAPTCHA in the opened CloakBrowser window
        </div>
        <p>
          Use the same persistent browser profile. Do not close that window; complete the Google Maps check, then resume this exact job.
        </p>
        {#if job?.blocked_url}
          <p class="break-all font-mono text-[10px]">{job.blocked_url}</p>
        {/if}
      </div>
    {/if}

    {#if job}
      <div class="flex flex-wrap gap-2" aria-label="Job actions">
        {#if status === 'running'}
          <Button variant="outline" size="sm" onclick={onPause} disabled={actionPending}>
            <Pause class="size-4" /> Pause
          </Button>
          <Button variant="destructive" size="sm" onclick={onCancel} disabled={actionPending}>
            <Square class="size-4" /> Cancel
          </Button>
        {:else if status === 'paused_requires_action'}
          <Button size="sm" onclick={onResume} disabled={actionPending}>
            <Play class="size-4" /> I completed browser check — Resume
          </Button>
          <Button variant="destructive" size="sm" onclick={onCancel} disabled={actionPending}>Cancel</Button>
        {:else if status === 'paused' || status === 'interrupted' || status === 'paused_limit_reached' || status === 'queued'}
          <Button size="sm" onclick={onResume} disabled={actionPending}>
            <Play class="size-4" /> Resume
          </Button>
          <Button variant="destructive" size="sm" onclick={onCancel} disabled={actionPending}>Cancel</Button>
        {:else if status === 'partial'}
          <Button variant="outline" size="sm" onclick={onRetryFailed} disabled={actionPending}>
            <RefreshCw class="size-4" /> Retry Failed Units
          </Button>
          <Button variant="outline" size="sm" onclick={onRetryEmail} disabled={actionPending}>
            <RefreshCw class="size-4" /> Retry Email Enrichment
          </Button>
        {:else if status === 'failed' && job.failed_units > 0}
          <Button variant="outline" size="sm" onclick={onRetryFailed} disabled={actionPending}>
            <RefreshCw class="size-4" /> Retry Failed Units
          </Button>
        {/if}
      </div>
    {/if}

    <div class="flex-1 min-h-[260px] max-h-[460px] p-3 rounded-lg bg-slate-950 text-slate-100 font-mono text-[11px] overflow-y-auto flex flex-col gap-1 border border-slate-800">
      {#if logs.length === 0}
        <div class="text-slate-500 italic">Console output logs will stream here...</div>
      {:else}
        {#each logs as log, index (index)}
          <div class="leading-relaxed border-b border-slate-900/60 pb-0.5">{log}</div>
        {/each}
      {/if}
    </div>
  </Card.Content>
</Card.Root>
