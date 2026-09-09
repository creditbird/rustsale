# Lead Import Excel CSV Actual Status

Title: Lead Import Excel CSV
Date: 2026-09-09
Status: P0 Complete
Companion plan: `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-plan.md`
Companion evidence: `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-evidence.md`
Companion benchmark: `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-benchmark.md`

## Purpose

This file records the real current state before implementation.

Implementation must not start until the target scope has a completed status row, evidence IDs, and a downstream plan decision.

This file does not replace `evidence.md`. It classifies current state from evidence.

Use exact evidence IDs from `evidence.md`, such as `E0-P0A-SRC1`, not broad section IDs such as `E0` or `E1`.

## Freshness / Refresh Rules

This actual-status file is a living current-state record, not a one-time P0 snapshot.

P0 records the baseline before implementation. After implementation begins, keep the Current Status Matrix updated so the next agent can trust it as the latest repo reality.

Update this file:

- after each completed implementation slice;
- before starting the next phase if repo state changed;
- whenever evidence changes a current-state classification;
- whenever the next phase's status assumptions, next action, or work steps need updating because reality differs from the previous status.

When refreshing status:

- update only the rows affected by the completed work or new evidence;
- use explicit transitions such as `missing -> correct`, `partial -> correct`, `fake-or-stub -> removed`, or `unbound -> bound-correct`;
- append a Status Refresh Log row instead of deleting history;
- keep detailed proof in `evidence.md`; store only classifications, evidence IDs, touch mode, and plan consequences here.

## Scope

Target scope:

- Native Excel (.xlsx, .xls) and CSV (.csv) lead import module in Rust backend (`src-tauri/src/lead_importer/`).
- File inspection (sheet enumeration, header detection, sample rows preview).
- Execution engine with streaming IPC channel, Vietnamese mobile phone normalization, email normalization, deduplication modes (Skip, Overwrite, Allow), SQLite batch write (`BEGIN IMMEDIATE`), and row-level error reporting.
- Sample template generator for Excel and CSV with text-formatted phone numbers.
- Tauri IPC commands registered in `src-tauri/src/commands.rs` and `src-tauri/src/lib.rs`.
- Frontend service in `src/lib/services/leadImportService.ts`.
- Svelte 5 Import Wizard modal in `src/lib/components/LeadImportModal.svelte`.
- Trigger button in `src/lib/components/LeadsList.svelte`.
- i18n translation keys in `messages/vi.json` and `messages/en.json`.

Out of scope:

- Exporting leads (already exists and functioning via `exportLeadsToCsv`).
- Third-party CRM sync (HubSpot, Salesforce API direct sync).
- Modifying underlying SQLite `leads` table schema (all required columns already exist).

## Relationship / Impact Evidence

For each target file:

```text
anvien file-detail <path> --json
```

| Unit / File / Surface | File Detail Evidence | Related File Count | Relationship Summary | Impact Note |
|-----------------------|----------------------|--------------------|----------------------|-------------|
| `src-tauri/Cargo.toml` | E0-P0A-FD1 | 0 related files | Cargo manifest for backend dependencies | low: adding `calamine` and `csv` crates |
| `src-tauri/src/commands.rs` | E0-P0A-FD2 | 4 related files | Tauri IPC command handlers | high scope warning: central command hub; keep new commands strictly isolated in dedicated module |
| `src-tauri/src/lib.rs` | E0-P0A-FD3 | 0 related files | Tauri builder & command registration | low: only adding handler registration |
| `src/lib/components/LeadsList.svelte` | E0-P0A-FD4 | 1 related file | Lead listing UI and top action bar | high: existing CRM table; only add trigger button and modal binding |
| `src/lib/services/crmService.ts` | E0-P0A-FD5 | 3 related files | CRM service & event bus | medium: reuse `notifyLeadsChanged()` |

## Status Rules

| Status | Meaning | Allowed next action |
|--------|---------|---------------------|
| `correct` | Already behaves as required. | Preserve. Add evidence or tests only if needed. |
| `partial` | Some required behavior exists, but gaps remain. | Change only the missing parts. Preserve correct parts. |
| `wrong` | Current behavior, source, or contract is incorrect. | Replace with required behavior. Record the exact reason. |
| `missing` | Required behavior, source, or contract does not exist. | Implement the missing piece only. |
| `unbound` | Surface exists but is not wired to the real source, flow, or contract. | Bind to the real source only. Preserve approved surface. |
| `fake-or-stub` | Prototype, demo, mock, fallback, or placeholder data is being used as real behavior. | Remove fake behavior or replace it with an approved truthful state. |
| `blocked` | Source, authority, contract, or required evidence is unclear. | Stop. Do not implement until resolved. |

## Current Status Matrix

| Unit | Current State | Required State | Status | Relationship Count | Evidence | Next Plan Decision |
|------|---------------|----------------|--------|--------------------|----------|--------------------|
| Excel & CSV backend parser crates | `Cargo.toml` contains `calamine = "0.26"` and `csv = "1.3"` | `Cargo.toml` includes `calamine = "0.26"` and `csv = "1.3"` | `correct` | 0 related files | E1-P1A-DEP1 | preserve |
| Lead importer backend module | `src-tauri/src/lead_importer/` fully implemented and tested with 24 integration tests | Complete module with inspection, streaming import, normalization, dedup, identity tracking, and template generation | `correct` | 0 related files | E1-P1B-INSP1..E1-P1E-TST1, E4-P4A-TST24 | preserve |
| Tauri IPC import commands | `pick_lead_import_file`, `inspect_lead_import_file`, `execute_lead_import`, `download_lead_import_template` exposed and registered | Commands exposed via Tauri invoke and registered in `lib.rs` | `correct` | 4 related files | E2-P2A-CMD1, E2-P2B-TST1 | preserve |
| Lead import frontend service | `leadImportService.ts` implemented with header detection, BOM, and formula sanitization | Service with IPC wrappers, event progress callbacks, error CSV export, and template trigger | `correct` | 1 related file | E3-P3A-SVC1, E3-P3A-TST5 | preserve |
| Lead import UI Modal Wizard | `LeadImportModal.svelte` implemented with Svelte 5 runes, default currency, mapping validation, progress tracking | 4-step wizard modal (Upload & Template -> Smart Mapping -> Deduplication & Defaults -> Progress & Summary/Error Export) | `correct` | 3 related files | E3-P3B-UI1 | preserve |
| LeadsList Action Bar Integration | `LeadsList.svelte` contains "Import Excel / CSV" button triggering modal and refreshing on finish | Contains "Import Leads" button opening `LeadImportModal` | `correct` | 2 related files | E3-P3C-UI2 | preserve |
| i18n Translation Keys | `messages/vi.json` and `messages/en.json` contain all lead import dictionary keys | Fully translated strings for all 4 wizard steps, tooltips, errors, default currency, and template labels | `correct` | 0 related files | E3-P3D-I18N1 | preserve |

## Status Refresh Log

| Refresh | Date | Repo Basis | Changed Scope | Status Changes | Evidence | Next Phase Update |
|---------|------|------------|----------------|----------------|----------|-------------------|
| R0 | 2026-09-09 | baseline before P0 | Entire lead import scope | initial classification: all core units missing, LeadsList partial | E0-P0A-SRC1..E0-P0A-SRC7 | Proceed to P1-A after plan approval |
| R1 | 2026-09-09 | after P1-A Cargo.toml | `src-tauri/Cargo.toml` | Excel & CSV parser crates: missing -> correct | E1-P1A-DEP1 | Proceed to P1-B (types and inspector) |
| R2 | 2026-09-09 | after P1-B inspector | `src-tauri/src/lead_importer/types.rs`, `inspector.rs` | Inspection engine: missing -> correct | E1-P1B-INSP1 | Proceed to P1-C (streaming importer engine) |
| R3 | 2026-09-09 | after P1-C importer | `src-tauri/src/lead_importer/importer.rs` | Importer engine: missing -> correct | E1-P1C-IMP1 | Proceed to P1-D (sample template generator) |
| R4 | 2026-09-09 | after P1-D & P1-E template & tests | `src-tauri/src/lead_importer/` | Lead importer backend module: missing -> correct | E1-P1D-TMP1, E1-P1E-TST1 | Proceed to P2-A (Tauri IPC commands & dialog wiring) |
| R5 | 2026-09-09 | after P2 IPC commands & tests | `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs` | Tauri IPC import commands: missing -> correct | E2-P2A-CMD1, E2-P2B-TST1 | Proceed to P3 (Frontend Service & UI Modal Wizard) |
| R6 | 2026-09-09 | after P3 frontend service & modal | `src/lib/services/leadImportService.ts`, `LeadImportModal.svelte`, `LeadsList.svelte`, `messages/` | Frontend service, UI modal, LeadsList toolbar, and i18n: missing -> correct | E3-P3A-SVC1..E3-P3D-I18N1 | Proceed to P4 (QA Verification & Quality Gates) |
| R7 | 2026-09-09 | after Codex review runs 1-8 | `src-tauri/src/lead_importer/importer.rs`, `tests.rs`, `LeadImportModal.svelte` | Fixed F1 (currency validation), F2 (canonical phone matching), F3 (sheet invalidation), F4 (identity cleanup & stale batch removal) | E4-P4A-TST24, E4-P4B-CHK, E4-P4C-BLD, E4-P4D-ANV | Quality gates complete, final closure |






## Phase Touch Map

| Unit / File / Surface | Plan-Relevant Relationship File | Relationship to Target | Plan Item | Touch Mode | Evidence | Constraint |
|-----------------------|---------------------------------|------------------------|-----------|------------|----------|------------|
| `src-tauri/Cargo.toml` | `src-tauri/Cargo.lock` | package manifest | P1-A | edit | E0-P0A-SRC1 | preserve existing dependencies, add `calamine` and `csv` |
| `src-tauri/src/lead_importer/` | `src-tauri/src/db.rs` | database operations | P1-B, P1-C, P1-D | edit (new) | E0-P0A-SRC2 | isolate in dedicated module, use `rusqlite::TransactionBehavior::Immediate` |
| `src-tauri/src/commands.rs` | `src-tauri/src/lib.rs` | command registration | P2-A | edit | E0-P0A-SRC3 | keep command handlers minimal, delegate to `lead_importer` module |
| `src-tauri/src/lib.rs` | `src-tauri/src/commands.rs` | Tauri runtime entry | P2-A | edit | E0-P0A-SRC3 | register commands in `generate_handler![]` |
| `src/lib/services/leadImportService.ts` | `@tauri-apps/api/core` | frontend IPC caller | P3-A | edit (new) | E0-P0A-SRC4 | strict TypeScript typings |
| `src/lib/components/LeadImportModal.svelte` | `src/lib/components/ui/` | UI modal component | P3-B | edit (new) | E0-P0A-SRC5 | Svelte 5 runes (`$state`, `$derived`, `$props`), accessible UI |
| `src/lib/components/LeadsList.svelte` | `src/lib/services/crmService.ts` | CRM list container | P3-C | edit | E0-P0A-SRC6 | preserve existing multiselect, search, and category modals |
| `messages/vi.json` & `en.json` | `src/lib/paraglide/` | Paraglide i18n | P3-D | edit | E0-P0A-SRC7 | run `bun run i18n:compile` after editing |

## Detailed Findings

1. `leads` table in `src-tauri/src/db.rs` contains all needed columns: `name`, `company`, `status`, `value`, `last_activity`, `owner`, `email`, `phone`, `about`, `address`, `currency`. No schema migration is required.
2. `src-tauri/src/google_maps_jobs/importer.rs` provides a proven reference pattern for `normalize_vn_mobile`, `TransactionBehavior::Immediate`, and intra-batch + database deduplication.
3. `src/lib/utils/leadsExportCsv.ts` provides existing header order and `sanitizeCsvField` formula injection neutralization.
4. `LeadsList.svelte` has an existing header action bar where an "Import" button fits cleanly next to "Add Lead".
