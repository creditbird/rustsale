// Supabase Edge Function: /meta-webhook
// Receives Meta platform webhooks (messages, feed, leadgen).
// Security invariants:
// 1. Missing server secrets FAIL CLOSED (500) — an empty-string HMAC key can
//    never be used to validate attacker-controlled signatures.
// 2. The X-Hub-Signature-256 HMAC is computed over RAW request bytes before
//    any JSON parsing, with strict 64-hex-char format validation.
// 3. Durable persistence failures return a RETRYABLE non-2xx so Meta redelivers
//    instead of silently dropping events.
// 4. One malformed event is skipped without poisoning the rest of the batch.
import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2.49.4";

const META_APP_SECRET = Deno.env.get("META_APP_SECRET") ?? "";
const META_VERIFY_TOKEN = Deno.env.get("META_VERIFY_TOKEN") ?? "";
const SUPABASE_URL = Deno.env.get("SUPABASE_URL") ?? "";
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") ?? "";

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
    // Read raw bytes BEFORE parsing JSON so signature verification cannot be bypassed.
    const signature = req.headers.get("X-Hub-Signature-256");
    const rawBody = new Uint8Array(await req.arrayBuffer());

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
    const rowsToInsert: any[] = [];
    let malformedSkipped = 0;

    const pushRow = (
      pageId: unknown,
      eventType: string,
      eventKey: unknown,
      payloadEvent: unknown,
    ): void => {
      if (
        typeof pageId !== "string" ||
        pageId.length === 0 ||
        typeof eventKey !== "string" ||
        eventKey.length === 0
      ) {
        malformedSkipped += 1;
        return;
      }
      rowsToInsert.push({
        page_id: pageId,
        event_type: eventType,
        event_key: eventKey,
        payload: payloadEvent,
        status: "pending",
      });
    };

    for (const entry of entries) {
      const pageId = entry?.id;

      if (Array.isArray(entry?.messaging)) {
        for (const msg of entry.messaging) {
          if (msg?.message?.mid) {
            pushRow(pageId, msg.message.is_echo ? "message_echoes" : "messages", msg.message.mid, msg);
          } else if (msg?.delivery && typeof msg.delivery?.watermark !== "undefined") {
            pushRow(
              pageId,
              "message_deliveries",
              `${msg.recipient?.id}_${msg.delivery.watermark}`,
              msg,
            );
          } else if (msg?.read && typeof msg.read?.watermark !== "undefined") {
            pushRow(pageId, "message_reads", `${msg.recipient?.id}_${msg.read.watermark}`, msg);
          } else if (msg?.reaction?.mid) {
            // Include action/emoji/timestamp so react -> unreact/change are
            // distinct logical events, while true Meta retries (identical
            // payloads) still dedupe.
            pushRow(
              pageId,
              "message_reactions",
              `${msg.reaction.mid}_${msg.sender?.id}_${msg.reaction.action ?? ""}_${msg.reaction.emoji ?? ""}_${msg.timestamp ?? ""}`,
              msg,
            );
          } else {
            malformedSkipped += 1;
          }
        }
      }

      if (Array.isArray(entry?.changes)) {
        for (const change of entry.changes) {
          if (
            change?.field === "feed" &&
            change.value?.item === "comment" &&
            change.value?.verb === "add"
          ) {
            pushRow(pageId, "feed", change.value.comment_id, change);
          } else if (change?.field === "leadgen") {
            pushRow(pageId, "leadgen", change.value?.leadgen_id, change);
          } else {
            malformedSkipped += 1;
          }
        }
      }
    }

    let persisted = 0;
    if (rowsToInsert.length > 0) {
      const { error: upsertErr, count } = await supabaseAdmin
        .from("meta_webhook_buffer")
        .upsert(rowsToInsert, {
          onConflict: "page_id,event_type,event_key",
          ignoreDuplicates: true,
          count: "exact",
        });
      if (upsertErr) {
        // Durable persistence failed: tell Meta to retry instead of acking loss.
        logStructured({
          event: "buffer_persist_failed",
          batch: rowsToInsert.length,
          error_code: upsertErr.code ?? null,
        });
        return new Response("Temporary storage failure", { status: 503 });
      }
      persisted = count ?? rowsToInsert.length;
    }

    logStructured({
      event: "events_accepted",
      received_batches: entries.length,
      accepted: persisted,
      duplicates_or_empty: rowsToInsert.length - persisted >= 0 ? rowsToInsert.length - persisted : 0,
      malformed_skipped: malformedSkipped,
    });

    // Ack fast (<1s) — Meta retries non-2xx with backoff.
    return new Response("EVENT_RECEIVED", { status: 200 });
  }

  return new Response("Method not allowed", { status: 405 });
});
