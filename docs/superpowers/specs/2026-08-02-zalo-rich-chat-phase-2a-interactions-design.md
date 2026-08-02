# Zalo Rich Chat Phase 2A Interaction Design

## 1. Goal

Phase 2A completes the high-value message interaction workflow on top of Zalo Rich Chat Phase 1:

- Add and remove Zalo reactions.
- Forward an existing provider message natively to one or more conversations.
- Insert and manage local quick-reply templates through `/`.
- Mention active participants in Zalo group messages through `@`.

The design keeps presentation components provider-neutral. Generic Conversation capabilities and contracts describe the features, while only the Zalo adapter implements the provider writes in this phase.

## 2. Prerequisites and Relationship to Phase 1

Phase 2A requires the approved Phase 1 design and implementation plan:

- `docs/superpowers/specs/2026-08-02-zalo-rich-chat-phase-1-design.md`
- `docs/superpowers/plans/2026-08-02-zalo-rich-chat-phase-1-implementation-plan.md`

The following Phase 1 foundations are mandatory before Phase 2A execution:

- Stable provider message and provider client-message identifiers.
- Capability-driven Conversation UI.
- Durable batch/part outbox processing.
- Provider echo reconciliation without content/time heuristics.
- Current-account identity projection.
- Read-only reaction projection and actor details.
- Message actions menu and rich content rendering.

Phase 2A extends those foundations. It does not replace the Phase 1 outbox, staging, normalization, or timeline architecture.

## 3. Confirmed Scope

### 3.1 Included

#### Reaction writes

- Add one of six standard reactions: Heart, Like, Haha, Wow, Cry, Angry.
- Remove an active current-account reaction through the provider.
- Show pending, failed, and delivery-unknown mutation state.
- Reconcile local state from provider reaction echoes and bounded reaction refresh.
- Prevent duplicate count increments when a self echo follows a successful local command.

#### Native Forward

- Forward through the Zalo native forward API only.
- Select up to 10 target conversations in one user action.
- Create independent durable delivery state for every target.
- Retry only failed targets.
- Preserve provider forward metadata and source reference.
- Support only source content types proven by the provider contract probe.

#### Quick Templates

- Local SQLite CRUD.
- Workspace-wide templates shared by all channel accounts.
- Plain-text body.
- Shortcut, title, category, active state, usage count, and last-used timestamp.
- Slash popup with keyboard navigation and query filtering.
- Insert into the current composer draft without auto-sending.

#### Group Mentions

- `@` participant search in group conversations.
- Active group participants only.
- Structured mention ranges stored and sent with text.
- Inbound and outbound mention projection for highlighted rendering.
- UTF-16 offsets compatible with provider payload requirements.

### 3.2 Explicitly excluded

- Copy-based Forward fallback.
- Forwarding through a different Zalo account from the source message account.
- Reaction writes for Email, WhatsApp, or Messenger.
- Custom provider reaction catalog beyond the six standard reactions.
- Quick-template variables, conditional rendering, rich text, styles, or attachments.
- Cloud template synchronization, roles, approval workflow, or template sharing permissions.
- Direct-message mentions.
- Typing indicators.
- Message-content search.
- Media library, voice recording, CRM panel, or AI suggestions.

## 4. Chosen Architecture

### 4.1 Extension-first architecture

Phase 2A extends the existing Conversation domain instead of introducing a parallel chat service:

- Generic capabilities describe reaction writes, forwarding, quick templates, and mentions.
- Zalo commands validate canonical local IDs and resolve provider identifiers in Rust.
- Reaction writes use synchronous provider mutations with a durable mutation ledger and no automatic resend after delivery uncertainty.
- Forward creates one existing-style send batch and one outbox part per target conversation.
- Quick templates are local Conversation-owned data.
- Mention metadata is normalized into a dedicated message child table.

### 4.2 Rejected alternatives

#### Direct untracked provider mutations

Calling reaction and forward APIs without durable state is rejected because crashes and timeouts would leave no idempotency or delivery-unknown record.

#### Copy-based forwarding

Re-sending normalized content as a new message is rejected because it loses provider-native forward metadata, may alter media semantics, and can expose content the provider no longer permits forwarding.

#### Importing the ZaloCRM template/media architecture

ZaloCRM includes organization permissions, rich templates, global media assets, tags, automation, and server-side collaboration. Those concerns do not fit RustSale_v2's local-first Phase 2A scope.

## 5. Provider Contract Probe

Implementation begins with a non-production probe using a dedicated Zalo account and consenting recipient. Feature capabilities remain disabled until the probe artifacts and tests are accepted.

### 5.1 Reaction probe

Verify direct and group behavior for:

1. Add Heart.
2. Add Haha.
3. Remove Heart.
4. Add two different reactions from the current account if the provider permits it.
5. Remove one active reaction without removing another.
6. Request old reactions after each mutation.

Capture sanitized request, response, realtime echo, and old-reaction payload fixtures.

The current normalizer must not assume that a numeric reaction type alone indicates removal. The provider enum maps Haha and remove-related values through overlapping or context-dependent fields. The probe must define removal using the full payload contract. If exact per-reaction removal cannot be proven, reaction removal remains disabled and this specification must be amended before implementation continues.

### 5.2 Forward probe

Verify one target per provider call for direct and group destinations. Probe these source content types:

- Text.
- Image.
- Video.
- Audio/voice.
- File.
- Sticker.
- Provider-normalized link.

Capture:

- Required source message body.
- Provider message ID.
- Provider client-message ID.
- Source timestamp/reference fields.
- Forward level behavior.
- Response message/client IDs.
- Realtime and history echo shape.

Text forwarding is mandatory. Other content types become forwardable only when the probe proves correct native behavior. Unsupported types expose `forwardable: false`; they are never copied as a fallback.

### 5.3 Mention probe

Send group messages containing:

- ASCII participant names.
- Vietnamese combining characters.
- Precomposed Vietnamese characters.
- Emoji before a mention.
- Emoji inside surrounding text.
- Multiple mentions.

The probe must prove whether provider positions and lengths are UTF-16 code units. The implementation contract uses UTF-16 only after the fixture proves it.

## 6. Data Model

### 6.1 `conversation_quick_templates`

```sql
CREATE TABLE conversation_quick_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    shortcut TEXT NOT NULL,
    normalized_shortcut TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    body_text TEXT NOT NULL,
    category TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    usage_count INTEGER NOT NULL DEFAULT 0,
    last_used_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

Rules:

- `normalized_shortcut` matches `^[a-z0-9_-]{1,32}$`.
- `title` is 1–100 Unicode scalar values after trimming.
- `body_text` is 1–4,000 Unicode scalar values.
- `category` is optional and limited to 50 Unicode scalar values.
- Maximum active plus inactive template count is 500.
- Deleting a template is a hard local delete because no historical message references depend on it.

### 6.2 `message_mentions`

```sql
CREATE TABLE message_mentions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    identity_id INTEGER REFERENCES channel_identities(id) ON DELETE SET NULL,
    actor_external_id TEXT NOT NULL,
    display_text TEXT NOT NULL,
    start_utf16 INTEGER NOT NULL,
    length_utf16 INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(message_id, actor_external_id, start_utf16, length_utf16)
);
```

Indexes support projection by `message_id` and identity resolution by `(actor_external_id, message_id)`.

### 6.3 `message_reaction_mutations`

```sql
CREATE TABLE message_reaction_mutations (
    client_mutation_id TEXT PRIMARY KEY,
    message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    channel_account_id INTEGER NOT NULL REFERENCES channel_accounts(id),
    reaction_code TEXT NOT NULL,
    requested_active INTEGER NOT NULL,
    status TEXT NOT NULL,
    provider_result_json TEXT,
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    completed_at TEXT
);
```

Allowed statuses are `running`, `succeeded`, `failed`, and `delivery_unknown`.

On startup, `running` mutations become `delivery_unknown`. They are never automatically resent. A later provider echo or old-reaction reconciliation may convert a matching mutation to `succeeded`.

### 6.4 `message_forward_operations`

```sql
CREATE TABLE message_forward_operations (
    client_operation_id TEXT PRIMARY KEY,
    source_message_id INTEGER NOT NULL REFERENCES messages(id),
    channel_account_id INTEGER NOT NULL REFERENCES channel_accounts(id),
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

Allowed statuses are `queued`, `sending`, `partial`, `sent`, `failed`, and `delivery_unknown`.

### 6.5 `message_forward_targets`

```sql
CREATE TABLE message_forward_targets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_operation_id TEXT NOT NULL REFERENCES message_forward_operations(client_operation_id) ON DELETE CASCADE,
    target_conversation_id INTEGER NOT NULL REFERENCES channel_conversations(id),
    send_batch_id TEXT NOT NULL REFERENCES message_send_batches(id),
    status TEXT NOT NULL,
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(client_operation_id, target_conversation_id)
);
```

Every target owns one send batch with one `forward` part. Provider calls remain one-target-per-call even though the vendored API can accept several thread IDs. This preserves independent delivery and retry semantics.

Target statuses are `queued`, `sending`, `sent`, `failed`, and `delivery_unknown`. The operation is:

- `queued` when every target is queued.
- `sending` when at least one target is active and none is terminal.
- `sent` when every target is sent.
- `failed` when every target is permanently failed.
- `delivery_unknown` when every unresolved target is delivery unknown and none is sent.
- `partial` for every mixed terminal outcome, including sent plus failed or delivery unknown.

## 7. Conversation Capabilities

Phase 1 capabilities extend additively:

```ts
type ConversationCapabilities = {
  // Phase 1 fields remain unchanged.
  reactions: {
    read: boolean;
    write: boolean;
    supported: ConversationReactionCode[];
  };
  forward: {
    enabled: boolean;
    maxTargets: number;
  };
  mentions: {
    enabled: boolean;
    maxPerMessage: number;
  };
  quickTemplates: {
    enabled: boolean;
    maxTemplates: number;
  };
};
```

Message-level action eligibility adds:

```ts
type ConversationMessageActions = {
  reply: boolean;
  copy: boolean;
  recall: boolean;
  forward: boolean;
  react: boolean;
};
```

The backend derives message-level actions from account capability, connection state, direction, tombstone state, exact provider identifiers, conversation kind, and provider-probed content support.

## 8. Contracts

### 8.1 Reaction mutation

```ts
type ConversationReactionCode =
  | 'heart'
  | 'like'
  | 'haha'
  | 'wow'
  | 'cry'
  | 'angry';

type SetConversationReactionRequest = {
  messageId: number;
  reaction: ConversationReactionCode;
  active: boolean;
  clientMutationId: string;
};

type ConversationReactionMutationResult = {
  clientMutationId: string;
  messageId: number;
  reaction: ConversationReactionCode;
  requestedActive: boolean;
  status: 'running' | 'succeeded' | 'failed' | 'delivery_unknown';
  reactions: ConversationReactionSummary[];
  errorCode: string | null;
  errorMessage: string | null;
};
```

### 8.2 Forward operation

```ts
type ForwardConversationMessageRequest = {
  sourceMessageId: number;
  targetConversationIds: number[];
  clientOperationId: string;
};

type ConversationForwardTargetResult = {
  targetConversationId: number;
  clientBatchId: string;
  status: 'queued' | 'sending' | 'sent' | 'failed' | 'delivery_unknown';
  errorCode: string | null;
  errorMessage: string | null;
};

type ForwardConversationMessageResult = {
  clientOperationId: string;
  sourceMessageId: number;
  status: 'queued' | 'sending' | 'partial' | 'sent' | 'failed' | 'delivery_unknown';
  targets: ConversationForwardTargetResult[];
};
```

The command is idempotent by `clientOperationId`. Repeating the same operation ID returns the existing operation and targets without creating new batches.

### 8.3 Quick templates

```ts
type ConversationQuickTemplate = {
  id: number;
  shortcut: string;
  title: string;
  bodyText: string;
  category: string | null;
  isActive: boolean;
  usageCount: number;
  lastUsedAt: string | null;
  createdAt: string;
  updatedAt: string;
};
```

Commands:

- `list_conversation_quick_templates`
- `create_conversation_quick_template`
- `update_conversation_quick_template`
- `delete_conversation_quick_template`
- `mark_conversation_quick_template_used`

### 8.4 Mention candidates and send ranges

```ts
type ConversationMentionCandidate = {
  identityId: number;
  externalId: string;
  displayName: string;
  avatarUrl: string | null;
  participantRole: string;
};

type ConversationMentionInput = {
  identityId: number;
  displayText: string;
  startUtf16: number;
  lengthUtf16: number;
};
```

`ConversationSendPart` text variants add `mentions: ConversationMentionInput[]`.

## 9. Reaction Workflow

1. UI creates a stable `clientMutationId` and marks the selected reaction pending.
2. Rust verifies message ownership, account connection, exact provider IDs, capability, and reaction allowlist.
3. Rust inserts or returns the mutation ledger row.
4. Rust calls the provider synchronously.
5. On confirmed success, Rust applies the current account's normalized reaction state locally and marks the mutation succeeded.
6. Provider echo upserts the same `(message, actor, reaction)` row idempotently.
7. On permanent failure, Rust records the typed error and leaves normalized reactions unchanged.
8. On timeout or ambiguous transport failure, Rust marks delivery unknown, requests bounded reaction reconciliation, and never retries automatically.

The UI disables duplicate clicks for the same `(message, reaction)` while a mutation is running.

## 10. Forward Workflow

1. UI pins the source message ID and selected target IDs.
2. Rust loads the source message and exact provider reference fields.
3. Rust validates that every target is active, belongs to the same channel account, and does not exceed the 10-target limit.
4. In one transaction, Rust creates the operation, target rows, one send batch per target, one local queued message per target, and one outbox job per target.
5. The existing account worker claims each target job independently.
6. The Zalo adapter calls native Forward for exactly one target using a stable caller-controlled client ID.
7. Phase 1 echo reconciliation updates the queued local target message by provider client ID.
8. Operation status is recalculated from target states.

Forward job retries use the Phase 1 retry classifier. A running job after restart becomes delivery unknown and is not automatically resent.

## 11. Mention Workflow

### 11.1 Composer state

The draft adds:

```ts
type ComposerMentionRange = {
  localId: string;
  identityId: number;
  displayText: string;
  startUtf16: number;
  lengthUtf16: number;
};
```

Rules:

- Mention tokens include the leading `@` in their range.
- Ranges are sorted and non-overlapping.
- Editing inside a mention removes that mention's structured metadata but preserves text.
- Edits before a mention shift its range by the UTF-16 delta.
- Replacing a range that intersects a mention removes that mention.
- Switching drafts preserves mentions only within the owning conversation/account key.

### 11.2 Backend validation

For each mention, Rust verifies:

- The conversation is a group.
- The identity is an active participant in that conversation.
- The identity has a provider external UID.
- The range is within the final text's UTF-16 length.
- `displayText` starts with `@`, contains no control characters, and is at most 100 Unicode scalar values.
- The selected text exactly matches the submitted `displayText`.
- Ranges are sorted, non-overlapping, and at most 20 entries.

The backend resolves provider UIDs; it never trusts a frontend-supplied external UID.

## 12. Quick Template Workflow

### 12.1 Slash detection

The popup opens when the caret is after a token beginning with `/` at the start of text or immediately after whitespace. The query is the current token without `/`.

### 12.2 Search ranking

Templates rank by:

1. Exact shortcut.
2. Shortcut prefix.
3. Title prefix.
4. Category match.
5. Body substring.
6. Usage count descending.
7. Last used descending.

Inactive templates never appear in the composer popup.

### 12.3 Selection

Selecting a template replaces only the current slash token, preserves surrounding text and valid mention ranges, closes the popup, restores focus, and records usage asynchronously after insertion.

Template insertion never submits the composer.

## 13. UI Components

### 13.1 Reaction components

- `MessageReactionPicker.svelte`
- `MessageReactionMutationState.svelte`
- Existing read-only reaction summary gains current-account toggle behavior.

The picker supports pointer, keyboard, Shift+F10 context action, and screen-reader labels.

### 13.2 Forward components

- `ForwardConversationDialog.svelte`
- `ForwardTargetResultList.svelte`
- Existing message actions menu gains Forward eligibility.

The dialog filters targets by the source channel account, supports bounded search, enforces 10 targets, and pins source/targets during submission. Changing the active workspace conversation cannot retarget an in-flight operation.

### 13.3 Quick-template components

- `QuickTemplatePopover.svelte`
- `QuickTemplateManagerDialog.svelte`
- `QuickTemplateForm.svelte`

The popup supports Arrow Up/Down, Enter, Escape, and Tab behavior without breaking composer send shortcuts.

### 13.4 Mention components

- `MentionCandidatePopover.svelte`
- `MessageMentionText.svelte`
- Pure mention-range helpers in a focused TypeScript module.

## 14. Error Model

Phase 2A adds typed errors:

- `ReactionNotSupported`
- `ReactionRemovalNotVerified`
- `ReactionDeliveryUnknown`
- `ForwardNotSupported`
- `ForwardSourceMissingProviderIds`
- `ForwardTargetAccountMismatch`
- `ForwardTargetLimitExceeded`
- `ForwardDeliveryUnknown`
- `TemplateLimitExceeded`
- `TemplateShortcutConflict`
- `InvalidMentionRange`
- `MentionParticipantNotFound`
- `MentionDirectConversationUnsupported`

Frontend normalization continues to support legacy string errors but new commands return structured `{code,message}` errors.

## 15. Security and Safety

- Quick-template content is rendered and inserted as plain text only.
- Template fields never become raw HTML.
- Backend validates all lengths and shortcut format.
- Frontend never supplies trusted provider thread IDs, message IDs, or participant external UIDs.
- Forward resolves source and target provider identifiers from canonical local rows.
- Forward raw provider payload is never exposed to the UI.
- Reaction and Forward delivery-unknown operations are not automatically repeated.
- Provider fixtures and logs redact cookies, session tokens, phone numbers, and message bodies not required by the test.

## 16. Testing Strategy

### 16.1 Provider and vendored-client tests

- Caller-controlled forward client IDs.
- Direct and group native Forward payloads.
- Forward response normalization.
- Six standard reaction mappings.
- Exact removal mapping from probe fixtures.
- Mention payload offsets for Vietnamese and emoji.

### 16.2 Rust tests

- Additive and idempotent migration.
- Reaction command validation and ledger idempotency.
- Reaction self echo does not duplicate counts.
- Running reaction mutations become delivery unknown after restart.
- Forward operation creates one batch/job per target atomically.
- Duplicate operation IDs create no duplicate target batches.
- Successful targets are never resent when siblings fail.
- Account mismatch and unsupported content rejection.
- Mention participant/range validation.
- Quick-template CRUD, uniqueness, limits, ranking, and usage.

### 16.3 Frontend tests

- Reaction pending/success/failure/unknown states.
- Active reaction toggle and actor counts.
- Forward dialog filtering, limits, pinned source, and per-target results.
- Quick-template slash detection, ranking, insertion, CRUD, and keyboard behavior.
- Mention search, range shifting, overlap invalidation, UTF-16 behavior, and draft isolation.
- Capability gating without direct channel-name checks.

## 17. Rollout

Rollout order:

1. Provider contract probes and sanitized fixtures.
2. Additive schema and generic contracts.
3. Quick-template backend and manager UI.
4. Mention normalization, send contract, and UI.
5. Reaction writes behind capability.
6. Native Forward behind capability.
7. Targeted automated validation.
8. Full validation and live smoke gate.

Reaction and Forward capabilities remain disabled independently until their provider probes pass.

Rollback disables capabilities. Additive tables and columns remain; no destructive schema rollback is required.

## 18. Live Smoke Gate

Using a non-production account and non-sensitive content:

1. Add and remove every supported reaction in direct chat.
2. Add and remove Heart and Haha in group chat.
3. Confirm no self-echo double count.
4. Simulate reaction timeout and confirm delivery-unknown without automatic resend.
5. Forward verified content types to one direct target.
6. Forward to one group target.
7. Forward to 10 targets and confirm independent status.
8. Interrupt network during a multi-target forward and confirm successful targets are not resent.
9. Create, edit, disable, search, insert, and delete quick templates.
10. Mention participants with Vietnamese names and emoji before the mention.
11. Confirm direct chat never exposes Mention.

## 19. Acceptance Criteria

Phase 2A is complete when:

- Reaction add/remove uses verified provider semantics.
- Provider self echoes never duplicate reaction counts or actors.
- Delivery-unknown reaction mutations are never automatically resent.
- Native Forward preserves provider reference metadata.
- One Forward action creates independently retryable target deliveries.
- No copy-based Forward fallback exists.
- Quick templates are durable local plain text and never auto-send.
- Slash insertion preserves unrelated draft text and valid mention ranges.
- Mentions resolve active group participants and send verified UTF-16 offsets.
- Provider IDs and raw payloads remain backend-owned.
- All behavior is capability-driven.
- Non-Zalo channels preserve their existing behavior.
- Targeted tests, full tests, Svelte checks, Rust formatting, build, and lint gates pass or unrelated baseline failures are documented exactly.
- GitNexus change detection shows only expected Conversation, Zalo adapter, vendored provider, schema, command, and UI flows.

## 20. Implementation Guardrails

- Run Agent Memory recall before implementation.
- Run GitNexus impact before modifying every existing symbol.
- Stop and warn before HIGH or CRITICAL changes.
- Preserve unrelated working-tree changes.
- Use `bun` and `bunx`; never use `npm` or `npx`.
- Use TDD for every behavior change.
- Do not commit unless explicitly requested.
- Do not expand into Phase 2B, message search, folders/tags, CRM, AI, or multi-channel rich writes.
