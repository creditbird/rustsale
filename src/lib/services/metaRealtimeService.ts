import { getSupabaseClient } from '$lib/services/supabaseClient';
import { syncFacebookConversations } from '$lib/services/conversationService';
import type { RealtimeChannel } from '@supabase/supabase-js';

let realtimeChannel: RealtimeChannel | null = null;
let debounceTimers = new Map<string, ReturnType<typeof setTimeout>>();

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
    syncFacebookConversations(pageId).catch((err) => {
      console.warn('[MetaRealtime] Background page sync failed:', err);
    });
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
}
