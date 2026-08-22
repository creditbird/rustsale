// Supabase Edge Function: /meta-webhook (revised plan Task 2)
// Durable webhook relay with canonical deterministic JSON hashing.
// - GET  hub verification handshake (hub.verify_token).
// - POST X-Hub-Signature-256 HMAC over RAW body -> strict 64-hex format check
//        -> per-page ownership resolution from meta_tenant_pages
//        -> atomic batch ingestion via service-role-only RPC
//           ingest_meta_webhook_batch (buffer + interactions + lead identities,
//           idempotent on tenant/page/event_key), fanned out per mapping.
// Ingestion failures return non-2xx so Meta retries instead of losing events.
import { serve } from "https://deno.land/std@0.177.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2.39.8";

const SUPABASE_URL = Deno.env.get("SUPABASE_URL")!;
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!;
const META_APP_SECRET = Deno.env.get("META_APP_SECRET")!;
const META_VERIFY_TOKEN = Deno.env.get("META_VERIFY_TOKEN")!;

// Meta webhook payloads are small JSON documents; 1 MiB is generous.
const MAX_BODY_BYTES = 1_048_576;

const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

function logStructured(fields: Record<string, unknown>): void {
  // Redacted by construction: only counters/flags, never payloads or tokens.
  console.log(JSON.stringify({ fn: "meta-webhook", ...fields }));
}

function canonicalJsonStringify(obj: any): string {
  if (obj === null || typeof obj !== "object") {
    return JSON.stringify(obj);
  }
  if (Array.isArray(obj)) {
    return "[" + obj.map(canonicalJsonStringify).join(",") + "]";
  }
  const keys = Object.keys(obj).sort();
  const pairs = keys.map((k) => JSON.stringify(k) + ":" + canonicalJsonStringify(obj[k]));
  return "{" + pairs.join(",") + "}";
}

async function computeSha256(str: string): Promise<string> {
  const encoder = new TextEncoder();
  const data = encoder.encode(str);
  const hash = await crypto.subtle.digest("SHA-256", data);
  return Array.from(new Uint8Array(hash)).map((b) => b.toString(16).padStart(2, "0")).join("");
}

async function verifyMetaSignature(req: Request, rawBody: string): Promise<boolean> {
  const signatureHeader = req.headers.get("X-Hub-Signature-256");
  if (!signatureHeader || !signatureHeader.startsWith("sha256=")) return false;

  const expectedSignature = signatureHeader.replace("sha256=", "");
  // Strict structural check: exactly 64 hex chars (SHA-256 digest length).
  if (!/^[0-9a-fA-F]{64}$/.test(expectedSignature)) return false;

  const encoder = new TextEncoder();
  const keyData = encoder.encode(META_APP_SECRET);
  const key = await crypto.subtle.importKey("raw", keyData, { name: "HMAC", hash: "SHA-256" }, false, ["verify"]);
  const signatureBytes = new Uint8Array(
    expectedSignature.match(/.{1,2}/g)?.map((byte) => parseInt(byte, 16)) || []
  );

  return await crypto.subtle.verify("HMAC", key, signatureBytes, encoder.encode(rawBody));
}

serve(async (req: Request) => {
  const url = new URL(req.url);

  if (req.method === "GET") {
    const mode = url.searchParams.get("hub.mode");
    const token = url.searchParams.get("hub.verify_token");
    const challenge = url.searchParams.get("hub.challenge");

    if (mode === "subscribe" && token === META_VERIFY_TOKEN) {
      logStructured({ event: "subscription_verified" });
      return new Response(challenge, { status: 200 });
    }
    logStructured({ event: "subscription_rejected" });
    return new Response("Forbidden", { status: 403 });
  }

  if (req.method === "POST") {
    // Resource bound before reading the full body.
    const declaredLength = Number(req.headers.get("content-length") ?? "0");
    if (Number.isFinite(declaredLength) && declaredLength > MAX_BODY_BYTES) {
      logStructured({ event: "body_too_large", declared_length: declaredLength });
      return new Response("Payload too large", { status: 413 });
    }

    const rawBody = await req.text();
    if (rawBody.length > MAX_BODY_BYTES) {
      logStructured({ event: "body_too_large", actual_length: rawBody.length });
      return new Response("Payload too large", { status: 413 });
    }

    const isValid = await verifyMetaSignature(req, rawBody);
    if (!isValid) {
      logStructured({ event: "signature_rejected" });
      return new Response("Invalid signature", { status: 401 });
    }

    let payload: any;
    try {
      payload = JSON.parse(rawBody);
    } catch {
      return new Response("Invalid JSON", { status: 400 });
    }
    if (payload.object !== "page") {
      logStructured({ event: "ignored_non_page_object" });
      return new Response("Ignored non-page object", { status: 200 });
    }

    for (const entry of payload.entry || []) {
      const pageId = entry.id;
      const { data: pageMappings, error: pageMappingError } = await supabaseAdmin
        .from("meta_tenant_pages")
        .select("tenant_id")
        .eq("page_id", pageId);

      if (pageMappingError) {
        logStructured({ event: "mapping_lookup_failed" });
        // Retryable: Meta will redeliver when persistence is available again.
        return new Response("Internal Server Error: Page mapping query failed", { status: 500 });
      }

      if (!pageMappings || pageMappings.length === 0) continue;

      const events: any[] = [];
      const interactions: any[] = [];
      const leadInteractions: any[] = [];

      if (entry.messaging) {
        for (const msg of entry.messaging) {
          const senderId = msg.sender?.id;
          const timestamp = msg.timestamp ? new Date(msg.timestamp).toISOString() : new Date().toISOString();
          if (senderId && senderId !== pageId) {
            interactions.push({ user_id: senderId, app_user_id: null, timestamp });
          }

          let eventKey = "";
          if (msg.message?.mid) eventKey = `msg_${msg.message.mid}`;
          else if (msg.delivery?.mids) eventKey = `del_${msg.delivery.mids.join("_")}_${msg.delivery.watermark}`;
          else if (msg.read?.watermark) eventKey = `read_${senderId}_${msg.read.watermark}`;
          else if (msg.reaction?.mid) {
            // Include action+emoji so react -> unreact/change remain distinct
            // logical events; true retries carry identical payloads and dedupe.
            eventKey = `react_${msg.reaction.mid}_${senderId}_${msg.reaction.action ?? ""}_${msg.reaction.emoji ?? ""}_${msg.reaction.reaction || "none"}`;
          } else eventKey = `misc_${senderId}_${await computeSha256(canonicalJsonStringify(msg))}`;

          events.push({ field: "messages", event_key: eventKey, payload: msg });
        }
      }

      if (entry.changes) {
        for (const ch of entry.changes) {
          const val = ch.value || {};
          const field = ch.field;
          const fromId = val.from?.id;
          const timestamp = val.created_time ? new Date(val.created_time * 1000).toISOString() : new Date().toISOString();

          if (fromId && fromId !== pageId) {
            interactions.push({ user_id: fromId, app_user_id: null, timestamp });
          }

          if (field === "leadgen" && val.leadgen_id) {
            leadInteractions.push({
              leadgen_id: val.leadgen_id,
              form_id: val.form_id || null,
              created_at: timestamp,
            });
          }

          const payloadHash = await computeSha256(canonicalJsonStringify(val));
          let eventKey = "";
          if (val.comment_id) eventKey = `comment_${val.comment_id}_${val.verb || "edit"}_${payloadHash}`;
          else if (val.post_id) eventKey = `post_${val.post_id}_${val.verb || "edit"}_${payloadHash}`;
          else if (val.leadgen_id) eventKey = `leadgen_${val.leadgen_id}_${payloadHash}`;
          else eventKey = `change_${field}_${payloadHash}`;

          events.push({ field, event_key: eventKey, payload: ch });
        }
      }

      if (events.length > 0) {
        for (const mapping of pageMappings) {
          const { error: rpcError } = await supabaseAdmin.rpc("ingest_meta_webhook_batch", {
            p_tenant_id: mapping.tenant_id,
            p_page_id: pageId,
            p_events: events,
            p_interactions: interactions,
            p_lead_interactions: leadInteractions,
          });

          if (rpcError) {
            logStructured({
              event: "ingest_failed",
              tenants_pending: pageMappings.length,
              code: rpcError.code ?? null,
            });
            return new Response("Internal Server Error: Database ingestion failed", { status: 500 });
          }
        }
      }
    }

    logStructured({ event: "events_accepted", received_batches: (payload.entry || []).length });
    return new Response("OK", { status: 200 });
  }

  return new Response("Method Not Allowed", { status: 405 });
});
