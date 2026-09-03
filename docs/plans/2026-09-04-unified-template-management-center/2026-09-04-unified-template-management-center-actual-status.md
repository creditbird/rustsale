# Unified Template Management Center Actual Status

Title: Unified Template Management Center
Date: 2026-09-04
Status: P0 Complete
Companion plan: `docs/plans/2026-09-04-unified-template-management-center/2026-09-04-unified-template-management-center-plan.md`
Companion evidence: `docs/plans/2026-09-04-unified-template-management-center/2026-09-04-unified-template-management-center-evidence.md`
Companion benchmark: `docs/plans/2026-09-04-unified-template-management-center/2026-09-04-unified-template-management-center-benchmark.md`

## Purpose

This file records the real current state before implementation.

Implementation must not start until the target scope has a completed status row, evidence IDs, and a downstream plan decision.

This file does not replace `evidence.md`. It classifies current state from evidence.

Use exact evidence IDs from `evidence.md`, such as `E0-P0A-SRC1`.

## Freshness / Refresh Rules

This actual-status file is a living current-state record, not a one-time P0 snapshot.

P0 records the baseline before implementation. After implementation begins, keep the Current Status Matrix updated so the next agent can trust it as the latest repo reality.

Update this file:
- after each completed implementation slice;
- before starting the next phase if repo state changed;
- whenever evidence changes a current-state classification;
- whenever the next phase's status assumptions, next action, or work steps need updating because reality differs from the previous status.

## Scope

Target scope:
- Unified SQLite schema (`template_folders`, `templates`) and zero-loss idempotent migration from legacy `outreach_templates` and `email_templates`.
- Backward compatibility with legacy `outreach_history` and `email_outreach_history`.
- Tauri IPC commands for template CRUD, folder CRUD, filtering by channel/tags/folders, and usage counting.
- Frontend Unified Template Management Center (`TemplateCenter.svelte`) with folder taxonomy, channel filtering, tag badges, and channel-aware previews.
- Reusable `TemplatePickerModal.svelte` and `SaveTemplateModal.svelte` integrated into `SequenceBuilder.svelte` and outreach views.
- Navigation bar integration for quick access to the Template Center.

Out of scope:
- External CRM cloud sync (HubSpot/Salesforce remote APIs).
- Automated AI fine-tuning training pipelines.

## Relationship / Impact Evidence

| Unit / File / Surface | File Detail Evidence | Related File Count | Relationship Summary | Impact Note |
|---|---|---|---|---|
| `src-tauri/src/db.rs` | `E0-P0A-DET1` | 42 | Central SQLite schema & queries | Medium blast radius; additive tables and safe migration |
| `src-tauri/src/models.rs` | `E0-P0A-DET2` | 18 | Data struct models for IPC serde | Low blast radius; additive structs |
| `src-tauri/src/commands.rs` | `E0-P0A-DET3` | 24 | Tauri IPC command handlers | Low blast radius; additive commands |
| `src-tauri/src/lib.rs` | `E0-P0A-DET4` | 8 | Command registration table | Low blast radius; handler list addition |
| `src/lib/components/campaign/SequenceBuilder.svelte` | `E0-P0A-DET5` | 4 | Sequence builder UI with step templates | Medium blast radius; add template picker and save buttons |

## Status Rules

| Status | Meaning | Allowed next action |
|---|---|---|
| `correct` | Already behaves as required. | Preserve. Add evidence or tests only if needed. |
| `partial` | Some required behavior exists, but gaps remain. | Change only the missing parts. Preserve correct parts. |
| `wrong` | Current behavior, source, or contract is incorrect. | Replace with required behavior. Record the exact reason. |
| `missing` | Required behavior, source, or contract does not exist. | Implement the missing piece only. |
| `unbound` | Surface exists but is not wired to the real source, flow, or contract. | Bind to the real source only. Preserve approved surface. |
| `fake-or-stub` | Prototype, demo, mock, fallback, or placeholder data is being used as real behavior. | Remove fake behavior or replace it with an approved truthful state. |
| `blocked` | Source, authority, contract, or required evidence is unclear. | Stop. Do not implement until resolved. |

## Current Status Matrix

| Unit | Current State | Required State | Status | Relationship Count | Evidence | Next Plan Decision |
|---|---|---|---|---|---|---|
| `db.sqlite:templates` | Unified `templates` table created with indexes | Unified `templates` table with channel discriminator, folder, stage, tags, media, ai_prompt | `correct` | 12 related queries | `E1-P1A-SCHEMA` | Ready for frontend binding |
| `db.sqlite:template_folders` | Table created with name, color, timestamps | `template_folders` table for grouping collections | `correct` | 4 related queries | `E1-P1A-SCHEMA` | Ready for frontend binding |
| `db.sqlite:migration` | Idempotent migration copying existing Zalo & Email templates into `templates` | Idempotent migration copying existing Zalo and Email templates into `templates` without breaking legacy FKs | `correct` | 6 related tables | `E1-P1B-MIGRATION` | Verified idempotent |
| `src-tauri/src/commands.rs` | Unified IPC commands registered in `lib.rs`: `get_templates`, `get_template_by_id`, `create_template`, `update_template`, `delete_template`, `get_template_folders`, `create_template_folder`, `delete_template_folder`, `increment_template_usage` | Unified IPC commands: `get_templates`, `create_template`, `update_template`, `delete_template`, `get_template_folders`, `create_template_folder` | `correct` | 14 handlers | `E1-P1C-TEST` | Ready for frontend binding |
| `src/lib/services/templateService.ts` | TypeScript service with IPC functions and helper utilities | TypeScript client for unified template & folder IPC commands | `correct` | 3 callers | `E2-P2A-TS` | Complete |
| `src/lib/components/template/TemplatePickerModal.svelte` | Modal component with channel filtering, spintax preview, and selection callback | Modal to pick template with channel filtering, live preview, and fast insert | `correct` | 1 caller | `E2-P2B-MODAL` | Complete |
| `src/lib/components/template/SaveTemplateModal.svelte` | Modal component with spintax support, folder selection, and tag input | Modal to save step content as new template into library | `correct` | 1 caller | `E2-P2B-MODAL` | Complete |
| `src/lib/components/template/TemplateCenter.svelte` | Full Template Hub page with folder tree, channel filters, search, edit/create, live preview | Comprehensive Template Management Hub page | `correct` | 1 caller | `E2-P2C-CENTER` | Complete |
| `SequenceBuilder.svelte` | Integrated with TemplatePickerModal and SaveTemplateModal, step template_id badge | Step has `[Chọn từ Thư Viện Mẫu]` and `[Lưu thành Mẫu]` buttons | `correct` | 4 callers | `E3-P3A-INTEG` | Complete |
| Navigation Sidebar | `Sidebar.svelte` and `+page.svelte` have `template_center` tab wired | Navigation link to "Thư Viện Mẫu" | `correct` | 2 files | `E3-P3B-NAV` | Complete |

## Status Refresh Log

| Refresh Date | Trigger / Completed Slice | Transitions | Downstream Plan Consequence |
|---|---|---|---|
| 2026-09-04 | P0 Baseline inspection | Initial baseline recorded | Proceed with P1 backend implementation |
| 2026-09-04 | P1 Backend implementation completed | `templates`, `template_folders`, `migration`, `commands.rs` -> `correct` | Proceed with P2 frontend service and UI |
| 2026-09-04 | P2 & P3 Frontend implementation completed | `templateService.ts`, `TemplatePickerModal.svelte`, `SaveTemplateModal.svelte`, `TemplateCenter.svelte`, `SequenceBuilder.svelte`, Sidebar & Navigation -> `correct` | Proceed with P4 verification & commit |
