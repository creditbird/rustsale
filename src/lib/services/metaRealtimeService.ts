import { getSupabaseClient } from '$lib/services/supabaseClient';
import { syncFacebookConversations } from '$lib/services/conversationService';
import type { RealtimeChannel } from '@supabase/supabase-js';

let realtimeChannel: RealtimeChannel | null = null;
let debounceTimers = new Map<string, ReturnType<typeof setTimeout>>();
let inFlightSyncs = new Set<string>();
let pendingFollowupSyncs = new Set<string>();

/**
 * Executes idempotent, serialized sync for a specific page.
 * Prevents concurrent stampedes while guaranteeing that events arriving
 * during an active sync trigger exactly one followup sync.
 */
async function executeSerializedSync(pageId?: string): Promise<void> {
  const key = pageId || '__all__';

  if (inFlightSyncs.has(key)) {
    pendingFollowupSyncs.add(key);
    return;
  }

  inFlightSyncs.add(key);
  try {
    await syncFacebookConversations(pageId);
  } catch (err) {
    console.warn(`[MetaRealtime] Serialized sync failed for ${key}:`, err);
  } finally {
    inFlightSyncs.delete(key);
    if (pendingFollowupSyncs.has(key)) {
      pendingFollowupSyncs.delete(key);
      void executeSerializedSync(pageId);
    }
  }
}

/**
 * Trigger debounce-buffered sync for a specific page (or all pages if undefined).
 */
function schedulePageSync(pageId?: string) {
  const key = pageId || '__all__';
  const existing = debounceTimers.get(key);
  if (existing) {
    clearTimeout(existing);
  }

  const timer = setTimeout(() => {
    debounceTimers.delete(key);
    void executeSerializedSync(pageId);
  }, 100);

  debounceTimers.set(key, timer);
}

/**
 * Initialize Supabase Realtime Listener for Meta Webhook events.
 * Listens for both Broadcasts and Postgres CDC on `meta_webhook_buffer`.
 */
export function startMetaRealtimeListener(): () => void {
  if (typeof window === 'undefined') {
    return () => {};
  }

  if (realtimeChannel) {
    return stopMetaRealtimeListener;
  }

  try {
    const supabase = getSupabaseClient();
    realtimeChannel = supabase
      .channel('meta-realtime-channel')
      .on(
        'broadcast',
        { event: 'meta_message' },
        (event) => {
          const pageId = (event.payload as { page_id?: string })?.page_id;
          schedulePageSync(pageId);
        }
      )
      .on(
        'postgres_changes',
        {
          event: 'INSERT',
          schema: 'public',
          table: 'meta_webhook_buffer'
        },
        (payload) => {
          const pageId = (payload.new as { page_id?: string })?.page_id;
          schedulePageSync(pageId);
        }
      )
      .subscribe((status) => {
        if (status === 'SUBSCRIBED') {
          console.info('[MetaRealtime] Supabase Realtime subscribed successfully for Meta events');
        } else if (status === 'CLOSED' || status === 'CHANNEL_ERROR') {
          console.warn('[MetaRealtime] Realtime disconnected or error, scheduling recovery sync');
          schedulePageSync();
        }
      });
  } catch (err) {
    console.warn('[MetaRealtime] Failed to initialize Supabase Realtime listener:', err);
  }

  return stopMetaRealtimeListener;
}

/**
 * Unsubscribe and clean up Supabase Realtime Listener.
 */
export function stopMetaRealtimeListener(): void {
  if (realtimeChannel) {
    try {
      const supabase = getSupabaseClient();
      supabase.removeChannel(realtimeChannel);
    } catch {}
    realtimeChannel = null;
  }
  for (const timer of debounceTimers.values()) {
    clearTimeout(timer);
  }
  debounceTimers.clear();
  inFlightSyncs.clear();
  pendingFollowupSyncs.clear();
}
