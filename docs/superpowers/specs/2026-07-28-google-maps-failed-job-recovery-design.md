# Google Maps Failed Job Recovery Design

**Date:** 2026-07-28

## Goal

Recover the persisted Google Maps campaign shown in the production diagnostic without repeating completed work, prevent a transient blank browser page from becoming an unrecoverable selector failure, and create CRM leads from durable results before a multi-day crawl finishes.

## Evidence

- Job `gmaps-1785052851759-4fba6d373d614b40` stopped on grid unit `00935` after 935 completed units.
- The diagnostic screenshot is a completely blank page, not a rendered Google Maps page with changed selectors.
- The database contains 140 durable scrape results; 117 have a usable phone or email, but every result still has `import_status='pending'`.
- The current UI exposes retry only for `partial`; the current transition table rejects `failed -> queued`.
- The current user-retry transaction resets only selected infrastructure failures from `partial`, not `selector_changed` from `failed`.

## Design

### Blank-page recovery

The CloakBrowser health classifier gains an explicit `blank_page` state. Rust readiness polling treats it like a loading state during the normal readiness window. If the page remains blank when the window expires, the scraper returns retryable `transient_network`, causing the existing runner to close the browser, relaunch the persistent profile, and retry with backoff. Non-empty unknown Google Maps DOM remains `selector_changed`.

### Failed-job retry

User retry permits both `partial -> queued` and `failed -> queued`. The transition transaction requeues failed units with recoverable infrastructure errors or `selector_changed`, clears their terminal fields, decrements processed/failed counters by the exact reset count, and increments `run_generation`. Completed units and persisted results remain untouched.

The job panel shows `Retry Failed Units` for a failed job that has failed units. It uses the existing button, icon, callback, and styling.

### Incremental lead import

The importer gains a ready-only mode. It imports pending results only after email enrichment has reached a terminal state (`found`, `not_found`, `failed`, or `skipped_outside_area`). Results still awaiting enrichment remain pending so later email discovery is not lost.

After a completed unit checkpoints new results, the repository runs ready-only import and returns the refreshed job counters. Manual failed-unit retry also flushes older ready results before changing job status, which salvages the existing campaign immediately. Finalization retains the existing full pending-result import for remaining enriched results.

## Safety

- Import remains idempotent through result status plus phone/email duplicate checks.
- Checkpoint persistence completes before incremental import; a crash can delay import but cannot lose scraped data.
- User retry never resets completed units.
- True selector drift remains non-retryable during the active run, but becomes manually retryable after an application update.

## Acceptance Criteria

- A blank Google Maps page produces retryable `transient_network`, not terminal `selector_changed`.
- A failed selector unit can be requeued through the existing retry command.
- The failed job renders a retry action.
- Ready results create or deduplicate leads during scraping and when retrying an older failed job.
- Pending email-enrichment results are not imported early.
- Existing Rust, Bun, Svelte, and sidecar tests pass.
