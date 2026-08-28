# WhatsApp Conversation Media Preview Remediation

## Control Header

| Field | Value |
|---|---|
| Work item | Restore reliable WhatsApp image previews in the unified conversation timeline |
| Approval basis | Explicit user request on 2026-08-28 to view WhatsApp images like Zalo |
| Risk tier | Tier 3 — cross-layer ingestion, SQLite persistence, encrypted media download, and Svelte rendering |
| Out of scope | Reconstructing an E2EE `media_key` that was never persisted and is no longer available from WhatsApp |

## Problem Statement

WhatsApp image messages may retain only an encrypted CDN path while losing the E2EE media key. The current partial remediation also has two idempotency gaps: a redelivery with a changed CDN URL can create a second attachment instead of enriching the existing row, and an incomplete redelivery can remove previously persisted encryption metadata through an RFC 7396 null patch. The UI then keeps selecting an unresolved attachment and cannot render the image.

## Requirements and Acceptance Criteria

### WA-MEDIA-001 — Lossless media ingestion

- Extract image/video/audio/document/sticker download metadata from the WhatsApp protobuf.
- Persist `direct_path`, `media_key`, `file_sha256`, and `file_enc_sha256` in attachment metadata.
- Never persist a WhatsApp CDN path or URL in `local_path`.
- Evidence: protobuf-to-SQLite integration tests using a real `whatsapp-rust` message model.

### WA-MEDIA-002 — Idempotent redelivery enrichment

- Re-ingesting the same provider message must retain exactly one canonical attachment per media slot, even when the CDN URL changes.
- A richer redelivery must fill missing URL, thumbnail, dimensions, hashes, and media key.
- A poorer redelivery must never erase a previously stored media key, hashes, dimensions, local cache, or thumbnail.
- Evidence: adversarial redelivery tests in both rich-to-poor and poor-to-rich order.

### WA-MEDIA-003 — Safe legacy reconciliation

- Startup reconciliation must move legacy CDN values out of `local_path`, preserving a recoverable `direct_path` before clearing the polluted field.
- It must backfill encryption metadata from the stored raw WhatsApp payload when that payload contains the key.
- Reconciliation must be idempotent, transactionally safe, and scoped only to WhatsApp attachments.
- Evidence: migration tests covering recoverable and irrecoverable legacy rows plus second-run idempotency.

### WA-MEDIA-004 — Bounded media resolution and caching

- The download command must resolve an existing safe preview, a valid local cache, or encrypted WhatsApp media with complete metadata.
- Local and remote media reads must be bounded and reject oversized payloads before encoding them into a data URL.
- Successful downloads must persist a reusable preview and invalidate the relevant conversation.
- Missing E2EE metadata must return a stable, typed, non-retryable error; transient provider failures remain retryable.
- Evidence: command tests for cached, local, downloadable, oversized, missing-key, and raw-payload-backfill paths.

### WA-MEDIA-005 — Honest image UI parity with Zalo

- Resolvable WhatsApp images render inline and open in the existing image viewer just like Zalo images.
- The component auto-resolves encrypted media once and permits manual retry only for transient failures.
- Permanently unrecoverable historical images display a concise request-to-resend message and do not loop or pretend retry can recover a missing key.
- Structured Tauri errors, strings, and ordinary JavaScript errors must all normalize consistently.
- Evidence: Svelte component tests and frontend static checks.

### WA-MEDIA-006 — Cross-channel non-regression

- Zalo image rendering and existing conversation attachment behavior remain unchanged.
- The final diff and GitNexus affected flows must match the requirement matrix.
- Evidence: targeted Zalo/conversation tests, full conversation test suite, `git diff --check`, and final GitNexus change detection.

## Failure and Recovery Semantics

- A missing `media_key` is a permanent local-data condition. AES media keys cannot be derived from `direct_path`, hashes, or the session message secret.
- If WhatsApp later redelivers the original message with complete metadata, normal ingestion enriches the existing attachment and the normal invalidation path makes it viewable.
- If no redelivery is available, the sender must resend the image (or the account must perform a user-authorized fresh history import). The UI must state this instead of offering an endless retry.

## Verification Strategy

1. Add RED tests for URL-changing duplicate redelivery and rich-to-poor metadata preservation.
2. Implement the smallest transactional upsert that makes those tests green.
3. Add reconciliation and typed-error tests before changing those seams.
4. Run targeted Rust and Bun suites after each layer.
5. Run adversarial integration checks against a temporary SQLite database and audit the live database read-only.
6. Run final formatting, lint/static checks, affected tests, diff hygiene, and GitNexus scope analysis.
