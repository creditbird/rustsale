# Zalo Group Member Deep Scan Production Remediation Design

## Goal

Make hidden Zalo group-member discovery production-safe by coordinating account-wide WebSocket history through the existing supervisor, fully paginating Group Board and Poll sources, canonicalizing every discovered Zalo UID transactionally, and exposing explicit, stale-safe, diagnosable UI workflows without changing the current modal layout.

## Requirements

- **REQ-001 — Supervisor-owned history:** Deep Scan must request historical messages through `ZaloSessionSupervisor`; command 511 must never be sent directly from a Tauri command or a second listener path.
- **REQ-002 — Account-wide semantics:** Group history backfill is account-wide because command 511 carries no group ID. Backend and UI diagnostics must never claim that provider history was scoped to the selected group.
- **REQ-003 — Singleflight and bounded completion:** Only one message-history backfill may run per Zalo account. Concurrent callers join the active run. Each request burst remains bounded, resumes from persisted cursors, and returns a typed completed, timed-out, unavailable, or failed outcome.
- **REQ-004 — Durable cursor ownership:** The supervisor remains the only owner of message-history pagination state and persisted checkpoints. A completed in-memory stream may be explicitly re-armed from its durable cursor without resetting unrelated reaction streams or realtime watermarks.
- **REQ-005 — No false HTTP fallback:** Remove the unverified `api/group/history` Deep Scan loop. Malformed provider history messages must surface a parse error instead of being silently discarded.
- **REQ-006 — Complete Board pagination:** `api/board/list` must be fetched until exhaustion or a repeated-page safeguard. Fixed page-one or fixed-50 extraction is not acceptable.
- **REQ-007 — Board payload compatibility:** UID extraction must inspect both top-level `data` fields and decoded `data.params`, handle poll IDs represented as JSON numbers or numeric strings, and collect creators, owners, direct voters, poll-detail creators, and poll-detail voters.
- **REQ-008 — Partial-source diagnostics:** Board-page and Poll-detail failures must be retained as structured warnings while successful pages and items continue. Persistence failures remain fatal and must not be converted to warnings.
- **REQ-009 — Transactional UID canonicalization:** Every discovered UID must be normalized to `channel_identities.identity_namespace = 'zalo_uid'` inside a transaction. Empty and sentinel UIDs are rejected. Namespace collisions are merged deterministically.
- **REQ-010 — Reference migration:** Identity merging must preserve the canonical lead link and migrate conversation participants, message senders, message mentions, and other foreign-key references used by the feature before deleting or retiring duplicate identities.
- **REQ-011 — Complete local evidence:** Local member discovery must include active participants, message senders, mentions, reaction actors, and quote owners. Quote-owner identities must be persisted during message normalization so later scans can collect them.
- **REQ-012 — Safe import membership:** Batch import may only accept identities that belong to the requested conversation through canonical participant or locally observed message evidence. Display names and caller-supplied IDs are not authorization evidence.
- **REQ-013 — Deterministic lead linking:** Remove `leads.about LIKE` identity matching. Existing leads are resolved only through canonical channel identity links or another exact, explicit key. Duplicate request items are collapsed before mutation.
- **REQ-014 — Atomic import effects:** Lead creation/update, category creation/assignment, identity linking, link metadata, and result counts must commit atomically. SQL, link, and category failures must propagate instead of being ignored.
- **REQ-015 — Standard invalidation:** Successful imports must publish the normal conversation/lead invalidation events so every open view refreshes consistently.
- **REQ-016 — Explicit UI work:** Opening the modal loads local data only. Deep Scan starts only from the explicit button and must not run automatically after import.
- **REQ-017 — Stale-result isolation:** Closing the modal, switching conversations, or starting a newer request invalidates older asynchronous responses. Repeated Deep Scan clicks do not create duplicate backend work.
- **REQ-018 — Structured diagnostics contract:** TypeScript must expose every backend diagnostic field, including page counts, message counts, stop reason, source warnings, and backfill outcome. The UI distinguishes complete membership snapshots from UID evidence accumulated from local/account-wide sources.
- **REQ-019 — Safe export:** CSV cells beginning with `=`, `+`, `-`, or `@` after leading whitespace must be escaped to prevent spreadsheet formula execution. CSV quoting and UTF-8 BOM behavior remain correct.
- **REQ-020 — Accessible modal:** Preserve the current screenshot layout while adding dialog semantics, an accessible title/description, focus containment, Escape close, focus restoration, and usable loading/error announcements.
- **REQ-021 — Test-first delivery:** Every changed behavior must first have a failing automated test, followed by the smallest implementation that makes it pass.
- **REQ-022 — Legacy database startup compatibility:** Existing SQLite databases whose `messages` table predates `reply_sender_identity_id` must add the column before creating `idx_messages_reply_sender_identity`. Fresh installs and upgrades must both finish with the column, foreign key, and partial index present.

## Architecture

### Supervisor control plane

Add a control channel owned by `ZaloSessionSupervisor`. A Deep Scan command resolves the conversation's Zalo account and sends a `BackfillGroupMessages` request containing a completion responder and timeout budget. The supervisor serializes requests per account, joins concurrent callers to the same active run, and uses its connected client plus existing `HistoryPaginationState` to request command 511 pages.

The run starts from the persisted `group_message_history` cursor, or the group message watermark only when no history cursor exists. A stream previously marked complete may be re-armed for an explicit scan, but the supervisor does not delete checkpoints or restart reaction streams. Empty cursors, repeated cursors, provider exhaustion, cancellation, and timeout terminate deterministically. The returned report describes account-wide work and never promises that all pages belong to the selected group.

The Tauri command waits only up to the configured timeout. A timeout does not kill the connected supervisor or corrupt cursor state; it reports that account-wide synchronization continues or stopped at the last durable checkpoint. If the account has no running supervisor/client, the command returns a typed unavailable diagnostic and continues with local and Board/Poll evidence where safe.

### Provider history contract

Delete the Deep Scan dependency on `get_group_chat_history_with_cursor` and the unverified `api/group/history` endpoint. Restore strict deserialization of provider history batches: one malformed message makes the batch fail with contextual information instead of silently reducing counts.

Command 511 remains account-wide. Conversation-specific discovery happens only after normalized events are persisted and queried by `conversation_id` in SQLite.

### Board and Poll extraction

Replace `fetch_group_board_uids -> Vec<String>` with a structured report containing unique UIDs, pages attempted, pages succeeded, items inspected, polls attempted, polls succeeded, stop reason, and source warnings.

Fetch Board pages sequentially with a production count supported by the provider. Stop on an empty page, a short page, an explicit provider exhaustion signal when available, or a repeated page signature. Enforce a defensive maximum-page bound and report when the bound is reached.

For each item, inspect both `item.data` and decoded `item.data.params`. UID extraction accepts known creator/owner keys and voter arrays without treating arbitrary numeric fields as identities. Poll IDs accept positive integer JSON numbers and trimmed positive numeric strings. Each Poll detail failure adds a warning tagged with its poll ID; it does not discard UIDs collected from the Board item or other polls.

### Canonical identity persistence

Introduce a repository-level transactional canonicalization operation for Zalo UIDs rather than scattering `if let Ok` inserts through Tauri commands. It validates the UID, locates all `channel='zalo'` identities with the same external ID, selects or creates the `zalo_uid` canonical row, and merges useful profile/link metadata deterministically.

Before a duplicate identity is removed, references used by conversations are migrated to the canonical identity. Participant conflicts are coalesced without losing role, joined/left timestamps, or active membership. Message sender and mention references are updated. Lead-link metadata follows the canonical link contract rather than direct SQL shortcuts. Any migration failure rolls back the entire UID batch.

Quote owners already normalized from Zalo payloads must be persisted as canonical identities and referenced by the stored message/quote representation. The local discovery query then unions participants, senders, mentions, active reaction actors, and quote owners for only the selected conversation.

### Deep Scan orchestration

`sync_and_deep_scan_group_members` becomes an orchestrator with explicit stages:

1. Resolve and validate the selected Zalo group conversation and account.
2. Refresh the supported direct group snapshot when available.
3. Request or join supervisor-owned account-wide group-message backfill with a bounded wait.
4. Fetch all Board pages and Poll details.
5. Canonicalize and attach discovered UIDs transactionally.
6. Re-query selected-conversation local evidence after persistence.
7. Hydrate profiles in bounded chunks and persist successful profile updates.
8. Return members plus structured stage diagnostics.

Provider source failures may produce partial results when local state remains valid. Database, canonicalization, or invariant failures fail the command. Counts must represent committed results, not attempted mutations.

### Import transaction

The import request includes `conversation_id`. The backend resolves each item against canonical identities evidenced by that conversation, rejects foreign identities, collapses duplicates by canonical identity, and ignores caller-supplied names for membership validation.

Within one SQLite transaction, resolve or create the optional category, resolve exact existing lead links, create or update leads according to `skip_existing`, link identities through the standard link operation with source/confidence/timestamps, assign categories idempotently, and calculate committed counts. After commit, publish standard lead and conversation invalidations once.

### Frontend state model

Opening `GroupMembersModal` starts only local member and category reads. Deep Scan is a separate user action. The modal keeps a monotonically increasing request generation tied to `open` and `conversationId`; responses apply only when their generation still matches. Closing or switching conversations clears selection and invalidates pending results. The backend singleflight is the source of truth for duplicate scan suppression, while the button also disables duplicate local clicks.

The status area renders structured diagnostics: direct snapshot completeness, Board/Poll coverage, account-wide backfill outcome, locally observed message count, profile hydration count, warnings, and stop reason. Wording explicitly differentiates “full provider member snapshot” from “UIDs observed for this conversation.” Import completion refreshes local members only and does not trigger Deep Scan.

Keep existing dimensions, spacing, filters, colors, and member-table composition. Add semantic dialog behavior and screen-reader announcements without visually redesigning the modal.

### CSV safety

CSV export normalizes each value to text, prefixes dangerous spreadsheet-leading values with an apostrophe after detecting leading whitespace plus `=`, `+`, `-`, or `@`, doubles embedded quotes, wraps every field in quotes, and retains the UTF-8 BOM. Tests cover formulas, whitespace-prefixed formulas, commas, quotes, and line breaks.

## Diagnostics Contract

The scan response contains members and committed counts plus structured fields equivalent to:

- Direct snapshot: attempted, complete, count, warning.
- Board: pages/items/polls attempted and succeeded, stop reason, warnings.
- History backfill: account ID, scope=`account_group_messages`, joined-existing-run, outcome, pages requested/received when observable, durable cursor, stop reason, warning.
- Local evidence: messages scanned and source-specific UID counts.
- Profile hydration: attempted, succeeded, failed, warnings.
- Overall: partial-result flag and ordered warnings.

Diagnostics must not expose cookies, tokens, raw payloads, full phone numbers, or other provider secrets.

## Failure Behavior

- Missing or non-group conversations fail before provider or database mutation.
- A disconnected account returns local results with an explicit unavailable diagnostic; it never reports a successful backfill.
- Concurrent Deep Scans for one account join one supervisor run and receive compatible completion reports.
- Timeout preserves the last durable cursor and returns a partial diagnostic without spawning another command-511 loop.
- Repeated/empty cursors and defensive page bounds stop pagination with explicit reasons.
- Board or individual Poll failures preserve other successful provider evidence and appear as warnings.
- Malformed history messages fail their provider batch; they are never silently dropped.
- UID canonicalization and import database failures roll back their full transactions.
- Foreign or fabricated import identities are rejected before lead mutation.
- Stale frontend responses cannot overwrite state for a closed modal or another conversation.

## Acceptance Evidence

- Supervisor tests prove command 511 is issued only through the supervisor, explicit completed streams can re-arm from durable cursors, concurrent callers join one run, timeouts are bounded, and repeated/empty cursors terminate.
- Vendor tests prove command 511 remains account-wide and malformed history messages return contextual errors.
- Client tests prove multi-page Board extraction, decoded `data.params`, numeric and string poll IDs, creator/voter collection, repeated-page protection, partial Poll failures, and structured diagnostics.
- Repository/command tests prove collision-safe `zalo_uid` canonicalization, transactional reference migration, quote-owner persistence, rollback on SQL failure, and selected-conversation evidence isolation.
- Import tests prove exact membership validation, duplicate collapse, removal of `about LIKE`, exact lead reuse, atomic category/link mutations, metadata updates, error propagation, and invalidation after commit.
- Service contract tests prove all Rust diagnostic fields are represented in TypeScript.
- Frontend tests prove local-only open, explicit singleflight scan, no post-import scan, stale-response rejection, diagnostics wording, CSV formula protection, Escape/focus behavior, and accessible dialog semantics.
- Pixel measurements confirm the remediated modal preserves the approved screenshot layout.
- A legacy-schema regression reproduces the startup failure before the fix and proves migration adds `reply_sender_identity_id` before its dependent index; `bun run tauri dev` also starts successfully against the backup-protected real database.
- Focused tests pass before broader Rust, vendored crate, Bun, Svelte, release-build, formatting, clippy, diff, and GitNexus scope checks.

## Non-Goals

- Claiming a complete Zalo group roster when the provider hides direct membership and only local evidence is available.
- Adding a group ID to command 511 or reverse-engineering a new undocumented provider protocol.
- Replacing the existing conversation supervisor, repository, identity-link, or invalidation architecture.
- Redesigning the Group Members modal or changing unrelated conversation UI behavior.
- Importing users inferred only from arbitrary Board numeric fields, display names, or fuzzy lead text.
