# Google Maps Scraper Production Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Do not use subagents unless the user explicitly authorizes delegation.

**Goal:** Replace the current fire-and-forget Google Maps scraper with a persistent, resumable, bounded, cross-platform job engine using an officially downloaded CloakBrowser runtime, typed browser errors, safe email enrichment, and idempotent Lead import.

**Architecture:** Rust owns durable jobs, checkpoints, grid planning, email enrichment, Lead import, commands, and events. A Bun-compiled `cloak-runtime` sidecar owns the official CloakBrowser Playwright context and exposes a typed newline-delimited JSON protocol. SQLite is the source of truth; UI events are only notifications and are scoped by job ID and sequence.

**Tech Stack:** Rust 2021, Tauri v2, Tokio, rusqlite/WAL, serde, thiserror, async-trait, tokio-util cancellation tokens, reqwest/rustls, scraper HTML parser, URL/public-suffix validation, Bun, TypeScript, official `cloakbrowser` package, Playwright Core, Svelte 5, shadcn-svelte.

**Approved Spec:** `docs/superpowers/specs/2026-07-26-google-maps-scraper-production-design.md`

**Repository Constraints:** Use `bun`/`bunx`, never `npm`/`npx`. Run `df -h . /tmp` before every Cargo build/check/test/clippy command. Run GitNexus impact analysis before editing every existing symbol. Do not commit unless the user explicitly requests commits; record task checkpoints in the execution ledger instead.

---

## File Structure

### New Rust job-engine files

- `src-tauri/src/google_maps_jobs/mod.rs` — module exports and shared service state.
- `src-tauri/src/google_maps_jobs/types.rs` — job, unit, result, status, stage, error, and event DTOs.
- `src-tauri/src/google_maps_jobs/repository.rs` — SQLite persistence, transitions, checkpoint transactions, startup recovery.
- `src-tauri/src/google_maps_jobs/validation.rs` — request validation and grid preflight.
- `src-tauri/src/google_maps_jobs/runner.rs` — one-worker execution loop, retry, cancellation, checkpointing.
- `src-tauri/src/google_maps_jobs/events.rs` — sequenced job-scoped event publishing.
- `src-tauri/src/google_maps_jobs/email.rs` — safe HTTP client, robots rules, HTML/email extraction.
- `src-tauri/src/google_maps_jobs/importer.rs` — transactional phone/email Lead import.
- `src-tauri/src/google_maps_jobs/commands.rs` — Tauri job commands and legacy wrappers.

### Existing Rust files

- `src-tauri/src/db.rs` — add scraper schema and reusable configured connection helper.
- `src-tauri/src/db_test.rs` — schema, indexes, foreign keys, migration idempotency tests.
- `src-tauri/src/lib.rs` — initialize logging/job service and register commands.
- `src-tauri/src/commands.rs` — remove legacy implementation after compatibility wrappers delegate.
- `src-tauri/src/commands_test.rs` — migrate command tests to job-oriented behavior.
- `src-tauri/src/scrapers/browser.rs` — replace direct Chromium/CDP ownership with typed sidecar client/supervisor.
- `src-tauri/src/scrapers/browser_test.rs` — protocol, lifecycle, timeout, cleanup tests.
- `src-tauri/src/scrapers/google_maps.rs` — typed search contract and no swallowed errors.
- `src-tauri/src/scrapers/grid.rs` — bounded clipped grid planner and subdivision policy.
- `src-tauri/src/scrapers/mod.rs` — module exports.
- `src-tauri/Cargo.toml` — add job/runtime/enrichment/logging dependencies.
- `src-tauri/tauri.conf.json` — register sidecar external binary.
- `src-tauri/capabilities/default.json` — permit the required sidecar scope only.

### New CloakBrowser sidecar files

- `sidecars/cloak-runtime/src/index.ts` — protocol server and official runtime adapter.
- `sidecars/cloak-runtime/src/protocol.ts` — request/response/error types.
- `sidecars/cloak-runtime/src/runtime.ts` — ensure, inspect, launch, health-check, and shutdown.
- `sidecars/cloak-runtime/src/googleMaps.ts` — domain browser commands and block detection.
- `sidecars/cloak-runtime/src/index.test.ts` — protocol tests.
- `scripts/build-cloak-runtime.ts` — Bun compile script using the active Rust target triple.
- `src-tauri/binaries/.gitkeep` — generated sidecar output directory marker.

### Frontend files

- `src/lib/services/googleMapsScraperService.ts` — typed commands, events, DTOs.
- `src/lib/components/GoogleMapsRuntimePanel.svelte` — runtime download/status controls.
- `src/lib/components/GoogleMapsJobPanel.svelte` — active job, action-required, progress, controls.
- `src/lib/components/GoogleMapsJobHistory.svelte` — persisted history and resume/delete actions.
- `src/lib/components/GoogleMapsScraper.svelte` — compose new panels while preserving preset/geocode/custom bbox inputs.
- `src/lib/components/__tests__/GoogleMapsScraper.spec.ts` — component behavior and stale-event tests.
- `src/lib/services/__tests__/googleMapsScraperService.spec.ts` — command/event contract tests.

### Tooling and release files

- `package.json` — exact CloakBrowser/Playwright dependencies and sidecar scripts.
- `bun.lock` — locked sidecar dependencies.
- `bunfig.toml` — browser test environment corrections if required by baseline diagnosis.
- `.github/workflows/google-maps-scraper.yml` — Linux/Windows/macOS build and lifecycle matrix.
- `docs/superpowers/plans/2026-07-26-google-maps-scraper-production-ledger.md` — requirement and QA evidence ledger.

---

### Task 1: Baseline, Impact, and Execution Ledger

**Files:**
- Create: `docs/superpowers/plans/2026-07-26-google-maps-scraper-production-ledger.md`
- Inspect: all files listed in the File Structure section

- [x] **Step 1: Record the untouched baseline**

Run:

```bash
git status --porcelain=v1
git diff --check
df -h . /tmp && cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml
df -h . /tmp && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
bun run check
bun run test
bun run build
```

Expected baseline from the approved audit:

- Rust tests: 36 passed.
- Rust format/clippy: existing failures recorded, not hidden.
- Svelte check: zero errors and warnings.
- Frontend tests: existing harness failures recorded.
- Frontend build: succeeds with large-chunk warning.

- [x] **Step 2: Run mandatory GitNexus impact analysis**

Run upstream impact for every existing symbol that will be edited:

```text
init_db
run
BrowserState
BrowserManager
start
acquire
close
navigate
evaluate
GoogleMapsScraper
scrape_url
extract_current_place
generate_grid
radius_to_zoom
search_grid
run_google_maps_scrape_task
start_google_maps_scrape
stop_google_maps_scrape
startScrape
stopScrape
```

Record exact risk, d=1 callers, affected processes, and warnings in the ledger. If any result is HIGH or CRITICAL, stop before implementation and report it to the user.

- [x] **Step 3: Create the requirement ledger**

Create rows `REQ-001` through `REQ-020` covering:

```text
REQ-001 persistent jobs
REQ-002 atomic checkpoints
REQ-003 startup interruption recovery
REQ-004 one active worker
REQ-005 typed browser errors
REQ-006 official runtime download/cache
REQ-007 cross-platform sidecar builds
REQ-008 persistent CAPTCHA profile
REQ-009 backend grid validation
REQ-010 bounded adaptive subdivision
REQ-011 raw listing saturation
REQ-012 spatial filtering and dedup
REQ-013 safe email enrichment
REQ-014 phone/email Lead qualification
REQ-015 idempotent Lead import
REQ-016 job-scoped sequenced events
REQ-017 pause/resume/cancel race safety
REQ-018 structured diagnostics and cleanup
REQ-019 compatibility wrappers
REQ-020 full release QA
```

- [x] **Step 4: Record checkpoint**

Expected: ledger contains baseline commands, impact results, requirement ownership, and no implementation claim.

---

### Task 2: SQLite Schema and Domain Types

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/db_test.rs`
- Create: `src-tauri/src/google_maps_jobs/mod.rs`
- Create: `src-tauri/src/google_maps_jobs/types.rs`
- Modify: `src-tauri/src/lib.rs`

- [x] **Step 1: Write failing schema tests**

Add tests asserting:

```rust
#[test]
fn google_maps_job_schema_is_idempotent_and_enforces_keys() {
    let conn = crate::db::init_db(":memory:").unwrap();
    for table in [
        "google_maps_scrape_jobs",
        "google_maps_scrape_units",
        "google_maps_scrape_results",
    ] {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "missing table {table}");
    }
}

#[test]
fn google_maps_unit_key_is_unique_per_job() {
    // Insert one job and duplicate (job_id, unit_key); expect constraint failure.
}
```

- [x] **Step 2: Verify RED**

Run:

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml google_maps_job_schema -- --nocapture
```

Expected: FAIL because the tables do not exist.

- [x] **Step 3: Add domain enums and DTOs**

Implement serializable enums with exact string forms:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrapeJobStatus {
    Queued,
    Running,
    Paused,
    PausedRequiresAction,
    PausedLimitReached,
    Interrupted,
    Partial,
    Completed,
    Failed,
    Cancelled,
}

impl ScrapeJobStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Partial | Self::Completed | Self::Failed | Self::Cancelled)
    }
}
```

Also define `ScrapeStage`, `ScrapeUnitStatus`, `ScrapeMode`, `ScrapeErrorKind`, `ScrapeJob`, `ScrapeUnit`, `ScrapeResult`, `CreateScrapeJobRequest`, and `ScrapeJobEvent`.

- [x] **Step 4: Add schema and configured connection helper**

Add:

```rust
pub fn open_db_connection(db_path: &str) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}
```

Create the three approved tables, indexes on status/job/unit keys, and Lead indexes on phone and `lower(trim(email))`.

- [x] **Step 5: Verify GREEN**

Run:

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml google_maps_ -- --nocapture
```

Expected: all new schema tests pass.

- [x] **Step 6: Record checkpoint**

Update ledger requirements `REQ-001`, `REQ-002`, and `REQ-015` with files and test evidence.

---

### Task 3: Repository State Machine and Atomic Checkpoints

**Files:**
- Create: `src-tauri/src/google_maps_jobs/repository.rs`
- Modify: `src-tauri/src/google_maps_jobs/mod.rs`
- Test: `src-tauri/src/google_maps_jobs/repository.rs`

- [x] **Step 1: Write failing transition tests**

Cover:

```rust
#[test]
fn terminal_job_rejects_later_transition() {
    assert!(!is_valid_transition(
        ScrapeJobStatus::Completed,
        ScrapeJobStatus::Running,
    ));
}

#[test]
fn startup_recovery_marks_running_jobs_interrupted() {
    // Persist Running, call recover_interrupted_jobs, assert Interrupted.
}

#[test]
fn checkpoint_rolls_back_results_when_unit_update_fails() {
    // Force a constraint failure inside checkpoint transaction and assert no result persisted.
}
```

- [x] **Step 2: Verify RED**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml google_maps_jobs::repository -- --nocapture
```

Expected: FAIL because repository functions do not exist.

- [x] **Step 3: Implement explicit transition matrix**

```rust
pub fn is_valid_transition(
    from: ScrapeJobStatus,
    to: ScrapeJobStatus,
    cause: TransitionCause,
) -> bool {
    use ScrapeJobStatus::*;
    matches!(
        (from, to),
        (Queued, Running)
            | (Running, Paused)
            | (Running, PausedRequiresAction)
            | (Running, PausedLimitReached)
            | (Running, Interrupted)
            | (Running, Partial)
            | (Running, Completed)
            | (Running, Failed)
            | (Running, Cancelled)
            | (Paused, Running)
            | (Paused, Cancelled)
            | (PausedRequiresAction, Running)
            | (PausedRequiresAction, Cancelled)
            | (PausedLimitReached, Running)
            | (PausedLimitReached, Cancelled)
            | (Interrupted, Running)
            | (Interrupted, Cancelled)
            | (Partial, Queued) if cause == TransitionCause::UserRetry
    )
}
```

`TransitionCause` is `System`, `UserAction`, or `UserRetry`. A user retry increments `run_generation`, resets only retryable failed units to `pending`, and then queues the job. Background tasks may never reopen a quiescent terminal job.

- [x] **Step 4: Implement repository operations**

Required methods:

```rust
create_job
get_job
list_jobs
transition_job
recover_interrupted_jobs
insert_units
claim_next_unit
schedule_retry
checkpoint_completed_unit
checkpoint_failed_unit
upsert_results
list_results_for_enrichment
list_results_for_import
delete_terminal_job
```

Every mutating method opens a configured SQLite connection inside `spawn_blocking` and uses a transaction where multiple rows change.

- [x] **Step 5: Verify GREEN and concurrency behavior**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml google_maps_jobs::repository -- --nocapture
```

Expected: transition, recovery, rollback, uniqueness, and optimistic-version tests pass.

- [x] **Step 6: Record checkpoint**

Update `REQ-001`, `REQ-002`, and `REQ-003`.

---

### Task 4: Backend Validation and Bounded Grid Planner

**Files:**
- Create: `src-tauri/src/google_maps_jobs/validation.rs`
- Modify: `src-tauri/src/scrapers/grid.rs`
- Test: `src-tauri/src/scrapers/grid.rs`
- Test: `src-tauri/src/google_maps_jobs/validation.rs`

- [x] **Step 1: Write failing validation tests**

Add exact cases:

```rust
#[test]
fn rejects_zero_negative_nan_and_infinite_step() {}

#[test]
fn rejects_reversed_or_out_of_range_bbox() {}

#[test]
fn returns_recommended_step_when_initial_cell_cap_is_exceeded() {}

#[test]
fn clips_last_cell_to_bbox_and_keeps_center_inside() {}

#[test]
fn subdivision_stops_when_zoom_does_not_increase() {}

#[test]
fn subdivision_stops_after_two_low_yield_levels() {}
```

- [x] **Step 2: Verify RED**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml grid -- --nocapture
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml validation -- --nocapture
```

Expected: FAIL against the current unbounded planner.

- [x] **Step 3: Introduce bounded geometry types**

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BoundingBox {
    pub lat_min: f64,
    pub lat_max: f64,
    pub lng_min: f64,
    pub lng_max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridCell {
    pub bbox: BoundingBox,
    pub center_lat: f64,
    pub center_lng: f64,
    pub depth: u8,
    pub parent_unit_id: Option<String>,
    pub consecutive_low_yield_levels: u8,
}

pub struct GridLimits {
    pub max_depth: u8,
    pub min_cell_size_m: f64,
    pub max_initial_cells: usize,
    pub max_total_cells: usize,
}
```

- [x] **Step 4: Implement preflight without allocation**

Expose:

```rust
pub fn validate_and_estimate_grid(
    bbox: BoundingBox,
    step_km: f64,
    limits: GridLimits,
) -> Result<GridEstimate, ScrapeError>;
```

`GridEstimate` includes `initial_cells`, `recommended_min_step_km`, and area dimensions.

- [x] **Step 5: Implement clipped generation and subdivision decision**

Replace `search_grid` traversal responsibilities with pure functions:

```rust
generate_initial_cells
subdivide_cell
radius_to_zoom
should_subdivide
```

The job runner, not `GridScraper`, owns persistence and traversal.

- [x] **Step 6: Verify GREEN**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml grid -- --nocapture
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml validation -- --nocapture
```

Expected: all validation, clipping, cap, zoom, and low-yield tests pass without browser startup.

- [x] **Step 7: Record checkpoint**

Update `REQ-009` and `REQ-010`.

---

### Task 5: CloakBrowser Sidecar Protocol and Runtime Download

**Files:**
- Create: `sidecars/cloak-runtime/src/protocol.ts`
- Create: `sidecars/cloak-runtime/src/runtime.ts`
- Create: `sidecars/cloak-runtime/src/googleMaps.ts`
- Create: `sidecars/cloak-runtime/src/index.ts`
- Create: `sidecars/cloak-runtime/src/index.test.ts`
- Create: `scripts/build-cloak-runtime.ts`
- Create: `src-tauri/binaries/.gitkeep`
- Modify: `package.json`
- Modify: `bun.lock`

- [x] **Step 1: Add exact sidecar dependencies with Bun**

Run:

```bash
bun add --exact cloakbrowser@0.5.1 playwright-core@1.57.0 chromium-bidi@11.0.0
```

Add scripts:

```json
{
  "build:cloak-runtime": "bun run scripts/build-cloak-runtime.ts",
  "test:cloak-runtime": "bun test sidecars/cloak-runtime/src"
}
```

- [x] **Step 2: Write failing protocol tests**

```ts
test('returns a correlated typed error', async () => {
  const response = await dispatch(
    { id: '42', command: 'navigate', payload: { url: 'https://example.test' } },
    failingRuntime('navigation_timeout')
  );

  expect(response).toEqual({
    id: '42',
    ok: false,
    error: {
      kind: 'navigation_timeout',
      message: 'navigation timed out',
      retryable: true
    }
  });
});
```

- [x] **Step 3: Verify RED**

```bash
bun test sidecars/cloak-runtime/src/index.test.ts
```

Expected: FAIL because protocol and dispatcher are absent.

- [x] **Step 4: Implement protocol and runtime adapter**

Support:

```ts
type RuntimeCommand =
  | 'runtime_status'
  | 'ensure_runtime'
  | 'launch'
  | 'navigate'
  | 'evaluate'
  | 'screenshot'
  | 'health_check'
  | 'shutdown';
```

Use official `ensureBinary`, `binaryInfo`, and `launchPersistentContext`. Set cache/version/update variables before importing the runtime adapter. Emit only JSON on stdout; send diagnostic logs to stderr.

- [x] **Step 5: Implement block detection**

`health_check` returns typed page state:

```ts
type GoogleMapsPageState =
  | 'ready'
  | 'empty_results'
  | 'captcha'
  | 'consent_required'
  | 'rate_limited'
  | 'selector_changed';
```

- [x] **Step 6: Implement target-aware Bun compile script**

The script obtains the target triple from `TAURI_ENV_TARGET_TRIPLE` or `rustc --print host-tuple`, compiles the executable, and writes the exact Tauri sidecar filename under `src-tauri/binaries`.

- [x] **Step 7: Verify GREEN without real runtime download**

```bash
bun test sidecars/cloak-runtime/src/index.test.ts
bun run build:cloak-runtime
```

Expected: protocol tests pass and the host-target sidecar binary exists.

- [x] **Step 8: Record checkpoint**

Update `REQ-006` and `REQ-007`.

---

### Task 6: Rust Browser Supervisor and Typed Automation Client

**Files:**
- Modify: `src-tauri/src/scrapers/browser.rs`
- Modify: `src-tauri/src/scrapers/browser_test.rs`
- Modify: `src-tauri/src/scrapers/mod.rs`
- Modify: `src-tauri/src/google_maps_jobs/types.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/capabilities/default.json`

- [x] **Step 1: Write failing client/lifecycle tests**

Cover:

```rust
#[tokio::test]
async fn maps_correlated_sidecar_error_to_scrape_error_kind() {}

#[tokio::test]
async fn shutdown_times_out_then_terminates_sidecar() {}

#[tokio::test]
async fn only_one_in_flight_request_owns_a_correlation_id() {}

#[tokio::test]
async fn closed_stdout_fails_all_pending_requests_as_disconnected() {}
```

- [x] **Step 2: Verify RED**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml scrapers::browser_test -- --nocapture
```

Expected: FAIL because the current manager directly spawns Chromium.

- [x] **Step 3: Define the object-safe automation trait**

```rust
#[async_trait::async_trait]
pub trait BrowserAutomation: Send + Sync {
    async fn runtime_status(&self) -> Result<RuntimeStatus, ScrapeError>;
    async fn ensure_runtime(&self) -> Result<RuntimeStatus, ScrapeError>;
    async fn launch(&self, request: LaunchRequest) -> Result<(), ScrapeError>;
    async fn navigate(&self, url: &str, timeout: Duration) -> Result<(), ScrapeError>;
    async fn evaluate(&self, script: &str, timeout: Duration) -> Result<serde_json::Value, ScrapeError>;
    async fn screenshot(&self, path: &Path) -> Result<(), ScrapeError>;
    async fn health_check(&self) -> Result<GoogleMapsPageState, ScrapeError>;
    async fn shutdown(&self) -> Result<(), ScrapeError>;
}
```

- [x] **Step 4: Implement `BrowserSupervisor`**

Responsibilities:

- spawn the registered sidecar;
- own stdin/stdout tasks;
- correlate responses;
- fail pending calls on sidecar exit;
- enforce request timeouts;
- perform graceful then forced shutdown;
- never block Tokio with `std::process::Child::wait`.

- [x] **Step 5: Register sidecar and least-privilege capability**

Add `tauri-plugin-shell`, register `cloak-runtime` in `bundle.externalBin`, and permit only that sidecar command.

- [x] **Step 6: Verify GREEN**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml scrapers::browser_test -- --nocapture
```

Expected: all fake-sidecar protocol/lifecycle tests pass.

- [x] **Step 7: Record checkpoint**

Update `REQ-005`, `REQ-006`, `REQ-007`, and `REQ-018`.

---

### Task 7: Typed Google Maps Search and Place Extraction

**Files:**
- Modify: `src-tauri/src/scrapers/google_maps.rs`
- Test: `src-tauri/src/scrapers/google_maps.rs`

- [x] **Step 1: Write failing browser fault tests**

Create a `FakeBrowserAutomation` script queue and cover:

```rust
disconnect_during_initial_navigation_returns_disconnected
disconnect_during_load_probe_is_not_converted_to_false
disconnect_during_scroll_is_not_converted_to_null
disconnect_during_place_navigation_stops_detail_traversal
selector_change_returns_selector_changed
legitimate_empty_page_returns_reached_end_with_zero_listings
outside_bbox_result_is_retained_but_not_qualified
missing_coordinate_result_requires_deterministic_text_location_match
```

- [x] **Step 2: Verify RED**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml scrapers::google_maps::tests -- --nocapture
```

Expected: the disconnect tests fail against current `unwrap_or`/continue behavior.

- [x] **Step 3: Replace `BusinessInfo` search output**

Implement:

```rust
pub struct SearchPageResult {
    pub raw_listing_count: usize,
    pub reached_end: bool,
    pub blocked_reason: Option<ScrapeErrorKind>,
    pub places: Vec<PlaceCandidate>,
}

pub struct PlaceCandidate {
    pub place_id: Option<String>,
    pub canonical_url: String,
    pub name: String,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub website: Option<String>,
    pub facebook: Option<String>,
    pub zalo: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}
```

- [x] **Step 4: Implement deterministic search phases**

```text
navigate
health check
wait for ready/empty/block state
bounded feed scroll
collect canonical place references
visit each unprocessed reference once
extract typed details
return raw count and reached_end
```

Every automation error propagates. Per-place retry belongs to the job runner, not silent `continue` logic.

- [x] **Step 5: Verify GREEN**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml scrapers::google_maps::tests -- --nocapture
```

Expected: all happy-path, empty-result, selector, and disconnect tests pass.

- [x] **Step 6: Record checkpoint**

Update `REQ-005`, `REQ-011`, and `REQ-012`.

---

### Task 8: Persistent Job Runner, Retry, Pause, Resume, and Cancel

**Files:**
- Create: `src-tauri/src/google_maps_jobs/runner.rs`
- Create: `src-tauri/src/google_maps_jobs/events.rs`
- Modify: `src-tauri/src/google_maps_jobs/mod.rs`
- Test: `src-tauri/src/google_maps_jobs/runner.rs`

- [x] **Step 1: Write failing runner tests**

Cover:

```rust
only_one_job_can_run
fatal_grid_error_does_not_claim_next_cell
fatal_deep_scan_error_does_not_claim_next_district
retryable_error_retries_three_times_with_backoff
cancel_prevents_later_completion_event
pause_commits_state_before_browser_shutdown
resume_claims_exact_pending_unit
application_restart_marks_running_job_interrupted
stale_event_sequence_is_never_reused
```

- [x] **Step 2: Verify RED**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml google_maps_jobs::runner -- --nocapture
```

Expected: FAIL because no durable runner exists.

- [x] **Step 3: Implement one-worker ownership**

```rust
pub struct ActiveJob {
    pub job_id: String,
    pub cancellation: tokio_util::sync::CancellationToken,
    pub handle: tokio::task::JoinHandle<()>,
}

pub struct ScrapeJobRunner {
    active: tokio::sync::Mutex<Option<ActiveJob>>,
    repository: Arc<ScrapeJobRepository>,
    browser: Arc<dyn BrowserAutomation>,
    events: Arc<ScrapeEventPublisher>,
}
```

- [x] **Step 4: Implement retry and checkpoint loop**

For each claimed unit:

```text
check cancellation and persisted status
run unit
on success: atomic checkpoint and enqueue children
on retryable error: schedule retry with exponential backoff
on action-required error: commit pause and launch headed profile
on fatal error: commit failure and stop claiming units
```

- [x] **Step 5: Implement terminal-state lock**

Terminal transition and event sequence increment occur in one transaction. Publisher emits only the committed state and sequence.

- [x] **Step 6: Verify GREEN**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml google_maps_jobs::runner -- --nocapture
```

Expected: all retry, traversal, ownership, event, and race tests pass under paused Tokio time where applicable.

- [x] **Step 7: Record checkpoint**

Update `REQ-003`, `REQ-004`, `REQ-016`, and `REQ-017`.

---

### Task 9: CAPTCHA and Same-Profile Resume

**Files:**
- Modify: `src-tauri/src/google_maps_jobs/runner.rs`
- Modify: `src-tauri/src/scrapers/browser.rs`
- Modify: `sidecars/cloak-runtime/src/runtime.ts`
- Test: `src-tauri/src/google_maps_jobs/runner.rs`
- Test: `sidecars/cloak-runtime/src/index.test.ts`

- [x] **Step 1: Write failing CAPTCHA handoff tests**

Assert:

```text
headless profile path == headed resume profile path
paused_requires_action committed before relaunch
user confirmation calls health_check
failed health_check remains paused
successful health_check resumes the same unit
cancel while waiting for action closes the profile and becomes cancelled
```

- [x] **Step 2: Verify RED**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml captcha -- --nocapture
bun test sidecars/cloak-runtime/src/index.test.ts --test-name-pattern captcha
```

Expected: FAIL because the action-required workflow is absent.

- [x] **Step 3: Implement profile-stable relaunch**

Persist `profile_dir` and `blocked_url` with the job. Sidecar `launch` accepts the same `userDataDir` with `headless: false`.

- [x] **Step 4: Implement confirmation command contract**

The resume command for `paused_requires_action` first performs `health_check`. It transitions to `running` only when the page reports `ready` or `empty_results`.

- [x] **Step 5: Verify GREEN**

Run the two commands from Step 2. Expected: all CAPTCHA tests pass.

- [x] **Step 6: Record checkpoint**

Update `REQ-008` and `REQ-017`.

---

### Task 10: Safe Email Enrichment

**Files:**
- Create: `src-tauri/src/google_maps_jobs/email.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/src/google_maps_jobs/email.rs`

- [x] **Step 1: Write failing SSRF and extraction tests**

Cover:

```rust
rejects_loopback_private_link_local_multicast_and_metadata_ips
rejects_private_redirect_after_public_start
pins_the_validated_dns_address_for_the_request
stays_on_same_registrable_domain
stops_after_five_html_pages
rejects_non_html_and_body_over_two_megabytes
respects_robots_disallow
extracts_first_mailto_before_visible_text
extracts_first_visible_email_when_no_mailto_exists
filters_example_and_malformed_addresses
```

- [x] **Step 2: Verify RED**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml google_maps_jobs::email -- --nocapture
```

Expected: FAIL because enrichment does not exist.

- [x] **Step 3: Implement `SafeHttpClient`**

```rust
pub struct SafeHttpClient {
    connect_timeout: Duration,
    request_timeout: Duration,
    max_redirects: usize,
    max_body_bytes: usize,
}

impl SafeHttpClient {
    pub async fn fetch_html(&self, url: Url) -> Result<SafeHtmlResponse, ScrapeError>;
}
```

Disable automatic redirects. For every request and redirect, resolve the hostname, reject disallowed destinations, and pin the chosen approved address for that request.

- [x] **Step 4: Implement deterministic page discovery and email extraction**

Homepage first, then relevant links in DOM order. Stop at five HTML pages and return the first valid email.

- [x] **Step 5: Implement retryable enrichment statuses**

Persist `pending`, `running`, `found`, `not_found`, `retry_wait`, and `failed` without repeating Google Maps scraping.

- [x] **Step 6: Verify GREEN**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml google_maps_jobs::email -- --nocapture
```

Expected: all SSRF, redirect, robots, limit, and email-order tests pass.

- [x] **Step 7: Record checkpoint**

Update `REQ-013`.

---

### Task 11: Idempotent Phone/Email Lead Import

**Files:**
- Create: `src-tauri/src/google_maps_jobs/importer.rs`
- Modify: `src-tauri/src/commands_test.rs`
- Test: `src-tauri/src/google_maps_jobs/importer.rs`

- [x] **Step 1: Write failing importer tests**

Cover:

```rust
creates_lead_for_mobile_without_email
creates_lead_for_email_without_mobile
rejects_landline_without_email
deduplicates_by_normalized_phone
deduplicates_by_case_insensitive_email
does_not_create_second_lead_when_phone_and_email_match_different_existing_rows
rolls_back_batch_when_insert_fails
replaying_import_is_idempotent
```

- [x] **Step 2: Verify RED**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml google_maps_jobs::importer -- --nocapture
```

Expected: FAIL because current import hardcodes `has_email = false` and swallows insert failures.

- [x] **Step 3: Implement normalization and qualification**

```rust
pub fn qualifies_for_lead(phone: Option<&str>, email: Option<&str>) -> bool {
    phone.is_some_and(is_vn_mobile) || email.is_some_and(is_valid_email)
}
```

- [x] **Step 4: Implement one transactional import batch**

Use prepared lookups for normalized phone and `lower(trim(email))`. Any SQL error returns `ScrapeErrorKind::Database`; no result is marked imported before commit.

- [x] **Step 5: Verify GREEN**

Run the command from Step 2. Expected: all qualification, dedup, rollback, and replay tests pass.

- [x] **Step 6: Record checkpoint**

Update `REQ-014` and `REQ-015`.

---

### Task 12: Tauri Commands, State Initialization, and Sequenced Events

**Files:**
- Create: `src-tauri/src/google_maps_jobs/commands.rs`
- Modify: `src-tauri/src/google_maps_jobs/events.rs`
- Modify: `src-tauri/src/google_maps_jobs/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands_test.rs`

- [x] **Step 1: Write failing command tests**

Test:

```text
create returns job_id and persisted queued job
invalid grid is rejected before runtime launch
second active job returns conflict
pause/resume/cancel enforce state transitions
resume interrupted job starts exact pending unit
delete rejects non-terminal job
event payload includes job_id and monotonic sequence
```

- [x] **Step 2: Verify RED**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml google_maps_scrape_job_command -- --nocapture
```

Expected: FAIL because job commands are not registered.

- [x] **Step 3: Implement commands**

Implement the approved command list with thin delegation to `GoogleMapsJobService`. Commands must not contain scraping loops or SQL.

- [ ] **Step 4: Initialize service in Tauri setup**

During setup:

```text
initialize DB
recover interrupted jobs
initialize structured logging
create repository
create browser supervisor
create job runner/service
manage shared service state
do not start browser
```

- [x] **Step 5: Register commands and event publisher**

Every emitted payload includes committed `job_id`, sequence, status, and stage.

- [x] **Step 6: Verify GREEN**

Run the command from Step 2. Expected: all command/state/event tests pass.

- [x] **Step 7: Record checkpoint**

Update `REQ-003`, `REQ-004`, `REQ-016`, and `REQ-017`.

---

### Task 13: Frontend Service and Job-Scoped State

**Files:**
- Create: `src/lib/services/googleMapsScraperService.ts`
- Create: `src/lib/services/__tests__/googleMapsScraperService.spec.ts`

- [x] **Step 1: Write failing service tests**

```ts
test('ignores events from another job', () => {
  const state = reduceJobEvent(activeJob('a'), eventFor('b', 10));
  expect(state.jobId).toBe('a');
});

test('ignores stale event sequences', () => {
  const state = reduceJobEvent(jobAtSequence('a', 20), eventFor('a', 19));
  expect(state.sequence).toBe(20);
});
```

- [x] **Step 2: Verify RED**

```bash
bun test src/lib/services/__tests__/googleMapsScraperService.spec.ts
```

Expected: FAIL because the service does not exist.

- [x] **Step 3: Implement typed DTOs and commands**

Export exact methods corresponding to backend commands and one `subscribeToGoogleMapsJobEvents` helper that returns a cleanup function.

- [x] **Step 4: Implement pure reducer**

The reducer rejects other job IDs, stale sequences, and events after a terminal state.

- [x] **Step 5: Verify GREEN**

Run the command from Step 2. Expected: command mapping and reducer tests pass.

- [x] **Step 6: Record checkpoint**

Update `REQ-016`.

---

### Task 14: Google Maps Scraper UI Migration

**Required Skills Before Editing:** `pixel-perfect`, `shadcn-svelte`

**Files:**
- Create: `src/lib/components/GoogleMapsRuntimePanel.svelte`
- Create: `src/lib/components/GoogleMapsJobPanel.svelte`
- Create: `src/lib/components/GoogleMapsJobHistory.svelte`
- Modify: `src/lib/components/GoogleMapsScraper.svelte`
- Create: `src/lib/components/__tests__/GoogleMapsScraper.spec.ts`

- [x] **Step 1: Capture the current visual baseline**

Run the app and save desktop screenshots of keyword and grid modes before DOM changes. Record viewport and screenshot paths in the ledger.

- [x] **Step 2: Write failing component tests**

Cover:

```text
preserves preset/geocode/custom bbox controls
shows runtime download state
preflight displays estimated cells and recommended step errors
pause/resume/cancel call correct job commands
interrupted job exposes Resume
CAPTCHA state shows headed-browser instructions
partial is not rendered as completed
other-job and stale events do not update active UI
reload restores job from backend state
```

- [x] **Step 3: Verify RED**

```bash
bun test src/lib/components/__tests__/GoogleMapsScraper.spec.ts
```

Expected: FAIL because job panels and service integration are absent.

- [x] **Step 4: Build panels using existing shadcn-svelte primitives**

Preserve existing input layout. Add panels without introducing a second visual system. Use existing `Card`, `Progress`, `Badge`, `Button`, `Input`, and `Select` components.

- [x] **Step 5: Replace global boolean workflow with persisted job state**

`isScraping` becomes a derived state from active job status. Start returns `job_id`; stop becomes explicit Cancel. Add Pause and Resume.

- [x] **Step 6: Verify GREEN and visual drift**

```bash
bun test src/lib/components/__tests__/GoogleMapsScraper.spec.ts
bun run check
```

Run the app, capture matching keyword/grid screenshots, and use pixel measurements for changed elements. Expected: no unintended drift in existing controls; new panels meet keyboard and label accessibility requirements.

- [x] **Step 7: Record checkpoint**

Update `REQ-006`, `REQ-008`, `REQ-009`, `REQ-016`, and `REQ-017`.

---

### Task 15: Legacy Compatibility, Logging, Cleanup, and Retention

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/google_maps_jobs/commands.rs`
- Modify: `src-tauri/src/google_maps_jobs/repository.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`

- [x] **Step 1: Write failing compatibility and cleanup tests**

Cover:

```text
legacy_start_delegates_to_create_job
legacy_stop_delegates_to_cancel_active_job
legacy_wrapper_never_emits_duplicate_terminal_event
terminal_job_profile_cleanup_respects_seven_day_retention
captcha_profile_is_not_deleted
job_results_cleanup_respects_thirty_day_retention
logs_redact_license_cookie_and_local_storage_fields
```

- [x] **Step 2: Verify RED**

```bash
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml legacy_google_maps -- --nocapture
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml retention -- --nocapture
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml redaction -- --nocapture
```

Expected: FAIL because the old implementation still owns browser lifecycle and no retention service exists.

- [x] **Step 3: Replace old command bodies with wrappers**

Remove `run_google_maps_scrape_task` after its tests have migrated. `start_google_maps_scrape` maps the old payload to `CreateScrapeJobRequest`; `stop_google_maps_scrape` cancels the active job.

- [x] **Step 4: Add structured logging**

Initialize non-blocking rolling logs. Attach `job_id`, `unit_id`, stage, attempt, and error kind. Redact secrets and browser storage fields.

- [x] **Step 5: Add cleanup service**

Run cleanup at startup and after terminal jobs. Delete only records/profiles older than approved retention and never delete action-required profiles.

- [x] **Step 6: Verify GREEN**

Run the command from Step 2. Expected: all wrapper, terminal-event, retention, and redaction tests pass.

- [x] **Step 7: Record checkpoint**

Update `REQ-018` and `REQ-019`.

---

### Task 16: Integration, Runtime Smoke, Cross-Platform CI, and Release QA

**Files:**
- Create: `.github/workflows/google-maps-scraper.yml`
- Modify: `bunfig.toml` only if root-cause analysis proves test environment configuration is wrong
- Modify: formatter/clippy/test-harness files required for zero-failure gates
- Modify: `docs/superpowers/plans/2026-07-26-google-maps-scraper-production-ledger.md`

- [x] **Step 1: Add end-to-end mock integration tests**

Cover:

```text
keyword job to result checkpoint to Lead import
deep-scan disconnect stops later districts
grid saturation creates bounded child units
grid disconnect stops later cells
crash recovery resumes exact pending unit
database checkpoint failure does not advance progress
database import failure does not emit completion
email enrichment retry does not repeat Maps scraping
CAPTCHA pause and same-profile resume
```

- [x] **Step 2: Repair baseline QA blockers by root cause**

Use systematic debugging for:

- existing Rust formatter failures;
- all clippy `-D warnings` failures;
- Bun Svelte test environment resolving server-side `mount`;
- missing `runed` dependency resolution.

Limit behavior changes to the scraper work and mechanical QA fixes. Record pre-existing versus introduced failures in the ledger.

- [x] **Step 3: Add CI matrix**

Matrix:

```yaml
os: [ubuntu-latest, windows-latest, macos-13, macos-14]
```

Each job runs Bun install, sidecar tests/build, frontend check/tests/build, Rust format/clippy/tests, and target Tauri compile. Runtime smoke is separately gated by secrets/network availability and must not be reported as pass when skipped.

- [x] **Step 4: Run Linux real-runtime smoke test**

```bash
bun run build:cloak-runtime
RUN_CLOAK_BROWSER_SMOKE=1 bun test sidecars/cloak-runtime/src --test-name-pattern smoke
```

Expected: official runtime downloads or reuses cache, launches, loads a controlled page, health-check succeeds, and no sidecar/browser process remains after shutdown.

- [x] **Step 5: Run final Rust gates**

```bash
df -h . /tmp && cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
df -h . /tmp && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml
df -h . /tmp && cargo build --manifest-path src-tauri/Cargo.toml --release
```

Expected: zero errors, zero denied warnings, all tests pass.

- [x] **Step 6: Run final Bun/frontend gates**

```bash
bun install --frozen-lockfile
bun run test
bun run check
bun run build:cloak-runtime
bun run build
```

Expected: all tests pass, Svelte diagnostics show zero errors/warnings, sidecar builds, frontend production build succeeds.

- [ ] **Step 7: Run runtime/browser validation**

Launch the Tauri app and verify with the real backend/database:

```text
runtime first-download/retry
keyword happy path
grid happy path
pause/resume
cancel
forced browser termination
app restart/interrupted resume
CAPTCHA action-required simulation
email enrichment
Lead dedup
no leftover processes
no console/network errors
```

Linux real-runtime QA passes first-download, keyword, grid, pause/resume, cancel, forced browser termination, crash recovery, import/dedup, and process cleanup. A real CAPTCHA was not intentionally triggered, and Windows/macOS real-runtime QA remains unavailable; keep this step open.

- [x] **Step 8: Run final repository checks**

```bash
git diff --check
git status --short
bunx gitnexus analyze
```

Then run `gitnexus_detect_changes({scope: "all"})`, review every affected flow, and record exact risk.

Final GitNexus analysis indexed 1,901 nodes, 4,873 edges, 71 clusters, and 156 flows. Repeated `detect_changes(scope=staged)` runs report CRITICAL risk because the complete staged change set modifies 607 symbols and affects 124–147 symbols across 69 files, including DB startup, browser lifecycle, and pre-existing CRM/Zalo changes. The startup/DB, browser launch-shutdown, durable runner/checkpoint, and existing CRM/Zalo test flows were reviewed; full Rust/frontend and Linux runtime QA cover their d=1 behavior.

- [ ] **Step 9: Complete requirement coverage**

Every `REQ-001` through `REQ-020` must have implementation files, fresh tests, and PASS evidence. Unavailable Windows/macOS real-runtime evidence remains an explicit blocker or user-approved waiver; build-only evidence cannot be described as runtime verification.

- [x] **Step 10: Record final status**

Allowed statuses:

- `VERIFIED READY` only when all mandatory release evidence passes.
- `CONDITIONALLY READY` only with explicit non-blocking waivers.
- `NOT READY` if any required gate, runtime platform, or high-risk impact remains unresolved.

Recorded status: `NOT READY`. Linux implementation and runtime QA pass, but Step 9 remains open because `REQ-006`, `REQ-007`, `REQ-008`, and `REQ-020` still lack Windows/macOS real-runtime and real CAPTCHA evidence.

---

## Implementation Order and Checkpoints

Execute Tasks 1–16 in order. Do not start browser/runtime work before schema, repository, and pure grid validation tests pass. Do not migrate the UI before command/event contracts are green. Do not remove the legacy implementation before job integration tests pass. After any failed gate, fix the first failing layer, rerun the narrow check, then rerun every invalidated broader gate.
