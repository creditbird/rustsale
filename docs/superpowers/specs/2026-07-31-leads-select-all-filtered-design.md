# Select All Filtered Leads Design Specification

**Date**: 2026-07-31  
**Status**: Approved  
**Target File**: `src/lib/components/LeadsList.svelte`

## 1. Objective
Currently, when a user searches or filters leads in `LeadsList.svelte`, clicking the top table header checkbox only selects the 10 leads rendered on the current pagination page (`paginatedLeads`). Users need a fast and clear way to select **all** matching leads (e.g. 186 entries across 19 pages) in a single click to perform bulk operations, specifically **Assign to Category**.

## 2. User Experience & Design Mechanics
1. **Header Checkbox Interaction**:
   - Ticking the header checkbox selects all items on the current page (`paginatedLeads`).
   - If not all filtered leads are selected, a notification banner appears above the table.
2. **Selection Banner**:
   - When all items on the current page are selected (and `filteredLeads.length > paginatedLeads.length`), display:
     > ℹ️ *All **10** leads on this page are selected.* **[Select all {filteredLeads.length} leads matching search]**
   - When the user clicks **[Select all {filteredLeads.length} leads matching search]**:
     - `selectedRows` is populated with all IDs in `filteredLeads`.
     - The banner text updates to:
       > ℹ️ *All **{filteredLeads.length}** leads matching search are selected.* **[Clear selection]**
3. **Action Bar Integration**:
   - The bulk action button **Assign to Category ({selectedRows.size})** updates to show the full count (e.g., `Assign to Category (186)`).
   - Clicking **Assign to Category** passes the full set of IDs to `openCategoryModal()` and `handleAssignCategory()`.
4. **Filter & Search Safety**:
   - Modifying `searchQuery`, `pageSize`, or any dropdown filters automatically clears `selectedRows` to prevent accidental bulk actions on stale selection states.

## 3. Architecture & Code Changes
- **Target File**: [LeadsList.svelte](file:///home/arch/Project/test/RustSale_v2/src/lib/components/LeadsList.svelte)
- **State Additions/Updates**:
  - `isAllFilteredSelected`: Derived boolean (`selectedRows.size === filteredLeads.length && filteredLeads.length > 0`).
  - `isCurrentPageAllSelected`: Derived boolean (`paginatedLeads.length > 0 && paginatedLeads.every(l => l.id && selectedRows.has(l.id))`).
- **Helper Functions**:
  - `selectAllFiltered()`: Iterates `filteredLeads` and adds all `l.id` to `selectedRows`.
  - `clearSelection()`: Clears `selectedRows`.
- **UI Structure**:
  - Add a banner component right above `<Table.Root>` that conditionally renders based on `selectedRows.size > 0`.

## 4. Error Handling & Performance
- `crmService.addLeadsToCategory` handles bulk ID arrays (`number[]`). The backend SQLite command uses a transaction batch insert which completes in < 10ms for thousands of records.
- Null/undefined ID safety checks are preserved.

## 5. Verification Plan
- Unit/Component tests in `src/lib/components/__tests__/LeadsList.spec.ts` (if existing) or manual verification of filter + multi-page selection.
- Run `bun test` and `bun run check` to ensure zero regressions and zero svelte-check errors.
