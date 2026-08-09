# Zalo Reaction Reliability Remediation Design

## Goal

Make Zalo reactions durable and recoverable when history and realtime events arrive before their target messages, preserve provider reaction icons without lossy `Unknown` conversion, and automatically resume historical reaction backfill until the provider reports exhaustion.

## Requirements

- **REQ-001 — Durable unmatched events:** A reaction whose target message is not yet present must be stored durably instead of discarded.
- **REQ-002 — Atomic replay:** When the matching message is inserted or reconciled, staged reactions must be applied and removed in the same SQLite transaction.
- **REQ-003 — Idempotency:** Duplicate history or realtime events must not duplicate actor counts or corrupt add/remove ordering.
- **REQ-004 — Safe checkpoints:** Reaction checkpoints may advance after a reaction is applied or durably staged, never after silent loss.
- **REQ-005 — Continuous bounded backfill:** Each request burst remains bounded, but paused history streams resume automatically while the account stays connected.
- **REQ-006 — Raw icon preservation:** Unknown provider `rIcon` strings must survive deserialization and normalization unchanged.
- **REQ-007 — Safe unsupported UI:** Unsupported reaction icons remain visible and show actor details, but never default to sending a Like reaction.
- **REQ-008 — Existing-data recovery:** Existing Zalo accounts receive a one-time reaction-history cursor reset after deployment so missing historical reactions can be replayed.
- **REQ-009 — Correct provider semantics:** Haha with `rType=0` is an add; an empty icon or negative type is a provider-wide remove.
- **REQ-010 — Selective desktop removal:** Removing one local emoji never calls the provider-wide remove operation and does not require provider message IDs.
- **REQ-011 — Self-echo and identity safety:** Successful outbound adds suppress one matching self-echo per account and message-ID alias; failures clear cache entries; local projection uses the real Zalo self UID.

## Architecture

### Durable pending reactions

Add `pending_message_reactions`, keyed by account, thread, provider event/message IDs, actor, reaction, and add/remove state. `ConversationRepository::apply_reaction` stages unresolved events and commits them before returning `None`.

`ConversationRepository::apply_normalized_message` queries pending rows matching either provider message ID or provider client-message ID, applies them chronologically through the same canonical reaction mutation helper, and deletes them before committing the message transaction.

### Historical backfill

Keep the existing five-page burst guard. Reaching the guard pauses the stream at the next cursor instead of marking it complete. A low-frequency supervisor ticker resumes paused streams. Empty or repeated cursors still terminate a stream.

### Provider icon contract

Replace lossy `#[serde(other)] EventReactions::Unknown` behavior with custom string serialization/deserialization and `Unknown(String)`. Known values retain their existing variants; unknown values retain the original provider string.

### Recovery migration

Create the pending table and add a one-time checkpoint marker per Zalo account. Accounts without the marker have only `direct_reaction_history` and `group_reaction_history` cursors deleted, then receive the marker. Message and realtime watermarks remain untouched.

### Desktop mutation semantics

Treat Haha with `rType=0` and a non-empty icon as an add. Treat an empty icon or negative type as a provider-wide remove. Desktop adds call Zalo, then persist locally after provider success. Desktop removal deletes only the selected canonical emoji locally and never invokes provider-wide removal.

Resolve the local actor from `channel_accounts.self_external_id`, falling back to `ZaloClient::own_external_id()` and persisting that UID before projection. Missing identity returns a typed error before any provider call. A five-second, account-scoped, consume-once cache suppresses successful self-echoes across both provider message-ID aliases and is cleared on provider failure.

## Failure Behavior

- Missing channel account is an ingest error; the supervisor degrades and does not advance the event checkpoint.
- Pending staging, replay, or checkpoint-reset failure aborts its transaction.
- Unknown icons are displayed as provider text and are read-only.
- Repeated cursor and empty-page safeguards prevent infinite pagination loops.

## Acceptance Evidence

- Repository regression tests prove reaction-before-message staging and replay through provider ID and client ID.
- Migration tests prove one-time cursor reset and idempotency.
- Vendor tests prove unknown icon round-trip preservation.
- Supervisor tests prove paused history resumes and terminates correctly.
- Frontend tests prove unsupported icons render without sending Like.
- Service and reaction tests prove selective remove preserves other emoji, works without provider IDs, makes no provider call, and returns the refreshed `includes_me` projection.
- Normalizer and echo-cache tests prove Haha `rType=0`, provider-wide removal, account isolation, alias cleanup, consume-once suppression, provider-failure cleanup, and real self-UID persistence.
- Focused Rust, vendor, Bun, Svelte check, formatting, and diff checks pass.

## Non-Goals

- Adding new writable Zalo reaction types beyond the six supported mutations.
- Sending live reactions or messages to external accounts during automated verification.
- Replacing the existing Conversation invalidation bridge.
