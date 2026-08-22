// Supabase Edge Function: /meta-oauth
// Zero-secret OAuth bridge: the Meta App Secret lives ONLY on this server.
// POST { code } with the desktop user's Supabase Bearer JWT.
// 1. Exchanges the authorization code for a short-lived user token.
// 2. Exchanges it again for a Long-Lived User Token (~60 days).
// 3. Fetches all managed Pages via /me/accounts (long-lived Page tokens).
// 4. Persists tenant <-> page ownership in meta_tenant_pages.
// 5. Returns tokens so the desktop app can store them in Stronghold/Keyring.
import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const META_APP_ID = Deno.env.get("META_APP_ID") || "";
const META_APP_SECRET = Deno.env.get("META_APP_SECRET") || "";
const META_GRAPH_VERSION = Deno.env.get("META_GRAPH_VERSION") || "v26.0";
const SUPABASE_URL = Deno.env.get("SUPABASE_URL") || "";
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") || "";

const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
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

serve(async (req: Request) => {
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

  if (!META_APP_ID || !META_APP_SECRET) {
    return json({ error: "Server missing Meta app credentials" }, 500);
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
    return json({ error: String(err) }, 502);
  }

  // Step 3: fetch all managed pages (never-expiring page tokens unless revoked)
  type PageRow = { id: string; name: string; access_token: string };
  let pages: PageRow[];
  try {
    const meAccounts = await jsonFetch<{ data: PageRow[] }>(
      graph("me/accounts", {
        access_token: longLived.access_token,
        fields: "id,name,access_token",
        limit: "100",
      }),
    );
    pages = meAccounts.data ?? [];
  } catch (err) {
    return json({ error: String(err) }, 502);
  }

  // Step 4: persist tenant <-> page ownership mapping
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
      return json({ error: `Failed to store page mapping: ${upsertErr.message}` }, 500);
    }
  }

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
