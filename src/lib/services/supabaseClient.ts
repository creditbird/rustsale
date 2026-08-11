import { createClient, type SupabaseClient } from '@supabase/supabase-js';

const SUPABASE_URL_KEY = 'rustsale_supabase_url';
const SUPABASE_ANON_KEY = 'rustsale_supabase_anon_key';

// Default fallback configuration (can be updated by user in settings)
const DEFAULT_URL = import.meta.env.VITE_SUPABASE_URL || 'https://xyzcompany.supabase.co';
const DEFAULT_ANON_KEY = import.meta.env.VITE_SUPABASE_ANON_KEY || 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.dummy';

let clientInstance: SupabaseClient | null = null;

export function getSupabaseConfig() {
  if (typeof window === 'undefined') {
    return { url: DEFAULT_URL, anonKey: DEFAULT_ANON_KEY };
  }
  const url = localStorage.getItem(SUPABASE_URL_KEY) || DEFAULT_URL;
  const anonKey = localStorage.getItem(SUPABASE_ANON_KEY) || DEFAULT_ANON_KEY;
  return { url, anonKey };
}

export function setSupabaseConfig(url: string, anonKey: string) {
  if (typeof window !== 'undefined') {
    localStorage.setItem(SUPABASE_URL_KEY, url);
    localStorage.setItem(SUPABASE_ANON_KEY, anonKey);
    clientInstance = null; // reset client to re-initialize
  }
}

export function getSupabaseClient(): SupabaseClient {
  if (!clientInstance) {
    const { url, anonKey } = getSupabaseConfig();
    clientInstance = createClient(url, anonKey, {
      auth: {
        persistSession: true,
        autoRefreshToken: true
      }
    });
  }
  return clientInstance;
}
