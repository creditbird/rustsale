// Supabase Edge Function: /meta-oauth
// Zero-secret OAuth bridge: the Meta App Secret lives ONLY on this server.
// POST { code } with the desktop user's Supabase Bearer JWT.
// 1. Exchanges the authorization code for a short-lived user token.
// 2. Exchanges it again for a Long-Lived User Token (~60 days).
// 3. Fetches ALL managed Pages via /me/accounts with full cursor pagination
//    (long-lived Page tokens).
// 4. Atomically replaces this tenant's page ownership via the
//    replace_meta_tenant_pages() SECURITY DEFINER RPC — invoked on EVERY
//    complete successful enumeration, INCLUDING an empty result set, so
//    revocation and stale-owner takeover are deterministic. A failed
//    enumeration aborts BEFORE any database mutation.
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

const GRAPH_TIMEOUT_MS = 15_000;
const MAX_PAGINATION_REQUESTS = 50; // 50 x 100 = 5,000 Pages hard bound.

function logStructured(fields: Record<string, unknown>): void {
  // Never log codes, tokens, or secrets.
  console.log(JSON.stringify({ fn: "meta-oauth", ...fields }));
}

function json(body: unknown, status = 200): Response {
  // Token-bearing bodies must never be cached; browser/WebView callers need
  // ACAO on every actual response, not only the preflight.
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      "Content-Type": "application/json",
      "Cache-Control": "no-store",
      "Access-Control-Allow-Origin": "*",
    },
  });
}

function graph(path: string, params: Record<string, string>): string {
  const search = new URLSearchParams(params);
  return `https://graph.facebook.com/${META_GRAPH_VERSION}/${path}?${search.toString()}`;
}

async function jsonFetch<T>(url: string): Promise<T> {
  const resp = await fetch(url, { signal: AbortSignal.timeout(GRAPH_TIMEOUT_MS) });
  let body: unknown;
  try {
    body = await resp.json();
  } catch {
    throw new Error(`Meta API non-JSON response (HTTP ${resp.status})`);
  }
  if (!resp.ok) {
    const message =
      (body as any)?.error?.message ?? `Meta API HTTP ${resp.status}`;
    throw new Error(`Meta API error: ${message}`);
  }
  return body as T;
}

type PageRow = { id: string; name: string; access_token: string };

async function fetchAllManagedPages(longLivedToken: string): Promise<PageRow[]> {
  // Follow paging.next cursors until exhausted so >100-Page accounts are not
  // silently truncated. Any intermediate failure throws -> caller aborts
  // without touching the database.
  let pages: PageRow[] = [];
  let url: string | null = graph("me/accounts", {
    access_token: longLivedToken,
    fields: "id,name,access_token",
    limit: "100",
  });
  let cursorCount = 0;
  while (url) {
    if (++cursorCount > MAX_PAGINATION_REQUESTS) {
      // Defensive bound against malformed/cyclic paging chains.
      throw new Error("Meta page enumeration exceeded pagination limit");
    }
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
  } catch {
    logStructured({ event: "code_exchange_failed" });
    return json({ error: "Meta authorization code exchange failed" }, 502);
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
  } catch {
    logStructured({ event: "long_lived_exchange_failed" });
    return json({ error: "Meta token upgrade failed" }, 502);
  }

  // Step 3: fetch ALL managed pages (paginated). Reconciliation depends on
  // this being a COMPLETE enumeration, so failures abort before any delete.
  let pages: PageRow[];
  try {
    pages = await fetchAllManagedPages(longLived.access_token);
  } catch {
    logStructured({ event: "page_enumeration_failed" });
    return json({ error: "Meta page enumeration failed" }, 502);
  }

  // Step 4: ATOMIC reconciliation via SERVICE-ROLE-ONLY SECURITY DEFINER RPC.
  // The tenant id is passed explicitly from the JWT we just verified (the
  // service-role client carries no auth.uid() context). The RPC runs on every
  // complete enumeration — including zero Pages (= revoke all) — and also
  // purges other tenants' buffered history for Pages being (re)claimed, so
  // ownership transfer never leaks the previous tenant's events.
  const verifiedPages = pages.map((p) => ({ page_id: p.id, page_name: p.name }));
  const { error: rpcErr } = await supabaseAdmin.rpc("replace_meta_tenant_pages", {
    p_tenant_id: tenantId,
    p_pages: verifiedPages,
  });
  if (rpcErr) {
    logStructured({ event: "reconciliation_failed", detail: rpcErr.code ?? null });
    return json({ error: "Failed to reconcile page mappings" }, 500);
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
