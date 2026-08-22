// Supabase Edge Function: /meta-oauth
// Zero-secret OAuth bridge: the Meta App Secret lives ONLY on this server.
// POST { code } with the desktop user's Supabase Bearer JWT.
// 1. Exchanges the authorization code for a short-lived user token.
// 2. Exchanges it again for a Long-Lived User Token (~60 days).
// 3. Fetches ALL managed Pages via /me/accounts with full cursor pagination
//    (long-lived Page tokens).
// 4. Persists tenant <-> page ownership and reconciles stale mappings — but
//    ONLY after a complete, successful enumeration, so an incomplete Meta
//    response can never revoke valid access.
// 5. Returns tokens so the desktop app can store them in Stronghold/Keyring.
import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2.49.4";

const META_APP_ID = Deno.env.get("META_APP_ID") ?? "";
const META_APP_SECRET = Deno.env.get("META_APP_SECRET") ?? "";
const META_GRAPH_VERSION = Deno.env.get("META_GRAPH_VERSION") || "v26.0";
const SUPABASE_URL = Deno.env.get("SUPABASE_URL") ?? "";
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") ?? "";

const supabaseAdmin =
  SUPABASE_URL && SUPABASE_SERVICE_ROLE_KEY
    ? createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY)
    : null;

function logStructured(fields: Record<string, unknown>): void {
  // Never log codes, tokens, or secrets.
  console.log(JSON.stringify({ fn: "meta-oauth", ...fields }));
}

function json(body: unknown, status = 200): Response {
  // Token-bearing bodies must never be cached by any intermediary or WebView.
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json", "Cache-Control": "no-store" },
  });
}

function graph(path: string, params: Record<string, string>): string {
  const search = new URLSearchParams(params);
  return `https://graph.facebook.com/${META_GRAPH_VERSION}/${path}?${search.toString()}`;
}

async function jsonFetch<T>(url: string): Promise<T> {
  const resp = await fetch(url);
  const body = await resp.json();
  if (!resp.ok) {
    throw new Error(
      body?.error?.message ? `Meta API error: ${body.error.message}` : `Meta API HTTP ${resp.status}`,
    );
  }
  return body as T;
}

type PageRow = { id: string; name: string; access_token: string };

async function fetchAllManagedPages(longLivedToken: string): Promise<PageRow[]> {
  // Follow paging.next cursors until exhausted so >100-Page accounts are not
  // silently truncated.
  let pages: PageRow[] = [];
  let url: string | null = graph("me/accounts", {
    access_token: longLivedToken,
    fields: "id,name,access_token",
    limit: "100",
  });
  while (url) {
    const page: { data?: PageRow[]; paging?: { next?: string } } = await jsonFetch(url);
    pages = pages.concat(page.data ?? []);
    url = page.paging?.next ?? null;
  }
  return pages;
}

serve(async (req: Request) => {
  if (!supabaseAdmin || !META_APP_ID || !META_APP_SECRET) {
    logStructured({ event: "server_misconfigured" });
    return json({ error: "Server configuration error" }, 500);
  }

  // Browser/WebView callers send an OPTIONS preflight before the POST.
  if (req.method === "OPTIONS") {
    return new Response(null, {
      status: 204,
      headers: {
        "Access-Control-Allow-Origin": "*",
        "Access-Control-Allow-Methods": "POST, OPTIONS",
        "Access-Control-Allow-Headers": "authorization, content-type",
        "Access-Control-Max-Age": "86400",
      },
    });
  }

  if (req.method !== "POST") {
    return json({ error: "Method not allowed" }, 405);
  }

  // Resolve tenant identity from the caller's Supabase JWT (never trust body-provided IDs).
  const authHeader = req.headers.get("Authorization") ?? "";
  const jwt = authHeader.startsWith("Bearer ") ? authHeader.slice(7) : "";
  if (!jwt) {
    return json({ error: "Missing Supabase bearer token" }, 401);
  }
  const { data: userData, error: userErr } = await supabaseAdmin.auth.getUser(jwt);
  if (userErr || !userData?.user?.id) {
    return json({ error: "Invalid Supabase session" }, 401);
  }
  const tenantId = userData.user.id;

  let code: string | undefined;
  try {
    ({ code } = await req.json());
  } catch {
    return json({ error: "Invalid JSON body" }, 400);
  }
  if (!code) {
    return json({ error: "Missing OAuth code" }, 400);
  }

  // Step 1: code -> short-lived user access token
  let shortLived: { access_token: string };
  try {
    shortLived = await jsonFetch(
      graph("oauth/access_token", {
        client_id: META_APP_ID,
        client_secret: META_APP_SECRET,
        redirect_uri: `${new URL(req.url).origin}/functions/v1/meta-oauth`,
        code,
      }),
    );
  } catch (err) {
    logStructured({ event: "code_exchange_failed" });
    return json({ error: String(err) }, 502);
  }

  // Step 2: short-lived -> long-lived user token (~60 days)
  let longLived: { access_token: string; expires_in?: number };
  try {
    longLived = await jsonFetch(
      graph("oauth/access_token", {
        grant_type: "fb_exchange_token",
        client_id: META_APP_ID,
        client_secret: META_APP_SECRET,
        fb_exchange_token: shortLived.access_token,
      }),
    );
  } catch (err) {
    logStructured({ event: "long_lived_exchange_failed" });
    return json({ error: String(err) }, 502);
  }

  // Step 3: fetch ALL managed pages (paginated). Reconciliation depends on
  // this being a COMPLETE enumeration, so failures abort before any delete.
  let pages: PageRow[];
  try {
    pages = await fetchAllManagedPages(longLived.access_token);
  } catch (err) {
    logStructured({ event: "page_enumeration_failed" });
    return json({ error: String(err) }, 502);
  }

  // Step 4a: persist current ownership mapping (service role bypasses RLS;
  // page_id is globally UNIQUE so one page belongs to exactly one tenant).
  if (pages.length > 0) {
    const rows = pages.map((p) => ({
      tenant_id: tenantId,
      page_id: p.id,
      page_name: p.name,
    }));
    const { error: upsertErr } = await supabaseAdmin
      .from("meta_tenant_pages")
      .upsert(rows, { onConflict: "tenant_id,page_id" });
    if (upsertErr) {
      logStructured({ event: "page_mapping_upsert_failed" });
      return json({ error: `Failed to store page mapping: ${upsertErr.message}` }, 500);
    }
  }

  // Step 4b: reconcile stale mappings. Only reachable on a complete,
  // successful enumeration above. Removes pages this tenant no longer
  // manages so former admins cannot keep reading webhook payloads via RLS.
  if (pages.length > 0) {
    const keepIds = pages.map((p) => p.id);
    const { data: ownedRows, error: selectErr } = await supabaseAdmin
      .from("meta_tenant_pages")
      .select("page_id")
      .eq("tenant_id", tenantId);
    if (selectErr) {
      logStructured({ event: "reconcile_select_failed" });
      return json({ error: `Failed to reconcile page mappings: ${selectErr.message}` }, 500);
    }
    const staleIds = ((ownedRows ?? []) as Array<{ page_id: string }>)
      .map((r) => r.page_id)
      .filter((pid: string) => !keepIds.includes(pid));
    if (staleIds.length > 0) {
      const { error: delErr } = await supabaseAdmin
        .from("meta_tenant_pages")
        .delete()
        .eq("tenant_id", tenantId)
        .in("page_id", staleIds);
      if (delErr) {
        logStructured({ event: "reconcile_delete_failed", stale_count: staleIds.length });
        return json({ error: `Failed to remove stale page mappings: ${delErr.message}` }, 500);
      }
      logStructured({ event: "stale_mappings_removed", count: staleIds.length });
    }
  }

  logStructured({ event: "oauth_completed", page_count: pages.length });

  // Step 5: hand tokens back for Stronghold storage. The App Secret never leaves this function.
  return json({
    user_access_token: longLived.access_token,
    expires_in: longLived.expires_in ?? null,
    pages: pages.map((p) => ({
      page_id: p.id,
      page_name: p.name,
      access_token: p.access_token,
    })),
  });
});
