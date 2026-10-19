# Molecular Production Audit & Remediation Actual Status

## Metadata

- Date: `2026-10-19`
- Status: `active`
- Plan: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-plan.md`
- Evidence: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-evidence.md`
- Benchmark: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-benchmark.md`
- Actual status: `docs/plans/2026-10-19-molecular-production-audit-remediation/2026-10-19-molecular-production-audit-remediation-actual-status.md`

## Current Status Matrix

| Surface / Target | Reality Classification | Touch Mode | Evidence ID | Plan Consequence |
| :--- | :--- | :--- | :--- | :--- |
| `src-tauri/src/commands.rs` & `src-tauri/src/conversation/service.rs` | `implemented` | verified | `E1-P1A-RUST-FIX`, `E1-P1A-OUTBOX` | Strict version validation, `spawn_blocking` extraction, traversal/symlink-safe template export, and async `read_outbox_attachment_bytes` verified |
| `src/routes/+page.svelte` & `src/lib/components/LeadsList.svelte` | `implemented` | verified | `E2-P2A-CORE` | Async notification bridge unmount guard, `$derived` lead lists, wired header search/notification controls, phone search matching, and multi-space initials verified |
| `src/lib/components/LeadDetails.svelte`, `Dashboard.svelte`, `LicenseModal.svelte` | `implemented` | verified | `E2-P2A-CORE` | 0 `svelte-check` warnings, keyed `{#each}` blocks, `hasValidEmail` / `'No Email'` handling, and `LicenseModal` `pollTimer` unmount cleanup verified |
| `src/lib/components/template/TemplateCenter.svelte`, `TemplatePickerModal.svelte`, `SaveTemplateModal.svelte` | `implemented` | verified | `E2-P2B-COMPONENTS` | `TemplateCenter.svelte` `{{name}}` placeholder fixed and all `{#each}` blocks keyed |
| `src/lib/components/facebook/FacebookHub.svelte` | `implemented` | verified | `E2-P2B-COMPONENTS` | Safe `getPostLinkHostname(postLinkUrl)` helper prevents `TypeError` on partial URL input; all `{#each}` blocks keyed |
| `VoiceRecorderBar.svelte`, `ZaloAccountsSettings.svelte`, `WhatsAppAccountsSettings.svelte`, `FacebookAccountsSettings.svelte`, `messageMedia.ts` | `implemented` | verified | `E2-P2B-PASS2`, `E2-P2B-REDGREEN` | Pending `getUserMedia` mic track cleanup, in-flight `listen()` unlisten race guards, external URL click handler, and $O(N)$ gallery deduplication verified with `MolecularAuditPass2.spec.ts` |
| Remaining 11 Svelte components flagged by `svelte-autofixer` | `implemented` | verified | `E2-P2B-AUTOFIXER` | All `{#each}` blocks keyed, unused `svelte-ignore` comments removed, and `$derived` used in `ChannelBadges.svelte` |

## Status Refresh Log

| Refresh ID | Timestamp | Surface | Previous State | New State | Evidence Ref | Action Taken |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| SR-01 | 2026-10-19T22:28:00+07:00 | Full Repo Audit | `unknown` | `partial` | `E0-P0A-RUST1` .. `E0-P0A-SVELTE5` | Completed 100% `svelte-autofixer` scan (52 files), `svelte-check`, `cargo clippy`, and molecular source inspection |
| SR-02 | 2026-10-19T23:08:30+07:00 | Pass 1 Target Files (22 files) | `partial` / `broken-edge` | `implemented` | `E1-P1A-RUST-FIX`, `E2-P2A-CORE`, `E2-P2B-COMPONENTS`, `E3-P3A-VERIFY` | Remediated Rust security/async blocking issues, Svelte 5 warnings, UI crashes/leaks, and `svelte-autofixer` findings |
| SR-03 | 2026-10-20T05:40:00+07:00 | Pass 2 Molecular Audit (7 files + regression test suite) | `broken-edge` | `implemented` | `E1-P1A-OUTBOX`, `E2-P2B-PASS2`, `E2-P2B-REDGREEN`, `E3-P3A-VERIFY` | Fixed `VoiceRecorderBar` mic leak, `Zalo`/`WhatsApp` QR `listen()` race, `LeadDetails` `'No Email'`/initials edge cases, outbox async attachment reader, and verified with red->green tests |
