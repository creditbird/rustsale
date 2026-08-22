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
// 5. Every buffered row carries the tenant_id owning the Page at ingestion
//    time, so later Page ownership transfers cannot leak history across
//    tenants. Events for Pages unknown to the relay are acked but not stored.
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

type BufferRow = {
  tenant_id: string;
  page_id: string;
  event_type: string;
  event_key: string;
  payload: unknown;
  status: "pending";
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
    // Staged per entry page; every component must be a real string so no
    // "undefined_*" synthetic keys can reach the dedup index.
    type StagedRow = { tenant_id: string | null; row: Omit<BufferRow, "tenant_id"> };
    const staged: StagedRow[] = [];
    let malformedSkipped = 0;

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
        tenant_id: null,
        row: {
          page_id: pageId,
          event_type: eventType,
          event_key: eventKey,
          payload: payloadEvent,
          status: "pending",
        },
      });
    };

    const requireId = (value: unknown): string | null =>
      typeof value === "string" && value.length > 0 ? value : null;

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
              stageRow(
                pageId,
                "message_deliveries",
                `${recipientId}_${msg.delivery.watermark}`,
                msg,
              );
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

    let persisted = 0;
    let droppedUnmapped = 0;

    if (staged.length > 0) {
      // Resolve the CURRENT owner of each involved Page so rows are stamped
      // with the tenant that owned the Page at ingestion time.
      const involvedPages = [...new Set(staged.map((s) => s.row.page_id))];
      const { data: mappings, error: mapErr } = await supabaseAdmin
        .from("meta_tenant_pages")
        .select("page_id,tenant_id")
        .in("page_id", involvedPages);
      if (mapErr) {
        logStructured({ event: "tenant_lookup_failed" });
        return new Response("Temporary storage failure", { status: 503 });
      }
      const ownerByPage = new Map<string, string>(
        (mappings ?? []).map((m: { page_id: string; tenant_id: string }) => [m.page_id, m.tenant_id]),
      );

      const rowsToInsert: BufferRow[] = [];
      for (const item of staged) {
        const tenantId = ownerByPage.get(item.row.page_id);
        if (!tenantId) {
          droppedUnmapped += 1; // Page not connected to any RustSale tenant.
          continue;
        }
        rowsToInsert.push({ ...item.row, tenant_id: tenantId });
      }

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
    }

    logStructured({
      event: "events_accepted",
      received_batches: entries.length,
      accepted: persisted,
      duplicates_or_empty: staged.length - droppedUnmapped - persisted >= 0
        ? staged.length - droppedUnmapped - persisted
        : 0,
      malformed_skipped: malformedSkipped,
      dropped_unmapped_page: droppedUnmapped,
    });

    // Ack fast (<1s) — Meta retries non-2xx with backoff.
    return new Response("EVENT_RECEIVED", { status: 200 });
  }

  return new Response("Method not allowed", { status: 405 });
});
