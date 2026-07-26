import { beforeEach, describe, expect, mock, test } from 'bun:test';

const invocations: Array<{ command: string; args?: unknown }> = [];
let eventHandler: ((event: { payload: unknown }) => void) | undefined;
let unlistenCalls = 0;

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string, args?: unknown) => {
    invocations.push({ command, args });
    return { id: 'job-1' };
  }
}));

mock.module('@tauri-apps/api/event', () => ({
  listen: async (_event: string, handler: (event: { payload: unknown }) => void) => {
    eventHandler = handler;
    return () => {
      unlistenCalls += 1;
    };
  }
}));

import {
  createGoogleMapsScrapeJob,
  getGoogleMapsRuntimeStatus,
  pauseGoogleMapsScrapeJob,
  prepareGoogleMapsRuntime,
  reduceGoogleMapsJobEvent,
  subscribeToGoogleMapsJobEvents,
  type GoogleMapsJobEvent,
  type GoogleMapsJobStatus,
  type GoogleMapsJobViewState
} from '../googleMapsScraperService';

function state(
  jobId: string,
  sequence: number,
  status: GoogleMapsJobStatus = 'running'
): GoogleMapsJobViewState {
  return {
    jobId,
    sequence,
    status,
    stage: 'scraping',
    processedUnits: 1,
    totalUnits: 10,
    percentage: 10,
    message: 'running',
    error: null,
    actionRequired: null,
    timestamp: '2026-07-26T00:00:00Z'
  };
}

function event(
  jobId: string,
  sequence: number,
  status: GoogleMapsJobStatus = 'running'
): GoogleMapsJobEvent {
  return {
    job_id: jobId,
    sequence,
    timestamp: '2026-07-26T00:00:01Z',
    status,
    stage: 'scraping',
    processed_units: 2,
    total_units: 10,
    percentage: 20,
    message: 'updated',
    error: null,
    action_required: null
  };
}

beforeEach(() => {
  invocations.length = 0;
  eventHandler = undefined;
  unlistenCalls = 0;
});

describe('Google Maps scraper service', () => {
  test('ignores events from another job', () => {
    const current = state('a', 10);

    expect(reduceGoogleMapsJobEvent(current, event('b', 11))).toBe(current);
  });

  test('ignores stale event sequences', () => {
    const current = state('a', 20);

    expect(reduceGoogleMapsJobEvent(current, event('a', 19))).toBe(current);
  });

  test('ignores events after a terminal state', () => {
    const current = state('a', 20, 'completed');

    expect(reduceGoogleMapsJobEvent(current, event('a', 21))).toBe(current);
  });

  test('maps typed commands to exact backend command names and arguments', async () => {
    await getGoogleMapsRuntimeStatus();
    await prepareGoogleMapsRuntime();
    await createGoogleMapsScrapeJob({
      mode: 'keyword',
      keyword: 'cafe',
      location: 'HCM',
      bbox: null,
      step_km: null,
      districts: [],
      headless: true
    });
    await pauseGoogleMapsScrapeJob('job-1');

    expect(invocations).toEqual([
      { command: 'get_google_maps_runtime_status', args: undefined },
      { command: 'prepare_google_maps_runtime', args: undefined },
      {
        command: 'create_google_maps_scrape_job',
        args: {
          request: {
            mode: 'keyword',
            keyword: 'cafe',
            location: 'HCM',
            bbox: null,
            step_km: null,
            districts: [],
            headless: true
          }
        }
      },
      { command: 'pause_google_maps_scrape_job', args: { jobId: 'job-1' } }
    ]);
  });

  test('subscribes to job events and returns an idempotent cleanup function', async () => {
    const received: GoogleMapsJobEvent[] = [];
    const cleanup = await subscribeToGoogleMapsJobEvents((payload) => received.push(payload));
    eventHandler?.({ payload: event('job-1', 1) });

    cleanup();
    cleanup();

    expect(received).toHaveLength(1);
    expect(unlistenCalls).toBe(1);
  });
});
