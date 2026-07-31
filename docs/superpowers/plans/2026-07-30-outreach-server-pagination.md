# Outreach Server Pagination Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate multi-second Outreach tab stalls by paging matching leads in SQLite, bounding rendered rows, preserving realtime refresh, and resolving all-matching campaign selections in the backend.

**Architecture:** Add non-breaking Tauri commands for cursor-based lead pages and ID snapshot resolution. Svelte components keep page cursor history and cache, render 50 records, and represent selection as all-matching plus exclusions or explicit IDs.

**Tech Stack:** Rust, rusqlite, Tauri 2, Svelte 5 runes, Bun tests.

---

### Task 1: Backend Pagination Contract

**Files:** `src-tauri/src/commands.rs`, `src-tauri/src/commands_test.rs`, `src-tauri/src/lib.rs`

- [x] Add failing Rust tests for 50-row pages, cursor ordering, total counts, search/filter behavior, and excluded-ID resolution.
- [x] Add `OutreachLeadsPage`, shared query construction, `get_outreach_leads_page`, and `resolve_outreach_lead_ids`.
- [x] Register both commands without changing existing command signatures.
- [x] Run targeted Rust tests and confirm GREEN.

### Task 2: Frontend Service Contract

**Files:** `src/lib/services/crmService.ts`, `src/lib/services/__tests__/crmService.spec.ts`

- [x] Add typed page/filter request interfaces and Tauri wrappers.
- [x] Add tests for exact command names and camelCase payloads.
- [x] Run targeted service tests and confirm GREEN.

### Task 3: Zalo Paginated Selector

**Files:** `src/lib/components/ZaloOutreach.svelte`, `src/lib/components/__tests__/ZaloOutreach.spec.ts`

- [x] Add RED tests proving large datasets render 50 rows and verification is manual.
- [x] Replace full lead loading with cursor pages, page cache, and all-matching selection.
- [x] Resolve campaign IDs immediately before launch.
- [x] Preserve CRM-only realtime refresh and retry behavior.
- [x] Run targeted Zalo tests and confirm GREEN.

### Task 4: Email Paginated Selector

**Files:** `src/lib/components/EmailOutreach.svelte`, `src/lib/components/__tests__/EmailOutreach.spec.ts`

- [x] Add RED tests proving large datasets render 50 rows.
- [x] Replace full lead loading with cursor pages, page cache, and all-matching selection.
- [x] Resolve campaign IDs immediately before launch.
- [x] Preserve CRM-only realtime refresh and retry behavior.
- [x] Run targeted Email tests and confirm GREEN.

### Task 5: Performance and Release QA

**Files:** all modified files

- [x] Benchmark both tabs with 1,594 mocked leads and record long tasks.
- [x] Run `bun run test`, `bun run check`, `bun run build`, targeted Rust tests, and `git diff --check`.
- [x] Run `gitnexus_detect_changes({scope: "all"})` and review the expected scope.
