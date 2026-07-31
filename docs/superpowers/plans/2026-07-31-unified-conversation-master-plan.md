# Unified Omnichannel Conversation Master Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a local-first Unified Conversation module whose first production adapter is Zalo personal-session automation, with direct customer timelines, separate group conversations, channel badges, durable synchronization, and Outreach history integration.

**Architecture:** Add a focused `conversation` Rust module beside the existing CRM commands. SQLite stores canonical provider conversations and messages; long-lived Zalo clients feed normalized events into transactional repositories, and compact Tauri invalidation events update a Svelte conversation workspace. The implementation is split into five dependency-ordered child plans so each subsystem is independently testable.

**Tech Stack:** Rust 2021, Tauri 2, Tokio, SQLite/rusqlite, vendored `zca-rs`, Svelte 5, TypeScript, shadcn-svelte, Bun test tooling.

---

## Source Design

Implement against:

- `docs/superpowers/specs/2026-07-31-unified-conversation-design.md`

Do not begin production implementation until the live Zalo synchronization spike passes.

## Child Plans

Execute in this exact order:

1. `docs/superpowers/plans/2026-07-31-zalo-conversation-spike-plan.md`
2. `docs/superpowers/plans/2026-07-31-conversation-storage-foundation-plan.md`
3. `docs/superpowers/plans/2026-07-31-zalo-conversation-sync-plan.md`
4. `docs/superpowers/plans/2026-07-31-conversation-ui-plan.md`
5. `docs/superpowers/plans/2026-07-31-outreach-conversation-integration-plan.md`

Dependency graph:

```text
Zalo live spike
    |
    v
Storage + read contracts
    |
    v
Zalo sync + send runtime
    |
    +------------+
    |            |
    v            v
Conversation UI Outreach persistence
```

The UI plan depends on stable storage commands. Outreach integration depends on stable repository helpers but can run after the Zalo sync plan without waiting for all UI polish.

## Required File Boundaries

The implementation must use these focused modules instead of adding conversation logic to `src-tauri/src/commands.rs` or `src-tauri/src/db.rs`:

```text
src-tauri/src/conversation/
  mod.rs
  schema.sql
  migration.rs
  types.rs
  repository.rs
  events.rs
  commands.rs
  service.rs
  adapter.rs
  outreach.rs
  zalo/
    mod.rs
    client.rs
    normalizer.rs
    supervisor.rs
    probe.rs
    probe_test.rs
  migration_test.rs
  repository_test.rs
  service_test.rs
  commands_test.rs
  outreach_test.rs

src/lib/services/
  conversationService.ts
  __tests__/
    conversationService.spec.ts

src/lib/components/conversation/
  ConversationWorkspace.svelte
  ConversationSidebar.svelte
  ChannelBadges.svelte
  ConversationTimeline.svelte
  MessageBubble.svelte
  MessageComposer.svelte
  UnknownContactActions.svelte
  timelineState.ts
  __tests__/
```

Existing files receive only integration changes:

- `src-tauri/src/db.rs`: call the isolated conversation migration.
- `src-tauri/src/lib.rs`: register state, startup recovery, and commands.
- `src-tauri/src/commands.rs`: persist successful Outreach sends through one helper.
- `src/routes/+page.svelte`: add the Conversation workspace tab.
- `src/lib/components/Sidebar.svelte`: add the Conversation navigation item.

## Shared Backend Contracts

The child plans must use these exact names consistently.

### Domain enums

```rust
pub enum Channel {
    Zalo,
    WhatsApp,
    Messenger,
    Email,
}

pub enum ConversationKind {
    Direct,
    Group,
    EmailThread,
}

pub enum MessageDirection {
    Inbound,
    Outbound,
    System,
}

pub enum MessageStatus {
    Queued,
    Sending,
    Sent,
    Delivered,
    Read,
    Received,
    Failed,
    Removed,
}
```

All enum JSON and database values use `snake_case`.

All Conversation timestamps and timeline cursors use UTC RFC 3339 with fixed millisecond precision and a `Z` suffix.

### Composer reply target

```rust
pub struct ConversationReplyTarget {
    pub conversation_id: i64,
    pub channel_account_id: i64,
    pub channel: Channel,
    pub account_display_name: Option<String>,
    pub conversation_kind: ConversationKind,
    pub last_inbound_at: Option<String>,
    pub last_message_at: Option<String>,
}
```

Every inbox row includes `reply_targets`. Customer/unknown rows may expose multiple direct provider conversations; group rows expose exactly one locked target.

Group inbox rows also include active participant count and a bounded participant preview. Customer/unknown rows return an empty participant preview.

### Tauri command names

```text
list_conversation_inbox
get_contact_timeline
get_conversation_timeline
link_conversation_identity
send_conversation_message
get_conversation_runtime_status
restart_conversation_account
```

### Tauri event name

```text
conversation-invalidated
```

### Frontend service names

```ts
listConversationInbox()
getContactTimeline()
getConversationTimeline()
linkConversationIdentity()
sendConversationMessage()
getConversationRuntimeStatus()
restartConversationAccount()
initConversationEventBridge()
onConversationInvalidated()
```

## GitNexus Risk Gates

Impact analysis performed while writing this plan:

| Symbol | Risk | Direct dependents | Required action |
|---|---:|---:|---|
| `init_db` | CRITICAL | 34 | Keep SQL in `conversation/migration.rs`; run all database and Rust tests after the one-line integration call |
| `run` | LOW | 1 | Test app startup and state registration |
| `send_zalo_outreach_internal` | LOW | 4 | Update its two direct callers/tests when return handling changes |
| `send_zalo_campaign_internal` | MEDIUM | 5 | Run all four campaign regression tests plus new conversation persistence tests |
| `start_zalo_outreach` | LOW | 0 graph callers | Add pure partition tests so selected IMEIs distribute leads instead of each receiving the full list |

Before modifying any additional existing function, class, or method during execution, run `mcp__gitnexus.impact` for that symbol. Stop and warn the user before editing any symbol reported as HIGH or CRITICAL.

## Global Test Commands

Repository commands must use Bun where applicable.

Targeted Rust conversation suite:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::
```

Existing Zalo outreach regression suite:

```bash
cargo test --manifest-path src-tauri/Cargo.toml test_send_zalo_
```

Full Rust suite:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Frontend targeted suite:

```bash
bun test src/lib/services/__tests__/conversationService.spec.ts src/lib/components/conversation/__tests__
```

Frontend full verification:

```bash
bun run test
bun run check
bun run build
```

Formatting and diff checks:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
git diff --check
```

Do not fix unrelated existing test failures. Record them separately with the exact failing command and error.

## Execution Checkpoints

After every task:

1. Run the task's targeted test.
2. Run `git diff --check`.
3. Review changed files against the task's declared write set.
4. Do not commit unless the user explicitly authorizes commits.

After every child plan:

1. Run `mcp__gitnexus.detect_changes({scope: "all"})`.
2. Confirm all d=1 dependents from prior impact reports were updated.
3. Run the child plan's broader verification commands.
4. Update Agent Memory with completed behavior, files, validation, and known limitations.

## Completion Definition

Phase 0–4 is complete only when:

- Live Zalo probe verifies direct/group realtime events and backfill semantics.
- Conversation migrations are idempotent and preserve every existing table/test.
- Zalo sessions remain connected through a supervised runtime.
- Direct, group, reaction, undo, delivered, and seen events persist idempotently.
- A lead timeline aggregates direct messages across its linked channels/accounts.
- Group messages never enter a private lead timeline.
- Channel badges filter loaded and subsequently fetched messages.
- Composer sends through the selected Zalo IMEI and records provider IDs.
- Outreach to N recipients produces N direct conversations and zero groups.
- Existing Zalo and Email Outreach remain functional.
- Targeted and full validation commands pass, except explicitly documented unrelated pre-existing failures.
