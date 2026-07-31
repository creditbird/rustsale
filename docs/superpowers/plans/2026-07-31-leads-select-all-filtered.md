# Select All Filtered Leads Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow users in the Leads Management view (`LeadsList.svelte`) to select all filtered leads across all pagination pages with one click and assign them to a category.

**Architecture:** Extend selection state and derived computations in `LeadsList.svelte` with `selectAllFiltered()` and `clearSelection()` helper functions. Render a selection notification banner directly above the leads table whenever page items are selected, offering a single-click action to select all matching leads across all pages.

**Tech Stack:** Svelte 5 (Runes), TailwindCSS, Lucide-Svelte icons, Bun, Tauri (Rust).

## Global Constraints
- Target File: `src/lib/components/LeadsList.svelte`
- Package Manager: Always use `bun` and `bunx` (never `npm` or `npx`).
- Run `bun run check` to verify zero `svelte-check` diagnostics.

---

### Task 1: Add Select All Filtered State Helpers, UI Notification Banner, and Auto-Reset in `LeadsList.svelte`

**Files:**
- Modify: `src/lib/components/LeadsList.svelte:100-380`

**Interfaces:**
- Consumes: `filteredLeads` (derived array of `Lead`), `paginatedLeads` (derived page array of `Lead`), `selectedRows` (state `Set<number>`).
- Produces: `isAllFilteredSelected` (derived boolean), `isCurrentPageAllSelected` (derived boolean), `selectAllFiltered()` (function), `clearSelection()` (function).

- [ ] **Step 1: Define derived selection states and helper functions in `LeadsList.svelte`**

Add the derived selection states and helper functions after `paginatedLeads` definition:

```svelte
  let isAllFilteredSelected = $derived(
    filteredLeads.length > 0 && selectedRows.size === filteredLeads.length
  );

  let isCurrentPageAllSelected = $derived(
    paginatedLeads.length > 0 && paginatedLeads.every((l: Lead) => l.id !== undefined && selectedRows.has(l.id))
  );

  function selectAllFiltered() {
    filteredLeads.forEach((l: Lead) => {
      if (l.id !== undefined) selectedRows.add(l.id);
    });
    selectedRows = new Set(selectedRows);
  }

  function clearSelection() {
    selectedRows.clear();
    selectedRows = new Set(selectedRows);
  }
```

- [ ] **Step 2: Render Selection Banner above `<Table.Root>`**

Right above `<Table.Root class="border border-border rounded-lg bg-card overflow-hidden">` (line ~294), add the interactive selection banner:

```svelte
  <!-- Selection Banner -->
  {#if selectedRows.size > 0}
    <div class="px-4 py-2 bg-emerald-500/10 border border-emerald-500/20 rounded-lg flex items-center justify-between text-xs text-emerald-700 dark:text-emerald-300 font-medium">
      <div class="flex items-center gap-2">
        <span>
          {#if isAllFilteredSelected}
            All <strong>{filteredLeads.length}</strong> leads matching this search/filter are selected.
          {:else}
            Selected <strong>{selectedRows.size}</strong> lead(s) on this page.
          {/if}
        </span>
        {#if !isAllFilteredSelected && filteredLeads.length > paginatedLeads.length}
          <button 
            type="button" 
            onclick={selectAllFiltered} 
            class="underline font-bold hover:text-emerald-800 dark:hover:text-emerald-100 transition-colors ml-1 cursor-pointer"
          >
            Select all {filteredLeads.length} leads matching search
          </button>
        {/if}
      </div>
      <button 
        type="button" 
        onclick={clearSelection} 
        class="text-xs text-emerald-600 dark:text-emerald-400 hover:underline cursor-pointer"
      >
        Clear selection
      </button>
    </div>
  {/if}
```

- [ ] **Step 3: Run `bun run check` to verify TypeScript & Svelte 5 compilation**

Run: `bun run check`  
Expected: 0 errors / 0 warnings.

- [ ] **Step 4: Commit changes to Git**

```bash
git add src/lib/components/LeadsList.svelte
git commit -m "feat(leads): add select all filtered leads banner and state helpers"
```

---

### Task 2: Quality Gates & End-to-End Verification

**Files:**
- Modify: `src/lib/components/LeadsList.svelte`

- [ ] **Step 1: Run `bun run check` to verify zero type diagnostics**

Run: `bun run check`  
Expected: 0 errors / 0 warnings.

- [ ] **Step 2: Run `git diff --check` to verify no trailing whitespace or formatting issues**

Run: `git diff --check`  
Expected: Exit code 0 with clean output.

- [ ] **Step 3: Run `bun run build` to verify Vite release build passes**

Run: `bun run build`  
Expected: Exit code 0, dist bundle generated cleanly.
