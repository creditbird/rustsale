# Lead Import Excel CSV Plan

## Metadata

- Date: `2026-09-09`
- Status: `draft`
- Plan: `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-plan.md`
- Evidence: `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-evidence.md`
- Benchmark: `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-benchmark.md`
- Actual status: `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-actual-status.md`

## Goal

Add a high-performance native Lead Import feature to RustSale CRM supporting Excel (.xlsx, .xls) and CSV (.csv) files, complete with a 4-step wizard modal, downloadable sample templates, smart column mapping, Vietnamese mobile number and email normalization, flexible deduplication options, real-time streaming progress, and downloadable error report.

## Rules

- Complete P0 actual status before implementation work.
- Update each checklist item immediately when it is completed.
- Record evidence as work completes.
- Record benchmarkable counts or measurements when they are taken.
- Update later phase status assumptions, next actions, and work steps when actual-status evidence changes the repo state.
- After completing a phase or implementation slice and refreshing `actual-status.md`, update the next affected phase's work steps as needed to match the latest repo reality, while preserving that phase's original goal, scope, acceptance criteria, and major phase order.
- Run Anvien detect-changes before every implementation-slice commit when implementation work was performed.
- For public runtime or UI-facing changes, validate the real user-visible runtime with browser or Playwright evidence.
- Non-blocking async loop: run all CPU-bound parsing and SQLite batch inserts in `tauri::async_runtime::spawn_blocking`.
- State & Locks: drop all mutex guards before `.await`.
- Every checklist item must be a complete mini-plan by itself.
- Tests must prove product behavior; delete or replace tests that only assert mocks or static DOM.

## Problem

Sales and marketing users frequently maintain prospect lists in Microsoft Excel (.xlsx, .xls) and CSV spreadsheets. Currently, RustSale CRM only supports manual one-by-one lead creation or scraping via Google Maps, with no way to bulk import existing lead lists. Users need a fast, resilient, and intuitive way to import thousands of leads with phone normalization, duplicate prevention, and downloadable sample templates.

## Scope

- Rust backend crates: `calamine = "0.26"` and `csv = "1.3"`.
- Backend module `src-tauri/src/lead_importer/`:
  - `types.rs`: Shared structs for inspection, mapping, options, progress, and results.
  - `inspector.rs`: Fast sheet enumeration, header detection, and 5-row sample preview.
  - `importer.rs`: Streaming parser, Vietnamese phone normalization, email normalization, deduplication modes (`skip`, `overwrite`, `allow`), SQLite `BEGIN IMMEDIATE` batch insertion, and error row quarantine.
  - `template.rs`: In-memory generation of Excel (.xlsx) and CSV (.csv) sample files.
  - `tests.rs`: Comprehensive unit tests.
- Tauri IPC commands registered in `src-tauri/src/commands.rs` & `src-tauri/src/lib.rs`.
- Frontend service in `src/lib/services/leadImportService.ts`.
- Svelte 5 Import Wizard modal in `src/lib/components/LeadImportModal.svelte`.
- Trigger button in `src/lib/components/LeadsList.svelte` header action bar.
- Bilingual localization in `messages/vi.json` and `messages/en.json`.

## Non-Goals

- Exporting leads (already exists in `src/lib/utils/leadsExportCsv.ts`).
- Direct cloud CRM integration (HubSpot, Salesforce REST APIs).
- Altering existing SQLite database schema or tables.

## Requirements

1. **Format Support**: Parse `.xlsx`, `.xls` (via `calamine`) and `.csv` (via `csv` with UTF-8 BOM handling).
2. **2-Phase Architecture**: Phase 1 file inspection (< 20ms) returning headers and 5 preview rows; Phase 2 streaming execution via `spawn_blocking` emitting progress events.
3. **Data Normalization**: Vietnamese phone normalization (+84, 84, 0084 -> 0, prepend missing leading 0 for 9-digit numbers, validate 03/05/07/08/09 10-digit mobile); lowercase trimmed email.
4. **Deduplication Strategy**: User-selectable conflict handling: `Skip` (retain existing, record duplicate count), `Overwrite` (update non-empty fields of existing lead), `Allow` (insert duplicate).
5. **Sample Templates**: Generate downloadable `.xlsx` and `.csv` template files with headers and 2 example rows, formatting phone numbers as text.
6. **Error Reporting**: Collect all failed/rejected rows with specific failure reasons and allow downloading an `import_errors.csv` file.
7. **Instant Reactivity**: Trigger `notifyLeadsChanged()` upon import completion so `LeadsList.svelte` updates instantly.

## Acceptance Criteria

- [ ] File inspector correctly reads headers and first 5 rows of `.xlsx`, `.xls`, and `.csv` files.
- [ ] Importer processes at least 5,000 rows/second on local SQLite database without locking UI.
- [ ] Phone numbers with missing leading zeros or international prefixes are properly normalized.
- [ ] Duplicate leads are detected and handled per user's selected mode (`Skip`, `Overwrite`, `Allow`).
- [ ] Template download generates valid, opening-ready `.xlsx` and `.csv` files.
- [ ] Full build (`cargo check`, `cargo clippy -- -D warnings`, `cargo test`, `bun run check`, `bun test`) passes with 0 errors.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: establish the real current state.
  - Work Steps: inspect source-of-truth files, classify each surface, record blocked or missing pieces, and update later phase status assumptions, next actions, and work steps from evidence.
  - Implementation Gate: no implementation or editing starts until `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-actual-status.md` has a final P0 decision.
  - Acceptance: actual status identifies correct, partial, missing/unbound, fake-or-stub, and blocked surfaces for this scope.

### P1: Backend Rust Core Importer & Template Engine

- Phase Goal: Implement native high-performance file inspection, streaming lead import, normalization, deduplication, and template generation in Rust.
- Phase Boundary:
  - In scope: `src-tauri/Cargo.toml`, `src-tauri/src/lead_importer/` (new module).
  - Out of scope: Frontend Svelte components and Tauri IPC commands.
  - Dependencies: `rusqlite`, `calamine`, `csv`.
- Phase Implementation Rule: implement sequentially from P1-A to P1-E.

- [x] P1-A: Add `calamine` and `csv` dependencies to `src-tauri/Cargo.toml`.
  - Goal: Equip backend with pure Rust Excel and CSV parsing capabilities.
  - Scope Boundary:
    - Editable: `src-tauri/Cargo.toml`
    - Inspect-only: `src-tauri/Cargo.lock`
    - Preserve-only: all existing dependencies
    - Out of scope: source code
  - Pre-flight Questions:
    - Data source: crates.io
    - DB read/write: N/A
    - UI flow: N/A
  - Work Steps:
    1. Add `calamine = "0.26"` and `csv = "1.3"` under `[dependencies]` in `src-tauri/Cargo.toml`.
    2. Run `cargo check --manifest-path src-tauri/Cargo.toml` to verify dependency resolution.
  - Implementation Gate: Anvien file-detail confirms `Cargo.toml` is clean.
  - Acceptance: `cargo check` compiles successfully.

- [x] P1-B: Implement types and file inspector (`types.rs`, `inspector.rs`).
  - Goal: Provide file inspection extracting sheet names, headers, and 5-row preview.
  - Scope Boundary:
    - Editable: `src-tauri/src/lead_importer/types.rs`, `src-tauri/src/lead_importer/inspector.rs`, `src-tauri/src/lead_importer/mod.rs`
    - Inspect-only: `src-tauri/src/db.rs`
    - Out of scope: Tauri IPC commands
  - Work Steps:
    1. Define `ImportInspection`, `ColumnMapping`, `LeadImportOptions`, `ImportProgress`, `ImportSummaryResult`, `ImportErrorRow` in `types.rs`.
    2. Implement `inspect_file(path: &Path)` in `inspector.rs` supporting `.xlsx`, `.xls`, and `.csv`.
  - Implementation Gate: `inspector.rs` compiles without warnings.
  - Acceptance: Unit test confirms inspection returns sheet names, headers, and first 5 rows.

- [x] P1-C: Implement streaming execution engine (`importer.rs`).
  - Goal: Stream rows, normalize VN phone/email, deduplicate against DB and current batch, batch insert into `leads`.
  - Scope Boundary:
    - Editable: `src-tauri/src/lead_importer/importer.rs`
    - Inspect-only: `src-tauri/src/google_maps_jobs/importer.rs`
  - Work Steps:
    1. Implement `normalize_phone_vn` and `normalize_email`.
    2. Implement `execute_import` with `TransactionBehavior::Immediate`, intra-batch dedup, conflict resolution, progress channel tick, and row quarantine.
  - Implementation Gate: `importer.rs` passes type-check.
  - Acceptance: Successfully imports test rows with phone normalization and duplicate skipping.

- [x] P1-D: Implement in-memory sample Excel & CSV template generator (`template.rs`).
  - Goal: Generate standard sample `.xlsx` and `.csv` templates.
  - Scope Boundary:
    - Editable: `src-tauri/src/lead_importer/template.rs`
    - Inspect-only: `src/lib/utils/leadsExportCsv.ts`
  - Work Steps:
    1. Implement `generate_csv_template()` with UTF-8 BOM and headers.
    2. Implement `generate_excel_template()` (or structured CSV template compatible with Excel).
  - Acceptance: Generated template opens cleanly in spreadsheet viewers with text phone column.

- [x] P1-E: Write unit tests in `src-tauri/src/lead_importer/tests.rs`.
  - Goal: Thoroughly test normalization, dedup, and parser edge cases.
  - Scope Boundary:
    - Editable: `src-tauri/src/lead_importer/tests.rs`
  - Work Steps:
    1. Write tests for missing leading zero recovery, international prefix stripping, and invalid mobile rejection.
    2. Write tests for Skip, Overwrite, and Allow conflict modes in SQLite memory DB.
    3. Run `cargo test lead_importer`.
  - Acceptance: All unit tests pass.


### P2: Tauri IPC Commands & Dialog Wiring

- Phase Goal: Expose native import commands to the Tauri IPC boundary with non-blocking async execution.
- Phase Boundary:
  - In scope: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/commands_test.rs`.
- [x] P2-A: Implement Tauri commands `inspect_lead_import_file`, `execute_lead_import`, `download_lead_import_template`, and `pick_lead_import_file`.
  - Work Steps:
    1. Implement commands in `src-tauri/src/commands.rs` wrapping `lead_importer` in `spawn_blocking`.
    2. Register commands in `src-tauri/src/lib.rs` under `generate_handler![]`.
  - Acceptance: Commands compile and are callable via Tauri invoke.
- [x] P2-B: Integration tests for Tauri commands in `src-tauri/src/commands_test.rs`.
  - Work Steps:
    1. Add tests verifying IPC command execution and error serialization.
    2. Run `cargo test --test commands_test` or `cargo test test_lead_import`.
  - Acceptance: IPC tests pass.


### P3: Frontend Service & UI Modal Wizard

- Phase Goal: Deliver the Svelte 5 Import Wizard modal, smart column mapping, and leads list integration.
- Phase Boundary:
  - In scope: `src/lib/services/leadImportService.ts`, `src/lib/components/LeadImportModal.svelte`, `src/lib/components/LeadsList.svelte`, `messages/vi.json`, `messages/en.json`.
- [x] P3-A: Implement `src/lib/services/leadImportService.ts` and test.
  - Work Steps:
    1. Create TypeScript types matching Rust IPC contracts.
    2. Implement `inspectImportFile`, `executeLeadImport`, `pickImportFile`, `downloadTemplate`.
    3. Add unit test `leadImportService.spec.ts`.
  - Acceptance: `bun test src/lib/services/__tests__/leadImportService.spec.ts` passes.
- [x] P3-B: Implement `src/lib/components/LeadImportModal.svelte`.
  - Work Steps:
    1. Step 1: File dropzone & file picker, template download buttons, sheet selector.
    2. Step 2: Auto-column mapping UI with sample preview table.
    3. Step 3: Conflict strategy radio options, default category & status dropdowns.
    4. Step 4: Real-time progress bar and summary dialog with error export button.
  - Acceptance: Component renders cleanly without Svelte check errors.
- [x] P3-C: Wire "Import Leads" trigger button into `src/lib/components/LeadsList.svelte`.
  - Work Steps:
    1. Add "Import Leads" button to header action bar next to "Add Lead".
    2. Bind modal open state.
  - Acceptance: Clicking button opens modal; completing import refreshes table via `notifyLeadsChanged()`.
- [x] P3-D: Add bilingual translation keys in `messages/vi.json` and `messages/en.json`.
  - Work Steps:
    1. Add all `leads_import_*` translation keys to `messages/vi.json` and `messages/en.json`.
    2. Run `bun run i18n:compile`.
  - Acceptance: Clean i18n compilation with 0 missing keys.

### P4: Verification, Quality Gate & Closure

- [x] P4-A: Call supervisor for the implemented-plan acceptance loop.
  - Goal: Verify completed plan work against accepted plan, actual-status, evidence, and benchmarks before closure.
  - Work Steps:
    1. Call supervisor skill / Codex review to review the full completed plan work.
    2. Address any findings (F1, F2, F3, F4 completely resolved with 0 defects).
  - Acceptance: Codex review passes with VERDICT: APPROVED.
- [x] P4-B: Remove dead work created during this plan.
  - Goal: Ensure final diff contains no temporary or dead artifacts.
  - Work Steps: Clean up temporary test files or debug logs.
  - Acceptance: Clean `git status`.
- [x] P4-C: Close the plan.
  - Goal: Final validation, evidence, benchmark, detect-changes, and commit.
  - Work Steps:
    1. Run `cargo test`, `bun run check`, `bun test`, `bun run build`.
    2. Run Anvien `detect-changes`.
    3. Record closure evidence and commit.
  - Acceptance: Full pass on all verification gates and clean commit.

## Risk Notes

- **Large File Memory**: Files with >50,000 rows are processed via row streaming iterator in Rust, keeping memory < 20MB.
- **SQLite Lock Contention**: Using `TransactionBehavior::Immediate` prevents deadlock while allowing readers during preparation.
- **Malformed Phones**: Rows with completely invalid phone/email are quarantined into error report rather than failing the whole batch.
