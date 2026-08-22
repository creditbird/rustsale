// Supabase Edge Function: /meta-oauth (revised plan Task 1)
// Zero-Secret PKCE OAuth with server-side session state machine:
// - POST .../meta-oauth-start  { state, code_challenge } + Supabase JWT
// - GET  .../meta-oauth-callback?code=...&state=...   (browser, from Meta)
// The Meta App Secret and the payload encryption key live ONLY on the server.
// Page tokens reach the desktop as a single-use, encrypted, PKCE-bound
// exchange code delivered via an HTTP redirect to the rustsale:// deep link.
import { serve } from "https://deno.land/std@0.177.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2.39.8";

const SUPABASE_URL = Deno.env.get("SUPABASE_URL")!;
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!;
const META_APP_ID = Deno.env.get("META_APP_ID")!;
const META_APP_SECRET = Deno.env.get("META_APP_SECRET")!;
// Configured absolute callback URL, e.g.
// https://<project>.supabase.co/functions/v1/meta-oauth/meta-oauth-callback
const META_OAUTH_REDIRECT_URI = Deno.env.get("META_OAUTH_REDIRECT_URI")!;

const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

const GRAPH_TIMEOUT_MS = 30_000; // per request, well below the 5-minute lease
const MAX_PAGINATION_REQUESTS = 50;

function logStructured(fields: Record<string, unknown>): void {
  // Redacted by construction: never codes, tokens, secrets, or raw payloads.
  console.log(JSON.stringify({ fn: "meta-oauth", ...fields }));
}

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json", "Cache-Control": "no-store" },
  });
}

async function jsonFetch<T>(url: string): Promise<T> {
  const resp = await fetch(url, { signal: AbortSignal.timeout(GRAPH_TIMEOUT_MS) });
  let body: unknown;
  try {
    body = await resp.json();
  } catch {
    throw new Error(`non_json_response_${resp.status}`);
  }
  if (!resp.ok) {
    throw new Error(`graph_error_${resp.status}`);
  }
  return body as T;
}

type PageRow = { id: string; name: string; access_token?: string; category?: string };

async function fetchAllPages(longLivedUserToken: string): Promise<PageRow[]> {
  let pages: PageRow[] = [];
  let url: string | null =
    `https://graph.facebook.com/v26.0/me/accounts?fields=id,name,access_token,category&limit=100&access_token=${encodeURIComponent(longLivedUserToken)}`;
  let requests = 0;
  while (url) {
    if (++requests > MAX_PAGINATION_REQUESTS) throw new Error("pagination_limit");
    const page: { data?: PageRow[]; paging?: { next?: string } } = await jsonFetch(url);
    pages = pages.concat(page.data ?? []);
    url = page.paging?.next ?? null;
  }
  return pages;
}

type InteractionRow = { page_id: string; app_user_id: string; page_scoped_user_id: string };

async function fetchAllPageIds(
  appUserId: string,
  longLivedUserToken: string,
): Promise<InteractionRow[]> {
  let interactions: InteractionRow[] = [];
  let url: string | null =
    `https://graph.facebook.com/v26.0/${encodeURIComponent(appUserId)}/ids_for_pages?limit=100&access_token=${encodeURIComponent(longLivedUserToken)}`;
  let requests = 0;
  while (url) {
    if (++requests > MAX_PAGINATION_REQUESTS) throw new Error("pagination_limit");
    const page: { data?: any[]; paging?: { next?: string } } = await jsonFetch(url);
    for (const item of page.data ?? []) {
      if (item.page?.id && item.id) {
        interactions.push({
          page_id: item.page.id,
          app_user_id: appUserId,
          page_scoped_user_id: item.id,
        });
      }
    }
    url = page.paging?.next ?? null;
  }
  return interactions;
}

serve(async (req: Request) => {
  const url = new URL(req.url);

  if (url.pathname.endsWith("/meta-oauth-start")) {
    const authHeader = req.headers.get("Authorization");
    if (!authHeader || !authHeader.startsWith("Bearer ")) {
      return jsonResponse({ error: "unauthorized" }, 401);
    }

    // Validate the caller's identity with the service client + explicit JWT
    // (the JWT must not be passed in the API-key slot).
    const jwt = authHeader.slice(7);
    const { data: userData, error: userError } = await supabaseAdmin.auth.getUser(jwt);
    if (userError || !userData?.user) {
      return jsonResponse({ error: "unauthorized" }, 401);
    }

    let state: unknown;
    let codeChallenge: unknown;
    try {
      ({ state, code_challenge: codeChallenge } = await req.json());
    } catch {
      return jsonResponse({ error: "invalid_body" }, 400);
    }

    // Early rejection of unusable sessions: state must be high-entropy and
    // code_challenge must be a valid S256 base64url value (43-128 chars).
    if (
      typeof state !== "string" ||
      state.length < 16 ||
      state.length > 256 ||
      typeof codeChallenge !== "string" ||
      !/^[A-Za-z0-9_-]{43,128}$/.test(codeChallenge)
    ) {
      return jsonResponse({ error: "invalid_parameters" }, 400);
    }

    const { error: sessionError } = await supabaseAdmin
      .from("oauth_authorization_sessions")
      .insert({
        state,
        tenant_id: userData.user.id,
        code_challenge: codeChallenge,
        status: "pending",
        expires_at: new Date(Date.now() + 10 * 60 * 1000).toISOString(),
      });

    if (sessionError) {
      logStructured({ event: "session_insert_failed", code: sessionError.code ?? null });
      return jsonResponse({ error: "session_creation_failed" }, 500);
    }

    const metaAuthUrl =
      `https://www.facebook.com/v26.0/dialog/oauth?client_id=${encodeURIComponent(META_APP_ID)}` +
      `&redirect_uri=${encodeURIComponent(META_OAUTH_REDIRECT_URI)}` +
      `&state=${encodeURIComponent(state)}` +
      `&scope=pages_show_list,pages_read_engagement,pages_manage_posts,pages_messaging,leads_retrieval`;

    return jsonResponse({ auth_url: metaAuthUrl });
  }

  if (url.pathname.endsWith("/meta-oauth-callback")) {
    const code = url.searchParams.get("code");
    const state = url.searchParams.get("state");
    const errorParam = url.searchParams.get("error");

    if (!state) {
      return new Response("invalid_request", { status: 400, headers: { "Cache-Control": "no-store" } });
    }

    if (errorParam || !code) {
      await supabaseAdmin.rpc("fail_meta_oauth_session", { p_state: state, p_lease_id: null }).then(
        () => {},
        () => {},
      );
      logStructured({ event: "consent_failed_or_canceled" });
      return new Response("oauth_failed", { status: 400, headers: { "Cache-Control": "no-store" } });
    }

    // Atomic lease claim with real SQL predicates + fresh fence token.
    const { data: claimed, error: claimError } = await supabaseAdmin.rpc(
      "claim_meta_oauth_session",
      { p_state: state },
    );
    if (claimError || !claimed) {
      logStructured({ event: "claim_rejected", code: claimError?.message ?? null });
      return new Response("session_not_claimable", { status: 400, headers: { "Cache-Control": "no-store" } });
    }
    const leaseId: string = claimed.lease_id;
    const tenantId: string = claimed.tenant_id;

    try {
      // Same exact configured redirect URI for authorization and exchange.
      const tokenUrl =
        `https://graph.facebook.com/v26.0/oauth/access_token?client_id=${encodeURIComponent(META_APP_ID)}` +
        `&redirect_uri=${encodeURIComponent(META_OAUTH_REDIRECT_URI)}` +
        `&client_secret=${encodeURIComponent(META_APP_SECRET)}` +
        `&code=${encodeURIComponent(code)}`;
      const tokenData = await jsonFetch<{ access_token?: string }>(tokenUrl);
      const shortToken = tokenData.access_token;
      if (!shortToken) throw new Error("missing_short_token");

      const longData = await jsonFetch<{ access_token?: string }>(
        `https://graph.facebook.com/v26.0/oauth/access_token?grant_type=fb_exchange_token` +
          `&client_id=${encodeURIComponent(META_APP_ID)}&client_secret=${encodeURIComponent(META_APP_SECRET)}` +
          `&fb_exchange_token=${encodeURIComponent(shortToken)}`,
      );
      const longLivedUserToken = longData.access_token;
      if (!longLivedUserToken) throw new Error("missing_long_token");

      // Fail-closed ASID resolution from /me
      const meData = await jsonFetch<{ id?: string }>(
        `https://graph.facebook.com/v26.0/me?access_token=${encodeURIComponent(longLivedUserToken)}`,
      );
      const appUserId = meData.id;
      if (!appUserId) throw new Error("missing_asid");

      // Fully paginate both edges.
      const pages = await fetchAllPages(longLivedUserToken);
      const interactions = await fetchAllPageIds(appUserId, longLivedUserToken);

      // Enforce 100% coverage IN MEMORY before persisting anything.
      const mappedPageIds = new Set(interactions.map((i) => i.page_id));
      const unmappedPages = pages.filter((p) => !mappedPageIds.has(p.id));
      if (pages.length === 0) throw new Error("no_pages_returned");
      if (unmappedPages.length > 0) {
        logStructured({
          event: "coverage_failure",
          unmapped_count: unmappedPages.length,
        });
        throw new Error("incomplete_psid_coverage");
      }

      // Single transaction: interactions + encrypted single-use exchange code
      // + fenced session finalization.
      const exchangeCode = crypto.randomUUID();
      const { error: finalizeErr } = await supabaseAdmin.rpc("finalize_meta_oauth", {
        p_state: state,
        p_lease_id: leaseId,
        p_interactions: interactions,
        p_code: exchangeCode,
        p_pages_json: JSON.stringify(pages),
      });
      if (finalizeErr) {
        logStructured({ event: "finalize_failed", code: finalizeErr.message ?? null });
        throw new Error("finalize_failed");
      }

      logStructured({ event: "oauth_completed", page_count: pages.length });

      // Hosted Edge Functions rewrite text/html to text/plain, so an HTML
      // bootstrap page cannot be relied on — use a plain HTTP redirect which
      // the browser follows regardless of content-type rewriting.
      const desktopCallback =
        `rustsale://meta-oauth?code=${encodeURIComponent(exchangeCode)}&state=${encodeURIComponent(state)}`;
      return new Response(null, {
        status: 302,
        headers: { Location: desktopCallback, "Cache-Control": "no-store" },
      });
    } catch (err) {
      // Fenced failure: only invalidates the session if we still hold the lease.
      await supabaseAdmin
        .rpc("fail_meta_oauth_session", { p_state: state, p_lease_id: leaseId })
        .then(() => {}, () => {});
      logStructured({
        event: "callback_failed",
        stage: err instanceof Error ? err.message : "unknown",
      });
      // Stable public message; details stay in structured logs.
      return new Response("oauth_processing_error", { status: 500, headers: { "Cache-Control": "no-store" } });
    }
  }

  return new Response("not_found", { status: 404, headers: { "Cache-Control": "no-store" } });
});
