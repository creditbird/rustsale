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

let supabaseAdmin: ReturnType<typeof createClient> | null = null;

function serverMisconfigured(): Response {
  logStructured({ event: "server_misconfigured" });
  return new Response("Server configuration error", { status: 500 });
}

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

async function verifyMetaSignature(req: Request, rawBodyBytes: Uint8Array): Promise<boolean> {
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

  // Verify against the ORIGINAL request bytes — no decode/re-encode round trip.
  return await crypto.subtle.verify("HMAC", key, signatureBytes, rawBodyBytes);
}

// Malformed timestamps must not poison an otherwise valid delivery.
function safeTimestampIso(value: unknown): string {
  try {
    const d = new Date(value as any);
    if (!Number.isNaN(d.getTime())) return d.toISOString();
  } catch {
    // fall through
  }
  return new Date().toISOString();
}

// Messenger messaging events carry millisecond epoch timestamps.
function messengerTimestampIso(value: unknown): string {
  return safeTimestampIso(value);
}

// Page feed / leadgen webhooks carry Unix-SECONDS created_time values; a bare
// number must be scaled to milliseconds or it decodes to January 1970.
function changeTimestampIso(value: unknown): string {
  if (typeof value === "number" && Number.isFinite(value)) {
    return safeTimestampIso(value * 1000);
  }
  if (typeof value === "string" && /^\d+$/.test(value.trim())) {
    return safeTimestampIso(Number(value.trim()) * 1000);
  }
  return safeTimestampIso(value);
}

serve(async (req: Request) => {
  if (
    !SUPABASE_URL ||
    !SUPABASE_SERVICE_ROLE_KEY ||
    !META_APP_SECRET ||
    !META_VERIFY_TOKEN
  ) {
    return serverMisconfigured();
  }
  if (!supabaseAdmin) {
    supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);
  }

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

    // Read the ORIGINAL raw bytes once: size bound and HMAC both operate on
    // bytes (UTF-16 String.length is not a wire-byte measure), decoding only
    // happens after signature verification succeeds.
    const rawBodyBytes = new Uint8Array(await req.arrayBuffer());
    if (rawBodyBytes.byteLength > MAX_BODY_BYTES) {
      logStructured({ event: "body_too_large", actual_length: rawBodyBytes.byteLength });
      return new Response("Payload too large", { status: 413 });
    }

    const isValid = await verifyMetaSignature(req, rawBodyBytes);
    if (!isValid) {
      logStructured({ event: "signature_rejected" });
      return new Response("Invalid signature", { status: 401 });
    }

    let payload: any;
    try {
      payload = JSON.parse(new TextDecoder().decode(rawBodyBytes));
    } catch {
      return new Response("Invalid JSON", { status: 400 });
    }

    let malformedSkipped = 0;
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

      const messagingEntries = [
        ...(Array.isArray(entry.messaging) ? entry.messaging : []),
        ...(Array.isArray(entry.standby) ? entry.standby : []),
      ];

      if (messagingEntries.length > 0) {
        for (const msg of messagingEntries) {
          const senderId =
            typeof msg.sender?.id === "string" && msg.sender.id.length > 0 ? msg.sender.id : null;
          const timestamp = safeTimestampIso(msg.timestamp);
          if (senderId && senderId !== pageId) {
            interactions.push({ user_id: senderId, app_user_id: null, timestamp });
          }

          let eventKey = "";
          if (msg.message?.mid) eventKey = `msg_${msg.message.mid}`;
          else if (
            msg.delivery?.mids &&
            Array.isArray(msg.delivery.mids) &&
            msg.delivery.mids.length > 0 &&
            typeof msg.delivery.watermark !== "undefined"
          ) eventKey = `del_${msg.delivery.mids.join("_")}_${msg.delivery.watermark}`;
          else if (msg.read?.watermark && senderId) eventKey = `read_${senderId}_${msg.read.watermark}`;
          else if (msg.reaction?.mid && senderId) {
            // Include action+emoji so react -> unreact/change remain distinct
            // logical events; true retries carry identical payloads and dedupe.
            eventKey = `react_${msg.reaction.mid}_${senderId}_${msg.reaction.action ?? ""}_${msg.reaction.emoji ?? ""}_${msg.reaction.reaction || "none"}`;
          } else if (senderId) eventKey = `misc_${senderId}_${await computeSha256(canonicalJsonStringify(msg))}`;
          else {
            malformedSkipped += 1;
            continue; // No durable identity components: skip instead of synthesizing keys.
          }

          events.push({ field: "messages", event_key: eventKey, payload: msg });
        }
      }

      if (entry.changes) {
        for (const ch of entry.changes) {
          const val = ch.value || {};
          const field = typeof ch.field === "string" && ch.field.length > 0 ? ch.field : null;
          if (!field) {
            malformedSkipped += 1;
            continue; // field is NOT NULL in the buffer schema.
          }
          const fromId = val.from?.id;
          const timestamp = changeTimestampIso(val.created_time);

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

        // Broadcast realtime notification to connected clients for instant UI update via REST API
        try {
          const realtimeUrl = `${SUPABASE_URL.replace(/\/$/, "")}/realtime/v1/api/broadcast`;
          const broadcastResp = await fetch(realtimeUrl, {
            method: "POST",
            headers: {
              "apikey": SUPABASE_SERVICE_ROLE_KEY,
              "Authorization": `Bearer ${SUPABASE_SERVICE_ROLE_KEY}`,
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              messages: [
                {
                  topic: "meta-realtime-channel",
                  event: "meta_message",
                  payload: {
                    page_id: pageId,
                    event_count: events.length,
                    events: events.map((e) => e.payload),
                    timestamp: new Date().toISOString(),
                  },
                },
                {
                  topic: "realtime:meta-realtime-channel",
                  event: "meta_message",
                  payload: {
                    page_id: pageId,
                    event_count: events.length,
                    events: events.map((e) => e.payload),
                    timestamp: new Date().toISOString(),
                  },
                },
              ],
            }),
          });
          if (!broadcastResp.ok) {
            logStructured({
              event: "broadcast_http_failed",
              status: broadcastResp.status,
              statusText: broadcastResp.statusText,
            });
          }
        } catch (bErr) {
          console.warn("[meta-webhook] Failed to broadcast realtime event via REST API:", bErr);
        }
      }
    }

    logStructured({
      event: "events_accepted",
      received_batches: (payload.entry || []).length,
      malformed_skipped: malformedSkipped,
    });
    return new Response("OK", { status: 200 });
  }

  return new Response("Method Not Allowed", { status: 405 });
});
