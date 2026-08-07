# Zalo Chat Features & Conversation Enhancements Implementation Plan

> **Goal**: Upgrade RustSale_v2 conversation module with 5 core Zalo Chat features:
> 1. Voice Record directly on `ComposerToolbar.svelte`
> 2. Pinned Messages (DB storage, backend IPC, context menu pin/unpin, and `PinnedMessageBanner.svelte`)
> 3. In-Thread Search ("Ctrl+F" search bar in `ConversationWorkspace.svelte` & `MessageTimeline.svelte`)
> 4. Visual Read/Delivered Receipts in `MessageBubble.svelte` (single check, double gray check, double blue check, failed status)
> 5. Smart Conversation Triage Filters in `ConversationSidebar.svelte` (All, Unread, Priority, Stuck)

---

## 1. Database & Backend Changes (`src-tauri/`)

### A. Pinned Messages Table & Repository
- Add `conversation_pinned_messages` table to `src-tauri/src/conversation/schema.sql`:
  ```sql
  CREATE TABLE IF NOT EXISTS conversation_pinned_messages (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      conversation_id INTEGER NOT NULL,
      message_id INTEGER NOT NULL,
      pinned_by TEXT,
      pinned_at TEXT NOT NULL,
      UNIQUE(conversation_id, message_id),
      FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
      FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
  );
  ```
- Implement `pin_message`, `unpin_message`, `list_pinned_messages` in `src-tauri/src/conversation/repository.rs` and `service.rs`.
- Expose Tauri IPC commands in `src-tauri/src/commands.rs`:
  - `pin_conversation_message`
  - `unpin_conversation_message`
  - `list_conversation_pinned_messages`

---

## 2. Frontend Services (`src/lib/services/`)

### A. `conversationService.ts` Updates
- Export DTOs for `PinnedMessage`:
  ```ts
  export type PinnedMessage = {
    id: number;
    conversationId: number;
    messageId: number;
    pinnedBy: string | null;
    pinnedAt: string;
    messageText: string | null;
    senderName: string | null;
  };
  ```
- Export functions: `pinConversationMessage`, `unpinConversationMessage`, `listConversationPinnedMessages`.
- Export audio blob staging helper: `stageConversationAudioBlob`.

---

## 3. UI Component Implementation (`src/lib/components/conversation/`)

### A. Voice Record (`ComposerToolbar.svelte` & `MessageComposer.svelte`)
- Add `Mic` button to `ComposerToolbar.svelte`.
- Inline recorder state: toggle active recording using `MediaRecorder` API.
- Show timer, visual recording wave/indicator, Cancel & Send buttons.
- On record finish, stage audio blob and add to composer attachments.

### B. Pin Message (`PinnedMessageBanner.svelte` & `MessageActionsMenu.svelte`)
- Add "Ghim tin nhắn" / "Bỏ ghim" action in `MessageActionsMenu.svelte`.
- Create `PinnedMessageBanner.svelte` to display at top of `MessageTimeline.svelte`.
- Banner shows pinned text preview, pin icon, jump-to-message button, and unpin button.

### C. In-Thread Search (`InThreadSearchBar.svelte` & Keyboard Shortcut)
- Listen for `Ctrl+F` / `Cmd+F` in `ConversationWorkspace.svelte`.
- Open `InThreadSearchBar.svelte` floating bar at top right of timeline.
- Highlights match occurrences, provides match count ("1 / 4"), and Next/Prev match navigation.

### D. Message Receipts & Status (`MessageBubble.svelte`)
- Upgrade outbound status indicator:
  - `queued` / `sending`: `Clock` icon (gray)
  - `sent` / `accepted`: `Check` icon `✓` (gray)
  - `delivered`: `CheckCheck` icon `✓✓` (gray)
  - `read`: `CheckCheck` icon `✓✓` (blue/green)
  - `failed`: `AlertCircle` icon (red) with error tooltip

### E. Conversation Triage Filters (`ConversationSidebar.svelte`)
- Add filter chips bar below search input: `Tất cả` | `Chưa đọc` | `Ưu tiên` | `Kẹt (Cần xử lý)`
- Filtering rules:
  - **Tất cả**: Show all conversations
  - **Chưa đọc**: `item.unreadCount > 0` or unread status
  - **Ưu tiên**: Linked lead priority / high message volume
  - **Kẹt**: Inbound message received > 2 hours ago without outbound response

---

## 4. Verification Plan

- **Rust Backend**:
  - Run `bun run test` / `cargo test` for conversation repository and service.
- **Frontend Svelte**:
  - Run `bun run test` / `bun test` for Svelte components.
  - Run `bun run check` (svelte-check) to ensure zero type errors.
