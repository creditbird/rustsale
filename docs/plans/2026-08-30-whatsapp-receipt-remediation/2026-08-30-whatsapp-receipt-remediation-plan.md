# WhatsApp Receipt Remediation and Review Standards Alignment Plan

## Metadata

- Date: `2026-08-30`
- Status: `completed`
- Plan: `docs/plans/2026-08-30-whatsapp-receipt-remediation/2026-08-30-whatsapp-receipt-remediation-plan.md`
- Evidence: `docs/plans/2026-08-30-whatsapp-receipt-remediation/2026-08-30-whatsapp-receipt-remediation-evidence.md`
- Benchmark: `docs/plans/2026-08-30-whatsapp-receipt-remediation/2026-08-30-whatsapp-receipt-remediation-benchmark.md`
- Actual status: `docs/plans/2026-08-30-whatsapp-receipt-remediation/2026-08-30-whatsapp-receipt-remediation-actual-status.md`

## Goal

Resolve 100% of review findings (Standards 1-6 and Spec 1-5), closing all lock conditions under the Implementation Completion Harness to achieve `VERIFIED READY`.

## Problem

The previous review concluded `NOT READY` due to:
1. `git diff --check HEAD` exit code 2 (trailing whitespace in `supervisor_test.rs:396`).
2. Missing accounting for CRITICAL risk level (339 changed symbols, 34 affected, 21 tracked files) and GitNexus Completion Lock verification.
3. Test dependency failure not proving warning logging and listener liveness.
4. Scope ledger inaccuracies compared to diff from HEAD.
5. `ChannelSenderRegistry` & `ChannelCapabilities` dead code / speculative generality.
6. `start_account_bot` bloated with duplicate receipt/ack branches.
7. WhatsApp receipt query not filtering `direction = 'outbound'` or validating `receipt.chat_jid`.
8. Delivery state machine allowing invalid regressions (`read -> failed`, `failed -> read`).
9. Production transport dropping `ServerError` and `ServerAck(ack.error)`.
10. Partial completion risk where lead query error drops UI invalidations.
11. E4 protocol documentation / scenario alignment on aggregate batch status.

## Scope

In Scope:
- `src-tauri/src/conversation/whatsapp/normalizer.rs`: Enforce outbound direction requirement, validate `receipt.chat_jid` match against conversation, and enforce strict monotonic non-regressing delivery state machine.
- `src-tauri/src/conversation/whatsapp/supervisor.rs`: Unified `map_whatsapp_receipt_event`, map `ServerError` & `ack.error` to `failed`, make invalidation loop resilient to query errors.
- `src-tauri/src/conversation/whatsapp/normalizer_test.rs`: Outbound message test setup, full state machine transition matrix test on identical message, immunity of inbound messages against receipt mutation, mismatched chat_jid isolation.
- `src-tauri/src/conversation/whatsapp/supervisor_test.rs`: Remove trailing whitespace, add error logging and listener liveness verification test across sequential events.
- `src-tauri/src/conversation/adapter.rs` & `src-tauri/src/conversation/mod.rs`: Eliminate `ChannelCapabilities`, `ChannelSenderRegistry`, and `adapter_test.rs`.
- Scope Ledger, Implementation Completion Harness, and evidence ledgers.

Out of Scope:
- Modifying CRM lead tables or core billing logic.

## Requirement Ledger

| Requirement ID | Standard / Spec | Invariant | Positive Test Case | Negative Test Case | Required Evidence | Owner | Status |
|----------------|-----------------|-----------|--------------------|--------------------|-------------------|-------|--------|
| `REQ-STD-1` | Standard 1 | Zero whitespace defects across git diff | `git diff --check f937999..HEAD` returns exit code 0 | Trailing newline/space triggers check error | `E1-P1A-DIFF` | Backend | GREEN |
| `REQ-STD-2` | Standard 2 | Receipts only update `outbound` messages in matching conversation `chat_jid` | Delivery/read receipt on matching outbound message updates status | Inbound message or mismatched `chat_jid` receipt is ignored | `E2-P1B-NORM` | Backend | GREEN |
| `REQ-STD-3` | Standard 3 | Listener DB failure logs `warn!` and continues processing subsequent events on same boundary | First event fails with DB error, second event succeeds on same handler | Unhandled panic or crashed stream | `E4-P1D-LIVE` | Backend | GREEN |
| `REQ-STD-4` | Standard 4 | Monotonic delivery state machine without regressions | `sending -> sent -> delivered -> read` transitions persist monotonically | `read -> failed`, `failed -> read`, `delivered -> sent` rejected | `E2-P1B-NORM` | Backend | GREEN |
| `REQ-STD-5` | Standard 5 | Zero dead code or speculative abstractions | `cargo check --tests` compiles cleanly without unused traits/registries | Extraneous unused abstractions | `E5-P1E-DEAD` | Backend | GREEN |
| `REQ-STD-6` | Standard 6 | Unified `map_whatsapp_receipt_event` handling all receipt types | Receipt events (Read, Delivered, Played, ServerError) map cleanly | Unknown event variant returns None | `E3-P1C-SUP` | Backend | GREEN |
| `REQ-SPEC-1` | Spec 1 | Dependency-realistic ServerAck NACK (no `class`, server `from`, error `479`) maps to `failed` | No-class NACK with error `479` updates outbound status to `failed` and publishes invalidation | Stanza without error maps to `sent` | `E3-P1C-SUP` | Backend | GREEN |
| `REQ-SPEC-2` | Spec 2 | ServerAck with non-message class (e.g. `call`) or empty ID is rejected | Non-message class with error returns `None` | Message class with error returns Some(failed) | `E3-P1C-SUP` | Backend | GREEN |
| `REQ-SPEC-3` | Spec 3 | Participant lead query failure does not drop invalidations | Ingestion loop continues and publishes invalidation with `lead_id = None` | Loop early returns on optional lead error | `E3-P1C-SUP` | Backend | GREEN |
| `REQ-SPEC-4` | Spec 4 | `MessageBatchGroup.svelte` aggregate status rendering | Mixed batch displays lowest common status accurately | Single message regression | `E6-P1F-UI` | Frontend | GREEN |
| `REQ-SPEC-5` | Spec 5 | Zalo group offline backfill & self-UID normalization | Messages sent from mobile (`uid_from == own_uid`) recognize `is_self = true`, group chat history backfills on connect & periodically | Unrelated sender UID recognized as peer | `E7-P1F-ZALO` | Backend | GREEN |

## Acceptance Criteria

1. `git diff --check f937999..HEAD` exits with 0.
2. `WhatsAppNormalizer::ingest_receipt` only updates `outbound` messages belonging to the conversation matching `receipt.chat_jid`.
3. Delivery state machine rejects all regressions: `read` cannot become `failed/delivered/sent`, `failed` cannot become `read/delivered/sent`, `delivered` cannot become `sent/failed`, `removed` cannot be updated.
4. Production WhatsApp transport correctly maps `ReceiptType::ServerError` and `ServerAck` errors (including no-class NACKs) to `"failed"`.
5. Event listener processes errors cleanly with `warn!` and continues processing subsequent events without crashing or stalling.
6. No speculative generality or dead code in `adapter.rs`.
7. All cargo tests (backend) and bun tests (frontend) pass cleanly.
8. GitNexus change detection and impact analysis fully accounted for in Implementation Completion Harness report.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: establish the real current state.
  - Work Steps: inspect source files, record baseline findings in `actual-status.md`.
  - Implementation Gate: `actual-status.md` completed.
  - Acceptance: actual status accurately reflects the 11 findings.

### P1: Implementation and Verification Slices

- [x] P1-A: Clean trailing whitespace and satisfy `git diff --check HEAD`.
  - Goal: Ensure all files pass git diff check with 0 whitespace errors.
  - Work Steps: fix `src-tauri/src/conversation/whatsapp/supervisor_test.rs:396` and markdown EOFs.
  - Implementation Gate: `git diff --check f937999..HEAD` returns exit code 0.
  - Acceptance: 0 whitespace errors.

- [x] P1-B: Enforce WhatsApp receipt direction, conversation matching, and monotonic state machine.
  - Goal: Restrict receipts to outbound messages matching `chat_jid` and prevent status regressions.
  - Work Steps: update `normalizer.rs` query to check `m.direction = 'outbound'` and match `receipt.chat_jid`. Enforce monotonic transitions (`read` terminal, `failed` terminal, `delivered` cannot regress). Update `normalizer_test.rs` with comprehensive matrix test on identical message, outbound isolation, and chat_jid mismatch tests.
  - Implementation Gate: `cargo test conversation::whatsapp::normalizer_test` passes.
  - Acceptance: All state transitions verified and passing.

- [x] P1-C: Unify WhatsApp event mapping and support failed receipts / error acks (including no-class NACKs).
  - Goal: Extract `map_whatsapp_receipt_event`, map `ServerError` & `ack.error` (with/without class) to `failed`, simplify `start_account_bot`.
  - Work Steps: add `map_whatsapp_receipt_event` in `supervisor.rs`, update `start_account_bot` event loop, ensure resilient invalidation loop in `process_and_publish_whatsapp_receipt`.
  - Implementation Gate: `cargo check --tests` passes.
  - Acceptance: ServerError and error acks generate failed receipts cleanly.

- [x] P1-D: Add listener error handling and liveness proof test.
  - Goal: Verify listener handles DB failures with `warn!` and processes subsequent events normally.
  - Work Steps: add `test_whatsapp_receipt_listener_event_handling_logging_and_liveness` in `supervisor_test.rs`.
  - Implementation Gate: `cargo test conversation::whatsapp::supervisor_test` passes.
  - Acceptance: Logging verified and sequential event delivery proved.

- [x] P1-E: Remove dead code and speculative abstractions.
  - Goal: Eliminate `ChannelCapabilities` and `ChannelSenderRegistry` from `adapter.rs` and `mod.rs`.
  - Work Steps: revert `adapter.rs` additions, delete `adapter_test.rs`, remove module declaration in `mod.rs`.
  - Implementation Gate: `cargo check --tests` passes.
  - Acceptance: 0 dead code abstractions.

- [x] P1-F: Fix Zalo mobile-sent message detection and group chat history synchronization.
  - Goal: Fix `is_self` detection when messages are sent from mobile (`uid_from == own_uid`) in `zca-rs`, ensure group `thread_id` preservation, and implement startup & periodic active group history synchronization.
  - Work Steps: update `message.rs`, `event.rs`, `group.rs` in `zca-rs`, add `sync_active_group_histories` in `supervisor.rs` with startup and periodic tickers.
  - Implementation Gate: `cargo test` passes with 798 passed tests.
  - Acceptance: Group messages and self-sent mobile messages synced reliably.
