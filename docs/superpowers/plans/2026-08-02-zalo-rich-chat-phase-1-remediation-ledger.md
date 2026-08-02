# Zalo Rich Chat Phase 1 Remediation Ledger

## Control Header

| Field | Value |
|---|---|
| Work item | Complete and remediate Zalo Rich Chat Phase 1 Tasks 0-18 |
| Repository/worktree | `/home/arch/Project/test/RustSale_v2` on `master` |
| Approved specification | `docs/superpowers/specs/2026-08-02-zalo-rich-chat-phase-1-design.md` |
| Approved plan | `docs/superpowers/plans/2026-08-02-zalo-rich-chat-phase-1-implementation-plan.md` |
| Started at | 2026-08-02 |
| Current phase | BASELINE |
| Overall status | NOT READY |
| Pre-existing changes boundary | Clean at audit start; `bunx gitnexus analyze` then refreshed counts in `AGENTS.md` and `CLAUDE.md` |

## Requirement Coverage Matrix

| Requirement ID | Requirement | Acceptance criteria | Evidence | Owning task | Status |
|---|---|---|---|---|---|
| REQ-001 | Durable safe staging | 50 MiB/file, 100 MiB/request, 1 GiB spool, no source paths in frontend/errors, rollback partial staging | Rust unit tests + serialization checks | TASK-001 | COMPLETE |
| REQ-002 | Rich composer integration | File picker, drag/drop, clipboard, emoji, sticker, reply and atomic batch send are wired into `MessageComposer` | Svelte component tests + typecheck | TASK-002 | COMPLETE |
| REQ-003 | Safe rich rendering | Image/file/video/audio/sticker/link renderers reject unsafe URLs and raw local paths | Unit tests + security search | TASK-003 | COMPLETE |
| REQ-004 | Message actions and reactions | Reply/copy/recall eligibility, recall confirmation, read-only reactions and current-account marker | Component/workspace tests + Rust projection tests | TASK-004 | COMPLETE |
| REQ-005 | Batch timeline behavior | Adjacent parts group, virtual boundaries expand, optimistic batch merge scrolls once | Timeline/workspace tests | TASK-005 | COMPLETE |
| REQ-006 | Release verification | Targeted/full Rust and frontend gates, build, clippy, security audit and GitNexus scope review pass | Fresh command log | TASK-006 | BLOCKED_EXTERNAL |

Coverage invariants:

- [x] Every approved Phase 1 requirement has an owner task.
- [x] Every task has measurable acceptance evidence.
- [x] Every cross-layer flow has an integration test owner.
- [x] All in-scope automated evidence is fresh and passing.

## Dependency Graph

| Task | Depends on | Cross-layer boundaries | GitNexus risk |
|---|---|---|---|
| TASK-001 | None | Rust contracts, repository, staging, projection | LOW per symbol |
| TASK-002 | TASK-001 | Tauri IPC, Svelte composer, platform APIs | LOW per file; overall change set CRITICAL |
| TASK-003 | TASK-001 | Rust DTO to Svelte renderer | LOW per file |
| TASK-004 | TASK-001, TASK-002 | Repository projection, Tauri recall, workspace UI | LOW per symbol/file |
| TASK-005 | TASK-002 | Timeline cache, virtualization, optimistic UI | LOW per symbol/file |
| TASK-006 | TASK-001..005 | Full repository | CRITICAL aggregate |

## Task Ledger

### TASK-001: Backend safety and contracts

**Status:** `COMPLETE`

- [x] Inspect staging, projection, contracts and impact.
- [x] Add failing tests for batch-byte limit, redaction, serialization and current-account reactions.
- [x] Implement minimal backend fixes.
- [x] Run targeted Rust QA.

### TASK-002: Rich composer integration

**Status:** `COMPLETE`

- [x] Add failing composer/platform integration tests.
- [x] Wire draft cache, staging, drop, clipboard, emoji, stickers, quote and batch send.
- [x] Run targeted composer tests and typecheck.

### TASK-003: Safe rich renderers

**Status:** `COMPLETE`

- [x] Add failing media helper and renderer tests.
- [x] Add video, audio and link renderers and safe preview helpers.
- [x] Remove frontend local-path contract and fallback usage.
- [x] Run renderer tests and typecheck.

### TASK-004: Actions, recall and reactions

**Status:** `COMPLETE`

- [x] Add failing action, reaction and workspace recall tests.
- [x] Implement shared context/dropdown actions, reaction popover and recall dialog.
- [x] Wire exact quote selection and recall cache replacement.
- [x] Run targeted action/workspace tests.

### TASK-005: Batch grouping and optimistic merge

**Status:** `COMPLETE`

- [x] Add failing grouping, boundary and batch status tests.
- [x] Implement grouping helpers and `MessageBatchGroup`.
- [x] Replace single-message optimistic callback with batch merge.
- [x] Run timeline/workspace tests.

### TASK-006: Release verification

**Status:** `BLOCKED_EXTERNAL`

- [x] Run formatting, targeted and full Rust gates.
- [x] Run targeted/full frontend tests, check and build.
- [x] Run security searches and inspect results.
- [x] Run final GitNexus detect-changes audit.
- [ ] Record unavailable live-Zalo evidence as a waiver if no test account is accessible.

## QA Evidence Log

| Evidence ID | Gate | Command | Exit | Result |
|---|---|---|---:|---|
| EV-001 | GitNexus refresh | `bunx gitnexus analyze` | 0 | PASS; indexed `8bf0842`, 3991 nodes, 9792 edges, 300 flows |
| EV-002 | Aggregate impact | GitNexus compare against `14ba16e` | 0 | CRITICAL; 646 symbols, 327 files, 80 affected flows |
| EV-003 | Per-symbol impact | Staging/contracts/projection/Svelte files | 0 | LOW; no HIGH/CRITICAL individual symbol found |
| EV-004 | Rust formatting | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | 0 | PASS |
| EV-005 | Rust tests | vendored `zca-rs`, `conversation::`, full crate | 0 | PASS; `18 + 1`, `113`, and `299` tests passed; expected ignored tests remained ignored |
| EV-006 | Frontend tests | targeted and full browser/Svelte suites | 0 | PASS; `135` targeted and `218` full tests passed; one unrelated smoke test skipped |
| EV-007 | Frontend static/build | `bun run check`, `bun run build` | 0 | PASS; `0 errors`, `0 warnings`, production build emitted successfully |
| EV-008 | Security/diff hygiene | unsafe URL/path, hard-coded composer gate, excluded feature searches, `git diff --check` | 0 | PASS |
| EV-009 | Pixel parity | Chrome DevTools at `1920x937` | 0 | PASS; composer, textarea, send button and account trigger all measured `0px` diff from baseline |
| EV-010 | Clippy scope | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | 101 | Conversation/Zalo clean; full command remains blocked by four pre-existing Outreach warnings in `src/commands.rs` |
| EV-011 | GitNexus final audit | `detect_changes(scope: all)` | 0 | CRITICAL aggregate blast radius; 349 changed symbols, 40 tracked files, 44 affected flows |
| EV-012 | Live Zalo probe | required environment inspection | N/A | BLOCKED; `RUSTSALE_ZALO_PROBE_IMEI`, `RUSTSALE_ZALO_PROBE_USER_AGENT`, and `RUSTSALE_ZALO_PROBE_COOKIES_JSON` are unset |

## Completion Lock

- [x] Requirement coverage is verified by fresh evidence.
- [x] All in-scope automated task QA gates pass after the final edit.
- [x] Full release QA runs after the final edit.
- [x] GitNexus final scope is recorded and reviewed.
- [ ] Live Zalo smoke gate passes or has an explicit user-approved waiver.

**Final eligible status:** `BLOCKED — LIVE ZALO CREDENTIALS OR USER-APPROVED WAIVER REQUIRED`
