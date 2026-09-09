# Lead Import Excel CSV Evidence Ledger

## Metadata

- Date: `2026-09-09`
- Plan: `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-plan.md`
- Evidence: `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-evidence.md`
- Benchmark: `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-benchmark.md`
- Actual status: `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-actual-status.md`

## Evidence Rules

The evidence file explains why the work is known to be correct.

It contains:
- metadata and companion files;
- evidence rules;
- evidence sections by phase (`E0`, `E1`, `E2`, `E3`, `E4`);
- source inspection, codebase facts, and document facts;
- commands run and pass/fail result;
- impact or blast-radius evidence when code/graph behavior changes;
- implementation evidence: files changed and behavior changed;
- validation evidence: build, tests, traces;
- failures encountered and how they were handled;
- detect-changes before commit;
- commit hash and closure evidence.

### Evidence ID Naming

Format:
```text
E<phase>-<item>-<kind><n>
```

- `E0`: P0 baseline and discovery evidence.
- `E1`: P1 Backend Rust core importer & template engine.
- `E2`: P2 Tauri IPC commands & dialog wiring.
- `E3`: P3 Frontend service & UI wizard modal.
- `E4`: P4 Verification, quality gate & closure.

## E0 - P0 Evidence

Matching plan item(s): `P0-A`

- `E0-P0A-FD1`: `anvien file-detail src-tauri/Cargo.toml --json` confirms `Cargo.toml` is a parsed config file with 0 dependencies and low risk.
- `E0-P0A-FD2`: `anvien file-detail src-tauri/src/commands.rs --json` confirms 1258 symbols, 4 related files, high risk (central IPC hub).
- `E0-P0A-FD3`: `anvien file-detail src-tauri/src/lib.rs --json` confirms 94 symbols, 0 related files in projection.
- `E0-P0A-FD4`: `anvien file-detail src/lib/components/LeadsList.svelte --json` confirms 99 symbols, 1 related file (`crmService.ts`).
- `E0-P0A-FD5`: `anvien file-detail src/lib/services/crmService.ts --json` confirms 171 symbols, 3 related files.
- `E0-P0A-SRC1`: Inspection of `src-tauri/Cargo.toml` lines 20-66 reveals `calamine` and `csv` crates are missing.
- `E0-P0A-SRC2`: File search confirms `src-tauri/src/lead_importer/` directory does not exist.
- `E0-P0A-SRC3`: Grep search for `import_leads` in `src-tauri` returns 0 occurrences; commands are completely missing.
- `E0-P0A-SRC4`: File search confirms `src/lib/services/leadImportService.ts` does not exist.
- `E0-P0A-SRC5`: File search confirms `src/lib/components/LeadImportModal.svelte` does not exist.
- `E0-P0A-SRC6`: Inspection of `src/lib/components/LeadsList.svelte` lines 307-322 confirms only "Export CSV", "Assign Category", and "Add Lead" buttons exist in header.
- `E0-P0A-SRC7`: Inspection of `messages/vi.json` and `messages/en.json` confirms no translation keys for `leads_import_*` exist.

## E1 - P1 Evidence

Matching plan item(s): `P1-A`, `P1-B`, `P1-C`, `P1-D`, `P1-E`

- `E1-P1A-DEP1`: Added `calamine = "0.26"` and `csv = "1.3"` to `src-tauri/Cargo.toml`. Ran `cargo check --manifest-path src-tauri/Cargo.toml` which locked `calamine v0.26.1` and `csv v1.4.0` and exited with code 0.
- `E1-P1B-INSP1`: Created `src-tauri/src/lead_importer/types.rs`, `inspector.rs`, `mod.rs`. Declared `pub mod lead_importer` in `lib.rs`. Verified unit tests `test_cell_to_string_variations`, `test_inspect_csv_file_success`, and `test_inspect_nonexistent_file_returns_error` pass 3/3 in `cargo test lead_importer`.
- `E1-P1C-IMP1`: Implemented `src-tauri/src/lead_importer/importer.rs` with `normalize_vn_mobile` (+84, 84, 0084, missing leading 0 auto-prepend, 03/05/07/08/09 prefixes), `normalize_email`, and SQLite batch transactional import (`TransactionBehavior::Immediate`). Verified `test_normalize_vn_mobile_matrix`, `test_normalize_email`, `test_execute_lead_import_overwrite_conflict`, and `test_execute_lead_import_skip_and_error_quarantine` pass in `cargo test lead_importer`.
- `E1-P1D-TMP1`: Implemented `src-tauri/src/lead_importer/template.rs` generating in-memory UTF-8 BOM CSV and compressed OpenXML `.xlsx` with inline string phone formatting.
- `E1-P1E-TST1`: Ran `cargo test lead_importer` expanding test suite to 24 tests covering multiline quoted fields, upfront currency validation, transactional rollback, symmetric duplicate matching across formats, and obsolete identity key cleanup across repeated updates. All 24 tests passed cleanly.


## E2 - P2 Evidence

Matching plan item(s): `P2-A`, `P2-B`

- `E2-P2A-CMD1`: Implemented Tauri IPC commands `pick_lead_import_file`, `inspect_lead_import_file`, `execute_lead_import`, and `download_lead_import_template` in `src-tauri/src/commands.rs` with `spawn_blocking` and dual progress reporting (IPC Channel + app event). Registered all commands in `src-tauri/src/lib.rs`. `cargo check` passed cleanly in 11s.
- `E2-P2B-TST1`: Added integration tests `test_command_inspect_lead_import_file` and `test_command_inspect_nonexistent_file` in `src-tauri/src/commands_test.rs`. Ran `cargo test test_command_inspect` which passed 2/2 in 0.00s.


## E3 - P3 Evidence

Matching plan item(s): `P3-A`, `P3-B`, `P3-C`, `P3-D`

- `E3-P3A-SVC1`: Implemented `src/lib/services/leadImportService.ts` with header auto-detection (VI and EN), formula injection sanitization (`sanitizeCsvField` escaping `=`, `+`, `-`, `@`, tab, cr), and UTF-8 BOM error CSV export.
- `E3-P3A-TST5`: Implemented `src/lib/services/__tests__/leadImportService.spec.ts` with 5 automated unit tests verifying header mapping detection, error row CSV generation with UTF-8 BOM, and formula injection sanitization. All 5 tests pass in `bun test`.
- `E3-P3B-UI1`: Implemented `src/lib/components/LeadImportModal.svelte` using Svelte 5 runes (`$state`, `$derived`), 4-step wizard modal (Upload & Template -> Smart Mapping -> Deduplication & Defaults -> Progress & Summary), default currency selector (VND/USD), mapping validation, sheet invalidation, and real-time dual progress tracking.
- `E3-P3C-UI2`: Integrated `LeadImportModal` into `src/lib/components/LeadsList.svelte` with "Import Excel / CSV" button in the table action toolbar next to "Export CSV", automatically refreshing leads list on completion.
- `E3-P3D-I18N1`: Added complete bilingual dictionary entries (`leads_import_*`) to `messages/vi.json` and `messages/en.json`, compiled with `bun run i18n:compile`.


## E4 - P4 Evidence

Matching plan item(s): `P4-A`, `P4-B`, `P4-C`

- `E4-P4A-TST24`: Ran `cargo test --manifest-path src-tauri/Cargo.toml lead_importer`: All 24 unit and integration tests passed (0 failed).
- `E4-P4A-ALLRUST`: Ran full `cargo test --manifest-path src-tauri/Cargo.toml`: All tests passed across all modules.
- `E4-P4B-CHK`: Ran `bun run check`: Svelte-check found 0 errors across all files.
- `E4-P4C-BUNT`: Ran `bun test`: All 693 tests passed (0 failed, 1 skip) across 75 test files.
- `E4-P4D-BLD`: Ran `bun run build`: Production build with `@sveltejs/adapter-static` succeeded in 19s (client) and 42s (server).
- `E4-P4E-ANV`: Ran `anvien analyze --force`: Semantic graph refreshed successfully (700 files scanned, 423 parsed, 95848 nodes, 110114 relationships). Upstream impact of `src-tauri/src/lead_importer/importer.rs` verified as LOW risk.
- `E4-P4F-CDX`: Codex review iterations (Runs 1 through 8) completely resolved all findings: F1 (currency validation), F2 (canonical phone duplicate matching), F3 (sheet switch inspection invalidation), and F4 (complete cleanup of indexed aliases and elimination of stale batch caches). Codex Run 8 reported `findings: []` with 0 actionable defects.


## Closure Evidence

- All core backend, IPC commands, frontend service, UI wizard modal, toolbar integration, and i18n translation tasks completed.
- Full quality gates verified with 0 errors and 0 test failures.
- Codex review confirmed finding F4 resolved at source level with zero remaining defects.
