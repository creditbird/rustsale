# Molecular Production Audit & Remediation Evidence Ledger

## Metadata

- Date: `2026-10-19`
- Status: `active`
- Plan: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-plan.md`
- Evidence: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-evidence.md`
- Benchmark: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-benchmark.md`
- Actual status: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-actual-status.md`

## Evidence Sections

### E0: Baseline Inspection Evidence

- `E0-P0A-GRAPH`: Ran `anvien analyze --force` and `bunx gitnexus analyze`. Both semantic graphs refreshed cleanly.
- `E0-P0A-RUST1`: Inspected `src-tauri/src/commands.rs`, `src-tauri/src/db.rs`, `src-tauri/tauri.conf.json`, and `src-tauri/capabilities/default.json`. Found `install_portable_linux_update` (lines 7753-8010) lacking strict `clean_version` character validation and executing blocking `std::fs`/`std::process::Command` tar extraction on the Tokio async thread; found `download_lead_import_template` (lines 7694-7749) writing directly via `tokio::fs::write` without `ParentDir` (`..`) rejection, extension validation, or atomic symlink-safe file open.
- `E0-P0A-SVELTE1`: Inspected `src/routes/+page.svelte` and `src/lib/components/LeadsList.svelte`. Found async `initDesktopNotificationBridge` unmount leak in `+page.svelte:114-121`, inline `.filter()` on leads in `+page.svelte:240-242`, unwired header Quick Search input and Notifications button in `+page.svelte:190-209`, and missing `lead.phone` matching in `LeadsList.svelte:74-84`.
- `E0-P0A-SVELTE2`: Executed `bun run check` (`svelte-check`). Found 0 errors and 6 warnings across 3 files: `LeadDetails.svelte:55` (`state_referenced_locally`), `Dashboard.svelte:63,65,66` (`state_referenced_locally`), and `LicenseModal.svelte:578,587` (`a11y_label_has_associated_control`). Also found `LicenseModal.svelte:95-98` missing `onMount` cleanup for `stopPaymentPolling()`.
- `E0-P0A-SVELTE3`: Executed `svelte-autofixer` via Svelte MCP across all 52 `.svelte` components in `src/routes/` and `src/lib/components/`. Found critical placeholder interpolation bug in `TemplateCenter.svelte:962` (`placeholder="VD: {{name}} ơi..."` parsed as `{ { name } }`), unnecessary `{' '}` in `TemplateCenter.svelte:1160` and `TemplatePickerModal.svelte:481`, and unkeyed `{#each}` blocks.
- `E0-P0A-SVELTE4`: Inspected `src/lib/components/facebook/FacebookHub.svelte`. Found line 800 calling `new URL(postLinkUrl).hostname` directly inside template expression without `try/catch`, which throws `TypeError: Invalid URL` during typing of a link post URL.
- `E0-P0A-SVELTE5`: `svelte-autofixer` across all 52 `.svelte` files identified unkeyed `{#each}` blocks across 16 components, unused `svelte-ignore` comments in `MessageReactionPicker.svelte` and `UserProfilePopover.svelte`, and `$state` + `$effect` replaceable by `$derived` in `ChannelBadges.svelte`.

### E1: Backend Security & Async Runtime Hardening Evidence (`P1-A`)

- `E1-P1A-ANVIEN`: Ran `anvien file-detail src-tauri/src/commands.rs --json` and `anvien impact file src-tauri/src/commands.rs --direction upstream` before editing.
- `E1-P1A-RUST-FIX`: Hardened `download_lead_import_template` with `ParentDir` (`..`) rejection, `.xlsx`/`.csv` extension validation, and `open_export_file_unix` / `open_export_file_windows` inside `tokio::task::spawn_blocking`. Hardened `install_portable_linux_update` with `is_valid_release_version` validation and moved all blocking `std::fs` and `std::process::Command` tar extraction/installation into `tokio::task::spawn_blocking`. Added unit test `security_audit_tests::validates_release_version_strictly` in `src-tauri/src/commands.rs`.
- `E1-P1A-OUTBOX`: Replaced blocking `std::fs::read` inside `process_next_zalo_outbox_job` and `process_next_messenger_outbox_job` (`src-tauri/src/conversation/service.rs`) with async helper `read_outbox_attachment_bytes` enforcing `ParentDir` (`..`) rejection, `tokio::fs::symlink_metadata` regular-file verification, and a 50 MiB (`MAX_ATTACHMENT_BYTES`) limit before `tokio::fs::read(target_path).await`.

### E2: Svelte 5 & UI Molecular Remediation Evidence (`P2-A`, `P2-B`)

- `E2-P2A-ANVIEN`: Ran `anvien file-detail` and `anvien impact` across all target `.svelte`, `.ts`, and `.rs` files before editing.
- `E2-P2A-CORE`: Fixed `+page.svelte` (wired Quick Search input & Bell button, memoized `activeLeads` and `convertedContacts` via `$derived`, guarded `initDesktopNotificationBridge` cleanup on unmount), `LeadsList.svelte` (`searchQuery = $bindable('')`, `lead.phone` search matching, and multi-space initials via `.trim().split(/\s+/).filter(Boolean)`), `LeadDetails.svelte` (`untrack(() => initialLead)`, 8 keyed `{#each}` blocks, `getLeadInitials`, `hasValidEmail` guard for `mailto:` links and `openSendEmailModal`, normalized `editEmail` in `startEditing()`, optional `#edit-email` in `handleSaveLead`), `Dashboard.svelte` (resolved all 3 `state_referenced_locally` warnings and keyed all 7 `{#each}` blocks), and `LicenseModal.svelte` (`stopPaymentPolling()` unmount cleanup, associated `<label for>` / `<input id>`, and keyed `availablePlans`).
- `E2-P2B-COMPONENTS`: Fixed `TemplateCenter.svelte` (`placeholder={'VD: {{name}} ơi...'}` and 9 keyed `{#each}` blocks), `FacebookHub.svelte` (safe `getPostLinkHostname(postLinkUrl)` helper with `try/catch` around `new URL()` and 6 keyed `{#each}` blocks), `ChannelBadges.svelte` (writable `$derived([...activeChannels])`), `MessageReactionPicker.svelte` & `UserProfilePopover.svelte` (removed unused `svelte-ignore`), and keyed all `{#each}` blocks across `TemplatePickerModal.svelte`, `SaveTemplateModal.svelte`, `KanbanBoard.svelte`, `LeadImportModal.svelte`, `WhatsAppOutreach.svelte`, `CampaignCenter.svelte`, `SequenceBuilder.svelte`, `FacebookAccountsSettings.svelte`, `LanguageSettings.svelte`, `AICopilotBar.svelte`, and `GroupMembersModal.svelte`.
- `E2-P2B-PASS2`: Fixed `VoiceRecorderBar.svelte` (`unmounted` guard stopping `MediaStreamTrack`s when `getUserMedia` resolves after unmount/cancel), `ZaloAccountsSettings.svelte` & `WhatsAppAccountsSettings.svelte` (`registerQrListener` session sequence/attempt guard preventing in-flight `await listen(...)` leaks on unmount/dialog close, plus immutable `Set` reassignment for `disconnecting` in `WhatsAppAccountsSettings.svelte`), `FacebookAccountsSettings.svelte` (`openExternalUrl('https://developers.facebook.com/apps')` click handler), and `messageMedia.ts` ($O(N)$ `Set<string>` deduplication in `extractGalleryImages`).
- `E2-P2B-REDGREEN`: Created `src/lib/components/__tests__/MolecularAuditPass2.spec.ts`, verified all 3 regression tests failed (RED) before Pass 2 fixes, and verified all 3 passed (GREEN) after fixes.
- `E2-P2B-AUTOFIXER`: Verified modified components with `mcp_svelte_svelte-autofixer` (`desired_svelte_version: 5`) returning `issues: []`.

### E3: Full Build, Change Detection & Verification Harness Evidence (`P3-A`)

- `E3-P3A-DETECT`: Executed `anvien analyze --force && anvien detect-changes --scope all`.
- `E3-P3A-VERIFY`: Executed `bun --no-env-file run agent:verify 4f6e43cd-31a9-4a0c-a353-00778c1f3724` covering `check` (`svelte-check --threshold warning`), `test:unit`, `build`, `rust:check`, `rust:clippy`, `rust:test`, `rust-security-audit-tests`, `svelte-check-zero-warnings`, and `task-frontend-unit-tests`.
