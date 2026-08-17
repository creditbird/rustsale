<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import AdministrativeAreaSelector from '$lib/components/AdministrativeAreaSelector.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import * as Card from '$lib/components/ui/card';
  import * as Select from '$lib/components/ui/select';
  import GoogleMapsJobHistory from '$lib/components/GoogleMapsJobHistory.svelte';
  import GoogleMapsJobPanel from '$lib/components/GoogleMapsJobPanel.svelte';
  import GoogleMapsRuntimePanel from '$lib/components/GoogleMapsRuntimePanel.svelte';
  import type {
    AdministrativeAreaSummary,
    AdministrativeCountry
  } from '$lib/services/administrativeCatalogService';
  import {
    cancelGoogleMapsScrapeJob,
    createGoogleMapsScrapeJob,
    deleteGoogleMapsScrapeJob,
    getGoogleMapsRuntimeStatus,
    listGoogleMapsScrapeJobs,
    pauseGoogleMapsScrapeJob,
    prepareGoogleMapsRuntime,
    reduceGoogleMapsJobEvent,
    resumeGoogleMapsScrapeJob,
    retryGoogleMapsEmailEnrichment,
    retryGoogleMapsFailedUnits,
    subscribeToGoogleMapsJobEvents,
    viewStateFromJob,
    type GoogleMapsJobEvent,
    type GoogleMapsJobStatus,
    type GoogleMapsJobViewState,
    type GoogleMapsRuntimeStatus,
    type GoogleMapsScrapeJob
  } from '$lib/services/googleMapsScraperService';
  import { Search, MapPin, Play, Building, Grid, Locate } from 'lucide-svelte';
  import { m } from '$lib/i18n';

  let mode = $state<'keyword' | 'grid'>('keyword');
  let keyword = $state('');
  let location = $state('');

  let minLat = $state(10.7);
  let maxLat = $state(10.8);
  let minLng = $state(106.6);
  let maxLng = $state(106.7);
  let stepKm = $state(2.0);

  let geocodeQuery = $state('');
  let isGeocoding = $state(false);

  let deepScan = $state(true);
  let selectedCountryCode = $state('VN');
  let selectedCountryName = $state('Vietnam');
  let selectedAreas = $state<AdministrativeAreaSummary[]>([]);

  function handleCountryChange(country: AdministrativeCountry) {
    selectedCountryName = country.name;
  }

  async function detectCoordinates() {
    if (!geocodeQuery.trim()) {
      appendLog("Error: Please enter a region or address to search.");
      return;
    }
    isGeocoding = true;
    appendLog(`Geocoding coordinates for: "${geocodeQuery}"...`);
    try {
      const url = `https://nominatim.openstreetmap.org/search?q=${encodeURIComponent(geocodeQuery + ", Vietnam")}&format=json&limit=1`;
      const response = await fetch(url, { headers: { 'User-Agent': 'RustSale-CRM-v2-Client' } });
      if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
      const data = await response.json();
      if (data && data.length > 0) {
        const bbox = data[0].boundingbox;
        minLat = parseFloat(bbox[0]);
        maxLat = parseFloat(bbox[1]);
        minLng = parseFloat(bbox[2]);
        maxLng = parseFloat(bbox[3]);
        appendLog(`Successfully detected coordinates for "${data[0].display_name}": Lat: ${minLat}-${maxLat}, Lng: ${minLng}-${maxLng}`);
      } else {
        appendLog(`Error: Could not find coordinates for "${geocodeQuery}".`);
      }
    } catch (err) {
      appendLog(`Geocoding failed: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      isGeocoding = false;
    }
  }

  let logs = $state<string[]>([]);
  let headless = $state(true);
  let presetProvinces = $state<Record<string, { minLat: number; maxLat: number; minLng: number; maxLng: number }>>({});
  let selectedPresetProvince = $state('');
  let runtime = $state<GoogleMapsRuntimeStatus | null>(null);
  let runtimePreparing = $state(false);
  let runtimeError = $state<string | null>(null);
  let jobs = $state<GoogleMapsScrapeJob[]>([]);
  let activeJob = $state<GoogleMapsScrapeJob | null>(null);
  let activeView = $state<GoogleMapsJobViewState | null>(null);
  let actionPending = $state(false);
  let eventCleanup: (() => void) | null = null;

  let isScraping = $derived(activeJob !== null && isOpenStatus(activeJob.status));
  let gridEstimate = $derived.by(() => estimateGrid(
    Number(minLat),
    Number(maxLat),
    Number(minLng),
    Number(maxLng),
    Number(stepKm)
  ));

  function appendLog(msg: string) {
    const timestamp = new Date().toLocaleTimeString();
    logs = [...logs, `[${timestamp}] ${msg}`];
  }

  function isOpenStatus(status: GoogleMapsJobStatus): boolean {
    return !['partial', 'completed', 'failed', 'cancelled'].includes(status);
  }

  function estimateGrid(
    latMin: number,
    latMax: number,
    lngMin: number,
    lngMax: number,
    step: number
  ): { initialCells: number; recommendedStepKm: number | null; error: string | null } {
    const values = [latMin, latMax, lngMin, lngMax, step];
    if (values.some((value) => !Number.isFinite(value))) {
      return { initialCells: 0, recommendedStepKm: null, error: 'Coordinates and step must be finite numbers.' };
    }
    if (latMin < -90 || latMax > 90 || lngMin < -180 || lngMax > 180) {
      return { initialCells: 0, recommendedStepKm: null, error: 'Latitude or longitude is outside the supported range.' };
    }
    if (latMin >= latMax || lngMin >= lngMax) {
      return { initialCells: 0, recommendedStepKm: null, error: 'Bounding-box minimums must be less than maximums.' };
    }
    if (step <= 0) {
      return { initialCells: 0, recommendedStepKm: null, error: 'Grid step must be greater than zero.' };
    }

    const earthRadiusKm = 6_371.0088;
    const heightKm = Math.abs((latMax - latMin) * Math.PI / 180) * earthRadiusKm;
    const nearestEquatorLat = latMin <= 0 && latMax >= 0
      ? 0
      : Math.abs(latMin) < Math.abs(latMax) ? latMin : latMax;
    const widthKm = Math.abs((lngMax - lngMin) * Math.PI / 180)
      * earthRadiusKm
      * Math.max(1e-9, Math.abs(Math.cos(nearestEquatorLat * Math.PI / 180)));
    const countAtStep = (candidateStep: number) => {
      const rows = Math.max(1, Math.ceil(heightKm / candidateStep));
      const columns = Math.max(1, Math.ceil(widthKm / candidateStep));
      return rows * columns;
    };
    const initialCells = countAtStep(step);
    if (initialCells <= 10_000) {
      return { initialCells, recommendedStepKm: null, error: null };
    }

    let low = step;
    let high = Math.max(widthKm, heightKm, step);
    while (countAtStep(high) > 10_000) high *= 2;
    for (let iteration = 0; iteration < 64; iteration += 1) {
      const middle = (low + high) / 2;
      if (countAtStep(middle) > 10_000) low = middle;
      else high = middle;
    }
    return { initialCells, recommendedStepKm: high, error: null };
  }

  function syncJob(updatedJob: GoogleMapsScrapeJob, message?: string): void {
    const existing = jobs.findIndex((job) => job.id === updatedJob.id);
    jobs = existing >= 0
      ? jobs.map((job) => job.id === updatedJob.id ? updatedJob : job)
      : [updatedJob, ...jobs];
    activeJob = updatedJob;
    const nextView = viewStateFromJob(updatedJob);
    activeView = message ? { ...nextView, message } : nextView;
  }

  async function prepareRuntime(): Promise<GoogleMapsRuntimeStatus | null> {
    runtimePreparing = true;
    runtimeError = null;
    appendLog('Preparing official CloakBrowser runtime...');
    try {
      const prepared = await prepareGoogleMapsRuntime();
      if (!prepared.installed) throw new Error('CloakBrowser runtime preparation did not produce an installed runtime.');
      runtime = prepared;
      appendLog(`CloakBrowser runtime ready${prepared.version ? `: ${prepared.version}` : ''}.`);
      return prepared;
    } catch (err) {
      runtimeError = err instanceof Error ? err.message : String(err);
      appendLog(`CloakBrowser runtime error: ${runtimeError}`);
      return null;
    } finally {
      runtimePreparing = false;
    }
  }

  async function startScrape() {
    if (!keyword.trim()) {
      appendLog("Error: Keyword is required.");
      return;
    }

    if (mode === 'grid' && (gridEstimate.error || gridEstimate.recommendedStepKm !== null)) {
      appendLog(`Error: ${gridEstimate.error ?? `Grid estimate exceeds 10,000 cells. Use at least ${gridEstimate.recommendedStepKm?.toFixed(2)} km.`}`);
      return;
    }

    let districtsVal: string[] = [];
    let locationVal = mode === 'keyword' ? location : null;
    if (mode === 'keyword' && deepScan) {
      locationVal = selectedCountryName;
      districtsVal = selectedAreas.map((area) => area.canonical_path);
    }

    logs = [];
    appendLog(`Starting Google Maps Scraper in ${mode} mode...`);

    try {
      const prepared = runtime?.installed ? runtime : await prepareRuntime();
      if (!prepared) return;
      const bbox: [number, number, number, number] | null = mode === 'grid'
        ? [Number(minLat), Number(maxLat), Number(minLng), Number(maxLng)]
        : null;
      const stepVal = mode === 'grid' ? Number(stepKm) : null;

      const created = await createGoogleMapsScrapeJob({
        keyword,
        mode,
        location: locationVal,
        bbox,
        step_km: stepVal,
        districts: districtsVal,
        headless
      });
      syncJob(created, 'Durable job created. Starting the first pending unit...');
      const started = await resumeGoogleMapsScrapeJob(created.id);
      syncJob(started, 'Scraping is running.');
      appendLog(`Durable job ${started.id} started.`);
    } catch (err) {
      appendLog(`Error: ${err}`);
    }
  }

  async function runJobAction(
    label: string,
    action: (jobId: string) => Promise<GoogleMapsScrapeJob>
  ): Promise<void> {
    if (!activeJob || actionPending) return;
    actionPending = true;
    try {
      appendLog(`${label} job ${activeJob.id}...`);
      const updated = await action(activeJob.id);
      syncJob(updated, updated.last_error_message ?? `${label} completed.`);
    } catch (err) {
      appendLog(`${label} failed: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      actionPending = false;
    }
  }

  const pauseActiveJob = () => runJobAction('Pausing', pauseGoogleMapsScrapeJob);
  const resumeActiveJob = () => runJobAction('Resuming', resumeGoogleMapsScrapeJob);
  const cancelActiveJob = () => runJobAction('Cancelling', cancelGoogleMapsScrapeJob);
  const retryFailedUnits = () => runJobAction('Retrying failed units for', retryGoogleMapsFailedUnits);
  const retryEmailEnrichment = () => runJobAction('Retrying email enrichment for', retryGoogleMapsEmailEnrichment);

  function handleJobEvent(event: GoogleMapsJobEvent): void {
    if (!activeJob || event.job_id !== activeJob.id) return;
    const nextView = reduceGoogleMapsJobEvent(activeView, event);
    if (nextView === activeView) return;
    activeView = nextView;
    const updatedJob: GoogleMapsScrapeJob = {
      ...activeJob,
      status: event.status,
      stage: event.stage,
      processed_units: event.processed_units,
      total_units: event.total_units,
      failed_units: event.failed_units,
      businesses_discovered: event.businesses_discovered,
      businesses_qualified: event.businesses_qualified,
      leads_created: event.leads_created,
      event_sequence: event.sequence,
      updated_at: event.timestamp,
      last_error_kind: event.error?.kind ?? null,
      last_error_message: event.error?.message ?? null,
      action_required: event.action_required
    };
    activeJob = updatedJob;
    jobs = jobs.map((job) => job.id === updatedJob.id ? updatedJob : job);
    appendLog(`[${event.status}] ${event.message}`);
  }

  function selectJob(job: GoogleMapsScrapeJob): void {
    activeJob = job;
    activeView = viewStateFromJob(job);
  }

  async function deleteJob(job: GoogleMapsScrapeJob): Promise<void> {
    try {
      await deleteGoogleMapsScrapeJob(job.id);
      jobs = jobs.filter((candidate) => candidate.id !== job.id);
      if (activeJob?.id === job.id) {
        activeJob = jobs[0] ?? null;
        activeView = activeJob ? viewStateFromJob(activeJob) : null;
      }
    } catch (err) {
      appendLog(`Delete failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  onMount(async () => {
    try {
      runtime = await getGoogleMapsRuntimeStatus();
      runtimeError = null;
    } catch (err) {
      runtimeError = err instanceof Error ? err.message : String(err);
      appendLog(`Error loading CloakBrowser runtime status: ${runtimeError}`);
    }

    try {
      eventCleanup = await subscribeToGoogleMapsJobEvents(handleJobEvent);
    } catch (err) {
      appendLog(`Error subscribing to durable job events: ${err}`);
    }

    try {
      presetProvinces = await invoke('get_preset_provinces');
    } catch (err) {
      appendLog(`Error loading preset provinces: ${err}`);
    }

    try {
      jobs = await listGoogleMapsScrapeJobs();
      const restored = jobs.find((job) => isOpenStatus(job.status)) ?? jobs[0] ?? null;
      if (restored) selectJob(restored);
    } catch (err) {
      appendLog(`Error restoring Google Maps jobs: ${err}`);
    }

  });

  onDestroy(() => {
    eventCleanup?.();
  });
</script>

<div class="flex flex-col gap-4">
  <!-- Top Header Bar -->
  <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
    <div class="flex flex-col">
      <h2 class="text-xl font-bold text-foreground tracking-tight">{m.tab_scraper_title()}</h2>
      <p class="text-xs text-muted-foreground mt-0.5">{m.tab_scraper_subtitle()}</p>
    </div>
    <div class="w-full max-w-xl">
      <GoogleMapsRuntimePanel
        {runtime}
        preparing={runtimePreparing}
        error={runtimeError}
        onPrepare={prepareRuntime}
      />
    </div>
  </div>

  <!-- Main Grid -->
  <div class="grid grid-cols-1 lg:grid-cols-12 gap-5">
    <!-- Left Form Configuration Panel -->
    <Card.Root class="lg:col-span-7 flex flex-col justify-between">
      <Card.Header class="pb-3 border-b border-border">
        <Card.Title class="text-base font-bold flex items-center gap-2">
          <Building class="size-4 text-emerald-600" />
          {m.scraper_search_config()}
        </Card.Title>
      </Card.Header>
      <Card.Content class="p-5 flex flex-col gap-4 text-xs">
        <!-- Mode Switcher Tabs -->
        <div class="flex items-center gap-1 p-1 bg-muted rounded-lg w-full">
          <button
            type="button"
            class="flex-1 py-1.5 px-3 rounded-md text-xs font-semibold flex items-center justify-center gap-2 transition-all cursor-pointer {mode === 'keyword' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
            onclick={() => mode = 'keyword'}
            disabled={isScraping}
          >
            <Search class="size-3.5" /> Keyword Search
          </button>
          <button
            type="button"
            class="flex-1 py-1.5 px-3 rounded-md text-xs font-semibold flex items-center justify-center gap-2 transition-all cursor-pointer {mode === 'grid' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
            onclick={() => mode = 'grid'}
            disabled={isScraping}
          >
            <Grid class="size-3.5" /> Grid Crawl
          </button>
        </div>

        <!-- Search Keyword -->
        <div class="flex flex-col gap-1">
          <label for="keyword-input" class="font-semibold text-foreground">{m.scraper_keywords_label()} *</label>
          <div class="relative">
            <Search class="size-4 absolute left-3 top-2.5 text-muted-foreground" />
            <Input id="keyword-input" bind:value={keyword} placeholder={m.scraper_keywords_placeholder()} class="pl-9 text-xs" disabled={isScraping} />
          </div>
        </div>

        {#if mode === 'keyword'}
          <!-- Location Targeting Mode Switcher -->
          <div class="flex flex-col gap-1.5">
            <span class="font-semibold text-foreground">Location Targeting Mode</span>
            <div class="flex items-center gap-1 p-1 bg-muted/60 rounded-md">
              <button
                type="button"
                class="flex-1 py-1 px-2.5 rounded text-xs font-medium transition-all cursor-pointer {!deepScan ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
                onclick={() => deepScan = false}
                disabled={isScraping}
              >
                Text Search (Simple)
              </button>
              <button
                type="button"
                class="flex-1 py-1 px-2.5 rounded text-xs font-medium transition-all cursor-pointer {deepScan ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
                onclick={() => deepScan = true}
                disabled={isScraping}
              >
                Select Administrative Areas (Deep Scan)
              </button>
            </div>
          </div>

          {#if !deepScan}
            <!-- Simple Location Input -->
            <div class="flex flex-col gap-1">
              <label for="location-input" class="font-semibold text-foreground">Location / Area</label>
              <div class="relative">
                <MapPin class="size-4 absolute left-3 top-2.5 text-muted-foreground" />
                <Input id="location-input" bind:value={location} placeholder="e.g. District 1, Ho Chi Minh City" class="pl-9 text-xs" disabled={isScraping} />
              </div>
              <p class="text-[11px] text-muted-foreground">Focuses search on a specific neighborhood or town name.</p>
            </div>
          {:else}
            <!-- Deep Scan Configurations -->
            <AdministrativeAreaSelector
              bind:selectedCountryCode
              bind:selectedAreas
              disabled={isScraping}
              onCountryChange={handleCountryChange}
            />
          {/if}

        {:else}
          <!-- Grid Mode Settings -->
          <div class="flex flex-col gap-3 border-t border-border pt-3">
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <div class="flex flex-col gap-1">
                <label for="preset-select" class="font-semibold text-foreground">Quick Select Province / City</label>
                <Select.Root
                  type="single"
                  bind:value={selectedPresetProvince}
                  disabled={isScraping}
                  onValueChange={(val) => {
                    if (val && presetProvinces[val]) {
                      const coords = presetProvinces[val];
                      minLat = coords.minLat;
                      maxLat = coords.maxLat;
                      minLng = coords.minLng;
                      maxLng = coords.maxLng;
                      appendLog(`Applied preset coords for ${val}`);
                    }
                  }}
                >
                  <Select.Trigger class="w-full h-9 text-xs">
                    {selectedPresetProvince || '-- Select Province / City --'}
                  </Select.Trigger>
                  <Select.Content>
                    {#each Object.keys(presetProvinces) as prov (prov)}
                      <Select.Item value={prov}>{prov}</Select.Item>
                    {/each}
                  </Select.Content>
                </Select.Root>
              </div>

              <div class="flex flex-col gap-1">
                <label for="geocode-query" class="font-semibold text-foreground">Search Administrative Area</label>
                <div class="flex gap-2">
                  <Input id="geocode-query" bind:value={geocodeQuery} placeholder="e.g. District 1, Ho Chi Minh City" class="text-xs h-9" disabled={isScraping || isGeocoding} />
                  <Button variant="outline" size="sm" onclick={detectCoordinates} disabled={isScraping || isGeocoding} class="h-9 px-3 gap-1 shrink-0">
                    <Locate class="size-3.5" /> {isGeocoding ? 'Detecting...' : 'Detect'}
                  </Button>
                </div>
              </div>
            </div>

            <div class="flex flex-col gap-1.5">
              <span class="font-semibold text-foreground">Bounding Box Coordinates (Lat/Lng)</span>
              <div class="grid grid-cols-2 sm:grid-cols-4 gap-2">
                <div class="flex flex-col gap-1">
                  <label for="min-lat" class="text-[10px] text-muted-foreground">Min Lat</label>
                  <Input id="min-lat" type="number" step="0.0001" bind:value={minLat} disabled={isScraping} class="h-8 text-xs" />
                </div>
                <div class="flex flex-col gap-1">
                  <label for="max-lat" class="text-[10px] text-muted-foreground">Max Lat</label>
                  <Input id="max-lat" type="number" step="0.0001" bind:value={maxLat} disabled={isScraping} class="h-8 text-xs" />
                </div>
                <div class="flex flex-col gap-1">
                  <label for="min-lng" class="text-[10px] text-muted-foreground">Min Lng</label>
                  <Input id="min-lng" type="number" step="0.0001" bind:value={minLng} disabled={isScraping} class="h-8 text-xs" />
                </div>
                <div class="flex flex-col gap-1">
                  <label for="max-lng" class="text-[10px] text-muted-foreground">Max Lng</label>
                  <Input id="max-lng" type="number" step="0.0001" bind:value={maxLng} disabled={isScraping} class="h-8 text-xs" />
                </div>
              </div>
            </div>

            <div class="flex flex-col gap-1">
              <div class="flex justify-between items-center text-xs">
                <label for="step-km" class="font-semibold text-foreground">Grid Step Size</label>
                <span class="font-bold text-emerald-600">{stepKm.toFixed(1)} km</span>
              </div>
              <input id="step-km" type="range" min="0.5" max="10.0" step="0.5" bind:value={stepKm} disabled={isScraping} class="w-full accent-emerald-600 cursor-pointer" />
              <p class="text-[11px] text-muted-foreground">Smaller steps increase accuracy but require more search grid points.</p>
              <div class="flex flex-col gap-1 rounded-md border border-border bg-muted/20 p-2 text-[11px]" aria-live="polite">
                {#if gridEstimate.error}
                  <span class="font-medium text-destructive">{gridEstimate.error}</span>
                {:else}
                  <span class="font-medium text-foreground">
                    Estimated initial cells: {gridEstimate.initialCells.toLocaleString('en-US')}
                  </span>
                  {#if gridEstimate.recommendedStepKm !== null}
                    <span class="text-destructive">
                      Estimate exceeds the 10,000-cell safety limit. Recommended minimum step: {gridEstimate.recommendedStepKm.toFixed(2)} km.
                    </span>
                  {:else}
                    <span class="text-muted-foreground">Within the 10,000-cell backend safety limit.</span>
                  {/if}
                {/if}
              </div>
            </div>
          </div>
        {/if}

        <!-- Headless Option & Action Button -->
        <div class="flex items-center justify-between pt-2 border-t border-border">
          <div 
            class="flex items-center gap-2 text-xs font-medium text-muted-foreground cursor-pointer"
            onclick={() => !isScraping && (headless = !headless)}
            role="button"
            tabindex="0"
            onkeydown={(e) => e.key === 'Enter' && !isScraping && (headless = !headless)}
          >
            <Checkbox checked={headless} disabled={isScraping} />
            <span>Headless Mode (Hide browser)</span>
          </div>

          {#if isScraping && activeJob}
            <Badge variant="outline" class="text-[10px]">
              Active durable job: {activeJob.status.replaceAll('_', ' ')}
            </Badge>
          {:else}
            <Button size="sm" onclick={startScrape} disabled={!keyword.trim()} class="gap-2 bg-emerald-600 hover:bg-emerald-700 text-white">
              <Play class="size-4 fill-white" /> Start Scrape Campaign
            </Button>
          {/if}
        </div>
      </Card.Content>
    </Card.Root>

    <GoogleMapsJobPanel
      job={activeJob}
      view={activeView}
      {logs}
      {actionPending}
      onPause={pauseActiveJob}
      onResume={resumeActiveJob}
      onCancel={cancelActiveJob}
      onRetryFailed={retryFailedUnits}
      onRetryEmail={retryEmailEnrichment}
    />
  </div>

  {#if jobs.length > 0}
    <GoogleMapsJobHistory
      {jobs}
      activeJobId={activeJob?.id ?? null}
      onSelect={selectJob}
      onDelete={deleteJob}
    />
  {/if}
</div>
