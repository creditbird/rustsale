# Zalo Group Member Deep Scan Remediation Execution Ledger

## Control Header

| Field | Value |
|---|---|
| Work item | Production remediation for Zalo Group Board/Poll UID discovery and supervisor-owned history backfill |
| Repository/worktree | `/home/arch/Project/test/RustSale_v2` on existing `master` working tree |
| Approved specification | `docs/superpowers/specs/2026-08-10-zalo-group-member-deep-scan-remediation-design.md` |
| Approved plan | `docs/superpowers/plans/2026-08-10-zalo-group-member-deep-scan-remediation.md` |
| Started at | 2026-08-10T19:18:29+07:00 |
| Current phase | COMPLETE |
| Overall status | PRODUCTION GATES PASSED |
| Pre-existing changes boundary | Preserve all user changes listed by `git status`; do not revert unrelated work in `AGENTS.md`, `CLAUDE.md`, commands, Zalo client/vendor, DB tests, app registration, workspace UI, service contract, or the untracked modal. |

## Requirement Coverage Matrix

| Requirement | Acceptance criterion | Evidence | Owning task | Status |
|---|---|---|---|---|
| REQ-001 | Deep Scan uses only supervisor-owned command 511 history | Rust command and supervisor tests | TASK-002, TASK-005 | VERIFIED |
| REQ-002 | Diagnostics label history as account-wide | Rust/TypeScript contract and UI tests | TASK-005, TASK-007 | VERIFIED |
| REQ-003 | Per-account singleflight returns bounded typed outcomes | Supervisor concurrency/timeout tests | TASK-002 | VERIFIED |
| REQ-004 | Re-arm resumes durable group-message cursor only | Supervisor cursor/re-arm tests | TASK-002 | VERIFIED |
| REQ-005 | No HTTP history fallback; malformed history fails contextually | Vendor and command regression tests | TASK-001, TASK-005 | VERIFIED |
| REQ-006 | Board pages continue until bounded stop condition | Zalo client pagination tests | TASK-003 | VERIFIED |
| REQ-007 | Board/Poll payload variants extract all required UID sources | Pure extraction and poll tests | TASK-003 | VERIFIED |
| REQ-008 | Partial provider failures return structured warnings; DB failures are fatal | Client and command failure tests | TASK-003, TASK-005 | VERIFIED |
| REQ-009 | UID canonicalization is transactional and rejects sentinels | Repository rollback/collision tests | TASK-004 | VERIFIED |
| REQ-010 | Identity references and lead links survive canonical merge | Repository reference migration tests | TASK-004 | VERIFIED |
| REQ-011 | Selected-conversation evidence includes quote owners and all local sources | Normalizer/repository tests | TASK-004, TASK-005 | VERIFIED |
| REQ-012 | Import accepts only canonical identities evidenced by requested conversation | Command/repository rejection tests | TASK-006 | VERIFIED |
| REQ-013 | Import deduplicates and resolves leads only by exact links/keys | Import tests | TASK-006 | VERIFIED |
| REQ-014 | Import mutations and counts commit atomically | Failure-injection transaction tests | TASK-006 | VERIFIED |
| REQ-015 | Import publishes one standard post-commit invalidation | Service event tests | TASK-006 | VERIFIED |
| REQ-016 | Modal open and post-import refresh remain local-only | Component tests | TASK-007 | VERIFIED |
| REQ-017 | Request generations reject stale responses and duplicate clicks | Component tests | TASK-007 | VERIFIED |
| REQ-018 | Rust, TypeScript, and UI expose all structured diagnostics | Contract/component tests | TASK-005, TASK-007 | VERIFIED |
| REQ-019 | CSV neutralizes formulas and preserves RFC-style quoting/BOM | Pure CSV tests | TASK-007 | VERIFIED |
| REQ-020 | Dialog semantics, focus, Escape, restoration, and announcements work without pixel drift | Accessibility tests and screenshots | TASK-007 | VERIFIED |
| REQ-021 | Every remaining behavior follows RED, GREEN, regression | Focused logs plus final gates | TASK-001 through TASK-008 | VERIFIED |
| REQ-022 | Legacy databases add `reply_sender_identity_id` before creating its dependent index | Migration RED/GREEN, focused suites, real DB startup/schema inspection | TASK-009 | VERIFIED |

Coverage invariants:

- [x] Every approved requirement has a row.
- [x] Every row has measurable acceptance criteria.
- [x] Every row owns at least one task.
- [x] Every requirement has planned automated evidence.
- [x] Non-goals remain in the approved specification.

## Dependency Graph

| Task | Depends on | Unlocks | Cross-layer boundaries | GitNexus risk |
|---|---|---|---|---|
| TASK-001 Provider history semantics | None | TASK-002, TASK-005 | Vendor decoder to listener | HIGH |
| TASK-002 Supervisor backfill | TASK-001 | TASK-005 | WebSocket, supervisor, SQLite cursor | CRITICAL |
| TASK-003 Board/Poll pagination | None | TASK-005 | Zalo HTTP provider to client report | MEDIUM |
| TASK-004 Canonical identities | None | TASK-005, TASK-006 | Migration, repository, message normalization | CRITICAL |
| TASK-005 Deep Scan orchestration | TASK-001 through TASK-004 | TASK-007 | Tauri, supervisor, provider, repository, TypeScript | CRITICAL |
| TASK-006 Atomic import | TASK-004 | TASK-007 | Tauri, repository, CRM leads/categories, events | CRITICAL |
| TASK-007 Modal/CSV/a11y | TASK-005, TASK-006 | TASK-008 | Svelte UI, Tauri contract | HIGH |
| TASK-008 Release verification | TASK-001 through TASK-007 | Final report | Full application | CRITICAL |
| TASK-009 Legacy startup migration | TASK-004 | Final report | SQLite bootstrap, schema, migration | LOW |

## Task Ledger

| Task | Status | Current evidence |
|---|---|---|
| TASK-001 Provider history semantics | VERIFIED | Focused vendor/listener tests completed in prior checkpoint; must rerun in TASK-008 |
| TASK-002 Supervisor-controlled singleflight | VERIFIED | Focused supervisor module passed in prior checkpoint; must rerun in TASK-008 |
| TASK-003 Board/Poll pagination | VERIFIED | Focused client tests passed in prior checkpoint; must rerun in TASK-008 |
| TASK-004 Canonical identities | VERIFIED | `task4-focused.log`: repository 66/66, migration 17/17, normalizer 18/18 |
| TASK-005 Deep Scan orchestration | VERIFIED | Deep Scan command module 7/7; Board top-level failure and snapshot mismatch RED/GREEN logs; TypeScript service contract 34/34 |
| TASK-006 Atomic import | VERIFIED | Repository 3/3, service event 2/2, command integration 1/1; TypeScript service contract 34/34 |
| TASK-007 Modal/CSV/a11y | VERIFIED | Modal 9/9, CSV 2/2, workspace focus integration 1/1, typecheck 0/0, modal and wizard pixel/style diff 0 |
| TASK-008 Release verification | VERIFIED | Rust 478/0/1 ignored after startup follow-up; vendor 27/0 plus 1 doctest; Bun 363/0/1 skipped; typecheck 0/0; production build, fmt, clippy, release build, and diff check passed; GitNexus CRITICAL scope reviewed |
| TASK-009 Legacy startup migration | VERIFIED | Regression RED/GREEN; migration 18/18; DB 18/18; full Rust 478/0/1 ignored; clippy/release clean; real `bun run tauri dev` startup and SQLite schema inspection passed |

## QA Evidence Log

| Evidence ID | Level | Time | Command or inspection | Exit | Result | Relevant output/path |
|---|---|---|---|---:|---|---|
| EV-004-01 | E2 | 2026-08-10 | `cargo test --manifest-path src-tauri/Cargo.toml canonical_zalo_uid_ -- --nocapture` | 0 | 3 passed | `/home/arch/.cache/codex-rustsale-review-20260810/identity-compile.log` |
| EV-004-02 | E2 | 2026-08-10 | Repository, migration, and normalizer focused module tests | 0 | 66 + 17 + 18 passed | `/home/arch/.cache/codex-rustsale-review-20260810/task4-focused.log` |
| EV-005-01 | E2 | 2026-08-10 | `cargo test --manifest-path src-tauri/Cargo.toml group_member_deep_scan -- --nocapture` | 0 | 7 passed | `/home/arch/.cache/codex-rustsale-review-20260810/task5-command-group-final.log` |
| EV-005-02 | E2 | 2026-08-10 | Board top-level failure regression RED/GREEN | 0 | Failure reproduced, then 1 passed | `/home/arch/.cache/codex-rustsale-review-20260810/task5-board-top-red.log`, `/home/arch/.cache/codex-rustsale-review-20260810/task5-board-top-green.log` |
| EV-005-03 | E2 | 2026-08-10 | Direct snapshot account/thread mismatch regression RED/GREEN | 0 | Failure reproduced, then 1 passed | `/home/arch/.cache/codex-rustsale-review-20260810/task5-snapshot-mismatch-red.log`, `/home/arch/.cache/codex-rustsale-review-20260810/task5-snapshot-mismatch-green.log` |
| EV-006-01 | E2 | 2026-08-10 | Focused repository, service-event, and command import tests | 0 | 3 + 2 + 1 passed | `/home/arch/.cache/codex-rustsale-review-20260810/task6-focused-final.log` |
| EV-006-02 | E2 | 2026-08-10 | `bun test src/lib/services/__tests__/conversationService.spec.ts` | 0 | 34 passed | Fresh terminal output on 2026-08-10 |
| EV-006-03 | E2 | 2026-08-10 | `bun run check` | 0 | 0 errors, 0 warnings | Fresh terminal output on 2026-08-10 |
| EV-007-01 | E2 | 2026-08-10 | Focused CSV, modal, workspace focus, and Svelte checks | 0 | 2 + 9 + 1 passed; 0 errors, 0 warnings | `/home/arch/.cache/codex-rustsale-review-20260810/task7-focused-final.log` |
| EV-007-02 | E3 | 2026-08-10 | Chromium 1440×1000 modal and wizard before/after geometry plus computed-style comparison | 0 | Every measured rect/style diff is 0 | `/home/arch/.cache/codex-rustsale-review-20260810/group-members-pixel-final.json` |
| EV-007-03 | E3 | 2026-08-10 | Modal and wizard screenshots before/after | 0 | Layout preserved while Dialog semantics were added | `/home/arch/.cache/codex-rustsale-review-20260810/group-members-modal-before.png`, `/home/arch/.cache/codex-rustsale-review-20260810/group-members-modal-after.png`, `/home/arch/.cache/codex-rustsale-review-20260810/group-members-wizard-before.png`, `/home/arch/.cache/codex-rustsale-review-20260810/group-members-wizard-after.png` |
| EV-008-01 | E4 | 2026-08-10 | `TMPDIR=/home/arch/.cache/codex-rustsale-review-20260810 cargo test --manifest-path src-tauri/Cargo.toml` | 0 | 477 passed, 0 failed, 1 ignored | `/home/arch/.cache/codex-rustsale-review-20260810/task8-rust-final.log` |
| EV-008-02 | E4 | 2026-08-10 | `TMPDIR=/home/arch/.cache/codex-rustsale-review-20260810 cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml` | 0 | 27 unit tests and 1 doctest passed | `/home/arch/.cache/codex-rustsale-review-20260810/task8-vendor-final.log` |
| EV-008-03 | E4 | 2026-08-10 | `bun test` | 0 | 363 passed, 0 failed, 1 explicitly skipped | `/home/arch/.cache/codex-rustsale-review-20260810/task8-bun-final.log` |
| EV-008-04 | E4 | 2026-08-10 | `bun run check` | 0 | 0 errors, 0 warnings | `/home/arch/.cache/codex-rustsale-review-20260810/task8-check-final.log` |
| EV-008-05 | E4 | 2026-08-10 | `bun run build` | 0 | Static production build completed | `/home/arch/.cache/codex-rustsale-review-20260810/task8-build-final.log` |
| EV-008-06 | E4 | 2026-08-10 | `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`; `git diff --check` | 0 | Formatting and whitespace clean | `/home/arch/.cache/codex-rustsale-review-20260810/task8-fmt-final.log`, `/home/arch/.cache/codex-rustsale-review-20260810/task8-diff-check-preledger.log` |
| EV-008-07 | E4 | 2026-08-10 | Clippy with `-D warnings`; Rust release build | 0 | Both completed without warnings or errors | `/home/arch/.cache/codex-rustsale-review-20260810/task8-clippy-final.log`, `/home/arch/.cache/codex-rustsale-review-20260810/task8-release-final.log` |
| EV-008-08 | E3 | 2026-08-10 | GitNexus `detect_changes(scope="all")` plus `run_session_loop` upstream impact | 0 | Working tree remains CRITICAL: 357 changed symbols, 182 affected; `run_session_loop` HIGH with two reviewed d=1 callers (`start_account_task`, `restart_account`) | Fresh MCP output on 2026-08-10 |
| EV-008-09 | E3 | 2026-08-10 | Changed-test provider mutation audit | 0 | Added tests use fake clients/supervisor harnesses; no live Zalo message, reaction, invite, poll, or membership mutation is sent | Fresh terminal inspection on 2026-08-10 |
| EV-009-01 | E2 | 2026-08-10 | `legacy_schema_adds_reply_sender_identity_before_creating_index` RED/GREEN | 0 | RED reproduced `no such column: reply_sender_identity_id`; GREEN passed 1/1 after moving index creation behind the column upgrade | `/home/arch/.cache/codex-rustsale-review-20260810/startup-migration-red.log`, `/home/arch/.cache/codex-rustsale-review-20260810/startup-migration-green.log` |
| EV-009-02 | E2 | 2026-08-10 | Focused conversation migration and DB suites | 0 | Migration 18/18 and DB 18/18 passed | `/home/arch/.cache/codex-rustsale-review-20260810/startup-migration-suite.log`, `/home/arch/.cache/codex-rustsale-review-20260810/startup-db-suite.log` |
| EV-009-03 | E4 | 2026-08-10 | Full Rust tests; clippy with `-D warnings`; Rust release build | 0 | 478 passed, 0 failed, 1 ignored; clippy and release build completed cleanly | `/home/arch/.cache/codex-rustsale-review-20260810/startup-fix-rust-full.log`, `/home/arch/.cache/codex-rustsale-review-20260810/startup-fix-clippy.log`, `/home/arch/.cache/codex-rustsale-review-20260810/startup-fix-release.log` |
| EV-009-04 | E3 | 2026-08-10 | `bun run tauri dev` against the backup-protected real DB plus SQLite schema inspection | 0 | App remained running past initialization; column, FK, and partial index exist; `pragma_foreign_key_check` reports 0 violations | `/home/arch/.cache/codex-rustsale-review-20260810/startup-fix-tauri-dev.log`, `/home/arch/.cache/codex-rustsale-review-20260810/startup-fix-db-schema.log`, `/home/arch/.cache/codex-rustsale-review-20260810/rustsale-before-reply-sender-migration.db` |
| EV-009-05 | E3 | 2026-08-10 | Rust fmt check, `git diff --check`, `bun run check`, GitNexus final scope, and upstream impact for `migrate`/`init_db` | 0 | Formatting/whitespace clean; Svelte 0 errors/0 warnings; working tree remains CRITICAL at 357 changed symbols and 178 affected because it includes the broader pre-existing remediation; both startup symbols are LOW with no d=1 callers | `/home/arch/.cache/codex-rustsale-review-20260810/startup-fix-final-checks.log`; fresh MCP output on 2026-08-10 |

## Integration Audit Matrix

| Flow | Entry point | DDL | Repository | Domain/provider | Tauri contract | Frontend | Runtime test | Status |
|---|---|---|---|---|---|---|---|---|
| Deep Scan | `sync_and_deep_scan_group_members` | Quote-owner FK + legacy-safe index | Canonical attach/evidence | Supervisor + Board/Poll | Structured report | Modal diagnostics | Focused command + UI + real DB startup | VERIFIED |
| Batch import | `batch_import_group_members_to_leads` | Existing CRM schema | Atomic import transaction | Conversation evidence | Request includes conversation ID | Import action | Failure injection + UI | VERIFIED |

## Completion Lock

- [x] Requirement coverage is 100% verified.
- [x] All tasks are VERIFIED.
- [x] All task-level QA gates pass after final source edits.
- [x] Full Rust, vendor, Bun, typecheck, build, fmt, clippy, and release gates pass.
- [x] Pixel measurements show no unintended modal drift.
- [x] GitNexus final `detect_changes(scope="all")` is reviewed.
- [x] No live Zalo mutation is sent by tests.
- [x] Legacy and real databases start with `reply_sender_identity_id` and its dependent index created in safe order.

Known repository-wide, non-feature blockers:

- Full multi-file Bun execution emits Svelte `derived_inert` teardown warnings across many existing conversation suites. The modal suite passes 9/9 and repeated focused executions emit zero such warnings, while `bun run check` reports zero warnings.
- The production build retains the existing large-chunk warning (`2,416.59 kB` main client chunk). This remediation does not introduce a new chunk or alter the established bundling boundary.
- The pre-existing dirty `master` working tree includes tracked vendored `target/` artifacts. Exclude generated binaries when curating a release commit; they are outside this remediation source scope.

**Final eligible status:** `PRODUCTION GATES PASSED`
