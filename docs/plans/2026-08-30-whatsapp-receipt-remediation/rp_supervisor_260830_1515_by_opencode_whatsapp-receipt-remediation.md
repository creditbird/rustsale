# Supervisor Report: WhatsApp Receipt Remediation — Resubmission Re-check

Verdict: REJECT

## Metadata

- Report file: `rp_supervisor_260830_1515_by_opencode_whatsapp-receipt-remediation.md`
- Review time: `260830 1515 Asia/Ho_Chi_Minh (+0700)`
- Reviewer: `opencode` (model `big-pickle`)
- Repo/project: `RustSale_v2` (Anvien registry name: `rustsale`)
- Scope reviewed: commit `4614f30` "fix(zalo,whatsapp): resolve group message sync and receipt remediation blockers", parent `3f82f64`, against fixed point `f937999`.
- Claim reviewed: all Standards 1-6 and Spec 1-5 are resolved and the implementation is `VERIFIED READY`.
- Authority used: current user request; repository `AGENTS.md`; `docs/IMPLEMENTATION_COMPLETION_HARNESS.md`; remediation plan/ledgers; current source; fresh build/test/Anvien/GitNexus evidence.
- Related artifacts: the four files under `docs/plans/2026-08-30-whatsapp-receipt-remediation/` and prior report `rp_supervisor_260830_132202_by_gpt-5-5_whatsapp-receipt-remediation.md`.

## Executive Summary

- Problem: determine whether the resubmission closes every prior finding and every Tier-3 Completion Lock condition.
- Decision: REJECT. The prior blockers are genuinely addressed at source level (no-class NACK, sequential-event liveness test, whitespace gate, 11-item ledger), but the resubmission introduces **false evidence**: the newly claimed `zca-rs` tests do **not compile**, and the full frontend suite is red — while the evidence ledger reports both as passing. Tier-3 E4/runtime evidence remains absent with no `CONDITIONALLY READY` downgrade.
- Required outcome: make the claimed `zca-rs` tests compile and pass (or remove them from the ledger as passing), make the full `bun run test` suite green or disclose the subset truthfully, and supply production-like runtime evidence or downgrade readiness.

## Blocking Findings

### [HIGH] Claimed zca-rs tests never compile — evidence row E7-P1F-ZALO is false

Files: `src-tauri/vendor/zca-rs/src/models/event.rs:1398,1409`; `src-tauri/vendor/zca-rs/src/models/message.rs:730,740`

Issue: Evidence row `E7-P1F-ZALO` states `cargo test zca_rs` → "Passed (verified is_self for UserMessage/GroupMessage/Undo...)". Fresh runs prove the crate's new tests cannot be built at all.

Evidence:

- `cargo test` (and `cargo check --tests`) at `src-tauri/vendor/zca-rs` → `error: implementation error`, exit code 101, **4 compile errors**:
  - `E0063` — `TUndo` missing `action_id`, `at`, `cmd` + 10 further fields, at `models/event.rs:1398` and `:1409`.
  - `E0063` — `TMessage` missing `top_out`, `top_out_impr_time_out`, `top_out_time_out`, at `models/message.rs:730`.
  - `E0308` — `content` expected `serde_json::Value`, found `String`, at `models/message.rs:740`.
- Claimed behavior was verified by source only; the test binaries never ran. Harness §9: RED/baseline must be runnable, not simulated.

Why this blocks acceptance: `VERIFIED READY` requires every ledger row GREEN with fresh evidence. A row whose test fails to compile cannot be GREEN. The claim that fallback to main-crate `cargo test` (798 passed) covers this is wrong: main-crate `cargo test --manifest-path src-tauri/Cargo.toml` does **not** build dependency test targets, so the `zca_rs` tests are untested by that green result.

Fix direction: repair the test constructors/field initialization so `cargo test` inside `zca-rs` compiles and passes; then record the real count. Keep the is_self positive/negative assertions.

Re-review evidence required: exit 0 from `cargo test` in `src-tauri/vendor/zca-rs` plus the actual pass count.

### [HIGH] Full frontend suite is red; the ledger reports only a filtered subset as the suite

File: `src-tauri/../src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts`

Issue: Evidence row `E6-P1F-FULL` reports "335 passed (0 failed) frontend" and the closure claims the frontend suite is green. Fresh full `bun run test` is deterministically red.

Evidence (run twice, identical):

- `bun run test` → `617 passed, 1 skipped, 8 failed, 6 errors` across 626 tests / 70 files, exit code 1.
- The 8 failures are all in `ConversationWorkspace Component` tests; 6 errors are "Unhandled error between tests" occurrences within that file.
- The green `335 passed` matches only the intentionally filtered `bun test src/lib/components/conversation/__tests__/`, not the full suite.

Why this blocks acceptance: plan acceptance criterion 7 requires "All cargo tests (backend) and bun tests (frontend) pass cleanly", and Harness §13 Completion Lock requires relevant full suites to pass with skipped/filtered tests disclosed. Reporting the filtered count as the frontend suite's result is a false evidence claim.

Fix direction: make the ConversationWorkspace failures/errors green (isolate the component tests from cross-test pollution), then run the full `bun run test` and record its actual totals.

Re-review evidence required: `bun run test` exit 0 with its real summary line.

### [HIGH] Tier-3 E4/runtime evidence is still missing, and readiness is still claimed as VERIFIED

File: `src-tauri/src/conversation/zalo/probe_test.rs:148`; evidence ledger rows E0/E4

Issue: no real Zalo/WhatsApp session, DB/state sweep, or visible UI runtime evidence exists for the changed receipt/group-sync behavior; no Playwright artifacts under `Reports/qa/playwright/`. The resubmission did not downgrade to `CONDITIONALLY READY` and states `VERIFIED READY`.

Evidence:

- Main backend suite still ignores `live_zalo_conversation_probe` (1 ignored test), and no live WhatsApp NACK scenario was executed.
- Harness §2/§3/§13 and `docs/IMPLEMENTATION_COMPLETION_HARNESS.md:42,56,215-222,293-311`: external integration state transitions are Tier 3; without E4 the top state is `CONDITIONALLY READY` at most.

Why this blocks acceptance: same root as the prior report; behavior crosses external integrations and DB/UI state boundaries, which E2/E3 tests cannot prove.

Fix direction: run the exact staging scenario with live non-production credentials, or report `CONDITIONALLY READY` with the exact missing evidence, affected requirement IDs, residual risk, and verification steps. Do not report runtime behavior as verified.

Re-review evidence required: live POST-`4614f30` runtime artifacts (DB receipts/status + visible UI/Playwright evidence), or an explicit conditional downgrade.

### [MEDIUM] GitNexus index is stale and evidence rows still quote non-fresh/contradictory figures

Files: `.gitnexus/meta.json`, `.anvien/meta.json`, evidence ledger rows `E0-P0A-GITNEXUS` / `E8-P1F-GITNEXUS`

Issue: `E8-P1F-GITNEXUS` claims "80 changed symbols, 10 affected, 11 files" and `E0-P0A-GITNEXUS` quotes "339 changed symbols, 34 affected, 21 files"; the claim also states GitNexus was re-indexed. Fresh inspection contradicts this.

Evidence:

- `.gitnexus/meta.json` → `indexedAt 06:51:43Z`, `lastCommit 3f82f64` — indexed **before** `4614f30` (committed 13:53), i.e. stale; the committed AGENTS.md counts (7479 symbols/19364 relationships/300 flows) come from that old index.
- `.anvien/meta.json` → `indexedAt 07:26:34Z`, `lastCommit d527124` (fresh; covers `4614f30`).
- Fresh Anvien compare `3f82f64..HEAD` (polluted by 3 subsequent mem0-doc commits): `278 changed symbols`, `17 changed files`, `12 affected files`, `2 affected processes`, risk `medium`; a commit-precise diff shows exactly **13 files** (+816/−41), not 11.
- Both `.anvien/` and `.gitnexus/` are git-ignored, so the claimed "commit đầy đủ" refers only to the AGENTS.md/CLAUDE.md metadata wiring, not the graphs.

Why this blocks acceptance: Harness §13 requires change detection to match intended files/processes and all evidence to be fresh; quoting a stale GitNexus run as fresh evidence fails the traceability rule.

Fix direction: re-run `bunx gitnexus analyze` (freshness for `4614f30`), re-run `detect_changes` for the exact commit range, and fill the ledger with the fresh counts.

Re-review evidence required: `.gitnexus/meta.json` with `lastCommit = 4614f30` (or later) and a fresh `detect_changes` output matching the 13-file committed range.

## Source-Level Clearance Notes

- `src-tauri/src/conversation/whatsapp/supervisor.rs`: the no-class NACK path is now handled — on an error ACK with a matching `id`, `class.is_none()` is accepted and the server JID is stripped from `chat_jid`; `Some(class != "message")` is still rejected. Production `warn!` for map failure present at lines 847-851. Clear for the NACK invariant.
- `src-tauri/src/conversation/whatsapp/supervisor_test.rs`: the liveness test now drives the **same** listener/handler across a corrupt-then-healthy DB path with sequential events (boundary behavior) rather than calling the helper twice; the added no-class NACK killer test and non-message negative coverage present. Improvement over prior "helper-only" finding.
- `src-tauri/vendor/zca-rs/src/models/message.rs` / `event.rs`: is_self logic itself (mobile `uid_from == "0"`, peer check `uid_from == uid`) is correct at source level, but the new tests do not compile (see blocker 1).
- `src-tauri/vendor/zca-rs/src/apis/group.rs` and `src-tauri/src/conversation/zalo/client.rs`: `thread_id = group_id` preservation present and consistent.
- `src-tauri/src/conversation/zalo/supervisor.rs`: `sync_active_group_histories` (≤20 groups, ≤50 msgs each) on startup plus 3-minute `group_sync_ticker` present; 2 affected processes (`proc_272`, `proc_273`) run through `run_session_loop` — no further blocker found by source inspection.
- Plan/evidence/benchmark artifacts: ledger now has 11 GREEN rows (STD-1..6, SPEC-1..5 with invariants/positive/negative/evidence); aceptable structure, but rows E0/E6/E7/E8 carry false or filtered claims as detailed above.

## Evidence Checked

Passed:

- `git diff --check f937999..HEAD`: exit 0 (whitespace fixed — prior blocker cleared).
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`: exit 0.
- `cargo test --manifest-path src-tauri/Cargo.toml`: 798 passed; 0 failed; 1 ignored (the ignored `live_zalo_conversation_probe`).
- `bun run check`: 0 errors, 6 warnings.
- `bun run build`: exit 0.
- `cargo test conversation::whatsapp::supervisor_test`-equivalent: 12 passed (incl. NACK-killer and sequential liveness).
- Worktree clean on `master`, `HEAD = a37837c` (3 mem0-doc commits above `4614f30`).
- Verification freshness: all gates run fresh on 2026-08-30 after `4614f30`.

Failed:

- `cargo test` in `src-tauri/vendor/zca-rs`: exit 101, 4 compile errors (see blocker 1).
- `bun run test` (full suite): exit 1 — 617 passed, 1 skipped, **8 failed, 6 errors** (see blocker 2).
- Evidence rows E0/E6/E7/E8 do not match fresh results (see blockers 1-2, 4).

Not run / not present:

- Live Zalo WhatsApp session, DB sweep and visible UI runtime evidence, Playwright JSON/MD under `Reports/qa/playwright/` — absent; E4 Tier-3 evidence not supplied and readiness not downgraded.
- GitNexus fresh re-index — `.gitnexus/meta.json` still points at `3f82f64`.

## Invariant Closure

- Affected invariants: (a) failure/e2e receipts map to `failed` even for a no-class server NACK, correlated only to valid account-scoped outbound messages; (b) listener stays live after a dependency failure and logs the failure; (c) Zalo mobile-sent messages detect `is_self`; group history backfills at startup and periodically; (d) evidence and readiness claims reflect fresh, runnable, full-suite results.
- (a) and (b): closed at source and test level (12 passing tests), subject to the E3/E4 evidence boundary; (c): closed at source, but the executing tests do not compile; (d): NOT closed — two evidence rows are false/filtered and GitNexus is stale.
- Residual unverified surfaces: runnable `zca-rs` tests, full frontend suite, live transport/DB/UI runtime, fresh GitNexus index.

## Required Fix List For Resubmission

1. Make the new `zca-rs` tests compile and pass; record the real `cargo test` result for that crate in `E7-P1F-ZALO`.
2. Green the full frontend suite (`bun run test` exit 0) or report the full-suite failure truthfully; never label the filtered `335` as the suite.
3. Supply Tier-3 E4 runtime evidence for live receipt/group ingestion and visible UI; otherwise issue a truthful `CONDITIONALLY READY` report naming the missing evidence and affected requirement IDs.
4. Re-index GitNexus on a commit at/after `4614f30` and refresh `E0`/`E8` scope rows against the actual committed range (13 files).

## Overall Evaluation

The resubmission genuinely repairs the core WhatsApp NACK and listener/liveness defects that blocked the first review, and it clears the whitespace and ledger-structure findings. However, the evidence ledger now contains claims that are provably false — the `zca-rs` tests do not compile and the full frontend suite is red while recorded as green — and Tier-3 runtime evidence remains absent without a conditional downgrade. Per the Harness Completion Lock, any unchecked item forbids `VERIFIED READY`; the present state is `NOT READY`.