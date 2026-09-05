# Evidence Ledger - Unified Omnichannel Outreach, Follow-up & AI Agent

## Metadata

- Date: `2026-09-02`
- Status: `planned`
- Plan: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-plan.md`
- Evidence: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-evidence.md`
- Benchmark: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-benchmark.md`
- Actual status: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-actual-status.md`

## Evidence Entries

| ID | Phase | Step | Target Area | Evidence Description | Result / Output |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `EV-P0-01` | P0 | P0-A | Codebase Audit | Identified `ZaloOutreach.svelte` (1171 lines) and `WhatsAppOutreach.svelte` (1208 lines) as legacy monolithic components needing unification. | Verified file paths and line counts |
| `EV-P0-02` | P0 | P0-A | Schema Audit | Discovered separate `whatsapp_campaigns` table and ad-hoc Zalo template storage in SQLite. | Confirmed need for unified schema |
| `EV-P1-01` | P1 | P1-A | SQLite Migration & CRUD | Created tables `campaigns`, `campaign_steps`, `campaign_executions`, `lead_journey_states`, `ai_evidence_logs`, `ai_action_proposals` with foreign keys and indexes. Added complete CRUD API. | Unit tests in `db_test.rs` pass (100% assertions verified) |
| `EV-P1-02` | P1 | P1-B | Internal Event Bus | Implemented `EventBus` in `src/events/mod.rs` with `AppEvent` variants for message receipt, lead journey transitions, campaign step dues, and AI proposals. | Broadcast tests verify zero-loss propagation across multiple subscribers |
| `EV-P2-01` | P2 | P2-A | AI Evidence & Policy Gate | Implemented `EvidenceExtractor` (lead, deal, journey, products, conversation history) & `PolicyGate` (profanity/forbidden words, unsubscribe protection, complaint escalation, price commitment rules, confidence thresholds). | Unit tests verify auto-approval vs review requirements vs immediate rejection |
| `EV-P2-02` | P2 | P2-B | Intent & SmartReply Generator | Implemented `IntentClassifier` (heuristic fast-path for VN queries + LLM fallback), `SmartReplyGenerator` (3 variants: Brief, Detailed, Promotional), `LlmGateway` (OpenAI, Gemini, Claude, Ollama, Mock fallback), and Tauri commands. | 5 unit tests pass with 100% assertions in `ai/tests.rs` |
| `EV-P3-01` | P3 | P3-A | Sequence Runner & Evaluator | Implemented `StepConditionEvaluator` (`ALWAYS`, `IF_NOT_REPLIED`, `IF_DEAL_STAGE_IS`, `IF_INTENT_IS`) and `CampaignScheduler` background worker with step progression and next step auto-scheduling. | 6 unit tests pass verifying multi-step drip progression, reply skipping, and journey state sync |
| `EV-P3-02` | P3 | P3-B | Dispatcher & Throttling | Implemented `OmnichannelDispatcher` (spintax, CRM variable interpolation, dry-run & live routing for Zalo, WhatsApp, Email, FB) and `AntiBanThrottler` + `AccountRotator` (jitter delays, burst limits, daily quotas, round-robin/LRU). | Unit tests verify round-robin rotation, quota exhaustion, burst cooldown, and analytics computation |
| `EV-P4-01` | P4 | P4-A | Campaign Center & Sequence Builder UI | Implemented `CampaignCenter.svelte` (KPI summary, campaign management, lead enrollment drawer, funnel analytics breakdown) and `SequenceBuilder.svelte` (visual step cards, multi-channel selector, delay presets, condition rules, spintax previewer). | Verified in `bun test` and `bun run check` (0 errors) |
| `EV-P4-02` | P4 | P4-B | AI Copilot Bar & Approval Queue UI | Implemented `AICopilotBar.svelte` embedded in `ConversationWorkspace.svelte` above composer (intent classifier badge, 3 reply pills, 1-click draft insertion) and `AIApprovalQueue.svelte` (human-in-the-loop review desk). Integrated navigation into `Sidebar.svelte` and `+page.svelte`. | 14 service unit tests + component tests passing cleanly |
| `EV-P5-01` | P5 | P5-A | End-to-End QA | Pending full E2E flow test. | Pending |
