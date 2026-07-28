# Google Maps Scraper Production Execution Ledger

**Date:** 2026-07-26
**Spec:** `docs/superpowers/specs/2026-07-26-google-maps-scraper-production-design.md`
**Plan:** `docs/superpowers/plans/2026-07-26-google-maps-scraper-production.md`
**Current Status:** TASK 15 COMPLETE; TASK 16 IN PROGRESS — Linux production runtime paths and release gates have passed, but Windows/macOS real-runtime and real CAPTCHA evidence are still missing. A peer-reviewed algorithm audit on 2026-07-26 additionally found four scrape/grid defects, two spec drifts, and — while investigating a serial-suite flake — an unretryable SQLite lock-upgrade defect, none of which was covered by any existing gate; see "Algorithm Audit 2026-07-26". REQ-005, REQ-010, REQ-011, and REQ-012 were reverted from COMPLETE to IN PROGRESS as a result. A CRITICAL code review of that remediation then found three further blockers, one of which was introduced by the remediation itself; see "Remediation Review 2026-07-26". Those are fixed with regression tests, but no claim is made here that the module is complete: the reworked unit model has still never been exercised against a real runtime. Final status remains **NOT READY**.

**Execution Mode:** Inline on `master`, explicitly selected by the user. No commits will be created unless requested.

**Pre-existing Change Boundary:** The worktree was already dirty before production implementation. Existing changes in `commands.rs`, `commands_test.rs`, `db_test.rs`, `grid.rs`, `GoogleMapsScraper.svelte`, outreach/CRM/UI files, repository instructions, `package.json`, and `bun.lock` must be preserved and reviewed rather than overwritten.

## Requirements

| ID | Requirement | Owner Task | Implementation Evidence | QA Evidence | Status |
|---|---|---:|---|---|---|
| REQ-001 | Persistent jobs | 2-3 | Durable schema plus async repository create/get/list/unit/result/delete operations | Task 2/3 tests; serial full Rust 54/54 | COMPLETE |
| REQ-002 | Atomic checkpoints | 2-3 | Transactional result/child/unit/job checkpoint, rollback on invalid unit state, batch unit rollback, optimistic versioning | Repository 12/12; rollback and counter tests | COMPLETE |
| REQ-003 | Startup interruption recovery | 3, 8, 12, 16 | Repository atomically marks running jobs interrupted and resets running units pending; Tauri setup initializes the singleton repository/runner/service and runs startup recovery without launching the browser | Automated restart tests plus Linux SIGKILL QA: sidecar/Chromium exited, restart converted `running` to `interrupted`, UI restored Resume/Cancel, resumed job then cancelled cleanly | COMPLETE |
| REQ-004 | One active worker | 8, 12 | Singleton managed runner owns one `ActiveJob`, rejects a conflicting second start, and executes units sequentially | Concurrent-start and service conflict tests; full Rust 124/124 parallel plus serial | COMPLETE |
| REQ-005 | Typed browser errors | 6-8, 12, 16 | Object-safe automation and production processor propagate typed search, grid, enrichment, import, and terminal-state failures without converting them to empty success; sidecar error serialization removes terminal escape sequences | Automated browser/Google Maps/processor tests; Linux forced Chromium termination retried exactly three times and stopped later cells; ANSI regression 7/7 with 1 intentional smoke skip. **Audit 2026-07-26 contradicts the "without converting them to empty success" claim: AUD-003 shows a detail-level non-ready state is converted into a successful truncated traversal, and AUD-004 shows a single malformed place escalates to a whole-job `Failed`.** | IN PROGRESS |
| REQ-006 | Official runtime download/cache | 5-6, 14, 16 | Sidecar pins CloakBrowser wrapper `0.5.1` and Chromium `146.0.7680.177.5`; Rust manager uses only the bundled sidecar; UI exposes direct first-use official download, retry, verification, and installed-version state | Linux clean-cache smoke downloaded about 206 MB from the official channel, verified Ed25519 and SHA-256, launched, reused cache, and cleaned up; Windows/macOS real-runtime evidence remains missing | IN PROGRESS |
| REQ-007 | Cross-platform sidecar builds | 5-6, 16 | Target-aware Bun compile plus Rust target-path resolution cover Linux x64/ARM64, Windows x64, and macOS x64/ARM64; Tauri registers `binaries/cloak-runtime` as `externalBin`; CI matrix covers Ubuntu, Windows, Intel macOS, and Apple Silicon macOS | Linux x64 standalone/runtime QA passes and target-aware CI commands are defined; Windows/macOS real-runtime QA remains missing | IN PROGRESS |
| REQ-008 | Persistent CAPTCHA profile | 9, 14, 16 | Jobs persist `profile_dir` and `blocked_url`; action-required detection relaunches the same profile headed; UI restores `paused_requires_action`, explains manual CAPTCHA completion, and resumes the exact job | Automated runner/browser/component same-profile tests pass; Linux pause/resume reused the exact `profile_dir`; a real CAPTCHA was not intentionally triggered | IN PROGRESS |
| REQ-009 | Backend grid validation | 4, 14, 16 | Finite/range/order/step validation, no-allocation estimate, 10,000 initial-cell cap, backend recommendation, and matching UI preflight estimate/warning | Automated validation/component tests plus Linux BBox `[10.775,10.7755,106.699,106.6995]` produced exactly one cell and completed `1/1` | COMPLETE |
| REQ-010 | Bounded adaptive subdivision | 4, 8, 12, 16 | Production processor checkpoints clipped child cells and enforces depth/min-size/zoom/total-cell/low-yield policy through the durable runner | Grid/runner/processor tests plus forced termination job `gmaps-1785048270761-296db70c275dde93`: first unit retried, eight later units were never claimed before recovery/cancel. **Audit 2026-07-26: the min-size policy is enforced at `min_cell_size_m = 20.0` against a spec value of 250 (AUD-005), and the total-cell budget is enforced by a silent `return false` instead of the specified `paused_limit_reached` transition (AUD-006).** | IN PROGRESS |
| REQ-011 | Raw listing saturation | 7-8, 12, 16 | Production processor uses independent `raw_listing_count`, `reached_end`, blocked reason, and deduplicated details to decide subdivision and terminal state | Automated typed scraper tests plus real keyword job discovered 15 listings without false empty/selector completion. **Audit 2026-07-26: both inputs to the subdivision decision are unsound — `reached_end` is reported `false` for a demonstrably exhausted feed (AUD-001) and `raw_listing_count` is rewritten to zero by a detail-level fault (AUD-003). Two red characterization tests are committed and failing.** | IN PROGRESS |
| REQ-012 | Spatial filtering and dedup | 7-8, 12, 16 | Processor persists candidates, applies inclusive BBox/exact-location qualification, and preserves place ID, canonical URL, phone, then name/address dedup precedence; keyword/deep-scan jobs without BBox are explicitly inside scope | Spatial/processor regressions plus Linux keyword and one-cell grid runtime QA. **Audit 2026-07-26: qualification reads the viewport-centre `@lat,lng` in preference to the authoritative `!3d`/`!4d` place coordinates (AUD-007), and candidates qualified as outside the requested area are stranded at `import_status='pending'` with no reason counter (AUD-008).** | IN PROGRESS |
| REQ-013 | Safe email enrichment | 10 | `SafeHttpClient` allows only credential-free HTTP(S), disables proxies and automatic redirects, resolves and validates every hop, rejects non-public destinations, pins the approved address, stays on one registrable domain, enforces three redirects/2 MB/5s connect/15s request; deterministic robots-aware crawl visits at most five HTML pages and selects mailto before visible email; repository atomically claims and checkpoints `running/found/not_found/retry_wait/failed` without repeating Maps scraping | Email security/extraction 14/14 including real reqwest pinned-address integration; repository enrichment checkpoint 1/1; full Rust 102/102 parallel and serial | COMPLETE |
| REQ-014 | Phone/email Lead qualification | 11, 16 | Importer independently normalizes Vietnamese mobile formats (`03/05/07/08/09`, local/`84`/`0084`) and normalized email; landline-only results are rejected | Automated importer tests plus real `Dien Lanh HK` job: 15 qualified results, 10 Leads created, 5 contact duplicates | COMPLETE |
| REQ-015 | Idempotent Lead import | 2, 11, 16 | `BEGIN IMMEDIATE` batch revalidates contacts, deduplicates normalized phone and case-insensitive trimmed email across existing Leads and the current batch, writes Lead/result status/job counters atomically, and rolls back on any SQL/read/insert failure | Automated replay/rollback tests plus SQLite evidence for job `gmaps-1785047852973-54455993d3e8a22f`: `imported=10`, `duplicate=5`, total Leads `10` | COMPLETE |
| REQ-016 | Job-scoped sequenced events | 8, 12-14 | Publisher emits committed snapshots with `job_id` and monotonic sequence; typed frontend reducer and component reject other-job, stale, and post-terminal events | Rust event/race tests, service 5/5, component 9/9, and full Rust 124/124 | COMPLETE |
| REQ-017 | Pause/resume/cancel race safety | 8-9, 12-16 | Backend commits pause/action/cancel transitions before cleanup; UI maps Pause, Resume, Cancel, retry, interrupted recovery, and CAPTCHA confirmation to the active persisted job and resets state from command responses | Automated race/profile/UI tests; Linux pause reached `paused` in 237 ms, resume reused the same profile, cancel completed in about 90 ms, and no scraper-owned process remained | COMPLETE |
| REQ-018 | Structured diagnostics and cleanup | 6, 15-16 | Sidecar diagnostics stay on stderr, stdout is typed JSON, rolling logs redact secrets, diagnostics are job/unit scoped, retention preserves action-required profiles, shutdown is graceful then forced with async wait/reap, and protocol errors strip ANSI controls | Task 15 redaction/retention/wrapper tests; forced termination/crash recovery/clean-cache process checks; ANSI regression test passes | COMPLETE |
| REQ-019 | Compatibility wrappers | 15 | Legacy start/stop commands delegate to durable create/resume/cancel flows and cannot emit duplicate terminal events | Task 15 compatibility tests and full Rust release suite | COMPLETE |
| REQ-020 | Full release QA | 16 | Cross-platform CI and complete local Linux release matrix are implemented | Linux release gates and real-runtime QA pass; Windows/macOS real-runtime and real CAPTCHA evidence remain mandatory blockers | IN PROGRESS |

## Algorithm Audit 2026-07-26

An independent scrape/grid algorithm audit was performed after Task 16, peer-reviewed, and corrected before being recorded here. It found defects that no existing gate covers: every finding below is invisible to the 142-test Rust suite, to Clippy, and to the Linux runtime QA, because each one produces a *successful-looking* outcome rather than a failure. No historical QA evidence in this ledger is retracted — the runs happened and passed as recorded; they simply did not exercise these paths.

**No production code has been changed by this audit.** Three red characterization tests are committed and failing by design.

| ID | Severity | Finding | Location |
|---|---|---|---|
| AUD-001 | HIGH | `scroll_feed` reports `reached_end = false` whenever the localized end-of-list sentinel is absent, even after the feed height has stopped changing. `should_subdivide` then fans a demonstrably exhausted cell into four children. Measured fan-out is roughly 21 cells per affected initial cell — the `consecutive_low_yield_levels >= 2` guard caps the cascade at two to three levels, so this is a ~20x waste, not an unbounded explosion. | `src-tauri/src/scrapers/google_maps.rs:244`, `src-tauri/src/scrapers/grid.rs:180` |
| AUD-002 | HIGH | A CAPTCHA encountered part-way through detail traversal returns the places collected so far, but the processor converts the blocked reason into an `Err` before `page_to_execution`, and `checkpoint_completed_unit` persists results only for a successful unit. All detail work before the block is discarded and the unit restarts from zero on resume. Because that traffic is what provokes the block, the unit can fail at the same point indefinitely. | `src-tauri/src/scrapers/google_maps.rs:167`, `src-tauri/src/google_maps_jobs/processor.rs:136`, `src-tauri/src/google_maps_jobs/repository.rs:560` |
| AUD-003 | HIGH | A non-ready state on a *place detail* page is folded into the search-level `EmptyResults` branch, which hardcodes `raw_listing_count` to zero; the caller then forces `reached_end = false`. The unit is checkpointed as completed with partial data, the unvisited place URLs are dropped permanently, and the zeroed count feeds the subdivision decision. | `src-tauri/src/scrapers/google_maps.rs:374` |
| AUD-004 | MEDIUM-HIGH | A place detail page missing an `<h1>`, or whose URL fails canonicalization, raises a non-retryable place-level error. The runner escalates it to a whole-job `Failed` and abandons every remaining pending unit. One anomalous listing can end a 500-district job. | `src-tauri/src/scrapers/google_maps.rs:294`, `src-tauri/src/google_maps_jobs/runner.rs:523` |
| AUD-005 | MEDIUM | Spec drift: the design specifies `min_cell_size_m = 250`; `GridLimits::default()` uses `20.0`, permitting far deeper subdivision than the approved safety budget. | spec §11.3, `src-tauri/src/scrapers/grid.rs:35` |
| AUD-006 | MEDIUM | Spec drift: the design specifies that reaching a safety budget pauses the job as `paused_limit_reached`. `ScrapeJobStatus::PausedLimitReached` exists in the enum, the transition table, and the `start()` gate, but no code path ever emits it. Hitting `max_total_cells` silently stops subdivision and the job reports `Completed` despite truncated coverage. | spec §11.3, `src-tauri/src/scrapers/grid.rs:192` |
| AUD-007 | MEDIUM | Both the injected JavaScript and the Rust parser prefer the viewport-centre `@lat,lng` over the authoritative `!3d`/`!4d` place coordinates. `inside_requested_area` depends on these values, so listings near a bbox edge can be misqualified and silently dropped. | `src-tauri/src/scrapers/google_maps.rs:485`, `sidecars/cloak-runtime/src/googleMaps.ts` |
| AUD-008 | MEDIUM | Candidates with `inside_requested_area = 0` are stranded at `import_status='pending'` forever with no reason counter, so the gap between `businesses_discovered` and `leads_created` is unattributable in the UI. Note that enrichment does *not* filter on this column — only import does — so these rows still consume outbound HTTP. | `src-tauri/src/google_maps_jobs/processor.rs:376`, `src-tauri/src/google_maps_jobs/importer.rs:79`, `src-tauri/src/google_maps_jobs/repository.rs:864` |
| AUD-009 | MEDIUM | `classifyGoogleMapsPageState` matches `'try again later'`, `'rate limit'`, and `'no results found'` against up to 20,000 characters of `innerText`, and orders those checks *before* the structural `hasFeed → ready` check. Ordinary review or listing text can therefore pause an entire job for user action. | `sidecars/cloak-runtime/src/googleMaps.ts:20` |
| AUD-010 | MEDIUM | `RateLimited` routes straight to action-required, pausing the job for a human instead of backing off and retrying a potentially transient throttle. | `src-tauri/src/google_maps_jobs/runner.rs:646` |
| AUD-011 | UNPROVEN | Hypothesis that Google writes the `!1s` feature id into the URL asynchronously, so `place_id` may be absent at extraction time and dedup falls back to a `canonical_url` that varies by entry viewport, inflating `businesses_discovered`. The URL fallback path is real and confirmed by code; the asynchronous-timing premise requires runtime capture to prove and is **not** treated as a confirmed defect. | `src-tauri/src/scrapers/google_maps.rs:444` |
| AUD-012 | HIGH | Found while investigating a serial-suite flake, and confirmed to be a production defect rather than a test artefact. All 20 repository transactions were `BEGIN DEFERRED` while every one of them reads before it writes. Two concurrent operations therefore take read locks and then deadlock on upgrade, which SQLite reports as `SQLITE_BUSY_SNAPSHOT` (error 517, "Cannot promote read transaction to write transaction") — a code `busy_timeout` deliberately does not retry, despite WAL and a 5 s timeout both being configured correctly. Reproduced 5/5 with 24 concurrent claims. In production this is reachable whenever a Tauri command runs while the runner checkpoints, for example the user pressing Cancel mid-unit. The importer already used `BEGIN IMMEDIATE`; nothing else did. | `src-tauri/src/google_maps_jobs/repository.rs`, `src-tauri/src/db.rs:28` |

### Red Characterization Tests

Committed and failing by design. These are characterization tests, not acceptance tests — they pin current wrong behaviour so a fix is observable.

| Test | Covers | Original failure | Now |
|---|---|---|---|
| `stable_feed_without_end_sentinel_reports_reached_end` | AUD-001 | `reached_end` is `false` for a feed whose height and place count stopped changing | GREEN |
| `default_grid_limits_match_the_approved_safety_budget` | AUD-005 | `min_cell_size_m` is `20.0`, spec requires `250.0` | GREEN |
| `detail_page_empty_results_preserves_search_listing_count` | AUD-003 | `raw_listing_count` is `0`, expected `2` | REPLACED |
| `detail_page_empty_results_is_not_reported_as_a_clean_traversal` | AUD-003 | one of two places visited, still reported as a successful unit | REPLACED |

The two AUD-003 characterization tests were replaced when AUD-002 landed, exactly as this ledger required and never cited as evidence of the fix. Their replacements assert the durable contract instead of the wrong behaviour:

| Replacement test | Asserts |
|---|---|
| `collecting_a_search_page_does_not_visit_place_details` | the search unit stops at the listing; any detail navigation would be an unscripted browser call |
| `place_detail_empty_results_is_a_place_scoped_fault` | a place that fails to render is a retryable place-level `InvalidResponse`, never a search-level empty result |
| `place_detail_captcha_surfaces_as_captcha` | a block on one place is reported as that block, so the runner can pause without discarding checkpointed places |
| `search_unit_emits_one_place_child_per_listing` | one durable `place` unit per URL, so the job cannot complete while any place URL is unprocessed |
| `a_place_found_by_two_cells_creates_one_unit` | the viewport-independent place unit key collapses cross-cell duplicates without failing the checkpoint |
| `a_failed_place_unit_does_not_abort_the_remaining_units` | a place failure is recorded and the run continues; the job ends `Partial` |
| `place_failures_past_the_threshold_stop_the_job` | tolerance is bounded, so a systemic failure still stops the job |
| `a_failed_search_unit_still_stops_the_job` | search-level failures remain fatal |
| `rate_limiting_retries_before_requiring_user_action` | a throttle is retried with backoff before a human is asked to intervene |
| `grid_budget_exhaustion_pauses_the_job_as_limit_reached` | budget exhaustion pauses the job and leaves pending units intact |
| `concurrent_writes_never_fail_on_a_lock_upgrade` | concurrent repository writes never fail with an unretryable lock upgrade |
| `enrichment_skips_results_outside_the_requested_area` | out-of-area rows do not consume outbound enrichment HTTP |

**These two AUD-003 tests must not be cited as evidence that AUD-002 or AUD-003 is fixed.** When the AUD-002 partial-checkpoint design lands, `detail_page_empty_results_preserves_search_listing_count` is to be replaced by an assertion against the partial/checkpoint payload, and `detail_page_empty_results_is_not_reported_as_a_clean_traversal` by a repository- or processor-level test proving a unit cannot complete while place URLs remain unprocessed.

### Agreed Remediation Order

1. This ledger update.
2. AUD-001 via termination semantics, plus AUD-005. Agreed thresholds: compare the `(height, place_count)` pair rather than height alone; either value changing resets the counter; three consecutive unchanged observations (four snapshots including the baseline) sets `reached_end = true`; the sentinel and the 120 cap still terminate immediately; exhausting `max_scroll_attempts` while data is still changing remains `reached_end = false`. No separate "exhausted" and "gave up" thresholds — `max_scroll_attempts` already is the give-up branch.
3. AUD-002 by promoting each place detail to a child unit, or by checkpointing `next_place_index` together with partial results.
4. AUD-003 on top of that checkpoint; `EmptyResults` must distinguish search level from detail level.
5. AUD-004 via a per-place error ledger and an error-rate threshold.
6. Then AUD-009, AUD-007, AUD-011, AUD-008, AUD-010.

AUD-003 must not be merged independently ahead of AUD-002; doing so only converts silent data loss into a retry/livelock of a different shape.

### Audit Gate Evidence

Measured at the audit checkpoint, before any remediation:

| Gate | Command | Exit | Result |
|---|---|---:|---|
| Rust tests | `cargo test --manifest-path src-tauri/Cargo.toml` | 101 | 142 passed, 3 failed — the three red characterization tests only, no regression |
| Rust format | `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | 0 | PASS |
| Rust Clippy | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | 0 | PASS, no issues found |

### Remediation Progress

**AUD-001 — FIXED.** `scroll_feed` now compares the `(height, place_count)` pair rather than height alone, resets its counter when either value moves, and sets `reached_end = true` after `STABLE_FEED_OBSERVATIONS = 3` consecutive identical observations. The sentinel and the 120-result cap still terminate immediately, and exhausting `max_scroll_attempts` while either value is still changing still reports `false`, which remains the distinct give-up branch. `should_subdivide` and spec §12 were deliberately left unchanged: the defect was in the evidence feeding the decision, not in the decision rule. GitNexus impact for `scroll_feed` was LOW — one direct caller, 14 impacted symbols confined to the `Scrapers` module, zero affected processes.

**AUD-005 — FIXED.** `GridLimits::default().min_cell_size_m` moved from `20.0` to the specified `250.0`, pinned by a test that also pins `max_depth`, `max_initial_cells`, and `max_total_cells` against future drift. Side benefit: it independently bounds any residual AUD-001-style fan-out, because cells below 250 m no longer subdivide at all.

**AUD-002, AUD-003, AUD-004 — FIXED together, by making a place detail its own durable unit.** A search unit now stops at the listing (`collect_search_listing`) and emits one `place` child unit per URL; a place unit visits exactly one page (`scrape_place`). This resolves all three findings at their shared root rather than individually:

- AUD-002: places already scraped are already completed units, so a block part-way through a listing costs the current place only. Resume re-runs one place, not the whole search page.
- AUD-003: a non-ready place page is classified by `place_state_error`, which is structurally separate from the search-level `terminal_listing_state`. `EmptyResults` on a place is now a retryable `InvalidResponse` scoped to that place, and can no longer zero the search listing count or complete a truncated traversal.
- AUD-004: `tolerable_unit_failure` absorbs a failed `place` unit, records it, and continues to the next unit; the job ends `Partial` because a failure was recorded. Search and grid unit failures remain job-fatal, because an unreadable search page would fail every later unit identically. Tolerance is bounded by `FailureTolerance` — see REV-003, which replaced the first attempt's absolute count.

Two side effects worth recording. Place unit keys are viewport-independent, so the same business found by two grid cells collapses to one unit through the existing `INSERT OR IGNORE` child insert — covered by a new test, since that behaviour is now load-bearing. And subdivision novelty is now measured from place URLs rather than extracted candidates, because the search unit completes before any detail is fetched; `place_url_dedup_key` applies the same `place_id`-then-URL precedence the extracted candidate would.

**AUD-006 — FIXED.** `grid_budget_exhausted` distinguishes "this cell does not need splitting" from "this cell needs splitting but the budget is gone". The processor reports the second case through `UnitExecution::budget_exhausted`, and the runner transitions the job to `PausedLimitReached` with pending units left intact, as spec §11.3 requires. This is the first code path that ever emits that status.

**AUD-007 — FIXED.** `coordinates_from_url` and the injected extraction script both prefer `!3d`/`!4d` over the `@lat,lng` viewport centre.

**AUD-008 — FIXED.** A candidate outside the requested area is persisted as `import_status='skipped_outside_area'` rather than being stranded at `pending`, making the discovered-to-imported gap attributable by query. Enrichment now also filters on `import_status='pending'`, so these rows no longer consume outbound HTTP for a lead that can never be created. Surfacing the count in the UI remains outstanding.

**AUD-009 — FIXED.** `classifyGoogleMapsPageState` checks URL and structural signals before body text, so a feed or place heading outranks incidental listing and review copy. A genuine `/sorry/` interstitial or consent dialog still outranks structure.

**AUD-010 — FIXED.** `RateLimited` is raised as retryable and the runner's retry arm is now evaluated before the action-required arm, so a throttle is retried with exponential backoff and only becomes an action-required pause once retries are exhausted. `Captcha` and `ConsentRequired` stay non-retryable and pause immediately, because retrying them is pointless.

**AUD-011 — MITIGATED, premise still unproven.** Identity is derived by `place_identity`, which is viewport-independent, so the dedup fallback is identical across entry points. This removes the impact without relying on the asynchronous-`!1s` premise, which still has no runtime evidence either way. The first attempt at this fix over-stripped the URL and introduced REV-001; see "Remediation Review 2026-07-26".

**AUD-012 — FIXED.** All 20 repository transactions now use `BEGIN IMMEDIATE`, matching what the importer already did. The lock-upgrade failure is covered by a concurrency test that reproduced the defect 5/5 before the fix and passes 5/5 after.

### Post-Remediation Gate Evidence

The `google_maps_scrape_job_command_rejects_second_active_job` flake that exposed AUD-012 did not recur in six consecutive serial runs after the fix.

| Gate | Command | Exit | Result |
|---|---|---:|---|
| Rust tests, parallel | `cargo test --manifest-path src-tauri/Cargo.toml` | 0 | 161 passed, 0 failed |
| Rust tests, serial ×6 | `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | 0 | 161 passed, 0 failed on every run |
| Rust format | `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | 0 | PASS |
| Rust Clippy | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | 0 | PASS, no issues found |
| Rust release build | `cargo build --manifest-path src-tauri/Cargo.toml --release` | 0 | optimized binary built |
| Frontend tests | `bun run test` | 0 | 40 pass, 1 skip, 0 fail; 126 assertions |
| Svelte diagnostics | `bun run check` | 0 | 0 errors, 0 warnings across 4,633 files |
| Sidecar build | `bun run build:cloak-runtime` | 0 | Linux x64 standalone built; binary removed afterwards, `.gitkeep` retained |
| Repository whitespace | `git diff --check` | 0 | clean |

## Remediation Review 2026-07-26

An independent CRITICAL review of the remediation above found three blockers and three coverage gaps. **REV-001 was introduced by the remediation itself and was more damaging than the finding it came from**, which is recorded here rather than quietly fixed: a fix that adds a data-loss path is worse than the defect it replaces, and the review caught what the 161-test suite did not.

| ID | Severity | Finding | Resolution |
|---|---|---|---|
| REV-001 | CRITICAL | The place unit key used a helper that strips everything after `/@` or `/data=`, leaving the bare name slug. Two branches of a chain — different feature ids, different coordinates — collapsed onto one key such as `place:.../maps/place/starbucks`, and the `INSERT OR IGNORE` child insert then silently discarded one of the businesses. Introduced while fixing AUD-011. | Replaced by a single `place_identity` used by the place unit key, the listing novelty key, and the persisted result dedup key alike. Precedence: `!1s` feature id, then authoritative `!3d`/`!4d` coordinates, then the URL with only the viewport segment removed. The slug is never an identity. |
| REV-002 | HIGH | `max_total_cells` is a budget of grid cells, but `should_subdivide` and `grid_budget_exhausted` were given `job.total_units`, which after the place-unit split is dominated by place units. A job would pause as `paused_limit_reached` having barely subdivided. | New `count_units_by_type` measures grid units alone; place units no longer consume the cell budget. |
| REV-003 | HIGH | Failure tolerance was an absolute count, so a 20-place job where every place failed with `SelectorChanged` ran to completion and reported `Partial` — near-certain selector drift reported as a successful-ish run. The field name also implied one more tolerated failure than it allowed. | Replaced by `FailureTolerance`: a consecutive-failure guard (default 5) that catches systemic drift before any ratio has a sample, a failure ratio (default 0.5) applied only past a minimum sample (default 20), and a hard absolute cap (default 50). The consecutive counter resets on any successful unit. |
| REV-004 | MEDIUM | Subdivision novelty compared listing keys against completed results only. Grid children sort before place children on the same timestamp, so a place already scheduled but not yet scraped counted as new and inflated the low-yield heuristic. | Novelty now compares against completed results **and** existing place unit keys, via `list_place_unit_keys`. |
| REV-005 | Coverage | No cross-layer test proved the central claim of the place-unit architecture. | `a_block_mid_listing_only_costs_the_blocked_place` walks search checkpoint → place children → one completes → next is blocked → job pauses → resume claims the blocked place and only the blocked place. |
| REV-006 | Coverage | The classifier had no structural CAPTCHA signal, so a challenge overlay that leaves the feed DOM intact would be masked by the `hasFeed → ready` branch. | `hasCaptchaChallenge` is collected from the page and outranks the ready branch. |
| REV-007 | Coverage | Out-of-area results were no longer enriched but kept `enrichment_status='pending'` forever. | Both queues now carry the terminal `skipped_outside_area`, so neither shows work that will never be done. |

REV-005 passed on first run. It is a coverage test, not a red-then-green fix: it protects behaviour the place-unit architecture already provides, and is recorded as such rather than as evidence of a defect found.

## Second Review 2026-07-27

A second CRITICAL review found that three of the fixes above were incomplete. **Two of the three were unreachable or self-defeating in exactly the way the first round claimed to have solved**, which is recorded rather than folded silently into the tables above.

| ID | Severity | Finding | Resolution |
|---|---|---|---|
| REV-002a | HIGH | The REV-003 failure ratio could never fire. `checkpoint_failed_unit` increments `processed_units` *and* `failed_units` for the same unit, so a job-wide `failed / (processed + failed)` is capped at one half and any threshold above one half is unreachable dead code. Eleven failures against nine successes read as 35.5%, not 55%. | `count_place_unit_outcomes` counts completed and failed **place units** separately, and the ratio is computed over those. Covered by a test that interleaves failures with successes so the consecutive guard never fires and only the ratio can stop the job. |
| REV-002b | HIGH | Pausing as `paused_limit_reached` was informational only. The blocked cell is checkpointed as completed before the pause, so if it was the last unit, resuming found nothing to claim and went straight to `Completed` — reporting full coverage of an area the job stopped subdividing. | A durable `coverage_truncated` column is set when the budget is spent and never cleared. Finalization forces `Partial` while it is set, so resuming is the operator accepting a shortfall rather than the shortfall disappearing. |
| REV-002c | HIGH | The REV-001 identity fix still collapsed to the slug in one branch: when a URL has `/@…` with no path segment after it, `url_without_viewport` returned everything *before* `/@`, which is the bare slug. Two branches distinguished only by viewport merged again. The REV-001 test used two different `data=` payloads and never reached that branch. | `url_without_viewport` now returns `None` when nothing survives the strip, and identity keeps the whole URL including the viewport in that case. Where no discriminator exists, identity stays conservative: a duplicate unit is recoverable, a silently merged business is not. |
| REV-002d | MEDIUM | The same place listed once with `!1s` and once without produced a `place:` key and a `geo:` key, which the union check could not recognise as one business, so it could still be scheduled twice and inflate novelty. | `place_identity_aliases` returns every legitimate identity form, and both child planning and novelty compare against all of them rather than the primary key alone. |

`canonicalize_place_url` still drops the query string before identity is derived. No place discriminator is known to be carried there for `/maps/place/` feed anchors, so this is left as recorded rather than changed speculatively; if a URL arrives with no feature id, no coordinates, and no path discriminator, identity is genuinely indeterminate from the URL and the conservative viewport-preserving fallback is what limits the damage.

### Review Gate Evidence

| Gate | Command | Exit | Result |
|---|---|---:|---|
| Rust tests, parallel | `cargo test --manifest-path src-tauri/Cargo.toml` | 0 | 169 passed, 0 failed |
| Rust tests, serial ×5 | `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | 0 | 169 passed, 0 failed on every run |
| Rust format | `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | 0 | PASS |
| Rust Clippy | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | 0 | PASS, no issues found |
| Rust release build | `cargo build --manifest-path src-tauri/Cargo.toml --release` | 0 | optimized binary built |
| Frontend tests | `bun run test` | 0 | 41 pass, 1 skip, 0 fail; 127 assertions |
| Svelte diagnostics | `bun run check` | 0 | 0 errors, 0 warnings across 4,633 files |
| Repository whitespace | `git diff --check` | 0 | clean |

### Still Outstanding

Fixing the algorithm defects does not clear the release. These remain blockers and are unchanged by this work:

- Windows and macOS real-runtime download, launch, and cleanup evidence. CI compilation is not a substitute.
- A real CAPTCHA exercised end to end.
- Runtime re-validation of the reworked unit model on Linux. Every claim above rests on automated tests; the place-unit split changes the shape of a real run substantially — unit counts, event volume, and browser navigation pattern — and the Linux runtime QA in this ledger predates it.
- UI surfacing of the `skipped_outside_area` count (AUD-008). An earlier revision of this ledger also claimed `paused_limit_reached` was unrendered; that was wrong. `GoogleMapsJobPanel.svelte` already carries its status label, its explanatory message, and a Resume action, and `googleMapsScraperService.ts` already types the status. What is new is that resuming past a budget pause can no longer end as `Completed` — see REV-002b.

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
