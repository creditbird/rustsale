# Chat Quote Click Scroll Navigation Actual Status

Title: Chat Quote Click Scroll Navigation
Date: 2026-09-02
Status: Completed
Companion plan: `docs/plans/2026-09-02-chat-quote-scroll-to-message/2026-09-02-chat-quote-scroll-to-message-plan.md`
Companion evidence: `docs/plans/2026-09-02-chat-quote-scroll-to-message/2026-09-02-chat-quote-scroll-to-message-evidence.md`
Companion benchmark: `docs/plans/2026-09-02-chat-quote-scroll-to-message/2026-09-02-chat-quote-scroll-to-message-benchmark.md`

## Purpose

This file records the real current state before implementation.
Implementation must not start until the target scope has a completed status row, evidence IDs, and a downstream plan decision.

## Scope

Target scope:
- `MessageBubble.svelte` (binding `onJumpToMessage` instead of `onReply` when clicking `MessageReplyPreview`)
- `MessageBatchGroup.svelte` (passing `onJumpToMessage` prop)
- `ConversationTimeline.svelte` (passing `onJumpToMessage` prop)
- `ConversationWorkspace.svelte` (providing `scrollToMatchedMessage` handler as `onJumpToMessage` to timeline)

Out of scope:
- Backend IPC handlers, database schemas, channel connectors.

## Relationship / Impact Evidence

| Unit / File / Surface | File Detail Evidence | Related File Count | Relationship Summary | Impact Note |
|---|---|---|---|---|
| `src/lib/components/conversation/MessageBubble.svelte` | `E0-P0A-SRC1` | 14 related files | Direct consumer of MessageReplyPreview, rendered by Timeline and BatchGroup | Low blast radius |
| `src/lib/components/conversation/MessageReplyPreview.svelte` | `E0-P0A-SRC2` | 3 related files | Renders quote header & content inside bubble | Low blast radius |
| `src/lib/components/conversation/ConversationTimeline.svelte` | `E0-P0A-SRC3` | 8 related files | Virtual message list and scroll container | Low blast radius |
| `src/lib/components/conversation/ConversationWorkspace.svelte` | `E0-P0A-SRC4` | 22 related files | Workspace orchestration, contains `scrollToMatchedMessage` | Medium blast radius |

## Current Status Matrix

| Unit | Current State | Required State | Status | Relationship Count | Evidence | Next Plan Decision |
|---|---|---|---|---|---|---|
| `MessageBubble.svelte` quote click | Invokes `onJumpToMessage(replyTarget.messageId)` on clicking quote preview | Calls `onJumpToMessage(replyTarget.messageId)` when `replyTarget.messageId` is present | `correct` | 14 related files | `E1-P1A-TEST1` | Done |
| `MessageBatchGroup.svelte` props | Accepts and forwards `onJumpToMessage` to `MessageBubble` | Accepts and forwards `onJumpToMessage` to `MessageBubble` | `correct` | 4 related files | `E1-P1A-SRC1` | Done |
| `ConversationTimeline.svelte` props | Accepts and forwards `onJumpToMessage` to child bubbles and batch groups | Accepts and forwards `onJumpToMessage` to child bubbles and batch groups | `correct` | 8 related files | `E1-P1A-SRC2` | Done |
| `ConversationWorkspace.svelte` wire-up | `scrollToMatchedMessage` is passed as `onJumpToMessage` to `ConversationTimeline` and uses timeline virtual scrolling | `scrollToMatchedMessage` is passed as `onJumpToMessage` to `ConversationTimeline` | `bound-correct` | 22 related files | `E1-P1A-SRC3` | Done |

## Status Refresh Log

| Refresh | Date | Repo Basis | Changed Scope | Status Changes | Evidence | Next Phase Update |
|---|---|---|---|---|---|---|
| R0 | 2026-09-02 | commit 324bff9 | Chat quote click navigation | Initial classification: wrong/missing/unbound | `E0-P0A-SRC1..SRC4` | Proceed to P1-A implementation |
| R1 | 2026-09-02 | after P1-A | Chat quote click navigation | wrong/missing/unbound -> correct/bound-correct | `E1-P1A-SRC1..TEST1` | Plan completed |
