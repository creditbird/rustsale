// Supabase Edge Function: /meta-oauth (revised plan Task 1)
// Zero-Secret PKCE OAuth with server-side session state machine:
// - GET  /meta-oauth-start   (POST body: { state, code_challenge }) -> { auth_url }
// - GET  /meta-oauth-callback?code=...&state=... (browser redirect from Meta)
// The Meta App Secret and the page-token payload encryption key live ONLY on
// the server. Page tokens reach the desktop as a single-use, encrypted,
// PKCE-bound exchange code delivered via the rustsale:// deep link.
import { serve } from "https://deno.land/std@0.177.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2.39.8";

const SUPABASE_URL = Deno.env.get("SUPABASE_URL")!;
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!;
const META_APP_ID = Deno.env.get("META_APP_ID")!;
const META_APP_SECRET = Deno.env.get("META_APP_SECRET")!;

const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

serve(async (req: Request) => {
  const url = new URL(req.url);

  if (url.pathname.endsWith("/meta-oauth-start")) {
    const authHeader = req.headers.get("Authorization");
    if (!authHeader) return new Response("Unauthorized", { status: 401 });

    const userClient = createClient(SUPABASE_URL, authHeader.replace("Bearer ", ""), {
      global: { headers: { Authorization: authHeader } },
    });
    const { data: { user }, error: userError } = await userClient.auth.getUser();
    if (userError || !user) return new Response("Unauthorized", { status: 401 });

    const { state, code_challenge } = await req.json();
    if (!state || !code_challenge) return new Response("Missing parameters", { status: 400 });

    const { error: sessionError } = await supabaseAdmin
      .from("oauth_authorization_sessions")
      .insert({
        state,
        tenant_id: user.id,
        code_challenge,
        status: "pending",
        expires_at: new Date(Date.now() + 10 * 60 * 1000).toISOString(),
      });

    if (sessionError) return new Response("Session creation error", { status: 500 });

    const redirectUri = `${url.origin}/meta-oauth-callback`;
    const metaAuthUrl = `https://www.facebook.com/v26.0/dialog/oauth?client_id=${META_APP_ID}&redirect_uri=${encodeURIComponent(redirectUri)}&state=${encodeURIComponent(state)}&scope=pages_show_list,pages_read_engagement,pages_manage_posts,pages_messaging,leads_retrieval`;

    return new Response(JSON.stringify({ auth_url: metaAuthUrl }), {
      headers: { "Content-Type": "application/json" },
    });
  }

  if (url.pathname.endsWith("/meta-oauth-callback")) {
    const code = url.searchParams.get("code");
    const state = url.searchParams.get("state");
    const errorParam = url.searchParams.get("error");

    if (errorParam || !code || !state) {
      if (state) {
        await supabaseAdmin
          .from("oauth_authorization_sessions")
          .update({ status: "failed", updated_at: new Date().toISOString() })
          .eq("state", state);
      }
      return new Response("OAuth consent canceled or failed", { status: 400 });
    }

    // Atomic session claim with 5-minute lease deadline
    const { data: claimedSession, error: claimError } = await supabaseAdmin
      .from("oauth_authorization_sessions")
      .update({
        status: "processing",
        attempts: 1,
        lease_deadline: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
        updated_at: new Date().toISOString(),
      })
      .eq("state", state)
      .or("status.eq.pending,and(status.eq.processing,lease_deadline.lt.now())")
      .gt("expires_at", new Date().toISOString())
      .select("tenant_id, code_challenge")
      .single();

    if (claimError || !claimedSession) {
      return new Response("Invalid, expired, or already claimed session state", { status: 400 });
    }

    try {
      const redirectUri = `${url.origin}/meta-oauth-callback`;
      const tokenUrl = `https://graph.facebook.com/v26.0/oauth/access_token?client_id=${META_APP_ID}&redirect_uri=${encodeURIComponent(redirectUri)}&client_secret=${META_APP_SECRET}&code=${encodeURIComponent(code)}`;
      const tokenResp = await fetch(tokenUrl);
      if (!tokenResp.ok) throw new Error(`Token exchange failed: ${tokenResp.status}`);
      const tokenData = await tokenResp.json();
      const shortToken = tokenData.access_token;

      const longTokenUrl = `https://graph.facebook.com/v26.0/oauth/access_token?grant_type=fb_exchange_token&client_id=${META_APP_ID}&client_secret=${META_APP_SECRET}&fb_exchange_token=${encodeURIComponent(shortToken)}`;
      const longResp = await fetch(longTokenUrl);
      if (!longResp.ok) throw new Error(`Long lived token exchange failed: ${longResp.status}`);
      const longData = await longResp.json();
      const longLivedUserToken = longData.access_token;

      // Fail-closed ASID resolution from /me
      const meUrl = `https://graph.facebook.com/v26.0/me?access_token=${encodeURIComponent(longLivedUserToken)}`;
      const meResp = await fetch(meUrl);
      if (!meResp.ok) throw new Error(`Failed to resolve user identity from Meta Graph API: ${meResp.status}`);
      const meData = await meResp.json();
      const appUserId = meData.id;
      if (!appUserId) throw new Error("Meta user identity ID missing from /me response");

      const accountsUrl = `https://graph.facebook.com/v26.0/me/accounts?access_token=${encodeURIComponent(longLivedUserToken)}`;
      const accResp = await fetch(accountsUrl);
      if (!accResp.ok) throw new Error(`Accounts retrieval failed: ${accResp.status}`);
      const accData = await accResp.json();
      const pages = (accData.data || []).map((p: any) => ({
        id: p.id,
        name: p.name,
        access_token: p.access_token,
        category: p.category,
      }));

      // Fail-closed PSID resolution for each managed page via ids_for_pages
      const idsUrl = `https://graph.facebook.com/v26.0/${appUserId}/ids_for_pages?access_token=${encodeURIComponent(longLivedUserToken)}`;
      const idsResp = await fetch(idsUrl);
      if (!idsResp.ok) throw new Error(`Failed to resolve page-scoped identities from Meta Graph API: ${idsResp.status}`);
      const idsData = await idsResp.json();

      // Persist authoritative ASID-to-PSID mapping from Meta ids_for_pages
      const mappedPageIds = new Set<string>();
      for (const item of idsData.data || []) {
        if (item.page?.id && item.id) {
          const { error: upsertErr } = await supabaseAdmin.from("meta_user_page_interactions").upsert({
            tenant_id: claimedSession.tenant_id,
            page_id: item.page.id,
            app_user_id: appUserId,
            page_scoped_user_id: item.id,
            last_interaction_at: new Date().toISOString(),
          }, { onConflict: "tenant_id,page_id,page_scoped_user_id" });
          if (upsertErr) throw new Error(`Failed to persist durable user page interaction mapping: ${upsertErr.message}`);
          mappedPageIds.add(item.page.id);
        }
      }

      // Enforce 100% complete coverage: every connected page MUST have an authoritative ASID-to-PSID mapping
      const unmappedPages = pages.filter((p: any) => !mappedPageIds.has(p.id));
      if (unmappedPages.length > 0) {
        throw new Error(`Cannot connect pages: Meta Graph API /ids_for_pages did not return PSID mappings for connected page(s): ${unmappedPages.map((p: any) => p.id).join(", ")}. Every connected page requires an authoritative PSID mapping.`);
      }

      const exchangeCode = crypto.randomUUID();
      const { error: rpcError } = await supabaseAdmin.rpc("create_meta_oauth_exchange_code", {
        p_code: exchangeCode,
        p_tenant_id: claimedSession.tenant_id,
        p_code_challenge: claimedSession.code_challenge,
        p_pages_json: JSON.stringify(pages),
      });

      if (rpcError) throw rpcError;

      await supabaseAdmin
        .from("oauth_authorization_sessions")
        .update({ status: "consumed", updated_at: new Date().toISOString() })
        .eq("state", state);

      const desktopCallback = `rustsale://meta-oauth?code=${encodeURIComponent(exchangeCode)}&state=${encodeURIComponent(state)}`;
      return new Response(`<!DOCTYPE html><html><body><script>window.location.href = "${desktopCallback}";</script><p>Redirecting to RustSale Desktop...</p></body></html>`, {
        headers: { "Content-Type": "text/html" },
      });
    } catch (err) {
      await supabaseAdmin
        .from("oauth_authorization_sessions")
        .update({ status: "failed", updated_at: new Date().toISOString() })
        .eq("state", state);
      return new Response("OAuth processing error: " + String(err), { status: 500 });
    }
  }

  return new Response("Not Found", { status: 404 });
});
