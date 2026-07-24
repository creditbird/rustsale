# Zalo Session Health Check Design Specification

## Overview
This document specifies the design for adding live session health check (verification) capabilities to the Zalo Outreach system in RustSale Pro.

---

## 1. Requirements

- **Backend Command (`verify_zalo_session`)**:
  - Validates a single Zalo session by IMEI.
  - Performs authentication with Zalo API using stored cookies and fetches account profile (`/api/social/profile/me-v2`).
  - Returns `ZaloSessionStatus` containing validity, display name, avatar, and error message.
- **Backend Command (`verify_all_zalo_sessions`)**:
  - Concurrently validates all saved Zalo sessions stored in the SQLite database (`zalo_sessions`).
  - Returns `Vec<ZaloSessionStatus>`.
- **Frontend UI (`ZaloOutreach.svelte`)**:
  - Automatically triggers session health verification on page load (`onMount`).
  - Displays explicit, real-time status badges for each session:
    - 🟢 **Valid (Hoạt động)** with account display name/avatar if available.
    - 🔴 **Expired/Invalid (Hết hạn)** with error context and a quick action to re-scan QR.
    - 🔄 **Checking (Đang kiểm tra...)** spinner.
  - Adds a manual "Verify All" button in the Zalo Sessions card header and individual "Refresh / Verify" button per session item.

---

## 2. Architecture & Data Structures

### Rust Structs (`src-tauri/src/models.rs`)
```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZaloSessionStatus {
    pub imei: String,
    pub is_valid: bool,
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub error_message: Option<String>,
}
```

### Backend Commands (`src-tauri/src/commands.rs`)
- `verify_zalo_session(imei: String, state: State<DbState>) -> Result<ZaloSessionStatus, String>`
- `verify_all_zalo_sessions(state: State<DbState>) -> Result<Vec<ZaloSessionStatus>, String>`

### Handler Registration (`src-tauri/src/lib.rs`)
Register `verify_zalo_session` and `verify_all_zalo_sessions` in Tauri builder invocation handler.

---

## 3. Frontend Integration (`src/lib/components/ZaloOutreach.svelte`)

- State tracking:
  ```ts
  interface SessionStatusInfo {
    checking: boolean;
    valid: boolean | null;
    displayName?: string;
    avatar?: string;
    error?: string;
  }

  let sessionStatuses = $state<Record<string, SessionStatusInfo>>({});
  ```
- Component lifecycle:
  - `loadSessions()` fetches session list.
  - `verifyAllSessions()` is triggered on mount and on manual button clicks.
- UI elements:
  - Header button: "Kiểm tra tất cả phiên" (Verify All Sessions).
  - Item button: "Re-verify" (Kiểm tra lại).
  - Status badge with color-coded feedback and tooltips.
