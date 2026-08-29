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

## Acceptance Criteria

1. `git diff --check HEAD` exits with 0.
2. `WhatsAppNormalizer::ingest_receipt` only updates `outbound` messages belonging to the conversation matching `receipt.chat_jid`.
3. Delivery state machine rejects all regressions: `read` cannot become `failed/delivered/sent`, `failed` cannot become `read/delivered/sent`, `delivered` cannot become `sent/failed`, `removed` cannot be updated.
4. Production WhatsApp transport correctly maps `ReceiptType::ServerError` and `ServerAck` errors to `"failed"`.
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
  - Work Steps: fix `src-tauri/src/conversation/whatsapp/supervisor_test.rs:396`.
  - Implementation Gate: `git diff --check HEAD` returns exit code 0.
  - Acceptance: 0 whitespace errors.

- [x] P1-B: Enforce WhatsApp receipt direction, conversation matching, and monotonic state machine.
  - Goal: Restrict receipts to outbound messages matching `chat_jid` and prevent status regressions.
  - Work Steps: update `normalizer.rs` query to check `m.direction = 'outbound'` and match `receipt.chat_jid`. Enforce monotonic transitions (`read` terminal, `failed` terminal, `delivered` cannot regress). Update `normalizer_test.rs` with comprehensive matrix test on identical message, outbound isolation, and chat_jid mismatch tests.
  - Implementation Gate: `cargo test conversation::whatsapp::normalizer_test` passes.
  - Acceptance: All state transitions verified and passing.

- [x] P1-C: Unify WhatsApp event mapping and support failed receipts / error acks.
  - Goal: Extract `map_whatsapp_receipt_event`, map `ServerError` & `ack.error` to `failed`, simplify `start_account_bot`.
  - Work Steps: add `map_whatsapp_receipt_event` in `supervisor.rs`, update `start_account_bot` event loop, ensure resilient invalidation loop in `process_and_publish_whatsapp_receipt`.
  - Implementation Gate: `cargo check --tests` passes.
  - Acceptance: ServerError and error acks generate failed receipts cleanly.

- [x] P1-D: Add listener error handling and liveness proof test.
  - Goal: Verify listener handles DB failures with `warn!` and processes subsequent events normally.
  - Work Steps: add `test_whatsapp_receipt_listener_error_handling_and_liveness` in `supervisor_test.rs`.
  - Implementation Gate: `cargo test conversation::whatsapp::supervisor_test` passes.
  - Acceptance: Logging verified and sequential event delivery proved.

- [x] P1-E: Remove dead code and speculative abstractions.
  - Goal: Eliminate `ChannelCapabilities` and `ChannelSenderRegistry` from `adapter.rs` and `mod.rs`.
  - Work Steps: revert `adapter.rs` additions, delete `adapter_test.rs`, remove module declaration in `mod.rs`.
  - Implementation Gate: `cargo check --tests` passes.
  - Acceptance: 0 dead code abstractions.

- [x] P1-F: Final quality gates, GitNexus reindexing, and Completion Lock verification.
  - Goal: Execute full validation suite, reindex GitNexus, and produce authoritative `VERIFIED READY` report.
  - Work Steps: run `cargo fmt --check`, `cargo test`, `bun run test:unit`, `bunx gitnexus analyze`, `detect_changes`.
  - Implementation Gate: All tests green, diff clean.
  - Acceptance: `VERIFIED READY` state achieved.
