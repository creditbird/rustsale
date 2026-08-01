# Conversation Lead Linking Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add "Đổi lead" (Change Lead) and "Hủy liên kết" (Unlink Lead) capabilities to the conversation header when a conversation is already linked to a lead.

**Architecture:** Extend backend Rust (`repository.rs`, `service.rs`, `commands.rs`) with `unlink_identity`, expose Tauri IPC `unlink_conversation_identity`, create `LinkedLeadActions.svelte` using shadcn-svelte components, and integrate into `ConversationWorkspace.svelte`.

**Tech Stack:** Rust (Tauri 2 IPC, SQLite), Svelte 5 (Runes `$state`/`$derived`), Tailwind CSS, Lucide Icons, shadcn-svelte (`Dialog`, `Button`, `Input`).

## Global Constraints

- Package manager: **bun** and **bunx** exclusively.
- GitNexus: run `bunx gitnexus analyze` if index is stale; run impact analysis before editing existing symbols.
- Semantic styling: Use `bg-background`, `text-muted-foreground`, `size-*` for equal dimensions, `truncate` shorthand, `data-icon` for button icons.

---

### Task 1: Backend Rust `unlink_identity` Repository, Service & Tauri Command

**Files:**
- Modify: `src-tauri/src/conversation/repository.rs`
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/conversation/repository_test.rs`
- Test: `src-tauri/src/conversation/commands_test.rs`

**Interfaces:**
- Produces: Tauri IPC command `unlink_conversation_identity(request: { identityId: i64 }) -> Result<(), String>`

- [ ] **Step 1: Write failing Rust unit test for `unlink_identity` in `repository_test.rs`**

Add test in `src-tauri/src/conversation/repository_test.rs`:
```rust
#[test]
fn unlink_identity_resets_lead_id_and_returns_conversation_id() {
    let repo = setup_test_repository();
    let applied = repo
        .apply_inbound_message(&sample_inbound_message("zalo-1", "user-1", "Hello"))
        .unwrap();
    let identity_id = applied.identity_id.expect("identity exists");
    repo.link_identity(identity_id, 42).unwrap();

    let conv_id = repo.unlink_identity(identity_id).unwrap();
    assert!(conv_id > 0);

    let inbox = repo.list_inbox().unwrap();
    let item = inbox.iter().find(|i| i.conversation_id == Some(conv_id)).unwrap();
    assert_eq!(item.kind, InboxItemKind::Unknown);
    assert_eq!(item.lead_id, None);
}
```

- [ ] **Step 2: Run Rust test to verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml unlink_identity_resets_lead_id_and_returns_conversation_id`
Expected: FAIL with "no method named `unlink_identity` found"

- [ ] **Step 3: Implement `unlink_identity` in `repository.rs`**

In `src-tauri/src/conversation/repository.rs`:
```rust
pub fn unlink_identity(&self, identity_id: i64) -> Result<i64> {
    let mut conn = open_db_connection(&self.db_path)?;
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE channel_identities
         SET lead_id = NULL, link_source = 'unlinked', link_confidence = 0.0, verified_at = NULL, updated_at = ?1
         WHERE id = ?2",
        params![utc_now(), identity_id],
    )?;
    let conversation_id = tx
        .query_row(
            "SELECT cp.conversation_id
             FROM conversation_participants cp
             JOIN channel_conversations cc ON cc.id = cp.conversation_id
             WHERE cp.identity_id = ?1 AND cc.kind IN ('direct', 'email_thread')
             ORDER BY cc.last_message_at DESC, cc.id DESC
             LIMIT 1",
            [identity_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    tx.commit()?;
    Ok(conversation_id)
}
```

- [ ] **Step 4: Implement service method & Tauri command**

In `src-tauri/src/conversation/service.rs`:
```rust
pub fn unlink_identity(&self, identity_id: i64) -> Result<()> {
    let conversation_id = self.repository.unlink_identity(identity_id)?;
    if conversation_id > 0 {
        self.events
            .publish(ConversationInvalidatedEvent { conversation_id, lead_id: None });
    }
    Ok(())
}
```

In `src-tauri/src/conversation/commands.rs`:
```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlinkIdentityRequest {
    pub identity_id: i64,
}

#[tauri::command]
pub fn unlink_conversation_identity(
    state: tauri::State<'_, ConversationState>,
    request: UnlinkIdentityRequest,
) -> Result<(), String> {
    state
        .service
        .unlink_identity(request.identity_id)
        .map_err(|error| error.to_string())
}
```

Register `conversation::commands::unlink_conversation_identity` in `src-tauri/src/lib.rs` invoke_handler macro.

- [ ] **Step 5: Run tests to verify pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS

---

### Task 2: Frontend Service API Wrapper

**Files:**
- Modify: `src/lib/services/conversationService.ts`
- Test: `src/lib/services/__tests__/conversationService.spec.ts`

**Interfaces:**
- Consumes: Tauri command `unlink_conversation_identity`
- Produces: `unlinkConversationIdentity(identityId: number): Promise<void>`

- [ ] **Step 1: Write test for `unlinkConversationIdentity`**

In `src/lib/services/__tests__/conversationService.spec.ts`:
```typescript
test('unlinkConversationIdentity calls unlink_conversation_identity with request wrapper', async () => {
  await unlinkConversationIdentity(10);
  expect(invoke).toHaveBeenCalledWith('unlink_conversation_identity', {
    request: { identityId: 10 }
  });
});
```

- [ ] **Step 2: Run test to verify failure**

Run: `bun test src/lib/services/__tests__/conversationService.spec.ts`
Expected: FAIL (function undefined)

- [ ] **Step 3: Implement wrapper in `conversationService.ts`**

In `src/lib/services/conversationService.ts`:
```typescript
export const unlinkConversationIdentity = (identityId: number) =>
  invoke<void>('unlink_conversation_identity', { request: { identityId } });
```

- [ ] **Step 4: Run test to verify pass**

Run: `bun test src/lib/services/__tests__/conversationService.spec.ts`
Expected: PASS

---

### Task 3: `LinkedLeadActions.svelte` UI Component

**Files:**
- Create: `src/lib/components/conversation/LinkedLeadActions.svelte`
- Create: `src/lib/components/conversation/__tests__/LinkedLeadActions.spec.ts`

**Interfaces:**
- Props: `{ item: InboxItem, onRelinked?: (leadId: number) => void, onUnlinked?: () => void }`
- Consumes: `linkConversationIdentity`, `unlinkConversationIdentity`, `crmService.getLeads`

- [ ] **Step 1: Write component unit test `LinkedLeadActions.spec.ts`**

Create `src/lib/components/conversation/__tests__/LinkedLeadActions.spec.ts`:
```typescript
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, test, expect, vi } from 'vitest';
import LinkedLeadActions from '../LinkedLeadActions.svelte';

vi.mock('$lib/services/conversationService', () => ({
  linkConversationIdentity: vi.fn(async () => {}),
  unlinkConversationIdentity: vi.fn(async () => {})
}));

vi.mock('$lib/services/crmService', () => ({
  getLeads: vi.fn(async () => [
    { id: 1, name: 'Lead A', company: 'Comp A' },
    { id: 2, name: 'Lead B', company: 'Comp B' }
  ])
}));

describe('LinkedLeadActions Component', () => {
  const mockItem = {
    key: 'lead:1:direct:1',
    kind: 'lead' as const,
    leadId: 1,
    conversationId: 10,
    identityId: 100,
    title: 'Lead A',
    avatarUrl: null,
    lastMessagePreview: 'hi',
    lastMessageAt: '2026-08-01T00:00:00Z',
    channels: [],
    replyTargets: [],
    participantCount: 1,
    participants: []
  };

  test('renders Đổi lead and Hủy liên kết buttons', () => {
    render(LinkedLeadActions, { item: mockItem });
    expect(screen.getByRole('button', { name: /Đổi lead/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /Hủy liên kết/i })).toBeDefined();
  });
});
```

- [ ] **Step 2: Run test to verify failure**

Run: `bun test src/lib/components/conversation/__tests__/LinkedLeadActions.spec.ts`
Expected: FAIL (component missing)

- [ ] **Step 3: Implement `LinkedLeadActions.svelte`**

Create `src/lib/components/conversation/LinkedLeadActions.svelte`:
```svelte
<script lang="ts">
  import { linkConversationIdentity, unlinkConversationIdentity, type InboxItem } from '$lib/services/conversationService';
  import * as crmService from '$lib/services/crmService';
  import type { Lead } from '$lib/services/crmService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import * as Dialog from '$lib/components/ui/dialog';
  import { RefreshCw, Unlink, Search, AlertCircle } from 'lucide-svelte';

  let {
    item,
    onRelinked,
    onUnlinked
  }: {
    item: InboxItem;
    onRelinked?: (leadId: number) => void;
    onUnlinked?: () => void;
  } = $props();

  let isRelinkModalOpen = $state(false);
  let isUnlinkModalOpen = $state(false);

  // Relink state
  let leads = $state<Lead[]>([]);
  let isLoadingLeads = $state(false);
  let leadSearchText = $state('');
  let selectedLeadId = $state<number | null>(null);
  let isRelinking = $state(false);
  let relinkError = $state<string | null>(null);

  // Unlink state
  let isUnlinking = $state(false);
  let unlinkError = $state<string | null>(null);

  let filteredLeads = $derived.by(() => {
    const q = leadSearchText.trim().toLowerCase();
    if (!q) return leads;
    return leads.filter(
      (l) =>
        l.name.toLowerCase().includes(q) ||
        l.company.toLowerCase().includes(q) ||
        (l.email && l.email.toLowerCase().includes(q)) ||
        (l.phone && l.phone.includes(q))
    );
  });

  async function openRelinkModal() {
    relinkError = null;
    selectedLeadId = null;
    leadSearchText = '';
    isRelinkModalOpen = true;
    isLoadingLeads = true;
    try {
      leads = await crmService.getLeads();
    } catch (err) {
      relinkError = err instanceof Error ? err.message : String(err);
    } finally {
      isLoadingLeads = false;
    }
  }

  async function handleConfirmRelink() {
    if (!selectedLeadId || !item.identityId) return;
    isRelinking = true;
    relinkError = null;
    try {
      await linkConversationIdentity(item.identityId, selectedLeadId);
      isRelinkModalOpen = false;
      onRelinked?.(selectedLeadId);
    } catch (err) {
      relinkError = err instanceof Error ? err.message : String(err);
    } finally {
      isRelinking = false;
    }
  }

  async function handleConfirmUnlink() {
    if (!item.identityId) return;
    isUnlinking = true;
    unlinkError = null;
    try {
      await unlinkConversationIdentity(item.identityId);
      isUnlinkModalOpen = false;
      onUnlinked?.();
    } catch (err) {
      unlinkError = err instanceof Error ? err.message : String(err);
    } finally {
      isUnlinking = false;
    }
  }
</script>

<div class="flex items-center gap-2">
  <Button
    type="button"
    variant="outline"
    size="sm"
    disabled={!item.identityId}
    onclick={openRelinkModal}
    class="flex items-center gap-1.5 text-xs"
  >
    <RefreshCw class="size-3.5" data-icon="inline-start" />
    <span>Đổi lead</span>
  </Button>

  <Button
    type="button"
    variant="destructive"
    size="sm"
    disabled={!item.identityId}
    onclick={() => (isUnlinkModalOpen = true)}
    class="flex items-center gap-1.5 text-xs"
  >
    <Unlink class="size-3.5" data-icon="inline-start" />
    <span>Hủy liên kết</span>
  </Button>
</div>

<!-- Modal 1: Đổi lead -->
<Dialog.Root bind:open={isRelinkModalOpen}>
  <Dialog.Content class="sm:max-w-[480px]">
    <Dialog.Header>
      <Dialog.Title>Đổi liên kết lead</Dialog.Title>
      <Dialog.Description>
        Chọn lead mới trong CRM để thay thế cho cuộc trò chuyện này.
      </Dialog.Description>
    </Dialog.Header>

    <div class="flex flex-col gap-3 py-3">
      {#if relinkError}
        <div class="text-xs text-destructive bg-destructive/10 p-2.5 rounded-md flex items-center gap-1.5" role="alert">
          <AlertCircle class="size-4 shrink-0" />
          <span>{relinkError}</span>
        </div>
      {/if}

      <div class="relative">
        <Search class="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
        <Input
          bind:value={leadSearchText}
          placeholder="Tìm theo tên, email, sđt..."
          class="pl-8 text-sm"
        />
      </div>

      <div class="max-h-[220px] overflow-y-auto border rounded-md p-1 space-y-1">
        {#if isLoadingLeads}
          <div class="text-xs text-center text-muted-foreground py-4">Đang tải danh sách lead...</div>
        {:else if filteredLeads.length === 0}
          <div class="text-xs text-center text-muted-foreground py-4">Không tìm thấy lead phù hợp.</div>
        {:else}
          {#each filteredLeads as lead (lead.id)}
            <button
              type="button"
              onclick={() => (selectedLeadId = lead.id ?? null)}
              class="w-full text-left px-3 py-2 rounded-md text-sm transition-colors flex items-center justify-between {selectedLeadId === lead.id ? 'bg-accent text-accent-foreground font-medium' : 'hover:bg-muted/50'}"
            >
              <div>
                <div class="font-medium">{lead.name}</div>
                <div class="text-xs text-muted-foreground">{lead.company || 'Cá nhân'} {lead.phone ? `• ${lead.phone}` : ''}</div>
              </div>
              {#if selectedLeadId === lead.id}
                <span class="text-xs font-semibold text-primary">Đã chọn</span>
              {/if}
            </button>
          {/each}
        {/if}
      </div>
    </div>

    <Dialog.Footer>
      <Button variant="outline" onclick={() => (isRelinkModalOpen = false)}>Hủy</Button>
      <Button
        disabled={!selectedLeadId || isRelinking}
        onclick={handleConfirmRelink}
      >
        {isRelinking ? 'Đang cập nhật...' : 'Xác nhận đổi'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Modal 2: Xác nhận Hủy liên kết -->
<Dialog.Root bind:open={isUnlinkModalOpen}>
  <Dialog.Content class="sm:max-w-[420px]">
    <Dialog.Header>
      <Dialog.Title>Xác nhận hủy liên kết</Dialog.Title>
      <Dialog.Description>
        Bạn có chắc chắn muốn hủy liên kết giữa cuộc trò chuyện này và lead <strong>{item.title}</strong>? Cuộc trò chuyện sẽ quay về danh mục Chưa liên kết (Unknown).
      </Dialog.Description>
    </Dialog.Header>

    {#if unlinkError}
      <div class="text-xs text-destructive bg-destructive/10 p-2.5 rounded-md flex items-center gap-1.5 my-2" role="alert">
        <AlertCircle class="size-4 shrink-0" />
        <span>{unlinkError}</span>
      </div>
    {/if}

    <Dialog.Footer class="mt-4">
      <Button variant="outline" onclick={() => (isUnlinkModalOpen = false)}>Bỏ qua</Button>
      <Button
        variant="destructive"
        disabled={isUnlinking}
        onclick={handleConfirmUnlink}
      >
        {isUnlinking ? 'Đang hủy...' : 'Hủy liên kết'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
```

- [ ] **Step 4: Run component test to verify pass**

Run: `bun test src/lib/components/conversation/__tests__/LinkedLeadActions.spec.ts`
Expected: PASS

---

### Task 4: Integration in `ConversationWorkspace.svelte` & Suite Verification

**Files:**
- Modify: `src/lib/components/conversation/ConversationWorkspace.svelte`
- Test: `src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts`

- [ ] **Step 1: Import `LinkedLeadActions` and add `handleUnlinked` in `ConversationWorkspace.svelte`**

In `src/lib/components/conversation/ConversationWorkspace.svelte`:
1. Import `LinkedLeadActions`:
```typescript
import LinkedLeadActions from './LinkedLeadActions.svelte';
```
2. Add `handleUnlinked()` handler:
```typescript
function handleUnlinked() {
  refreshInbox();
  refreshOpenTimeline();
}
```
3. Update header render:
```svelte
{#if selectedItem.kind === 'unknown'}
  <UnknownContactActions item={selectedItem} onLinked={handleLinked} />
{:else if selectedItem.kind === 'lead'}
  <LinkedLeadActions item={selectedItem} onRelinked={handleLinked} onUnlinked={handleUnlinked} />
{/if}
```

- [ ] **Step 2: Run full test suite & TypeScript check**

Run: `bun test`
Run: `bun run build`
Expected: All tests pass, build completes with zero errors.
