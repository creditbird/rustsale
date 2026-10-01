# Multiplatform Desktop Release Workflow Evidence Ledger

## Metadata

- Date: `2026-10-01`
- Status: `active`
- Plan: `docs/plans/2026-10-01-multiplatform-release-workflow/2026-10-01-multiplatform-release-workflow-plan.md`
- Evidence: `docs/plans/2026-10-01-multiplatform-release-workflow/2026-10-01-multiplatform-release-workflow-evidence.md`
- Benchmark: `docs/plans/2026-10-01-multiplatform-release-workflow/2026-10-01-multiplatform-release-workflow-benchmark.md`
- Actual status: `docs/plans/2026-10-01-multiplatform-release-workflow/2026-10-01-multiplatform-release-workflow-actual-status.md`

## Evidence Sections

### E0: Baseline Inspection Evidence

- `E0-P0A-SRC1`: Inspected `.github/workflows/release.yml`. Observed missing `darwin-x86_64` matrix entry, Linux limited to `--bundles deb`, missing `APPIMAGE_EXTRACT_AND_RUN=1`, missing `patchelf`, and missing `TAURI_ENV_TARGET_TRIPLE` in sidecar build.
- `E0-P0A-SRC2`: Inspected `scripts/publish-release.ts`. Observed line 201 `if (!targetFile.includes(version)) continue;` which causes macOS `.app.tar.gz` to be ignored because its filename has no version.
- `E0-P0A-SRC3`: Inspected `src-tauri/tauri.conf.json`. Current version: `0.1.19`.
- `E0-P0A-SRC4`: Inspected `package.json`. Current version: `0.1.19`.
- `E0-P0A-R2`: Tested `curl -s https://rustsale.trantrongtri.com/latest.json`. Verified that only `linux-x86_64` (deb), `windows-x86_64`, and `windows-x86_64-msi` are present. macOS was entirely missing.

### E1: Implementation & Verification Evidence

- `E1-P1A-WF1`: Updated `.github/workflows/release.yml`. Configured 4 matrix entries: `windows-x86_64`, `linux-x86_64`, `darwin-aarch64`, `darwin-x86_64`. Enabled `APPIMAGE_EXTRACT_AND_RUN: '1'`, `NO_STRIP: 'true'`, added `patchelf`, set `TAURI_ENV_TARGET_TRIPLE: ${{ matrix.target }}`, and set `args: ${{ matrix.args }}`.
- `E1-P1B-PUB1`: Updated `scripts/publish-release.ts`. Changed signature scan to allow updater archives (`.tar.gz`, `.zip`) without version substring in filename. Added direct S3 `GetObjectCommand` fetch with cache-busting fallback. Added `darwin-x86_64` and `linux-x86_64-appimage` platform mapping.
- `E1-P1C-VER1`: Bumped version in `package.json` and `src-tauri/tauri.conf.json` from `0.1.19` to `0.1.20`.
- `E1-P1C-VER2`: Executed `bun run check`. Passed with 0 errors and 6 warnings.
- `E1-P1C-VER3`: Executed `bun --no-env-file run agent:verify c1499b1f-23d8-4621-8a9e-0c350e555591`. Passed with exit code 0 and `{"status":"PASS"}` on evidence path `/home/arch/.local/state/rustsale-agent/b7890786d3897ab36726/005826f106d44ea14465fc62/evidence.json`.
