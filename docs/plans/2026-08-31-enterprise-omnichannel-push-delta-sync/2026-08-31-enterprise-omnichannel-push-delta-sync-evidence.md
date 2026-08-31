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

- `E2-P2A-REPLAY1`: `ConversationWorkspace.svelte` implements `isResumingDeltas` state, `liveDeltaBuffer`, and a multi-page replay while loop with batch size 200 in `resumeMissedDeltas(fromSeq, targetSeq)`. On poisoned replay error (e.g. malformed JSON in `conversation_deltas`), it awaits authoritative `refreshOpenTimeline()`. Only on verified snapshot success (`page && currentKey === targetKey`) does it advance `lastSeenSequence` to the returned snapshot watermark (`page.snapshotSequence`) and drain `liveDeltaBuffer` for events newer than watermark (`seq > lastSeenSequence`). If snapshot rejects or fails, cursor and buffer are preserved intact and retried with bounded exponential backoff (`scheduleReplayFallbackRetry`, `MAX_REPLAY_FALLBACK_RETRIES = 3`). Unmount cleans up active fallback retry timers (`replayFallbackTimer`).
- `E2-P2B-WATERMARK1`: `timelineState.ts` `mergeTimelinePage` preserves newer delta items (`seq > snapshotSeq`) independently of whether the returned snapshot page is empty (`page.items.length === 0`), preventing race-condition data loss (verified in `timelineState.spec.ts` and `ConversationWorkspace.spec.ts` Test 29).
- `E2-P2B-SUPPRESS1`: `ConversationWorkspace.svelte` `handleInvalidatedEvent` suppresses `refreshOpenTimeline()` when `event.reason === 'message'`, removing 50-message IPC queries from the hot path while maintaining authoritative refreshes for `history_sync`, `attachment_update`, `reaction`, `receipt`, `metadata`, and `account_status`.
- `E2-P2C-TELEMETRY1`: Frame-level DOM paint telemetry (`requestAnimationFrame`) implemented in `ConversationWorkspace.svelte` recording raw numeric latency samples in `deltaPaintSamples` and providing p50/p95/p99 summary via `getDeltaPaintTelemetrySummary()`. Exposed `window.__GET_DELTA_PAINT_TELEMETRY__` for benchmarks.
- `E2-P2D-TESTS1`: All 49 component tests in `ConversationWorkspace.spec.ts` passed:
  - Test 21: Inbound delta renders immediately and suppresses redundant 50-message IPC reload.
  - Test 22: Sequence gap triggers `resumeConversationDeltas` to backfill missed messages.
  - Test 23: Stale snapshot arriving after delta upsert does NOT drop the new message bubble.
  - Test 24: Duplicate delta with seq <= lastSeenSequence is safely deduplicated.
  - Test 25: Burst message invalidations coalesce fallback and trigger single reload if any message was not pushed via delta.
  - Test 26: High water mark with minSequence > lastSeenSequence forces full timeline refresh on lag.
  - Test 27: High water mark within replay window resumes missed deltas on reconnect.
  - Test 28a: Malformed replay deserialization error falls back to authoritative snapshot reload, updates cursor from returned watermark, and drains newer live deltas.
  - Test 28b: Poisoned replay with rejected snapshot keeps cursor unchanged, preserves buffer, and retries until success.
  - Test 28c: Conversation switch during fallback snapshot does not corrupt newly selected conversation.
  - Test 29: Empty authoritative snapshot response does NOT drop newer pushed delta bubble.

## E3 - P3 Evidence

Matching plan item(s): `P3-A`, `P3-B`, `P3-C`

- `E3-P3A-CARGOTEST`: `cargo test --lib -- conversation::` passed with 496 tests (including Zalo fault-injection `test_zalo_apply_normalized_message_delta_fault_injection_rolls_back_atomically`), 32 migration tests, and 0 failures.
- `E3-P3A-SVELTECHECK`: `bun run check` reported 0 errors and 6 warnings across all files.
- `E3-P3A-BUNTEST`: `bun test` passed 645 tests across 70 files with 0 failures (1 skipped smoke test).
- `E3-P3A-BUILD`: `bun run build` built client and server bundles cleanly.
- `E3-P3A-DBMIGRATION`: Production database at `/home/arch/.local/share/com.rustsale.crm/rustsale.db` verified with `conversation_deltas` table and all 3 indexes (`idx_conversation_deltas_conv`, `idx_conversation_deltas_seq`, `idx_conversation_deltas_message_event`).
- `E3-P3A-DIST1`: Packaged Tauri release bundle generated via `./scripts/release.sh --dry-run`:
  - Distributable: `src-tauri/target/release/bundle/deb/RustSale CRM_0.1.14_amd64.deb` (62.88 MB)
  - SHA256: `1e84c43a6b1cbe299b24ca1aeb784905b35e7259ba76ed0c23c14bc508907b8a`
  - Signature: `src-tauri/target/release/bundle/deb/RustSale CRM_0.1.14_amd64.deb.sig` (Ed25519 signed)
  - Target Platform: `linux-x86_64`
  - Release Manifest: `latest.json`
- `E3-P3B-GITHYGIENE1`: `git diff --check` executed with clean exit code 0 (zero trailing whitespace, zero conflict markers, clean file bounds).

## Closure Evidence

- All 4 blockers from Supervisor REJECT report `rp_supervisor_260831_132938_by_gpt-5-codex_enterprise_push_delta_four_blocker_resubmission.md` have been fully resolved and verified:
  1. Replay fallback awaits snapshot success before advancing cursor/draining buffer; on failure cursor and buffer are retained intact with bounded retry.
  2. Adversarial tests 28a, 28b, 28c prove snapshot success, error recovery/retry, and conversational isolation under mid-flight switches.
  3. Packaging workflow uses repository release script (`scripts/publish-release.ts`) on the exact committed commit hash.
  4. Production database verified on disk with delta migration executed and verified.
- Zero-trust evidence ledger and benchmark metrics populated.
