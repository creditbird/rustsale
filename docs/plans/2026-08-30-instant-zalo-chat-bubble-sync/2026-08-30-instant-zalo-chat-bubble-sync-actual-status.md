# Instant Zalo Chat Bubble Synchronization Actual Status

Title: Instant Zalo Chat Bubble Synchronization
Date: 2026-08-30
Status: P1/P2 Complete
Companion plan: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-plan.md`
Companion evidence: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-evidence.md`
Companion benchmark: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-benchmark.md`

## Purpose

This file records the real current state before and after implementation.

## Scope

Target scope:
- `src-tauri/src/conversation/repository.rs` (`lookup_message_invalidation_info`)
- `src-tauri/src/conversation/service.rs` (`ingest_zalo_event` & invalidation event payload resolution)
- `src/lib/components/conversation/ConversationWorkspace.svelte` (`eventAffectsItem`, `handleInvalidatedEvent`)
- `src/lib/services/conversationService.ts` (`onConversationInvalidated`, `ensureConversationEventBridgeConnected`)

## Current Status Matrix

| Unit | Current State | Required State | Status | Relationship Count | Evidence | Next Plan Decision |
|---|---|---|---|---|---|---|
| `lookup_message_invalidation_info` | `ORDER BY (ci.lead_id IS NOT NULL) DESC, (ci.id = m.sender_identity_id) DESC, ci.lead_id DESC LIMIT 1` | Sort by non-null `lead_id` and sender identity match | correct | 18 related files | E1-P1A-RUST1, E1-P1A-TEST1 | preserve |
| `service.ingest_zalo_event` delta publisher | Emits `ConversationMessageDeltaEvent` with sequence and full message | Render-ready message delta published | correct | 14 related files | E1-P1A-DELTA1, E1-P1A-TEST2 | preserve |
| `eventAffectsItem` (customer items) | Matches by `replyTargets.conversationId`, `leadId`, or channel fallback without false-negative drop | Accurate matching | correct | 12 related files | E1-P1B-SVELTE1, E1-P1B-TEST1 | preserve |
| `handleMessageDelta` Direct Store Upsert | Upserts incoming message directly into `currentVariant.items` (<16ms) | Zero-IPC direct timeline bubble paint | correct | 12 related files | E1-P1B-SVELTE1, E1-P1B-TEST1 | preserve |
| `onConversationMessageDelta` bridge connection | Dual event bridge listener manages ref-counted connection for both events | Auto connection & retry backoff | correct | 8 related files | E1-P1B-TS1 | preserve |

## Status Refresh Log

| Refresh | Date | Repo Basis | Changed Scope | Status Changes | Evidence | Next Phase Update |
|---|---|---|---|---|---|---|
| R0 | 2026-08-30 | baseline before P0 | Backend lookup query and frontend event matching | initial classification | E0-P0A-RUST1..E0-P0A-TS1 | proceed with P1-A |
| R1 | 2026-08-30 | after P1 & P2 implementation | repository.rs, ConversationWorkspace.svelte, conversationService.ts | wrong/partial -> correct | E1-P1A-RUST1..E2-P2A-FULL2 | all gates complete |
| R2 | 2026-08-30 | Push-Delta architecture | events.rs, service.rs, ConversationWorkspace.svelte, conversationService.ts | 5s IPC delay -> <16ms instant delta paint | E1-P1A-DELTA1..E2-P2A-FULL2 | verified ready |

## Final P0 Decision

- [x] P0 complete. Target scope verified and remediated.
