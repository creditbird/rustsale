# UI/UX Refactor with shadcn-svelte Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the entire UI/UX of RustSale v2 using `shadcn-svelte` (Preset `b5JgPHhcO`), Tailwind CSS v4, and Lucide Svelte icons to create a high-performance, modern CRM & Outreach enterprise desktop app.

**Architecture:** Initialize shadcn-svelte base components (`button`, `card`, `badge`, `input`, `table`, `sheet`, `dialog`, `tabs`, `dropdown-menu`, `progress`), update global layout & app shell (`Sidebar.svelte`, `+page.svelte`), and iteratively upgrade each functional view module (`Dashboard`, `LeadsList`, `LeadDetails`, `CategoryManager`, `KanbanBoard`, `GoogleMapsScraper`, `ZaloOutreach`, `EmailOutreach`).

**Tech Stack:** Svelte 5 (Runes `$state`, `$derived`, `$effect`), SvelteKit 2, Tailwind CSS v4 (`@tailwindcss/vite`), `shadcn-svelte` (Preset `b5JgPHhcO`), `@lucide/svelte`.

## Global Constraints

- ALWAYS use `bun` and `bunx` instead of `npm` and `npx`.
- Run `bun run check` (`svelte-check`) to verify zero type/syntax errors after every task.
- Run `bunx gitnexus analyze` after committing changes.

---

### Task 1: Install shadcn-svelte primitives & Refactor App Shell (`Sidebar.svelte`, `+page.svelte`)

**Files:**
- Create/Install: `src/lib/components/ui/*`
- Modify: `src/lib/components/Sidebar.svelte`
- Modify: `src/routes/+page.svelte`
- Modify: `src/routes/+layout.svelte`

**Interfaces:**
- Consumes: Navigation tab state (`bind:activeTab`)
- Produces: Responsive sidebar and app shell layout with dark/light themes and Lucide icons

- [ ] **Step 1: Install shadcn-svelte UI components**

Run: `bunx shadcn-svelte add button card badge input table dialog tabs sheet dropdown-menu progress --overwrite`
Expected: Components added to `src/lib/components/ui/`

- [ ] **Step 2: Refactor `Sidebar.svelte` with Lucide icons and modern navigation styling**

Update `src/lib/components/Sidebar.svelte` to use `@lucide/svelte` icons (`LayoutDashboard`, `Users`, `FolderKanban`, `Tags`, `MapPin`, `MessageSquare`, `Mail`) and `shadcn-svelte` styling rules.

- [ ] **Step 3: Refactor `src/routes/+page.svelte` and `src/routes/+layout.svelte` app shell**

Update layout wrapper to include clean header toolbar, error notification banners using shadcn alerts/cards, and active view switching.

- [ ] **Step 4: Verify type checks**

Run: `bun run check`
Expected: 0 errors

- [ ] **Step 5: Commit changes & Update GitNexus index**

```bash
git add src/
git commit -m "feat(ui): refactor app shell and sidebar with shadcn-svelte and lucide icons"
bunx gitnexus analyze
```

---

### Task 2: Refactor Dashboard & KPI Overview (`Dashboard.svelte`)

**Files:**
- Modify: `src/lib/components/Dashboard.svelte`

**Interfaces:**
- Consumes: CRM metrics & recent activity data
- Produces: Modern stat cards, conversion rate indicators, and quick action widgets

- [ ] **Step 1: Update `Dashboard.svelte` with Card & Badge components**

Replace legacy CSS cards in `Dashboard.svelte` with shadcn `Card`, `CardHeader`, `CardTitle`, `CardContent`, and `Badge` components.

- [ ] **Step 2: Verify type checks**

Run: `bun run check`
Expected: 0 errors

- [ ] **Step 3: Commit & Update GitNexus index**

```bash
git add src/lib/components/Dashboard.svelte
git commit -m "feat(ui): refactor Dashboard view with shadcn-svelte Card and Badge components"
bunx gitnexus analyze
```

---

### Task 3: Refactor Leads List & Lead Details Drawer (`LeadsList.svelte`, `LeadDetails.svelte`)

**Files:**
- Modify: `src/lib/components/LeadsList.svelte`
- Modify: `src/lib/components/LeadDetails.svelte`

**Interfaces:**
- Consumes: Lead list data, filter state, Refresh callback
- Produces: Data table with pagination/search, status badges, action dropdowns, and sliding drawer detail view

- [ ] **Step 1: Refactor `LeadsList.svelte` with shadcn Table, Input, and DropdownMenu**

Replace custom HTML table with `Table`, `TableHeader`, `TableRow`, `TableCell`, `Input` for search, and status badges.

- [ ] **Step 2: Refactor `LeadDetails.svelte` into a modern Sheet / Modal view**

Update detail view to render inside a `Sheet` or `Dialog` with tabbed navigation for Lead info, timeline, and actions.

- [ ] **Step 3: Run svelte-check & tests**

Run: `bun run check && bun test`
Expected: All checks & tests pass

- [ ] **Step 4: Commit & Update GitNexus index**

```bash
git add src/lib/components/LeadsList.svelte src/lib/components/LeadDetails.svelte
git commit -m "feat(ui): refactor LeadsList and LeadDetails with shadcn-svelte Table and Sheet components"
bunx gitnexus analyze
```

---

### Task 4: Refactor Category Manager & Kanban Pipeline (`CategoryManager.svelte`, `KanbanBoard.svelte`)

**Files:**
- Modify: `src/lib/components/CategoryManager.svelte`
- Modify: `src/lib/components/KanbanBoard.svelte`

**Interfaces:**
- Consumes: Category records and Pipeline deal stages
- Produces: Dialog-driven category management and interactive Kanban deal board

- [ ] **Step 1: Refactor `CategoryManager.svelte` using Dialog and Badges**

Use shadcn `Dialog` for Add/Edit/Delete modals and `Badge` for category pills.

- [ ] **Step 2: Refactor `KanbanBoard.svelte` using Card & Button components**

Update stage columns and deal cards with Lucide icons, deal value chips, and stage transition buttons.

- [ ] **Step 3: Verify type checks**

Run: `bun run check`
Expected: 0 errors

- [ ] **Step 4: Commit & Update GitNexus index**

```bash
git add src/lib/components/CategoryManager.svelte src/lib/components/KanbanBoard.svelte
git commit -m "feat(ui): refactor CategoryManager and KanbanBoard with shadcn-svelte components"
bunx gitnexus analyze
```

---

### Task 5: Refactor Google Maps Scraper UI (`GoogleMapsScraper.svelte`)

**Files:**
- Modify: `src/lib/components/GoogleMapsScraper.svelte`

**Interfaces:**
- Consumes: Scraping options, progress events, and results table
- Produces: Modern search form, Progress bar, stats widgets, and live log console

- [ ] **Step 1: Refactor Scraper search forms and progress indicators**

Replace raw inputs and custom progress bars with shadcn `Input`, `Button`, `Progress`, and `Badge`.

- [ ] **Step 2: Refactor results table and logs panel**

Format results table with shadcn `Table` and log stream with dark monospace code block.

- [ ] **Step 3: Verify type checks**

Run: `bun run check`
Expected: 0 errors

- [ ] **Step 4: Commit & Update GitNexus index**

```bash
git add src/lib/components/GoogleMapsScraper.svelte
git commit -m "feat(ui): refactor GoogleMapsScraper UI with shadcn-svelte controls and progress"
bunx gitnexus analyze
```

---

### Task 6: Refactor Zalo & Email Outreach Modules (`ZaloOutreach.svelte`, `EmailOutreach.svelte`)

**Files:**
- Modify: `src/lib/components/ZaloOutreach.svelte`
- Modify: `src/lib/components/EmailOutreach.svelte`

**Interfaces:**
- Consumes: Session credentials, outreach campaign configuration, and template definitions
- Produces: Session status badges, interactive template composer, step wizard, and campaign log viewer

- [ ] **Step 1: Refactor `ZaloOutreach.svelte` session & campaign builder**

Update session cards with connection badges, template selector with variable insert pills, and step wizard controls.

- [ ] **Step 2: Refactor `EmailOutreach.svelte` SMTP management & template editor**

Update SMTP configuration form, template editor with variable insertion pills, and campaign summary cards.

- [ ] **Step 3: Full project verification**

Run: `bun run check && bun run build && bun test`
Expected: All checks, build, and tests pass cleanly

- [ ] **Step 4: Commit & Update GitNexus index**

```bash
git add src/lib/components/ZaloOutreach.svelte src/lib/components/EmailOutreach.svelte
git commit -m "feat(ui): refactor ZaloOutreach and EmailOutreach with shadcn-svelte components"
bunx gitnexus analyze
```
