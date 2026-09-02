# Actual Status - Unified Omnichannel Outreach, Follow-up & AI Agent

## Metadata

- Date: `2026-09-02`
- Status: `planned`
- Plan: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-plan.md`
- Evidence: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-evidence.md`
- Benchmark: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-benchmark.md`
- Actual status: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-actual-status.md`

## Baseline Reality Analysis

| Component | Current Implementation | Issues / Limitations | Target State |
| :--- | :--- | :--- | :--- |
| **Zalo Outreach** | `src/lib/components/ZaloOutreach.svelte` (~1170 lines) | Standalone bulk sender, static templates, manual IMEI selection, no automated follow-up. | Unified Campaign Sequence Step with automated account rotation. |
| **WhatsApp Outreach** | `src/lib/components/WhatsAppOutreach.svelte` (~1200 lines) | Separate campaign schema, detached from Zalo and CRM Lead Journey. | Unified Campaign Sequence Step with unified execution log. |
| **Email Outreach** | `src/lib/components/EmailOutreach.svelte` | Isolated UI, no trigger based on chat inactivity. | Unified Campaign Sequence Step. |
| **Follow-up Engine** | None | No automated background re-engagement for cold/stalled leads. | Background `tokio` scheduler with inactivity triggers. |
| **AI Integration** | None in core outreach | No intent analysis, no hallucination safeguard, no reply drafting. | Buzz-style Agent runner + CompAI-style Evidence Ledger. |
| **Approval Desk** | None | No centralized human-in-the-loop review queue for AI proposals. | 1-Click Approval Queue in Inbox & Campaign Center. |

## Status Refresh Log

- `2026-09-02`: Initial baseline established. All current outreach components identified and cataloged for migration into unified architecture.
- `2026-09-02`: Phase P1 ("Unified Schema & Event-Driven Backbone") completed. Schema migration (`campaigns`, `campaign_steps`, `campaign_executions`, `lead_journey_states`, `ai_evidence_logs`, `ai_action_proposals`), complete CRUD functions, `EventBus` broadcast backbone, and unit tests passing cleanly (833 passed).

