// Supabase Edge Function: /meta-webhook
// Receives Meta platform webhooks (messages, feed, leadgen).
// Security invariants:
// 1. Missing server secrets FAIL CLOSED (500) — an empty-string HMAC key can
//    never be used to validate attacker-controlled signatures.
// 2. The X-Hub-Signature-256 HMAC is computed over RAW request bytes before
//    any JSON parsing, with strict 64-hex-char format validation.
// 3. Durable persistence failures return a RETRYABLE non-2xx so Meta redelivers
//    instead of silently dropping events.
// 4. One malformed event is skipped without poisoning the rest of the batch,
//    and no synthetic "undefined_*" idempotency keys are ever persisted.
// 5. Persistence happens through the SERVICE-ROLE-ONLY RPC
//    stage_meta_webhook_events(), which resolves each Page's owner and inserts
//    rows inside ONE transaction while holding FOR SHARE locks on the mapping
//    rows — serialized against replace_meta_tenant_pages() so an event can
//    never be stored under the previous tenant after a takeover commits.
import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2.49.4";

const META_APP_SECRET = Deno.env.get("META_APP_SECRET") ?? "";
const META_VERIFY_TOKEN = Deno.env.get("META_VERIFY_TOKEN") ?? "";
const SUPABASE_URL = Deno.env.get("SUPABASE_URL") ?? "";
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") ?? "";
// Meta webhook payloads are small JSON documents; 1 MiB is generous.
const MAX_BODY_BYTES = 1_048_576;

const supabaseAdmin =
  SUPABASE_URL && SUPABASE_SERVICE_ROLE_KEY
    ? createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY)
    : null;

function logStructured(fields: Record<string, unknown>): void {
  // Redacted by construction: only counters/flags, never payloads or tokens.
  console.log(JSON.stringify({ fn: "meta-webhook", ...fields }));
}

async function verifyHmacSha256(
  signatureHeader: string | null,
  rawBody: Uint8Array,
): Promise<boolean> {
  if (!signatureHeader || !signatureHeader.startsWith("sha256=")) return false;
  const signature = signatureHeader.slice(7);
  // Strict structural check: exactly 64 hex chars (SHA-256 digest length).
  if (!/^[0-9a-fA-F]{64}$/.test(signature)) return false;
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(META_APP_SECRET),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["verify"],
  );
  const expectedSig = new Uint8Array(
    signature.match(/.{1,2}/g)?.map((byte) => parseInt(byte, 16)) || [],
  );
  return await crypto.subtle.verify("HMAC", key, expectedSig, rawBody);
}

type StagedEventRow = {
  page_id: string;
  event_type: string;
  event_key: string;
  payload: unknown;
};

serve(async (req: Request) => {
  // Fail closed when the deployment is misconfigured.
  if (!supabaseAdmin || !META_APP_SECRET) {
    logStructured({ event: "server_misconfigured" });
    return new Response("Server configuration error", { status: 500 });
  }

  const url = new URL(req.url);

  if (req.method === "GET") {
    const mode = url.searchParams.get("hub.mode");
    const token = url.searchParams.get("hub.verify_token");
    const challenge = url.searchParams.get("hub.challenge");
    if (!META_VERIFY_TOKEN) {
      logStructured({ event: "server_misconfigured", detail: "verify_token_missing" });
      return new Response("Server configuration error", { status: 500 });
    }
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

    // Read raw bytes BEFORE parsing JSON so signature verification cannot be bypassed.
    const signature = req.headers.get("X-Hub-Signature-256");
    const rawBody = new Uint8Array(await req.arrayBuffer());
    if (rawBody.byteLength > MAX_BODY_BYTES) {
      logStructured({ event: "body_too_large", actual_length: rawBody.byteLength });
      return new Response("Payload too large", { status: 413 });
    }

    const isValid = await verifyHmacSha256(signature, rawBody);
    if (!isValid) {
      logStructured({ event: "signature_rejected" });
      return new Response("Invalid signature", { status: 403 });
    }

    let payload: any;
    try {
      payload = JSON.parse(new TextDecoder().decode(rawBody));
    } catch {
      return new Response("Invalid JSON", { status: 400 });
    }

    const entries = Array.isArray(payload?.entry) ? payload.entry : [];
    const staged: StagedEventRow[] = [];
    let malformedSkipped = 0;

    const requireId = (value: unknown): string | null =>
      typeof value === "string" && value.length > 0 ? value : null;

    const stageRow = (
      pageId: unknown,
      eventType: string,
      eventKey: unknown,
      payloadEvent: unknown,
    ): void => {
      if (
        typeof pageId !== "string" ||
        pageId.length === 0 ||
        typeof eventKey !== "string" ||
        eventKey.length === 0 ||
        /\bundefined\b|\bnull\b/.test(eventKey)
      ) {
        malformedSkipped += 1;
        return;
      }
      staged.push({
        page_id: pageId,
        event_type: eventType,
        event_key: eventKey,
        payload: payloadEvent,
      });
    };

    for (const entry of entries) {
      const pageId = requireId(entry?.id);

      if (Array.isArray(entry?.messaging)) {
        for (const msg of entry.messaging) {
          if (msg?.message?.mid) {
            stageRow(pageId, msg.message.is_echo ? "message_echoes" : "messages", msg.message.mid, msg);
            continue;
          }

          if (msg?.delivery && typeof msg.delivery?.watermark !== "undefined") {
            const recipientId = requireId(msg.recipient?.id);
            if (recipientId && typeof msg.delivery.watermark !== "object") {
              stageRow(pageId, "message_deliveries", `${recipientId}_${msg.delivery.watermark}`, msg);
              continue;
            }
            malformedSkipped += 1;
            continue;
          }

          if (msg?.read && typeof msg.read?.watermark !== "undefined") {
            const recipientId = requireId(msg.recipient?.id);
            if (recipientId && typeof msg.read.watermark !== "object") {
              stageRow(pageId, "message_reads", `${recipientId}_${msg.read.watermark}`, msg);
              continue;
            }
            malformedSkipped += 1;
            continue;
          }

          if (msg?.reaction?.mid) {
            const senderId = requireId(msg.sender?.id);
            if (senderId) {
              // Include action/emoji/timestamp so react -> unreact/change are
              // distinct logical events, while true Meta retries (identical
              // payloads) still dedupe.
              stageRow(
                pageId,
                "message_reactions",
                `${msg.reaction.mid}_${senderId}_${msg.reaction.action ?? ""}_${msg.reaction.emoji ?? ""}_${msg.timestamp ?? ""}`,
                msg,
              );
              continue;
            }
            malformedSkipped += 1;
            continue;
          }

          malformedSkipped += 1;
        }
      }

      if (Array.isArray(entry?.changes)) {
        for (const change of entry.changes) {
          if (
            change?.field === "feed" &&
            change.value?.item === "comment" &&
            change.value?.verb === "add"
          ) {
            stageRow(pageId, "feed", change.value.comment_id, change);
          } else if (change?.field === "leadgen") {
            stageRow(pageId, "leadgen", change.value?.leadgen_id, change);
          } else {
            malformedSkipped += 1;
          }
        }
      }
    }

    let accepted = 0;
    let droppedUnmapped = 0;

    if (staged.length > 0) {
      // Single-transaction persistence: the SERVICE-ROLE-ONLY RPC locks the
      // involved mapping rows FOR SHARE, resolves owners, and inserts rows
      // stamped with ingestion-time tenancy — fully serialized against
      // ownership takeover.
      const { data: result, error: rpcErr } = await supabaseAdmin.rpc(
        "stage_meta_webhook_events",
        { p_rows: staged },
      );
      if (rpcErr || !result) {
        // Durable persistence failed: tell Meta to retry instead of acking loss.
        logStructured({
          event: "buffer_persist_failed",
          batch: staged.length,
          error_code: rpcErr?.code ?? null,
        });
        return new Response("Temporary storage failure", { status: 503 });
      }
      accepted = typeof result.accepted === "number" ? result.accepted : 0;
      droppedUnmapped =
        typeof result.dropped_unmapped === "number" ? result.dropped_unmapped : 0;
    }

    logStructured({
      event: "events_accepted",
      received_batches: entries.length,
      accepted,
      duplicates_or_empty: Math.max(0, staged.length - droppedUnmapped - accepted),
      malformed_skipped: malformedSkipped,
      dropped_unmapped_page: droppedUnmapped,
    });

    // Ack fast (<1s) — Meta retries non-2xx with backoff.
    return new Response("EVENT_RECEIVED", { status: 200 });
  }

  return new Response("Method not allowed", { status: 405 });
});
