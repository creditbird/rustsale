# Zalo Rich Chat Phase 2A Interactions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add verified Zalo reaction writes, provider-native multi-target Forward, local slash quick templates, and validated group mentions to the capability-driven Conversation workspace.

**Architecture:** Extend the Phase 1 Conversation contracts rather than creating a parallel chat subsystem. Reaction writes use a synchronous provider call plus a durable no-auto-retry mutation ledger, Forward creates one existing-style batch/outbox part per target, quick templates use focused local SQLite storage, and mentions are normalized message children with backend-validated UTF-16 ranges.

**Tech Stack:** Rust 2021, Tauri 2, SQLite/rusqlite, vendored `zca-rs`, Tokio, Svelte 5 runes, TypeScript, shadcn-svelte, Bun test runner, Testing Library, Tailwind CSS 4.

---

## Source of Truth

- Approved Phase 2A design: `docs/superpowers/specs/2026-08-02-zalo-rich-chat-phase-2a-interactions-design.md`
- Required Phase 1 design: `docs/superpowers/specs/2026-08-02-zalo-rich-chat-phase-1-design.md`
- Required Phase 1 plan: `docs/superpowers/plans/2026-08-02-zalo-rich-chat-phase-1-implementation-plan.md`

Do not execute this plan until Phase 1 implementation and acceptance gates are complete. Phase 2A assumes Phase 1 types, files, migrations, batch worker, rich projection, reactions, and message-action components exist.

## Execution Rules

- Use `bun` and `bunx` only. Never run `npm` or `npx`.
- Read Agent Memory before implementation.
- Use `superpowers:using-git-worktrees` if work is isolated.
- Run GitNexus impact before editing every existing symbol; warn and stop on HIGH or CRITICAL risk.
- Run `mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})` at provider, backend, frontend, and final checkpoints.
- Preserve unrelated working-tree changes.
- Use `apply_patch` for manual edits.
- Do not commit unless the user explicitly requests it. Review checkpoints replace commit steps.
- Keep provider probes sanitized: no cookies, tokens, phone numbers, or unrelated message bodies.

## Planning-Time GitNexus Snapshot

| Symbol | Risk | Direct dependents | Execution note |
|---|---:|---:|---|
| `migrate` | LOW | 0 | Re-run before schema edits |
| `ZaloClient` | LOW | 1 | `RealZaloClient`; test fakes also require updates |
| `apply_reaction` | LOW | 0 indexed | Service and tests still need direct inspection |
| `process_next_outbox_job` | LOW | 0 indexed | Phase 1 tests call it directly |
| `ConversationMessage` | LOW | 1 | `timelineState.ts`; Svelte consumers are not fully indexed |

Re-run every impact during execution because Phase 1 may change the graph.

## Required File Map

### Backend files to create

```text
src-tauri/src/conversation/quick_templates.rs
src-tauri/src/conversation/quick_templates_test.rs
src-tauri/src/conversation/mentions.rs
src-tauri/src/conversation/mentions_test.rs
src-tauri/src/conversation/reactions.rs
src-tauri/src/conversation/reactions_test.rs
src-tauri/src/conversation/forwarding.rs
src-tauri/src/conversation/forwarding_test.rs
src-tauri/src/conversation/zalo/phase2a_probe.rs
src-tauri/src/conversation/zalo/phase2a_probe_test.rs
src-tauri/src/conversation/zalo/fixtures/phase2a/reaction_add_heart.json
src-tauri/src/conversation/zalo/fixtures/phase2a/reaction_add_haha.json
src-tauri/src/conversation/zalo/fixtures/phase2a/reaction_remove_heart.json
src-tauri/src/conversation/zalo/fixtures/phase2a/reaction_multi_active.json
src-tauri/src/conversation/zalo/fixtures/phase2a/reaction_old_reactions.json
src-tauri/src/conversation/zalo/fixtures/phase2a/forward_text_echo.json
src-tauri/src/conversation/zalo/fixtures/phase2a/forward_content_matrix.json
src-tauri/src/conversation/zalo/fixtures/phase2a/mention_vietnamese_emoji.json
src-tauri/src/conversation/zalo/fixtures/phase2a/mention_utf16_matrix.json
```

### Backend files to modify

```text
src-tauri/src/lib.rs
src-tauri/src/conversation/mod.rs
src-tauri/src/conversation/schema.sql
src-tauri/src/conversation/migration.rs
src-tauri/src/conversation/migration_test.rs
src-tauri/src/conversation/types.rs
src-tauri/src/conversation/error.rs
src-tauri/src/conversation/adapter.rs
src-tauri/src/conversation/repository.rs
src-tauri/src/conversation/repository_test.rs
src-tauri/src/conversation/projection.rs
src-tauri/src/conversation/service.rs
src-tauri/src/conversation/service_test.rs
src-tauri/src/conversation/commands.rs
src-tauri/src/conversation/commands_test.rs
src-tauri/src/conversation/zalo/mod.rs
src-tauri/src/conversation/zalo/client.rs
src-tauri/src/conversation/zalo/client_test.rs
src-tauri/src/conversation/zalo/normalizer.rs
src-tauri/src/conversation/zalo/normalizer_test.rs
src-tauri/vendor/zca-rs/src/models/message.rs
src-tauri/vendor/zca-rs/src/apis/message.rs
```

### Frontend files to create

```text
src/lib/components/conversation/quickTemplateState.ts
src/lib/components/conversation/mentionState.ts
src/lib/components/conversation/forwardState.ts
src/lib/components/conversation/QuickTemplatePopover.svelte
src/lib/components/conversation/QuickTemplateManagerDialog.svelte
src/lib/components/conversation/QuickTemplateForm.svelte
src/lib/components/conversation/MentionCandidatePopover.svelte
src/lib/components/conversation/MessageMentionText.svelte
src/lib/components/conversation/MessageReactionPicker.svelte
src/lib/components/conversation/MessageReactionMutationState.svelte
src/lib/components/conversation/ForwardConversationDialog.svelte
src/lib/components/conversation/ForwardTargetResultList.svelte
src/lib/components/conversation/__tests__/quickTemplateState.spec.ts
src/lib/components/conversation/__tests__/mentionState.spec.ts
src/lib/components/conversation/__tests__/forwardState.spec.ts
src/lib/components/conversation/__tests__/QuickTemplatePopover.spec.ts
src/lib/components/conversation/__tests__/QuickTemplateManagerDialog.spec.ts
src/lib/components/conversation/__tests__/MentionCandidatePopover.spec.ts
src/lib/components/conversation/__tests__/MessageMentionText.spec.ts
src/lib/components/conversation/__tests__/MessageReactionPicker.spec.ts
src/lib/components/conversation/__tests__/ForwardConversationDialog.spec.ts
```

### Frontend files to modify

```text
src/lib/services/conversationService.ts
src/lib/services/__tests__/conversationService.spec.ts
src/lib/components/conversation/composerDraftState.ts
src/lib/components/conversation/MessageComposer.svelte
src/lib/components/conversation/MessageBubble.svelte
src/lib/components/conversation/MessageReactionSummary.svelte
src/lib/components/conversation/MessageActionsMenu.svelte
src/lib/components/conversation/ConversationTimeline.svelte
src/lib/components/conversation/ConversationWorkspace.svelte
src/lib/components/conversation/__tests__/composerDraftState.spec.ts
src/lib/components/conversation/__tests__/MessageComposer.spec.ts
src/lib/components/conversation/__tests__/MessageBubble.spec.ts
src/lib/components/conversation/__tests__/MessageReactionSummary.spec.ts
src/lib/components/conversation/__tests__/MessageActionsMenu.spec.ts
src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
```

---

## Task 0: Verify Phase 1 and Establish Baseline

**Files:** None.

- [ ] **Step 1: Read memory and inspect worktree**

```text
mcp__agentmemory.memory_recall({query: "RustSale_v2 Zalo rich chat Phase 1 Phase 2A reactions forward templates mentions decisions", format: "narrative", limit: 10})
```

```bash
git status --short
git log -12 --oneline --decorate
```

Record every pre-existing modified/untracked file. Never overwrite or stage it.

- [ ] **Step 2: Verify Phase 1 artifacts**

```bash
test -f src-tauri/src/conversation/projection.rs
test -f src-tauri/src/conversation/retry.rs
test -f src/lib/components/conversation/MessageReactionSummary.svelte
test -f src/lib/components/conversation/MessageActionsMenu.svelte
test -f src/lib/components/conversation/composerDraftState.ts
rg -n "send_conversation_batch|provider_client_message_id|ConversationCapabilities" src-tauri/src/conversation src/lib/services src/lib/components/conversation
```

Expected: every command succeeds. Otherwise stop and execute Phase 1 first.

- [ ] **Step 3: Run Phase 1 regression baseline**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::
bun test --conditions=browser --conditions=svelte src/lib/services/__tests__/conversationService.spec.ts src/lib/components/conversation/__tests__
bun run check
```

Expected: PASS. Document unrelated failures exactly before continuing.

- [ ] **Step 4: Re-run impact gates**

Run impact for `migrate`, `ZaloClient`, `apply_reaction`, `process_next_outbox_job`, `ConversationMessage`, `ConversationService`, and every existing Svelte file before its first edit. Stop and warn on HIGH/CRITICAL.

---

## Task 1: Capture the Phase 2A Provider Contract

**Files:**
- Create: `src-tauri/src/conversation/zalo/phase2a_probe.rs`
- Create: `src-tauri/src/conversation/zalo/phase2a_probe_test.rs`
- Create: `src-tauri/src/conversation/zalo/fixtures/phase2a/reaction_add_heart.json`
- Create: `src-tauri/src/conversation/zalo/fixtures/phase2a/reaction_add_haha.json`
- Create: `src-tauri/src/conversation/zalo/fixtures/phase2a/reaction_remove_heart.json`
- Create: `src-tauri/src/conversation/zalo/fixtures/phase2a/reaction_multi_active.json`
- Create: `src-tauri/src/conversation/zalo/fixtures/phase2a/reaction_old_reactions.json`
- Create: `src-tauri/src/conversation/zalo/fixtures/phase2a/forward_text_echo.json`
- Create: `src-tauri/src/conversation/zalo/fixtures/phase2a/forward_content_matrix.json`
- Create: `src-tauri/src/conversation/zalo/fixtures/phase2a/mention_vietnamese_emoji.json`
- Create: `src-tauri/src/conversation/zalo/fixtures/phase2a/mention_utf16_matrix.json`
- Modify: `src-tauri/src/conversation/zalo/mod.rs`

- [ ] **Step 1: Write failing fixture-contract tests**

| Test | Required assertion |
|---|---|
| `heart_fixture_is_classified_as_add` | code `heart`, `removed = false` |
| `haha_fixture_is_not_classified_as_remove` | code `haha`, `removed = false` |
| `remove_fixture_identifies_exact_reaction` | code `heart`, `removed = true` |
| `multi_active_fixture_preserves_other_reaction` | removing one leaves the other active |
| `old_reactions_fixture_round_trips_actor_state` | authoritative post-mutation list normalizes exactly |
| `forward_fixture_exposes_provider_and_client_ids` | both IDs non-empty; source reference survives |
| `forward_matrix_classifies_every_content_type` | text/image/video/audio-voice/file/sticker/link each records verified support and echo shape |
| `mention_fixture_uses_utf16_offsets` | range selects exact `@Tên😀` token |
| `mention_matrix_covers_required_unicode_cases` | ASCII, combining, precomposed, emoji-before, emoji-around, and multiple mentions |
| `phase2a_probe_fixtures_are_sanitized` | no cookies, tokens, phone numbers, or unrelated bodies |

- [ ] **Step 2: Run fixture tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2a_probe_test
```

Expected: FAIL because parser/classifier functions do not exist.

- [ ] **Step 3: Add exact probe helpers**

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Phase2AProbeSummary {
    pub reaction_add_verified: bool,
    pub reaction_remove_verified: bool,
    pub forwardable_content_types: Vec<String>,
    pub mention_offsets_are_utf16: bool,
}

pub fn classify_reaction_fixture(value: &serde_json::Value) -> anyhow::Result<ProbeReaction>;
pub fn utf16_slice(value: &str, start: usize, len: usize) -> Option<String>;
```

Classification uses the complete payload contract, never `r_type == 0` alone.

- [ ] **Step 4: Run sanitized live probe**

Use a dedicated non-production account to capture direct/group Heart, Haha, exact remove, two simultaneous reactions, removing one while preserving another, and old-reaction snapshots after every mutation. Probe native Forward for text, image, video, audio/voice, file, sticker, and normalized link with one direct and one group target per call. Probe ASCII, combining/precomposed Vietnamese, emoji-before/around, and multiple mentions. Store only sanitized request/response/realtime/history metadata. If exact per-reaction removal or UTF-16 offsets cannot be proven, stop and amend the spec.

- [ ] **Step 5: Run fixture tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2a_probe_test
```

Expected: PASS.

---

## Task 2: Extend Vendored Zalo Mutation Contracts

**Files:**
- Modify: `src-tauri/vendor/zca-rs/src/models/message.rs`
- Modify: `src-tauri/vendor/zca-rs/src/apis/message.rs`

- [ ] **Step 1: Run provider-symbol impact/context**

Inspect `add_reaction`, `forward_message`, `MessageMention`, and `MessageContent`. Stop on HIGH/CRITICAL.

- [ ] **Step 2: Write failing vendored tests**

| Test | Required assertion |
|---|---|
| `phase2a_reaction_request_uses_caller_client_id` | exact `react_list[0].clientId` |
| `phase2a_reaction_remove_matches_probe` | exact removal type/source/icon |
| `phase2a_forward_direct_uses_one_target` | one `toIds` entry and supplied ID |
| `phase2a_forward_group_uses_one_target` | one `grids` entry and supplied ID |
| `phase2a_forward_response_normalizes_ids` | message/client IDs exposed |
| `phase2a_mentions_serialize_utf16_ranges` | exact UID/pos/len |

- [ ] **Step 3: Run vendored tests RED**

```bash
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml phase2a_
```

Expected: FAIL.

- [ ] **Step 4: Add request and receipt types**

```rust
pub struct ReactionRequest {
    pub client_id: String,
    pub reaction: ReactionType,
    pub msg_id: String,
    pub cli_msg_id: String,
    pub thread_id: String,
    pub thread_type: ThreadType,
}

pub struct ForwardMessageRequest {
    pub client_id: String,
    pub message: String,
    pub thread_id: String,
    pub thread_type: ThreadType,
    pub reference: ForwardReference,
    pub ttl: Option<u32>,
}

pub struct ForwardMessageReceipt {
    pub msg_id: String,
    pub cli_msg_id: String,
}
```

Expose `set_reaction(request)` and `forward_message_one(request)`. Keep current methods as compatibility wrappers where required.

- [ ] **Step 5: Implement pure payload builders**

Move JSON construction into testable functions. Never replace a caller-supplied client ID.

- [ ] **Step 6: Run vendored tests GREEN**

```bash
cargo fmt --manifest-path src-tauri/vendor/zca-rs/Cargo.toml -- --check
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml phase2a_
```

Expected: PASS.

- [ ] **Step 7: Provider checkpoint**

```text
mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})
```

---

## Task 3: Add Phase 2A Contracts, Capabilities, and Errors

**Files:**
- Modify: `src-tauri/src/conversation/types.rs`
- Modify: `src-tauri/src/conversation/error.rs`
- Modify: `src-tauri/src/conversation/adapter.rs`
- Modify: `src-tauri/src/conversation/service_test.rs`

- [ ] **Step 1: Run shared-contract impact**

Run impact for `ConversationCapabilities`, `ConversationMessage`, `ConversationSendPartRequest`, `ConversationAdapter`, and `ConversationErrorCode`.

- [ ] **Step 2: Write failing serialization/capability tests**

Assert exact camelCase JSON for reaction, Forward, template, and Mention contracts. Verify Zalo 2A exposes six reactions, max 10 Forward targets, max 20 mentions, and max 500 templates; text-only channels expose none.

- [ ] **Step 3: Run contract tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2a_contract
```

Expected: FAIL.

- [ ] **Step 4: Add exact domain types**

```rust
pub enum ConversationReactionCode { Heart, Like, Haha, Wow, Cry, Angry }

pub struct ConversationMentionInput {
    pub identity_id: i64,
    pub display_text: String,
    pub start_utf16: usize,
    pub length_utf16: usize,
}

pub struct SetConversationReactionRequest {
    pub message_id: i64,
    pub reaction: ConversationReactionCode,
    pub active: bool,
    pub client_mutation_id: String,
}

pub struct ForwardConversationMessageRequest {
    pub source_message_id: i64,
    pub target_conversation_ids: Vec<i64>,
    pub client_operation_id: String,
}

pub enum ReactionMutationStatus { Running, Succeeded, Failed, DeliveryUnknown }
pub enum ForwardTargetStatus { Queued, Sending, Sent, Failed, DeliveryUnknown }
pub enum ForwardOperationStatus { Queued, Sending, Partial, Sent, Failed, DeliveryUnknown }

pub struct ConversationReactionMutationResult {
    pub client_mutation_id: String,
    pub message_id: i64,
    pub reaction: ConversationReactionCode,
    pub requested_active: bool,
    pub status: ReactionMutationStatus,
    pub reactions: Vec<ConversationReactionSummary>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

pub struct ConversationForwardTargetResult {
    pub target_conversation_id: i64,
    pub client_batch_id: String,
    pub status: ForwardTargetStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

pub struct ForwardConversationMessageResult {
    pub client_operation_id: String,
    pub source_message_id: i64,
    pub status: ForwardOperationStatus,
    pub targets: Vec<ConversationForwardTargetResult>,
}

pub struct ConversationQuickTemplate {
    pub id: i64,
    pub shortcut: String,
    pub title: String,
    pub body_text: String,
    pub category: Option<String>,
    pub is_active: bool,
    pub usage_count: i64,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct CreateConversationQuickTemplateRequest {
    pub shortcut: String,
    pub title: String,
    pub body_text: String,
    pub category: Option<String>,
}

pub struct UpdateConversationQuickTemplateRequest {
    pub id: i64,
    pub shortcut: String,
    pub title: String,
    pub body_text: String,
    pub category: Option<String>,
    pub is_active: bool,
}

pub struct ListConversationQuickTemplatesRequest {
    pub include_inactive: Option<bool>,
}

pub struct DeleteConversationQuickTemplateRequest {
    pub id: i64,
}

pub struct MarkConversationQuickTemplateUsedRequest {
    pub id: i64,
}

pub struct ListConversationMentionCandidatesRequest {
    pub conversation_id: i64,
    pub query: String,
    pub limit: Option<usize>,
}

pub struct RetryConversationForwardTargetRequest {
    pub client_operation_id: String,
    pub target_conversation_id: i64,
}

pub struct ConversationMention {
    pub identity_id: Option<i64>,
    pub external_id: String,
    pub display_text: String,
    pub start_utf16: usize,
    pub length_utf16: usize,
}

pub struct ConversationMentionCandidate {
    pub identity_id: i64,
    pub external_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub participant_role: String,
}

pub struct ConversationForwardCapabilities {
    pub enabled: bool,
    pub max_targets: usize,
}

pub struct ConversationMentionCapabilities {
    pub enabled: bool,
    pub max_per_message: usize,
}

pub struct ConversationQuickTemplateCapabilities {
    pub enabled: bool,
    pub max_templates: usize,
}

pub struct ConversationMessageActions {
    pub reply: bool,
    pub copy: bool,
    pub recall: bool,
    pub forward: bool,
    pub react: bool,
}
```

Extend `ConversationReactionCapabilities` with `supported: Vec<ConversationReactionCode>`. Extend `ConversationCapabilities` with `forward`, `mentions`, and `quick_templates`. Extend `ConversationMessage` with `mentions: Vec<ConversationMention>` and `actions: ConversationMessageActions`. Extend the text part with `mentions: Vec<ConversationMentionInput>` and add a backend-only `Forward` part. If Phase 2B already exists, preserve `voice_recording`, `media_library`, voice send parts, and voice attachment metadata while merging these fields.

- [ ] **Step 5: Add stable errors and adapter methods**

Extend `ConversationErrorCode` with the exact stable variants:

```rust
ReactionNotSupported,
ReactionRemovalNotVerified,
ReactionDeliveryUnknown,
ForwardNotSupported,
ForwardSourceMissingProviderIds,
ForwardTargetAccountMismatch,
ForwardTargetLimitExceeded,
ForwardDeliveryUnknown,
TemplateLimitExceeded,
TemplateShortcutConflict,
InvalidMentionRange,
MentionParticipantNotFound,
MentionDirectConversationUnsupported,
```

Extend the adapter:

```rust
async fn set_reaction(&self, request: ProviderReactionRequest) -> ConversationResult<()>;
async fn forward_message(&self, request: ProviderForwardRequest) -> ConversationResult<ProviderSendReceipt>;
```

Define provider requests in `adapter.rs`:

```rust
pub struct ProviderReactionRequest {
    pub client_mutation_id: String,
    pub provider_message_id: String,
    pub provider_client_message_id: String,
    pub thread_id: String,
    pub thread_type: ProviderThreadType,
    pub reaction: ConversationReactionCode,
    pub active: bool,
}

pub struct ProviderForwardRequest {
    pub client_part_id: String,
    pub source_provider_message_id: String,
    pub source_provider_client_message_id: String,
    pub source_occurred_at_ms: u64,
    pub source_text: String,
    pub target_thread_id: String,
    pub target_thread_type: ProviderThreadType,
}
```

Reuse the Phase 1 `ProviderSendReceipt` and provider thread type.

Non-Zalo adapters return typed unsupported errors.

- [ ] **Step 6: Run contract tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2a_contract
```

Expected: PASS.

---

## Task 4: Add the Additive Phase 2A Migration

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql`
- Modify: `src-tauri/src/conversation/migration.rs`
- Modify: `src-tauri/src/conversation/migration_test.rs`

- [ ] **Step 1: Run `migrate` impact**

```text
mcp__gitnexus.impact({target: "migrate", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

- [ ] **Step 2: Write failing migration tests**

| Test | Required assertions |
|---|---|
| `phase2a_migration_creates_template_table` | columns, unique shortcut, indexes |
| `phase2a_migration_creates_mentions_table` | FKs, unique range, indexes |
| `phase2a_migration_creates_reaction_mutations` | client ID primary key, status check |
| `phase2a_migration_creates_forward_tables` | correct FKs and unique target |
| `phase2a_migration_is_idempotent` | two runs preserve schema/data |
| `phase2a_migration_preserves_phase1_rows` | canonical rows unchanged |

- [ ] **Step 3: Run migration tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2a_migration_
```

Expected: FAIL.

- [ ] **Step 4: Create tables in dependency order**

Create all five design tables. `message_forward_targets.send_batch_id` references `message_send_batches(id)`. Add checks for booleans, positive Mention lengths, and every status set before creating indexes.

- [ ] **Step 5: Run migration tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2a_migration_
```

Expected: PASS.

---

## Task 5: Implement Quick Template Repository and Ranking

**Files:**
- Create: `src-tauri/src/conversation/quick_templates.rs`
- Create: `src-tauri/src/conversation/quick_templates_test.rs`
- Modify: `src-tauri/src/conversation/mod.rs`

- [ ] **Step 1: Write failing repository tests**

Test normalized shortcut conflicts, active composer filtering, exact/prefix/title/category/body ranking, usage increments, 500-item limit, validation lengths, and isolated deletion.

- [ ] **Step 2: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml quick_templates_test
```

Expected: FAIL.

- [ ] **Step 3: Implement focused repository**

```rust
pub struct QuickTemplateRepository { db_path: PathBuf }

impl QuickTemplateRepository {
    pub fn list(&self, include_inactive: bool) -> ConversationResult<Vec<ConversationQuickTemplate>>;
    pub fn search(&self, query: &str, limit: usize) -> ConversationResult<Vec<ConversationQuickTemplate>>;
    pub fn create(&self, request: CreateConversationQuickTemplateRequest) -> ConversationResult<ConversationQuickTemplate>;
    pub fn update(&self, request: UpdateConversationQuickTemplateRequest) -> ConversationResult<ConversationQuickTemplate>;
    pub fn delete(&self, id: i64) -> ConversationResult<()>;
    pub fn mark_used(&self, id: i64, now: &str) -> ConversationResult<()>;
}
```

Clamp search to 1–50. Normalize shortcuts before queries.

- [ ] **Step 4: Implement deterministic rank buckets**

```text
0 exact shortcut
1 shortcut prefix
2 title prefix
3 category match
4 body substring
5 no textual match
```

Then usage descending, last used descending, ID ascending.

- [ ] **Step 5: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml quick_templates_test
```

Expected: PASS.

---

## Task 6: Add Quick Template Service and Commands

**Files:**
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/service_test.rs`
- Modify: `src-tauri/src/conversation/commands.rs`
- Modify: `src-tauri/src/conversation/commands_test.rs`

- [ ] **Step 1: Run service/command impact**

Run impact for `ConversationService`, `ConversationState`, and affected command wrappers.

- [ ] **Step 2: Write failing command tests**

Cover exact IPC behavior for list/create/update/delete/mark-used, plus conflict, validation, not-found, and limit errors.

- [ ] **Step 3: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml quick_template_command_
```

Expected: FAIL.

- [ ] **Step 4: Add service methods and thin commands**

Commands only deserialize, call the service, and serialize. Validation and SQL remain in the focused repository.

- [ ] **Step 5: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml quick_template_command_
```

Expected: PASS.

---

## Task 7: Normalize, Validate, and Project Mentions

**Files:**
- Create: `src-tauri/src/conversation/mentions.rs`
- Create: `src-tauri/src/conversation/mentions_test.rs`
- Modify: `src-tauri/src/conversation/repository.rs`
- Modify: `src-tauri/src/conversation/repository_test.rs`
- Modify: `src-tauri/src/conversation/projection.rs`
- Modify: `src-tauri/src/conversation/zalo/normalizer.rs`
- Modify: `src-tauri/src/conversation/zalo/normalizer_test.rs`

- [ ] **Step 1: Run projection/normalizer impact**

Run impact for `apply_normalized_message` and Phase 1 projection loaders.

- [ ] **Step 2: Write failing UTF-16 helper tests**

```rust
assert_eq!(utf16_len("A😀B"), 4);
assert_eq!(utf16_slice("Xin @Hà😀 nhé", 4, 5), Some("@Hà😀".into()));
```

Also test sorted/non-overlap rules, bounds, max 20, and control-character rejection.

- [ ] **Step 3: Write failing persistence/projection tests**

| Test | Required assertions |
|---|---|
| `outbound_mentions_require_active_group_participants` | direct/inactive/foreign fail |
| `backend_resolves_mention_external_uid` | frontend identity becomes provider UID |
| `incoming_mentions_upsert_without_duplicates` | one row per authoritative range |
| `message_projection_includes_mentions` | ordered camelCase DTOs |
| `authoritative_echo_replaces_stale_mentions` | stale child rows removed |

- [ ] **Step 4: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml mention_
```

Expected: FAIL.

- [ ] **Step 5: Implement validation and persistence**

```rust
pub fn validate_mentions(
    text: &str,
    inputs: &[ConversationMentionInput],
    participants: &[MentionParticipant],
) -> ConversationResult<Vec<ResolvedConversationMention>>;
```

Resolve provider UIDs in Rust. Persist/replace mentions in the message transaction.

- [ ] **Step 6: Bulk hydrate projection**

Add `mentions: Vec<ConversationMention>` to `ConversationMessage`. Load all mentions for a timeline page in one query; no N+1 queries.

- [ ] **Step 7: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml mention_
```

Expected: PASS.

---

## Task 8: Send Group Mentions Through the Batch Worker

**Files:**
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/service_test.rs`
- Modify: `src-tauri/src/conversation/zalo/client.rs`
- Modify: `src-tauri/src/conversation/zalo/client_test.rs`

- [ ] **Step 1: Run enqueue/worker/`ZaloClient` impact**

Stop on HIGH/CRITICAL.

- [ ] **Step 2: Write failing tests**

Cover group persistence, direct rejection, exact range round-trip, unchanged stable client ID, and quote plus mentions on the same text part.

- [ ] **Step 3: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml mention_
```

Expected: FAIL.

- [ ] **Step 4: Extend outbound text dispatch**

Load resolved mentions with the Phase 1 part. Convert them directly into vendored `MessageMention`; never recompute positions in the worker.

- [ ] **Step 5: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml mention_
```

Expected: PASS.

---

## Task 9: Implement Reaction Ledger and Echo Reconciliation

**Files:**
- Create: `src-tauri/src/conversation/reactions.rs`
- Create: `src-tauri/src/conversation/reactions_test.rs`
- Modify: `src-tauri/src/conversation/repository.rs`
- Modify: `src-tauri/src/conversation/repository_test.rs`
- Modify: `src-tauri/src/conversation/zalo/normalizer.rs`
- Modify: `src-tauri/src/conversation/zalo/normalizer_test.rs`

- [ ] **Step 1: Run `apply_reaction` impact**

```text
mcp__gitnexus.impact({target: "apply_reaction", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

- [ ] **Step 2: Write failing ledger tests**

```text
begin_mutation_is_idempotent_by_client_id
same_client_id_with_different_payload_is_rejected
successful_local_apply_upserts_self_actor_once
self_echo_does_not_double_count
remove_echo_marks_exact_reaction_removed
startup_running_mutations_become_delivery_unknown
matching_reconciliation_completes_unknown_mutation
```

- [ ] **Step 3: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml reactions_test
```

Expected: FAIL.

- [ ] **Step 4: Implement mutation repository**

```rust
pub struct ReactionMutationRepository { db_path: PathBuf }
pub enum BeginReactionMutation { Created(ReactionMutation), Existing(ReactionMutation) }

pub fn begin(&self, request: &SetConversationReactionRequest, account_id: i64) -> ConversationResult<BeginReactionMutation>;
pub fn succeed(&self, client_id: &str, self_external_id: &str, occurred_at: &str) -> ConversationResult<Vec<ConversationReactionSummary>>;
pub fn fail(&self, client_id: &str, code: &str, message: &str, unknown: bool) -> ConversationResult<()>;
pub fn mark_running_delivery_unknown(&self, now: &str) -> ConversationResult<usize>;
```

- [ ] **Step 5: Normalize removal from fixture contract**

Replace any numeric-type-only inference and preserve exact reaction code.

- [ ] **Step 6: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml reaction_
```

Expected: PASS.

---

## Task 10: Add Reaction Provider Service and Command

**Files:**
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/service_test.rs`
- Modify: `src-tauri/src/conversation/commands.rs`
- Modify: `src-tauri/src/conversation/commands_test.rs`
- Modify: `src-tauri/src/conversation/zalo/client.rs`
- Modify: `src-tauri/src/conversation/zalo/client_test.rs`

- [ ] **Step 1: Write failing command tests**

Cover missing IDs, disconnected/disabled capability, success local apply, duplicate client ID, permanent failure, timeout delivery unknown, and no automatic second call.

- [ ] **Step 2: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml set_reaction_
```

Expected: FAIL.

- [ ] **Step 3: Implement service workflow**

```rust
pub async fn set_conversation_reaction(
    &self,
    request: SetConversationReactionRequest,
) -> ConversationResult<ConversationReactionMutationResult>;
```

Resolve account, self UID, thread type, and provider IDs in Rust. Map `active = false` to the probe-verified payload.

- [ ] **Step 4: Reconcile unknown without resend**

Request bounded old reactions for the affected scope. Return delivery unknown immediately; never repeat the provider mutation automatically.

- [ ] **Step 5: Add thin Tauri command**

```rust
#[tauri::command]
pub async fn set_conversation_reaction(
    state: tauri::State<'_, ConversationState>,
    request: SetConversationReactionRequest,
) -> Result<ConversationReactionMutationResult, ConversationCommandError>;
```

- [ ] **Step 6: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml set_reaction_
```

Expected: PASS.

---

## Task 11: Enqueue Native Forward Operations Atomically

**Files:**
- Create: `src-tauri/src/conversation/forwarding.rs`
- Create: `src-tauri/src/conversation/forwarding_test.rs`
- Modify: `src-tauri/src/conversation/repository.rs`
- Modify: `src-tauri/src/conversation/repository_test.rs`
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/service_test.rs`

- [ ] **Step 1: Run Phase 1 enqueue impact**

Stop on HIGH/CRITICAL.

- [ ] **Step 2: Write failing atomic tests**

Cover one batch/job per target, idempotent operation ID, max 10, duplicate target rejection, cross-account rejection, missing source IDs, unsupported content, and transaction rollback.

- [ ] **Step 3: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml forward_
```

Expected: FAIL.

- [ ] **Step 4: Implement focused enqueue**

```rust
pub fn enqueue_forward_operation(
    tx: &rusqlite::Transaction<'_>,
    request: &ForwardConversationMessageRequest,
    now: &str,
) -> ConversationResult<ForwardConversationMessageResult>;
```

Derive deterministic target batch IDs from operation ID plus target conversation ID. Store source local ID and exact provider reference fields; no raw payload.

- [ ] **Step 5: Insert queued target messages**

Create one `content_type = 'forward'` local message per target with fallback summary `Đã chuyển tiếp một tin nhắn`; do not copy source into editable text.

- [ ] **Step 6: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml forward_
```

Expected: PASS.

---

## Task 12: Dispatch Forward Parts and Recalculate Status

**Files:**
- Modify: `src-tauri/src/conversation/forwarding.rs`
- Modify: `src-tauri/src/conversation/forwarding_test.rs`
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/service_test.rs`
- Modify: `src-tauri/src/conversation/zalo/client.rs`
- Modify: `src-tauri/src/conversation/zalo/client_test.rs`
- Modify: `src-tauri/src/conversation/zalo/normalizer.rs`
- Modify: `src-tauri/src/conversation/zalo/normalizer_test.rs`

- [ ] **Step 1: Run worker and `ZaloClient` impact**

Stop on HIGH/CRITICAL.

- [ ] **Step 2: Write failing worker/status tests**

```text
forward_worker_calls_one_target_with_stable_client_id
forward_success_persists_provider_ids
forward_echo_updates_queued_target_without_duplicate
successful_target_is_not_requeued_when_sibling_fails
forward_running_after_restart_becomes_delivery_unknown
forward_operation_status_rules_cover_mixed_states
manual_retry_requeues_only_failed_target
```

- [ ] **Step 3: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml forward_
```

Expected: FAIL.

- [ ] **Step 4: Extend worker dispatch**

Handle `forward` with one provider target, stable client ID, source reference, and probe-approved source type. Reuse Phase 1 retry classification.

- [ ] **Step 5: Implement exact aggregate status**

Use the design status table. Map running-after-restart to target delivery unknown; never auto-requeue.

- [ ] **Step 6: Add failed-target retry**

```rust
pub fn retry_forward_target(
    &self,
    client_operation_id: &str,
    target_conversation_id: i64,
) -> ConversationResult<ConversationForwardTargetResult>;
```

Allow only `failed`, never `sent`, `sending`, or `delivery_unknown`.

- [ ] **Step 7: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml forward_
```

Expected: PASS.

---

## Task 13: Register Commands and Startup Recovery

**Files:**
- Modify: `src-tauri/src/conversation/commands.rs`
- Modify: `src-tauri/src/conversation/commands_test.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/conversation/mod.rs`

- [ ] **Step 1: Write failing registration tests**

Assert registration for reaction, Forward, failed-target retry, mention candidates, and all five template commands.

- [ ] **Step 2: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2a_command_registration
```

Expected: FAIL.

- [ ] **Step 3: Register and recover**

Run migrations, mark reaction mutations left `running` as delivery unknown, then start workers. Do not reorder Phase 1 startup cleanup.

- [ ] **Step 4: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2a_command_registration
```

Expected: PASS.

- [ ] **Step 5: Backend checkpoint**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml conversation::
```

```text
mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})
```

---

## Task 14: Add Frontend Contracts and IPC Wrappers

**Files:**
- Modify: `src/lib/services/conversationService.ts`
- Modify: `src/lib/services/__tests__/conversationService.spec.ts`

- [ ] **Step 1: Run `ConversationMessage` impact/context**

Inspect all Svelte imports. Stop on HIGH/CRITICAL.

- [ ] **Step 2: Write failing wrapper tests**

Assert exact invoke names and `{request}` arguments for every new command. Verify error normalization preserves all Phase 2A codes.

- [ ] **Step 3: Run tests RED**

```bash
bun test --conditions=browser --conditions=svelte src/lib/services/__tests__/conversationService.spec.ts
```

Expected: FAIL.

- [ ] **Step 4: Mirror Rust types**

```ts
export type ConversationReactionCode = 'heart' | 'like' | 'haha' | 'wow' | 'cry' | 'angry';
export type ConversationMentionInput = { identityId: number; displayText: string; startUtf16: number; lengthUtf16: number };
export type ConversationMention = { identityId: number | null; externalId: string; displayText: string; startUtf16: number; lengthUtf16: number };
export type ConversationQuickTemplate = { id: number; shortcut: string; title: string; bodyText: string; category: string | null; isActive: boolean; usageCount: number; lastUsedAt: string | null; createdAt: string; updatedAt: string };
export type ListConversationQuickTemplatesRequest = { includeInactive?: boolean };
export type CreateConversationQuickTemplateRequest = { shortcut: string; title: string; bodyText: string; category: string | null };
export type UpdateConversationQuickTemplateRequest = { id: number; shortcut: string; title: string; bodyText: string; category: string | null; isActive: boolean };
export type DeleteConversationQuickTemplateRequest = { id: number };
export type MarkConversationQuickTemplateUsedRequest = { id: number };
export type SetConversationReactionRequest = { messageId: number; reaction: ConversationReactionCode; active: boolean; clientMutationId: string };
export type ConversationReactionMutationResult = { clientMutationId: string; messageId: number; reaction: ConversationReactionCode; requestedActive: boolean; status: 'running' | 'succeeded' | 'failed' | 'delivery_unknown'; reactions: ConversationReactionSummary[]; errorCode: string | null; errorMessage: string | null };
export type ForwardConversationMessageRequest = { sourceMessageId: number; targetConversationIds: number[]; clientOperationId: string };
export type ConversationForwardTargetResult = { targetConversationId: number; clientBatchId: string; status: 'queued' | 'sending' | 'sent' | 'failed' | 'delivery_unknown'; errorCode: string | null; errorMessage: string | null };
export type ForwardConversationMessageResult = { clientOperationId: string; sourceMessageId: number; status: 'queued' | 'sending' | 'partial' | 'sent' | 'failed' | 'delivery_unknown'; targets: ConversationForwardTargetResult[] };
export type RetryConversationForwardTargetRequest = { clientOperationId: string; targetConversationId: number };
export type ConversationMentionCandidate = { identityId: number; externalId: string; displayName: string; avatarUrl: string | null; participantRole: string };
export type ListConversationMentionCandidatesRequest = { conversationId: number; query: string; limit?: number };

export type ConversationMessageActions = {
  reply: boolean;
  copy: boolean;
  recall: boolean;
  forward: boolean;
  react: boolean;
};

export type ConversationCapabilities = {
  sendText: boolean;
  sendSticker: boolean;
  quoteReply: boolean;
  recallMessage: boolean;
  reactions: {
    read: boolean;
    write: boolean;
    supported: ConversationReactionCode[];
  };
  attachments: {
    image: boolean;
    video: boolean;
    audio: boolean;
    file: boolean;
    maxFilesPerBatch: number;
    maxBytesPerFile: number;
    maxBytesPerBatch: number;
  };
  forward: { enabled: boolean; maxTargets: number };
  mentions: { enabled: boolean; maxPerMessage: number };
  quickTemplates: { enabled: boolean; maxTemplates: number };
};
```

Replace the Phase 1 `ConversationCapabilities` declaration with the additive final shape above. Add `mentions: ConversationMention[]` and `actions: ConversationMessageActions` to `ConversationMessage`. Add `mentions: ConversationMentionInput[]` to the text variant of `ConversationSendPart`. If Phase 2B types already exist, retain `voiceRecording`, `mediaLibrary`, `ConversationVoicePart`, `durationMs`, and `mediaRole`; produce one merged declaration rather than parallel or duplicate types.

- [ ] **Step 5: Add wrappers**

```ts
export const setConversationReaction = (request: SetConversationReactionRequest) =>
  invoke<ConversationReactionMutationResult>('set_conversation_reaction', { request });

export const forwardConversationMessage = (request: ForwardConversationMessageRequest) =>
  invoke<ForwardConversationMessageResult>('forward_conversation_message', { request });

export const retryConversationForwardTarget = (request: RetryConversationForwardTargetRequest) =>
  invoke<ConversationForwardTargetResult>('retry_conversation_forward_target', { request });

export const listConversationQuickTemplates = (request: ListConversationQuickTemplatesRequest = {}) =>
  invoke<ConversationQuickTemplate[]>('list_conversation_quick_templates', { request });

export const createConversationQuickTemplate = (request: CreateConversationQuickTemplateRequest) =>
  invoke<ConversationQuickTemplate>('create_conversation_quick_template', { request });

export const updateConversationQuickTemplate = (request: UpdateConversationQuickTemplateRequest) =>
  invoke<ConversationQuickTemplate>('update_conversation_quick_template', { request });

export const deleteConversationQuickTemplate = (id: number) =>
  invoke<void>('delete_conversation_quick_template', { request: { id } });

export const markConversationQuickTemplateUsed = (id: number) =>
  invoke<void>('mark_conversation_quick_template_used', { request: { id } });

export const listConversationMentionCandidates = (request: ListConversationMentionCandidatesRequest) =>
  invoke<ConversationMentionCandidate[]>('list_conversation_mention_candidates', { request });
```

- [ ] **Step 6: Run tests GREEN**

```bash
bun test --conditions=browser --conditions=svelte src/lib/services/__tests__/conversationService.spec.ts
bun run check
```

Expected: PASS.

---

## Task 15: Build Quick Template State, Popup, and Manager

**Files:**
- Create: `src/lib/components/conversation/quickTemplateState.ts`
- Create: `src/lib/components/conversation/QuickTemplatePopover.svelte`
- Create: `src/lib/components/conversation/QuickTemplateManagerDialog.svelte`
- Create: `src/lib/components/conversation/QuickTemplateForm.svelte`
- Create: `src/lib/components/conversation/__tests__/quickTemplateState.spec.ts`
- Create: `src/lib/components/conversation/__tests__/QuickTemplatePopover.spec.ts`
- Create: `src/lib/components/conversation/__tests__/QuickTemplateManagerDialog.spec.ts`
- Modify: `src/lib/components/conversation/MessageComposer.svelte`
- Modify: `src/lib/components/conversation/__tests__/MessageComposer.spec.ts`

- [ ] **Step 1: Run composer context/impact**

Inspect imports manually if Svelte symbols are not indexed.

- [ ] **Step 2: Write failing pure-state tests**

```ts
expect(findSlashToken('Xin /gia', 8)).toEqual({ start: 4, end: 8, query: 'gia' });
expect(findSlashToken('https://a/b', 11)).toBeNull();
expect(replaceSlashToken('Xin /gia nhé', { start: 4, end: 8, query: 'gia' }, 'Bảng giá')).toEqual({ value: 'Xin Bảng giá nhé', caret: 12 });
```

Also test ranking, inactive filtering, surrounding text, and Mention-range shifts.

- [ ] **Step 3: Write failing component tests**

Cover popup rules, exact/prefix ranking, keyboard behavior, no auto-send, CRUD validation/conflict, active toggle, usage update, and 500-item limit messaging.

- [ ] **Step 4: Run tests RED**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/quickTemplateState.spec.ts src/lib/components/conversation/__tests__/QuickTemplatePopover.spec.ts src/lib/components/conversation/__tests__/QuickTemplateManagerDialog.spec.ts src/lib/components/conversation/__tests__/MessageComposer.spec.ts
```

Expected: FAIL.

- [ ] **Step 5: Implement pure helpers**

```ts
export type SlashToken = { start: number; end: number; query: string };
export function findSlashToken(value: string, caret: number): SlashToken | null;
export function replaceSlashToken(value: string, token: SlashToken, body: string): { value: string; caret: number };
export function rankQuickTemplates(items: ConversationQuickTemplate[], query: string): ConversationQuickTemplate[];
```

- [ ] **Step 6: Implement popup/manager and composer integration**

Components use typed callbacks and no channel checks. Escape closes templates before quote cancellation. Insert into the active draft only and mark usage after insertion.

- [ ] **Step 7: Run tests GREEN**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/quickTemplateState.spec.ts src/lib/components/conversation/__tests__/QuickTemplatePopover.spec.ts src/lib/components/conversation/__tests__/QuickTemplateManagerDialog.spec.ts src/lib/components/conversation/__tests__/MessageComposer.spec.ts
bun run check
```

Expected: PASS.

---

## Task 16: Build Mention State, Popup, and Rendering

**Files:**
- Create: `src/lib/components/conversation/mentionState.ts`
- Create: `src/lib/components/conversation/MentionCandidatePopover.svelte`
- Create: `src/lib/components/conversation/MessageMentionText.svelte`
- Create: `src/lib/components/conversation/__tests__/mentionState.spec.ts`
- Create: `src/lib/components/conversation/__tests__/MentionCandidatePopover.spec.ts`
- Create: `src/lib/components/conversation/__tests__/MessageMentionText.spec.ts`
- Modify: `src/lib/components/conversation/composerDraftState.ts`
- Modify: `src/lib/components/conversation/MessageComposer.svelte`
- Modify: `src/lib/components/conversation/MessageBubble.svelte`
- Modify: `src/lib/components/conversation/__tests__/composerDraftState.spec.ts`
- Modify: `src/lib/components/conversation/__tests__/MessageComposer.spec.ts`
- Modify: `src/lib/components/conversation/__tests__/MessageBubble.spec.ts`

- [ ] **Step 1: Write failing UTF-16/edit tests**

```ts
expect(utf16Length('A😀B')).toBe(4);
expect(sliceUtf16('Xin @Hà😀 nhé', 4, 5)).toBe('@Hà😀');
```

Cover combining marks, insert-before, delete-inside, paste-overlap, and unrelated-range preservation.

- [ ] **Step 2: Write failing popup/render tests**

Cover group-only behavior, candidate query, max 20, keyboard navigation, active participants, highlighted ranges, and unresolved identity fallback.

- [ ] **Step 3: Run tests RED**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/mentionState.spec.ts src/lib/components/conversation/__tests__/MentionCandidatePopover.spec.ts src/lib/components/conversation/__tests__/MessageMentionText.spec.ts src/lib/components/conversation/__tests__/MessageComposer.spec.ts src/lib/components/conversation/__tests__/MessageBubble.spec.ts
```

Expected: FAIL.

- [ ] **Step 4: Implement range helpers**

```ts
export type ComposerMentionRange = { localId: string; identityId: number; displayText: string; startUtf16: number; lengthUtf16: number };
export type TextEdit = { startUtf16: number; endUtf16: number; insertedText: string };
export type MentionToken = { startUtf16: number; endUtf16: number; query: string };
export type MentionInsertResult = { value: string; caretUtf16: number; mention: ComposerMentionRange };
export function applyTextEditToMentions(previous: string, next: string, edit: TextEdit, mentions: ComposerMentionRange[]): ComposerMentionRange[];
export function insertMention(value: string, token: MentionToken, candidate: ConversationMentionCandidate): MentionInsertResult;
export function toMentionInputs(mentions: ComposerMentionRange[]): ConversationMentionInput[];
```

- [ ] **Step 5: Extend draft snapshot and send**

Drafts/snapshots include mentions. Successful enqueue clears only submitted mention local IDs. Debounce candidate IPC 150 ms and discard stale generations.

- [ ] **Step 6: Render semantic Mention spans**

Split by validated projected ranges. Never inject HTML.

- [ ] **Step 7: Run tests GREEN**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/mentionState.spec.ts src/lib/components/conversation/__tests__/MentionCandidatePopover.spec.ts src/lib/components/conversation/__tests__/MessageMentionText.spec.ts src/lib/components/conversation/__tests__/MessageComposer.spec.ts src/lib/components/conversation/__tests__/MessageBubble.spec.ts
bun run check
```

Expected: PASS.

---

## Task 17: Add Reaction Write UI

**Files:**
- Create: `src/lib/components/conversation/MessageReactionPicker.svelte`
- Create: `src/lib/components/conversation/MessageReactionMutationState.svelte`
- Create: `src/lib/components/conversation/__tests__/MessageReactionPicker.spec.ts`
- Modify: `src/lib/components/conversation/MessageReactionSummary.svelte`
- Modify: `src/lib/components/conversation/MessageActionsMenu.svelte`
- Modify: `src/lib/components/conversation/MessageBubble.svelte`
- Modify: `src/lib/components/conversation/ConversationWorkspace.svelte`
- Modify: `src/lib/components/conversation/__tests__/MessageReactionSummary.spec.ts`
- Modify: `src/lib/components/conversation/__tests__/MessageActionsMenu.spec.ts`
- Modify: `src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts`

- [ ] **Step 1: Write failing eligibility/mutation tests**

Verify message-level eligibility, six reactions, current-account active state, active toggle off, duplicate lock, authoritative success merge, self-echo no double count, failure retry, unknown warning without retry, Arrow-key navigation, Enter/Space activation, Escape close, Shift+F10 context opening, and screen-reader labels.

- [ ] **Step 2: Run tests RED**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/MessageReactionPicker.spec.ts src/lib/components/conversation/__tests__/MessageReactionSummary.spec.ts src/lib/components/conversation/__tests__/MessageActionsMenu.spec.ts src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
```

Expected: FAIL.

- [ ] **Step 3: Implement pending-key state**

```ts
type ReactionMutationKey = `${number}:${ConversationReactionCode}`;
let pendingReactions = $state(new Map<ReactionMutationKey, ConversationReactionMutationResult>());
```

Generate one client mutation ID per click. Never increment counts locally; replace with command/refresh summaries.

- [ ] **Step 4: Integrate hover and context actions**

Hover/pointer, keyboard, and Shift+F10 context paths call the same handler. Use roving focus or the existing menu primitive, label every reaction by name/state, restore focus on close, and preserve Phase 1 actor-detail behavior.

- [ ] **Step 5: Run tests GREEN**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/MessageReactionPicker.spec.ts src/lib/components/conversation/__tests__/MessageReactionSummary.spec.ts src/lib/components/conversation/__tests__/MessageActionsMenu.spec.ts src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
bun run check
```

Expected: PASS.

---

## Task 18: Add Native Forward Dialog and Target Results

**Files:**
- Create: `src/lib/components/conversation/forwardState.ts`
- Create: `src/lib/components/conversation/ForwardConversationDialog.svelte`
- Create: `src/lib/components/conversation/ForwardTargetResultList.svelte`
- Create: `src/lib/components/conversation/__tests__/forwardState.spec.ts`
- Create: `src/lib/components/conversation/__tests__/ForwardConversationDialog.spec.ts`
- Modify: `src/lib/components/conversation/MessageActionsMenu.svelte`
- Modify: `src/lib/components/conversation/ConversationWorkspace.svelte`
- Modify: `src/lib/components/conversation/__tests__/MessageActionsMenu.spec.ts`
- Modify: `src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts`

- [ ] **Step 1: Write failing state tests**

```ts
const snapshot = createForwardSnapshot(source, [31, 32]);
expect(snapshot.sourceMessageId).toBe(source.id);
expect(snapshot.targetConversationIds).toEqual([31, 32]);
```

Cover dedupe, max 10, account filtering, per-target merge, aggregate status, and failed-only retry.

- [ ] **Step 2: Write failing dialog/workspace tests**

Verify backend action eligibility, same-account search, pinned source/targets after workspace change, one stable operation ID, all status labels, failed-target retry, and durable jobs surviving dialog close.

- [ ] **Step 3: Run tests RED**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/forwardState.spec.ts src/lib/components/conversation/__tests__/ForwardConversationDialog.spec.ts src/lib/components/conversation/__tests__/MessageActionsMenu.spec.ts src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
```

Expected: FAIL.

- [ ] **Step 4: Implement state helpers**

```ts
export type ForwardDialogSnapshot = {
  clientOperationId: string;
  sourceMessageId: number;
  sourceChannelAccountId: number;
  targetConversationIds: number[];
};

export function createForwardSnapshot(source: ConversationMessage, targetConversationIds: number[]): ForwardDialogSnapshot;
export function mergeForwardResult(current: ForwardConversationMessageResult | null, next: ForwardConversationMessageResult): ForwardConversationMessageResult;
export function canRetryForwardTarget(target: ConversationForwardTargetResult): boolean;
```

- [ ] **Step 5: Implement dialog/results**

Use existing inbox pagination/search. Filter by pinned account before selection. TypeScript never constructs provider thread IDs.

- [ ] **Step 6: Run tests GREEN**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/forwardState.spec.ts src/lib/components/conversation/__tests__/ForwardConversationDialog.spec.ts src/lib/components/conversation/__tests__/MessageActionsMenu.spec.ts src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
bun run check
```

Expected: PASS.

- [ ] **Step 7: Frontend checkpoint**

```text
mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})
```

---

## Task 19: Full Verification and Live Gate

**Files:**
- Modify only defects in files already listed.
- Do not expand Phase 2A scope.

- [ ] **Step 1: Run targeted provider/backend tests**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml phase2a_
cargo test --manifest-path src-tauri/Cargo.toml phase2a_
cargo test --manifest-path src-tauri/Cargo.toml quick_template_
cargo test --manifest-path src-tauri/Cargo.toml mention_
cargo test --manifest-path src-tauri/Cargo.toml reaction_
cargo test --manifest-path src-tauri/Cargo.toml forward_
```

Expected: PASS.

- [ ] **Step 2: Run full backend validation**

```bash
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: PASS or exact unrelated baseline failures documented.

- [ ] **Step 3: Run frontend validation**

```bash
bun test --conditions=browser --conditions=svelte src/lib/services/__tests__/conversationService.spec.ts src/lib/components/conversation/__tests__
bun run check
bun test --conditions=browser --conditions=svelte
bun run build
```

Expected: PASS.

- [ ] **Step 4: Audit invariants**

```bash
rg -n "channel === ['\"]zalo|channel == ['\"]zalo" src/lib/components/conversation
rg -n "provider.*thread|provider.*message|raw_payload|cookies_json" src/lib/components/conversation src/lib/services/conversationService.ts
rg -n "innerHTML|{@html}" src/lib/components/conversation/QuickTemplate* src/lib/components/conversation/MessageMentionText.svelte
rg -n "copy.*forward|fallback.*forward" src-tauri/src/conversation src/lib/components/conversation
```

Expected: no presentation provider checks, raw provider exposure, HTML templates, or copy Forward fallback.

- [ ] **Step 5: Run GitNexus final audit**

```text
mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})
```

Update every d=1 dependent from impact runs.

- [ ] **Step 6: Execute live smoke gate**

Run all 11 cases from design section 18: direct/group reactions, Heart/Haha removal, timeout unknown, verified native Forward types, 10-target partial failure, template CRUD/insertion, Vietnamese/emoji Mention, and direct-chat Mention absence.

- [ ] **Step 7: Verify capability rollback**

Disable each capability independently. Confirm Phase 1 text/rich send, quote, recall, and read-only reaction display remain functional.

- [ ] **Step 8: Final diff review**

```bash
git diff --check
git status --short
git diff --stat
```

Do not commit without explicit authorization.

---

## Acceptance Checklist

- [ ] Reaction add/remove uses accepted provider fixtures.
- [ ] Heart and Haha are never confused with removal.
- [ ] Self echo never duplicates reaction counts or actors.
- [ ] Reaction delivery unknown is never automatically resent.
- [ ] Native Forward creates one independent target batch/job per destination.
- [ ] Successful Forward targets are never resent because siblings fail.
- [ ] No copy-based Forward fallback exists.
- [ ] Quick templates are durable local plain text and never auto-send.
- [ ] Slash insertion preserves surrounding text and valid Mention ranges.
- [ ] Mentions validate active group participants and exact UTF-16 ranges.
- [ ] Direct conversations never send mentions.
- [ ] Provider identifiers and raw provider payloads remain backend-owned.
- [ ] UI behavior is capability-driven rather than channel-name-driven.
- [ ] Non-Zalo channels preserve existing behavior.
- [ ] All targeted/full validation gates pass or unrelated failures are documented exactly.
- [ ] GitNexus final scope matches approved Phase 2A files and flows.
