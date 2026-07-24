# Zalo Session Verification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement live Zalo session health checking in Tauri backend and Svelte frontend to allow users to see valid/expired session status and re-verify sessions on demand.

**Architecture:** Add `ZaloSessionStatus` struct and `verify_zalo_session`/`verify_all_zalo_sessions` Tauri commands in Rust that attempt `Zalo::login` and `fetch_account_info`. Integrate verification status into `ZaloOutreach.svelte` with visual badges and manual re-check actions.

**Tech Stack:** Rust (Tauri v2, zca-rs, Tokio), Svelte 5 (SvelteKit, Runes `$state`), TypeScript.

## Global Constraints

- ALWAYS use `bun` and `bunx` instead of `npm` and `npx`.
- Run `gitnexus_impact` before modifying Rust backend symbols.
- Ensure all tests pass (`bun test`, `cargo test`).

---

### Task 1: Backend Data Model & Verification Commands

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/commands_test.rs`

**Interfaces:**
- Produces: `ZaloSessionStatus` struct, `verify_zalo_session(imei: String)` command, `verify_all_zalo_sessions()` command.

- [ ] **Step 1: Add ZaloSessionStatus struct to models.rs**

Add to `src-tauri/src/models.rs`:
```rust
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ZaloSessionStatus {
    pub imei: String,
    pub is_valid: bool,
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub error_message: Option<String>,
}
```

- [ ] **Step 2: Add verify_zalo_session and verify_all_zalo_sessions commands in commands.rs**

In `src-tauri/src/commands.rs`:
```rust
#[tauri::command]
pub async fn verify_zalo_session(
    imei: String,
    state: tauri::State<'_, DbState>,
) -> Result<ZaloSessionStatus, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let session = crate::db::get_zalo_session(&conn, &imei)
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or_else(|| format!("Session with IMEI {} not found", imei))?;
    drop(conn);

    verify_zalo_session_internal(session).await
}

pub async fn verify_zalo_session_internal(session: crate::models::ZaloSession) -> Result<ZaloSessionStatus, String> {
    let cookies: serde_json::Value = match serde_json::from_str(&session.cookies_json) {
        Ok(c) => c,
        Err(e) => {
            return Ok(ZaloSessionStatus {
                imei: session.imei,
                is_valid: false,
                display_name: None,
                avatar: None,
                error_message: Some(format!("Invalid cookies format: {}", e)),
            });
        }
    };

    let options = zca_rs::ZaloOptions::default();
    match zca_rs::Zalo::login(session.imei.clone(), session.user_agent.clone(), cookies, options).await {
        Ok(zalo) => match zalo.fetch_account_info().await {
            Ok(info) => {
                let name = info.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
                let avatar = info.get("avatar").and_then(|v| v.as_str()).map(|s| s.to_string());
                Ok(ZaloSessionStatus {
                    imei: session.imei,
                    is_valid: true,
                    display_name: name,
                    avatar,
                    error_message: None,
                })
            }
            Err(e) => Ok(ZaloSessionStatus {
                imei: session.imei,
                is_valid: false,
                display_name: None,
                avatar: None,
                error_message: Some(format!("Failed to fetch account profile: {}", e)),
            }),
        },
        Err(e) => Ok(ZaloSessionStatus {
            imei: session.imei,
            is_valid: false,
            display_name: None,
            avatar: None,
            error_message: Some(format!("Login failed: {}", e)),
        }),
    }
}

#[tauri::command]
pub async fn verify_all_zalo_sessions(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<ZaloSessionStatus>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let sessions = crate::db::get_all_zalo_sessions(&conn).map_err(|e| format!("Database error: {}", e))?;
    drop(conn);

    let mut results = Vec::new();
    for session in sessions {
        match verify_zalo_session_internal(session).await {
            Ok(status) => results.push(status),
            Err(e) => eprintln!("Session verification error: {}", e),
        }
    }
    Ok(results)
}
```

- [ ] **Step 3: Register commands in lib.rs**

In `src-tauri/src/lib.rs`, add `commands::verify_zalo_session` and `commands::verify_all_zalo_sessions` to `tauri::Builder::default().invoke_handler(...)`.

- [ ] **Step 4: Add unit tests in commands_test.rs**

Add unit tests verifying mock behavior for `ZaloSessionStatus` serialization and internal helper methods.

- [ ] **Step 5: Run cargo test**

Run: `cargo test` in `src-tauri` directory.
Expected: PASS.

- [ ] **Step 6: Commit Backend Changes**

```bash
git add src-tauri/src/models.rs src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/src/commands_test.rs
git commit -m "feat: add Zalo session health check commands in Tauri backend"
```

---

### Task 2: Frontend Integration in ZaloOutreach.svelte

**Files:**
- Modify: `src/lib/components/ZaloOutreach.svelte`
- Modify: `src/lib/components/__tests__/ZaloOutreach.spec.ts`

**Interfaces:**
- Consumes: Tauri invoke `verify_zalo_session` and `verify_all_zalo_sessions`.

- [ ] **Step 1: Add Session Health State and Helper Functions**

In `src/lib/components/ZaloOutreach.svelte`:
```ts
  interface ZaloSessionStatus {
    imei: string;
    is_valid: boolean;
    display_name?: string;
    avatar?: string;
    error_message?: string;
  }

  interface SessionStatusInfo {
    checking: boolean;
    valid: boolean | null;
    displayName?: string;
    avatar?: string;
    error?: string;
  }

  let sessionStatuses = $state<Record<string, SessionStatusInfo>>({});
  let isVerifyingAll = $state(false);

  async function verifySession(imei: string) {
    sessionStatuses[imei] = {
      ...sessionStatuses[imei],
      checking: true
    };
    try {
      const res = await invoke<ZaloSessionStatus>('verify_zalo_session', { imei });
      sessionStatuses[imei] = {
        checking: false,
        valid: res.is_valid,
        displayName: res.display_name,
        avatar: res.avatar,
        error: res.error_message
      };
    } catch (e) {
      sessionStatuses[imei] = {
        checking: false,
        valid: false,
        error: String(e)
      };
    }
  }

  async function verifyAllSessions() {
    if (sessions.length === 0) return;
    isVerifyingAll = true;
    for (const s of sessions) {
      sessionStatuses[s.imei] = {
        ...sessionStatuses[s.imei],
        checking: true
      };
    }
    try {
      const results = await invoke<ZaloSessionStatus[]>('verify_all_zalo_sessions');
      for (const res of results) {
        sessionStatuses[res.imei] = {
          checking: false,
          valid: res.is_valid,
          displayName: res.display_name,
          avatar: res.avatar,
          error: res.error_message
        };
      }
    } catch (e) {
      console.error('Error verifying all sessions:', e);
      for (const s of sessions) {
        if (sessionStatuses[s.imei]?.checking) {
          sessionStatuses[s.imei] = { checking: false, valid: false, error: 'Verification failed' };
        }
      }
    } finally {
      isVerifyingAll = false;
    }
  }
```

Trigger `verifyAllSessions()` inside `onMount()` right after `loadSessions()` completes.

- [ ] **Step 2: Update Session Card Header and Items UI**

In `ZaloOutreach.svelte`:
- Add a "Kiểm tra tất cả" / "Verify All" button next to "Add Session" in the Zalo Sessions card header.
- On each session item card:
  - Display status icon dynamically:
    - `checking`: Spinning `sync` icon.
    - `valid === true`: Green `check_circle` icon with tooltip/display name.
    - `valid === false`: Red `error` icon with error message tooltip.
  - Display user display name if `displayName` exists alongside `IMEI: {s.imei}`.
  - Add an action button `refresh` to re-verify an individual session.

- [ ] **Step 3: Run Frontend Tests and Build Check**

Run: `bun run check && bun test`
Expected: PASS.

- [ ] **Step 4: Commit Frontend Changes**

```bash
git add src/lib/components/ZaloOutreach.svelte src/lib/components/__tests__/ZaloOutreach.spec.ts
git commit -m "feat: display Zalo session health status badges and manual verification in UI"
```
