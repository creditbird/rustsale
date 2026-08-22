// Supabase Edge Function: /meta-webhook
// Receives Meta platform webhooks (messages, feed, leadgen), verifies the
// X-Hub-Signature-256 HMAC against RAW request bytes before any JSON parsing,
// unpacks entry[] into durable buffer rows, and returns 200 immediately.
import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const META_APP_SECRET = Deno.env.get("META_APP_SECRET") || "";
const META_VERIFY_TOKEN = Deno.env.get("META_VERIFY_TOKEN") || "";
const SUPABASE_URL = Deno.env.get("SUPABASE_URL") || "";
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") || "";

const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

async function verifyHmacSha256(signatureHeader: string | null, rawBody: Uint8Array): Promise<boolean> {
  if (!signatureHeader || !signatureHeader.startsWith("sha256=")) return false;
  const signature = signatureHeader.slice(7);
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
  const url = new URL(req.url);

  if (req.method === "GET") {
    const mode = url.searchParams.get("hub.mode");
    const token = url.searchParams.get("hub.verify_token");
    const challenge = url.searchParams.get("hub.challenge");
    if (mode === "subscribe" && token === META_VERIFY_TOKEN) {
      return new Response(challenge, { status: 200 });
    }
    return new Response("Forbidden", { status: 403 });
  }

  if (req.method === "POST") {
    // Read raw bytes BEFORE parsing JSON so signature verification cannot be bypassed.
    const signature = req.headers.get("X-Hub-Signature-256");
    const rawBody = new Uint8Array(await req.arrayBuffer());

    const isValid = await verifyHmacSha256(signature, rawBody);
    if (!isValid) {
      return new Response("Invalid signature", { status: 403 });
    }

    let payload: any;
    try {
      payload = JSON.parse(new TextDecoder().decode(rawBody));
    } catch {
      return new Response("Invalid JSON", { status: 400 });
    }

    const entries = payload.entry || [];
    const rowsToInsert: any[] = [];

    for (const entry of entries) {
      const pageId = entry.id;

      if (entry.messaging) {
        for (const msg of entry.messaging) {
          if (msg.message?.mid) {
            rowsToInsert.push({
              page_id: pageId,
              event_type: msg.message.is_echo ? "message_echoes" : "messages",
              event_key: msg.message.mid,
              payload: msg,
              status: "pending",
            });
          } else if (msg.delivery) {
            rowsToInsert.push({
              page_id: pageId,
              event_type: "message_deliveries",
              event_key: `${msg.recipient.id}_${msg.delivery.watermark}`,
              payload: msg,
              status: "pending",
            });
          } else if (msg.read) {
            rowsToInsert.push({
              page_id: pageId,
              event_type: "message_reads",
              event_key: `${msg.recipient.id}_${msg.read.watermark}`,
              payload: msg,
              status: "pending",
            });
          } else if (msg.reaction) {
            rowsToInsert.push({
              page_id: pageId,
              event_type: "message_reactions",
              event_key: `${msg.reaction.mid}_${msg.sender.id}`,
              payload: msg,
              status: "pending",
            });
          }
        }
      }

      if (entry.changes) {
        for (const change of entry.changes) {
          if (
            change.field === "feed" &&
            change.value?.item === "comment" &&
            change.value?.verb === "add"
          ) {
            rowsToInsert.push({
              page_id: pageId,
              event_type: "feed",
              event_key: change.value.comment_id,
              payload: change,
              status: "pending",
            });
          } else if (change.field === "leadgen") {
            rowsToInsert.push({
              page_id: pageId,
              event_type: "leadgen",
              event_key: change.value.leadgen_id,
              payload: change,
              status: "pending",
            });
          }
        }
      }
    }

    if (rowsToInsert.length > 0) {
      await supabaseAdmin.from("meta_webhook_buffer").upsert(rowsToInsert, {
        onConflict: "page_id,event_type,event_key",
        ignoreDuplicates: true,
      });
    }

    // Ack fast (<1s) — Meta retries non-2xx with backoff.
    return new Response("EVENT_RECEIVED", { status: 200 });
  }

  return new Response("Method not allowed", { status: 405 });
});
