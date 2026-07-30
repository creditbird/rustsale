# Google Maps Realtime Counters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep Discovered, Qualified, Leads, and failed-unit counters synchronized while a Google Maps scrape job is running.

**Architecture:** Extend the existing Rust-to-Tauri `ScrapeJobEvent` contract with the counters already stored on `ScrapeJob`. Mirror those fields in the TypeScript interface and merge them into the active job snapshot when a newer event is accepted. Keep the existing durable job database and event sequencing unchanged.

**Tech Stack:** Rust, Serde, Tauri events, TypeScript, Svelte 5, Bun test.

---

### Task 1: Add Regression Coverage

**Files:**
- Modify: `src-tauri/src/google_maps_jobs/types.rs:194`
- Modify: `src/lib/components/__tests__/GoogleMapsScraper.spec.ts:329`

- [x] Add `failed_units`, `businesses_discovered`, `businesses_qualified`, and `leads_created` to the Rust round-trip event fixture.
- [x] Add a component test that starts with zero counters, emits a newer event containing non-zero counters, and asserts the job panel renders the new values without reloading.
- [x] Run `bun test --conditions=browser --conditions=svelte src/lib/components/__tests__/GoogleMapsScraper.spec.ts` and `cargo test google_maps_job_request_and_event_round_trip --manifest-path src-tauri/Cargo.toml` to confirm the tests fail before production changes.

### Task 2: Extend the Realtime Contract

**Files:**
- Modify: `src-tauri/src/google_maps_jobs/types.rs:194`
- Modify: `src-tauri/src/google_maps_jobs/events.rs:24`
- Modify: `src/lib/services/googleMapsScraperService.ts:82`
- Modify: `src/lib/components/GoogleMapsScraper.svelte:404`

- [x] Add the four counters to `ScrapeJobEvent` and populate them from the checkpointed `ScrapeJob` in `publish_job`.
- [x] Add matching required fields to `GoogleMapsJobEvent`.
- [x] Merge the event counters into `activeJob` only after the existing job-ID, sequence, and terminal-state guards accept the event.
- [x] Update existing typed event fixtures to include the required fields.

### Task 3: Verify the Fix

**Files:**
- Test: `src-tauri/src/google_maps_jobs/types.rs`
- Test: `src/lib/components/__tests__/GoogleMapsScraper.spec.ts`
- Test: `src/lib/services/__tests__/googleMapsScraperService.spec.ts`

- [x] Run the targeted Rust and Bun tests and confirm zero failures.
- [x] Run `bun run check`, `cargo fmt --check --manifest-path src-tauri/Cargo.toml`, and focused Google Maps tests.
- [x] Run `git diff --check` and GitNexus `detect_changes` to confirm only the event contract and its consumers changed.
