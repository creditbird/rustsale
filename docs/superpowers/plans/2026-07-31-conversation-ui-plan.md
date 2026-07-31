# Unified Conversation UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Zalo/WhatsApp-style Conversation workspace that renders unified direct-customer history, separate provider groups, default-active channel badges, cursor pagination, account-aware sending, runtime status, and unknown-contact linking.

**Architecture:** A dedicated `conversationService.ts` owns Tauri IPC types and one ref-counted invalidation bridge. `ConversationWorkspace.svelte` orchestrates an inbox read model, per-channel-set timeline caches, runtime account state, and stale-view refresh; focused presentational components render the sidebar, badges, bounded timeline, bubbles, composer, and identity actions. Direct rows use the unified lead timeline, while group rows always load one canonical provider conversation and lock sending to its account.

**Tech Stack:** Svelte 5 runes, TypeScript, Tauri 2 event/core APIs, shadcn-svelte, lucide-svelte, Bun test runner, Testing Library, Tailwind CSS 4.

---

## Preconditions and Risk Gates

- Complete `docs/superpowers/plans/2026-07-31-conversation-storage-foundation-plan.md` and `docs/superpowers/plans/2026-07-31-zalo-conversation-sync-plan.md` first.
- Confirm `InboxItem.replyTargets` exists and every direct/group row exposes the exact `conversationId` and `channelAccountId` required by `send_conversation_message`.
- Re-run GitNexus impact before editing `src/routes/+page.svelte` and `src/lib/components/Sidebar.svelte`. The current file-level page impact is LOW.
- Keep the existing single-page tab architecture. Do not introduce a second router or frontend state framework.
- Use only `bun` and `bunx --bun` for JavaScript tooling.

## Task 1: Add Required shadcn-svelte Primitives

**Files:**
- Create through CLI: `src/lib/components/ui/field/`
- Create through CLI: `src/lib/components/ui/textarea/`
- Create through CLI: `src/lib/components/ui/scroll-area/`
- Create through CLI: `src/lib/components/ui/skeleton/`
- Create through CLI: `src/lib/components/ui/toggle-group/`
- Modify through CLI: `package.json`
- Modify through CLI: `bun.lock`

- [ ] **Step 1: Verify the components are absent**

Run:

```bash
test ! -d src/lib/components/ui/field
test ! -d src/lib/components/ui/textarea
test ! -d src/lib/components/ui/scroll-area
test ! -d src/lib/components/ui/skeleton
test ! -d src/lib/components/ui/toggle-group
```

Expected: PASS before installation.

- [ ] **Step 2: Add the official components with Bun**

Run:

```bash
bunx --bun shadcn-svelte@1.4.2 add field textarea scroll-area skeleton toggle-group
```

Expected: the five component directories exist and imports use the aliases from `components.json`.

- [ ] **Step 3: Verify generated imports compile**

Run:

```bash
bun run check
```

Expected: PASS before custom Conversation UI code is added.

## Task 2: Add Frontend Conversation Contracts and IPC Bridge

**Files:**
- Create: `src/lib/services/conversationService.ts`
- Create: `src/lib/services/__tests__/conversationService.spec.ts`

- [ ] **Step 1: Write failing IPC contract tests**

Create `conversationService.spec.ts` with a mocked `invoke` and `listen`. Cover these exact calls:

```ts
expect(invokedCommands).toContainEqual({
  command: 'get_contact_timeline',
  args: {
    leadId: 7,
    activeChannels: ['zalo', 'email'],
    before: { occurredAt: '2026-07-31T01:00:00.000Z', messageId: 12 },
    limit: 50
  }
});

expect(invokedCommands).toContainEqual({
  command: 'send_conversation_message',
  args: {
    request: {
      conversationId: 31,
      channelAccountId: 4,
      text: 'Xin chào',
      replyToMessageId: null
    }
  }
});
```

Also assert:

1. Two `initConversationEventBridge()` consumers create one Tauri listener.
2. Both `onConversationInvalidated()` subscribers receive the same event.
3. Removing the final bridge consumer invokes the Tauri unlisten callback once.
4. A failed initial `listen` schedules one bounded retry and final cleanup cancels the retry.

- [ ] **Step 2: Run the service test and verify failure**

Run:

```bash
bun test src/lib/services/__tests__/conversationService.spec.ts
```

Expected: FAIL because `conversationService.ts` does not exist.

- [ ] **Step 3: Implement exact frontend types**

Create `conversationService.ts` with these exported contracts:

```ts
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type Channel = 'zalo' | 'whatsapp' | 'messenger' | 'email';
export type ConversationKind = 'direct' | 'group' | 'email_thread';
export type InboxItemKind = 'customer' | 'unknown' | 'group';
export type MessageDirection = 'inbound' | 'outbound' | 'system';
export type MessageStatus =
  | 'queued'
  | 'sending'
  | 'sent'
  | 'delivered'
  | 'read'
  | 'received'
  | 'failed'
  | 'removed';
export type AccountConnectionStatus =
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'degraded'
  | 'invalid';

export interface TimelineCursor {
  occurredAt: string;
  messageId: number;
}

export interface ChannelBadgeSummary {
  channel: Channel;
  messageCount: number;
}

export interface ConversationReplyTarget {
  conversationId: number;
  channelAccountId: number;
  channel: Channel;
  accountDisplayName: string | null;
  conversationKind: ConversationKind;
  lastInboundAt: string | null;
  lastMessageAt: string | null;
}

export interface ConversationParticipantSummary {
  identityId: number;
  displayName: string | null;
  avatarUrl: string | null;
  participantRole: string;
}

export interface InboxItem {
  key: string;
  kind: InboxItemKind;
  leadId: number | null;
  conversationId: number | null;
  identityId: number | null;
  title: string;
  avatarUrl: string | null;
  lastMessagePreview: string | null;
  lastMessageAt: string | null;
  channels: ChannelBadgeSummary[];
  replyTargets: ConversationReplyTarget[];
  participantCount: number;
  participants: ConversationParticipantSummary[];
}

export interface ConversationMessage {
  id: number;
  conversationId: number;
  channelAccountId: number;
  channel: Channel;
  accountDisplayName: string | null;
  senderIdentityId: number | null;
  senderDisplayName: string | null;
  direction: MessageDirection;
  providerMessageId: string | null;
  contentType: string;
  textContent: string | null;
  content: Record<string, unknown>;
  occurredAt: string;
  status: MessageStatus;
  removedAt: string | null;
  errorMessage: string | null;
  replyToMessageId: number | null;
}

export interface TimelinePage {
  items: ConversationMessage[];
  nextCursor: TimelineCursor | null;
}

export interface ConversationAccountRuntime {
  channelAccountId: number;
  channel: Channel;
  providerAccountKey: string;
  displayName: string | null;
  status: AccountConnectionStatus;
  lastConnectedAt: string | null;
  lastError: string | null;
}

export interface ConversationRuntimeStatus {
  accounts: ConversationAccountRuntime[];
}

export interface SendConversationMessageRequest {
  conversationId: number;
  channelAccountId: number;
  text: string;
  replyToMessageId: number | null;
}

export interface ConversationInvalidatedEvent {
  conversationId: number;
  leadId: number | null;
  channel: Channel;
  reason: 'message' | 'receipt' | 'reaction' | 'metadata' | 'link';
  messageId?: number;
}
```

- [ ] **Step 4: Implement the command wrappers**

Use the exact public names from the master plan:

```ts
export const listConversationInbox = () =>
  invoke<InboxItem[]>('list_conversation_inbox');

export const getContactTimeline = (
  leadId: number,
  activeChannels: Channel[],
  before: TimelineCursor | null,
  limit = 50
) => invoke<TimelinePage>('get_contact_timeline', { leadId, activeChannels, before, limit });

export const getConversationTimeline = (
  conversationId: number,
  before: TimelineCursor | null,
  limit = 50
) => invoke<TimelinePage>('get_conversation_timeline', { conversationId, before, limit });

export const linkConversationIdentity = (identityId: number, leadId: number) =>
  invoke<void>('link_conversation_identity', { request: { identityId, leadId } });

export const sendConversationMessage = (request: SendConversationMessageRequest) =>
  invoke<ConversationMessage>('send_conversation_message', { request });

export const getConversationRuntimeStatus = () =>
  invoke<ConversationRuntimeStatus>('get_conversation_runtime_status');

export const restartConversationAccount = (providerAccountKey: string) =>
  invoke<void>('restart_conversation_account', { providerAccountKey });
```

- [ ] **Step 5: Implement the ref-counted invalidation bridge**

Use one module-level listener, a `Set` of event subscribers, retry delays from 100 ms to 5 seconds, and these exports:

```ts
type InvalidationListener = (event: ConversationInvalidatedEvent) => void;

export function onConversationInvalidated(listener: InvalidationListener): () => void;
export function initConversationEventBridge(): () => void;
export function resetConversationEventBridgeState(): void;
```

`initConversationEventBridge` increments a reference count and lazily calls:

```ts
listen<ConversationInvalidatedEvent>('conversation-invalidated', (event) => {
  for (const subscriber of invalidationListeners) subscriber(event.payload);
});
```

The final cleanup cancels pending retries and invokes the stored Tauri unlisten function. Keep `resetConversationEventBridgeState` exported only for deterministic tests.

- [ ] **Step 6: Run service tests**

Run:

```bash
bun test src/lib/services/__tests__/conversationService.spec.ts
```

Expected: PASS.

## Task 3: Add Deterministic Timeline State Helpers

**Files:**
- Create: `src/lib/components/conversation/timelineState.ts`
- Create: `src/lib/components/conversation/__tests__/timelineState.spec.ts`

- [ ] **Step 1: Write failing merge, filter, and cache tests**

Tests must assert:

```ts
expect(channelSetKey(['email', 'zalo'])).toBe('email|zalo');
expect(channelSetKey(['zalo', 'email'])).toBe('email|zalo');

expect(mergeMessages([message(2)], [message(1), message(2)]).map((item) => item.id))
  .toEqual([1, 2]);

expect(filterMessages([zaloMessage, emailMessage], ['zalo']).map((item) => item.id))
  .toEqual([zaloMessage.id]);
```

Also cover:

- chronological ordering by `occurredAt`, then `id`;
- deduplication by local message `id`;
- maximum retained cache of 200 messages per channel-set key;
- LRU eviction after five distinct non-empty channel-set keys;
- all channels inactive returns an empty list without a fetch request;
- an anchor message hidden by a badge toggle chooses the nearest surviving chronological neighbor.

- [ ] **Step 2: Run the helper tests and verify failure**

```bash
bun test src/lib/components/conversation/__tests__/timelineState.spec.ts
```

Expected: FAIL because the helpers do not exist.

- [ ] **Step 3: Implement timeline helpers**

Use these contracts:

```ts
import type {
  Channel,
  ConversationMessage,
  TimelineCursor,
  TimelinePage
} from '$lib/services/conversationService';

export const TIMELINE_PAGE_SIZE = 50;
export const MAX_CACHED_MESSAGES = 200;
export const MAX_CHANNEL_VARIANTS = 4;

export interface TimelineVariant {
  items: ConversationMessage[];
  nextCursor: TimelineCursor | null;
  hasLoaded: boolean;
  loading: boolean;
}

export function channelSetKey(channels: Channel[]): string {
  return [...channels].sort().join('|');
}

export function mergeMessages(
  current: ConversationMessage[],
  incoming: ConversationMessage[]
): ConversationMessage[];

export function filterMessages(
  items: ConversationMessage[],
  channels: Channel[]
): ConversationMessage[];

export function mergeTimelinePage(
  current: TimelineVariant,
  page: TimelinePage,
  mode: 'refresh' | 'older'
): TimelineVariant;

export function nearestSurvivingAnchor(
  previousItems: ConversationMessage[],
  nextItems: ConversationMessage[],
  anchorMessageId: number | null
): number | null;
```

`mergeMessages` sorts ascending. `mergeTimelinePage` keeps the newest 200 when `mode = 'refresh'` and the oldest 200 when `mode = 'older'`, so recent refreshes and older pagination do not discard the side the user is viewing.

- [ ] **Step 4: Run timeline helper tests**

```bash
bun test src/lib/components/conversation/__tests__/timelineState.spec.ts
```

Expected: PASS.

## Task 4: Build Inbox Sidebar and Channel Badges

**Files:**
- Create: `src/lib/components/conversation/ConversationSidebar.svelte`
- Create: `src/lib/components/conversation/ChannelBadges.svelte`
- Create: `src/lib/components/conversation/__tests__/ConversationSidebar.spec.ts`
- Create: `src/lib/components/conversation/__tests__/ChannelBadges.spec.ts`

- [ ] **Step 1: Write failing sidebar tests**

Render customer, unknown, and group fixtures. Assert:

1. Search filters by title and last-message preview.
2. Customer/unknown rows render under `Khách hàng`; group rows render under `Nhóm`.
3. Clicking a row invokes `onSelect(item)` once.
4. Available channel labels and message counts render.
5. Group rows render member count and participant preview names.
6. The selected row exposes `aria-current="true"`.

- [ ] **Step 2: Write failing badge tests**

Assert all available badges start active, `aria-pressed` reflects state, click toggles only that channel, and an empty active set displays `Không có kênh nào đang được hiển thị`.

- [ ] **Step 3: Implement `ConversationSidebar.svelte`**

Use callback props:

```ts
let {
  items,
  selectedKey,
  loading = false,
  onSelect
}: {
  items: InboxItem[];
  selectedKey: string | null;
  loading?: boolean;
  onSelect: (item: InboxItem) => void;
} = $props();
```

Use `Input` for search, `ScrollArea` for rows, `Skeleton` for the initial state, and initials when `avatarUrl` is missing. Do not merge group rows into customer rows. Keep row keys from `InboxItem.key`.

- [ ] **Step 4: Implement `ChannelBadges.svelte`**

Use `ToggleGroup.Root type="multiple"` and one `ToggleGroup.Item` per available channel. Bind the selected value to a local `Channel[]`, forward changes through `onToggle`, and include a channel icon plus message count. Keep `aria-label` on every item. Callback props remain:

```ts
let {
  channels,
  activeChannels,
  onToggle
}: {
  channels: ChannelBadgeSummary[];
  activeChannels: Channel[];
  onToggle: (channel: Channel) => void;
} = $props();
```

Only render channels present in `channels`. Do not render placeholder WhatsApp/Messenger/Email badges before history or a linked provider target exists.

- [ ] **Step 5: Run sidebar and badge tests**

```bash
bun test \
  src/lib/components/conversation/__tests__/ConversationSidebar.spec.ts \
  src/lib/components/conversation/__tests__/ChannelBadges.spec.ts
```

Expected: PASS.

## Task 5: Build Safe Message Rendering and Bounded Timeline

**Files:**
- Create: `src/lib/components/conversation/MessageBubble.svelte`
- Create: `src/lib/components/conversation/ConversationTimeline.svelte`
- Create: `src/lib/components/conversation/__tests__/MessageBubble.spec.ts`
- Create: `src/lib/components/conversation/__tests__/ConversationTimeline.spec.ts`

- [ ] **Step 1: Write failing message rendering tests**

Cover exact states:

- inbound text shows sender, channel, account, and Vietnamese-localized timestamp;
- outbound `queued`, `sending`, `sent`, `delivered`, `read`, and `failed` labels;
- `removed` renders a tombstone and hides original text;
- `contentType = 'email'` renders a card using `content.subject`, `content.from`, and `content.to` without `{@html}`;
- unsupported content renders `Nội dung chưa được hỗ trợ` and never injects HTML from `content`;
- group sender name is visible while direct outbound messages do not invent a customer sender.

- [ ] **Step 2: Write failing timeline behavior tests**

Assert:

1. Items render in ascending chronological order.
2. The top sentinel calls `onLoadOlder()` only when `hasMore && !loadingOlder`.
3. At most 160 bubbles are mounted at once.
4. New live messages auto-scroll only when the viewport was already within 80 px of the bottom.
5. Capturing and restoring `{ messageId, offsetTop }` preserves the visible anchor after prepend/filter changes.

- [ ] **Step 3: Implement `MessageBubble.svelte`**

Use `Intl.DateTimeFormat('vi-VN', { dateStyle: 'short', timeStyle: 'short' })`. Render text with normal Svelte escaping. For media, render only validated `http:`, `https:`, `data:image/png`, `data:image/jpeg`, `data:image/webp`, `data:image/gif`, or Tauri asset URLs; unknown schemes receive a safe attachment card. Never use `{@html}` for provider content.

- [ ] **Step 4: Implement `ConversationTimeline.svelte`**

Use these props:

```ts
let {
  items,
  loading = false,
  loadingOlder = false,
  hasMore = false,
  emptyMessage,
  onLoadOlder
}: {
  items: ConversationMessage[];
  loading?: boolean;
  loadingOlder?: boolean;
  hasMore?: boolean;
  emptyMessage: string;
  onLoadOlder: () => void;
} = $props();
```

Maintain a 160-item DOM window around the visible message indexes. Use `IntersectionObserver` on top and bottom sentinels to shift the window; use message elements with `data-message-id`. Export component methods through `bind:this`:

```ts
export function captureAnchor(): { messageId: number; offsetTop: number } | null;
export async function restoreAnchor(anchor: { messageId: number; offsetTop: number } | null): Promise<void>;
export function isNearBottom(): boolean;
export async function scrollToBottom(): Promise<void>;
```

- [ ] **Step 5: Run message and timeline tests**

```bash
bun test \
  src/lib/components/conversation/__tests__/MessageBubble.spec.ts \
  src/lib/components/conversation/__tests__/ConversationTimeline.spec.ts
```

Expected: PASS.

## Task 6: Build Account-Aware Composer and Unknown Contact Actions

**Files:**
- Create: `src/lib/components/conversation/MessageComposer.svelte`
- Create: `src/lib/components/conversation/UnknownContactActions.svelte`
- Create: `src/lib/components/conversation/__tests__/MessageComposer.spec.ts`
- Create: `src/lib/components/conversation/__tests__/UnknownContactActions.spec.ts`

- [ ] **Step 1: Write failing composer tests**

Cover:

- direct customer defaults to the connected reply target with the latest `lastInboundAt`;
- a manually selected account is not replaced when runtime status refreshes;
- group mode exposes one locked Zalo target and no channel switcher;
- disconnected/connecting/degraded/invalid accounts show a visible reason and cannot send;
- Enter sends trimmed text, Shift+Enter inserts a newline;
- the returned queued message clears the textarea and calls `onQueued(message)`;
- send failure preserves the draft and exposes the error.

- [ ] **Step 2: Implement `MessageComposer.svelte`**

Use `Select`, `Textarea`, `Button`, and these props:

```ts
let {
  item,
  runtime,
  disabled = false,
  onQueued
}: {
  item: InboxItem;
  runtime: ConversationRuntimeStatus;
  disabled?: boolean;
  onQueued: (message: ConversationMessage) => void;
} = $props();
```

Build options by joining `item.replyTargets` to runtime accounts by `channelAccountId`. Render all `Select.Item` nodes inside `Select.Group`. Eligible status is `connected`; `degraded` remains visible but disabled until the user restarts the account. Track `selectionTouched` and only recompute the default while false. Call `sendConversationMessage` with the selected target's exact conversation/account IDs.

- [ ] **Step 3: Write failing unknown-contact tests**

Assert:

1. `Liên kết lead` loads CRM leads, filters them locally, calls `linkConversationIdentity`, and emits `onLinked(leadId)`.
2. `Tạo lead` pre-fills provider display name, calls `crmService.addLead`, links the returned ID, and emits `onLinked`.
3. Missing `identityId` disables both actions.
4. Dialogs include accessible titles and validation messages.

- [ ] **Step 4: Implement `UnknownContactActions.svelte`**

Use `Dialog`, `Field`, `Input`, and `Button`. For minimum lead creation call:

```ts
const leadId = await crmService.addLead(
  name.trim(),
  company.trim() || 'Cá nhân',
  'New',
  0,
  'Self',
  email.trim(),
  phone.trim(),
  `Tạo từ ${item.channels[0]?.channel ?? 'conversation'}`
);
await linkConversationIdentity(item.identityId!, leadId);
```

Do not auto-link by display name. Only link after explicit user confirmation.

- [ ] **Step 5: Run composer and identity tests**

```bash
bun test \
  src/lib/components/conversation/__tests__/MessageComposer.spec.ts \
  src/lib/components/conversation/__tests__/UnknownContactActions.spec.ts
```

Expected: PASS.

## Task 7: Orchestrate the Conversation Workspace

**Files:**
- Create: `src/lib/components/conversation/ConversationWorkspace.svelte`
- Create: `src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts`

- [ ] **Step 1: Write failing workspace tests**

Mock `conversationService` and assert:

1. Mounting while `activeTab !== 'conversation'` does not load inbox/runtime data.
2. First activation loads inbox and runtime exactly once.
3. Selecting a customer activates all available badges and calls `getContactTimeline(..., null, 50)`.
4. Selecting a group calls `getConversationTimeline(conversationId, null, 50)` and does not show channel badges as filters.
5. Toggling a badge filters loaded messages immediately and stores a separate cache under the new `channelSetKey`.
6. All badges inactive makes no timeline IPC call and shows the explicit empty-filter state.
7. Older pagination uses the current variant's `nextCursor` and active channels.
8. An invalidation for the open lead refreshes its current variant; another lead only marks inbox stale.
9. A link invalidation with `conversationId = 0` reloads the full inbox.
10. Inactive workspace invalidations mark data stale and one refresh occurs on the next activation.
11. A queued outbound message appears immediately without waiting for realtime echo; later echo deduplicates by local row ID.

- [ ] **Step 2: Run the workspace test and verify failure**

```bash
bun test src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
```

Expected: FAIL because the workspace does not exist.

- [ ] **Step 3: Implement workspace state**

Use these core runes:

```ts
let { activeTab = 'conversation' }: { activeTab?: string } = $props();
let inbox = $state<InboxItem[]>([]);
let selectedKey = $state<string | null>(null);
let activeChannels = $state<Channel[]>([]);
let runtime = $state<ConversationRuntimeStatus>({ accounts: [] });
let variants = $state(new Map<string, TimelineVariant>());
let inboxStale = $state(true);
let timelineStale = $state(new Set<string>());
```

Selection rules:

- customer/unknown: `activeChannels = item.channels.map(({ channel }) => channel)`;
- group: no filter set; use `item.conversationId`;
- store active badges per direct `item.key` for the current session only;
- select the first inbox row only after inbox data arrives and no previous selection remains.

- [ ] **Step 4: Implement loading and invalidation**

Initialize the bridge in `onMount`, subscribe with `onConversationInvalidated`, and debounce affected refreshes for 50 ms. Refresh the inbox row set after every message/metadata/link event. Refresh the open timeline only when:

```ts
const affectsOpenItem =
  (selected.kind !== 'group' && event.leadId === selected.leadId) ||
  (selected.kind === 'group' && event.conversationId === selected.conversationId);
```

When active, merge the newest page and preserve the timeline anchor. When inactive, add the selected cache key to `timelineStale` and defer IPC until activation.

- [ ] **Step 5: Implement pagination and cache transitions**

For a direct row, key each variant as `${item.key}:${channelSetKey(activeChannels)}`. On a badge toggle:

1. Capture the timeline anchor.
2. Switch immediately to filtered messages from all known variants for that item.
3. If the target variant has never loaded, request its newest page with `before = null`.
4. Merge/dedupe the page and restore the nearest surviving anchor.
5. Retain at most four variants and 200 messages per variant.

This prevents re-enabled channels from being skipped after the user paginated while they were inactive.

- [ ] **Step 6: Compose the workspace layout**

Use a two-column `h-full min-h-0` layout:

```text
ConversationSidebar (320 px)
  + selected header
  + ChannelBadges for direct rows
  + UnknownContactActions when needed
  + ConversationTimeline
  + MessageComposer
```

For a selected group, the header shows `participantCount` and the available participant preview names; message bubbles show the sender for each inbound group message. If no row exists, show `Chưa có hội thoại`. If runtime loading fails, keep history readable and disable sending with a retry action.

- [ ] **Step 7: Run workspace tests**

```bash
bun test src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
```

Expected: PASS.

## Task 8: Integrate Conversation Navigation and Full-Height Layout

**Files:**
- Modify: `src/lib/components/Sidebar.svelte`
- Modify: `src/lib/components/__tests__/Sidebar.spec.ts`
- Modify: `src/routes/+page.svelte`

- [ ] **Step 1: Extend the existing Sidebar test**

Add assertions:

```ts
expect(screen.getByText('Conversations')).toBeTruthy();
await fireEvent.click(screen.getByText('Conversations'));
expect(activeTab).toBe('conversation');
```

- [ ] **Step 2: Run Sidebar tests and verify failure**

```bash
bun test src/lib/components/__tests__/Sidebar.spec.ts
```

Expected: FAIL because the navigation item is absent.

- [ ] **Step 3: Add the navigation item**

Import `MessagesSquare` and add before Outreach:

```ts
{ id: 'conversation', label: 'Conversations', icon: MessagesSquare },
```

- [ ] **Step 4: Integrate the workspace into `+page.svelte`**

Import `ConversationWorkspace`, add:

```ts
conversation: {
  title: 'Unified Conversations',
  subtitle: 'Zalo, WhatsApp, Messenger and Email customer history'
},
```

Render:

```svelte
<div class={activeTab === 'conversation' ? 'h-full' : 'hidden'}>
  <ConversationWorkspace {activeTab} />
</div>
```

Change the main content wrapper so Conversation receives the full remaining height while existing tabs keep padded scrolling:

```svelte
<div class={activeTab === 'conversation'
  ? 'flex-1 min-h-0 overflow-hidden'
  : 'flex-1 overflow-y-auto p-6 flex flex-col gap-6'}>
```

Do not unmount existing tabs; their current active-tab caching behavior remains intact.

- [ ] **Step 5: Run navigation and workspace tests**

```bash
bun test \
  src/lib/components/__tests__/Sidebar.spec.ts \
  src/lib/components/conversation/__tests__
```

Expected: PASS.

## Task 9: UI Verification

**Files:**
- No new files

- [ ] **Step 1: Run all frontend tests**

```bash
bun test
```

Expected: PASS.

- [ ] **Step 2: Run Svelte and production build checks**

```bash
bun run check
bun run build
```

Expected: PASS with no accessibility errors for dialogs, buttons, selects, or the timeline.

- [ ] **Step 3: Run manual Tauri smoke checks**

Run:

```bash
bun run tauri dev
```

Verify:

1. Conversation opens full-height without nested page scrolling.
2. Direct customer badges start active and hide/show messages immediately.
3. Re-enabled badges do not skip older history.
4. Scroll anchor remains stable after filtering and loading older pages.
5. Groups stay separate and the composer is locked to the group account.
6. Unknown identity linking moves the row into the lead timeline without duplicated messages.
7. Outbound queued/sent/delivered/read/failed states update in place.
8. Switching tabs does not cause an IPC request loop.

- [ ] **Step 4: Run final scope checks**

```bash
git diff --check
```

Then run:

```text
mcp__gitnexus.detect_changes({repo: "RustSale_v2", scope: "all"})
```

Expected: only the Conversation frontend modules, generated shadcn primitives, Sidebar navigation, and page integration are affected by this child plan.
