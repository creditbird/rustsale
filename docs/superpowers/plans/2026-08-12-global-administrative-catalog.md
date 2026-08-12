# Global Administrative Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace incomplete hard-coded location data with an all-country, refreshable GeoNames administrative catalog while preserving the existing Google Maps scraping pipeline.

**Architecture:** A Rust module downloads and parses one GeoNames country dump on demand into a dedicated SQLite cache. Typed Tauri commands expose countries, cache state, hierarchy children, and search to a reusable Svelte selector; `GoogleMapsScraper.svelte` only maps selected canonical paths into its existing `districts` request field.

**Tech Stack:** Rust, Tauri 2, reqwest, zip, rusqlite, serde, Svelte 5, TypeScript, Bun test.

---

### Task 1: Bundle the country catalog

**Files:**
- Create: `src-tauri/resources/geonames-country-info.txt`
- Create: `src-tauri/src/administrative_catalog/countries.rs`
- Create: `src-tauri/src/administrative_catalog/types.rs`
- Test: `src-tauri/src/administrative_catalog/countries.rs`

- [ ] Add a parser test requiring at least 249 ISO-coded countries and representative entries.
- [ ] Run `cargo test --lib administrative_catalog::countries` and confirm it fails because the module does not exist.
- [ ] Save the current GeoNames `countryInfo.txt` snapshot and implement deterministic parsing and continent grouping.
- [ ] Run the targeted test and confirm it passes.

### Task 2: Parse GeoNames country dumps

**Files:**
- Create: `src-tauri/src/administrative_catalog/parser.rs`
- Modify: `src-tauri/src/administrative_catalog/types.rs`
- Test: `src-tauri/src/administrative_catalog/parser.rs`

- [ ] Add fixtures covering ADM1–ADM5, PPL records, aliases, malformed rows, historical records, and missing parent codes.
- [ ] Run the parser tests and confirm the new cases fail.
- [ ] Implement bounded TSV parsing, filtering, parent lookup, and canonical paths without file or network I/O.
- [ ] Run the parser tests and confirm they pass.

### Task 3: Add the transactional cache

**Files:**
- Create: `src-tauri/src/administrative_catalog/repository.rs`
- Test: `src-tauri/src/administrative_catalog/repository.rs`

- [ ] Add repository tests for schema initialization, country replacement, hierarchy listing, search, and preservation after a failed refresh.
- [ ] Run repository tests and confirm they fail.
- [ ] Implement the dedicated SQLite schema and transactional operations.
- [ ] Run repository tests and confirm they pass.

### Task 4: Add download service and Tauri commands

**Files:**
- Create: `src-tauri/src/administrative_catalog/service.rs`
- Create: `src-tauri/src/administrative_catalog/commands.rs`
- Create: `src-tauri/src/administrative_catalog/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/src/administrative_catalog/service.rs`

- [ ] Add fake-downloader tests for cache reuse, force refresh, invalid country, ZIP limits, and preserving ready data on refresh failure.
- [ ] Run service tests and confirm they fail.
- [ ] Implement bounded streaming download, ZIP extraction, service orchestration, managed state, and Tauri commands.
- [ ] Run administrative-catalog tests and confirm they pass.

### Task 5: Add the typed frontend catalog service

**Files:**
- Create: `src/lib/services/administrativeCatalogService.ts`
- Create: `src/lib/services/__tests__/administrativeCatalogService.spec.ts`

- [ ] Add invoke-contract tests for all catalog commands.
- [ ] Run the single Bun test and confirm it fails.
- [ ] Implement the DTOs and typed wrappers.
- [ ] Run the single Bun test and confirm it passes.

### Task 6: Build the administrative selector

**Files:**
- Create: `src/lib/components/AdministrativeAreaSelector.svelte`
- Create: `src/lib/components/__tests__/AdministrativeAreaSelector.spec.ts`

- [ ] Add component tests for country coverage, lazy loading, hierarchy navigation, search selection, refresh, errors, attribution, and the 500-target cap.
- [ ] Run the component test and confirm it fails.
- [ ] Implement the selector using installed shadcn-svelte primitives and existing design tokens.
- [ ] Run the component test and `bun run check` until both pass.

### Task 7: Integrate without changing scraper behavior

**Files:**
- Modify: `src/lib/components/GoogleMapsScraper.svelte`
- Modify: `src/lib/components/__tests__/GoogleMapsScraper.spec.ts`
- Delete: `src/lib/constants/world_locations.ts`
- Delete: `src/lib/constants/vietnam-divisions.json`

- [ ] Add a regression test asserting selected canonical paths become the unchanged `districts` request field.
- [ ] Run the Google Maps component test and confirm the regression test fails.
- [ ] Replace the hard-coded selectors with `AdministrativeAreaSelector`; retain free-form keyword mode and existing grid controls.
- [ ] Run the Google Maps component test and confirm it passes.

### Task 8: Review, verify, and commit

**Files:**
- Modify: any files required by review findings within scope.

- [ ] Run `cargo fmt --all -- --check`, targeted Rust tests, and `cargo clippy --all-targets --all-features -- -D warnings` after checking disk space.
- [ ] Run `bun run check`, targeted frontend tests, full `bun test`, `cargo test --lib google_maps_jobs`, and `bun run build`.
- [ ] Run GitNexus change detection and review all affected symbols and flows.
- [ ] Perform separate standards and specification code review; fix all blocking findings and rerun invalidated gates.
- [ ] Verify `git diff --check`, remove obsolete artifacts, stage only task files, and commit with `feat: add global administrative catalog`.
