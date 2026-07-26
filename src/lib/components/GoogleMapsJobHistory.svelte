<script lang="ts">
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import type { GoogleMapsScrapeJob } from '$lib/services/googleMapsScraperService';
  import { History, Trash2 } from 'lucide-svelte';

  interface Props {
    jobs: GoogleMapsScrapeJob[];
    activeJobId?: string | null;
    onSelect: (job: GoogleMapsScrapeJob) => void;
    onDelete: (job: GoogleMapsScrapeJob) => void | Promise<void>;
  }

  let { jobs, activeJobId = null, onSelect, onDelete }: Props = $props();

  function canDelete(job: GoogleMapsScrapeJob): boolean {
    return ['partial', 'completed', 'failed', 'cancelled'].includes(job.status);
  }
</script>

<Card.Root data-testid="google-maps-job-history">
  <Card.Header class="pb-3 border-b border-border">
    <Card.Title class="text-base font-bold flex items-center gap-2">
      <History class="size-4 text-emerald-600" /> Job History
    </Card.Title>
    <Card.Description>Persisted jobs remain available after application restarts.</Card.Description>
  </Card.Header>
  <Card.Content class="p-4">
    {#if jobs.length === 0}
      <p class="text-xs text-muted-foreground">No Google Maps scrape jobs have been created yet.</p>
    {:else}
      <div class="flex flex-col gap-2">
        {#each jobs as job (job.id)}
          <div class="flex flex-col gap-2 rounded-lg border border-border p-3 sm:flex-row sm:items-center sm:justify-between {job.id === activeJobId ? 'bg-muted/40' : ''}">
            <button type="button" class="flex min-w-0 flex-1 flex-col gap-1 text-left" onclick={() => onSelect(job)}>
              <div class="flex flex-wrap items-center gap-2">
                <span class="truncate text-sm font-semibold text-foreground">{job.keyword}</span>
                <Badge variant="outline">{job.status.replaceAll('_', ' ')}</Badge>
              </div>
              <span class="text-[11px] text-muted-foreground">
                {job.processed_units}/{job.total_units} units · {job.businesses_discovered} businesses · {job.leads_created} leads
              </span>
            </button>
            {#if canDelete(job)}
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={`Delete job ${job.keyword}`}
                onclick={() => onDelete(job)}
              >
                <Trash2 class="size-4" />
              </Button>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </Card.Content>
</Card.Root>
