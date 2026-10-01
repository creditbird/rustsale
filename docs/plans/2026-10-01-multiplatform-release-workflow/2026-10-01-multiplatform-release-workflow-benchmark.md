# Multiplatform Desktop Release Workflow Benchmark Ledger

## Metadata

- Date: `2026-10-01`
- Status: `active`
- Plan: `docs/plans/2026-10-01-multiplatform-release-workflow/2026-10-01-multiplatform-release-workflow-plan.md`
- Evidence: `docs/plans/2026-10-01-multiplatform-release-workflow/2026-10-01-multiplatform-release-workflow-evidence.md`
- Benchmark: `docs/plans/2026-10-01-multiplatform-release-workflow/2026-10-01-multiplatform-release-workflow-benchmark.md`
- Actual status: `docs/plans/2026-10-01-multiplatform-release-workflow/2026-10-01-multiplatform-release-workflow-actual-status.md`

## Benchmark Inventory

| Metric | Baseline (v0.1.19) | Target (v0.1.20) | Actual (v0.1.20) | Notes |
| :--- | :--- | :--- | :--- | :--- |
| Supported Desktop OS Platforms | 2 (Win x64, Linux deb only) | 4 (Win x64, macOS arm64, macOS x64, Linux) | 4 | Complete desktop coverage |
| Linux Packaging Formats | 1 (`.deb`) | 2 (`.deb`, `.AppImage`) | 2 | AppImage enables universal portability |
| macOS Architectures | 1 (`aarch64` only, broken updater) | 2 (`aarch64` + `x86_64`) | 2 | Native support for M-series and Intel Macs |
| Windows Installers | 2 (`.exe` NSIS, `.msi` WiX) | 2 (`.exe` NSIS, `.msi` WiX) | 2 | Full installer support |
