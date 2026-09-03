# Unified Template Management Center Evidence Ledger

Title: Unified Template Management Center Evidence Ledger
Date: 2026-09-04
Plan: `docs/plans/2026-09-04-unified-template-management-center/2026-09-04-unified-template-management-center-plan.md`
Benchmark: `docs/plans/2026-09-04-unified-template-management-center/2026-09-04-unified-template-management-center-benchmark.md`
Actual status: `docs/plans/2026-09-04-unified-template-management-center/2026-09-04-unified-template-management-center-actual-status.md`

## Purpose

This document stores concrete evidence IDs, test assertions, migration proofs, and verification logs for the Unified Template Management Center.

## Evidence Matrix

| Evidence ID | Phase / Slice | Type | Description | Result / Value |
|---|---|---|---|---|
| `E0-P0A-SRC1` | P0-A | Code Inspection | Verification that legacy templates are split across `outreach_templates` and `email_templates` | Verified in `src-tauri/src/db.rs` |
| `E0-P0A-SRC2` | P0-A | Schema Inspection | `campaign_steps` has `template_content` string without `template_id` FK | Verified in `src-tauri/src/db.rs` line 485 |
| `E0-P0A-SRC3` | P0-A | IPC Inspection | Tauri commands only exist for legacy templates (`get_outreach_templates`, etc.) | Verified in `src-tauri/src/commands.rs` |
| `E0-P0A-DET1` | P0-A | Blast Radius | `anvien file-detail` on `src-tauri/src/db.rs` | Parsed cleanly, 89k nodes graph |
| `E1-P1A-SCHEMA` | P1-A | Schema Check | SQLite schema creation for `template_folders`, `templates`, and `template_id` | Verified in `db.rs` and `test_db_init_and_tables` (all tables & columns exist) |
| `E1-P1B-MIGRATION`| P1-B | Migration Run | Idempotent migration from `outreach_templates` and `email_templates` | Verified in `test_template_migration_idempotency` (0 duplicate rows on multiple runs) |
| `E1-P1C-TEST` | P1-C | Unit Tests | Rust unit tests for template CRUD and filtering | 847 tests passed (846 unit + 1 integration), 0 failed |
| `E2-P2A-TS` | P2-A | Typecheck | TypeScript compilation of `templateService.ts` | 0 errors via `bun run check` |
| `E2-P2B-MODAL` | P2-B | Component Check | `TemplatePickerModal` and `SaveTemplateModal` rendered | Svelte 5 runes, live Spintax preview, modal dialogs verified |
| `E2-P2C-CENTER` | P2-C | Component Check | `TemplateCenter.svelte` rendered and functional | Folders, stages, channel tabs, metrics, CRUD modals verified |
| `E3-P3A-INTEG` | P3-A | UI Flow | Template picker integrated into `SequenceBuilder.svelte` | Pick template & Save as template buttons + template_id badge verified |
| `E3-P3B-NAV` | P3-B | Navigation | "Thư Viện Mẫu" navigation link added to layout/sidebar | Sidebar menu item + +page.svelte tab route verified |
| `E4-P4A-GATE` | P4-A | Full Suite | `cargo test`, `bun run check`, `bun run build` | `bun run check`: 0 errors, `bun run build`: static site built in 29.24s cleanly |
