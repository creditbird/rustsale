# Google Maps Failed Job Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make transient blank pages retryable, reopen failed jobs safely, and import durable ready results before the crawl completes.

**Architecture:** Extend the browser health contract with `blank_page`, reuse the runner's existing retry policy, broaden only the explicit user-retry transition, and add a ready-only importer path called after result checkpoints and before failed-job restart. Reuse the existing Svelte action callback and button components.

**Tech Stack:** Rust, Tokio, rusqlite, Bun, TypeScript, Svelte 5, Tauri 2, CloakBrowser sidecar.

---

### Task 1: Reproduce blank-page classification

**Files:**
- Modify: `sidecars/cloak-runtime/src/index.test.ts`
- Modify: `src-tauri/src/scrapers/google_maps.rs`

- [x] Add a sidecar test asserting an empty snapshot is `blank_page` while non-empty unknown DOM remains `selector_changed`.
- [x] Add a Rust scraper test asserting persistent `BlankPage` becomes retryable `TransientNetwork`.
- [x] Run `bun test sidecars/cloak-runtime/src/index.test.ts` and the targeted Cargo test; confirm failure before implementation.

### Task 2: Implement blank-page retry

**Files:**
- Modify: `sidecars/cloak-runtime/src/googleMaps.ts`
- Modify: `src-tauri/src/scrapers/browser.rs`
- Modify: `src-tauri/src/scrapers/google_maps.rs`

- [x] Add `blank_page` to both health-state contracts.
- [x] Preserve the readiness polling window for blank pages.
- [x] Convert an exhausted blank page to retryable `transient_network`.
- [x] Run the targeted sidecar and Rust tests until green.

### Task 3: Reproduce failed-job recovery

**Files:**
- Modify: `src-tauri/src/google_maps_jobs/repository.rs`
- Modify: `src/lib/components/__tests__/GoogleMapsScraper.spec.ts`

- [x] Add a repository test for `failed -> queued` user retry resetting a failed selector unit without resetting completed units.
- [x] Add a component test showing and invoking `Retry Failed Units` for `failed`.
- [x] Run targeted Rust and Bun tests; confirm failure before implementation.

### Task 4: Implement failed-job recovery

**Files:**
- Modify: `src-tauri/src/google_maps_jobs/repository.rs`
- Modify: `src-tauri/src/google_maps_jobs/commands.rs`
- Modify: `src/lib/components/GoogleMapsJobPanel.svelte`

- [x] Permit explicit user retry from `partial` and `failed`.
- [x] Requeue recoverable and selector-changed failed units atomically.
- [x] Refresh the job version after pre-retry lead import.
- [x] Render the existing retry action for failed jobs with failed units.
- [x] Run targeted Rust, Bun, and Svelte checks.

### Task 5: Reproduce incremental import

**Files:**
- Modify: `src-tauri/src/google_maps_jobs/importer.rs`
- Modify: `src-tauri/src/google_maps_jobs/repository.rs`

- [x] Add an importer test proving ready results import while pending enrichment is preserved.
- [x] Add a checkpoint test proving ready results update leads and job counters before finalization.
- [x] Run targeted Cargo tests; confirm failure before implementation.

### Task 6: Implement incremental import

**Files:**
- Modify: `src-tauri/src/google_maps_jobs/importer.rs`
- Modify: `src-tauri/src/google_maps_jobs/repository.rs`
- Modify: `src-tauri/src/google_maps_jobs/commands.rs`

- [x] Add ready-only import filtering without changing final import semantics.
- [x] Flush ready results after result-bearing checkpoints.
- [x] Flush older ready results before retrying a failed job.
- [x] Reload and return current job counters after each flush.
- [x] Run targeted Cargo tests until green.

### Task 7: Verify integration

**Files:**
- Verify: all modified files

- [x] Run `bun run test:cloak-runtime`.
- [x] Run `bun run test`.
- [x] Run `bun run check`.
- [x] Check disk space, then run targeted and full Cargo tests.
- [x] Run `cargo fmt --check` and `cargo clippy -- -D warnings`.
- [x] Run `mcp__gitnexus.detect_changes` and inspect affected flows.
- [x] Run `git diff --check` and review only expected files.

## Verification Evidence

- `bun run test:cloak-runtime`: 10 passed, 1 skipped, 0 failed.
- `bun run test`: 42 passed, 1 skipped, 0 failed.
- `bun run check`: 0 errors, 0 warnings.
- `bun run build`: completed successfully.
- `cargo test`: 178 passed, 0 failed.
- `cargo fmt --check`: passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `cargo build --release`: passed.
- `bun run build:cloak-runtime`: rebuilt the host sidecar successfully.
- Production DB copy recovery probe: 935 completed units preserved, one selector failure requeued, all 140 results preserved, 116 results ready for incremental import.
- GitNexus staged diff reported `CRITICAL` because file-level change detection marked unrelated symbols inside central Rust files; exact hunk review and per-symbol impacts were LOW. Reviewed flows: `Start_current → Open_db_connection`, `Execute_search → LaunchRequest`, and `A_failed_place_unit_does_not_abort_the_remaining_units → Load_job`.
