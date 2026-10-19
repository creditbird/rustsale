# Molecular Production Audit & Remediation Plan

## Metadata

- Date: `2026-10-19`
- Status: `active`
- Plan: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-plan.md`
- Evidence: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-evidence.md`
- Benchmark: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-benchmark.md`
- Actual status: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-actual-status.md`

## Goal

Audit the entire `RustSale_v2` codebase at a molecular level across security, backend/UI performance, Svelte 5 MCP compliance (`svelte-autofixer`), and interactive controls/links, fixing every identified vulnerability, crash, memory/timer leak, warning, and DOM reconciliation issue until all checks pass with zero warnings/issues.

## Rules

- Complete P0 actual status before implementation work.
- Update each checklist item immediately when it is completed.
- Record evidence as work completes.
- Record benchmarkable counts or measurements when they are taken.
- Run Anvien `file-detail` + `impact` before editing files/symbols, and `detect-changes` before committing.

## Problem

1. **Backend Security & Async Runtime Blocking (`src-tauri/src/commands.rs`)**:
   - `install_portable_linux_update` does not validate `clean_version` characters before interpolating into `download_url` and temp paths, and runs blocking `std::fs` and `std::process::Command` tar extraction on the Tokio async thread.
   - `download_lead_import_template` writes directly via `tokio::fs::write` without rejecting `ParentDir` (`..`), checking `.xlsx`/`.csv` extension, or using the atomic symlink-safe `open_export_file_unix` / `open_export_file_windows` helpers.
2. **Critical UI Bugs & Runtime Crashes**:
   - `src/lib/components/template/TemplateCenter.svelte:962`: `placeholder="VD: {{name}} ơi..."` uses double quotes around `{{name}}`, causing Svelte to evaluate `{ { name } }` as `[object Object]`.
   - `src/lib/components/facebook/FacebookHub.svelte:800`: `{postLinkUrl ? new URL(postLinkUrl).hostname : 'EXAMPLE.COM'}` throws an uncaught `TypeError: Invalid URL` during template rendering whenever a user types a partial URL into the link input.
   - `src/routes/+page.svelte`: Top header Quick Search `<input>` and Bell `<button>` are unwired; `initDesktopNotificationBridge` promise resolution can leak its listener if the component unmounts before resolution; `leads.filter(...)` is recomputed inline instead of via `$derived`.
   - `src/lib/components/LeadsList.svelte`: Search bar filters by name, company, email, and owner, but omits `lead.phone`.
   - `src/lib/components/LicenseModal.svelte`: `onMount` does not return a cleanup function to call `stopPaymentPolling()`, leaking `pollTimer` on unmount.
3. **Svelte 5 Compiler Warnings & `svelte-autofixer` Issues**:
   - 6 `svelte-check` warnings across `LeadDetails.svelte`, `Dashboard.svelte`, and `LicenseModal.svelte`.
   - 68 unkeyed `{#each}` blocks across 16 Svelte components, 2 files with unused `svelte-ignore` comments (`MessageReactionPicker.svelte`, `UserProfilePopover.svelte`), and 1 `$state` + `$effect` synchronization in `ChannelBadges.svelte` replaceable by `$derived`.

## Scope

- In scope:
  - `src-tauri/src/commands.rs`
  - `src/routes/+page.svelte`
  - `src/lib/components/LeadsList.svelte`
  - `src/lib/components/LeadDetails.svelte`
  - `src/lib/components/Dashboard.svelte`
  - `src/lib/components/LicenseModal.svelte`
  - `src/lib/components/template/TemplateCenter.svelte`
  - `src/lib/components/template/TemplatePickerModal.svelte`
  - `src/lib/components/template/SaveTemplateModal.svelte`
  - `src/lib/components/facebook/FacebookHub.svelte`
  - `src/lib/components/KanbanBoard.svelte`
  - `src/lib/components/LeadImportModal.svelte`
  - `src/lib/components/WhatsAppOutreach.svelte`
  - `src/lib/components/campaign/CampaignCenter.svelte`
  - `src/lib/components/campaign/SequenceBuilder.svelte`
  - `src/lib/components/settings/FacebookAccountsSettings.svelte`
  - `src/lib/components/settings/LanguageSettings.svelte`
  - `src/lib/components/conversation/AICopilotBar.svelte`
  - `src/lib/components/conversation/GroupMembersModal.svelte`
  - `src/lib/components/conversation/MessageReactionPicker.svelte`
  - `src/lib/components/conversation/UserProfilePopover.svelte`
  - `src/lib/components/conversation/ChannelBadges.svelte`
  - `src/lib/components/conversation/VoiceRecorderBar.svelte`
  - `src/lib/components/conversation/messageMedia.ts`
  - `src/lib/components/settings/ZaloAccountsSettings.svelte`
  - `src/lib/components/settings/WhatsAppAccountsSettings.svelte`
  - `src-tauri/src/conversation/service.rs`
  - `src/lib/components/__tests__/MolecularAuditPass2.spec.ts`

## Requirements

- R1: Harden `install_portable_linux_update` and `download_lead_import_template` in `src-tauri/src/commands.rs` against version/path injection, symlink overwrites, and Tokio async thread blocking; harden `read_outbox_attachment_bytes` in `src-tauri/src/conversation/service.rs` with async `tokio::fs::read`, `ParentDir` (`..`) rejection, symlink rejection, and 50 MiB size cap.
- R2: Eliminate all 6 `svelte-check` warnings (`LeadDetails.svelte`, `Dashboard.svelte`, `LicenseModal.svelte`) so `bun run check` reports 0 errors and 0 warnings.
- R3: Fix molecular UI/functional bugs (`TemplateCenter.svelte` `{{name}}` placeholder bug, `FacebookHub.svelte` `new URL()` crash on partial URL, `+page.svelte` notification bridge leak & header search/bell wiring, `LeadsList.svelte` phone search & multi-space initials, `LeadDetails.svelte` multi-space initials & `'No Email'` handling, `LicenseModal.svelte` polling cleanup, `VoiceRecorderBar.svelte` pending `getUserMedia` hardware track leak, `ZaloAccountsSettings.svelte` & `WhatsAppAccountsSettings.svelte` in-flight `listen()` leak and `Set` reactivity, `FacebookAccountsSettings.svelte` external URL handler, and `messageMedia.ts` $O(N)$ `extractGalleryImages` deduplication).
- R4: Remediate all `svelte-autofixer` issues across all 19 flagged Svelte components (keying all `{#each}` blocks, removing unused `svelte-ignore` comments, and using writable `$derived` in `ChannelBadges.svelte`) and verify 0 issues via `mcp_svelte_svelte-autofixer`.
- R5: Pass all verification gates (`bun run check`, `bun test src`, `cargo test --manifest-path src-tauri/Cargo.toml`, `bun run build`, and `bun --no-env-file run agent:verify 4f6e43cd-31a9-4a0c-a353-00778c1f3724`).

## Acceptance Criteria

- Definition of done:
  - `bun run check` exits 0 with `0 errors and 0 warnings`.
  - `mcp_svelte_svelte-autofixer` returns `issues: []` and `suggestions: []` on every modified `.svelte` component.
  - `bun test src` (including `MolecularAuditPass2.spec.ts`) and `cargo test --manifest-path src-tauri/Cargo.toml` pass with 0 failures.
  - `bun --no-env-file run agent:verify 4f6e43cd-31a9-4a0c-a353-00778c1f3724` exits 0 with `PASS`.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: establish baseline evidence across Rust backend and all 52 Svelte components.
  - Work Steps: run `anvien analyze --force`, `bun run check`, `cargo clippy`, `svelte-autofixer` on all 52 `.svelte` files, and molecular code inspection.
  - Implementation Gate: `actual-status.md` completed.
  - Acceptance: all target surfaces classified in `actual-status.md`.

### P1: Backend Security & Async Runtime Hardening (`src-tauri/src/commands.rs`, `src-tauri/src/conversation/service.rs`)

- [x] P1-A: Harden `install_portable_linux_update`, `download_lead_import_template`, and `read_outbox_attachment_bytes`
  - Goal: prevent URL/path injection, symlink clobbering, unbounded attachment reads, and async executor blocking.
  - Work Steps:
    1. Run Anvien `file-detail` and `impact` on `src-tauri/src/commands.rs` and `src-tauri/src/conversation/service.rs`.
    2. Validate `clean_version` in `install_portable_linux_update` (ASCII digits/letters/dots/hyphens, starting with digit, max 32 chars) and move blocking `std::fs` + `tar` + binary swap into `tokio::task::spawn_blocking`.
    3. Validate `file_path` in `download_lead_import_template` (reject empty path, reject `ParentDir`, require `.xlsx` or `.csv` extension, require existing parent dir) and write via `open_export_file_unix` / `open_export_file_windows` inside `tokio::task::spawn_blocking`.
    4. Replace blocking `std::fs::read` in `process_next_zalo_outbox_job` and `process_next_messenger_outbox_job` (`src-tauri/src/conversation/service.rs`) with async `read_outbox_attachment_bytes` enforcing `ParentDir` rejection, symlink rejection, and 50 MiB limit.
  - Implementation Gate: `cargo check` and `cargo clippy` pass.
  - Acceptance: all Tauri commands and outbox workers enforce strict path/version validation and non-blocking file I/O.

### P2: Svelte 5 Compiler Warnings, Functional Bugs & `svelte-autofixer` Zero-Issue Remediation

- [x] P2-A: Fix core CRM & Workspace components (`+page.svelte`, `LeadsList.svelte`, `LeadDetails.svelte`, `Dashboard.svelte`, `LicenseModal.svelte`)
  - Goal: eliminate all 6 `svelte-check` warnings, fix lifecycle leaks, wire header controls, add phone search, normalize multi-space initials and `'No Email'` handling, and key all `{#each}` blocks.
  - Work Steps:
    1. Run Anvien `file-detail` and `impact` on target files.
    2. Update `+page.svelte`, `LeadsList.svelte`, `LeadDetails.svelte`, `Dashboard.svelte`, and `LicenseModal.svelte`.
    3. Verify with `mcp_svelte_svelte-autofixer`.
  - Implementation Gate: `svelte-autofixer` returns 0 issues for all 5 files.
  - Acceptance: `bun run check` reports 0 errors and 0 warnings.

- [x] P2-B: Fix Template, Facebook Hub, Kanban, Import, Outreach, Settings & Conversation components (Pass 1 & Pass 2)
  - Goal: fix `TemplateCenter.svelte` `{{name}}` bug, `FacebookHub.svelte` `new URL()` crash, `VoiceRecorderBar.svelte` mic track leak, `ZaloAccountsSettings.svelte` & `WhatsAppAccountsSettings.svelte` `listen()` race & `Set` reactivity, `FacebookAccountsSettings.svelte` external link, `messageMedia.ts` $O(N)$ gallery deduplication, and all `svelte-autofixer` findings across 18 components.
  - Work Steps:
    1. Run Anvien `file-detail` and `impact` on target files.
    2. Write red-capable regression tests in `src/lib/components/__tests__/MolecularAuditPass2.spec.ts`, confirm RED before fixes, apply fixes, and confirm GREEN.
    3. Re-run `mcp_svelte_svelte-autofixer` on every touched file to confirm 0 issues and 0 suggestions.
  - Implementation Gate: `mcp_svelte_svelte-autofixer` returns 0 issues across all touched files.
  - Acceptance: 100% of `.svelte` components pass `svelte-autofixer` and `bun run check` with 0 warnings/errors.

### P3: Full Build, Verification Loop & Change Detection

- [x] P3-A: Execute full verification harness and change detection
  - Goal: verify all unit/integration tests, production build, and `agent:verify` pass with exit 0.
  - Work Steps:
    1. Run `bun run check`, `bun test src`, `cargo test --manifest-path src-tauri/Cargo.toml`, and `bun run build`.
    2. Run `bun --no-env-file run agent:verify 4f6e43cd-31a9-4a0c-a353-00778c1f3724`.
    3. Run `anvien detect-changes --repo RustSale_v2 --scope all` and update plan ledgers.
  - Implementation Gate: `agent:verify` exits 0 with `PASS`.
  - Acceptance: all checks green and evidence recorded.
