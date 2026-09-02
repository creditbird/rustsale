# Unified Omnichannel Outreach, Follow-up & AI Agent Implementation Plan

## Metadata

- Date: `2026-09-02`
- Status: `planned`
- Plan: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-plan.md`
- Evidence: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-evidence.md`
- Benchmark: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-benchmark.md`
- Actual status: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-actual-status.md`

## Goal

Unify RustSale's fragmented outreach silos (`ZaloOutreach.svelte`, `WhatsAppOutreach.svelte`, `EmailOutreach.svelte`) into an **Enterprise-Grade Omnichannel Sequence & Follow-up Engine**, powered by **Block/Buzz's Event-Driven Rust Engine & Agent Identity** and **Comp AI's Evidence Ledger & Autonomous Follow-up Workflows**.

## Rules

- Complete P0 actual status before implementation work.
- Update each checklist item immediately upon completion.
- Record evidence as each evidenced task is completed.
- Record benchmark metrics (database query throughput, memory consumption, IPC latency).
- Run Anvien `detect-changes` before each implementation-slice commit.
- Enforce strict Human-in-the-loop safeguards on AI outbound communications.
- Follow Tauri v2 optimization standards: non-blocking async handlers (`tokio::spawn` / `spawn_blocking`), minimal IPC overhead, zero UI freeze.

## Problem

1. **Channel Fragmentation**: Outreach logic is duplicated across separate UI files with inconsistent database schemas and no cross-channel coordination.
2. **Lack of Automated Follow-up Sequences**: Outreach is limited to static one-time bulk broadcasts (spintax templates) without multi-step drip logic (e.g., Step 1 Zalo -> wait 2 days -> Step 2 WhatsApp if no reply).
3. **No AI-Powered Lead Intent & Evidence Verification**: Inbound messages are not automatically analyzed for buying intent, and there is no verification ledger to prevent AI hallucination when drafting responses.
4. **Disconnection from Lead Journey**: Outreach events and Omnichannel Inbox conversations do not synchronize seamlessly with lead lifecycle stages.

## Scope

- **Database Layer**: Introduce unified SQLite tables for `campaigns`, `campaign_steps`, `campaign_executions`, `lead_journey_states`, `ai_evidence_logs`, and `ai_action_proposals`.
- **Rust Engine Core (`src-tauri`)**:
  - `src-tauri/src/events/`: Internal event bus for message receipt, campaign progression, and AI triggers.
  - `src-tauri/src/campaign/`: Durable background scheduler with anti-ban rate limiting, account rotation, and condition evaluation.
  - `src-tauri/src/ai/`: Agent runner, intent classifier, evidence extractor, and reply generator.
- **Frontend Svelte 5 (`src/lib`)**:
  - `CampaignCenter.svelte`: Unified multi-step campaign builder and monitor replacing channel-specific outreach pages.
  - `AICopilotBar.svelte`: AI smart suggestions and quick-reply toolbar in Omnichannel Inbox.
  - `AIApprovalQueue.svelte`: 1-click human review desk for proposed outbound messages.

## Non-Goals

- Deploying public decentralized Nostr relays (using internal embedded event bus instead).
- Full autonomous unmonitored AI chatting without safety thresholds or human review gates.
- Rebuilding existing native Zalo/WhatsApp/FB connection protocol adapters.

## Architecture

```
+----------------------------------------------------------------------------------------------------+
|                                    SVELTE 5 PRESENTATION LAYER                                     |
|  • Unified Campaign Center (Sequence Builder, Audience Filter, Analytics)                         |
|  • Omnichannel Inbox + AI Copilot Toolbar + 1-Click Approval Queue                                |
+----------------------------------------------------------------------------------------------------+
                                                  │
                                                  │ Tauri IPC (Commands & Event Stream)
                                                  ▼
+----------------------------------------------------------------------------------------------------+
|                                      RUST ENGINE CORE (TAURI v2)                                   |
|                                                                                                    |
|  +-----------------------------+     +-------------------------------+     +--------------------+  |
|  |     Internal Event Bus      |     |      Rust AI Agent Runner     |     |  Campaign Sequence |  |
|  | (Event Sourcing from Buzz)  | ──> | (Evidence Ledger from CompAI) | ──> |   Scheduler (Rust) |  |
|  +-----------------------------+     +-------------------------------+     +--------------------+  |
|                                                      │                                             |
|                                                      ▼                                             |
|                                      +-------------------------------+                             |
|                                      |   Omnichannel Dispatcher Pool |                             |
|                                      | (Zalo, WhatsApp, FB, Email)   |                             |
|                                      +-------------------------------+                             |
+----------------------------------------------------------------------------------------------------+
                                                  │
                                                  ▼
+----------------------------------------------------------------------------------------------------+
|  LOCAL STORAGE (SQLite WAL Mode): Campaigns, Steps, Executions, Journey States, AI Evidence Logs   |
+----------------------------------------------------------------------------------------------------+
```

## Checklist

- [ ] P0-A: Complete actual status and baseline analysis.
  - Goal: Audit existing outreach implementations, database schemas, and blast radius.
  - Acceptance: `actual-status.md` completed with R0 status matrix and evidence logged.

### P1: Unified Schema & Event-Driven Backbone (Buzz Architecture)

- Phase Goal: Establish database schema for campaigns/sequences and internal event sourcing bus in Rust.
- [x] P1-A: Create SQLite migration for Unified Campaigns & AI Evidence
  - Goal: Add tables `campaigns`, `campaign_steps`, `campaign_executions`, `lead_journey_states`, `ai_evidence_logs`, `ai_action_proposals`.
  - Scope: `src-tauri/src/db.rs`, `src-tauri/src/conversation/schema.sql`, `src-tauri/src/models.rs`.
  - Acceptance: Migrations run cleanly, tables created with indexes, tests pass.
- [x] P1-B: Implement Internal Event Bus in Rust Core
  - Goal: Define `EventKind` enum (InboundMessage, LeadQualified, StepTriggered, ReplyApproved) and `tokio::sync::broadcast` router.
  - Scope: `src-tauri/src/conversation/events.rs`, `src-tauri/src/events/mod.rs`.
  - Acceptance: Event dispatch & listener unit tests verify zero-loss event propagation.

### P2: AI Agent Runner & Evidence Ledger (Comp AI Architecture)

- Phase Goal: Build the core AI brain with factual verification, intent classification, and follow-up generation.
- [x] P2-A: Implement Evidence Extractor & Policy Gate
  - Goal: Query internal product catalogue, pricing, and past interaction history as verifiable evidence prior to AI generation.
  - Scope: `src-tauri/src/ai/evidence.rs`, `src-tauri/src/ai/policy.rs`.
  - Acceptance: Verified facts pass to prompt; unverified claims flagged with low confidence.
- [x] P2-B: Implement Intent Classifier & Smart Reply Generator
  - Goal: Categorize incoming messages (AskingPrice, Schedule, Complaint, Unsubscribe) and generate contextual replies.
  - Scope: `src-tauri/src/ai/intent.rs`, `src-tauri/src/ai/generator.rs`, `src-tauri/src/ai/gateway.rs`.
  - Acceptance: Unit tests on sample customer queries produce accurate intents and drafted replies.

### P3: Rust Background Sequence Scheduler & Dispatcher

- Phase Goal: Build durable, non-blocking background sequence execution with anti-ban throttling.
- [x] P3-A: Implement Campaign Sequence State Machine
  - Goal: Execute multi-step sequences (Step 1 -> Delay -> Condition Check -> Step 2) in background `tokio` tasks.
  - Scope: `src-tauri/src/campaign/scheduler.rs`, `src-tauri/src/campaign/evaluator.rs`.
  - Acceptance: Mock sequences progress through steps, pause when customer replies, and respect delay intervals.
- [x] P3-B: Implement Omnichannel Dispatcher & Account Rotation
  - Goal: Route outbound step execution to the target platform adapter (Zalo, WhatsApp, Email, FB) with rate-limit throttling.
  - Scope: `src-tauri/src/campaign/dispatcher.rs`, `src-tauri/src/campaign/throttling.rs`.
  - Acceptance: Messages dispatched respecting per-account rate limits without blocking UI threads.

### P4: Unified Svelte 5 Frontend & Human-in-the-Loop Approval

- Phase Goal: Replace fragmented outreach pages with unified Campaign Center and AI Copilot Inbox integration.
- [ ] P4-A: Build Unified Campaign Center UI
  - Goal: Interactive sequence builder, audience segment selector, and campaign performance dashboard.
  - Scope: `src/lib/components/campaign/CampaignCenter.svelte`, `src/lib/components/campaign/SequenceBuilder.svelte`.
  - Acceptance: Users can configure multi-channel sequence campaigns, preview steps, and monitor progress.
- [ ] P4-B: Build AI Copilot Toolbar & 1-Click Approval Queue in Inbox
  - Goal: Display AI suggestions in conversation timeline and provide a centralized review desk for batch outreach.
  - Scope: `src/lib/components/conversation/AICopilotBar.svelte`, `src/lib/components/campaign/AIApprovalQueue.svelte`.
  - Acceptance: Sales reps can review, edit, and approve AI-generated messages with 1 click.

### P5: End-to-End Verification & Performance Benchmarking

- Phase Goal: Validate full workflow, ensure zero UI jank, and measure system throughput.
- [ ] P5-A: End-to-End Workflow & Anti-Regression Verification
  - Goal: Test complete lead journey: Scraped Lead -> Automated Zalo Sequence -> Customer Reply -> AI Intent Classification -> AI Copilot Suggestion -> Deal Closure.
  - Scope: Vitest component tests, Rust integration tests, Playwright E2E suite.
  - Acceptance: All test suites pass with 0 errors.
- [ ] P5-B: Performance Benchmarking & Memory Audit
  - Goal: Verify memory footprint (< 120MB idle), SQLite query latency (< 5ms), and non-blocking IPC under concurrent message bursts.
  - Scope: Benchmark ledger in `2026-09-02-unified-outreach-followup-ai-agent-benchmark.md`.
  - Acceptance: All benchmark criteria met.
