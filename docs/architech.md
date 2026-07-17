# System Architecture Specification - RustSale v2

This document describes the high-level system architecture, data flow, and directory structure of **RustSale v2**, an enterprise CRM and commission management desktop application.

---

## 1. System Overview

RustSale v2 is built as a hybrid desktop application utilizing:
- **Core Engine (Tauri v2 + Rust):** Handles window management, native system integration, thread management, file operations, and SQLite database storage.
- **Presentation Layer (Svelte 5 + TypeScript + Vanilla CSS):** Renders the user interface inside a secure WebView, using Svelte 5 Runes for reactivity.
- **Embedded Database (SQLite):** Stores CRM and commission records locally on the user's filesystem.

```
+-------------------------------------------------------------+
|               Svelte 5 Frontend (HTML5 WebView)             |
|  - UI Pages: Dashboard, Leads, Pipeline, Commissions, Inbox  |
|  - Svelte Runes ($state, $derived, $effect)                  |
|  - Svelte Services (calling tauri.invoke())                 |
+------------------------------------+------------------------+
                                     |
                          Tauri IPC  | (JSON over Stdout/Pipe)
                                     v
+------------------------------------+------------------------+
|                   Tauri Rust Backend App Core               |
|  - Tauri Command Router & State Manager                     |
|  - SQLite Database Connection (Mutex<Connection>)           |
|  - Schema migrations & seeding logic                        |
+------------------------------------+------------------------+
                                     |
                       rusqlite APIs |
                                     v
+------------------------------------+------------------------+
|                     SQLite Database File                     |
|  - Local storage path: ~/.config/rustsale/rustsale.db       |
+-------------------------------------------------------------+
```

---

## 2. Core Modules & Responsibilities

### Rust Backend (`src-tauri`)
- `main.rs`/`lib.rs`: Entry point. Initializes the database connection, handles schema migration, seeds mock data, registers Tauri commands, and runs the Tauri runtime loop.
- `db.rs`: Holds references to database initialization and table schemas. Handles database connection pooling or synchronous locking via `Mutex<Connection>`.
- `models.rs`: Declares Rust data models that map exactly to SQLite tables. Employs `serde::Serialize` and `serde::Deserialize` for IPC serialization.
- `commands.rs`: Implements handlers marked with `#[tauri::command]` to resolve database queries requested by Svelte.

### Svelte Frontend (`src`)
- `App.svelte`: Root component managing application state, main route switching, and structural grid layout.
- `index.css`: Central styling definitions including CSS variables representing color palettes, spacings, and typographies derived from the Stitch design system.
- `lib/services/crmService.ts`: Standard TS interface wrapping Tauri's `invoke` API to fetch records and push mutations to Rust.
- `lib/components/`: Modular UI units (Sidebar, Dashboard, LeadsList, LeadDetails, KanbanBoard).

---

## 3. Data Flow & State Synchronization

1. **Query Cycle:**
   - Svelte component mounts and invokes the service: `crmService.getLeads()`.
   - The service uses Tauri's JS library: `invoke('get_leads')`.
   - Tauri serializes the call into a JSON payload and transmits it across the IPC bridge.
   - Rust handler `get_leads()` intercepts the request, locks the database Mutex, queries the tables using `rusqlite`, maps the rows to `Lead` structs, and returns them.
   - Tauri serializes the Rust structs to JSON and sends them back to Svelte, resolving the promise.
   - Svelte updates reactive state (`$state` rune), triggering a UI re-render.

2. **Mutation Cycle (e.g. Kanban Drag & Drop):**
   - User drags a card from "Proposal" to "Won".
   - Drag event captures the deal ID and stage name, calling `crmService.updateDealStage(dealId, 'Won')`.
   - Rust command `update_deal_stage(id, stage)` executes `UPDATE deals SET stage = ?1 WHERE id = ?2`.
   - Upon successful database write, Rust returns `Ok(())` to Svelte.
   - Svelte refreshes local lists to keep views synchronized.

---

## 4. Build and Development Commands

### Development Server:
Runs the hot-reloading webview developer console alongside the Rust watcher:
```bash
bun run tauri dev
```

### Production Build:
Compiles optimized native executable targets for the host platform:
```bash
bun run tauri build
```
