# Outreach Tab Cache Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Zalo and Email Outreach tabs reuse cached data without duplicate IPC, retry failed initial loads, preserve the all-leads default, and prevent lead events from re-running Zalo network verification.

**Architecture:** Keep cache state local to each Svelte component. `$effect` is the sole lazy-load trigger; `onMount` only manages subscriptions. Loader functions return success status, CRM invalidations refresh only categories and leads, and Zalo session verification runs once after the first successful full load.

**Tech Stack:** Svelte 5 runes, Bun test runner, Testing Library Svelte, Tauri IPC.

---

### Task 1: Repair Component Test Isolation

**Files:**
- Modify: `src/lib/components/__tests__/Dashboard.spec.ts`
- Modify: `src/lib/components/__tests__/CategoryManager.spec.ts`

- [ ] Import `cleanup` from `@testing-library/svelte` and `afterEach` from `bun:test`.
- [ ] Register `afterEach(() => cleanup())` so mounted components, event subscribers, and timers do not survive into later test files.
- [ ] Run `bun run test` and confirm the existing four unhandled Dashboard errors disappear.

### Task 2: Add Zalo Cache Regression Tests

**Files:**
- Modify: `src/lib/components/__tests__/ZaloOutreach.spec.ts`

- [ ] Track counts for `get_categories`, `get_zalo_sessions`, `get_outreach_templates`, `get_leads`, `get_campaign_leads`, and `verify_all_zalo_sessions` in the Tauri mock.
- [ ] Replace the text-only clean-switch test with an IPC-count assertion.
- [ ] Add a test proving an active mount performs one base refresh.
- [ ] Add a test proving a failed initial session load retries after tab revisit.
- [ ] Add a test proving `notifyLeadsChanged()` refreshes CRM data without calling `verify_all_zalo_sessions` again.
- [ ] Add a test proving no selected category loads all leads.
- [ ] Run the Zalo test file and confirm the new tests fail for the expected duplicate-load, retry, verification-fanout, and lead-default reasons.

### Task 3: Add Email Cache Regression Tests

**Files:**
- Modify: `src/lib/components/__tests__/EmailOutreach.spec.ts`

- [ ] Add configurable category data and a one-shot SMTP load failure to the Tauri mock.
- [ ] Add a clean-switch test using IPC counts.
- [ ] Add a test proving an active mount performs one base refresh.
- [ ] Add a test proving a failed initial SMTP load retries after tab revisit.
- [ ] Add a test proving `notifyLeadsChanged()` refreshes categories/leads without reloading SMTP servers or templates.
- [ ] Add a test proving no selected category loads all leads rather than auto-selecting the first category.
- [ ] Run the Email test file and confirm the new tests fail for the expected reasons.

### Task 4: Harden Zalo Outreach Lifecycle

**Files:**
- Modify: `src/lib/components/ZaloOutreach.svelte`

- [ ] Make category, session, template, and lead loaders return `boolean` success values.
- [ ] Change `reloadLeads()` to call `getLeads()` when no category is selected and `getCampaignLeads()` when a category is selected.
- [ ] Guard the filter-driven lead effect with a non-reactive readiness flag so initial loading and clean tab switches do not cause duplicate fetches.
- [ ] Load categories, sessions, and templates in parallel, then load leads; set `hasLoaded` only when all required calls succeed.
- [ ] Run `verifyAllSessions()` once after the first successful full load.
- [ ] Add a CRM-only refresh path for `onLeadsChanged()` that reloads categories and leads but never sessions, templates, or verification.
- [ ] Remove the duplicate `refreshData()` call from `onMount`.
- [ ] Run the Zalo tests and confirm all pass.

### Task 5: Harden Email Outreach Lifecycle

**Files:**
- Modify: `src/lib/components/EmailOutreach.svelte`

- [ ] Make category, SMTP, template, and lead loaders return `boolean` success values.
- [ ] Stop auto-selecting the first category during load.
- [ ] Change `reloadLeads()` to call `getLeads()` when no category is selected and `getCampaignLeads()` when selected.
- [ ] Guard the filter-driven lead effect with a non-reactive readiness flag.
- [ ] Load categories, SMTP servers, and templates in parallel, then load leads; set `hasLoaded` only after complete success.
- [ ] Add a CRM-only invalidation path that does not reload SMTP servers or templates.
- [ ] Remove the duplicate `refreshData()` call from `onMount`.
- [ ] Run the Email tests and confirm all pass.

### Task 6: Verify the Integrated Change

**Files:**
- Verify all modified frontend and test files.

- [ ] Run `bun test --conditions=browser --conditions=svelte src/lib/components/__tests__/ZaloOutreach.spec.ts`.
- [ ] Run `bun test --conditions=browser --conditions=svelte src/lib/components/__tests__/EmailOutreach.spec.ts`.
- [ ] Run `bun run test` and require zero failures and zero unhandled errors.
- [ ] Run `bun run check` and require zero errors and zero warnings.
- [ ] Run `bun run build` and require exit code zero.
- [ ] Run `git diff --check`.
- [ ] Run GitNexus `detect_changes` and review the final affected scope.

No commits are created unless explicitly requested.
