// Supabase Edge Function: /meta-debug-token
// Server-side inspection of Meta access tokens (page/user) via the
// debug_token endpoint. Requires the caller's Supabase Bearer JWT and only
// reports metadata (validity, scopes, expiry) — never returns any token,
// never echoes the app secret.
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
  // Never log input tokens, app secrets, or full Meta responses.
  console.log(JSON.stringify({ fn: "meta-debug-token", ...fields }));
}

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json", "Cache-Control": "no-store" },
  });
}

function corsPreflight(): Response {
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

serve(async (req: Request) => {
  if (!supabaseAdmin || !META_APP_ID || !META_APP_SECRET) {
    logStructured({ event: "server_misconfigured" });
    return json({ error: "Server configuration error" }, 500);
  }

  // Browser/WebView callers send an OPTIONS preflight before the POST.
  if (req.method === "OPTIONS") {
    return corsPreflight();
  }

  if (req.method !== "POST") {
    return json({ error: "Method not allowed" }, 405);
  }

  const authHeader = req.headers.get("Authorization") ?? "";
  const jwt = authHeader.startsWith("Bearer ") ? authHeader.slice(7) : "";
  if (!jwt) {
    return json({ error: "Missing Supabase bearer token" }, 401);
  }
  const { error: userErr } = await supabaseAdmin.auth.getUser(jwt);
  if (userErr) {
    return json({ error: "Invalid Supabase session" }, 401);
  }

  let inputToken: string | undefined;
  try {
    ({ access_token: inputToken } = await req.json());
  } catch {
    return json({ error: "Invalid JSON body" }, 400);
  }
  if (!inputToken || typeof inputToken !== "string") {
    return json({ error: "Missing access_token to inspect" }, 400);
  }

  const params = new URLSearchParams({
    input_token: inputToken,
    access_token: `${META_APP_ID}|${META_APP_SECRET}`,
  });

  let resp: Response;
  try {
    resp = await fetch(
      `https://graph.facebook.com/${META_GRAPH_VERSION}/debug_token?${params.toString()}`,
      // Bound the outbound call so callers get a controlled error instead of
      // an unbounded hang.
      { signal: AbortSignal.timeout(15_000) },
    );
  } catch {
    logStructured({ event: "graph_unreachable" });
    return json({ error: "Meta Graph API unreachable" }, 504);
  }

  let body: any;
  try {
    body = await resp.json();
  } catch {
    logStructured({ event: "graph_non_json_response", status: resp.status });
    return json({ error: `Invalid response from Meta API (HTTP ${resp.status})` }, 502);
  }

  if (!resp.ok || body?.error) {
    logStructured({ event: "debug_token_rejected", status: resp.status });
    return json({ error: body?.error?.message ?? `Meta API HTTP ${resp.status}` }, 502);
  }

  const data = body?.data ?? {};
  // Normalised token health snapshot for the desktop Page Health Monitor.
  return json({
    is_valid: data.is_valid === true,
    app_id: data.app_id ?? null,
    user_id: data.user_id ?? null,
    scopes: Array.isArray(data.scopes) ? data.scopes : [],
    expires_at: data.expires_at ?? null, // 0 => never expires (page tokens)
    issued_at: data.issued_at ?? null,
    profile_type: data.profile_type ?? null,
  });
});
