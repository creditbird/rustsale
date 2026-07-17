# Design Spec: RustSale v2 - Core CRM & Base Layout
**Date:** 2026-07-17  
**Status:** PROPOSED  
**Tech Stack:** Tauri v2, Svelte 5, SQLite (`rusqlite`), Vanilla CSS (Light Mode)

---

## 1. Executive Summary & Goals
This document details the architectural design and implementation strategy for the first sub-project of **RustSale v2**, an enterprise-grade desktop CRM application. The design is based on the Stitch public project `4791221474246432333` (Enterprise CRM Design Module).

### Goals:
- High-fidelity implementation of the Light Mode corporate minimalist design system.
- Robust local-first data architecture using SQLite embedded in Rust via Tauri commands.
- Reactive frontend state management using Svelte 5 runes (`$state`, `$derived`, `$effect`).
- Establish strict QA gates at each step to ensure correctness.

---

## 2. Technology Stack & Integration Architecture

```mermaid
graph TD
    subgraph Frontend (Svelte 5 & Vite)
        A[App Layout / Shell] --> B[CRM Dashboard Overview]
        A --> C[Leads & Contacts List]
        A --> D[Lead Detailed Record]
        A --> E[Sales Pipeline Kanban]
        F[UI Services / Stores] -->|tauri.invoke| G[Tauri IPC Bridge]
    end

    subgraph Backend (Tauri v2 & Rust)
        G --> H[Tauri Command Handlers]
        H --> I[Database Connection Manager]
        I --> J[(SQLite Database File)]
    end
```

### Component Stack:
1.  **Frontend Framework:** Svelte 5 (Vite + TypeScript).
2.  **State Management:** Svelte 5 Runes.
3.  **Styling:** Native Vanilla CSS with CSS custom variables derived directly from the design token system. No TailwindCSS.
4.  **Backend Framework:** Tauri v2.
5.  **Database:** SQLite accessed synchronously via the `rusqlite` crate, wrapped in thread-safe Tauri state command handlers.
6.  **Icons:** Material Icons (Font-based) to render icons specified in the design.

---

## 3. Database Schema Design (SQLite)
The database will be initialized automatically in the user's local application data directory (e.g. `~/.config/rustsale/rustsale.db` or platform equivalent).

```sql
-- Table: leads
CREATE TABLE IF NOT EXISTS leads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    company TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('New', 'Contacted', 'Nurturing', 'Qualified', 'Lost', 'Won')),
    value REAL NOT NULL DEFAULT 0.0,
    last_activity TEXT NOT NULL, -- ISO-8601 string
    owner TEXT NOT NULL,
    email TEXT NOT NULL,
    phone TEXT NOT NULL,
    about TEXT
);

-- Table: activities
CREATE TABLE IF NOT EXISTS activities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    lead_id INTEGER NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('Email', 'Call', 'Meeting', 'Task', 'System')),
    content TEXT NOT NULL,
    created_at TEXT NOT NULL, -- ISO-8601 string
    performed_by TEXT NOT NULL,
    FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE CASCADE
);

-- Table: deals
CREATE TABLE IF NOT EXISTS deals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    lead_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    value REAL NOT NULL DEFAULT 0.0,
    stage TEXT NOT NULL CHECK(stage IN ('Prospecting', 'Demo', 'Proposal', 'Negotiation', 'Won', 'Lost')),
    created_at TEXT NOT NULL, -- ISO-8601 string
    FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE CASCADE
);
```

---

## 4. UI/UX Design System & Tokens
The styling will use a centralized CSS variables file (`src/index.css`) containing the token specifications from Stitch:

### CSS Custom Variables:
```css
:root {
  /* Colors */
  --color-background: #f8f9ff;
  --color-surface-lowest: #ffffff;
  --color-surface-low: #eff4ff;
  --color-surface: #e5eeff;
  --color-surface-high: #dce9ff;
  --color-surface-highest: #d3e4fe;
  --color-on-surface: #0b1c30;
  --color-on-surface-variant: #45464d;
  --color-inverse-surface: #213145;
  --color-inverse-on-surface: #eaf1ff;
  --color-outline: #e2e8f0;
  --color-outline-variant: #c6c6cd;
  
  --color-primary: #0f172a;
  --color-on-primary: #ffffff;
  --color-primary-container: #131b2e;
  --color-on-primary-container: #7c839b;
  
  --color-success: #10b981; /* Emerald 500 */
  --color-success-bg: #e6f7f0;
  --color-danger: #f43f5e; /* Coral/Rose 500 */
  --color-danger-bg: #ffebee;
  
  /* Typography */
  --font-family: 'Inter', system-ui, -apple-system, sans-serif;
  
  /* Spacing */
  --spacing-xs: 4px;
  --spacing-sm: 8px;
  --spacing-md: 16px;
  --spacing-lg: 24px;
  --spacing-xl: 32px;
  
  /* Border Radius */
  --radius-sm: 4px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --radius-full: 9999px;
  
  /* Transitions */
  --transition-fast: 0.15s ease;
  --transition-normal: 0.25s ease;
}
```

### Screens Layout mapping:
1.  **Sidebar:** Navigation panel fixed at `240px` (minimized to `64px`). Highlighted active tab.
2.  **Dashboard:** Metric cards (Won Value, Qualified Count), SVG/CSS-based funnels, schedule block list.
3.  **Leads List:** Clean data-grid layout with sticky header, row hover action buttons, and active search filter.
4.  **Lead Details:** Grid layout splitting details (left) and chronological activity feed (right).
5.  **Kanban Board:** Columns matching stages. Custom drag-and-drop handler utilizing HTML5 Drag and Drop API or a lightweight helper.

---

## 5. QA Gates & Quality Controls
To ensure exact compliance and robustness, the following gates are defined at each step:

| Gate ID | Area | Check Criteria | Command / Verification Method |
|---------|------|----------------|-------------------------------|
| **QA-DB** | Database | Database initialization, schema migration, and seed data verify successfully. | Rust integration tests checking connections. |
| **QA-RUST-LINT** | Rust Backend | No compiler errors, warnings, or clippy lints. | `cargo check && cargo clippy` |
| **QA-SVELTE-LINT** | Frontend | Svelte frontend builds successfully without TS or ESLint errors. | `npm run check` and compilation. |
| **QA-VISUAL** | Layout & UI | Giao diện khớp chính xác 100% về kích thước, màu sắc, khoảng cách (pixel-drift checked). | Manual pixel verification against design snapshots. |

---

## 6. Phase 1 Implementation Sub-Tasks
We decompose Phase 1 into 4 sequential tasks:
1.  **Task 1.1:** App Initialization & Workspace Config (Cargo + Svelte 5 + SQLite module).
2.  **Task 1.2:** Database Setup & Tauri Commands (migrations + CRUD command handlers).
3.  **Task 1.3:** Navigation Shell & Dashboard Screen.
4.  **Task 1.4:** Leads & Details Page + Kanban Pipeline Drag-Drop UI.
