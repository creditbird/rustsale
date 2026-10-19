# Molecular Production Audit & Remediation Benchmark Ledger

## Metadata

- Date: `2026-10-19`
- Status: `active`
- Plan: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-plan.md`
- Evidence: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-evidence.md`
- Benchmark: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-benchmark.md`
- Actual status: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-actual-status.md`

## Benchmark Inventory

| Metric | Baseline | Target | Actual | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `svelte-check --threshold warning` Errors / Warnings | 0 errors, 6 warnings | 0 errors, 0 warnings | 0 errors, 0 warnings | Eliminated all `state_referenced_locally` and `a11y_label_has_associated_control` warnings |
| `svelte-autofixer` Component Issues (52 files) | 19 files with issues (68 unkeyed `#each` + 1 `{{name}}` bug + 2 unused ignores + 1 `$effect` sync) | 0 issues across all 52 files | 0 issues across all 52 files | Verified via `mcp_svelte_svelte-autofixer` |
| Backend Security & Async Blocking Issues (`commands.rs`, `conversation/service.rs`) | 4 (`install_portable_linux_update`, `download_lead_import_template`, Zalo & Messenger outbox `std::fs::read`) | 0 | 0 | Strict version/path validation, `spawn_blocking` extraction, symlink-safe export, async outbox attachment reader with 50 MiB limit |
| Frontend Unit Tests (`bun test src`) | 708 pass, 0 fail | >= 711 pass, 0 fail | 711 pass, 0 fail | Full frontend suite + `MolecularAuditPass2.spec.ts` (3 red->green regression tests) |
| Rust Backend Tests (`cargo test`) | Pass (0 fail) | Pass (0 fail) | Pass (0 fail) + `validates_release_version_strictly` | Full Rust test suite + new security unit test |
