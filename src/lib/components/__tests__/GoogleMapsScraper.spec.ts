/// <reference types="bun-types" />
import { afterEach, beforeEach, expect, mock, test } from 'bun:test';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';

import type {
  GoogleMapsJobEvent,
  GoogleMapsJobStatus,
  GoogleMapsRuntimeStatus,
  GoogleMapsScrapeJob
} from '$lib/services/googleMapsScraperService';

type TauriEventListener = (event: { payload: unknown }) => void;

const invocations: Array<{ command: string; args?: any }> = [];
const eventListeners = new Map<string, TauriEventListener>();
let jobs: GoogleMapsScrapeJob[] = [];
let runtimeStatus: GoogleMapsRuntimeStatus;
let runtimePromise: Promise<GoogleMapsRuntimeStatus>;

function job(
  status: GoogleMapsJobStatus,
  overrides: Partial<GoogleMapsScrapeJob> = {}
): GoogleMapsScrapeJob {
  return {
    id: 'job-1',
    mode: 'keyword',
    keyword: 'cafe',
    location: 'Quận 1',
    bbox: null,
    step_km: null,
    headless: true,
    profile_dir: '/profiles/job-1',
    blocked_url: null,
    status,
    stage: 'scraping',
    total_units: 10,
    processed_units: 3,
    failed_units: 0,
    businesses_discovered: 12,
    businesses_qualified: 5,
    leads_created: 4,
    last_error_kind: null,
    last_error_message: null,
    action_required: null,
    event_sequence: 10,
    version: 1,
    run_generation: 1,
    created_at: '2026-07-26T00:00:00Z',
    started_at: '2026-07-26T00:00:01Z',
    updated_at: '2026-07-26T00:00:10Z',
    completed_at: null,
    ...overrides
  };
}

function updateJob(jobId: string, status: GoogleMapsJobStatus): GoogleMapsScrapeJob {
  const index = jobs.findIndex((candidate) => candidate.id === jobId);
  if (index < 0) throw new Error(`Unknown job ${jobId}`);
  jobs[index] = {
    ...jobs[index],
    status,
    event_sequence: jobs[index].event_sequence + 1,
    updated_at: '2026-07-26T00:00:20Z'
  };
  return jobs[index];
}

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string, args?: any) => {
    invocations.push({ command, args });
    switch (command) {
      case 'get_preset_provinces':
        return {
          'Thành phố Hồ Chí Minh': {
            minLat: 10.35,
            maxLat: 11.15,
            minLng: 106.3,
            maxLng: 107.0
          }
        };
      case 'prepare_google_maps_runtime':
        return runtimePromise;
      case 'get_google_maps_runtime_status':
        return { ...runtimeStatus };
      case 'list_google_maps_scrape_jobs':
        return jobs.map((candidate) => ({ ...candidate }));
      case 'get_google_maps_scrape_job': {
        const found = jobs.find((candidate) => candidate.id === args?.jobId);
        if (!found) throw new Error(`Unknown job ${args?.jobId}`);
        return { ...found };
      }
      case 'create_google_maps_scrape_job': {
        const request = args?.request;
        const created = job('queued', {
          id: 'job-created',
          mode: request.mode,
          keyword: request.keyword,
          location: request.location,
          bbox: request.bbox,
          step_km: request.step_km,
          headless: request.headless,
          total_units: request.mode === 'grid' ? 36 : Math.max(1, request.districts.length)
        });
        jobs = [created, ...jobs];
        return { ...created };
      }
      case 'pause_google_maps_scrape_job':
        return { ...updateJob(args?.jobId, 'paused') };
      case 'resume_google_maps_scrape_job':
        return { ...updateJob(args?.jobId, 'running') };
      case 'cancel_google_maps_scrape_job':
        return { ...updateJob(args?.jobId, 'cancelled') };
      case 'retry_google_maps_failed_units':
      case 'retry_google_maps_email_enrichment':
        return { ...updateJob(args?.jobId, 'running') };
      case 'delete_google_maps_scrape_job':
        jobs = jobs.filter((candidate) => candidate.id !== args?.jobId);
        return true;
      default:
        return undefined;
    }
  }
}));

mock.module('@tauri-apps/api/event', () => ({
  listen: async (eventName: string, listener: TauriEventListener) => {
    eventListeners.set(eventName, listener);
    return () => eventListeners.delete(eventName);
  }
}));

import GoogleMapsScraper from '../GoogleMapsScraper.svelte';

function emitJobEvent(event: GoogleMapsJobEvent): void {
  const listener = eventListeners.get('google-maps-scrape-job-event');
  if (!listener) throw new Error('Google Maps job listener was not registered');
  listener({ payload: event });
}

async function switchToSimpleKeywordMode(): Promise<void> {
  await fireEvent.click(screen.getByRole('button', { name: 'Text Search (Simple)' }));
  await fireEvent.input(screen.getByLabelText('Search Keyword *'), {
    target: { value: 'cafe' }
  });
}

beforeEach(() => {
  invocations.length = 0;
  eventListeners.clear();
  jobs = [];
  runtimeStatus = {
    installed: true,
    version: '132.0.0',
    bundledVersion: '132.0.0',
    tier: 'stable',
    platform: 'linux-x64',
    binaryPath: '/cache/cloak/chromium',
    cacheDir: '/cache/cloak',
    downloadUrl: null,
    wrapperVersion: '1.0.0'
  };
  runtimePromise = Promise.resolve({ ...runtimeStatus });
});

afterEach(() => {
  cleanup();
  eventListeners.clear();
});

test('preserves preset, geocode, and custom bounding-box controls', async () => {
  render(GoogleMapsScraper);

  await fireEvent.click(screen.getByRole('button', { name: 'Grid Crawl' }));

  expect(screen.getByText('Quick Select Province / City')).toBeTruthy();
  expect(screen.getByLabelText('Search Administrative Area')).toBeTruthy();
  expect(screen.getByLabelText('Min Lat')).toBeTruthy();
  expect(screen.getByLabelText('Max Lat')).toBeTruthy();
  expect(screen.getByLabelText('Min Lng')).toBeTruthy();
  expect(screen.getByLabelText('Max Lng')).toBeTruthy();
  expect(screen.getByLabelText('Grid Step Size')).toBeTruthy();
});

test('shows CloakBrowser runtime download state and installed version', async () => {
  runtimeStatus = { installed: false };
  let resolveRuntime!: (status: GoogleMapsRuntimeStatus) => void;
  runtimePromise = new Promise((resolve) => {
    resolveRuntime = resolve;
  });
  render(GoogleMapsScraper);

  await fireEvent.click(await screen.findByRole('button', { name: /Download.*CloakBrowser/i }));

  expect(await screen.findByText(/Downloading.*official CloakBrowser runtime/i)).toBeTruthy();

  resolveRuntime({
    installed: true,
    version: '132.0.0',
    platform: 'linux-x64',
    binaryPath: '/cache/cloak/chromium'
  });

  expect(await screen.findByText(/CloakBrowser 132\.0\.0 ready/i)).toBeTruthy();
});

test('restores installed CloakBrowser runtime status when the screen remounts', async () => {
  render(GoogleMapsScraper);

  expect(await screen.findByText(/CloakBrowser 132\.0\.0 ready/i)).toBeTruthy();
  expect(screen.getByRole('button', { name: 'Verify CloakBrowser' })).toBeTruthy();
  expect(invocations).toContainEqual({
    command: 'get_google_maps_runtime_status',
    args: undefined
  });
});

test('shows grid preflight estimate and recommended step when the cap is exceeded', async () => {
  render(GoogleMapsScraper);
  await fireEvent.click(screen.getByRole('button', { name: 'Grid Crawl' }));

  expect(await screen.findByText(/Estimated initial cells:/i)).toBeTruthy();

  await fireEvent.input(screen.getByLabelText('Min Lat'), { target: { value: '10' } });
  await fireEvent.input(screen.getByLabelText('Max Lat'), { target: { value: '20' } });
  await fireEvent.input(screen.getByLabelText('Min Lng'), { target: { value: '100' } });
  await fireEvent.input(screen.getByLabelText('Max Lng'), { target: { value: '110' } });
  await fireEvent.input(screen.getByLabelText('Grid Step Size'), { target: { value: '0.5' } });

  expect(await screen.findByText(/exceeds the 10,000-cell safety limit/i)).toBeTruthy();
  expect(screen.getByText(/Recommended minimum step:/i)).toBeTruthy();
});

test('creates then resumes a durable job instead of invoking the legacy scraper', async () => {
  render(GoogleMapsScraper);
  await switchToSimpleKeywordMode();

  await fireEvent.click(screen.getByRole('button', { name: 'Start Scrape Campaign' }));

  await waitFor(() => {
    expect(invocations.some(({ command }) => command === 'create_google_maps_scrape_job')).toBe(true);
    expect(invocations.some(({ command }) => command === 'resume_google_maps_scrape_job')).toBe(true);
  });
  expect(invocations.some(({ command }) => command === 'start_google_maps_scrape')).toBe(false);
});

test('maps Pause, Resume, and Cancel to the active durable job', async () => {
  jobs = [job('running')];
  render(GoogleMapsScraper);

  await fireEvent.click(await screen.findByRole('button', { name: 'Pause' }));
  await fireEvent.click(await screen.findByRole('button', { name: 'Resume' }));
  await fireEvent.click(await screen.findByRole('button', { name: 'Cancel' }));

  expect(invocations).toContainEqual({
    command: 'pause_google_maps_scrape_job',
    args: { jobId: 'job-1' }
  });
  expect(invocations).toContainEqual({
    command: 'resume_google_maps_scrape_job',
    args: { jobId: 'job-1' }
  });
  expect(invocations).toContainEqual({
    command: 'cancel_google_maps_scrape_job',
    args: { jobId: 'job-1' }
  });
});

test('restores an interrupted persisted job and exposes Resume after reload', async () => {
  jobs = [job('interrupted', { last_error_message: 'Application restarted during scrape' })];
  render(GoogleMapsScraper);

  expect(await screen.findByText('Interrupted')).toBeTruthy();
  expect(screen.getByText('Application restarted during scrape')).toBeTruthy();
  expect(screen.getByRole('button', { name: 'Resume' })).toBeTruthy();
  expect(invocations.some(({ command }) => command === 'list_google_maps_scrape_jobs')).toBe(true);
});

test('shows headed-browser instructions for CAPTCHA action-required state', async () => {
  jobs = [
    job('paused_requires_action', {
      blocked_url: 'https://www.google.com/maps',
      action_required: 'captcha',
      last_error_kind: 'captcha',
      last_error_message: 'Google Maps requires manual verification'
    })
  ];
  render(GoogleMapsScraper);

  expect(await screen.findByText(/Complete the CAPTCHA in the opened CloakBrowser window/i)).toBeTruthy();
  expect(screen.getByText(/same persistent browser profile/i)).toBeTruthy();
  expect(screen.getByRole('button', { name: /I completed.*Resume/i })).toBeTruthy();
});

test('renders partial as unresolved work, never as completed', async () => {
  jobs = [
    job('partial', {
      failed_units: 2,
      last_error_message: 'Two website enrichments remain unresolved',
      completed_at: '2026-07-26T00:01:00Z'
    })
  ];
  render(GoogleMapsScraper);

  expect(await screen.findByText('Partial')).toBeTruthy();
  expect(screen.getByText('Two website enrichments remain unresolved')).toBeTruthy();
  expect(screen.queryByText(/Completed successfully/i)).toBeNull();
  expect(screen.getByRole('button', { name: 'Retry Failed Units' })).toBeTruthy();
  expect(screen.getByRole('button', { name: 'Retry Email Enrichment' })).toBeTruthy();
});

test('exposes failed-unit retry for a failed persisted job', async () => {
  jobs = [
    job('failed', {
      failed_units: 1,
      last_error_kind: 'selector_changed',
      last_error_message: 'Google Maps rendered a blank page'
    })
  ];
  render(GoogleMapsScraper);

  await fireEvent.click(await screen.findByRole('button', { name: 'Retry Failed Units' }));

  expect(invocations).toContainEqual({
    command: 'retry_google_maps_failed_units',
    args: { jobId: 'job-1' }
  });
});

test('ignores other-job and stale events while applying the next active-job event', async () => {
  jobs = [job('running')];
  render(GoogleMapsScraper);
  expect(await screen.findByText('Running')).toBeTruthy();

  emitJobEvent({
    job_id: 'job-2',
    sequence: 11,
    timestamp: '2026-07-26T00:00:11Z',
    status: 'completed',
    stage: 'cleanup',
    processed_units: 10,
    total_units: 10,
    failed_units: 0,
    businesses_discovered: 12,
    businesses_qualified: 5,
    leads_created: 4,
    percentage: 100,
    message: 'wrong job completed',
    error: null,
    action_required: null
  });
  emitJobEvent({
    job_id: 'job-1',
    sequence: 9,
    timestamp: '2026-07-26T00:00:09Z',
    status: 'completed',
    stage: 'cleanup',
    processed_units: 10,
    total_units: 10,
    failed_units: 0,
    businesses_discovered: 12,
    businesses_qualified: 5,
    leads_created: 4,
    percentage: 100,
    message: 'stale completion',
    error: null,
    action_required: null
  });

  expect(screen.queryByText('wrong job completed')).toBeNull();
  expect(screen.queryByText('stale completion')).toBeNull();

  emitJobEvent({
    job_id: 'job-1',
    sequence: 11,
    timestamp: '2026-07-26T00:00:11Z',
    status: 'paused',
    stage: 'scraping',
    processed_units: 4,
    total_units: 10,
    failed_units: 0,
    businesses_discovered: 13,
    businesses_qualified: 6,
    leads_created: 4,
    percentage: 40,
    message: 'Paused safely after the current checkpoint',
    error: null,
    action_required: null
  });

  expect(await screen.findByText('Paused safely after the current checkpoint')).toBeTruthy();
  expect(screen.getByText('Paused')).toBeTruthy();
});

test('updates live scraper counters from the accepted job event', async () => {
  jobs = [
    job('running', {
      businesses_discovered: 0,
      businesses_qualified: 0,
      leads_created: 0
    })
  ];
  render(GoogleMapsScraper);

  expect(await screen.findByText('Running')).toBeTruthy();
  expect(screen.getByText('Discovered').previousElementSibling?.textContent).toBe('0');
  expect(screen.getByText('Qualified').previousElementSibling?.textContent).toBe('0');
  expect(screen.getByText('Leads').previousElementSibling?.textContent).toBe('0');

  emitJobEvent({
    job_id: 'job-1',
    sequence: 11,
    timestamp: '2026-07-29T00:00:11Z',
    status: 'running',
    stage: 'scraping',
    processed_units: 4,
    total_units: 10,
    failed_units: 1,
    businesses_discovered: 12,
    businesses_qualified: 5,
    leads_created: 4,
    percentage: 40,
    message: 'Completed place checkpoint',
    error: null,
    action_required: null
  });

  await waitFor(() => {
    expect(screen.getByText('Discovered').previousElementSibling?.textContent).toBe('12');
    expect(screen.getByText('Qualified').previousElementSibling?.textContent).toBe('5');
    expect(screen.getByText('Leads').previousElementSibling?.textContent).toBe('4');
  });
});
