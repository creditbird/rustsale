# Google Maps Scraper Production Design

**Date:** 2026-07-26
**Status:** Approved design, pending implementation plan
**Scope:** RustSale v2 desktop application, Google Maps keyword, deep-scan, and adaptive grid scraping

## 1. Purpose

Replace the current fire-and-forget Google Maps scraper with a persistent, bounded, resumable job engine that behaves predictably under browser disconnects, application restarts, CAPTCHA challenges, database failures, malformed grid input, and partial external-site failures.

The implementation must support Linux, Windows, and macOS. CloakBrowser remains the required browser runtime, but its Chromium binary is not redistributed inside the RustSale installer. The official CloakBrowser package downloads and caches the platform-specific runtime on first use.

## 2. Approved Product Decisions

- Use a persistent Tauri-local job engine backed by SQLite.
- Allow only one active browser worker at a time.
- Keep the current grid input mechanisms:
  - province/city presets;
  - geocoded bounding boxes;
  - manually editable bounding boxes;
  - manually editable grid step.
- Validate and bound every grid request in the Rust backend.
- Persist checkpoints after every district, ward, keyword search, or grid cell.
- Do not auto-resume after application restart. Convert abandoned active jobs to `interrupted` and require an explicit Resume action.
- When CAPTCHA, consent, or an automation block is detected, pause the job, reopen the same profile in headed mode, and require the user to complete the browser action manually.
- Use one sequential browser worker per job.
- Create a Lead when a valid Vietnamese mobile number or a valid email exists.
- If a Google Maps result has no valid mobile number but has a website, inspect the homepage and up to four same-domain contact/about pages for the first valid email.
- Treat a matching normalized phone or normalized email as an existing Lead.
- Keep one primary email on the Lead model.

## 3. Non-Goals

- Distributed or server-hosted scraper workers.
- Multiple concurrent browser workers.
- Automated CAPTCHA solving.
- Full-site email crawling.
- Multiple emails per Lead.
- Bundling or redistributing the CloakBrowser Chromium binary without an OEM agreement.
- Guaranteeing complete Google Maps coverage when Google itself withholds, personalizes, or blocks results.

## 4. Target Architecture

The module is divided into explicit layers:

1. `ScrapeJobRepository`
   - Persists jobs, units, results, state transitions, counters, and checkpoints.
2. `ScrapeJobRunner`
   - Enforces one active runner and executes resumable units.
3. `BrowserSupervisor`
   - Owns the CloakBrowser runtime sidecar and its process lifecycle.
4. `BrowserAutomationClient`
   - Sends typed JSON commands to the sidecar and maps typed responses into Rust errors.
5. `GoogleMapsScraper`
   - Discovers listings and extracts place details without swallowing browser errors.
6. `GridPlanner`
   - Validates, generates, clips, and adaptively subdivides grid cells.
7. `EmailEnricher`
   - Performs bounded same-domain HTML crawling with SSRF protection.
8. `LeadImporter`
   - Imports qualified results transactionally and idempotently.
9. `ScrapeEventPublisher`
   - Emits sequenced, job-scoped Tauri events.

## 5. Persistent Data Model

### 5.1 `google_maps_scrape_jobs`

Required fields:

- `id TEXT PRIMARY KEY`
- `mode TEXT NOT NULL`
- `keyword TEXT NOT NULL`
- `location TEXT`
- `bbox_json TEXT`
- `step_km REAL`
- `headless INTEGER NOT NULL`
- `status TEXT NOT NULL`
- `stage TEXT NOT NULL`
- `total_units INTEGER NOT NULL DEFAULT 0`
- `processed_units INTEGER NOT NULL DEFAULT 0`
- `failed_units INTEGER NOT NULL DEFAULT 0`
- `businesses_discovered INTEGER NOT NULL DEFAULT 0`
- `businesses_qualified INTEGER NOT NULL DEFAULT 0`
- `leads_created INTEGER NOT NULL DEFAULT 0`
- `last_error_kind TEXT`
- `last_error_message TEXT`
- `action_required TEXT`
- `event_sequence INTEGER NOT NULL DEFAULT 0`
- `version INTEGER NOT NULL DEFAULT 0`
- `created_at TEXT NOT NULL`
- `started_at TEXT`
- `updated_at TEXT NOT NULL`
- `completed_at TEXT`

### 5.2 `google_maps_scrape_units`

Each keyword query, district, ward, or grid cell is a checkpoint unit.

Required fields:

- `id TEXT PRIMARY KEY`
- `job_id TEXT NOT NULL`
- `unit_key TEXT NOT NULL`
- `unit_type TEXT NOT NULL`
- `payload_json TEXT NOT NULL`
- `status TEXT NOT NULL`
- `depth INTEGER NOT NULL DEFAULT 0`
- `attempt_count INTEGER NOT NULL DEFAULT 0`
- `next_retry_at TEXT`
- `last_error_kind TEXT`
- `last_error_message TEXT`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`
- `completed_at TEXT`
- unique constraint on `(job_id, unit_key)`
- foreign key to `google_maps_scrape_jobs`

### 5.3 `google_maps_scrape_results`

Required fields:

- `id TEXT PRIMARY KEY`
- `job_id TEXT NOT NULL`
- `source_unit_id TEXT NOT NULL`
- `place_id TEXT`
- `canonical_url TEXT`
- `dedup_key TEXT NOT NULL`
- `name TEXT NOT NULL`
- `phone TEXT`
- `normalized_phone TEXT`
- `email TEXT`
- `normalized_email TEXT`
- `address TEXT`
- `website TEXT`
- `facebook TEXT`
- `zalo TEXT`
- `latitude REAL`
- `longitude REAL`
- `inside_requested_area INTEGER`
- `detail_status TEXT NOT NULL`
- `enrichment_status TEXT NOT NULL`
- `import_status TEXT NOT NULL`
- `last_error_kind TEXT`
- `last_error_message TEXT`
- `first_seen_at TEXT NOT NULL`
- `last_seen_at TEXT NOT NULL`
- unique constraint on `(job_id, dedup_key)`
- foreign keys to jobs and units

### 5.4 Checkpoint Atomicity

For every completed unit, one SQLite transaction must:

1. upsert discovered results;
2. create child grid units when subdivision is required;
3. mark the current unit completed;
4. update job counters and progress;
5. increment the event sequence.

If the transaction fails, the unit remains retryable and no completed checkpoint is recorded.

## 6. Job State Machine

Valid states:

- `queued`
- `running`
- `paused`
- `paused_requires_action`
- `paused_limit_reached`
- `interrupted`
- `partial`
- `completed`
- `failed`
- `cancelled`

Valid stages:

- `planning`
- `installing_browser_runtime`
- `scraping`
- `email_enrichment`
- `importing`
- `cleanup`

Quiescent terminal states are `partial`, `completed`, `failed`, and `cancelled`. No background component may emit a later state transition after a terminal state is committed. An explicit user Retry action may reopen a retryable `partial` job by atomically incrementing its run generation, returning eligible failed units to `pending`, and transitioning the job to `queued`. `completed`, `failed`, and `cancelled` jobs are not reopened automatically.

At application startup, persisted `running` jobs become `interrupted`. The browser does not start until the user resumes the job.

## 7. CloakBrowser Runtime

### 7.1 Distribution Model

The RustSale installer contains a small `cloak-runtime` sidecar, not the CloakBrowser Chromium binary. The sidecar uses the official CloakBrowser package to download and cache the platform runtime on first use.

No system Chrome or Chromium fallback is allowed.

### 7.2 Sidecar Builds

Build a Bun-compiled executable for:

- Linux x64
- Linux ARM64
- Windows x64
- macOS Intel
- macOS Apple Silicon

Windows ARM64 remains unsupported until an official CloakBrowser binary exists and the target passes release QA.

### 7.3 Runtime Configuration

- Use an application-owned cache directory through `CLOAKBROWSER_CACHE_DIR`.
- Disable automatic runtime updates during jobs.
- Pin a wrapper and binary version that passed Google Maps regression tests.
- Keep checksum verification enabled.
- Never expose a license key to the frontend or logs.

### 7.4 Sidecar Protocol

Use newline-delimited JSON over stdin/stdout. Each request and response includes a correlation ID.

Required commands:

- `runtime_status`
- `ensure_runtime`
- `launch`
- `navigate`
- `evaluate`
- `screenshot`
- `health_check`
- `shutdown`

Required error shape:

```json
{
  "kind": "navigation_timeout",
  "message": "...",
  "retryable": true
}
```

### 7.5 Process Ownership

- Rust owns the sidecar process handle.
- The sidecar owns the Playwright persistent context and browser process.
- Normal shutdown sends `shutdown` and waits with a bounded timeout.
- Forced shutdown terminates the sidecar only after graceful shutdown fails.
- The sidecar handles operating-system termination signals and closes the browser context.
- Blocking process waits must not run on Tokio worker threads.

### 7.6 Profiles and CAPTCHA

Each job receives a profile directory under application data. Resume reuses the same directory.

When CAPTCHA, consent, or blocking is detected:

1. commit `paused_requires_action`;
2. close the headless context;
3. relaunch the same profile in headed mode;
4. navigate to the blocked URL;
5. wait for the user to confirm completion;
6. verify expected Google Maps selectors;
7. resume the interrupted unit.

The system does not automate CAPTCHA solving.

## 8. Browser Error Taxonomy

Replace string matching with typed errors:

- `Disconnected`
- `NavigationTimeout`
- `EvaluationTimeout`
- `TransientNetwork`
- `Captcha`
- `ConsentRequired`
- `RateLimited`
- `SelectorChanged`
- `InvalidResponse`
- `BrowserStartup`
- `RuntimeUnavailable`
- `InvalidInput`
- `Database`
- `Cancelled`

Policy:

- Disconnect, navigation timeout, evaluation timeout, and transient network errors retry a unit up to three times with exponential backoff and browser restart.
- CAPTCHA, consent, and rate limiting pause for user action.
- Selector changes and invalid responses fail the job to prevent uncontrolled traversal.
- Database errors rollback and fail the job.
- Cancellation is not reported as failure or completion.

## 9. Google Maps Search Contract

Every search returns:

```rust
SearchPageResult {
    raw_listing_count,
    reached_end,
    blocked_reason,
    places,
}
```

Every place candidate contains:

```rust
PlaceCandidate {
    place_id,
    canonical_url,
    name,
    phone,
    address,
    website,
    latitude,
    longitude,
}
```

The scraper must distinguish:

- a legitimate empty result;
- a truncated feed;
- a blocked page;
- a selector regression;
- a browser transport failure.

Browser errors must never become empty arrays, null values, or successful partial results without an explicit `partial` outcome.

## 10. Keyword and Deep-Scan Algorithm

- Keyword mode creates one search unit.
- Deep Scan creates one unit per selected district or ward.
- Units execute sequentially.
- A fatal browser error stops traversal immediately.
- A retryable unit failure follows the retry policy.
- A permanently failed place detail is stored as `detail_failed` and causes a `partial` terminal state if it remains unresolved.
- Results with coordinates outside the requested area are retained for audit and marked accordingly.

## 11. Grid Validation and Generation

### 11.1 Validation

The Rust backend validates:

- all coordinates and step values are finite;
- latitude is within `[-90, 90]`;
- longitude is within `[-180, 180]`;
- minimum values are less than maximum values;
- `step_km` is inside the supported range;
- the estimated initial-cell count is below the configured cap.

Invalid requests are rejected before browser startup.

### 11.2 Cell Geometry

Each cell stores true boundaries, center, and depth. Edge cells are clipped to the requested bbox. Cell centers may not lie outside the bbox.

The planner calculates the required initial-cell count before allocating the vector. If the count exceeds the safety limit, the response includes a recommended minimum step.

### 11.3 Default Safety Limits

- `max_depth = 6`
- `min_cell_size_m = 250`
- `max_initial_cells = 10_000`
- `max_total_cells = 25_000`

Reaching a configured safety budget pauses the job as `paused_limit_reached`; it does not discard pending units.

## 12. Adaptive Subdivision

A cell may subdivide only when all conditions hold:

1. the feed did not confirm `reached_end`, or the raw listing count reached saturation;
2. depth is below `max_depth`;
3. the cell is larger than `min_cell_size_m`;
4. child zoom is greater than parent zoom;
5. the total-unit safety budget permits four child units.

Saturation is based on raw Google Maps listings, not only businesses that expose a phone number.

If two consecutive subdivision levels add less than five percent new unique places, stop subdividing that branch.

## 13. Spatial Filtering and Deduplication

Within a job, deduplication precedence is:

1. `place_id`
2. canonical Google Maps URL
3. normalized phone
4. normalized name and address

Results with known coordinates outside the requested bbox are stored but do not affect saturation and are not imported as job-qualified Leads.

Results without coordinates remain auditable and may be imported only when their textual location matches the requested location strongly enough under a deterministic rule covered by tests.

## 14. Email Enrichment

Run enrichment only when a result lacks a valid Vietnamese mobile number and has an eligible website.

Visit, in order:

1. homepage;
2. contact or `lien-he` page;
3. about or `gioi-thieu` page;
4. up to two additional relevant same-domain links.

Choose the first valid email encountered:

1. `mailto:` addresses;
2. visible HTML text.

Do not prioritize role accounts over personal accounts. Store one primary email.

### 14.1 SSRF and Resource Limits

- Allow only HTTP and HTTPS.
- Reject credentials in URLs.
- Reject internal, loopback, private, link-local, multicast, metadata, and disallowed IP destinations.
- Resolve and validate DNS before every connection.
- Revalidate every redirect.
- Stay on the same registrable domain.
- Limit redirects to three.
- Fetch HTML only.
- Limit each response to 2 MB.
- Use a five-second connect timeout and fifteen-second total request timeout.
- Do not execute website JavaScript.
- Respect `robots.txt` disallow rules.

Enrichment failures are persisted. A result with a valid mobile remains qualified. An unresolved enrichment failure for a result without mobile causes a `partial` job outcome and can be retried without repeating Google Maps scraping.

## 15. Lead Import

Lead qualification requires either:

- a valid Vietnamese mobile number; or
- a valid normalized email.

Import behavior:

1. normalize phone and email;
2. search existing Leads by normalized phone;
3. search existing Leads by case-insensitive normalized email;
4. if either matches, mark the scrape result duplicate;
5. otherwise insert the Lead;
6. mark the result imported only after the insert succeeds.

The import stage runs transactionally. SQL preparation, lookup, insert, or commit errors fail the stage and never emit completion.

Add indexes supporting normalized email and phone lookups.

## 16. Tauri Commands

New commands:

- `prepare_google_maps_runtime`
- `create_google_maps_scrape_job`
- `pause_google_maps_scrape_job`
- `resume_google_maps_scrape_job`
- `cancel_google_maps_scrape_job`
- `retry_google_maps_failed_units`
- `retry_google_maps_email_enrichment`
- `get_google_maps_scrape_job`
- `list_google_maps_scrape_jobs`
- `delete_google_maps_scrape_job`

Existing `start_google_maps_scrape` and `stop_google_maps_scrape` commands remain temporary compatibility wrappers and are marked deprecated.

## 17. Event Contract

Every scraper event contains:

- `job_id`
- monotonic `sequence`
- timestamp
- status
- stage
- processed and total units
- percentage
- message
- optional typed error or action requirement

The frontend ignores events for other jobs and stale sequence values. On reload, the UI restores state from SQLite rather than depending on missed events.

## 18. User Interface

Keep the current preset, geocode, editable bbox, and step controls.

Add:

- CloakBrowser runtime status and download retry;
- preflight initial-cell estimate and validation warnings;
- active-job stage and unit counters;
- Pause, Resume, Cancel, Retry Failed Units, and Retry Email Enrichment actions;
- CAPTCHA action-required panel;
- interrupted-job Resume action after application restart;
- job history and result summary;
- explicit completed, partial, failed, cancelled, and interrupted states.

## 19. Observability and Retention

- Use structured logging with job, unit, attempt, stage, and error kind.
- Rotate log files in application data.
- Capture a screenshot and URL for CAPTCHA and selector failures.
- Do not log license keys, cookies, local storage, full HTML, or sensitive browser-profile data.
- Retain job history and scrape results for thirty days by default.
- Retain terminal browser profiles for seven days by default.
- Retain CAPTCHA or diagnostic profiles until the user resolves or deletes them.

## 20. Testing Strategy

### 20.1 Unit Tests

- bbox and step validation;
- safe initial-cell estimation;
- clipped edge cells;
- zoom progression;
- subdivision limits;
- low-new-result branch termination;
- typed error mapping;
- phone and email normalization;
- dedup precedence;
- URL and redirect SSRF rejection;
- email extraction and ordering;
- state-machine transitions.

### 20.2 Fault-Injection Tests

- browser disconnect during initial navigation;
- disconnect during feed scroll;
- disconnect while visiting place details;
- disconnect during deep scan;
- browser startup timeout;
- runtime download failure;
- CAPTCHA pause and same-profile resume;
- application interruption and checkpoint resume;
- database failure during result checkpoint;
- database failure during Lead import;
- pause, resume, and cancel races;
- double-start prevention;
- stale and cross-job event rejection.

### 20.3 Integration Tests

- mock Google Maps pages for keyword, deep scan, saturation, subdivision, and selector failure;
- mock websites for email enrichment and SSRF redirect chains;
- real CloakBrowser Linux smoke test;
- runtime download/cache/reuse test;
- process cleanup test ensuring no browser remains after terminal states.

### 20.4 Cross-Platform Gates

- build `cloak-runtime` and Tauri for all supported target triples;
- run platform-specific process lifecycle tests on Linux, Windows, and macOS runners;
- run a real launch smoke test on each OS before claiming that OS verified;
- test macOS Gatekeeper behavior on a real notarized release candidate.

## 21. Release Gates

The module is production-ready only when all are true:

- formatter, linter, compile, test, and build gates pass;
- no Google Maps scraper error is silently converted to an empty success;
- grid and deep scan fail fast on fatal browser errors;
- invalid grid input cannot loop indefinitely or allocate unbounded cells;
- crash/restart resumes the exact pending checkpoint;
- CAPTCHA uses the same persistent profile and resumes correctly;
- browser processes close on every terminal path;
- database failures never emit completion;
- phone/email Lead import is idempotent;
- email enrichment passes SSRF and resource-limit tests;
- event ordering and job isolation are proven;
- GitNexus change impact is reviewed;
- supported operating systems have fresh runtime evidence.

## 22. Rollout Plan

1. Introduce new tables and job repository without changing the UI.
2. Add typed grid planner and scraper contracts behind tests.
3. Add the CloakBrowser runtime sidecar and Linux smoke tests.
4. Add the job runner and checkpoint execution.
5. Add email enrichment and Lead import.
6. Migrate the UI to job-scoped commands/events.
7. Keep compatibility wrappers for one release.
8. Run cross-platform build and runtime verification.
9. Remove legacy fire-and-forget implementation after migration evidence passes.

## 23. Known External Dependency

RustSale does not currently have a CloakBrowser OEM redistribution license. The approved design therefore downloads the browser runtime from the official channel on first use and does not embed the Chromium binary in the installer.

The exact CloakBrowser wrapper and runtime version will be pinned during implementation after compatibility testing. Updating either version requires regression tests and rollback capability.
