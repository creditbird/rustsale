# Conversation Sidebar Option A Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Option A in `ConversationSidebar.svelte` to support a unified stream of conversations sorted by `lastMessageAt` DESC, along with conversation kind tabs (`Tất cả`, `Khách hàng`, `Nhóm`), ensuring new group messages immediately move to the top of the list.

**Architecture:** Add a new `$state` variable `activeKindTab` ('all' | 'customer' | 'group') in `ConversationSidebar.svelte`. Derive `filteredByKindItems` and unify the main list in `'all'` tab so that customer and group items are merged and sorted by `lastMessageAt` DESC. Render conversation kind selector tabs alongside the triage filter bar. Update unit tests to cover tab switching and unified sorting.

**Tech Stack:** Svelte 5 (Runes `$state`, `$derived`), Tailwind CSS, Bun test, `@testing-library/svelte`.

## Global Constraints
- Use `bun` and `bunx` for all commands.
- Preserve Svelte 5 runes syntax (`$state`, `$derived`, `$props`).
- Ensure 100% test pass rate in Bun test suite.

---

### Task 1: Update ConversationSidebar Component & Logic

**Files:**
- Modify: `src/lib/components/conversation/ConversationSidebar.svelte`
- Test: `src/lib/components/conversation/__tests__/ConversationSidebar.spec.ts`

**Interfaces:**
- Props: `items: InboxItem[]`, `selectedKey: string | null`, `loading?: boolean`, `onSelect: (item: InboxItem) => void`
- State: `activeKindTab: 'all' | 'customer' | 'group'`
- State: `activeFilterTab: 'all' | 'unread' | 'priority' | 'stuck'`

- [ ] **Step 1: Write failing unit test for Option A in `ConversationSidebar.spec.ts`**

Add tests for:
1. Conversation kind tabs rendering (`Tất cả`, `Khách hàng`, `Nhóm` with counts).
2. Unified sorting in `'all'` tab: items sorted strictly by `lastMessageAt` DESC regardless of whether they are `customer` or `group`.
3. Tab filtering: clicking `Khách hàng` shows only customer/unknown items; clicking `Nhóm` shows only group items.

- [ ] **Step 2: Run test to confirm failure**

Run `bun test src/lib/components/conversation/__tests__/ConversationSidebar.spec.ts` to verify new test assertions fail before implementation.

- [ ] **Step 3: Update `ConversationSidebar.svelte` implementation**

1. Introduce `activeKindTab = $state<'all' | 'customer' | 'group'>('all');`
2. Calculate counts: `customerCount`, `groupCount`, `allCount`.
3. Update derived filtering:
   - Apply search filter and triage filter tab first (`filteredItems`).
   - Filter by `activeKindTab`:
     - `'all'`: includes customer, unknown, and group.
     - `'customer'`: includes customer and unknown.
     - `'group'`: includes group.
   - Separate into `pinnedItems` and `unpinnedItems` (both sorted using `sortByLatest`).
4. Render Kind Tabs UI in the header bar above/alongside the triage filter bar.
5. Render `pinnedItems` (if any) and `unpinnedItems` (in unified list when in `'all'` tab, or with appropriate section header in filtered tabs).

- [ ] **Step 4: Run tests to verify pass**

Run `bun test src/lib/components/conversation/__tests__/ConversationSidebar.spec.ts` and verify all tests pass.

- [ ] **Step 5: Run full frontend test suite & typecheck**

Run `bun check` / `bun test` to ensure no regressions across the codebase.

- [ ] **Step 6: Commit changes**

Commit changes using GitNexus pre-commit change detection.
