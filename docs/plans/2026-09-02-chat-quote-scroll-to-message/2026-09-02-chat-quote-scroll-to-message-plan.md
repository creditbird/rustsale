# Chat Quote Click Scroll Navigation Plan

## Metadata

- Date: `2026-09-02`
- Status: `completed`
- Plan: `docs/plans/2026-09-02-chat-quote-scroll-to-message/2026-09-02-chat-quote-scroll-to-message-plan.md`
- Evidence: `docs/plans/2026-09-02-chat-quote-scroll-to-message/2026-09-02-chat-quote-scroll-to-message-evidence.md`
- Benchmark: `docs/plans/2026-09-02-chat-quote-scroll-to-message/2026-09-02-chat-quote-scroll-to-message-benchmark.md`
- Actual status: `docs/plans/2026-09-02-chat-quote-scroll-to-message/2026-09-02-chat-quote-scroll-to-message-actual-status.md`

## Goal

Ensure clicking on a quoted message preview (`MessageReplyPreview`) inside a chat bubble scrolls smoothly to the referenced message in the timeline and highlights it with a temporary flash pulse animation, instead of incorrectly setting the message composer's reply bar.

## Rules

- Complete P0 actual status before implementation work.
- Update each checklist item immediately when it is completed.
- Record evidence as work completes.
- Record benchmarkable counts or measurements when they are taken.
- Run Anvien detect-changes before every implementation-slice commit when implementation work was performed.
- Validate behavior with unit/component tests and verification commands.

## Problem

When a message bubble contains a `replyTarget` (a quoted earlier message), clicking on `MessageReplyPreview` was calling `onReply(message)`. This caused the message composer to open a reply draft for the *current* message, rather than navigating to the *quoted* message in the conversation timeline.

## Scope

- Wire `onJumpToMessage` prop from `ConversationWorkspace.svelte` -> `ConversationTimeline.svelte` -> `MessageBatchGroup.svelte` / `MessageBubble.svelte`.
- In `MessageBubble.svelte`, trigger `onJumpToMessage(message.replyTarget.messageId)` on clicking `MessageReplyPreview` when `message.replyTarget.messageId` is present.
- Enhance `scrollToMatchedMessage` in `ConversationWorkspace.svelte` to support visual pulse/flash highlight and virtual window scroll.
- Update unit tests in `src/lib/components/conversation/__tests__/MessageBubble.spec.ts` to assert the new behavior.

## Non-Goals

- Changing backend data structures or IPC protocols.
- Altering the composer's draft reply mechanics when clicking "Reply" from context menu.

## Requirements

1. Clicking `MessageReplyPreview` must invoke `onJumpToMessage(replyTarget.messageId)`.
2. When the target message exists in the timeline DOM, it smoothly scrolls into view (`center` align) and applies a visual highlight pulse (`ring-2 ring-primary bg-primary/10 transition-all duration-500`).
3. If `replyTarget.messageId` is null or not found, no crash occurs, and safe fallback handling is used.
4. "Reply" action from `MessageActionsMenu` must continue to invoke `onReply(message)` to draft a reply.

## Acceptance Criteria

1. All conversation component tests pass (`bun test`).
2. Type checking and full build succeed without errors (`bun run check`, `bun run build`).
3. Clicking a quote block jumps to the target message without altering the composer reply state.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: Establish baseline state and verify blast radius.
  - Acceptance: `actual-status.md` completed with R0 status matrix.

### P1: Implement Quote Click Scroll Navigation

- Phase Goal: Wire `onJumpToMessage` through timeline components and update `MessageBubble.svelte`.
- Phase Boundary:
  - In scope: `MessageBubble.svelte`, `MessageBatchGroup.svelte`, `ConversationTimeline.svelte`, `ConversationWorkspace.svelte`, test suite.
  - Out of scope: Backend services.

- [x] P1-A: Wire `onJumpToMessage` callback and update `MessageBubble.svelte`
  - Goal: Disconnect `onReply` from `MessageReplyPreview` and route `replyTarget.messageId` to `onJumpToMessage`.
  - Scope Boundary:
    - Editable: `src/lib/components/conversation/MessageBubble.svelte`, `src/lib/components/conversation/MessageBatchGroup.svelte`, `src/lib/components/conversation/ConversationTimeline.svelte`, `src/lib/components/conversation/ConversationWorkspace.svelte`, `src/lib/components/conversation/__tests__/MessageBubble.spec.ts`.
  - Work Steps:
    1. Update `MessageBubble.svelte` to accept `onJumpToMessage?: (messageId: number) => void` and call it on `MessageReplyPreview` click.
    2. Forward `onJumpToMessage` through `MessageBatchGroup.svelte` and `ConversationTimeline.svelte`.
    3. Pass `scrollToMatchedMessage` as `onJumpToMessage` in `ConversationWorkspace.svelte`.
    4. Update tests in `MessageBubble.spec.ts` to assert `onJumpToMessage` is called with target message ID.
    5. Run `bun run check` and `bun run build` and `bun test`.
  - Implementation Gate: Anvien impact check completed.
  - Acceptance: Quote click triggers jump navigation without setting composer draft. Passed with 368 tests passing, 0 type errors, clean static adapter build.
