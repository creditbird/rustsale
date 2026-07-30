import { beforeEach, describe, expect, mock, test } from 'bun:test';

let listenCallCount = 0;
let unlistenCallCount = 0;
let listenFailuresRemaining = 0;
let activeListenCount = 0;
let deferNextListen = false;
let resolveDeferredListen: (() => void) | null = null;

function createUnlisten(): () => void {
  activeListenCount += 1;
  let cleaned = false;
  return () => {
    if (cleaned) return;
    cleaned = true;
    activeListenCount -= 1;
    unlistenCallCount += 1;
  };
}

mock.module('@tauri-apps/api/core', () => ({
  invoke: async () => []
}));

mock.module('@tauri-apps/api/event', () => ({
  listen: async () => {
    listenCallCount += 1;
    if (listenFailuresRemaining > 0) {
      listenFailuresRemaining -= 1;
      throw new Error('transient listen failure');
    }
    if (deferNextListen) {
      deferNextListen = false;
      return new Promise<() => void>((resolve) => {
        resolveDeferredListen = () => resolve(createUnlisten());
      });
    }
    return createUnlisten();
  }
}));

import {
  getBridgePromise,
  getBridgeRefCount,
  handleScraperJobEvent,
  initScraperEventBridge,
  notifyLeadsChanged,
  onLeadsChanged,
  resetScraperJobEventBridgeState
} from '../crmService';
import type { GoogleMapsJobEvent } from '../googleMapsScraperService';

function mockJobEvent(overrides?: Partial<GoogleMapsJobEvent>): GoogleMapsJobEvent {
  return {
    job_id: 'job-test-1',
    sequence: 1,
    timestamp: '2026-07-30T12:00:00Z',
    status: 'running',
    stage: 'scraping',
    processed_units: 1,
    total_units: 10,
    failed_units: 0,
    businesses_discovered: 5,
    businesses_qualified: 2,
    leads_created: 0,
    percentage: 10,
    message: 'scraping unit 1',
    error: null,
    action_required: null,
    ...overrides
  };
}

beforeEach(() => {
  resetScraperJobEventBridgeState();
  listenCallCount = 0;
  unlistenCallCount = 0;
  listenFailuresRemaining = 0;
  activeListenCount = 0;
  deferNextListen = false;
  resolveDeferredListen = null;
});

describe('crmService real-time lead change & scraper bridge', () => {
  test('notifies subscribers when notifyLeadsChanged is invoked', () => {
    let callCount = 0;
    const unsub = onLeadsChanged(() => {
      callCount += 1;
    });

    notifyLeadsChanged();
    expect(callCount).toBe(1);

    unsub();
    notifyLeadsChanged();
    expect(callCount).toBe(1);
  });

  test('ref-counts initScraperEventBridge subscriptions correctly', async () => {
    const unsub1 = initScraperEventBridge();
    expect(getBridgeRefCount()).toBe(1);

    const unsub2 = initScraperEventBridge();
    expect(getBridgeRefCount()).toBe(2);

    const promise = getBridgePromise();
    await promise;

    // Unsubscribing consumer 1 should not destroy subscription for consumer 2
    unsub1();
    expect(getBridgeRefCount()).toBe(1);
    expect(unlistenCallCount).toBe(0);

    // Unsubscribing final consumer should clean up unlisten
    unsub2();
    expect(getBridgeRefCount()).toBe(0);
    expect(unlistenCallCount).toBe(1);
  });

  test('reuses a pending subscription when consumers unmount and remount', async () => {
    deferNextListen = true;

    const unsub1 = initScraperEventBridge();
    const pendingSubscription = getBridgePromise();
    expect(listenCallCount).toBe(1);

    unsub1();
    expect(getBridgeRefCount()).toBe(0);

    const unsub2 = initScraperEventBridge();
    const listenCallsAfterRemount = listenCallCount;

    resolveDeferredListen?.();
    await pendingSubscription;
    await getBridgePromise();

    expect(listenCallsAfterRemount).toBe(1);
    expect(activeListenCount).toBe(1);

    unsub2();
    expect(activeListenCount).toBe(0);
  });

  test('auto-re-establishes subscription when initial listen failed and recovery occurs', async () => {
    listenFailuresRemaining = 1;

    const unsub = initScraperEventBridge();
    expect(getBridgeRefCount()).toBe(1);

    const deadline = Date.now() + 1_000;
    while (listenCallCount < 2 && Date.now() < deadline) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }

    expect(listenCallCount).toBe(2);
    expect(getBridgeRefCount()).toBe(1);
    expect(getBridgePromise()).not.toBeNull();

    unsub();
    expect(unlistenCallCount).toBe(1);
  });

  test('cancels a scheduled reconnect when the final consumer unmounts', async () => {
    listenFailuresRemaining = 1;

    const unsub = initScraperEventBridge();
    const failedAttempt = getBridgePromise();
    await failedAttempt;

    unsub();
    await new Promise((resolve) => setTimeout(resolve, 150));

    expect(getBridgeRefCount()).toBe(0);
    expect(listenCallCount).toBe(1);
    expect(activeListenCount).toBe(0);
  });

  test('handleScraperJobEvent ignores progress events when leads_created has not increased', () => {
    let notifyCount = 0;
    const unsub = onLeadsChanged(() => {
      notifyCount += 1;
    });

    const handled1 = handleScraperJobEvent(mockJobEvent({ sequence: 1, leads_created: 0 }));
    expect(handled1).toBe(false);
    expect(notifyCount).toBe(0);

    const handled2 = handleScraperJobEvent(mockJobEvent({ sequence: 2, leads_created: 0 }));
    expect(handled2).toBe(false);
    expect(notifyCount).toBe(0);

    unsub();
  });

  test('handleScraperJobEvent notifies subscribers only when leads_created increases', () => {
    let notifyCount = 0;
    const unsub = onLeadsChanged(() => {
      notifyCount += 1;
    });

    handleScraperJobEvent(mockJobEvent({ sequence: 1, leads_created: 0 }));
    expect(notifyCount).toBe(0);

    const handledDelta = handleScraperJobEvent(mockJobEvent({ sequence: 2, leads_created: 3 }));
    expect(handledDelta).toBe(true);
    expect(notifyCount).toBe(1);

    const handledSame = handleScraperJobEvent(mockJobEvent({ sequence: 3, leads_created: 3 }));
    expect(handledSame).toBe(false);
    expect(notifyCount).toBe(1);

    const handledIncrease = handleScraperJobEvent(mockJobEvent({ sequence: 4, leads_created: 5 }));
    expect(handledIncrease).toBe(true);
    expect(notifyCount).toBe(2);

    unsub();
  });

  test('handleScraperJobEvent notifies subscribers when job enters terminal status', () => {
    let notifyCount = 0;
    const unsub = onLeadsChanged(() => {
      notifyCount += 1;
    });

    handleScraperJobEvent(mockJobEvent({ sequence: 1, leads_created: 5 }));
    expect(notifyCount).toBe(1);

    const handledCompleted = handleScraperJobEvent(
      mockJobEvent({ sequence: 2, status: 'completed', leads_created: 5 })
    );
    expect(handledCompleted).toBe(true);
    expect(notifyCount).toBe(2);

    unsub();
  });
});
