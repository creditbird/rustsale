# Zalo Account & Session Refactor — Production Architecture and Verification

**Project:** `RustSale_v2`  
**Date:** 2026-08-09  
**Scope:** Zalo multi-account, QR re-authentication, session lifecycle, duplicate-account migration, Unified Conversations  
**Status:** `VERIFIED READY` for the Zalo repair scope; repository-wide release remains conditional on separating or independently reviewing unrelated dirty-worktree changes

## 1. Problem and Root Cause

The previous implementation conflated two different domain concepts:

- `channel_accounts.provider_account_key` was sometimes an IMEI generated for a login session.
- A new QR login could therefore create another `channel_accounts` row for the same logical Zalo user.
- Existing conversations remained attached to the old row and old credentials.
- When those credentials became invalid or expired, Zalo returned code `102`, while `Reconnect` retried the obsolete session.

The root cause was not the conversation itself. It was an unstable account identity key.

## 2. Canonical Domain Model

```text
Zalo user UID
  └── ChannelAccount (one logical provider account)
        ├── ZaloSession (IMEI + cookies + auth state)
        └── ChannelConversation (many conversations owned by this account)
              └── Messages and conversation child records
```

### Account and conversation semantics

- One Zalo UID maps to one canonical `ChannelAccount`.
- Different Zalo UIDs map to different `ChannelAccount` rows.
- Each `ChannelConversation` belongs to exactly one `ChannelAccount` and one channel.
- One customer/lead can still have multiple channel identities and reply targets across Zalo, Facebook, WhatsApp, or multiple provider accounts.
- Unified Inbox may aggregate those reply targets for one customer, but it does not make one conversation belong to multiple accounts.
- IMEI is a credential/session identifier only. It is never the canonical identity for a newly authenticated Zalo account.

## 3. Production Invariants

| ID | Invariant | Enforcement |
|---|---|---|
| `INV-001` | A completed QR login must expose a stable Zalo UID before any session is persisted. | `require_zalo_uid` aborts the flow and emits `zalo-qr-error` when UID is missing or blank. |
| `INV-002` | A Zalo UID has at most one logical account row. | Partial unique index `idx_channel_accounts_zalo_self_ext_id`. |
| `INV-003` | A session maps explicitly to its logical account. | `zalo_sessions.zalo_uid` and `zalo_sessions.channel_account_id`. |
| `INV-004` | Runtime task, client registry, status updates, and ingestion use the stable UID. | `ZaloSession::provider_account_key`, `RealZaloClient`, and `ZaloSessionSupervisor`. |
| `INV-005` | Existing conversation history remains attached to the canonical account during deduplication. | Transactional migration deep-merges conversations, messages, and dependent rows before deleting duplicates. |
| `INV-006` | QR success is emitted only after persistence, account reconciliation, and realtime readiness. | Sequential `start_zalo_qr_login` flow and `restart_account` readiness wait. |
| `INV-007` | Failed activation does not leave invalid new state. | Session snapshot restore/delete and removal of an empty newly created account. |

## 4. Implemented Runtime Flow

### 4.1 QR login and re-authentication

1. Complete `zca_rs::Zalo::login_qr`.
2. Resolve UID from `get_own_id()`, account information, or known Zalo UID cookies.
3. Reject the login if no stable UID can be resolved. No session is saved.
4. Capture the current session snapshot for rollback.
5. Persist refreshed IMEI, user agent, and cookies with UPSERT semantics that preserve profile and account mapping fields.
6. Reconcile the session to the canonical account in a repository transaction.
7. Restart the supervisor using the Zalo UID and wait until the connected client is registered.
8. Emit `zalo-qr-success` only after the listener is ready.
9. On reconciliation or restart failure, restore the previous session or delete the new session and remove a new empty account.

### 4.2 Supervisor and ingestion

- `restart_account` resolves UID, IMEI, or mapped `channel_account_id` to the exact stored session.
- Task and connected-client registries use the stable UID whenever available.
- Legacy IMEI sessions can still connect and reconcile themselves once the runtime exposes `own_external_id`.
- Listener events, group snapshots, messages, reactions, receipts, and status updates are persisted under the canonical account key.
- Restart returns an explicit error when no matching session exists, the task stops before readiness, or readiness times out.

### 4.3 Code 102 UX

- `MessageComposer.svelte` detects the expired/invalid Zalo session-key error.
- Code `102` displays `Đăng nhập lại QR` instead of retrying the obsolete session.
- The action is wired through `ConversationWorkspace.svelte`, `+page.svelte`, `Settings.svelte`, and `ZaloAccountsSettings.svelte` to open the actual QR dialog.

## 5. Migration and Data Preservation

`db::init_db` invokes `conversation::migration::migrate` inside a SQLite transaction. The migration performs these steps in order:

1. Create or upgrade `zalo_sessions` before duplicate reconciliation.
2. Add `zalo_uid` and `channel_account_id` mappings.
3. Select a canonical account per Zalo UID, preferring the UID-keyed row, then the row with more conversations, then the lowest ID.
4. Backfill all matching sessions to the canonical account before duplicate deletion.
5. Move unique conversations and deep-merge conflicting conversations sharing `(external_thread_id, kind)`.
6. Merge duplicate messages using provider message ID, provider client message ID, local request ID, or send-batch identity.
7. Preserve the strongest message/account/batch status and richer metadata.
8. Re-link or merge dependent data before deleting duplicate headers.
9. Assert that no account-owned children remain on the duplicate account.
10. Create the UID and session lookup indexes after reconciliation succeeds.

### Preserved or merged dependent tables

- `messages`
- `conversation_participants`
- `conversation_pinned_messages`
- `message_attachments`
- `message_reactions`
- `message_receipts`
- `message_mentions`
- `reaction_mutations`
- `message_forward_operations`
- `message_forward_targets`
- `message_outbox_jobs`
- `message_send_batches`
- `conversation_staged_attachments`
- `channel_sync_checkpoints`

## 6. Requirement Coverage Matrix

| Requirement | Implementation owner | Regression evidence |
|---|---|---|
| `REQ-001` Same Zalo user reuses one account ID. | `ConversationRepository::reconcile_zalo_channel_account` | `mapped_zalo_imei_reuses_canonical_account_for_messages_groups_and_mutations` |
| `REQ-002` New QR login cannot create an IMEI-keyed account when UID is unavailable. | `require_zalo_uid`, `start_zalo_qr_login` | `test_require_zalo_uid_rejects_missing_identity_and_normalizes_valid_uid` |
| `REQ-003` Legacy schema upgrades before reconciliation. | `ensure_zalo_session_schema`, `migrate` | `legacy_zalo_session_schema_upgrades_before_duplicate_reconciliation` |
| `REQ-004` Duplicate migration preserves conflicting state and dependents. | `reconcile_duplicate_zalo_channel_accounts` and merge helpers | `duplicate_zalo_merge_preserves_conflicting_state_and_all_dependents` |
| `REQ-005` Runtime uses stable UID for registration and ingestion. | `ZaloSessionSupervisor`, `RealZaloClient` | `mapped_session_registers_and_ingests_under_stable_zalo_uid` |
| `REQ-006` Restart fails deterministically when session or readiness is unavailable. | `ZaloSessionSupervisor::restart_account` | `restart_account_fails_when_no_session_matches_account_key`, `restart_account_returns_error_when_runtime_never_becomes_ready` |
| `REQ-007` Credential refresh preserves profile and account mapping. | `db::save_zalo_session` | `saving_refreshed_zalo_credentials_preserves_profile_and_account_mapping` |
| `REQ-008` Failed QR activation rolls persistence back. | session snapshot and rollback helpers | `test_zalo_qr_persistence_rollback_restores_existing_session_or_removes_new_one` |
| `REQ-009` Code 102 opens direct QR re-authentication. | composer-to-settings QR action binding | `Zalo code 102 opens direct QR re-authentication for the selected account` |

## 7. Production-Copy Migration Audit

Audit date: **2026-08-09**. The source production database was opened with `sqlite3 -readonly`; migration ran only on `/tmp/rustsale-zalo-production-audit-1786250474.db`.

### Before and after

| Metric | Production source | Migrated copy | Interpretation |
|---|---:|---:|---|
| Zalo accounts | 2 | 1 | Duplicate UID accounts consolidated. |
| Conversations | 517 | 347 | Duplicate conversation headers merged. |
| Messages | 2,989 | 2,981 | Eight logical duplicate rows consolidated. |
| Conversation participants | 10,256 | 5,372 | Duplicate memberships consolidated by canonical conversation and identity. |
| Message receipts | 89 | 89 | Preserved. |
| Message attachments | 310 | 310 | Preserved. |
| Message reactions | 270 | 270 | Preserved. |
| Reaction mutations | 8 | 8 | Preserved. |
| Forward operations / targets | 3 / 3 | 3 / 3 | Preserved. |
| Outbox jobs | 90 | 90 | Preserved. |
| Send batches | 79 | 79 | Preserved. |
| Staged attachments | 2 | 2 | Preserved. |
| Sync checkpoints | 17 | 9 | Duplicate scopes merged under the canonical account. |

### Post-migration assertions

- Canonical account: `id=1`, UID/key `1503088233761505331`, status `connected`.
- Active session IMEI maps to UID `1503088233761505331` and `channel_account_id=1` with auth status `valid`.
- Provider message IDs `8131771818726`, `8131771897628`, and `8131773139962` retain final status `read`.
- `PRAGMA foreign_key_check` returns `0` violations.
- `idx_channel_accounts_zalo_self_ext_id`, `idx_zalo_sessions_zalo_uid`, and `idx_zalo_sessions_account_id` exist.
- The production source remains pre-migration and unchanged. SHA-256: `dabca7b39b5784686957db9f2af3e89a827769db874fa2b1d5bc0fb556554e8f`.

## 8. Release QA Evidence

| Gate | Command | Result |
|---|---|---|
| Rust formatting | `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | PASS, exit `0`. |
| Rust strict lint | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | PASS, exit `0`. |
| Rust library tests | `cargo test --manifest-path src-tauri/Cargo.toml --lib --quiet` | PASS: `409 passed`, `0 failed`, `1 ignored`. |
| Rust full tests | `cargo test --manifest-path src-tauri/Cargo.toml --quiet` | PASS: `409 passed`, `0 failed`, `1 ignored`; remaining targets contain `0` tests. |
| Critical supervisor flows | `cargo test --manifest-path src-tauri/Cargo.toml --lib supervisor_test --quiet` | PASS: `12 passed`, `0 failed`. |
| Frontend tests | `bun test --conditions=browser --conditions=svelte` | PASS: `341 passed`, `0 failed`, `1 skipped`, `46` files. |
| Svelte diagnostics | `bun run check` | PASS: `0 errors`, `0 warnings`. |
| Production frontend build | `bun run build` | PASS, static build written to `build`. |

## 9. GitNexus Scope Audit

`gitnexus_detect_changes({ repo: "RustSale_v2", scope: "all" })` reports `415` changed symbols, `145` affected symbols, `39` changed files, and repository-level risk `CRITICAL`.

The repository-level rating covers the entire pre-existing dirty worktree, not only this Zalo repair. The Zalo supervisor symbols `run_session_loop`, `start_account_task`, `mark_account_degraded`, and `unregister_connected_client` are individually `CRITICAL` because they participate in five realtime execution flows.

Reviewed d=1 dependents:

- `run_session_loop`: `start_account_task`, `restart_account`
- `start_account_task`: `start_all`
- `mark_account_degraded`: `run_session_loop`
- `unregister_connected_client`: `run_session_loop`

The dedicated supervisor suite passes `12/12`, including the five affected history, reaction, packet-error, persistence, and invalidation flows. The Zalo repair scope is therefore verified, but the unrelated worktree changes should not be shipped as one unreviewed change set.

## 10. Deployment and Recovery

- Do not modify the live database manually.
- Back up the database before first launch of the patched build.
- The patched application applies the migration through `db::init_db` on startup.
- If startup migration fails, the surrounding SQLite transaction rolls back.
- Keep the backup until account counts, conversation counts, session mapping, and `PRAGMA foreign_key_check` are verified in the deployed environment.

## 11. Worktree Boundary

The repository contained unrelated pre-existing modifications before this repair. This implementation does not revert or rewrite those changes. A worktree-wide `git diff --check` still reports extra EOF blank lines in seven unrelated attachment-rendering Svelte components; the Zalo production files and Rust formatter are clean.
