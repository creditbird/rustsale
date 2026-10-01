# Supervisor Report: Zalo outreach resubmission

Verdict: REJECT

## Metadata
- Review time: 2026-09-08T13:14:20.280191+07:00
- Reviewer: gpt-6; repository RustSale_v2.
- Claim: all four prior review findings completely resolved; consistent SQL/preflight/worker/UI eligibility, propagated SQLite errors, both IPC commands protected, complete runtime/DOB verification and clean quality gates.
- Authority: user resubmission, repository AGENTS.md, supervisor skill, implementation completion harness; prior report `rp_supervisor_260908_121200_by_gpt-6_zalo_outreach_fix_review.md`.
- Scope: five existing uncommitted modified production/test files. Review did not modify these files or commit anything.
- Evidence directory: `evidence/zalo_outreach_260908_resubmission/`; current file hashes retained.

## Executive Summary
Preflight SQLite errors now propagate, both IPC commands invoke preflight, and all three DOB models are tested against both negative values. These closures are acknowledged. Eligibility still differs across layers, runtime invalid-phone writes still discard failures, and the new mock test does not verify IPC/progress/UI. Therefore the full-completion claim remains rejected.

## Blocking Findings

### [P2] A single digit passes preflight but fails the actual worker
File: `src-tauri/src/commands.rs:114`, `:2929`, `:3395`, `:3904`; `src/lib/components/ZaloOutreach.svelte:554`.
SQL, preflight and UI accept any ASCII digit in a phone. Worker requires is_vn_mobile, which normalizes then requires a ten-digit number with an allowed prefix. Thus phone `1` or `+84-90-123` with no UID is counted and partitioned, but always skipped later. This leaves the prior progress/session-allocation invariant open.
Evidence: `eligibility_probe.rs` embeds the verbatim production SQL and Rust functions; `probe.log` shows both inputs yield SQL=true, preflight=true, worker_target=false. This is isolated source/SQLite evidence, not live sending.
Fix direction: define and share the worker-compatible phone eligibility rule before count/resolve/partition; preserve valid UID fallback. Add a test that fails when a digit-containing invalid phone reaches a partition.

### [P2] SQL UID filtering still disagrees with the sanitizer
File: `src-tauri/src/commands.rs:114`, `:1204`, `:2930`.
SQL compares raw UID against sentinel strings without trimming. Empty phone + UID ` -1 ` passes SQL (contains 1, not equal to raw -1), but helper and worker trim it and reject it. Display/count/select-all remain inconsistent with campaign preflight.
The opposite discrepancy also exists at the current internal contract: UID `cached-uid` is rejected by SQL's nonzero-digit requirement but accepted by the sanitizer and UI. This second example demonstrates divergent predicates; it is not an assertion that an external Zalo account has an alphabetic UID.
Evidence: current production SQL/func probe: ` -1 ` => true/false/false; `cached-uid` => false/true/true.
Fix direction: choose one UID contract and apply normalization and sentinel handling consistently; test padded sentinels and permitted UID forms in page/count/resolve/preflight/UI.

### [P1] Runtime invalid-phone persistence still swallows errors
File: `src-tauri/src/commands.rs:3367`, `:3401`, `:3877`, `:3910`.
The helper's SELECT and UPDATE propagation is fixed, but both mock and real worker guards retain `let _ = conn.execute(...)` and silently ignore lock failure. This is part of the previous finding, not a newly expanded requirement.
Concrete reachable path: phone `1` passes preflight without a write even in a read-only database; worker rejects it and attempts INVALID_PHONE persistence, then discards the write failure. The worker continues and can reach a completion event rather than surfacing the database error. A later DB failure or phone edit after preflight has the same risk.
Evidence: verbatim invalid-phone guard executed against SQLite query_only=ON continues with stored status UNTESTED (`probe.log`). This probe reproduces the guard itself; full IPC error-event behavior was not run. Source shows identical discarded-result logic in the real worker.
Fix direction: propagate invalid-phone persistence/lock errors from worker and ensure asynchronous command wrappers surface failures to the UI, not silently lose returned errors. Test a digit-containing invalid phone with a failed worker write and assert the command/event outcome.

### [P2] Mock database evidence does not establish runtime progress/IPC/UI closure
File: `src-tauri/src/commands_test.rs:5312`, `:5454`, `:5474`.
The new test proves partition round-robin/dedup and mock-worker DB history/activity mutations. It manually calls helper and partition, then worker with None app handles, so all event-emission branches are skipped. It does not invoke either Tauri command, assert progress payloads, or mount ZaloOutreach UI. No new UI/progress runtime artifact was found in the inspected scope.
The stated eligibility test coverage also exceeds source: test at :4938 covers tab/newline, 0, -1, null, valid-phone+0 and numeric UID-only. It does not include abc or +84-90-123; undefined only appears in older standalone sanitizer tests, not this SQL/helper matrix.
Fix direction: add a regression crossing the command/event boundary and real UI evidence required by repository rules. Keep mock persistence results described as mock integration evidence. No real customer outreach is needed to prove eligibility/count/progress.

## Closure Against Previous Review
| Previous finding | Current status | Evidence |
| --- | --- | --- |
| Helper swallows DB failures | Closed for helper; open for worker | :2962-2973; error tests pass and probe returns ReadOnly/missing-table Err |
| Inconsistent eligibility | Open | digit-only phones and padded -1 counterexamples |
| Single-session IPC bypasses preflight | Source-level closed | send_zalo_outreach :2867-2923 and start_zalo_outreach :2999 both filter before spawning/partitioning; actual IPC regression still absent |
| Incomplete DOB tests | Closed at deserialization boundary | all three models x both negative values at :5216 pass |
| Runtime/progress/UI evidence | Open | mock calls have app_handle None and no event assertions |
| Diff/rustfmt errors | Closed for submitted Rust files | git diff --check and explicit four-file rustfmt check both exit 0 |

## Source-Level Clearance Notes
- commands.rs: helper now distinguishes QueryReturnedNoRows and propagates other errors and writes. Both sending commands invoke helper. Remaining blockers above. Report mentions prepared stmt_mark_invalid but actual implementation calls conn.query_row/conn.execute in per-ID loops; prepared-statement optimization is not present. Synchronous DB work remains under shared mutex in async commands; no new await while holding that guard.
- ZaloOutreach.svelte: sentinel trimming/lowercase and digit predicate match helper for common input, but not SQL/worker. Manual resolution keeps off-page IDs via `!lead`; backend remains authoritative. No-phone badge is not displayed for all invalid inputs: `abc` takes the nonblank-phone display branch; UID 0 takes the nonblank-UID branch. Disabled state and badge claims should be tested separately.
- user.rs: i64 fields at :99/:133 confirmed, both negative values tested.
- friend.rs: i64 at :101 confirmed; actual model FriendRecommendationsDataInfo covered by both negative values.
- commands_test.rs: six relevant tests pass. New email test checks invalid/empty email and ignored verified-Zalo filter for segmentation all; it does not establish global never_sent/last_failed/template_success coverage.

## Evidence Checked
- Fresh `bun run check`: exit 0; **0 errors, 6 warnings in 3 files**.
- Fresh `cargo build --manifest-path src-tauri/Cargo.toml`: exit 0, debug native build.
- Fresh `bun run build`: exit 0, adapter-static completes to build; warnings present.
- Fresh `cargo test --manifest-path src-tauri/Cargo.toml --lib`: exit 101, **870 passed, 2 failed, 1 ignored, 0 filtered**. All six relevant outreach/email/DOB tests pass.
- Failures: scrapers::google_maps::tests::test_google_maps_scraper_search and scrapers::grid::tests::test_grid_scraper_adaptive_subdivision. Both fail launching CloakBrowser (browser closed, SIGTRAP). These are outside the changed Zalo slice; causation by this patch has not been established. This run cannot confirm the claimed 872/0 full-suite result.
- Ignored: live_zalo_conversation_probe, requiring explicit non-production credentials.
- Fresh `git diff --check`: exit 0.
- Fresh `rustfmt --edition 2021 --check src-tauri/src/commands.rs src-tauri/src/commands_test.rs src-tauri/vendor/zca-rs/src/models/user.rs src-tauri/vendor/zca-rs/src/apis/friend.rs`: exit 0. This is the four modified Rust files, not a claim of repository-wide cargo fmt clearance.
- Source/SQLite probe: current functions/SQL extracted into eligibility_probe.rs, compiled with rustc --edition=2021 using repository rusqlite rlib and dependency search path. Probe log records successful helper fixes and remaining counterexamples. Generated executable removed; source and log retained.
- Anvien refreshed before query: 661 scanned, 412 parsed code, 0 failed; 93,014 nodes, 107,090 relationships. Query maps helper, target validator, UI validator and both sending command flows; full graph output retained. commands.rs projection risk HIGH, 3,984 unresolved references; source tracing supplies the proof where graph resolution is incomplete.
- Agentmemory recalled; mem0 tool unavailable, so dual-memory write claim cannot be independently confirmed.

Not run:
- Real UI/desktop progress or IPC validation, live Zalo delivery, full integration/doc tests, packaged release or benchmark. Review already has concrete unresolved counterexamples and makes no runtime acceptance claim.
- No code fixes, source reformatting, or commit performed by reviewer.

## Invariant Closure
Affected invariant: SQL counts/IDs, UI selection, preflight partitioning and actual worker target eligibility agree; failed persistence is reported; signed DOB fixtures deserialize.
Sibling surfaces checked: both IPC commands, mock/real worker guards, sanitizer, shared email SQL, frontend resolver and row rendering, three DOB models and new tests.
Residual: eligibility and runtime error counterexamples open; command/progress/UI unverified.

## Required Fix List For Resubmission
1. Unify actual phone/UID validation across SQL/preflight/worker/UI; include padded sentinel and digit-only invalid-phone cases.
2. Close worker persistence failures and propagate them through async command completion/error events.
3. Supply command/event and UI evidence; accurately scope mock and deserialization evidence.
4. Resolve or explicitly classify the unrelated browser-test failures with fresh evidence before claiming a fully green library suite.

## Overall Evaluation
This resubmission makes measurable progress and closes preflight error handling, the source-level bypass, DOB fixture completeness and changed-file formatting. It does not yet close the earlier runtime/eligibility invariant. REJECT concerns the comprehensive-completion claim, not the already demonstrated local fixes.
