# Supervisor Report: Zalo outreach third submission

Verdict: REJECT

## Metadata
- Reviewer: gpt-6; review time: 2026-09-08T15:00:37.228163+07:00
- Repository: RustSale_v2; scope: current five modified source/test files and latest user completion claim.
- Authority: user review request, AGENTS.md, supervisor skill and implementation completion harness.
- Prior report: `rp_supervisor_260908_131500_by_gpt-6_zalo_outreach_resubmission.md`.
- Current evidence: `evidence/zalo_outreach_260908_review3/`; source hashes, verbatim-function probes and command logs retained.
- No submitted production code/tests modified, no commit, no real Zalo message sent by reviewer.

## Executive Summary
The previously supplied simple counterexamples (1, +84-90-123, padded -1, cached-uid) are fixed. Preflight calls the actual Rust mobile predicate. Invalid-phone worker writes now propagate errors, and a new MockRuntime test receives events. These improvements are confirmed. Complete consistency/error/evidence closure is still not proven: normalization diverges for allowed separators and whitespace; the real send-error branch still ignores persistence errors; the event test counts events without validating their contents.

## Blocking Findings

### [P2] SQL, Rust and frontend still normalize different inputs
File: `src-tauri/src/commands.rs:114`, `:1227`, `:2976`; `src/lib/components/ZaloOutreach.svelte:551`, `:564`.
Rust phone normalization stops at any character outside its explicit ASCII allowed set, including tab/newline. SQL removes tabs/newlines, and frontend permits all JS whitespace. Conversely Rust/frontend allow brackets and comma, but SQL does not remove these within a phone number.
Current probe results (no UID for phone cases):

| Input | SQL | Rust preflight/worker | Frontend |
| --- | --- | --- | --- |
| `1` | reject | reject | reject |
| `+84-90-123` | reject | reject | reject |
| `+84 901234567` | accept | accept | accept |
| tab followed by `0901234567` | accept | reject | accept |
| `090[123]4567` | reject | accept | accept |
| `090,1234567` | reject | accept | accept |
| empty phone, UID ` -1 ` | reject | reject | reject |
| empty phone, UID `cached-uid` | accept | accept | accept |
| empty phone, UID `0` + tab + `0` | reject | accept | reject |
| empty phone, UID NBSP + `0` + NBSP | accept | reject | reject |

UID discrepancy: SQL/UI remove interior tab/newline/CR; Rust sanitizer only trims the ends. SQLite TRIM also differs from Rust/JS for non-ASCII surrounding whitespace. The claims of uniform whitespace handling and complete identity agreement are therefore false on this snapshot.
Evidence: `predicate_probe.rs` executes the verbatim SQL plus Rust normalizers against in-memory SQLite. `ui_predicate_probe.ts` executes the verbatim frontend functions with bun on identical fixtures. These are isolated function/SQLite probes, not mounted app UI evidence. Logs record the table above.
Impact: a lead may appear/count/select in UI yet be discarded by campaign preflight, or be excluded from the server list despite satisfying worker eligibility. These are the original invariant's remaining cases.
Fix direction: choose a single normalization policy for the supported input domain and make SQL, Rust and UI agree; test each allowed separator and whitespace class in one shared matrix, including normalized target value as well as acceptance.

### [P1] Actual send-error branch still discards DB writes
File: `src-tauri/src/commands.rs:4395` through the update/history statements at :4397/:4401.
The new propagation covers invalid phone and lookup branches. The Err branch of the real `zalo.send_message(...)` still uses `if let Ok(conn)` and `let _ = conn.execute(...)` for both lead status and FAILED outreach_history. Its status can be VERIFIED_NOT_EXIST, BLOCKED_STRANGER, RATE_LIMITED or SEND_FAILED. On failed persistence it continues to log/progress/finalization without surfacing the database failure.
Evidence: direct inspection of this still-present branch. No live Zalo failure was invoked. The new write-error test uses the mock invalid-phone branch and does not exercise this real send-failure branch.
Why blocking: the resubmission claims all relevant VERIFIED_NOT_EXIST/history writes propagate; the earlier requirement was error propagation across the worker boundary, not only one branch.
Fix direction: propagate lock/update/history errors here too, with a deterministic provider-failure test plus failed-write injection. Assert the resulting command/UI error outcome and absence of misleading successful completion.

### [P2] New event test does not assert claimed payload/progress behavior
File: `src-tauri/src/commands_test.rs:5810`; payload type `src-tauri/src/commands.rs:2835`.
All three listeners discard `_event` and increment counters. Assertions require log >=1, progress >=1, complete ==1. A wrong status, percent, denominator, total_sent or message still passes. The test uses one successful lead and directly calls the worker; it does not validate multi-session orchestration, IPC wrappers, failure/cancellation progress, or Svelte rendering.
The report specifically claims outreach-complete contains status completed and progress 100.0. Actual OutreachCompletePayload has only total_sent and message; status/progress are fields of OutreachProgressPayload. No assertion checks either payload.
Related source risk: command error wrappers emit outreach-complete (:2966 and :3174), while the Svelte complete listener (:643-648) always sets progress to 100 and tags the log COMPLETE. The error message is displayed, so this is not silent success, but the event/UI model does not distinguish failed progress as claimed. Single-command error handling reports total_sent=0, and multi-session error handling reports only earlier completed partitions; partial sends in a failing partition are lost from that payload. Add a partial-success-then-error case.
Fix direction: parse captured payloads and verify event order, statuses, counts and progress at each lead/session and on error; invoke the registered orchestration boundary in tests; add actual user-visible runtime evidence. Retain the useful mock event transport test but describe its narrower proof accurately.

## Closure of Prior Findings
- Short/digit-only phones: CLOSED for the previous literal examples; broader normalization invariant still OPEN.
- Padded -1 and cached-uid: CLOSED for those examples; other whitespace handling still OPEN.
- Invalid-phone worker failed write: CLOSED at tested worker boundary; test passes and both mock/real invalid-phone source guards propagate.
- Helper DB errors and both IPC preflight calls: preserved in source.
- Runtime events: improved from no handle to MockRuntime delivery; payload and actual UI acceptance still OPEN.
- DOB: prior all-three-model/two-negative-value regression preserved and passes.
- Formatting: four changed Rust files and diff whitespace gates pass.

## Source-Level Clearance Notes
- commands.rs: changes correctly route preflight through is_vn_mobile, propagate several previously ignored writes and stop orchestration on returned worker errors. Remaining source blockers above. Per-ID SQLite work is still synchronous under the shared DB lock; no new performance acceptance claimed.
- ZaloOutreach.svelte: Invalid phone badge and disabled controls are present; normalization differences and completion/error representation need closure. Manual resolver still intentionally retains IDs absent from current page, requiring authoritative backend enforcement.
- user.rs: signed i64 confirmed by unchanged diff at :99/:133 and passing DOB regression.
- friend.rs: signed i64 at :101; FriendRecommendationsDataInfo included in passing DOB regression.
- commands_test.rs: 16-lead matrix checks the stated simple inputs; event/write-error tests exist and pass. Counter-only listeners cannot prove payload claims.

## Evidence Checked
- Fresh `bun run build`: exit 0; adapter-static wrote build output.
- `cargo build --manifest-path src-tauri/Cargo.toml`: exit 0, fresh native debug build.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib test_zalo`: exit 0; **14 passed, 0 failed, 0 ignored, 861 filtered**; includes new event and write-error tests.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib test_zca_dob`: exit 0; **1 passed, 874 filtered**.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib retryable_error_retries_three_times_with_backoff`: exit 0; **1 passed, 874 filtered**. This tests retry behavior; it does not rerun or close the two browser-launch failures in the previous review.
- `git diff --check`: empty log; no whitespace errors observed.
- `rustfmt --edition 2021 --check` on commands.rs, commands_test.rs, vendor user.rs and friend.rs: exit 0. Does not claim repository-wide cargo fmt cleanliness.
- Differential normalization probes: current production expressions/functions, SQLite and bun; contradictory results recorded as above. Executable removed after capture; reproducible sources retained.
- Anvien analyze --force: 672 scanned, 413 parsed code, 0 failed; 93,377 nodes, 107,476 relationships. Fresh query maps phone normalizer/target validation/worker; complete graph query retained. commands.rs projection risk HIGH, 4,006 unresolved references; source independently inspected.
- Agentmemory queried; mem0 unavailable in tools. Neither memory is treated as correctness evidence.

Not run:
- Full Rust library/integration/doc suites, previous browser-launch repros, live Zalo messages, real desktop/UI progress validation, packaged release or performance benchmark. No PASS is inferred for any of these.

## Invariant Closure
Affected invariant: query/count/resolve, UI eligibility, preflight and worker agree on target normalization; DB failures propagate across the worker and orchestration boundary; event payloads truthfully represent progress/outcome.
Sibling surfaces inspected: SQL filter, Rust sanitizer/mobile normalizer, UI validators and complete listener, both IPC wrappers, mock/real invalid-phone and lookup/send-error paths, DOB models and added tests.
Residual: the normalization counterexamples and real send-error DB writes remain open; event payload/orchestration/UI coverage remains insufficient.

## Required Fix List For Resubmission
1. Close normalization for all supported separator/whitespace inputs with differential matrix tests.
2. Propagate remaining real send-error DB writes and verify failure outcomes.
3. Assert event payloads and multi-lead/multi-session/error outcomes; verify the actual UI boundary and correct the report's complete-payload description.
4. Keep filtered test claims explicit; retry-unit-test success does not prove earlier browser-launch tests recovered.

## Overall Evaluation
The latest changes fix the specific prior examples and add useful worker/error/event tests. The remaining findings are concrete counterexamples to the same original invariant, not a request to expand into unrelated features. REJECT applies to the claim that all four findings are fully resolved.
