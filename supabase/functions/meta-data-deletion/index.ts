import { serve } from "https://deno.land/std@0.177.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2.39.8";

const SUPABASE_URL = Deno.env.get("SUPABASE_URL")!;
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!;
const META_APP_ID = Deno.env.get("META_APP_ID")!;
const META_APP_SECRET = Deno.env.get("META_APP_SECRET")!;

const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

interface PageScopedMapping {
  page_id: string;
  psid: string;
}

async function parseAndVerifySignedRequest(signedRequest: string, appSecret: string): Promise<any | null> {
  const parts = signedRequest.split(".");
  if (parts.length !== 2) return null;
  const [encodedSig, payloadStr] = parts;

  try {
    // Base64URL decode signature bytes
    const sigBinaryStr = atob(encodedSig.replace(/-/g, "+").replace(/_/g, "/"));
    const sigBytes = new Uint8Array(sigBinaryStr.length);
    for (let i = 0; i < sigBinaryStr.length; i++) {
      sigBytes[i] = sigBinaryStr.charCodeAt(i);
    }

    // Compute HMAC-SHA256 of payloadStr using appSecret
    const encoder = new TextEncoder();
    const key = await crypto.subtle.importKey(
      "raw",
      encoder.encode(appSecret),
      { name: "HMAC", hash: "SHA-256" },
      false,
      ["verify"]
    );
    const isValid = await crypto.subtle.verify(
      "HMAC",
      key,
      sigBytes,
      encoder.encode(payloadStr)
    );

    if (!isValid) return null;

    const data = JSON.parse(atob(payloadStr.replace(/-/g, "+").replace(/_/g, "/")));
    if (data.algorithm !== "HMAC-SHA256") return null;
    return data;
  } catch {
    return null;
  }
}

async function resolvePsidsForAsid(asid: string): Promise<PageScopedMapping[]> {
  if (!META_APP_ID || !META_APP_SECRET) {
    throw new Error("Missing Meta App credentials (META_APP_ID or META_APP_SECRET)");
  }
  const mappings: PageScopedMapping[] = [];
  const appToken = `${META_APP_ID}|${META_APP_SECRET}`;
  let nextUrl: string | null = `https://graph.facebook.com/v26.0/${encodeURIComponent(asid)}/ids_for_pages?access_token=${appToken}&limit=100`;

  while (nextUrl) {
    const res = await fetch(nextUrl);
    if (!res.ok) {
      const errBody = await res.text().catch(() => "");
      throw new Error(`Meta Graph API ids_for_pages returned HTTP ${res.status}: ${errBody}`);
    }
    const body = await res.json();
    if (!Array.isArray(body?.data)) {
      throw new Error("Meta Graph API ids_for_pages response is missing data array");
    }
    for (const item of body.data) {
      const psid = item?.id ? String(item.id).trim() : "";
      const pageId = item?.page?.id ? String(item.page.id).trim() : "";
      // Strict fail-closed validation: Reject any malformed item
      if (!psid || !pageId) {
        throw new Error(`Meta Graph API ids_for_pages returned malformed item without valid id and page.id: ${JSON.stringify(item)}`);
      }
      mappings.push({ page_id: pageId, psid });
    }
    nextUrl = body?.paging?.next || null;
  }

  return mappings;
}

serve(async (req: Request) => {
  const url = new URL(req.url);

  if (req.method === "POST" && url.pathname === "/meta-data-deletion") {
    const formData = await req.formData();
    const signedRequest = formData.get("signed_request") as string;
    if (!signedRequest) return new Response("Missing signed_request", { status: 400 });

    const data = await parseAndVerifySignedRequest(signedRequest, META_APP_SECRET);
    if (!data || !data.user_id) {
      return new Response("Invalid or untrusted signed_request signature", { status: 400 });
    }
    const userId = data.user_id;
    const confirmationCode = crypto.randomUUID();

    // Authoritatively resolve all Page-Scoped IDs (PSIDs) and associated page_ids for this App-Scoped ID (ASID) from Meta Graph API
    let graphMappings: PageScopedMapping[] = [];
    try {
      graphMappings = await resolvePsidsForAsid(userId);
    } catch (err) {
      console.error("Fail-closed: Failed to resolve PSIDs from Meta Graph API for ASID:", userId, err);
      return new Response("Bad Gateway: Unable to resolve user identities from Meta Graph API. Retry later.", { status: 502 });
    }

    // Always query local durable mapping in meta_user_page_interactions and union with Graph mappings to ensure 100% full coverage across all connected pages (even if Graph returns empty or partial set)
    const { data: durableInteractions, error: queryErr } = await supabaseAdmin
      .from("meta_user_page_interactions")
      .select("page_id, page_scoped_user_id")
      .eq("app_user_id", userId);

    if (queryErr) {
      console.error("Fail-closed: Failed to query local meta_user_page_interactions:", queryErr);
      return new Response("Service Unavailable: Failed to resolve local user page identities. Retry later.", { status: 503 });
    }

    const mappingMap = new Map<string, PageScopedMapping>();
    for (const m of graphMappings) {
      mappingMap.set(`${m.page_id}:${m.psid}`, m);
    }
    for (const item of durableInteractions || []) {
      if (item.page_id && item.page_scoped_user_id) {
        mappingMap.set(`${item.page_id}:${item.page_scoped_user_id}`, { page_id: item.page_id, psid: item.page_scoped_user_id });
      }
    }
    const resolvedMappings: PageScopedMapping[] = Array.from(mappingMap.values());

    // Call deletion RPC with resolved page mappings (or empty array if no active page identities or retained records exist)
    const { error: deletionRpcError } = await supabaseAdmin.rpc("create_data_deletion_request_and_tasks", {
      p_confirmation_code: confirmationCode,
      p_user_id: userId,
      p_target_mappings: resolvedMappings,
    });

    if (deletionRpcError) {
      console.error("Failed to create data deletion request:", deletionRpcError);
      return new Response("Internal Server Error: Failed to record deletion request", { status: 500 });
    }

    const statusUrl = `https://rustsale.trantrongtri.com/deletion-status?code=${confirmationCode}`;
    return new Response(JSON.stringify({ url: statusUrl, confirmation_code: confirmationCode }), {
      headers: { "Content-Type": "application/json" },
    });
  }

  if (req.method === "GET" && url.pathname === "/deletion-status") {
    const code = url.searchParams.get("code");
    if (!code || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(code)) {
      return new Response("Invalid or missing confirmation code", {
        status: 400,
        headers: {
          "Content-Type": "text/plain",
          "Cache-Control": "no-store, private",
          "X-Content-Type-Options": "nosniff",
          "X-Frame-Options": "DENY"
        }
      });
    }

    const { data: reqData, error: reqError } = await supabaseAdmin
      .from("meta_data_deletion_requests")
      .select("id, status, created_at, completed_at")
      .eq("confirmation_code", code)
      .maybeSingle();

    if (reqError) {
      console.error("Failed to query deletion request:", reqError);
      return new Response("Internal Server Error: Status query failed", {
        status: 500,
        headers: { "Cache-Control": "no-store, private" }
      });
    }

    if (!reqData) {
      return new Response("Deletion request not found", {
        status: 404,
        headers: { "Cache-Control": "no-store, private" }
      });
    }

    const { data: pendingTasks, error: taskError } = await supabaseAdmin
      .from("meta_device_deletion_tasks")
      .select("id")
      .eq("request_id", reqData.id)
      .neq("status", "completed");

    if (taskError) {
      console.error("Failed to query device deletion task status:", taskError);
      return new Response("Internal Server Error: Device task check failed", {
        status: 500,
        headers: { "Cache-Control": "no-store, private" }
      });
    }

    let currentStatus = reqData.status;
    if (reqData.status === "processing" && (!pendingTasks || pendingTasks.length === 0)) {
      currentStatus = "completed";
      const { error: updateError } = await supabaseAdmin
        .from("meta_data_deletion_requests")
        .update({ status: "completed", completed_at: new Date().toISOString() })
        .eq("id", reqData.id);

      if (updateError) {
        console.error("Failed to update deletion request completion:", updateError);
        return new Response("Internal Server Error: Failed to record completion", {
          status: 500,
          headers: { "Cache-Control": "no-store, private" }
        });
      }
    }

    return new Response(JSON.stringify({
      status: currentStatus,
      confirmation_code: code,
      created_at: reqData.created_at,
      completed_at: currentStatus === "completed" ? (reqData.completed_at || new Date().toISOString()) : null,
    }), {
      headers: {
        "Content-Type": "application/json",
        "Cache-Control": "no-store, private",
        "X-Content-Type-Options": "nosniff",
        "X-Frame-Options": "DENY"
      },
    });
  }

  return new Response("Not Found", { status: 404 });
});
