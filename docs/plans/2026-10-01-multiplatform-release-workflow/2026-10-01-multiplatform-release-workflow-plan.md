# Multiplatform Desktop Release Workflow Plan

## Metadata

- Date: `2026-10-01`
- Status: `active`
- Plan: `docs/plans/2026-10-01-multiplatform-release-workflow/2026-10-01-multiplatform-release-workflow-plan.md`
- Evidence: `docs/plans/2026-10-01-multiplatform-release-workflow/2026-10-01-multiplatform-release-workflow-evidence.md`
- Benchmark: `docs/plans/2026-10-01-multiplatform-release-workflow/2026-10-01-multiplatform-release-workflow-benchmark.md`
- Actual status: `docs/plans/2026-10-01-multiplatform-release-workflow/2026-10-01-multiplatform-release-workflow-actual-status.md`

## Goal

Configure and execute a complete multiplatform release for RustSale CRM desktop app (v0.1.20) covering 100% of desktop targets across Windows (x86_64 NSIS + MSI), macOS (Apple Silicon M1-M4 + Intel x86_64), and Linux (Debian/Ubuntu deb + Universal AppImage), fixing CI packaging issues and CDN manifest collisions.

## Rules

- Complete P0 actual status before implementation work.
- Update each checklist item immediately when it is completed.
- Record evidence as work completes.
- Record benchmarkable counts or measurements when they are taken.
- Update later phase status assumptions, next actions, and work steps when actual-status evidence changes the repo state.
- After completing a phase or implementation slice and refreshing `actual-status.md`, update the next affected phase's work steps as needed to match the latest repo reality.

## Problem

1. macOS in GitHub Actions release workflow only built for Apple Silicon (`aarch64-apple-darwin`), leaving Intel Mac users unable to run the application.
2. In `scripts/publish-release.ts`, line 201 required `targetFile.includes(version)`, which skipped Tauri v2 macOS updater archives (`.app.tar.gz`) because their names lack version numbers, preventing macOS entries from appearing in `latest.json` on Cloudflare R2.
3. Linux was restricted to `--bundles deb` due to prior FUSE permission issues with `linuxdeploy` on CI, omitting `.AppImage` (which breaks the in-app Tauri updater on Linux and locks out non-Debian distros).
4. `TAURI_ENV_TARGET_TRIPLE` was not passed during the Cloak Runtime sidecar compilation step.

## Scope

- In scope:
  - `.github/workflows/release.yml`
  - `scripts/publish-release.ts`
  - Version bump in `package.json` and `src-tauri/tauri.conf.json` to `0.1.20`
  - Triggering GitHub Actions release pipeline via git push and tag `v0.1.20`
- Out of scope:
  - Mobile targets (iOS / Android)
  - Modifying application UI or backend business logic

## Non-Goals

- Refactoring the entire S3 upload client architecture.
- Changing desktop notification or system tray business logic.

## Requirements

- R1: Add `darwin-x86_64` to the GitHub Actions matrix, install `patchelf`, enable `APPIMAGE_EXTRACT_AND_RUN=1`, set `TAURI_ENV_TARGET_TRIPLE: ${{ matrix.target }}`, and pass explicit `--target` arguments to `tauri-action`.
- R2: Fix `scripts/publish-release.ts` to recognize macOS `.app.tar.gz` and Linux `.AppImage`, support direct S3 manifest fetch with cache-busting fallback, and map all platform updater keys.
- R3: Bump version to `0.1.20` in `package.json` and `src-tauri/tauri.conf.json`.
- R4: Verification checks pass (`bun run check`, `bun run scripts/publish-release.ts --dry-run --skip-build`, `bun --no-env-file run agent:verify c1499b1f-23d8-4621-8a9e-0c350e555591`).
- R5: Commit changes, tag `v0.1.20`, and push to origin to trigger GitHub Action release workflow.

## Acceptance Criteria

- Definition of done:
  - `.github/workflows/release.yml` contains all 4 matrix targets: `windows-x86_64`, `linux-x86_64`, `darwin-aarch64`, `darwin-x86_64`.
  - `scripts/publish-release.ts` dry-run detects updater and installer artifacts without dropping macOS or Linux.
  - SvelteKit `check` passes cleanly.
  - Agent verification exits with 0 and PASS.
  - Tag `v0.1.20` is pushed to remote GitHub repository.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: establish the real current state.
  - Work Steps: inspect source-of-truth files, classify each surface, record blocked or missing pieces.
  - Implementation Gate: `actual-status.md` completed.
  - Acceptance: actual status classifies `.github/workflows/release.yml` and `scripts/publish-release.ts` as `partial`.

### P1: Multiplatform Release Workflow & Script Hardening

- Phase Goal: Update workflow matrix, environment variables, dependencies, and publisher script.
- Phase Boundary:
  - In scope: `.github/workflows/release.yml`, `scripts/publish-release.ts`
  - Out of scope: changing core Rust or SvelteKit source code
  - Dependencies: bun, git

- [x] P1-A: Update `.github/workflows/release.yml` matrix and environment
  - Goal: configure 4 desktop matrix builds with AppImage FUSE bypass and target triples.
  - Work Steps:
    1. Add `darwin-x86_64` (`x86_64-apple-darwin`) to matrix.
    2. Add `args` parameter to each matrix entry.
    3. Install `patchelf` and `libfuse2` in Linux dependencies step.
    4. Set `APPIMAGE_EXTRACT_AND_RUN: '1'` and `NO_STRIP: 'true'` at job level.
    5. Pass `TAURI_ENV_TARGET_TRIPLE: ${{ matrix.target }}` into sidecar build step.
  - Implementation Gate: syntax review.
  - Acceptance: `.github/workflows/release.yml` has 4 platforms configured.

- [x] P1-B: Update `scripts/publish-release.ts` artifact scanning and S3 manifest sync
  - Goal: ensure macOS `.app.tar.gz` and Linux `.AppImage` are scanned and mapped, and prevent CDN cache overwrites.
  - Work Steps:
    1. Update line 201 to accept `.tar.gz` and `.zip` updater archives without version substring.
    2. Add S3 `GetObjectCommand` direct fetch for `latest.json` with cache-busting HTTP fallback.
    3. Standardize platform mapping for `darwin-aarch64`, `darwin-x86_64`, `linux-x86_64`, `windows-x86_64`.
  - Implementation Gate: dry-run test passes without throwing errors.
  - Acceptance: `bun run scripts/publish-release.ts --dry-run --skip-build` runs cleanly.

- [x] P1-C: Bump application version to `0.1.20` and verify
  - Goal: synchronize version in `package.json` and `src-tauri/tauri.conf.json`.
  - Work Steps:
    1. Update `package.json` version to `0.1.20`.
    2. Update `src-tauri/tauri.conf.json` version to `0.1.20`.
    3. Run `bun run check`.
    4. Run `bun --no-env-file run agent:verify c1499b1f-23d8-4621-8a9e-0c350e555591`.
  - Implementation Gate: all checks pass with exit 0.
  - Acceptance: `agent:verify` reports PASS.

- [ ] P1-D: Commit, tag `v0.1.20`, and push to trigger GitHub Actions
  - Goal: publish tag `v0.1.20` to GitHub remote to trigger multiplatform build and release.
  - Work Steps:
    1. Stage changed files: `.github/workflows/release.yml`, `scripts/publish-release.ts`, `package.json`, `src-tauri/tauri.conf.json`, `latest.json`, `docs/plans/`, `.agents/task.json`.
    2. Commit with descriptive message.
    3. Create git tag `v0.1.20`.
    4. Push commit and tag to `origin master`.
    5. Monitor GitHub Action run status via `gh run list` / `gh run view`.
  - Implementation Gate: clean git working tree before tag and push.
  - Acceptance: GitHub Actions workflow is triggered and running.

## Risk Notes

- Building for Intel Mac on `macos-14` requires `dtolnay/rust-toolchain` target `x86_64-apple-darwin`, which is lightweight and supported by Apple's universal SDK in Xcode.
- `APPIMAGE_EXTRACT_AND_RUN=1` avoids FUSE mount failures in Ubuntu containers.
