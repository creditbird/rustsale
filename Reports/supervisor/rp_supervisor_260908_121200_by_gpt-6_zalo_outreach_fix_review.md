# Supervisor Report: Zalo outreach filter and negative DOB fix

Verdict: REJECT

## Metadata
- Reviewer: gpt-6
- Review time: 2026-09-08T12:11:42.328750+07:00
- Repository: RustSale_v2; HEAD 8f69c61; five pre-existing modified source/test files.
- Scope: User-supplied completion claim and current uncommitted changes.
- Authority: user review request, AGENTS.md, working-rules, supervisor, diagnosing-bugs, implementation completion harness, Tauri skill.
- Related evidence: `evidence/zalo_outreach_260908_review/`; source hashes recorded there.
- Review only: no production source or existing tests changed; no commit or external outreach performed.

## Executive Summary
The empty/space-only phone filter and signed DOB fields are implemented, and the two new tests pass. The claim that the entire invariant and diagnosis workflow are complete cannot be accepted. Eligibility differs between SQL, preflight, UI, and the worker; SQLite failures are silently accepted; a registered alternate IPC path bypasses preflight. Runtime UI/session-distribution and live API success claims remain unproven.

## Blocking Findings

### [P1] Database failures are converted to invalid leads or false persistence success
File: `src-tauri/src/commands.rs:2912`, `:2917`, `:2923`, `:2952`, `:2971`.
Every SELECT error becomes an invalid ID, and every UPDATE error is discarded. The Result-returning helper always returns Ok. Consequently the caller's Database error mapping cannot receive these failures; a database outage can become “No eligible leads” and an outreach-complete event. Runtime guards at :3311, :3346, :3821 and :3855 also discard update errors.
Evidence: `helper_probe.rs` contains verbatim current helper and UID sanitizer, compiled with the repository's rusqlite artifact against in-memory SQLite. `helper-probe.log` records:
- read-only DB: `Ok(([], [2]))` but persisted status remains `UNTESTED`;
- missing table: `Ok(([], [1]))`.
This contradicts guaranteed persistence and accurate failure reporting. Propagate database errors, distinguish missing records from infrastructure/type errors, and require failed-write/read regression coverage. No production DB was used.

### [P2] Eligibility is inconsistent with the actual worker
File: `src-tauri/src/commands.rs:113`, `:2900`, `:2903`; `src/lib/components/ZaloOutreach.svelte:558`, `:916`.
SQL, preflight, and frontend accept any nonempty UID. The worker calls `sanitize_valid_zalo_uid` (:1202, :3244, :3745), which rejects `0`, `-1`, `null`, and `undefined`. A lead with empty phone and UID `0` therefore appears selectable, survives preflight/session partitioning, then is skipped as missing phone at runtime.
Evidence: verbatim-helper probe returns `valid=[1] invalid=[]` while the actual sanitizer returns `None`. This is the original progress/partition problem through a concrete uncaught input.
Separately, SQL TRIM leaves tabs/newlines intact while Rust/JS trim removes them. `sql-probe.log`, executing the extracted production SQL in SQLite, shows a phone containing only `\t\n` is still included in query results/counts. Existing whitespace regression covers only ASCII spaces.
Required closure: use consistent identity/whitespace semantics across page/count/resolve, explicit selection, preflight and worker. Test sentinel UIDs with and without valid fallback phone, whitespace variants, and valid UID-only leads. Nonempty invalid phone strings also survive preflight and are rejected by is_vn_mobile later, so preflight cannot guarantee that every counted lead is sendable.

### [P2] Registered single-session command bypasses the new preflight
File: `src-tauri/src/commands.rs:2857`, `:2866`; registration `src-tauri/src/lib.rs:549`.
`send_zalo_outreach` still forwards raw lead IDs directly to `send_zalo_outreach_internal`. Only `start_zalo_outreach` calls the new helper. Direct invocation of this registered command with an empty-phone/no-UID lead still enters the worker's progress denominator (`display_total`, :3200 onward) and skips it there. Thus protection is incomplete at the authoritative IPC boundary, even though the current ZaloOutreach UI calls start_zalo_outreach.
Required closure: share preflight across supported IPC entrypoints or explicitly retire the alternate command; verify direct IPC/worker behavior and progress totals.

### [P2] Verification supports a narrower result than the completion claim
File: `src-tauri/src/commands_test.rs:4806`, `:4938`.
- Lead regression tests page/count/resolve and the helper, not start_zalo_outreach session allocation, progress events, frontend controls, or DB failure paths.
- DOB regression checks -25200 only for UserBasic and -72255600 only for User. It does not check both values in both models, does not test FriendRecommendationsDataInfo, and does not execute find_user or send a message.
- No fresh runtime UI/Playwright evidence or replay of the two reported real phone lookups was gathered. Passing deserialization alone cannot establish successful delivery.
- No matching durable RED/minimized original repro, ranked hypotheses, diagnosis cleanup record or requirement ledger for this fix was located in the inspected Reports/docs evidence. This review evaluates that claim; it does not rerun repair phases or invent historical evidence.
- `git diff --check` fails at commands_test.rs:5001 (new blank line at EOF). `cargo fmt --check` also fails, including unrelated existing formatting; do not attribute all formatting debt to this patch.
Required closure: targeted adversarial regressions, current runtime progress/selection evidence, accurate test/skipped counts, and the required completion records. Record live API evidence only using an authorized test account; do not imply unit tests prove real delivery.

## Source-Level Clearance Notes
- commands.rs: BLOCKED by persistence, eligibility, and alternate entrypoint findings. New synchronous per-ID SELECT/UPDATE loop runs under the shared DB mutex inside an async command; no throughput benchmark was supplied. No mutex-across-await was introduced in this block, but the new work is not offloaded.
- vendor/zca-rs/src/models/user.rs: signed i64 change confirmed at :99/:133; both existing new fixtures pass. No remaining unsigned dob field found in this response model group.
- vendor/zca-rs/src/apis/friend.rs: i64 confirmed at :101; actual struct is FriendRecommendationsDataInfo. Dedicated negative-DOB fixture not present in the new regression.
- ZaloOutreach.svelte: No phone badge, disabled checkbox and row guards are present. The explicit-selection helper intentionally keeps IDs absent from the current page (`!lead`); it is not a universal frontend filter. Server-side enforcement remains necessary. No real UI acceptance claimed.
- commands_test.rs: new tests assert useful positive and blank-phone cases but leave the adversarial/runtime boundaries above open.
- Email sibling: shared filter now enforces nonblank email, ignores filter_verified_zalo on email, and applies all segmentation modes globally. These are changes beyond just Zalo/never_sent and need explicit coverage.
- NULL qualification: current leads schema declares phone TEXT NOT NULL (`db.rs:73`). NULL-phone behavior requires a legacy-schema fixture; the current test only inserts empty strings/spaces.

## Evidence Checked
Fresh for current source hashes:
- `bun run build`: exit 0 (warnings); frontend production build only, no packaged release.
- `cargo build --manifest-path src-tauri/Cargo.toml`: exit 0; native debug build.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib`: exit 0; **868 passed, 0 failed, 1 ignored, 0 filtered**. Both named new regression tests pass. Ignored test: live_zalo_conversation_probe (requires explicit test credentials). This was the library suite, not all integration/doc tests.
- `bun run check`: exit 0; **0 errors, 6 warnings in 3 files**.
- SQLite helper and SQL probes: reproduce the failures documented above; isolated source/DB evidence, not full app runtime.
- `git diff --check`: exit 2; trailing blank line.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`: exit 1; formatting differences in both reviewed and unrelated files.
- Anvien refreshed: 648 scanned files, 411 parsed code, 0 failed; 92,586 nodes, 106,634 relationships. Full graph query/impact output retained without discarding graph evidence.
- Anvien helper impact: HIGH; 1 direct caller (`start_zalo_outreach`), 4 affected flows. commands.rs has 3,966 unresolved references; graph evidence is navigation support, not proof of complete coverage. Source traced independently.
- Agentmemory recall confirms a prior record matching the claim. mem0 tools are unavailable in this session, so the claim of saving to both systems cannot be independently verified.

Not run:
- Real desktop/browser UI control sweep, session-allocation/progress runtime, live Zalo lookup/delivery, full integration/doc suites, packaged release and performance benchmark.
- GitNexus change detection/commit: no implementation change or commit was performed by this review.

## Invariant Closure
Invariant: eligibility/count/selection/partition/worker identity checks must agree; database errors must not masquerade as missing phone or persisted status; DOB response parsing must accept signed timestamps.
Sibling surfaces inspected: shared email query, both registered Zalo sending commands, mock and real worker branches, UID sanitizer, three DOB models and the two new tests.
Residual unverified surfaces: actual UI, actual session progress/distribution and live API behavior. Proven counterexamples remain open.

## Required Fix List For Resubmission
1. Unify eligibility semantics and cover sentinel/whitespace/fallback cases across all boundaries.
2. Propagate SQLite failures and prove persistence/failure UI behavior.
3. Close the alternate registered command path.
4. Add focused runtime/session and DOB-model coverage, close whitespace gates, record fresh completion evidence and disclose the ignored live test.

## Overall Evaluation
The core signed-DOB correction and basic blank-phone exclusion are useful and pass their tests. Those passes do not establish full closure. REJECT applies to the complete-work claim; review did not modify the submitted implementation.
