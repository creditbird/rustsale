# Outreach Server Pagination Design

**Date:** 2026-07-30

## Problem

Zalo Outreach and Email Outreach fetch every matching lead and render every lead row. The production database currently contains 1,594 leads. Browser profiling reproduced multi-second main-thread stalls while creating the two lead selectors, even though the SQLite query completed in 51ms.

## Requirements

- **REQ-001 — Server pagination:** SQLite returns at most 50 lead records per request using an ID cursor, plus the total matching count and next cursor.
- **REQ-002 — Bounded DOM:** Each Outreach component renders only the current 50-row page and never maps every matching lead into DOM nodes.
- **REQ-003 — Realtime updates:** Google Maps lead events debounce refreshes. The active page and total count refresh without reloading sessions, templates, or SMTP configuration. Inactive tabs become stale and refresh on revisit.
- **REQ-004 — Selection semantics:** “Select All” means all records matching the current backend filters. Individual deselections are stored as exclusions. Campaign launch resolves a stable ID snapshot in the backend immediately before invoking the existing campaign command.
- **REQ-005 — Manual verification:** Opening Zalo Outreach never calls `verify_all_zalo_sessions`; verification runs only from the `Check All` action.
- **REQ-006 — Cache behavior:** Previously loaded pages may display from memory immediately. Filter, category, channel, and search changes invalidate incompatible page caches and return to page one.
- **REQ-007 — Failure behavior:** A failed initial page/configuration load keeps `hasLoaded` false and retries after tab revisit. A failed realtime refresh retains the last successful page and remains stale.

## Backend Contract

`get_outreach_leads_page` accepts category, segmentation, template, channel, search, Zalo quick filters, cursor, and limit. It returns:

```ts
interface OutreachLeadsPage {
  items: Lead[];
  total: number;
  next_cursor: number | null;
}
```

`resolve_outreach_lead_ids` accepts the same filters plus excluded IDs and returns the ordered campaign snapshot. Existing campaign commands remain unchanged.

## Frontend Behavior

- Page size is fixed at 50.
- Previous/Next navigation uses cached cursor history.
- Search/filter changes are debounced and reset pagination.
- Page one refreshes automatically for active realtime events.
- Later pages refresh in place and expose the updated total without jumping position.
- Selection count is `total - excludedIds.size` in all-matching mode.

## Non-Goals

- No database migration.
- No changes to the main Lead Management table.
- No third-party virtual-list dependency.
- No changes to Zalo or Email campaign execution internals.

## Acceptance Evidence

- Rust tests verify page size, cursor order, total count, filtering, and excluded-ID resolution.
- Svelte tests verify at most 50 lead rows render from a dataset larger than 50, realtime refresh does not reload unrelated configuration, and Zalo verification is manual.
- Browser benchmark with 1,594 mocked leads shows no multi-second long task during initial Outreach rendering.
- `bun run test`, `bun run check`, `bun run build`, Rust tests, and `git diff --check` pass.
