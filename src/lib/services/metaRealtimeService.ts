import { getSupabaseClient } from '$lib/services/supabaseClient';
import {
  syncFacebookConversations,
  ingestFacebookWebhookEvents,
} from '$lib/services/conversationService';
import type { RealtimeChannel } from '@supabase/supabase-js';

let realtimeChannel: RealtimeChannel | null = null;
let debounceTimers = new Map<string, ReturnType<typeof setTimeout>>();
let inFlightSyncs = new Set<string>();
let pendingFollowupSyncs = new Set<string>();

/**
 * Consumes and ACKs pending webhook buffer records from Supabase directly into SQLite.
 */
async function consumePendingWebhookBuffer(pageId?: string): Promise<void> {
  try {
    const supabase = getSupabaseClient();
    let query = supabase
      .from('meta_webhook_buffer')
      .select('id, field, event_key, payload, page_id')
      .in('status', ['pending', 'processing'])
      .order('id', { ascending: true })
      .limit(50);

    if (pageId) {
      query = query.eq('page_id', pageId);
    }

    const { data: rows, error } = await query;
    if (error || !rows || rows.length === 0) return;

    const byPage = new Map<string, { ids: number[]; events: any[] }>();
    for (const r of rows) {
      const pId = r.page_id;
      if (!pId) continue;
      if (!byPage.has(pId)) {
        byPage.set(pId, { ids: [], events: [] });
      }
      const group = byPage.get(pId)!;
      group.ids.push(r.id);
      group.events.push(r.payload);
    }

    for (const [pId, group] of byPage.entries()) {
      try {
        await ingestFacebookWebhookEvents(pId, group.events);
        await supabase
          .from('meta_webhook_buffer')
          .update({ status: 'completed' })
          .in('id', group.ids);
      } catch (ingestErr) {
        console.warn(`[MetaRealtime] Failed ingesting buffered events for page ${pId}:`, ingestErr);
      }
    }
  } catch (err) {
    console.warn('[MetaRealtime] Error consuming pending webhook buffer:', err);
  }
}

/**
 * Executes idempotent, serialized sync for a specific page.
 * 1. Consumes & ACKs pending buffered webhook events directly into SQLite.
 * 2. Triggers Graph API reconciliation in the background.
 */
async function executeSerializedSync(pageId?: string): Promise<void> {
  const key = pageId || '__all__';

  if (inFlightSyncs.has(key)) {
    pendingFollowupSyncs.add(key);
    return;
  }

  inFlightSyncs.add(key);
  try {
    // 1. Consume pending webhook buffer directly
    await consumePendingWebhookBuffer(pageId);
    // 2. Run Graph API reconciliation
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
          const payload = event.payload as { page_id?: string; events?: any[] };
          const pageId = payload?.page_id;
          if (pageId && Array.isArray(payload?.events) && payload.events.length > 0) {
            void ingestFacebookWebhookEvents(pageId, payload.events).catch((err) => {
              console.warn('[MetaRealtime] Fast-path webhook ingestion failed:', err);
            });
          }
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
          const rawPayload = (payload.new as { payload?: any })?.payload;
          if (pageId && rawPayload) {
            void ingestFacebookWebhookEvents(pageId, [rawPayload]).catch(() => {});
          }
          schedulePageSync(pageId);
        }
      )
      .subscribe((status) => {
        if (status === 'SUBSCRIBED') {
          console.info('[MetaRealtime] Supabase Realtime subscribed successfully for Meta events');
          void consumePendingWebhookBuffer();
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
