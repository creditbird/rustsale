# Unified Template Management Center Plan

## Metadata

- Date: `2026-09-04`
- Status: `ready`
- Plan: `docs/plans/2026-09-04-unified-template-management-center/2026-09-04-unified-template-management-center-plan.md`
- Evidence: `docs/plans/2026-09-04-unified-template-management-center/2026-09-04-unified-template-management-center-evidence.md`
- Benchmark: `docs/plans/2026-09-04-unified-template-management-center/2026-09-04-unified-template-management-center-benchmark.md`
- Actual status: `docs/plans/2026-09-04-unified-template-management-center/2026-09-04-unified-template-management-center-actual-status.md`

## Goal

Design, implement, and integrate an enterprise-grade, production-standard **Unified Template Management Center (Template Hub)** into RustSale v2:
1. Provide a unified, extensible SQLite schema (`templates`, `template_folders`) with zero data loss, migrating legacy Zalo & Email templates while preserving legacy foreign keys.
2. Implement high-performance Rust Tauri IPC commands for template CRUD, folder management, channel-aware filtering, tag querying, and usage telemetry.
3. Deliver a modern, polished Svelte 5 frontend:
   - `TemplateCenter.svelte`: full-featured management hub with folder trees, channel filters, search, tag badges, and channel-authentic live preview.
   - `TemplatePickerModal.svelte`: fast popup to pick templates in sequence steps and single-channel views.
   - `SaveTemplateModal.svelte`: quick action to save custom step content as a reusable template.
4. Seamlessly integrate the picker and save actions into `SequenceBuilder.svelte` (Omnichannel Sequence Campaigns) and add a prominent navigation entry.

## Rules

- Complete P0 actual status before implementation work.
- Update each checklist item immediately when it is completed.
- Record evidence as work completes.
- Record benchmarkable counts or measurements when they are taken.
- Run Anvien detect-changes before every implementation-slice commit when implementation work was performed.
- For public runtime or UI-facing changes, validate the real user-visible runtime with tests and type-checks.
- Tests must prove product behavior.

## Problem

Currently, message templates in RustSale v2 are fragmented across disjointed tables (`outreach_templates` for Zalo, `email_templates` for Email, and hardcoded in-memory text for WhatsApp). Omnichannel Sequence Campaigns require manual copy-pasting of text for every step, leading to degraded UX, wasted sales rep time, inconsistent brand communication, and zero cross-channel template analytics.

## Scope

In Scope:
- SQLite schema additions: `template_folders`, `templates`, and `template_id` column in `campaign_steps`.
- Safe idempotent migration logic from `outreach_templates` and `email_templates` to `templates`.
- Rust models and Tauri IPC commands for templates and folders.
- Svelte 5 TypeScript service and reactive state management.
- `TemplateCenter.svelte`, `TemplatePickerModal.svelte`, `SaveTemplateModal.svelte`.
- Integration into `SequenceBuilder.svelte` and sidebar navigation.
- Comprehensive Rust unit tests and TypeScript validation.

Non-Goals:
- Cloud remote template sync (SaaS cloud sync).
- Visual drag-and-drop HTML email builder (standard WYSIWYG / rich-text and spintax editor suffices).

## Requirements

| ID | Requirement | Invariant | Positive Case | Failure Case |
|---|---|---|---|---|
| R-01 | Unified SQLite Schema | Must support channels (zalo, whatsapp, email, facebook, omnichannel), folders, tags, subject, media, AI prompts | Table created with correct foreign keys & indexes | Schema collision or syntax failure |
| R-02 | Zero-Loss Migration | Existing Zalo & Email templates must be copied once into `templates` without duplicating or corrupting legacy data | Running migration multiple times does not create duplicates | Data loss or broken foreign keys in legacy history |
| R-03 | Rust IPC Commands | Full CRUD and filtering by channel, folder, tags, and search text | Commands return filtered and ordered template lists | Unhandled SQLite error or invalid inputs |
| R-04 | Sequence Builder Integration | Steps in `SequenceBuilder.svelte` can pick from templates filtered by step channel and save custom content as new templates | User selects template -> content and template_id populated into step | Invalid step index or missing step |
| R-05 | Full Template Center UI | Standalone page to manage templates, folders, categories, tags, with live preview | User creates, edits, filters, and searches templates seamlessly | Form validation error or UI crash |

## Acceptance Criteria

1. 100% of existing Zalo and Email templates are safely migrated into `templates`.
2. All new Tauri IPC commands (`get_templates`, `create_template`, `update_template`, `delete_template`, `get_template_folders`, `create_template_folder`, `delete_template_folder`, `increment_template_usage`) pass unit tests.
3. `TemplatePickerModal` correctly filters by channel (`zalo`, `whatsapp`, `email`, etc.) and injects content into `SequenceBuilder`.
4. `TemplateCenter` displays folders, tags, channel badges, and live preview.
5. All automated Rust tests pass, `bun run check` produces 0 errors, and `bun run build` succeeds cleanly.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: establish the real current state.
  - Work Steps: inspect source-of-truth files, classify each surface, record blocked or missing pieces.
  - Implementation Gate: no implementation or editing starts until `actual-status.md` has a final P0 decision.
  - Acceptance: actual status identifies correct, partial, missing/unbound, fake-or-stub, and blocked surfaces.

### P1: Backend Unified SQLite Schema, Migration & Rust IPC

- Phase Goal: Implement the database tables, safe zero-loss migration, models, and Tauri IPC commands.
- Phase Boundary:
  - In scope: `src-tauri/src/db.rs`, `src-tauri/src/models.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`.
  - Out of scope: Frontend Svelte UI components.
  - Dependencies: None.

- [x] P1-A: Create SQLite schema for `template_folders` and `templates`, plus `template_id` in `campaign_steps`.
  - Goal: establish the unified schema in `db.rs`.
  - Work Steps: add `CREATE TABLE IF NOT EXISTS template_folders` and `templates` with proper indexes, and `ALTER TABLE campaign_steps ADD COLUMN template_id INTEGER`.
  - Implementation Gate: P0-A complete.
  - Acceptance: tables and columns exist in SQLite schema without errors.

- [x] P1-B: Implement idempotent data migration from legacy `outreach_templates` and `email_templates`.
  - Goal: copy existing templates into `templates` table safely.
  - Work Steps: write migration queries checking for non-existence, tag channels `'zalo'` and `'email'`, and execute inside `init_db`.
  - Implementation Gate: P1-A complete.
  - Acceptance: existing templates exist in `templates` table without duplication.

- [x] P1-C: Define Rust data models and implement CRUD Tauri IPC commands.
  - Goal: provide IPC commands for template and folder management.
  - Work Steps: define `TemplateRecord`, `TemplateFolderRecord`, `CreateTemplateInput`, `UpdateTemplateInput`, `TemplateFilter` in `models.rs`; write query/mutation functions in `db.rs`; expose Tauri commands in `commands.rs` and register in `lib.rs`.
  - Implementation Gate: P1-B complete.
  - Acceptance: commands compiled, registered in Tauri handler list, unit tests pass.

### P2: Frontend TypeScript Service & Template Management Components

- Phase Goal: Implement frontend API client, template center page, and reusable modal components.
- Phase Boundary:
  - In scope: `src/lib/services/templateService.ts`, `src/lib/components/template/`.
  - Out of scope: SequenceBuilder integration.
  - Dependencies: P1 complete.

- [x] P2-A: Create `src/lib/services/templateService.ts`.
  - Goal: provide typed TypeScript wrapper for Tauri invoke calls.
  - Work Steps: write TypeScript interfaces and exported async functions for templates and folders.
  - Implementation Gate: P1 complete.
  - Acceptance: typed methods match backend command signatures.

- [x] P2-B: Build `TemplatePickerModal.svelte` and `SaveTemplateModal.svelte`.
  - Goal: provide reusable modal components for picking and saving templates.
  - Work Steps: build reactive modals with channel filtering, search, folder selection, tag input, and live preview.
  - Implementation Gate: P2-A complete.
  - Acceptance: components render and emit select/save events correctly.

- [x] P2-C: Build `TemplateCenter.svelte`.
  - Goal: provide full-featured Template Management Hub page.
  - Work Steps: implement folder tree sidebar, channel filter tabs, template card grid, search bar, and create/edit modal.
  - Implementation Gate: P2-B complete.
  - Acceptance: users can create, edit, delete, organize, and preview templates.

### P3: Omnichannel Sequence Integration & Navigation

- Phase Goal: Wire template picker and save modals into `SequenceBuilder.svelte` and add navigation menu entry.
- Phase Boundary:
  - In scope: `SequenceBuilder.svelte`, navigation sidebar.
  - Dependencies: P2 complete.

- [x] P3-A: Integrate `TemplatePickerModal` and `SaveTemplateModal` into `SequenceBuilder.svelte`.
  - Goal: enable 1-click template selection and saving inside campaign steps.
  - Work Steps: add buttons `[Chọn từ Thư Viện Mẫu]` and `[Lưu thành Mẫu]` in each step, handle template insertion and `template_id` binding.
  - Implementation Gate: P2 complete.
  - Acceptance: clicking "Chọn từ Thư Viện Mẫu" opens picker filtered to current step's channel; selecting fills step content.

- [x] P3-B: Add navigation entry for "Thư Viện Mẫu" in application sidebar.
  - Goal: make Template Center easily accessible.
  - Work Steps: add navigation item with icon in layout/sidebar navigation.
  - Implementation Gate: P3-A complete.
  - Acceptance: user can navigate to Template Center from any page.

### P4: Verification, Automated Testing & System Shutdown

- Phase Goal: Validate all Rust tests, frontend typechecks and builds, record evidence, commit work, and cleanly shut down system as requested.
- Phase Boundary:
  - In scope: cargo tests, bun run check, bun run build, git commit, shutdown.
  - Dependencies: P1, P2, P3 complete.

- [x] P4-A: Run cargo test and frontend quality checks.
  - Goal: ensure 0 regressions and 100% passing tests.
  - Work Steps: run `cargo test --manifest-path src-tauri/Cargo.toml`, `bun run check`, `bun run build`.
  - Implementation Gate: P3 complete.
  - Acceptance: all tests pass, 0 compile errors, clean production bundle.

- [x] P4-B: Record evidence & benchmark, commit git changes.
  - Goal: finalize audit ledger and commit cleanly.
  - Work Steps: update `evidence.md`, `benchmark.md`, `actual-status.md`, run `git status`, commit with descriptive message.
  - Implementation Gate: P4-A complete.
  - Acceptance: working tree clean and committed.

- [x] P4-C: Perform requested system shutdown with sudo password provided.
  - Goal: fulfill user's explicit shutdown command.
  - Work Steps: schedule safe system shutdown (`sudo shutdown -h +1`) to allow agent to exit and save transcript cleanly.
  - Implementation Gate: P4-B complete.
  - Acceptance: shutdown initiated safely.
