# WhatsApp Media Preview Remediation Ledger

## Control Header

| Field | Value |
|---|---|
| Work item | WhatsApp conversation image preview remediation |
| Approved specification | `docs/superpowers/plans/2026-08-28-whatsapp-media-preview-remediation.md` |
| Repository/worktree | `/home/arch/Project/test/RustSale_v2` on `master` |
| Started at | 2026-08-28 |
| Current phase | FINAL VERIFICATION |
| Overall status | CONDITIONALLY READY |
| Pre-existing changes boundary | Dirty worktree supplied by the user; all files listed by the baseline `git status --short` predate this ledger. This remediation must preserve and separately account for them. |

## Requirement Coverage Matrix

| Requirement ID | Owner task | Planned evidence | Status |
|---|---|---|---|
| WA-MEDIA-001 | TASK-001 | Protobuf extraction and initial SQLite ingest tests | PASS |
| WA-MEDIA-002 | TASK-001 | Changed-URL and rich-to-poor redelivery tests | PASS |
| WA-MEDIA-003 | TASK-002 | Legacy migration/reconciliation tests, second-run check | PASS |
| WA-MEDIA-004 | TASK-003 | Command tests for all resolution/error/size branches | PASS |
| WA-MEDIA-005 | TASK-004 | Svelte tests, `bun run check` | PASS |
| WA-MEDIA-006 | TASK-005 | Cross-channel/full suites, diff and GitNexus audit | PASS |
| WA-MEDIA-007 | TASK-006 | Low-resolution-to-full-resolution upgrade tests and live DB audit | PASS |

## Dependency Graph

| Task | Depends on | Cross-layer boundary | Pre-edit GitNexus risk |
|---|---|---|---|
| TASK-001 — canonical media upsert | None | WhatsApp protobuf → normalizer → SQLite | LOW |
| TASK-002 — legacy reconciliation | TASK-001 | startup migration → existing SQLite rows | CRITICAL through `migrate`; user warned before edit |
| TASK-003 — resolver contract/cache | TASK-001 | Tauri command → WhatsApp client → SQLite | LOW |
| TASK-004 — renderer behavior | TASK-003 | Tauri rejection → Svelte state → image viewer | LOW |
| TASK-005 — integration audit | TASK-001..004 | all conversation channels | LOW resolver; MEDIUM shared URL sanitizer |

## Task Ledger

### TASK-001: Canonical media ingest/upsert

**Status:** `COMPLETE`

- [x] Trace live/history ingestion and protobuf extraction.
- [x] Run pre-edit impact analysis.
- [x] Record RED tests for changed CDN URL and metadata non-regression.
- [x] Implement one canonical, lossless attachment upsert path.
- [x] Run targeted normalizer tests.

### TASK-002: Legacy reconciliation

**Status:** `COMPLETE`

- [x] Add recoverable/unrecoverable and idempotency tests.
- [x] Preserve `direct_path` before clearing polluted `local_path`.
- [x] Backfill non-empty raw-payload metadata without swallowing SQL errors.
- [x] Run migration and cross-channel regression tests.

### TASK-003: Bounded resolver and typed errors

**Status:** `COMPLETE`

- [x] Add stable error-code and resource-boundary tests.
- [x] Normalize and validate encryption metadata.
- [x] Bound local and provider payload size before data-URL encoding.
- [x] Persist successful preview and emit invalidation.

### TASK-004: Image renderer

**Status:** `COMPLETE`

- [x] Add structured-error and retry-classification tests.
- [x] Render permanent missing-key guidance without repeated calls.
- [x] Retain transient manual retry and modal open behavior.

### TASK-006: Progressive HD upgrade

**Status:** `COMPLETE`

- [x] Prove the live blurry thumbnail is 100×43 while the declared media is 197,638 bytes.
- [x] Add RED tests for encrypted thumbnail and outbound local-file upgrade.
- [x] Show the thumbnail immediately, then resolve and replace it with full media.
- [x] Mark successful cache entries with `preview_resolved=true` and avoid re-downloading an already-full byte-matching data URL.

### TASK-005: Verification and hostile integration audit

**Status:** `COMPLETE`

- [x] Run targeted and full frontend/backend gates.
- [x] Inspect the live SQLite attachment population read-only.
- [x] Verify no Zalo path changed or regressed.
- [x] Run `gitnexus_detect_changes` and reconcile all affected flows.
- [x] Record Completion Lock status.

## QA Evidence Log

| Evidence ID | Gate | Command/check | Result |
|---|---|---|---|
| EV-001 | Agent memory | `memory_recall` | PASS — prior WhatsApp session/runtime context reviewed |
| EV-002 | Mandatory harness | Read `docs/IMPLEMENTATION_COMPLETION_HARNESS.md` | PASS |
| EV-003 | Skills | Read `diagnosing-bugs`, `gitnexus-debugging`, `enterprise-implementation-harness`, `rust-base` | PASS |
| EV-004 | Baseline worktree | `git status --short --branch` | PASS — dirty pre-existing boundary recorded |
| EV-005 | GitNexus refresh | `bunx gitnexus analyze` | PASS — index reports current committed revision; uncommitted source cross-checked directly |
| EV-006 | Pre-edit impact | `ingest_message_transactional`, attachment command, extractor, renderer, migration entry point | PASS — LOW except CRITICAL migration entry point; warning issued |
| EV-007 | WhatsApp backend | `cargo test --manifest-path src-tauri/Cargo.toml --lib conversation::whatsapp::` | PASS — 63/63 |
| EV-008 | Migration | `cargo test --manifest-path src-tauri/Cargo.toml --lib conversation::migration_test::` | PASS — 22/22 |
| EV-009 | Attachment command | `cargo test --manifest-path src-tauri/Cargo.toml --lib conversation::commands_test::` | PASS — 34/34 after final HD edit |
| EV-010 | Full Rust library | `cargo test --manifest-path src-tauri/Cargo.toml --lib --quiet` | PASS — 756 passed, 1 ignored after the final edit |
| EV-011 | Conversation UI | `bun test src/lib/components/conversation/__tests__/` | PASS — 319/319 after final HD edit |
| EV-012 | Svelte static check | `bun run check` | PASS — 0 errors; 6 pre-existing warnings outside changed conversation files |
| EV-013 | Production frontend | `bun run build` | PASS — build completed; existing chunk-size/Svelte warnings recorded |
| EV-014 | Rust compile | `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | PASS |
| EV-015 | Rust lint | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets` | PASS with 40 existing warnings; strict `-D warnings` remains blocked outside this remediation |
| EV-016 | Live blurry image | Read-only SQLite + ImageMagick audit of message 10616 / attachment 1549 | PASS — stored thumbnail 100×43 / 3,461 bytes; declared full media 197,638 bytes; E2EE key and direct path present |
| EV-017 | Live clear comparison | Read-only SQLite + ImageMagick audit of message 10573 / attachment 1541 | PASS — cached preview 1600×702 / 197,638 bytes |
| EV-018 | Final GitNexus audit | `gitnexus_detect_changes(scope=all)` plus symbol-level upstream impact | PASS — file-level classifier reports CRITICAL because it attributes the whole command/test files; actual resolver impact is LOW, shared URL sanitizer is MEDIUM with five d=1 callers, and all callers are covered by the conversation suite/build |

## Adversarial Audit Queue

- [x] Same message, same media type, refreshed CDN URL: no duplicate row.
- [x] Rich metadata followed by sparse redelivery: no secret/hash loss.
- [x] Sparse metadata followed by rich redelivery: canonical row becomes viewable.
- [x] Legacy CDN path in `local_path`: copied to `direct_path`, then cleared.
- [x] Raw payload with key: backfilled; raw payload without key: honest terminal error.
- [x] Invalid base64 or non-32-byte media key: typed permanent metadata error.
- [x] Oversized local/provider bytes: bounded typed failure, no DB data URL.
- [x] Transient provider failure: retry remains available.
- [x] Zalo HTTP/data preview path: unchanged.
- [x] Low-resolution thumbnail with valid encrypted media: full payload replaces it and is cached.

## Completion Lock

- [x] Every requirement has fresh passing evidence.
- [x] Every in-scope test passes after the final edit.
- [x] Full relevant QA passes after the final edit.
- [x] No swallowed persistence failures remain in the changed flow.
- [x] Final GitNexus scope is reviewed and d=1 dependents are covered.
- [x] Current live-data limitation is reported truthfully.

**Final eligible status:** `CONDITIONALLY READY` — automated gates are green; a live post-restart WhatsApp download remains the external smoke check, and historical rows whose key was never stored remain unrecoverable without resend/redelivery.
