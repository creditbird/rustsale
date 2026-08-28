# WhatsApp Media Preview Remediation Ledger

## Control Header

| Field | Value |
|---|---|
| Work item | WhatsApp conversation image preview remediation |
| Approved specification | `docs/superpowers/plans/2026-08-28-whatsapp-media-preview-remediation.md` |
| Repository/worktree | `/home/arch/Project/test/RustSale_v2` on `master` |
| Started at | 2026-08-28 |
| Current phase | BASELINE / RED |
| Overall status | NOT READY |
| Pre-existing changes boundary | Dirty worktree supplied by the user; all files listed by the baseline `git status --short` predate this ledger. This remediation must preserve and separately account for them. |

## Requirement Coverage Matrix

| Requirement ID | Owner task | Planned evidence | Status |
|---|---|---|---|
| WA-MEDIA-001 | TASK-001 | Protobuf extraction and initial SQLite ingest tests | IN PROGRESS |
| WA-MEDIA-002 | TASK-001 | Changed-URL and rich-to-poor redelivery tests | IN PROGRESS |
| WA-MEDIA-003 | TASK-002 | Legacy migration/reconciliation tests, second-run check | PENDING |
| WA-MEDIA-004 | TASK-003 | Command tests for all resolution/error/size branches | PENDING |
| WA-MEDIA-005 | TASK-004 | Svelte tests, `bun run check` | PENDING |
| WA-MEDIA-006 | TASK-005 | Cross-channel/full suites, diff and GitNexus audit | PENDING |

## Dependency Graph

| Task | Depends on | Cross-layer boundary | Pre-edit GitNexus risk |
|---|---|---|---|
| TASK-001 — canonical media upsert | None | WhatsApp protobuf → normalizer → SQLite | LOW |
| TASK-002 — legacy reconciliation | TASK-001 | startup migration → existing SQLite rows | CRITICAL through `migrate`; user warned before edit |
| TASK-003 — resolver contract/cache | TASK-001 | Tauri command → WhatsApp client → SQLite | LOW |
| TASK-004 — renderer behavior | TASK-003 | Tauri rejection → Svelte state → image viewer | LOW |
| TASK-005 — integration audit | TASK-001..004 | all conversation channels | To be measured |

## Task Ledger

### TASK-001: Canonical media ingest/upsert

**Status:** `IN PROGRESS`

- [x] Trace live/history ingestion and protobuf extraction.
- [x] Run pre-edit impact analysis.
- [ ] Record RED tests for changed CDN URL and metadata non-regression.
- [ ] Implement one canonical, lossless attachment upsert path.
- [ ] Run targeted normalizer tests.

### TASK-002: Legacy reconciliation

**Status:** `PENDING`

- [ ] Add recoverable/unrecoverable and idempotency tests.
- [ ] Preserve `direct_path` before clearing polluted `local_path`.
- [ ] Backfill non-empty raw-payload metadata without swallowing SQL errors.
- [ ] Run migration and cross-channel regression tests.

### TASK-003: Bounded resolver and typed errors

**Status:** `PENDING`

- [ ] Add stable error-code and resource-boundary tests.
- [ ] Normalize and validate encryption metadata.
- [ ] Bound local and provider payload size before data-URL encoding.
- [ ] Persist successful preview and emit invalidation.

### TASK-004: Image renderer

**Status:** `PENDING`

- [ ] Add structured-error and retry-classification tests.
- [ ] Render permanent missing-key guidance without repeated calls.
- [ ] Retain transient manual retry and modal open behavior.

### TASK-005: Verification and hostile integration audit

**Status:** `PENDING`

- [ ] Run targeted and full frontend/backend gates.
- [ ] Inspect the live SQLite attachment population read-only.
- [ ] Verify no Zalo path changed or regressed.
- [ ] Run `gitnexus_detect_changes` and reconcile all affected flows.
- [ ] Record Completion Lock status.

## QA Evidence Log

| Evidence ID | Gate | Command/check | Result |
|---|---|---|---|
| EV-001 | Agent memory | `memory_recall` | PASS — prior WhatsApp session/runtime context reviewed |
| EV-002 | Mandatory harness | Read `docs/IMPLEMENTATION_COMPLETION_HARNESS.md` | PASS |
| EV-003 | Skills | Read `diagnosing-bugs`, `gitnexus-debugging`, `enterprise-implementation-harness`, `rust-base` | PASS |
| EV-004 | Baseline worktree | `git status --short --branch` | PASS — dirty pre-existing boundary recorded |
| EV-005 | GitNexus refresh | `bunx gitnexus analyze` | PASS — index reports current committed revision; uncommitted source cross-checked directly |
| EV-006 | Pre-edit impact | `ingest_message_transactional`, attachment command, extractor, renderer, migration entry point | PASS — LOW except CRITICAL migration entry point; warning issued |

## Adversarial Audit Queue

- [ ] Same message, same media type, refreshed CDN URL: no duplicate row.
- [ ] Rich metadata followed by sparse redelivery: no secret/hash loss.
- [ ] Sparse metadata followed by rich redelivery: canonical row becomes viewable.
- [ ] Legacy CDN path in `local_path`: copied to `direct_path`, then cleared.
- [ ] Raw payload with key: backfilled; raw payload without key: honest terminal error.
- [ ] Invalid base64 or non-32-byte media key: typed permanent metadata error.
- [ ] Oversized local/provider bytes: bounded typed failure, no DB data URL.
- [ ] Transient provider failure: retry remains available.
- [ ] Zalo HTTP/data preview path: unchanged.

## Completion Lock

- [ ] Every requirement has fresh passing evidence.
- [ ] Every in-scope test passes after the final edit.
- [ ] Full relevant QA passes after the final edit.
- [ ] No swallowed persistence failures remain in the changed flow.
- [ ] Final GitNexus scope is reviewed and d=1 dependents are covered.
- [ ] Current live-data limitation is reported truthfully.

**Final eligible status:** `NOT READY`
