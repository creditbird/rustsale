import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type GoogleMapsScrapeMode = 'keyword' | 'grid';
export type GoogleMapsJobStatus =
  | 'queued'
  | 'running'
  | 'paused'
  | 'paused_requires_action'
  | 'paused_limit_reached'
  | 'interrupted'
  | 'partial'
  | 'completed'
  | 'failed'
  | 'cancelled';
export type GoogleMapsScrapeStage =
  | 'planning'
  | 'installing_browser_runtime'
  | 'scraping'
  | 'email_enrichment'
  | 'importing'
  | 'cleanup';

export interface GoogleMapsRuntimeStatus {
  installed: boolean;
  version?: string | null;
  bundledVersion?: string | null;
  tier?: string | null;
  platform?: string | null;
  binaryPath?: string | null;
  cacheDir?: string | null;
  downloadUrl?: string | null;
  wrapperVersion?: string | null;
}

export interface CreateGoogleMapsScrapeJobRequest {
  mode: GoogleMapsScrapeMode;
  keyword: string;
  location: string | null;
  bbox: [number, number, number, number] | null;
  step_km: number | null;
  districts: string[];
  headless: boolean;
}

export interface GoogleMapsJobError {
  kind: string;
  message: string;
  retryable: boolean;
}

export interface GoogleMapsScrapeJob {
  id: string;
  mode: GoogleMapsScrapeMode;
  keyword: string;
  location: string | null;
  bbox: [number, number, number, number] | null;
  step_km: number | null;
  headless: boolean;
  profile_dir: string;
  blocked_url: string | null;
  status: GoogleMapsJobStatus;
  stage: GoogleMapsScrapeStage;
  total_units: number;
  processed_units: number;
  failed_units: number;
  businesses_discovered: number;
  businesses_qualified: number;
  leads_created: number;
  last_error_kind: string | null;
  last_error_message: string | null;
  action_required: string | null;
  event_sequence: number;
  version: number;
  run_generation: number;
  created_at: string;
  started_at: string | null;
  updated_at: string;
  completed_at: string | null;
}

export interface GoogleMapsJobEvent {
  job_id: string;
  sequence: number;
  timestamp: string;
  status: GoogleMapsJobStatus;
  stage: GoogleMapsScrapeStage;
  processed_units: number;
  total_units: number;
  percentage: number;
  message: string;
  error: GoogleMapsJobError | null;
  action_required: string | null;
}

export interface GoogleMapsJobViewState {
  jobId: string;
  sequence: number;
  timestamp: string;
  status: GoogleMapsJobStatus;
  stage: GoogleMapsScrapeStage;
  processedUnits: number;
  totalUnits: number;
  percentage: number;
  message: string;
  error: GoogleMapsJobError | null;
  actionRequired: string | null;
}

export function getGoogleMapsRuntimeStatus(): Promise<GoogleMapsRuntimeStatus> {
  return invoke<GoogleMapsRuntimeStatus>('get_google_maps_runtime_status');
}

export function prepareGoogleMapsRuntime(): Promise<GoogleMapsRuntimeStatus> {
  return invoke<GoogleMapsRuntimeStatus>('prepare_google_maps_runtime');
}

export function createGoogleMapsScrapeJob(
  request: CreateGoogleMapsScrapeJobRequest
): Promise<GoogleMapsScrapeJob> {
  return invoke<GoogleMapsScrapeJob>('create_google_maps_scrape_job', { request });
}

export function pauseGoogleMapsScrapeJob(jobId: string): Promise<GoogleMapsScrapeJob> {
  return invoke<GoogleMapsScrapeJob>('pause_google_maps_scrape_job', { jobId });
}

export function resumeGoogleMapsScrapeJob(jobId: string): Promise<GoogleMapsScrapeJob> {
  return invoke<GoogleMapsScrapeJob>('resume_google_maps_scrape_job', { jobId });
}

export function cancelGoogleMapsScrapeJob(jobId: string): Promise<GoogleMapsScrapeJob> {
  return invoke<GoogleMapsScrapeJob>('cancel_google_maps_scrape_job', { jobId });
}

export function retryGoogleMapsFailedUnits(jobId: string): Promise<GoogleMapsScrapeJob> {
  return invoke<GoogleMapsScrapeJob>('retry_google_maps_failed_units', { jobId });
}

export function retryGoogleMapsEmailEnrichment(jobId: string): Promise<GoogleMapsScrapeJob> {
  return invoke<GoogleMapsScrapeJob>('retry_google_maps_email_enrichment', { jobId });
}

export function getGoogleMapsScrapeJob(jobId: string): Promise<GoogleMapsScrapeJob> {
  return invoke<GoogleMapsScrapeJob>('get_google_maps_scrape_job', { jobId });
}

export function listGoogleMapsScrapeJobs(): Promise<GoogleMapsScrapeJob[]> {
  return invoke<GoogleMapsScrapeJob[]>('list_google_maps_scrape_jobs');
}

export function deleteGoogleMapsScrapeJob(jobId: string): Promise<boolean> {
  return invoke<boolean>('delete_google_maps_scrape_job', { jobId });
}

export async function subscribeToGoogleMapsJobEvents(
  onEvent: (event: GoogleMapsJobEvent) => void
): Promise<() => void> {
  const unlisten: UnlistenFn = await listen<GoogleMapsJobEvent>(
    'google-maps-scrape-job-event',
    ({ payload }) => onEvent(payload)
  );
  let cleaned = false;
  return () => {
    if (cleaned) return;
    cleaned = true;
    unlisten();
  };
}

export function reduceGoogleMapsJobEvent(
  current: GoogleMapsJobViewState | null,
  event: GoogleMapsJobEvent
): GoogleMapsJobViewState {
  if (current) {
    if (current.jobId !== event.job_id) return current;
    if (event.sequence <= current.sequence) return current;
    if (isTerminalStatus(current.status)) return current;
  }

  return {
    jobId: event.job_id,
    sequence: event.sequence,
    timestamp: event.timestamp,
    status: event.status,
    stage: event.stage,
    processedUnits: event.processed_units,
    totalUnits: event.total_units,
    percentage: event.percentage,
    message: event.message,
    error: event.error,
    actionRequired: event.action_required
  };
}

export function viewStateFromJob(job: GoogleMapsScrapeJob): GoogleMapsJobViewState {
  const percentage = job.total_units === 0 ? 0 : (job.processed_units / job.total_units) * 100;
  return {
    jobId: job.id,
    sequence: job.event_sequence,
    timestamp: job.updated_at,
    status: job.status,
    stage: job.stage,
    processedUnits: job.processed_units,
    totalUnits: job.total_units,
    percentage: Math.min(100, Math.max(0, percentage)),
    message: job.last_error_message ?? '',
    error:
      job.last_error_kind && job.last_error_message
        ? { kind: job.last_error_kind, message: job.last_error_message, retryable: false }
        : null,
    actionRequired: job.action_required
  };
}

function isTerminalStatus(status: GoogleMapsJobStatus): boolean {
  return ['partial', 'completed', 'failed', 'cancelled'].includes(status);
}
