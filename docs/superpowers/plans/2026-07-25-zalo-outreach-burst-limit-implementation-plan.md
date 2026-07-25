# Zalo Outreach Burst Limit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a configurable Burst Limit feature for Zalo Outreach campaigns in `RustSale_v2` with UI controls and backend rate-limiting execution.

**Architecture:** Update Tauri IPC command `start_zalo_outreach` and internal engine `send_zalo_outreach_internal` in Rust to support burst counters and delays. Add Svelte state & UI controls in `ZaloOutreach.svelte` to allow users to toggle and configure burst limits per campaign run.

**Tech Stack:** Rust (Tauri 2.x, tokio), Svelte 5 (Runes `$state`), Tailwind CSS, Lucide icons.

## Global Constraints

- **Package Manager**: Use `bun` and `bunx` exclusively. Do not use `npm` or `npx`.
- **GitNexus Index**: Run `bunx gitnexus analyze` after committing changes.
- **GitNexus Impact Check**: Perform impact analysis (`gitnexus_impact`) before editing symbols.

---

### Task 1: Backend Rust Burst Limit Support

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/commands_test.rs`

**Interfaces:**
- Consumes: `app.emit("outreach-log", ...)`
- Produces: `start_zalo_outreach(app_handle, imeis, template_id, lead_ids, enable_burst_limit, burst_max_messages, burst_delay_seconds, inter_message_delay_seconds)`

- [ ] **Step 1: Write failing unit test in commands_test.rs**

Add a test in `src-tauri/src/commands_test.rs` calling `send_zalo_outreach_internal` with new burst limit parameters:

```rust
#[tokio::test]
async fn test_send_zalo_outreach_burst_limit() {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::init_db(&conn).unwrap();
    let conn_mutex = std::sync::Mutex::new(conn);

    // Insert template and mock lead
    {
        let conn_guard = conn_mutex.lock().unwrap();
        conn_guard.execute(
            "INSERT INTO outreach_templates (id, name, content) VALUES (1, 'Test', 'Hello [name]')",
            [],
        ).unwrap();
        conn_guard.execute(
            "INSERT INTO leads (id, name, phone, has_zalo) VALUES (1, 'Test Lead', '0901234567', 1)",
            [],
        ).unwrap();
    }

    let res = send_zalo_outreach_internal(
        None,
        &conn_mutex,
        "mock-imei".to_string(),
        vec![1],
        1,
        true,
        5,
        30,
        1,
    ).await;

    assert!(res.is_ok());
}
```

- [ ] **Step 2: Run cargo test to verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_send_zalo_outreach_burst_limit`
Expected: FAIL due to signature mismatch in `send_zalo_outreach_internal`.

- [ ] **Step 3: Implement burst limit parameters and logic in commands.rs**

Update `start_zalo_outreach` signature and implementation in `src-tauri/src/commands.rs`:

```rust
pub async fn start_zalo_outreach(
    app_handle: tauri::AppHandle,
    imeis: Vec<String>,
    template_id: i32,
    lead_ids: Vec<i32>,
    enable_burst_limit: bool,
    burst_max_messages: u32,
    burst_delay_seconds: u64,
    inter_message_delay_seconds: u64,
) -> Result<(), String> {
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        let db_state = app_handle_clone.state::<DbState>();
        for imei in imeis {
            let _ = send_zalo_outreach_internal(
                Some(&app_handle_clone),
                &db_state.conn,
                imei,
                lead_ids.clone(),
                template_id,
                enable_burst_limit,
                burst_max_messages,
                burst_delay_seconds,
                inter_message_delay_seconds,
            )
            .await;
        }
    });
    Ok(())
}
```

Update `send_zalo_outreach_internal` signature and burst loop logic for both mock and real flows:

```rust
pub async fn send_zalo_outreach_internal(
    app_handle: Option<&tauri::AppHandle>,
    conn_mutex: &std::sync::Mutex<Connection>,
    imei: String,
    lead_ids: Vec<i32>,
    template_id: i32,
    enable_burst_limit: bool,
    burst_max_messages: u32,
    burst_delay_seconds: u64,
    inter_message_delay_seconds: u64,
) -> Result<(), String> {
    // Inside mock flow and real flow message loops:
    let mut burst_counter: u32 = 0;
    // ...
    // After successful message send:
    burst_counter += 1;

    // Apply inter-message delay
    if inter_message_delay_seconds > 0 {
        tokio::time::sleep(std::time::Duration::from_secs(inter_message_delay_seconds)).await;
    }

    // Check burst limit
    if enable_burst_limit && burst_max_messages > 0 && burst_counter >= burst_max_messages && (idx + 1) < lead_ids.len() {
        let log_msg = format!(
            "[Burst Limit] Đã gửi {} tin. Tạm dừng {} giây để tránh khóa tài khoản Zalo...",
            burst_counter, burst_delay_seconds
        );
        if let Some(app) = app_handle {
            let _ = app.emit("outreach-log", OutreachLogPayload { message: log_msg });
        }
        tokio::time::sleep(std::time::Duration::from_secs(burst_delay_seconds)).await;
        burst_counter = 0;
    }
```

- [ ] **Step 4: Run cargo test to verify pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_send_zalo_outreach_burst_limit`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/commands_test.rs
git commit -m "feat(zalo): add burst limit and delay handling to zalo outreach command"
```

---

### Task 2: Frontend UI & Campaign Invocation

**Files:**
- Modify: `src/lib/components/ZaloOutreach.svelte`

**Interfaces:**
- Consumes: `invoke('start_zalo_outreach', { imeis, templateId, leadIds, enableBurstLimit, burstMaxMessages, burstDelaySeconds, interMessageDelaySeconds })`

- [ ] **Step 1: Add Svelte 5 state variables for Burst Limit in ZaloOutreach.svelte**

```typescript
let enableBurstLimit = $state(true);
let burstMaxMessages = $state(5);
let burstDelaySeconds = $state(30);
let interMessageDelaySeconds = $state(3);
```

- [ ] **Step 2: Add Burst Limit configuration UI block**

Insert UI section in `ZaloOutreach.svelte` under Campaign Execution card before the "Start Campaign" button:

```svelte
<!-- Burst Limit & Safety Settings -->
<div class="rounded-lg border bg-card p-4 space-y-3">
  <div class="flex items-center justify-between">
    <div class="flex items-center space-x-2">
      <Checkbox id="burst-limit-toggle" bind:checked={enableBurstLimit} disabled={isCampaignRunning} />
      <label for="burst-limit-toggle" class="text-sm font-semibold cursor-pointer select-none">
        Kích hoạt Burst Limit (Chống block Zalo)
      </label>
    </div>
    <Badge variant={enableBurstLimit ? "default" : "outline"} class={enableBurstLimit ? "bg-emerald-600" : ""}>
      {enableBurstLimit ? "Đã bật bảo vệ" : "Đã tắt"}
    </Badge>
  </div>

  {#if enableBurstLimit}
    <div class="grid grid-cols-1 md:grid-cols-3 gap-3 pt-2">
      <div>
        <label class="text-xs font-medium text-muted-foreground block mb-1">Số tin tối đa / đợt</label>
        <Input type="number" min="1" max="100" bind:value={burstMaxMessages} disabled={isCampaignRunning} class="h-8 text-sm" />
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground block mb-1">Thời gian nghỉ đợt (Giây)</label>
        <Input type="number" min="1" max="600" bind:value={burstDelaySeconds} disabled={isCampaignRunning} class="h-8 text-sm" />
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground block mb-1">Trễ giữa 2 tin (Giây)</label>
        <Input type="number" min="0" max="60" bind:value={interMessageDelaySeconds} disabled={isCampaignRunning} class="h-8 text-sm" />
      </div>
    </div>
  {/if}
</div>
```

- [ ] **Step 3: Pass Burst Limit values in startCampaign() invoke call**

Update `invoke('start_zalo_outreach', ...)` call in `startCampaign()`:

```typescript
await invoke('start_zalo_outreach', {
  imeis: selectedImeis,
  templateId: selectedTemplateId,
  leadIds: selectedLeadIds,
  enableBurstLimit,
  burstMaxMessages: Number(burstMaxMessages) || 5,
  burstDelaySeconds: Number(burstDelaySeconds) || 30,
  interMessageDelaySeconds: Number(interMessageDelaySeconds) || 3,
});
```

- [ ] **Step 4: Verify Svelte build with bun**

Run: `bun run check`
Expected: PASS with 0 errors.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/ZaloOutreach.svelte
git commit -m "feat(ui): add Burst Limit configuration controls and pass to zalo outreach campaign"
```
