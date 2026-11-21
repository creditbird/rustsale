# Proposed v2 patch: Facebook Messenger sync loop (not applied)

`v2-facebook-sync-fix.patch` applies cleanly (`git apply --check`) on RustSale_v2 `ffc4dc4`
(branch `feat/read-attachment-webhook-atts`). It was generated from the v3 commit that fixed the
same files in `crates/rustsale-core`; v2's `normalizer.rs`, `error.rs`, tests and `mod.rs` were
byte-identical before the change, `supervisor.rs` differed only in two `tauri::async_runtime` lines.
Not compiled inside v2 and not deployed.

## Root cause

`normalizer.rs` inserted attachments with `ON CONFLICT(message_id, remote_url) ... DO NOTHING`, but
`message_attachments` also has `uq_message_attachments_provider` on `(message_id, provider_attachment_id)`.
Each poll re-reads the latest messages of every thread; Facebook returns a freshly signed CDN URL
for the same attachment id, so the URL target never matched and the provider-id index raised
`UNIQUE constraint failed: message_attachments.message_id, message_attachments.provider_attachment_id`.
The error aborted the whole batch transaction (nothing saved, checkpoint not advanced) and the next
tick, 20-30 s later, failed the same way. The same error also fired for one message that listed the
same attachment id twice. `last_error` was only written for fetch errors, so the UI showed a healthy
channel while sync was dead.

## What the patch changes

1. `normalizer.rs`: `ON CONFLICT DO NOTHING` without a target (both inserts).
2. `error.rs`: HTTP 429 without a Graph code is `RateLimited` (it was `Other`).
3. `backoff.rs` (new) + `supervisor.rs`: per account 30 s doubling to 15 min (5 min floor when throttled);
   `error!` only for the first failure of a streak, a changed error or every 20th repeat, else `debug!`;
   one `AccountStatus` event when the account goes bad and one on recovery; storage failures now set
   `channel_accounts.last_error`; "sync now" (`sync_page`) bypasses pacing. Auth failures still mark the
   account `invalid`, which already stops polling.
4. Tests: 3 normalizer regressions (rotated URL, repeated id, distinct ids), 9 backoff, 2 error
   classification, 5 supervisor (pacing, 429, recovery once, auth, storage failure).

Rolling it out needs no data migration: existing rows are valid, the next poll simply succeeds.
