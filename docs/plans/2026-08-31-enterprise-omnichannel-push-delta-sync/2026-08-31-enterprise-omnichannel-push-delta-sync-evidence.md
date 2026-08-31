# Enterprise Omnichannel Push-Delta Sync Evidence Ledger

## Metadata

- Date: `2026-08-31`
- Plan: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-plan.md`
- Evidence: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-evidence.md`
- Benchmark: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-benchmark.md`
- Actual status: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-actual-status.md`

## Evidence Rules

The evidence file explains why the work is known to be correct.

It contains:
- Baseline inspection evidence (`E0`).
- Phase-specific evidence (`E1`, `E2`, `E3`).
- Closure evidence and detect-changes verification.

### Evidence ID Naming

Format: `E<phase>-<item>-<kind><n>`

## E0 - P0 Evidence

Matching plan item(s): `P0-A`

- `E0-P0A-SRC1`: `src-tauri/src/conversation/events.rs` lines 42, 56-57 uses volatile `AtomicU64` in memory without database persistence or replay.
- `E0-P0A-SRC2`: `src-tauri/src/conversation/schema.sql` and `repository.rs` currently lack a `conversation_deltas` change sequence table.
- `E0-P0A-SRC3`: `src-tauri/src/conversation/service.rs` lines 610-628 publishes invalidation and delta, but sequence is not transactionally bound to message insertion.
- `E0-P0A-SRC4`: `src-tauri/src/conversation/whatsapp/supervisor.rs` lines 396-421 only publishes `ConversationInvalidatedEvent`.
- `E0-P0A-SRC5`: `src-tauri/src/conversation/facebook/supervisor.rs` lines 345-364 only publishes `ConversationInvalidatedEvent`.
- `E0-P0A-SRC6`: `src/lib/components/conversation/timelineState.ts` lines 131-148 drops non-queued/non-sending messages absent from fetched refresh page.
- `E0-P0A-SRC7`: `src/lib/services/conversationService.ts` lines 603-658 lacks delta replay and sequence resume methods.
- `E0-P0A-SRC8`: `src/lib/components/conversation/ConversationWorkspace.svelte` lines 1228-1235 only logs delta sequence and lines 1327-1335 triggers 50-message IPC reload on message invalidation.

## E1 - P1 Evidence

Matching plan item(s): `P1-A`, `P1-B`, `P1-C`, `P1-D`

- `E1-P1A-TX1`: `record_message_delta_tx` implemented in `repository.rs` and called inside `apply_normalized_message` and outreach message inserts within the exact same SQLite transaction `tx` before `tx.commit()?`.
- `E1-P1A-TEST1`: Unit tests `test_apply_normalized_message_atomically_records_delta` and `test_delta_transaction_rollback_guarantee` in `repository_test.rs` pass, proving atomic rollback and zero orphan messages.
- `E1-P1B-SEQ1`: Removed `AtomicU64` and `next_sequence()` from `ConversationEventPublisher` in `events.rs`. 100% delta event sequences originate from SQLite `AUTOINCREMENT` sequence in `conversation_deltas`.
- `E1-P1C-FB1`: `FacebookMessengerNormalizer::ingest_conversations_batch` and `ingest_webhook_event` transactionally record and return deltas for all messages in a batch.
- `E1-P1C-REASONS1`: Added `HistorySync` and `AttachmentUpdate` to `ConversationInvalidationReason` in `events.rs` and `conversationService.ts`. WhatsApp, Zalo history sync, and attachment hydration emit these distinct reasons and are never suppressed by the frontend.
- `E1-P1D-REPLAY1`: `test_replay_deltas_large_gap_pagination` in `repository_test.rs` validates paginated replay across >250 events with high-water mark metadata.
- `E1-P1D-BURST1`: Integration test `test_burst_300_events_persists_monotonic_sequence_and_replays_lagged_events` in `service_test.rs` ingested 300 burst messages, verified monotonic gapless sequences 1..300 in SQLite, and verified replay of lagged events with 100% data fidelity and 0 dropped bubbles.

## E2 - P2 Evidence

Matching plan item(s): `P2-A`, `P2-B`, `P2-C`, `P2-D`

- `E2-P2A-REPLAY1`: `ConversationWorkspace.svelte` implements `isResumingDeltas` state, `liveDeltaBuffer`, and a multi-page replay while loop with batch size 200 in `resumeMissedDeltas(fromSeq, targetSeq)`.
- `E2-P2B-WATERMARK1`: `timelineState.ts` `mergeTimelinePage` accepts `deltaWatermarkIds?: Set<number>`, ensuring stale snapshots never overwrite or discard newly received deltas.
- `E2-P2B-SUPPRESS1`: `ConversationWorkspace.svelte` `handleInvalidatedEvent` suppresses `refreshOpenTimeline()` when `event.reason === 'message'`, removing 50-message IPC queries from the hot path while maintaining authoritative refreshes for `history_sync`, `attachment_update`, `reaction`, `receipt`, `metadata`, and `account_status`.
- `E2-P2C-TELEMETRY1`: Frame-level DOM paint telemetry (`requestAnimationFrame`) implemented in `ConversationWorkspace.svelte` recording raw numeric latency samples in `deltaPaintSamples` and providing p50/p95/p99 summary via `getDeltaPaintTelemetrySummary()`. Exposed `window.__GET_DELTA_PAINT_TELEMETRY__` for benchmarks.
- `E2-P2D-TESTS1`: All 42 component tests in `ConversationWorkspace.spec.ts` passed, including:
  - Test 21: Inbound delta renders immediately and suppresses redundant 50-message IPC reload.
  - Test 22: Sequence gap triggers `resumeConversationDeltas` to backfill missed messages.
  - Test 23: Stale snapshot arriving after delta upsert does NOT drop the new message bubble.
  - Test 24: Duplicate delta with seq <= lastSeenSequence is safely deduplicated.

## E3 - P3 Evidence

Matching plan item(s): `P3-A`, `P3-B`, `P3-C`

- `E3-P3A-CARGOTEST`: `cargo test --package rustsale-crm --lib conversation` passed with 490 passed, 0 failed, 1 ignored in `src-tauri`.
- `E3-P3A-SVELTECHECK`: `bun run check` reported 0 errors across all files.
- `E3-P3A-BUNTEST`: `bun test` passed 635 tests across 70 files with 0 failures.
- `E3-P3A-BUILD`: `bun run build` built client and server bundles cleanly in 13.83s / 32.85s.
- `E3-P3B-GITHYGIENE1`: `git diff --check` executed with clean exit code 0 (zero trailing whitespace, zero conflict markers, clean file bounds).

## Closure Evidence

- All 8 supervisor reject blockers have been comprehensively resolved and verified with zero-trust evidence.
- Zero-trust evidence ledger and benchmark metrics populated.
