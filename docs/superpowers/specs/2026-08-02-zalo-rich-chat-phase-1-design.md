# Zalo Rich Chat Phase 1 Design

**Date:** 2026-08-02

## 1. Goal

Extend the existing RustSale_v2 Conversation module with a production-safe Zalo rich-chat baseline without replacing the unified-conversation architecture established in the 2026-07-31 design and implementation plans.

Phase 1 must support:

- Rich rendering for text, image, file, video, audio, sticker, link metadata, and quote previews.
- Emoji insertion and live Zalo sticker search.
- Multiple staged attachments from the native picker, drag-and-drop, and clipboard image paste.
- One logical send action containing independently retryable text and attachment parts.
- Quote Reply.
- Message actions for Reply, Copy, and provider-backed Recall.
- Read-only reaction summaries and actor details.
- Generic Conversation contracts while only the Zalo adapter implements rich write operations.

The implementation remains local-first inside the existing Tauri, SvelteKit, Rust, and SQLite process. It must reuse the current Conversation event invalidation, sync, and durable outbox mechanisms.

## 2. Relationship to Existing Conversation Plans

This design extends, rather than replaces:

- `docs/superpowers/specs/2026-07-31-unified-conversation-design.md`
- `docs/superpowers/plans/2026-07-31-unified-conversation-master-plan.md`
- The existing storage, sync, UI, and outreach child plans.

Existing invariants remain authoritative:

- `channel_conversations` remains the canonical provider thread.
- Unified lead timelines remain read models over provider conversations.
- Group conversations remain separate from private lead timelines.
- Every outbound message remains bound to its actual channel account and provider thread.
- SQLite is the durable source of truth.
- Tauri events invalidate UI projections; they do not carry authoritative message state.
- Provider messages are normalized before persistence.

## 3. Confirmed Phase 1 Scope

### 3.1 Included

#### Composer

- Unicode emoji picker.
- Dynamic Zalo sticker picker backed by live Zalo APIs.
- Native multi-file selection.
- Drag-and-drop attachments.
- Clipboard image paste.
- Attachment preview queue.
- Text and attachments submitted as one logical batch.
- Quote Reply.

#### Message rendering

- Text.
- Images.
- Downloadable files.
- Inline video player.
- Inline audio player.
- Zalo stickers.
- Provider-normalized link previews.
- Quote previews.
- Tombstones for recalled messages.

#### Message actions

- Reply.
- Copy.
- Recall through the Zalo provider.

#### Reactions

- Emoji/count chips.
- Current-account reaction marker.
- Actor detail popover.
- No reaction write operations.

### 3.2 Explicitly excluded

- Adding or removing reactions.
- Forwarding messages.
- Voice recording.
- Quick templates triggered by `/`.
- Mentions triggered by `@`.
- Message-content search.
- Media, file, or link library tabs.
- Conversation folders and tags.
- CRM contact side panel.
- Notes, appointments, follow-up reminders, or AI suggestions.
- Rich write implementations for WhatsApp, Messenger, or Email.

## 4. Chosen Architecture

### 4.1 Send Batch plus Part Outbox

One composer action creates one logical batch. Text and each attachment or sticker are stored as independent message parts, and each part receives its own existing-style outbox job.

```text
MessageComposer
  -> SendConversationBatchRequest
  -> validate target capabilities
  -> validate and claim staged files
  -> transaction creates batch, messages, attachments, and outbox jobs
  -> existing per-account Zalo worker sends each part
  -> persist provider message and client IDs
  -> publish conversation-invalidated
  -> timeline refresh reconciles optimistic and provider state
```

This architecture is selected because it provides:

- One user-visible send operation.
- Independent retry per attachment.
- No resend of successful parts when another part fails.
- Compatibility with the existing one-job-per-message outbox model.
- Accurate partial-delivery reporting.
- Exact provider-echo reconciliation per part.

### 4.2 Rejected alternatives

#### One provider request for the complete composer payload

Rejected because provider-side splitting can produce multiple results while the application would have only one retry unit. A failed attachment could require resending successful content.

#### One independent composer send per item

Rejected because it loses the logical relationship between the user's single action, produces poor aggregate status, and complicates draft clearing and error reporting.

### 4.3 Batch statuses

The user-visible aggregate statuses are:

- `queued`
- `sending`
- `partial`
- `sent`
- `failed`

Part status is authoritative. Batch status is a transactionally maintained projection recalculated after every relevant part or outbox transition.

## 5. Data Model

All schema changes are additive. Existing message, attachment, reaction, receipt, and outbox data remains valid.

### 5.1 `channel_accounts.self_external_id`

Add a nullable provider identity for the currently authenticated account:

```sql
ALTER TABLE channel_accounts ADD COLUMN self_external_id TEXT;
```

`provider_account_key` cannot serve this purpose for Zalo because it currently identifies the session by IMEI. `self_external_id` stores the Zalo user ID used to identify the current account in reaction actor data.

Credentials and cookies remain outside `channel_accounts`.

### 5.2 `message_send_batches`

```sql
CREATE TABLE message_send_batches (
    id TEXT PRIMARY KEY,
    conversation_id INTEGER NOT NULL,
    channel_account_id INTEGER NOT NULL,
    client_batch_id TEXT NOT NULL,
    reply_to_message_id INTEGER,
    status TEXT NOT NULL CHECK (
        status IN ('queued', 'sending', 'partial', 'sent', 'failed')
    ),
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    UNIQUE(channel_account_id, client_batch_id),
    FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE,
    FOREIGN KEY(reply_to_message_id) REFERENCES messages(id) ON DELETE SET NULL
);
```

`client_batch_id` is generated by the frontend and provides idempotent enqueue behavior. Repeating a request with the same account and client batch ID returns the existing batch rather than inserting duplicate messages.

### 5.3 Message part columns

Add the batch relationship and deterministic part order to `messages`:

```sql
ALTER TABLE messages ADD COLUMN send_batch_id TEXT;
ALTER TABLE messages ADD COLUMN batch_index INTEGER;

CREATE UNIQUE INDEX idx_messages_send_batch_part
ON messages(send_batch_id, batch_index)
WHERE send_batch_id IS NOT NULL;
```

Each part uses:

- Existing `messages.id` as the local part ID.
- Existing `local_request_id` for part-level enqueue identity.
- Existing `provider_message_id` for provider reconciliation.
- Existing `provider_client_message_id` for provider reconciliation and quote operations.
- Existing `content_type`, `text_content`, and `content_json` for normalized content.
- Existing `reply_to_message_id` on the first part only.

Text is part zero when present. Attachments follow in composer order. A sticker is submitted as a one-part batch with `content_type = 'sticker'`.

### 5.4 `conversation_staged_attachments`

```sql
CREATE TABLE conversation_staged_attachments (
    id TEXT PRIMARY KEY,
    conversation_id INTEGER NOT NULL,
    original_file_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    local_path TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN ('staged', 'claimed', 'discarded')
    ),
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    claimed_message_id INTEGER,
    FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
    FOREIGN KEY(claimed_message_id) REFERENCES messages(id) ON DELETE SET NULL
);
```

The frontend sends only the opaque staging ID back to send and discard commands. It never selects an arbitrary local path for worker delivery.

### 5.5 Reused attachment and reaction tables

`message_attachments` remains the canonical attachment table. Outbound claimed files reuse `message_attachments.local_path` for the application-controlled spool path.

Sticker provider identifiers, category, type, preview URL, and animation metadata are stored in `message_attachments.metadata_json`. No separate sticker table is introduced in Phase 1.

`message_reactions` remains the canonical reaction table. Actor display details are resolved against channel identities where available and fall back to external IDs.

## 6. Staging and File Ownership

### 6.1 Spool root

Rust owns the spool under Tauri's application-local data directory:

```text
app_local_data_dir/
  conversation-spool/
    staged/
    claimed/
```

The implementation adds `tauri-plugin-dialog` for the native picker. It does not add `tauri-plugin-fs`; JavaScript does not require arbitrary filesystem access.

### 6.2 Staging flow

For picker paths and drag-drop paths:

1. Validate that the selected source exists and is a regular file.
2. Copy it into a randomly named file under `staged/`.
3. Validate and inspect the copied bytes.
4. Compute size, MIME, and checksum from the staged copy.
5. Insert the staging record.
6. Return normalized metadata and an opaque stage ID.

Clipboard image paste uses a dedicated capped byte-staging command and follows the same durable staging process after payload validation.

### 6.3 Claim flow

Before enqueueing a batch:

1. Resolve every stage ID for the selected conversation.
2. Reject expired, discarded, claimed, missing, modified, or unsupported files.
3. Atomically rename staged files into the claimed area on the same filesystem.
4. In one SQLite transaction, create the batch, parts, attachments, and outbox jobs and mark stage records claimed.
5. If the SQLite transaction fails, compensate by moving files back where possible.
6. Startup cleanup handles any filesystem/SQLite orphan left by a process crash between rename and commit.

Correctness relies on database ownership and checksum validation, not only directory names.

## 7. Conversation Contracts

### 7.1 Capabilities

The UI consumes normalized capabilities instead of branching directly on `channel === 'zalo'`:

```ts
type ConversationCapabilities = {
  sendText: boolean;
  sendSticker: boolean;
  quoteReply: boolean;
  recallMessage: boolean;
  reactions: {
    read: boolean;
    write: boolean;
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
};
```

For Zalo Phase 1, reaction reads are enabled and reaction writes remain disabled.

### 7.2 Send request

```ts
type SendConversationBatchRequest = {
  conversationId: number;
  clientBatchId: string;
  replyToMessageId?: number;
  parts: ConversationSendPart[];
};

type ConversationSendPart =
  | {
      clientPartId: string;
      type: 'text';
      text: string;
    }
  | {
      clientPartId: string;
      type: 'attachment';
      stagedAttachmentId: string;
    }
  | {
      clientPartId: string;
      type: 'sticker';
      sticker: ZaloStickerReference;
    };
```

The backend derives the channel account from the conversation and validates it against the selected reply target. It does not trust a duplicated account ID supplied by the frontend.

Phase 1 does not mix stickers with text or attachments in the same batch.

### 7.3 Message projection

The Conversation timeline projection adds normalized fields for:

- `ConversationAttachment`
- `ConversationReplyPreview`
- `ConversationReactionSummary`
- `ConversationReactionActor`
- `ConversationSendBatchSummary`
- `ConversationCapabilities`

Existing messages without rich fields remain valid and render through the current fallback path.

### 7.4 Tauri commands

The Phase 1 command surface is:

- `stage_conversation_attachment_paths`
- `stage_conversation_clipboard_image`
- `discard_conversation_staged_attachment`
- `send_conversation_batch`
- `recall_conversation_message`
- `search_zalo_stickers`

The existing `send_conversation_message` command remains as a compatibility wrapper that creates a text-only batch.

## 8. Provider Client IDs and `zca-rs`

### 8.1 Required change

The current vendored `zca-rs` implementation generates `clientId` internally and returns only the provider message ID. Zalo quote and undo operations require both the provider message ID and the provider client message ID.

Phase 1 therefore extends the vendored send APIs so callers can supply a stable client ID for:

- Direct text messages.
- Group text messages.
- Attachments.
- Stickers.

The normalized send receipt contains both IDs. Compatibility wrappers preserve current callers that do not provide a client ID.

### 8.2 Client ID generation

The Conversation service generates a decimal 64-bit client ID before the network call and persists it on the message part. IDs must be unique for the active Zalo account and suitable for the numeric fields expected by Zalo.

Persisting the ID before sending enables:

- Exact realtime/history echo reconciliation.
- Quote and undo operations after delivery.
- Recovery when the application exits after provider acceptance but before local completion.

## 9. Outbox Worker

### 9.1 Part dispatch

The worker loads a normalized outbound part and dispatches by content type:

- Text: `send_message` with message text.
- Attachment: read one claimed spool file and send one attachment.
- Sticker: `send_sticker` with provider sticker identifiers.

The worker remains sequential per account. It reads at most one attachment into memory at a time.

### 9.2 Quote resolution

The batch stores the local reply target. Only the first part stores and sends the quote to avoid repeated quote cards on the recipient side.

Immediately before the provider call, the adapter constructs the provider quote from the referenced local message:

- Provider message ID.
- Provider client message ID.
- Sender external ID.
- Provider timestamp.
- Normalized message type and content.
- Provider-specific quote metadata from normalized or raw payload data.

If the required provider IDs are unavailable, the operation fails with `quote_not_ready`. It must not silently downgrade to an unquoted send.

### 9.3 Part and batch ordering

Pending jobs are initially claimed by batch creation order and `batch_index`. A part in retry wait does not block later parts in the same batch. This permits partial delivery while keeping each part independently retryable.

## 10. Retry and Crash Recovery

### 10.1 Error classification

Retryable errors include:

- Network timeout or connection loss.
- Provider server errors.
- Rate limits.
- Explicitly classified temporary provider failures.

Permanent errors include:

- Missing or modified spool files.
- Unsupported content.
- Invalid provider payload.
- Recipient not found.
- Expired or already claimed staging IDs.

Authentication, login, cookie, or invalid-session failures mark the account invalid and prevent continued sending on that account until recovery.

### 10.2 Retry schedule

Each part receives no more than five attempts. The default backoff schedule uses jitter around:

```text
5 seconds -> 15 seconds -> 45 seconds -> 2 minutes -> 5 minutes
```

Provider retry guidance takes precedence when available.

### 10.3 Delivery unknown

An outbox job found in `running` state after application restart is not automatically resent. Zalo does not guarantee that replaying the same client ID is idempotent.

Recovery behavior:

1. Mark the part with a stable `delivery_unknown` error state.
2. Preserve its provider client ID.
3. Allow realtime or history synchronization to reconcile it.
4. Complete the message and job if the provider echo appears.
5. Keep the diagnostic failure if no echo arrives within the configured grace period.

Avoiding duplicate delivery takes precedence over automatic resend after an ambiguous network boundary.

## 11. Provider Echo Reconciliation

Outbound provider events are matched in this order:

1. Exact provider message ID in the same conversation.
2. Exact provider client message ID in the same account and conversation.
3. Otherwise insert as a new provider message.

Content, filename, and near-timestamp heuristics are prohibited because they can merge two legitimate repeated messages.

When an echo matches a local part, synchronization:

- Updates provider identifiers and timestamps.
- Merges normalized content, attachments, and raw payload metadata.
- Marks the message sent.
- Completes or cancels the still-open outbox job.
- Recalculates batch status.
- Publishes the existing Conversation invalidation event.

Undo events remain authoritative and may convert the message into a tombstone even if the local recall command did not complete its database update.

## 12. Batch Status Rules

Batch status is recalculated after enqueue, claim, completion, retry scheduling, terminal failure, delivery-unknown recovery, and provider reconciliation.

- `queued`: no part has been accepted by the provider and no part is running.
- `sending`: at least one part is pending, running, or retrying and no successful/terminal mixture exists.
- `partial`: at least one part is sent and at least one other part is pending, retrying, delivery unknown, or failed.
- `sent`: every part is sent.
- `failed`: no part is sent and all parts are terminally failed or delivery unknown.

The UI may show part-level errors inside a batch group, but it renders one aggregate status footer.

## 13. Recall

Recall is a synchronous provider operation rather than a delayed outbox action because provider recall windows may be time-sensitive.

The command:

1. Validates that the message is outbound, sent, not already removed, and belongs to a capable Zalo account.
2. Requires both provider message and client IDs.
3. Calls Zalo `undo`.
4. Marks `removed_at` only after provider success.
5. Publishes Conversation invalidation.

An explicitly recognized provider response meaning already recalled is treated as idempotent success. Network failure leaves the message unchanged so the user can retry.

Recall never deletes the local row. The timeline keeps a tombstone, and Phase 1 provides no local-delete action.

## 14. Reactions

Phase 1 reads normalized reaction records only.

The projection groups active reactions by emoji and returns:

- Emoji identifier.
- Count.
- Whether `actor_external_id` matches `channel_accounts.self_external_id`.
- Actor display name and avatar when resolved through channel identities.
- External ID fallback when identity metadata is unavailable.

The UI has no action that calls Zalo reaction-write APIs.

## 15. Sticker Search

The sticker picker calls live Zalo sticker search/detail APIs through the active account adapter:

- Search by keyword.
- Fetch required sticker detail metadata.
- Normalize provider IDs, category, type, preview URL, and animation metadata.
- Cache successful results for a short bounded period.

The picker does not bundle or hard-code a Zalo sticker catalog. Sticker search is unavailable when the account is disconnected or lacks the capability.

Selecting a sticker sends a one-part batch immediately. Failure keeps the picker open and exposes a stable domain error.

## 16. Composer and Draft State

### 16.1 Component structure

`MessageComposer.svelte` remains the coordinator and delegates new UI to focused components:

- Composer toolbar.
- Emoji picker.
- Zalo sticker picker.
- Attachment preview queue.
- Reply preview bar.
- Drag-drop overlay.

### 16.2 Draft ownership

Drafts are scoped by actual target:

```text
conversationId:channelAccountId
```

Each in-memory draft contains text, staged attachments, reply target, and active picker state. Phase 1 does not persist drafts across application restarts.

Rules:

- Switching targets never leaks text or attachments to another customer or account.
- Removing an attachment discards its stage record.
- A failed enqueue preserves the draft.
- A successful enqueue clears only the snapshot submitted by that request.
- Text entered while an enqueue request is running is not discarded.

### 16.3 Send eligibility

The send action is enabled only when:

- The selected account is connected.
- At least one valid text, attachment, or sticker part exists.
- No selected attachment remains in staging.
- Every part is allowed by current capabilities.
- No duplicate enqueue request is running.

Enter sends and Shift+Enter inserts a newline. Escape closes an active popover before cancelling a quote.

## 17. Attachment UX

Attachment sources are:

- Native multi-file picker.
- Native Tauri drag-drop events.
- Clipboard image paste.

Each preview item exposes one of:

- `staging`
- `ready`
- `error`
- `removing`

The queue displays a thumbnail for images and MIME-specific metadata for video, audio, and files. Invalid items remain visible with a removable error state but are excluded from send requests.

## 18. Quote Reply UX

Reply flows from the message action menu through the timeline and workspace into the composer.

For a unified lead timeline, choosing Reply pins the composer to the exact conversation and channel account of the source message. The user must cancel the quote before selecting an incompatible target.

The composer quote bar and bubble quote preview show:

- Sender.
- Content type.
- A one-line text or attachment summary.

Selecting a quote preview scrolls to the original message if it is present in the current timeline cache. Phase 1 does not automatically load an unlimited history range to find it.

## 19. Timeline and Rendering

### 19.1 Batch grouping

Consecutive outbound messages with the same send batch ID are rendered in a logical group ordered by `batch_index`.

The group:

- Keeps each real message part addressable.
- Renders one aggregate status footer.
- Shows part-level errors where applicable.
- Does not replace stored messages with a synthetic message row.

Legacy messages and incoming messages continue to render individually.

### 19.2 Rich content renderer

Message shell and message content are separated. The renderer dispatches normalized content to focused image, file, video, audio, sticker, link, and quote components.

Provider-normalized link metadata may render a preview card. The frontend does not scrape arbitrary web pages. Plain links are restricted to safe HTTP and HTTPS URLs.

### 19.3 Message actions

The action menu opens through a hover/focus button, right-click, and keyboard context-menu interaction.

- Reply appears only for quote-capable messages.
- Copy appears for normalized text or safe URLs.
- Recall appears only for eligible sent outbound messages with required provider IDs.

Recall requires confirmation.

### 19.4 Reaction display

Reaction chips appear below supported non-tombstone messages. A current-account reaction receives accent styling. Selecting a chip opens a read-only actor popover.

## 20. File and Content Security

### 20.1 Validation

- The physical spool filename is a random identifier, never a user filename.
- The copied file, not the original source path, is validated.
- MIME is detected from bytes; extension is secondary metadata.
- Canonical paths must remain below the spool root.
- Spool symlinks and path traversal are rejected.
- Checksum mismatch prevents delivery.
- User filenames are treated as display text and sanitized where required.
- Sensitive original paths, cookies, and raw provider payloads are not exposed to UI errors.

### 20.2 Phase 1 application limits

The initial application safety limits are:

- 10 attachments per batch.
- 50 MiB per file.
- 100 MiB total per batch.
- 10 MiB per clipboard image.
- 1 GiB total application spool quota.

These are application safety limits, not claims about the provider's maximum advertised limits. The backend publishes the effective values through capabilities; the frontend does not hard-code them.

### 20.3 Cleanup

- Unclaimed staged files expire after 24 hours.
- Explicitly removed staged files are deleted immediately.
- Pending and retrying files are retained.
- Sent files are retained until provider echo supplies stable remote metadata, then removed.
- Terminally failed files are retained for 24 hours before removal.
- Startup and periodic cleanup use the same idempotent cleanup service.
- Filesystem and database orphans are handled safely when either side is already missing.

## 21. Domain Errors

Commands return stable domain codes that the frontend can localize:

- `capability_denied`
- `account_disconnected`
- `stage_expired`
- `stage_already_claimed`
- `file_too_large`
- `unsupported_media`
- `quote_not_ready`
- `provider_auth`
- `provider_rate_limited`
- `delivery_unknown`
- `recall_not_allowed`

Raw provider details may be retained in protected diagnostics but must not reveal authentication material or local filesystem information to the UI.

## 22. Testing Strategy

Implementation follows test-driven development. Every task begins with GitNexus impact analysis for the symbols being modified, adds a failing targeted test, implements the minimal behavior, and expands validation only after targeted tests pass.

### 22.1 Rust tests

- Additive and idempotent migration coverage.
- Transactional batch enqueue and rollback.
- Stage ownership, claim, discard, expiry, and cleanup.
- Capability and request validation.
- Provider client ID generation.
- Text, attachment, sticker, and quote dispatch.
- Retry classification and backoff.
- Partial batch status.
- Delivery-unknown recovery.
- Exact provider echo reconciliation.
- Recall and undo tombstones.
- Reaction current-account projection.

### 22.2 Vendored provider tests

- Caller-provided client IDs in direct and group payloads.
- Client IDs in attachment and sticker payloads.
- Quote payload identifiers.
- Normalized send receipts containing message and client IDs.
- Compatibility of existing send methods.

### 22.3 Frontend tests

- Draft isolation.
- Attachment staging and queue states.
- Capability gating.
- Send snapshot clearing.
- Reply target pinning.
- Sticker search states.
- Drag-drop and clipboard staging calls.
- Batch grouping and status rendering.
- Rich content selection.
- Read-only reactions.
- Recall eligibility and confirmation.
- Tombstone behavior.

### 22.4 Validation commands

Frontend:

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation
bun run check
bun test --conditions=browser --conditions=svelte
bun run build
```

Rust:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml conversation
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

`bun` and `bunx` are mandatory for JavaScript tooling in this repository. `npm` and `npx` must not be used.

## 23. Rollout Strategy

The delivery order is:

1. Provider client IDs and Zalo adapter changes.
2. Additive SQLite migration.
3. Durable staging and cleanup.
4. Batch enqueue and part worker.
5. Echo reconciliation and recall.
6. TypeScript contracts and service wrappers.
7. Composer rich controls.
8. Rich rendering, message actions, and reactions.
9. Full automated validation.
10. Live Zalo smoke gate.

Rich controls remain disabled unless backend capabilities enable them. Non-Zalo channels continue to use their current text behavior. The legacy text command remains a compatibility wrapper during Phase 1.

Database rollback does not require dropping the additive tables or columns. An older build ignores them, while the feature can be disabled by withholding rich capabilities.

## 24. Live Zalo Smoke Gate

Before Phase 1 is considered releasable, a dedicated non-production Zalo account and consenting recipient must verify:

- Text send.
- Image send.
- Document send.
- Video and audio send.
- A mixed text and multi-attachment batch.
- Live sticker search and send.
- Quote of incoming and reconciled outbound messages.
- Recall and tombstone rendering.
- Reaction actor display.
- Partial delivery during a controlled network interruption.
- Restart behavior with staged and pending files.
- No duplicate bubble after realtime/history echo.

Only non-sensitive test files may be used.

## 25. Acceptance Criteria

Phase 1 is complete when:

- One composer action creates one logical batch.
- Every text or attachment part has an independent durable outbox lifecycle.
- Successful parts are never resent because another part failed.
- Staged files survive restart and do not depend on their original source paths.
- Exact provider identifiers prevent duplicate outbound bubbles.
- Quote Reply never silently degrades when provider quote identifiers are unavailable.
- Sticker selection uses live Zalo APIs rather than a bundled provider catalog.
- Recall keeps a tombstone and only succeeds after provider confirmation.
- Reactions display counts, actors, and the current-account marker without write actions.
- Rich UI behavior is capability-driven.
- Non-Zalo channels retain existing behavior.
- Targeted tests, full tests, Svelte checks, Rust formatting, build, and approved lint gates pass or have unrelated pre-existing failures explicitly documented.
- GitNexus change detection shows only expected Conversation, Zalo adapter, vendored provider, Tauri registration, and conversation UI flows.

## 26. Implementation Guardrails

- Run GitNexus impact analysis before modifying every function, class, method, or shared type.
- Warn before proceeding with any HIGH or CRITICAL blast radius.
- Run GitNexus change detection at implementation checkpoints and before completion.
- Preserve unrelated working-tree changes.
- Do not commit unless explicitly requested.
- Do not add reaction write, forwarding, voice recording, templates, mentions, media library, folders, CRM panel, or AI features under this Phase 1 plan.
