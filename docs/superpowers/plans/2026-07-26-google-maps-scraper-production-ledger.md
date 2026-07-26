# Google Maps Scraper Production Execution Ledger

**Date:** 2026-07-26
**Spec:** `docs/superpowers/specs/2026-07-26-google-maps-scraper-production-design.md`
**Plan:** `docs/superpowers/plans/2026-07-26-google-maps-scraper-production.md`
**Current Status:** TASK 15 COMPLETE; TASK 16 IN PROGRESS — Linux production runtime paths and release gates have passed, but Windows/macOS real-runtime and real CAPTCHA evidence are still missing. Final status remains **NOT READY**.

**Execution Mode:** Inline on `master`, explicitly selected by the user. No commits will be created unless requested.

**Pre-existing Change Boundary:** The worktree was already dirty before production implementation. Existing changes in `commands.rs`, `commands_test.rs`, `db_test.rs`, `grid.rs`, `GoogleMapsScraper.svelte`, outreach/CRM/UI files, repository instructions, `package.json`, and `bun.lock` must be preserved and reviewed rather than overwritten.

## Requirements

| ID | Requirement | Owner Task | Implementation Evidence | QA Evidence | Status |
|---|---|---:|---|---|---|
| REQ-001 | Persistent jobs | 2-3 | Durable schema plus async repository create/get/list/unit/result/delete operations | Task 2/3 tests; serial full Rust 54/54 | COMPLETE |
| REQ-002 | Atomic checkpoints | 2-3 | Transactional result/child/unit/job checkpoint, rollback on invalid unit state, batch unit rollback, optimistic versioning | Repository 12/12; rollback and counter tests | COMPLETE |
| REQ-003 | Startup interruption recovery | 3, 8, 12, 16 | Repository atomically marks running jobs interrupted and resets running units pending; Tauri setup initializes the singleton repository/runner/service and runs startup recovery without launching the browser | Automated restart tests plus Linux SIGKILL QA: sidecar/Chromium exited, restart converted `running` to `interrupted`, UI restored Resume/Cancel, resumed job then cancelled cleanly | COMPLETE |
| REQ-004 | One active worker | 8, 12 | Singleton managed runner owns one `ActiveJob`, rejects a conflicting second start, and executes units sequentially | Concurrent-start and service conflict tests; full Rust 124/124 parallel plus serial | COMPLETE |
| REQ-005 | Typed browser errors | 6-8, 12, 16 | Object-safe automation and production processor propagate typed search, grid, enrichment, import, and terminal-state failures without converting them to empty success; sidecar error serialization removes terminal escape sequences | Automated browser/Google Maps/processor tests; Linux forced Chromium termination retried exactly three times and stopped later cells; ANSI regression 7/7 with 1 intentional smoke skip | COMPLETE |
| REQ-006 | Official runtime download/cache | 5-6, 14, 16 | Sidecar pins CloakBrowser wrapper `0.5.1` and Chromium `146.0.7680.177.5`; Rust manager uses only the bundled sidecar; UI exposes direct first-use official download, retry, verification, and installed-version state | Linux clean-cache smoke downloaded about 206 MB from the official channel, verified Ed25519 and SHA-256, launched, reused cache, and cleaned up; Windows/macOS real-runtime evidence remains missing | IN PROGRESS |
| REQ-007 | Cross-platform sidecar builds | 5-6, 16 | Target-aware Bun compile plus Rust target-path resolution cover Linux x64/ARM64, Windows x64, and macOS x64/ARM64; Tauri registers `binaries/cloak-runtime` as `externalBin`; CI matrix covers Ubuntu, Windows, Intel macOS, and Apple Silicon macOS | Linux x64 standalone/runtime QA passes and target-aware CI commands are defined; Windows/macOS real-runtime QA remains missing | IN PROGRESS |
| REQ-008 | Persistent CAPTCHA profile | 9, 14, 16 | Jobs persist `profile_dir` and `blocked_url`; action-required detection relaunches the same profile headed; UI restores `paused_requires_action`, explains manual CAPTCHA completion, and resumes the exact job | Automated runner/browser/component same-profile tests pass; Linux pause/resume reused the exact `profile_dir`; a real CAPTCHA was not intentionally triggered | IN PROGRESS |
| REQ-009 | Backend grid validation | 4, 14, 16 | Finite/range/order/step validation, no-allocation estimate, 10,000 initial-cell cap, backend recommendation, and matching UI preflight estimate/warning | Automated validation/component tests plus Linux BBox `[10.775,10.7755,106.699,106.6995]` produced exactly one cell and completed `1/1` | COMPLETE |
| REQ-010 | Bounded adaptive subdivision | 4, 8, 12, 16 | Production processor checkpoints clipped child cells and enforces depth/min-size/zoom/total-cell/low-yield policy through the durable runner | Grid/runner/processor tests plus forced termination job `gmaps-1785048270761-296db70c275dde93`: first unit retried, eight later units were never claimed before recovery/cancel | COMPLETE |
| REQ-011 | Raw listing saturation | 7-8, 12, 16 | Production processor uses independent `raw_listing_count`, `reached_end`, blocked reason, and deduplicated details to decide subdivision and terminal state | Automated typed scraper tests plus real keyword job discovered 15 listings without false empty/selector completion | COMPLETE |
| REQ-012 | Spatial filtering and dedup | 7-8, 12, 16 | Processor persists candidates, applies inclusive BBox/exact-location qualification, and preserves place ID, canonical URL, phone, then name/address dedup precedence; keyword/deep-scan jobs without BBox are explicitly inside scope | Spatial/processor regressions plus Linux keyword and one-cell grid runtime QA | COMPLETE |
| REQ-013 | Safe email enrichment | 10 | `SafeHttpClient` allows only credential-free HTTP(S), disables proxies and automatic redirects, resolves and validates every hop, rejects non-public destinations, pins the approved address, stays on one registrable domain, enforces three redirects/2 MB/5s connect/15s request; deterministic robots-aware crawl visits at most five HTML pages and selects mailto before visible email; repository atomically claims and checkpoints `running/found/not_found/retry_wait/failed` without repeating Maps scraping | Email security/extraction 14/14 including real reqwest pinned-address integration; repository enrichment checkpoint 1/1; full Rust 102/102 parallel and serial | COMPLETE |
| REQ-014 | Phone/email Lead qualification | 11, 16 | Importer independently normalizes Vietnamese mobile formats (`03/05/07/08/09`, local/`84`/`0084`) and normalized email; landline-only results are rejected | Automated importer tests plus real `Dien Lanh HK` job: 15 qualified results, 10 Leads created, 5 contact duplicates | COMPLETE |
| REQ-015 | Idempotent Lead import | 2, 11, 16 | `BEGIN IMMEDIATE` batch revalidates contacts, deduplicates normalized phone and case-insensitive trimmed email across existing Leads and the current batch, writes Lead/result status/job counters atomically, and rolls back on any SQL/read/insert failure | Automated replay/rollback tests plus SQLite evidence for job `gmaps-1785047852973-54455993d3e8a22f`: `imported=10`, `duplicate=5`, total Leads `10` | COMPLETE |
| REQ-016 | Job-scoped sequenced events | 8, 12-14 | Publisher emits committed snapshots with `job_id` and monotonic sequence; typed frontend reducer and component reject other-job, stale, and post-terminal events | Rust event/race tests, service 5/5, component 9/9, and full Rust 124/124 | COMPLETE |
| REQ-017 | Pause/resume/cancel race safety | 8-9, 12-16 | Backend commits pause/action/cancel transitions before cleanup; UI maps Pause, Resume, Cancel, retry, interrupted recovery, and CAPTCHA confirmation to the active persisted job and resets state from command responses | Automated race/profile/UI tests; Linux pause reached `paused` in 237 ms, resume reused the same profile, cancel completed in about 90 ms, and no scraper-owned process remained | COMPLETE |
| REQ-018 | Structured diagnostics and cleanup | 6, 15-16 | Sidecar diagnostics stay on stderr, stdout is typed JSON, rolling logs redact secrets, diagnostics are job/unit scoped, retention preserves action-required profiles, shutdown is graceful then forced with async wait/reap, and protocol errors strip ANSI controls | Task 15 redaction/retention/wrapper tests; forced termination/crash recovery/clean-cache process checks; ANSI regression test passes | COMPLETE |
| REQ-019 | Compatibility wrappers | 15 | Legacy start/stop commands delegate to durable create/resume/cancel flows and cannot emit duplicate terminal events | Task 15 compatibility tests and full Rust release suite | COMPLETE |
| REQ-020 | Full release QA | 16 | Cross-platform CI and complete local Linux release matrix are implemented | Linux release gates and real-runtime QA pass; Windows/macOS real-runtime and real CAPTCHA evidence remain mandatory blockers | IN PROGRESS |

## GitNexus Impact Record

| Symbol | Risk | Direct Callers | Affected Processes | Reviewed | Notes |
|---|---|---:|---:|---|---|
| `init_db` | CRITICAL | 28 | 15 | Yes | Warned user. d=1 includes `run` and DB/command tests across eight modules; schema changes must remain additive and idempotent. |
| `run` | LOW | 1 | 1 | Yes | d=1: `main`; preserve application initialization contract. |
| `BrowserState` | LOW | 0 | 0 | Yes | Struct edges are not linked by the current index; command state usages require manual review. |
| `BrowserManager` | LOW | 0 | 0 | Yes | Struct edges are not linked by the current index; method impacts below are authoritative. |
| `start` | CRITICAL | 8 | 6 | Yes | Warned user. d=1: start command plus browser, grid, scraper, and command tests. |
| `acquire` | CRITICAL | 7 | 6 | Yes | Warned user. d=1: start command plus browser, grid, scraper, and command tests. |
| `close` | CRITICAL | 7 | 5 | Yes | Warned user. d=1 includes start/stop commands and browser/grid/scraper tests. |
| `navigate` | HIGH | 2 | 3 | Yes | Warned user. d=1: `scrape_url`, browser lifecycle test. |
| `evaluate` | HIGH | 3 | 3 | Yes | Warned user. d=1: `scrape_url`, `extract_current_place`, browser lifecycle test. |
| `GoogleMapsScraper` | LOW | 0 | 0 | Yes | Struct edges are not linked by the current index; method impacts below are authoritative. |
| `scrape_url` | CRITICAL | 2 | 5 | Yes | Warned user. d=1: `search`, `search_region`; affects runner and grid/scraper/command tests. |
| `extract_current_place` | LOW | 1 | 2 | Yes | d=1: `scrape_url`; affects runner and scraper test flows transitively. |
| `generate_grid` | HIGH | 2 | 3 | Yes | Warned user. d=1: runner and grid generation test. |
| `radius_to_zoom` | HIGH | 1 | 4 | Yes | Warned user. d=1: `search_grid`; four runner/test processes. |
| `search_grid` | HIGH | 2 | 4 | Yes | Warned user. d=1: runner and adaptive subdivision test. |
| `run_google_maps_scrape_task` | LOW | 3 | 2 | Yes | d=1: start command and two command tests. |
| `start_google_maps_scrape` | LOW | 0 | 0 | Yes | Tauri command registration/caller edges are not represented by the current index. |
| `stop_google_maps_scrape` | LOW | 0 | 0 | Yes | Tauri command registration/caller edges are not represented by the current index. |
| `startScrape` | NOT INDEXED | 1 manual | 0 indexed | Yes | Frontend Svelte symbol remains absent from the index. Manual d=1 is the Start button; it now prepares CloakBrowser, creates a queued durable job, then resumes that exact job. |
| `GoogleMapsScraper.svelte` | NOT INDEXED | UI route manual boundary | 0 indexed | Yes | GitNexus resolved `GoogleMapsScraper` to the Rust struct, not the Svelte component. Service imports, event subscription, lifecycle actions, and preserved controls were reviewed manually and through 9 component tests. |
| `classifyGoogleMapsPageState` | LOW | 1 | 0 | Yes | Updated empty-result recognition for the real `Google Maps can't find ...` text; direct caller is the sidecar page inspection path. |
| `scrape_url_typed` | LOW | 3 | 1 | Yes | Final URL from health snapshot now controls search-to-place redirects; callers and typed search flow were regression-tested. |
| `wait_for_page_state` | LOW | 1 | 1 | Yes | Empty/ready/block state wait logic reviewed with search and place redirect tests. |
| `Step` | LOW | 0 | 0 | Yes | Test-only automation step fixture updated for final-URL health snapshots. |
| `candidate_to_result` | LOW | 2 | 1 | Yes | Keyword/deep-scan jobs without BBox now persist `inside_requested_area=true`; importer flow regression-tested. |
| `runtimeError` | LOW | 2 | 0 | Yes | Protocol serialization strips CSI/OSC terminal controls from typed and unknown runtime errors. |
| `RuntimeFault` | HIGH | 12 | 3 | Yes | Warned user; constructor was deliberately not modified. Sanitization was placed at the LOW-risk `runtimeError` boundary instead. |

## Initial Baseline Evidence

This table is historical Task 1 evidence and is superseded by the final release evidence recorded after Task 16.

| Gate | Local Time | Working Directory | Exact Command | Exit | Pass/Fail Counts | Warnings/Skips | Evidence | Result |
|---|---|---|---|---:|---|---|---|---|
| Diff whitespace | 2026-07-26 07:30:26 +07 | repo root | `git diff --check` | 2 | existing trailing-whitespace findings | Existing user changes in command/UI files | `/tmp/rustsale-git-diff-check.txt` | FAIL |
| Rust tests | 2026-07-26 07:30:26 +07 | repo root | `df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml` | 0 | 36 pass, 0 fail | Chromium shared-memory/kill diagnostics | `/tmp/rustsale-cargo-test.txt` | PASS |
| Rust format | 2026-07-26 07:30:26 +07 | repo root | `df -h . /tmp && cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | 1 | formatting diffs | Existing multi-file/vendor drift | `/tmp/rustsale-cargo-fmt.txt` | FAIL |
| Rust clippy | 2026-07-26 07:30:26 +07 | repo root | `df -h . /tmp && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | 101 | 28 error diagnostics, 1 warning summary | Existing warnings denied | `/tmp/rustsale-clippy.txt` | FAIL |
| Svelte diagnostics | 2026-07-26 07:30:26 +07 | repo root | `bun run check` | 0 | 0 errors, 0 warnings | None | `/tmp/rustsale-svelte-check.txt` | PASS |
| Frontend tests | 2026-07-26 07:30:26 +07 | repo root | `bun test` | 1 | 0 pass, 11 fail, 1 error | Svelte server-side `mount(...)`; existing harness failure | `/tmp/rustsale-bun-test.txt` | FAIL |
| Frontend build | 2026-07-26 07:30:26 +07 | repo root | `bun run build` | 0 | client and server builds pass | Vite chunk larger than 500 kB | `/tmp/rustsale-build.txt` | PASS |

## Final Release Evidence

All commands below were run after the ANSI error-message hardening edit. Cargo commands include the required immediately preceding disk-space check; available space was 84 GB on the workspace filesystem and 9.2 GB on `/tmp`.

| Gate | Local Time | Working Directory | Exact Command | Exit | Pass/Fail Counts | Warnings/Skips | Evidence | Result |
|---|---|---|---|---:|---|---|---|---|
| Dependency lock | 2026-07-26 14:06:06 +07 | repo root | `bun install --frozen-lockfile` | 0 | 176 installs checked across 258 packages; no changes | None | `/tmp/rustsale-final-qa/bun-install.txt` | PASS |
| Bun/frontend tests | 2026-07-26 14:06:06 +07 | repo root | `bun run test` | 0 | 38 pass, 1 skip, 0 fail; 122 assertions | Intentional real-runtime smoke skip; existing Svelte `derived_inert` test diagnostics | `/tmp/rustsale-final-qa/bun-test.txt` | PASS |
| Svelte diagnostics | 2026-07-26 14:06:14 +07 | repo root | `bun run check` | 0 | 0 errors, 0 warnings | None | `/tmp/rustsale-final-qa/bun-check.txt` | PASS |
| CloakBrowser sidecar build | 2026-07-26 14:06:22 +07 | repo root | `bun run build:cloak-runtime` | 0 | Linux x64 standalone binary built | Generated binary removed after release QA; `.gitkeep` retained | `/tmp/rustsale-final-qa/cloak-build.txt` | PASS |
| Official runtime clean-cache smoke | 2026-07-26 14:06:22 +07 | repo root | `RUN_CLOAK_BROWSER_SMOKE=1 bun test sidecars/cloak-runtime/src --test-name-pattern smoke` | 0 | 1 pass, 7 filtered, 0 fail | 206 MB clean download; Linux font-set advisory | `/tmp/rustsale-final-qa/cloak-smoke.txt` | PASS |
| Runtime authenticity | 2026-07-26 14:06:22 +07 | repo root | same smoke command | 0 | Ed25519 signature and SHA-256 checksum verified; Chromium `146.0.7680.177.5` launched and cleaned | None | `/tmp/rustsale-final-qa/cloak-smoke.txt` | PASS |
| Frontend production build | 2026-07-26 14:06:30 +07 | repo root | `bun run build` | 0 | client/server/static adapter builds pass | Main client chunk `1,879.64 kB` before gzip | `/tmp/rustsale-final-qa/frontend-build.txt` | PASS WITH WARNING |
| Rust format | 2026-07-26 14:07:09 +07 | repo root | `df -h . /tmp && cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | 0 | no formatting diff | None | `/tmp/rustsale-final-qa/cargo-fmt.txt` | PASS |
| Rust Clippy | 2026-07-26 14:07:10 +07 | repo root | `df -h . /tmp && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | 0 | zero denied warnings | None | `/tmp/rustsale-final-qa/cargo-clippy.txt` | PASS |
| Rust tests, parallel | 2026-07-26 14:07:23 +07 | repo root | `df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml` | 0 | 142 pass, 0 fail | No race failure | `/tmp/rustsale-final-qa/cargo-test-parallel.txt` | PASS |
| Rust tests, serial | 2026-07-26 14:07:36 +07 | repo root | `df -h . /tmp && cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | 0 | 142 pass, 0 fail | None | `/tmp/rustsale-final-qa/cargo-test-serial.txt` | PASS |
| Rust release build | 2026-07-26 14:07:57 +07 | repo root | `df -h . /tmp && cargo build --manifest-path src-tauri/Cargo.toml --release` | 0 | optimized binary built | None | `/tmp/rustsale-final-qa/cargo-release-build.txt` | PASS |
| Repository whitespace | 2026-07-26 14:11:02 +07 | repo root | `git diff --check` plus trailing-whitespace scan of untracked task files | 0 | no whitespace errors | Worktree remains intentionally dirty | `/tmp/rustsale-final-qa/git-precheck.txt` | PASS |
| GitNexus reindex | 2026-07-26 14:11:02 +07 | repo root | `bunx gitnexus analyze --force` | 0 | 1,901 nodes, 4,873 edges, 71 clusters, 156 flows | 1 generated/vendor-sized file skipped; embeddings remain 0 | `/tmp/rustsale-final-qa/gitnexus-analyze.txt` | PASS |
| GitNexus change impact | 2026-07-26 pre-commit | repo root | repeated `gitnexus_detect_changes({scope: "staged"})` | 0 | 607 changed symbols, 124–147 affected symbols, 69 files | Risk `CRITICAL`; reviewed startup/DB, browser lifecycle, runner/checkpoint, and existing CRM/Zalo flows | MCP evidence in final session | REVIEWED CRITICAL |

## Linux Real-Runtime Evidence

QA root: `/tmp/rustsale-linux-qa-fixed-FDbDBp`. The Tauri app and scraper-owned browser processes were stopped after validation.

| Scenario | Evidence | Result |
|---|---|---|
| Empty result | Real `Google Maps can't find ...` response completed with zero businesses in about two seconds instead of `selector_changed`; screenshots `keyword-empty-fixed-ready.png` and `keyword-empty-fixed-terminal.png` | PASS |
| Keyword happy path and import | Job `gmaps-1785047852973-54455993d3e8a22f` completed with 15 discovered, 15 qualified, 10 Leads created, and 5 duplicates; SQLite reports `imported=10`, `duplicate=5` | PASS |
| One-cell grid | BBox `[10.775,10.7755,106.699,106.6995]` produced one durable unit and completed `1/1`; job `gmaps-1785048135283-fbbef6d4eedbcc19` | PASS |
| Pause/resume/cancel | Pause committed in 237 ms, resume reused the exact `profile_dir`, cancel committed in about 90 ms; screenshots `pause-resume-paused.png`, `pause-resume-resumed.png`, and `pause-resume-cancelled.png` | PASS |
| Forced Chromium termination | Job `gmaps-1785048270761-296db70c275dde93` had nine cells; only the first cell was attempted/retried while the other eight stayed unclaimed until recovery/cancel, preventing the legacy grid-drain false completion | PASS |
| Crash recovery | SIGKILL during retry stopped the sidecar/Chromium; restart changed persisted `running` state to `interrupted`, restored Resume/Cancel, and resume/cancel cleanup succeeded; screenshots `interrupted-recovered-ui.png` and `interrupted-resumed-cancelled.png` | PASS |
| Official runtime first use | Clean isolated cache downloaded 206 MB, verified Ed25519 and SHA-256, launched Chromium `146.0.7680.177.5`, reused cache, and removed sidecar/browser processes | PASS |
| CAPTCHA | Same-profile action-required behavior is covered by automated tests; a real CAPTCHA was not intentionally triggered | BLOCKED |
| Windows/macOS | CI target builds are configured, but no real Windows or macOS runtime launch/download/cleanup evidence was executed locally | BLOCKED |

## Runtime Defects Found and Fixed

1. Real empty-result wording was not recognized. `classifyGoogleMapsPageState` now accepts `Google Maps can't find ...` and has a regression test.
2. A Google Maps search may redirect directly to `/maps/place/...`. `scrape_url_typed` now uses the final health-snapshot URL and extracts the place instead of applying search-page selectors.
3. Keyword/deep-scan jobs do not have a BBox. `candidate_to_result` now records `inside_requested_area=true` for this mode so the importer does not discard every result as spatially unknown.
4. Playwright may embed terminal formatting in error strings. `runtimeError` now strips CSI/OSC controls before emitting typed JSON; regression coverage includes color and hyperlink escape sequences.

## Task Checkpoints

| Task | RED Evidence | GREEN Evidence | Diff Reviewed | Requirements Updated | Status |
|---:|---|---|---|---|---|
| 1 | Baseline failures captured without suppression | Rust 36/36, Svelte check/build pass; expected pre-existing failures recorded | Yes | Yes | COMPLETE |
| 2 | Missing schema: `/tmp/rustsale-task2-schema-red.txt`; missing types: `/tmp/rustsale-task2-types-red.txt` | `google_maps_`: 9/9; full Rust: 42/42 in `/tmp/rustsale-task2-full-test.txt` | Yes; scoped `git diff --check` clean, placeholder scan clean | Yes | COMPLETE |
| 3 | Missing repository/types: `/tmp/rustsale-task3-red.txt`; retry/terminal invariant RED: `/tmp/rustsale-task3-invariants-red.txt` | Repository 12/12 in `/tmp/rustsale-task3-targeted-final.txt`; serial full Rust 54/54 in `/tmp/rustsale-task3-full-serial.txt` | Yes; scoped rustfmt/diff/placeholder checks clean | Yes | COMPLETE |
| 4 | Missing validation/grid APIs: `/tmp/rustsale-task4-grid-red.txt` and `/tmp/rustsale-task4-validation-red.txt`; legacy adapter runaway exposed during first GREEN | Validation 3/3, grid 5/5, full serial Rust 60/60 in Task 4 logs | Yes; scoped rustfmt/diff/placeholder checks clean; Antigravity fail-fast branch preserved | Yes | COMPLETE |
| 5 | Missing `googleMaps` module: `/tmp/rustsale-task5-red.txt` (0 pass, 1 fail, 1 error) | Protocol/block tests 4/4, scoped TypeScript pass, Linux x64 standalone build and status/shutdown protocol smoke in `/tmp/rustsale-task5-green.txt` | Yes; scoped diff whitespace and placeholder scans clean | Yes | COMPLETE |
| 6 | Missing supervisor/trait/async dependency in `/tmp/rustsale-task6-red.txt`; blank stdout and `about:blank` regressions reproduced in focused RED tests | Browser protocol/lifecycle 6/6; full Rust serial 63/63 in `/tmp/rustsale-task6-full-serial-final.txt`; parallel 63/63 in `/tmp/rustsale-task6-full-parallel.txt` | Yes; scoped diff/placeholder/system-Chrome scan clean; no orphan sidecar/browser process | Yes | COMPLETE |
| 7 | Missing typed search contract and fault propagation in `/tmp/rustsale-task7-red.txt` | Typed fault/spatial 8/8 plus real CloakBrowser mock 1/1 in `/tmp/rustsale-task7-google-maps-tests.txt`; full Rust 71/71 in `/tmp/rustsale-task7-full-parallel.txt` | Yes; scoped diff clean and no production error swallowing found | Yes | COMPLETE |
| 8 | Missing runner/event/processor APIs in `/tmp/rustsale-task8-red.txt` | Runner 9/9 in `/tmp/rustsale-task8-runner-final.txt`; repository 12/12; full Rust 80/80 in `/tmp/rustsale-task8-full-parallel.txt` | Yes; scoped diff clean, retry timestamp corruption fails closed, no task-owned Clippy diagnostics | Yes | COMPLETE |
| 9 | Missing profile columns, action processor contract, confirmation method, and launch policy produced the expected Rust/TypeScript RED failures | CAPTCHA runner/browser 8/8; migration/schema 2/2; sidecar 5/5; scoped TypeScript and sidecar build pass; full Rust 87/87 parallel and serial | Yes; scoped rustfmt/diff checks clean; launch path and SQL field mapping reviewed end-to-end | Yes | COMPLETE |
| 10 | Missing module/crates produced expected 49-error RED; missing repository claim/save methods produced expected method-not-found RED | Email SSRF/robots/extraction/status 14/14; repository persistence 1/1; real reqwest pinned-address integration; full Rust 102/102 parallel and serial | Yes; scoped rustfmt/diff checks clean; DNS→validation→pin→transport and outcome→repository SQL fields traced | Yes | COMPLETE |
| 11 | Missing importer APIs produced expected compile RED; same-batch contact-key and corrupt SQLite row regressions produced focused behavioral RED failures | Importer 12/12; full Rust 114/114 parallel in `/tmp/rustsale-task11-final-parallel.txt` and serial in `/tmp/rustsale-task11-final-serial.txt` | Yes; scoped rustfmt and `git diff --check` clean; transaction, dedup, status, and job-counter fields reviewed | Yes | COMPLETE |
| 12 | Missing command/service/processor wiring produced expected focused RED failures | Command/service 7/7, processor helpers 2/2, runtime prepare 1/1, full Rust 124/124 parallel and serial in `/tmp/rustsale-task12-full-parallel.txt` and `/tmp/rustsale-task12-full-serial.txt` | Yes; scoped rustfmt/diff checks clean and no Task 12 Clippy diagnostics | Yes | COMPLETE |
| 13 | Service import and command/reducer contract initially absent | Service 5/5; combined frontend contract 14/14; `bun run check` 0 errors and 0 warnings | Yes; scoped diff check clean | Yes | COMPLETE |
| 14 | Browser-condition RED: 1 characterization pass and 8 durable-job behavior failures | Component 9/9; combined service/component 14/14; `bun run check` 0 errors and 0 warnings | Yes; keyword/grid AFTER screenshots and exact computed-style measurements reviewed | Yes | COMPLETE |
| 15 | Compatibility, retention, redaction, and cleanup tests initially failed before wrappers/services existed | Legacy wrappers, rotating/redacted logging, seven-day profile retention, thirty-day result retention, and startup/post-terminal cleanup tests pass | Yes; wrapper lifecycle, terminal-event uniqueness, redaction, and profile deletion boundaries reviewed | Yes | COMPLETE |
| 16 | Runtime QA exposed three production blockers: real empty-result text misclassified, search redirect final URL ignored, and keyword results filtered because `inside_requested_area` was `NULL`; post-QA inspection also found ANSI controls in Playwright errors | Regressions pass; Linux keyword/grid/pause/resume/cancel/forced-kill/crash-recovery/clean-cache runtime QA passes; all local release gates pass | Yes; source diff, runtime SQLite state, screenshots, process cleanup, and generated sidecar cleanup reviewed | Yes | IN PROGRESS — Windows/macOS runtime and real CAPTCHA evidence missing |

## Waivers and Blockers

No waivers approved. Cross-platform runtime claims require fresh Windows and macOS runtime evidence; CI compilation alone is insufficient. Automated same-profile CAPTCHA tests do not replace a real action-required runtime exercise.

Task 2 note: repository-wide `cargo fmt --check` and Clippy remain failed by the pre-existing baseline. Scoped formatting for new `google_maps_jobs` files passes, and Task 2 Clippy produced no diagnostics in `db.rs`, `db_test.rs`, `lib.rs`, or `google_maps_jobs`; the command still exits 101 on 19 existing library errors in unrelated scraper/command code.

Task 3 note: the standard parallel full Rust run exposed the existing BrowserManager readiness race: 50 passed and 4 browser/scraper tests failed with CDP `Connection refused`. The failing browser test passes alone and the complete suite passes 54/54 with `--test-threads=1`. This is not waived; Task 6 owns the typed startup readiness fix. Task 3 Clippy still exits on the same 19 pre-existing library errors and reports no diagnostics in `google_maps_jobs`, `db.rs`, or `lib.rs`.

Task 4 note: pure validation/grid gates pass and the complete Rust suite passes 60/60 serially. Clippy now reports 17 existing library errors and no diagnostics in `validation.rs` or `grid.rs`; the previous grid approximate-constant diagnostic was removed by the bounded geometry implementation. Parallel browser-test readiness remains owned by Task 6.

Task 5 note: Bun standalone compilation initially failed because Playwright `1.57.0` references its upstream `chromium-bidi ^11.0.0` build dependency and optional Electron paths. The production build now pins `chromium-bidi 11.0.0`, bundles it, externalizes unused Electron-only paths, disables dotenv/bunfig autoloading, and hides the console for Windows targets. Repository-wide `bunx tsc --noEmit` still fails on pre-existing shadcn component export declarations; the scoped strict sidecar TypeScript command passes. The local official CloakBrowser runtime was already cached before the smoke test; Task 14 owns first-download UX and Task 16 owns clean-cache/OS runtime QA.

Task 6 note: GitNexus reports `navigate` and `evaluate` as HIGH risk with direct `scrape_url`/`extract_current_place` dependents; `start`/`acquire`/`close` method impact edges were incomplete, so their eight/seven manual callers were treated as high risk. The direct Chromium/CDP implementation, fixed port readiness race, system Chrome fallback, blocking child wait, and ignored kill/wait errors were removed. Clippy still exits on the same 17 pre-existing `commands.rs` diagnostics and reports no diagnostics in `browser.rs` or `browser_test.rs`.

Task 7 note: GitNexus reports `scrape_url` CRITICAL (9 impacted symbols, 5 flows) and `search`/`search_region` HIGH. Compatibility wrappers keep the existing `anyhow::Result<Vec<BusinessInfo>>` contract for command/grid callers while new runner-facing methods return typed `SearchPageResult`. CAPTCHA/consent/rate-limit states are explicit blocked results, selector drift is fatal, and transport/evaluation failures propagate. Clippy still exits on baseline `commands.rs` errors and reports no diagnostics in `google_maps.rs`.

Task 8 note: `runner.rs` and `events.rs` were new and absent from the stale GitNexus index; repository methods added to the previously unindexed Task 3 module were therefore reviewed through direct callers and tests. The runner never claims another unit after fatal grid/deep-scan errors, schedules exactly three exponential retries, checks cancellation and persisted status before checkpoints, commits pause/cancel/terminal state before cleanup, and only publishes committed monotonic sequences. Clippy returns to the 17 baseline `commands.rs` errors with no Task 8 diagnostics.

Task 9 note: GitNexus still does not index the uncommitted Task 2-9 job-engine symbols, and its current `init_db` impact result conflicts with the 28 manual/context callers recorded earlier; the schema/browser boundary was therefore treated as CRITICAL and verified with full parallel and serial Rust suites. `profile_dir` and `blocked_url` migrate idempotently from the previous schema, typed health snapshots retain state/URL/title, and headed relaunch reuses the exact profile path and blocked URL. Clippy still exits on the pre-existing `commands.rs`, `commands_test.rs`, and `grid.rs` baseline only; no Task 9 file appears in diagnostics. Runtime UI/action command wiring and profile retention remain Tasks 12-14, so the module is still NOT READY.

Task 10 note: enrichment uses manual redirect handling with system proxies disabled, validates every DNS answer and rejects the whole resolution set if any address is disallowed, then configures reqwest with the approved `SocketAddr` for that request. Tests cover same-domain private redirects, fourth-redirect rejection, credentials/non-HTTP rejection, robots disallow, content type/body caps, five-page traversal, email ordering/normalization, and a real local TCP server reached through reqwest hostname pinning. Clippy remains failed only by the pre-existing `commands.rs`, `commands_test.rs`, and `grid.rs` diagnostics; no Task 10 file appears. Runner/service integration remains Tasks 12-13, so the module is still NOT READY.

Task 11 note: the importer does not reuse the HIGH-risk legacy `commands.rs::normalize_phone_digits` contract. It owns a narrower Vietnamese-mobile validator, revalidates stored result fields, reserves every contact key for duplicate results to prevent a later row in the same batch from creating a second Lead, and fails closed when existing SQLite contact data cannot be decoded. `BEGIN IMMEDIATE` covers Lead inserts, result import statuses, `businesses_qualified`, `leads_created`, `event_sequence`, and `version`; trigger/read failures prove full rollback. Clippy still exits on 17 pre-existing `commands.rs` diagnostics and reports no `importer.rs` diagnostic. Command/runner integration remains Task 12, so the module is still NOT READY.

Task 12 note: durable commands, singleton service state, startup recovery, typed events, production processor, import, and runtime preparation are fully wired. The structured logging dependency was completed in Task 15, closing the previous checkpoint carry-over.

Task 13 note: the TypeScript service owns exact Tauri command names and arguments, maps persisted jobs to view state, subscribes with idempotent cleanup, and rejects other-job, stale, and post-terminal events.

Task 14 note: the UI preserves the existing preset/geocode/custom-BBox mechanism, exposes official runtime preparation and version state, estimates grid size before launch, restores durable jobs after reload, and maps pause/resume/cancel/retry/CAPTCHA actions to backend commands.

Task 15 note: legacy commands are compatibility wrappers over durable jobs; rolling diagnostics redact license/cookie/local-storage values; startup and post-terminal retention delete expired profiles/results while preserving CAPTCHA action-required profiles. All 10 Task 15 tests pass inside the 142-test Rust suite.

Task 16 note: all local Linux compile/test/build/runtime gates pass after the final ANSI hardening edit. CI defines Ubuntu, Windows x64, Intel macOS, and Apple Silicon macOS build jobs, but CI compilation cannot substitute for real runtime download/launch/cleanup on Windows/macOS. Real CAPTCHA remains unexecuted. Final release status is therefore NOT READY without waiver.

Final GitNexus note: repeated `detect_changes(scope=staged)` runs report CRITICAL because the complete staged change set spans 607 changed symbols and affects 124–147 symbols across 69 files. Representative traces reviewed include `Run → DiagnosticsGuard`, `Run → Open_db_connection`, `Launch → ScrapeJobError`, `Launch → RuntimeFault`, `Start_current → Load_unit`, pause/cancel/CAPTCHA race tests, grid/search shutdown tests, and existing CRM/Zalo DB flows. The risk is not downgraded; it is documented and covered by the 142/142 parallel and serial Rust suites, 38-pass frontend suite, clean-cache browser smoke, and Linux application runtime QA.
