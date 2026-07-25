# Zalo Outreach Burst Limit Feature Design Specification

## 1. Overview
The Burst Limit feature adds configurable rate limiting and burst window protection to the Zalo Outreach campaign flow in `RustSale_v2`. This prevents Zalo account bans by enforcing burst limits (e.g. maximum N messages sent, followed by a cooldown pause period) and inter-message delay configuration, fully controllable via a UI toggle and input controls.

## 2. User Interface (`src/lib/components/ZaloOutreach.svelte`)
- **Control Section**: Added under the Campaign Settings section.
- **Checkbox Toggle**: `Kích hoạt Burst Limit (Chống block Zalo)`
  - State: `enableBurstLimit` (boolean, default: `true`).
- **Configuration Inputs** (active when toggle is enabled):
  1. `burstMaxMessages` (number, default: `5`, min: `1`): Maximum number of messages sent per burst.
  2. `burstDelaySeconds` (number, default: `30`, min: `1`): Cooldown pause duration in seconds after reaching the burst limit.
  3. `interMessageDelaySeconds` (number, default: `3`, min: `1`): Delay in seconds between individual messages.
- **UI Terminal / Activity Log**: Emits structured log events when a burst pause triggers:
  - `[Burst Limit] Sent 5 messages. Pausing for 30 seconds to protect Zalo account safety...`

## 3. Backend Architecture (`src-tauri/src/commands.rs`)

### IPC Command Signature Update
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
) -> Result<(), String>
```

### Execution Loop (`send_zalo_outreach_internal`)
1. Maintain `burst_counter: u32 = 0`.
2. For each message successfully sent:
   - Increment `burst_counter += 1`.
   - Apply inter-message delay: `tokio::time::sleep(Duration::from_secs(inter_message_delay_seconds)).await;`
   - Check if `enable_burst_limit` is `true` AND `burst_counter >= burst_max_messages`:
     - Emit progress & terminal log event detailing the burst pause.
     - Execute burst cooldown: `tokio::time::sleep(Duration::from_secs(burst_delay_seconds)).await;`
     - Reset `burst_counter = 0`.
3. Support both `mock-imei` test mode and live Zalo session mode (`zca-rs`).

## 4. Verification & Testing Strategy
- Unit / integration check of Tauri IPC invocation with mock IMEI.
- UI validation in Svelte frontend ensuring values pass correctly to `start_zalo_outreach`.
- Verification of terminal log messages during campaign execution.
