# Instant Zalo Outbound Send Actual Status

Title: Instant Zalo Outbound Send & Optimistic Bubble Rendering
Date: 2026-08-31
Status: All Complete (100% Quality Gates Passed)
Companion plan: `docs/plans/2026-08-31-instant-zalo-outbound-send/2026-08-31-instant-zalo-outbound-send-plan.md`
Companion evidence: `docs/plans/2026-08-31-instant-zalo-outbound-send/2026-08-31-instant-zalo-outbound-send-evidence.md`
Companion benchmark: `docs/plans/2026-08-31-instant-zalo-outbound-send/2026-08-31-instant-zalo-outbound-send-benchmark.md`

## Current Status Matrix

| Unit | Current State | Required State | Status | Relationship Count | Evidence | Next Plan Decision |
|---|---|---|---|---|---|---|
| `MessageComposer.handleSend` | Synchronously constructs optimistic `ConversationMessage` and calls `onBatchQueued` | Construct optimistic `ConversationMessage` and call `onBatchQueued` synchronously | passed | 8 related files | E1-P1A-COMPOSER | Completed |
| `ConversationWorkspace.handleBatchQueued` | Replaces negative IDs cleanly and syncs sidebar preview | Replaces negative IDs by `clientBatchId` or `sendBatchId` cleanly | passed | 12 related files | E1-P1A-WORKSPACE | Completed |
| `repository.enqueue_outbound_batch` | Calls `hydrate_messages` before committing transaction | Calls `hydrate_messages` before returning | passed | 18 related files | E1-P1B-REPO | Completed |
| `service.ingest_zalo_event` echo handling | Emits delta/invalidation on duplicate message echo updates | Emits delta/invalidation on message status/timestamp update | passed | 14 related files | E1-P1B-SERVICE | Completed |

## Status Refresh Log

| Refresh | Date | Repo Basis | Changed Scope | Status Changes | Evidence | Next Phase Update |
|---|---|---|---|---|---|---|
| R0 | 2026-08-31 | baseline before P0 | Composer send flow, Workspace batch handler, repository enqueue hydration, service echo delta | initial classification | E0-P0A-COMPOSER1..E0-P0A-SERVICE1 | proceed with P1-A |
| R1 | 2026-08-31 | post implementation | Full optimistic store render, hydration, echo deltas, and CI tests | all units passed | E1-P1A-COMPOSER..E1-P2B-GATES | Complete |
