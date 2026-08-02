# Zalo Rich Chat Phase 2B Voice and Conversation Media Design

## 1. Goal

Phase 2B adds two related media workflows on top of Zalo Rich Chat Phase 1:

- Record, preview, stage, send, reconcile, and render native Zalo voice messages.
- Browse media already present in one conversation through a safe, paginated Image, Video, Audio/Voice, File, and Link panel.

Phase 2B remains local-first. Rust owns durable files and provider operations. The frontend owns temporary recording controls and object URLs but never receives arbitrary filesystem paths.

## 2. Prerequisites and Relationship to Other Phases

Phase 2B requires the Phase 1 design and implementation:

- Durable application-data spool.
- Opaque staged-attachment IDs.
- Batch/part outbox processing.
- Rich attachment and link projection.
- Safe HTTP/HTTPS URL rendering.
- Provider echo reconciliation by exact IDs.
- Audio and video renderers.

Phase 2B does not depend on Phase 2A. Both may be implemented after Phase 1 and released independently through capabilities.

## 3. Confirmed Scope

### 3.1 Included

#### Voice recording and send

- Microphone permission requested only after explicit user action.
- Start, stop, preview, cancel, and send workflow.
- Maximum recording duration of five minutes.
- Maximum staged voice size of 5 MiB.
- Runtime MIME support detection.
- Durable Rust staging before enqueue.
- Native Zalo voice send through a vendored `zca-rs` API.
- Stable caller-controlled client ID.
- Independent outbox lifecycle and exact provider echo reconciliation.
- Voice-specific bubble with playback progress, duration, and speed control.

#### Conversation Media Library

- Per-conversation panel.
- Tabs for Image, Video, Audio/Voice, File, and Link.
- Cursor pagination with 40 items per page.
- Per-tab counts.
- Safe preview/open behavior.
- Jump to the original message through a bounded context query.
- Cache invalidation from new messages, reconciliation, and recalls.

### 3.2 Explicitly excluded

- Global media asset management.
- Uploading reusable media independent of a conversation send.
- Media folders, tags, favorites, usage ranking, review workflow, or suggestions.
- Re-sending an archived media item from the library.
- Voice transcription, speech-to-text, waveform search, or AI summarization.
- Pause/resume recording.
- Server-side transcoding or bundling FFmpeg.
- Background microphone access.
- Full message-content search.
- CRM/AI features.
- Voice writes for Email, WhatsApp, or Messenger.

## 4. Chosen Architecture

### 4.1 Web recorder plus Rust-owned spool

The frontend uses the Web `MediaRecorder` API only for capture. The resulting Blob is sent to a dedicated capped Tauri command. Rust validates MIME, bytes, duration, and checksum, then writes the data into the existing Conversation spool and returns an opaque stage ID.

No source path is accepted. No recording remains frontend-only after the user chooses Send.

### 4.2 Native voice outbox part

Voice is a dedicated send part rather than a generic file attachment flag:

- The provider operation is `sendVoice`, not generic file send.
- Duration is first-class metadata.
- Projection renders `contentType = 'voice'`.
- Retry and delivery-unknown behavior reuse the Phase 1 outbox.

### 4.3 Read model over canonical messages

The media library queries existing `messages` and `message_attachments`. It does not create a second asset store or copy remote media.

Link items come from normalized link messages/attachments already persisted by Phase 1. The read model excludes tombstoned messages.

## 5. Runtime and Provider Probe

Phase 2B starts with a probe before enabling `voiceRecording`.

### 5.1 Recorder runtime matrix

On every supported desktop runtime, record at least 10 seconds for each supported MIME candidate:

1. `audio/webm;codecs=opus`
2. `audio/webm`
3. `audio/ogg;codecs=opus`
4. `audio/mp4`

Record:

- `MediaRecorder.isTypeSupported` result.
- Actual Blob MIME.
- Actual extension selected by the staging mapper.
- Blob size and duration.
- Local preview playback result.

Runtime capability is enabled only when at least one probed MIME records and previews correctly.

### 5.2 Native Zalo voice probe

For every runtime-selected MIME, verify direct and group send through the provider's native voice endpoint. Capture sanitized fixtures for:

- Upload request metadata.
- Duration units.
- Stable client ID placement.
- Provider response IDs.
- Realtime and history echo content types.
- Remote URL and duration metadata.

The release requires a provider-supported MIME on each supported desktop platform. If no candidate works on a platform, voice capability stays disabled there and implementation does not silently send the Blob as a generic file.

### 5.3 Size and duration probe

Verify provider behavior at:

- Empty recording.
- One second.
- Five minutes.
- Just below and above 5 MiB.

Application limits remain 5 minutes and 5 MiB even if the provider accepts more.

## 6. Data Model

### 6.1 Staged voice metadata

Phase 1 `conversation_staged_attachments` gains additive nullable columns:

```sql
ALTER TABLE conversation_staged_attachments ADD COLUMN duration_ms INTEGER;
ALTER TABLE conversation_staged_attachments ADD COLUMN media_role TEXT;
```

For voice records:

- `media_role = 'voice'`.
- `duration_ms` is between 1 and 300,000.
- MIME and checksum use existing fields.
- The staged file follows the same staged/claimed lifecycle and TTL cleanup as other attachments.

### 6.2 Message attachment metadata

Voice projection persists:

- `attachment_type = 'voice'`.
- Safe remote URL when available.
- MIME type.
- Size.
- Duration.
- Provider metadata required for future playback normalization.

No waveform samples or audio bytes are stored in SQLite.

### 6.3 Media indexes

Add or verify indexes supporting:

```text
messages(conversation_id, removed_at, occurred_at DESC, id DESC)
message_attachments(message_id, attachment_type, id)
```

Link queries use the normalized content type and message ordering indexes. Do not add SQLite FTS in Phase 2B.

## 7. Conversation Capabilities

Capabilities extend additively:

```ts
type ConversationCapabilities = {
  // Existing fields remain unchanged.
  voiceRecording: {
    enabled: boolean;
    maxDurationMs: number;
    maxBytes: number;
    supportedMimeTypes: string[];
  };
  mediaLibrary: {
    enabled: boolean;
    pageSize: number;
    kinds: ConversationMediaTabKind[];
  };
};
```

The backend returns provider support. A frontend runtime helper intersects provider MIME support with `MediaRecorder.isTypeSupported` before enabling the microphone button.

## 8. Voice Contracts

### 8.1 Stage request

```ts
type StageConversationVoiceRequest = {
  conversationId: number;
  bytesBase64: string;
  mimeType: string;
  durationMs: number;
};

type StagedConversationVoice = {
  stagedAttachmentId: string;
  fileName: string;
  mimeType: string;
  sizeBytes: number;
  durationMs: number;
  previewDataUrl: null;
};
```

The backend generates the filename and extension. The frontend never supplies or receives a local path.

### 8.2 Send part

```ts
type ConversationVoicePart = {
  clientPartId: string;
  type: 'voice';
  stagedAttachmentId: string;
  durationMs: number;
};
```

One Phase 2B voice send is a voice-only batch containing exactly one voice part. It cannot include text, another voice, an attachment, or a sticker. Captions and mixed voice batches remain outside this phase.

## 9. Media Library Contracts

```ts
type ConversationMediaTabKind = 'image' | 'video' | 'audio' | 'file' | 'link';

type ConversationMediaItemKind =
  | 'image'
  | 'video'
  | 'audio'
  | 'voice'
  | 'file'
  | 'link';

type ConversationMediaCursor = {
  occurredAt: string;
  messageId: number;
  attachmentId: number | null;
};

type ConversationMediaItem = {
  kind: ConversationMediaItemKind;
  conversationId: number;
  messageId: number;
  attachmentId: number | null;
  direction: 'inbound' | 'outbound';
  senderDisplayName: string | null;
  occurredAt: string;
  fileName: string | null;
  mimeType: string | null;
  sizeBytes: number | null;
  durationMs: number | null;
  thumbnailUrl: string | null;
  remoteUrl: string | null;
  linkTitle: string | null;
  linkDescription: string | null;
  linkDomain: string | null;
};

type ListConversationMediaRequest = {
  conversationId: number;
  kind: ConversationMediaTabKind;
  cursor?: ConversationMediaCursor;
  limit?: number;
};

type ConversationMediaPage = {
  items: ConversationMediaItem[];
  nextCursor: ConversationMediaCursor | null;
};

type ConversationMediaCounts = Record<ConversationMediaTabKind, number>;
```

Commands:

- `list_conversation_media`
- `get_conversation_media_counts`
- `get_conversation_message_context`

## 10. Voice Recording Workflow

1. UI checks account, provider, runtime, and permission capability.
2. User presses Record.
3. UI calls `getUserMedia({audio: true})` and creates `MediaRecorder` with the first supported MIME from the capability intersection.
4. UI collects chunks and updates a monotonic timer.
5. Recording auto-stops at five minutes.
6. Stop creates one Blob and one object URL for local preview.
7. Cancel stops tracks, revokes the URL, and clears bytes.
8. Send converts the Blob to chunked base64 and calls `stage_conversation_voice`.
9. Rust decodes with a 5 MiB decoded-byte limit, validates MIME, size, duration, checksum, and writes the spool record.
10. UI enqueues a voice-only batch using the opaque stage ID.
11. On successful enqueue, UI revokes the preview URL and clears only the submitted recording snapshot.
12. Worker claims the staged voice and calls native `sendVoice`.
13. Provider echo reconciles the local part using exact provider IDs.

If staging or enqueue fails, preview remains available for explicit retry or cancel during the current app session.

## 11. Voice Outbox and Recovery

- Voice uses the Phase 1 job claim, retry classification, and per-account ordering.
- Provider client ID is generated and persisted before the first provider call.
- The vendored native voice API accepts that caller-controlled client ID.
- Temporary network and rate-limit errors use bounded Phase 1 retry.
- A running job after restart becomes delivery unknown.
- Delivery unknown is never automatically resent.
- A later provider echo can complete a delivery-unknown voice part without inserting a duplicate message.
- Claimed spool bytes remain available until confirmed success, permanent failure cleanup policy, or operator/manual discard policy.

## 12. Voice Rendering

`VoiceMessageContent.svelte` renders:

- Play/pause.
- Current time and total duration.
- Progress seek when the browser permits seeking.
- Playback speed: 1×, 1.5×, 2×.
- Loading and safe fallback state.

Remote sources must pass the Phase 1 absolute HTTP/HTTPS sanitizer. The component never receives a local spool path.

Only one voice item in the Conversation workspace plays at a time. Starting another pauses the current player through a small shared playback coordinator.

## 13. Conversation Media Query

### 13.1 Ordering and cursor

Items order by:

```text
occurred_at DESC, message_id DESC, attachment_id DESC NULLS LAST
```

The cursor is exclusive and stable. The command clamps limits to 1–40.

### 13.2 Kind mapping

- `image`: image attachments.
- `video`: video attachments.
- `audio`: non-voice audio and native voice items; each result keeps its exact `audio` or `voice` item kind.
- `file`: document/archive/other file attachments.
- `link`: normalized provider link-preview messages.

Tombstoned messages and attachments without safe display metadata are excluded.

### 13.3 Counts

Counts use the same eligibility predicates as list queries. Counts and pages therefore cannot disagree because of removed messages or content-type mapping.

## 14. Jump to Original Message

`get_conversation_message_context` accepts:

```ts
type GetConversationMessageContextRequest = {
  conversationId: number;
  messageId: number;
  before?: number;
  after?: number;
};
```

The backend clamps `before` and `after` to 30 each and returns a bounded ordered timeline slice plus anchor metadata.

The UI:

1. Closes or collapses the media preview as appropriate.
2. Merges the context page into the timeline cache by message ID.
3. Scrolls to the anchor `data-message-id`.
4. Applies a temporary highlight.

It never loads unlimited history to find an anchor.

## 15. Media Library UI

### 15.1 Desktop

Use a collapsible right-side `ConversationMediaPanel` with tabs and count badges.

### 15.2 Narrow layouts

Use a Sheet containing the same tab state and page cache. Desktop and Sheet share one state module rather than duplicating fetch logic.

### 15.3 Items

- Images use a bounded thumbnail grid.
- Videos show a thumbnail/fallback and duration.
- Audio/voice show filename or sender plus duration.
- Files show MIME icon, filename, size, and date.
- Links show title, domain, description, and safe external-open action.

The panel has loading, empty, error, retry, and pagination states.

## 16. Cache and Invalidation

Frontend cache key:

```text
conversationId:mediaKind
```

Each entry stores pages, next cursor, loading/error state, and a generation number.

Invalidate only the affected conversation when:

- A realtime/history message adds eligible media.
- An outbound queued message reconciles with remote metadata.
- A message is recalled or tombstoned.
- Conversation identity merge moves messages into another canonical thread.

Counts invalidate with pages. Switching conversations never displays stale items from the previous key.

## 17. Error Model

Phase 2B adds:

- `MicrophoneUnavailable`
- `MicrophonePermissionDenied`
- `MediaRecorderUnsupported`
- `VoiceMimeUnsupported`
- `VoiceDurationExceeded`
- `VoiceSizeExceeded`
- `VoiceEmpty`
- `VoiceStageInvalid`
- `VoiceSendUnsupported`
- `VoiceDeliveryUnknown`
- `MediaKindUnsupported`
- `MediaAnchorNotFound`

Browser permission errors are normalized in the frontend. Rust/provider errors use structured `{code,message}` errors.

## 18. Security and Resource Safety

- Microphone access begins only after a click/tap.
- All tracks stop on stop, cancel, send, unmount, conversation switch, and account switch.
- Object URLs are revoked on replacement and cleanup.
- Base64 conversion is chunked to avoid call-stack overflow.
- Rust enforces decoded-byte limits before allocation grows beyond the configured cap.
- MIME is allowlisted and verified against bytes when practical.
- Empty, malformed, oversized, and over-duration recordings are rejected.
- No arbitrary local path crosses IPC.
- No spool path, cookies, tokens, or account session data reaches the UI.
- Library URLs permit only absolute HTTP/HTTPS values.
- The library does not fetch third-party metadata.
- Tombstoned media is not exposed through the library.

## 19. Testing Strategy

### 19.1 Runtime/platform tests

- MIME selection order.
- Unsupported runtime disables capability.
- Permission denial normalization.
- Timer and five-minute auto-stop.
- Track and object-URL cleanup.
- Blob staging request excludes data-URL prefixes and local paths.

### 19.2 Vendored provider tests

- Native voice direct and group payloads.
- Caller-controlled client IDs.
- Duration conversion.
- Response ID normalization.
- Probe fixture deserialization for realtime/history voice echoes.

### 19.3 Rust tests

- Additive migration.
- Voice byte, MIME, duration, checksum, stage ownership, claim, discard, and cleanup.
- Voice-only batch validation.
- Outbox retry and delivery-unknown behavior.
- Echo reconciliation without duplicate bubbles.
- Media kind mapping, cursor boundaries, counts, and tombstone exclusion.
- Bounded message context and cross-conversation rejection.

### 19.4 Frontend tests

- Recorder state machine.
- Preview, cancel, retry, and send snapshot clearing.
- Voice player coordination and speed control.
- Media tab loading, pagination, cache isolation, invalidation, and empty/error states.
- Safe URL behavior.
- Jump-to-message merge, anchor, and highlight.
- Capability gating without direct Zalo channel checks.

## 20. Rollout

Rollout order:

1. Runtime and provider probes.
2. Additive metadata and indexes.
3. Media read commands and panel behind `mediaLibrary` capability.
4. Message-context anchor command.
5. Vendored native voice API with stable client IDs.
6. Voice staging and outbox part.
7. Recorder and voice bubble behind `voiceRecording` capability.
8. Full automated validation.
9. Live smoke gate on every supported desktop runtime.

Media Library ships before Voice because it relies only on Phase 1 canonical data. Voice remains disabled until runtime and provider probes pass.

Rollback disables the two capabilities independently. Additive schema remains readable and does not require destructive rollback.

## 21. Live Smoke Gate

Using a non-production account and consenting recipient:

1. Record, preview, cancel, and re-record.
2. Deny microphone permission and confirm recovery instructions.
3. Send short direct voice.
4. Send short group voice.
5. Send a recording near five minutes without exceeding 5 MiB.
6. Reject recordings above configured limits.
7. Interrupt network during voice send and confirm no automatic resend after delivery uncertainty.
8. Restart with a queued voice and confirm safe recovery.
9. Confirm realtime/history echo produces one voice bubble.
10. Verify player progress and 1×/1.5×/2× speed.
11. Browse every media tab with more than 40 items.
12. Recall a media message and confirm it disappears from the library.
13. Jump to an original message outside the current timeline window.
14. Confirm no local path or unsafe URL appears in frontend data.

## 22. Acceptance Criteria

Phase 2B is complete when:

- Recorder capability reflects both provider and current runtime support.
- Microphone and object-URL resources always clean up.
- Recorded bytes enter the Rust-owned durable spool before enqueue.
- Native voice send uses a stable provider client ID.
- Delivery-unknown voice jobs are never automatically resent.
- Provider echo reconciles voice without duplicate bubbles.
- Voice playback is safe and coordinated.
- Media Library reads canonical Conversation data without a second asset store.
- Pagination and counts use identical eligibility rules.
- Tombstoned media is excluded.
- Jump-to-message loads only bounded context.
- Frontend receives no arbitrary local paths or auth/session data.
- All behavior is capability-driven.
- Non-Zalo channels preserve existing behavior.
- Targeted tests, full tests, Svelte checks, Rust formatting, build, and lint gates pass or unrelated baseline failures are documented exactly.
- GitNexus change detection shows only expected Conversation, Zalo adapter, vendored provider, staging, media query, command, and UI flows.

## 23. Implementation Guardrails

- Run Agent Memory recall before implementation.
- Run GitNexus impact before modifying every existing symbol.
- Stop and warn before HIGH or CRITICAL changes.
- Preserve unrelated working-tree changes.
- Use `bun` and `bunx`; never use `npm` or `npx`.
- Use TDD for every behavior change.
- Do not commit unless explicitly requested.
- Do not add transcoding, global media CMS, folders/tags, resend-from-library, transcription, CRM, AI, or multi-channel voice writes.
