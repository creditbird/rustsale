# Design Spec: UI/UX Refactor with shadcn-svelte (Preset: b5JgPHhcO)

**Date**: 2026-07-24  
**Project**: RustSale v2 (Svelte 5 + SvelteKit + Tailwind CSS v4 + Tauri v2)  
**Theme Preset**: `b5JgPHhcO` (Maia style, Inter Variable font, Custom CSS Variables)

---

## 1. Overview & Objectives

The goal of this refactor is to transform the existing UI/UX of **RustSale v2** into an enterprise-grade, modern, and highly responsive SaaS CRM & Outreach application. All raw custom CSS and legacy HTML controls will be upgraded to use standard `shadcn-svelte` components and Tailwind CSS v4 design tokens.

---

## 2. Architecture & Design System

- **Framework & Tooling**: Svelte 5 (Runes `$state`, `$derived`, `$effect`), SvelteKit 2, `@tailwindcss/vite` (Tailwind v4), `shadcn-svelte`.
- **Global Styles & Tokens**: Managed via `@import "shadcn-svelte/tailwind.css"` in `src/index.css`.
- **Component Directory**: `src/lib/components/ui/` (shadcn-svelte base primitives) and `src/lib/components/` (feature modules).
- **Icons**: `@lucide/svelte` replacing legacy Google Material Icons spans for consistent scaling and stroke weights.

---

## 3. Module Breakdown & Refactor Plan

### 3.1 App Shell & Navigation (`src/routes/+page.svelte`, `Sidebar.svelte`)
- **Sidebar**: Clean collapsible side navigation with Lucide icons (`LayoutDashboard`, `Users`, `FolderKanban`, `Tags`, `MapPin`, `MessageSquare`, `Mail`).
- **Header Toolbar**: Page title, real-time status indicators, dark/light theme controls.

### 3.2 Dashboard (`Dashboard.svelte`)
- **KPI Metrics Cards**: Stats using `Card`, `CardHeader`, `CardTitle`, `CardContent`, showing total leads, conversion rates, and active campaign counts.
- **Recent Activity & Deal Overview**: Styled data tables and activity feeds.

### 3.3 Leads & Contacts (`LeadsList.svelte`, `LeadDetails.svelte`)
- **Data Table**: Filterable table using `Table`, `Badge`, `DropdownMenu` for actions (Edit, Delete, Export).
- **Lead Detail Viewer**: Sliding drawer using `Sheet` or `Dialog` with tabs for Lead Info, Communications, and Activity History.

### 3.4 Pipeline Kanban Board (`KanbanBoard.svelte`)
- **Board Layout**: Stage columns with deal count and value summaries.
- **Card UI**: Drag-and-drop / stage move buttons with tag badges, contact details, and progress status.

### 3.5 Data Scraper (`GoogleMapsScraper.svelte`)
- **Control Panel**: Inputs, Select dropdowns, and primary Action Buttons for target query & location.
- **Scrape Execution**: Real-time progress bar using `Progress`, statistics grid, and streaming log viewer in a `ScrollArea` container.

### 3.6 Outreach Modules (`ZaloOutreach.svelte`, `EmailOutreach.svelte`)
- **Session Status Panel**: Visual indicators for connection state (Online, Offline, QR scan prompt).
- **Template Editor**: Interactive template composer with variable badges (`{{name}}`, `{{company}}`, `{{email}}`, etc.) that insert directly into focused input fields.
- **Campaign Execution Logs**: Tabbed views for setup, active campaign progress, and detailed execution logs.

---

## 4. Verification & Testing

- **Type & Syntax Check**: Run `bun run check` (Svelte 5 check) after each component refactor.
- **Build Verification**: Run `bun run build` to ensure static asset compilation succeeds.
