# Zalo Conversation Synchronization Spike Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove the real `zca-rs` behavior required for reliable direct/group conversation synchronization before adding production database migrations.

**Architecture:** Add a test-only live probe inside a focused `conversation::zalo` module. The probe logs into one non-production personal Zalo session, subscribes before requesting history, records only sanitized identifiers/counts, and asserts direct/group backfill plus realtime event behavior. No production state, Tauri commands, or database schema changes are introduced.

**Tech Stack:** Rust, Tokio, vendored `zca-rs`, Serde JSON, ignored Cargo integration-style tests.

---

## Preconditions

- Use a non-production Zalo personal account.
- The account must have at least one direct chat and one group chat with history.
- Prepare a second device/account that can send a direct message, a group message, a reaction, and a recall during the probe window.
- Never place cookies, user agents, IMEIs, or phone numbers in source files or committed command output.

## Task 1: Add Probe Module and Sanitized Event Summary

**Files:**
- Create: `src-tauri/src/conversation/mod.rs`
- Create: `src-tauri/src/conversation/zalo/mod.rs`
- Create: `src-tauri/src/conversation/zalo/probe.rs`
- Create: `src-tauri/src/conversation/zalo/probe_test.rs`
- Modify: `src-tauri/src/lib.rs:6-14`

- [ ] **Step 1: Run impact analysis before modifying module registration**

Run:

```text
mcp__gitnexus.impact({repo: "RustSale_v2", target: "run", direction: "upstream", maxDepth: 3, includeTests: true})
```

Expected: LOW risk with `main` as the only upstream caller. Stop if the result changes to HIGH or CRITICAL.

- [ ] **Step 2: Write the failing probe summary test**

Create `src-tauri/src/conversation/zalo/probe_test.rs`:

```rust
use super::probe::{observe_listener_event, ZaloProbeReport};
use serde_json::json;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::{Message, TMessage, UserMessage};

fn direct_message(uid_from: &str, id_to: &str, msg_id: &str) -> TMessage {
    serde_json::from_value(json!({
        "actionId": "action-1",
        "msgId": msg_id,
        "cliMsgId": "client-1",
        "msgType": "webchat",
        "uidFrom": uid_from,
        "idTo": id_to,
        "dName": "Probe Sender",
        "ts": "1785456000000",
        "status": 1,
        "content": "hello",
        "notify": "",
        "ttl": 0,
        "userId": "",
        "uin": "",
        "propertyExt": null,
        "paramsExt": {
            "countUnread": 1,
            "containType": 0,
            "platformType": 1
        },
        "cmd": 501,
        "st": 0,
        "at": 0,
        "realMsgId": "real-1",
        "quote": null
    }))
    .unwrap()
}

#[test]
fn probe_summary_records_sanitized_direct_identifiers() {
    let event = ListenerEvent::Message(Box::new(Message::User(UserMessage::new(
        "self-uid",
        direct_message("peer-uid", "0", "message-1"),
    ))));
    let mut report = ZaloProbeReport::default();

    observe_listener_event(&mut report, &event);

    assert_eq!(report.direct_messages, 1);
    assert_eq!(report.group_messages, 0);
    assert_eq!(report.provider_message_ids, vec!["message-1"]);
    assert_eq!(report.thread_ids, vec!["peer-uid"]);
    assert_eq!(report.self_messages, 0);
}
```

- [ ] **Step 3: Register the module and verify the test fails**

Create `src-tauri/src/conversation/mod.rs`:

```rust
pub mod zalo;
```

Create `src-tauri/src/conversation/zalo/mod.rs`:

```rust
pub mod probe;

#[cfg(test)]
mod probe_test;
```

Add to `src-tauri/src/lib.rs` beside the existing module declarations:

```rust
pub mod conversation;
```

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml probe_summary_records_sanitized_direct_identifiers
```

Expected: FAIL because `probe.rs` and its types/functions do not exist.

- [ ] **Step 4: Implement the sanitized report**

Create `src-tauri/src/conversation/zalo/probe.rs`:

```rust
use serde::Serialize;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::Message;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZaloProbeReport {
    pub direct_messages: usize,
    pub group_messages: usize,
    pub old_direct_batches: usize,
    pub old_group_batches: usize,
    pub reactions: usize,
    pub undo_events: usize,
    pub delivered_events: usize,
    pub seen_events: usize,
    pub group_events: usize,
    pub self_messages: usize,
    pub provider_message_ids: Vec<String>,
    pub client_message_ids: Vec<String>,
    pub real_message_ids: Vec<String>,
    pub thread_ids: Vec<String>,
}

fn observe_message(report: &mut ZaloProbeReport, message: &Message) {
    let (base, thread_id, is_self, is_group) = match message {
        Message::User(message) => (&message.data, &message.thread_id, message.is_self, false),
        Message::Group(message) => (
            &message.data.base,
            &message.thread_id,
            message.is_self,
            true,
        ),
    };

    if is_group {
        report.group_messages += 1;
    } else {
        report.direct_messages += 1;
    }
    if is_self {
        report.self_messages += 1;
    }
    if !base.msg_id.is_empty() {
        report.provider_message_ids.push(base.msg_id.clone());
    }
    if !base.cli_msg_id.is_empty() {
        report.client_message_ids.push(base.cli_msg_id.clone());
    }
    if !base.real_msg_id.is_empty() {
        report.real_message_ids.push(base.real_msg_id.clone());
    }
    if !thread_id.is_empty() {
        report.thread_ids.push(thread_id.clone());
    }
}

pub fn observe_listener_event(report: &mut ZaloProbeReport, event: &ListenerEvent) {
    match event {
        ListenerEvent::Message(message) => observe_message(report, message),
        ListenerEvent::OldMessages {
            messages,
            thread_type,
        } => {
            match thread_type {
                zca_rs::models::ThreadType::User => report.old_direct_batches += 1,
                zca_rs::models::ThreadType::Group => report.old_group_batches += 1,
            }
            for message in messages {
                observe_message(report, message);
            }
        }
        ListenerEvent::Reaction(_) | ListenerEvent::OldReactions { .. } => {
            report.reactions += 1;
        }
        ListenerEvent::Undo(_) => report.undo_events += 1,
        ListenerEvent::DeliveredMessages(events) => report.delivered_events += events.len(),
        ListenerEvent::SeenMessages(events) => report.seen_events += events.len(),
        ListenerEvent::GroupEvent(_) => report.group_events += 1,
        ListenerEvent::Connected
        | ListenerEvent::Disconnected { .. }
        | ListenerEvent::Closed { .. }
        | ListenerEvent::Error(_)
        | ListenerEvent::Typing(_)
        | ListenerEvent::UploadAttachment(_) => {}
    }
}
```

- [ ] **Step 5: Run the targeted test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml probe_summary_records_sanitized_direct_identifiers
```

Expected: PASS.

- [ ] **Step 6: Check formatting and diff**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml probe_summary_records_sanitized_direct_identifiers
git diff --check
```

Expected: PASS and no whitespace errors.

## Task 2: Add Ignored Live Session Probe

**Files:**
- Modify: `src-tauri/src/conversation/zalo/probe_test.rs`

- [ ] **Step 1: Add the ignored live test**

Append to `src-tauri/src/conversation/zalo/probe_test.rs`:

```rust
use std::time::Duration;
use zca_rs::context::ZaloCookie;
use zca_rs::{Options, Zalo};

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("missing required environment variable {name}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires a non-production Zalo session and operator-generated direct/group events"]
async fn live_zalo_conversation_probe() {
    let imei = required_env("RUSTSALE_ZALO_PROBE_IMEI");
    let user_agent = required_env("RUSTSALE_ZALO_PROBE_USER_AGENT");
    let cookies: Vec<ZaloCookie> = serde_json::from_str(&required_env(
        "RUSTSALE_ZALO_PROBE_COOKIES_JSON",
    ))
    .expect("cookies JSON must match Vec<ZaloCookie>");
    let timeout_seconds = std::env::var("RUSTSALE_ZALO_PROBE_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(90);

    let mut options = Options::default();
    options.self_listen = true;
    let zalo = Zalo::login(imei, user_agent, cookies, options)
        .await
        .expect("Zalo probe login failed");
    let mut receiver = zalo.subscribe();

    zalo
        .listener
        .request_old_messages(zca_rs::models::ThreadType::User, None)
        .expect("request direct history");
    zalo
        .listener
        .request_old_messages(zca_rs::models::ThreadType::Group, None)
        .expect("request group history");
    zalo
        .listener
        .request_old_reactions(zca_rs::models::ThreadType::User, None)
        .expect("request direct reactions");
    zalo
        .listener
        .request_old_reactions(zca_rs::models::ThreadType::Group, None)
        .expect("request group reactions");

    let groups = zalo.get_all_groups().await.expect("get all groups");
    let first_group_id = groups
        .grid_ver_map
        .keys()
        .next()
        .cloned()
        .expect("probe account must belong to at least one group");
    let group_history = zalo
        .get_group_chat_history(&first_group_id, Some(50))
        .await
        .expect("get group history");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_seconds);
    let mut report = ZaloProbeReport::default();
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(2), receiver.recv()).await {
            Ok(Ok(event)) => observe_listener_event(&mut report, &event),
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped))) => {
                panic!("probe receiver lagged by {skipped} events");
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                panic!("probe listener closed");
            }
            Err(_) => {}
        }
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("serialize sanitized probe report")
    );
    println!(
        "groupHistory={{groupIdLength:{},messageCount:{},more:{},lastActionIdLength:{}}}",
        first_group_id.len(),
        group_history.group_msgs.len(),
        group_history.more,
        group_history.last_action_id.len()
    );

    assert!(report.old_direct_batches >= 1, "direct history batch not observed");
    assert!(report.old_group_batches >= 1, "group history batch not observed");
    assert!(report.direct_messages >= 1, "no direct message observed");
    assert!(
        report.group_messages >= 1 || !group_history.group_msgs.is_empty(),
        "no group message observed"
    );
    assert!(
        !report.provider_message_ids.is_empty(),
        "provider message IDs were empty"
    );
    assert!(
        !report.client_message_ids.is_empty(),
        "client message IDs were empty"
    );
}
```

- [ ] **Step 2: Verify the ignored test compiles without running**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml live_zalo_conversation_probe --no-run
```

Expected: compilation succeeds.

- [ ] **Step 3: Run the live probe**

Export credentials in the active shell without writing them to files, then run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml live_zalo_conversation_probe -- --ignored --nocapture
```

During the configured window perform all actions:

1. Send a direct inbound message to the probe account.
2. Send a message from the probe account's mobile app to verify `self_listen`.
3. Send a group inbound message.
4. React to one direct or group message.
5. Recall one probe message.
6. Read one outbound message from the recipient device.

Expected: PASS and sanitized JSON counts/identifier lengths only. No cookie, phone, user-agent, or full raw payload output.

## Task 3: Verify Reconnect and Pagination Semantics

**Files:**
- Modify: `src-tauri/src/conversation/zalo/probe_test.rs`

- [ ] **Step 1: Add a deterministic cursor helper**

Append:

```rust
fn next_history_cursor(report: &ZaloProbeReport) -> Option<&str> {
    report.provider_message_ids.last().map(String::as_str)
}

#[test]
fn history_cursor_uses_last_provider_message_id() {
    let report = ZaloProbeReport {
        provider_message_ids: vec!["message-1".to_string(), "message-2".to_string()],
        ..ZaloProbeReport::default()
    };

    assert_eq!(next_history_cursor(&report), Some("message-2"));
}
```

- [ ] **Step 2: Run the helper test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml history_cursor_uses_last_provider_message_id
```

Expected: PASS.

- [ ] **Step 3: Extend the live probe with second-page and reconnect checks**

Insert after the first collection window:

```rust
if let Some(cursor) = next_history_cursor(&report) {
    zalo
        .listener
        .request_old_messages(zca_rs::models::ThreadType::User, Some(cursor))
        .expect("request second direct-history page");
}

zalo.listener.stop();
tokio::time::sleep(Duration::from_secs(2)).await;
zalo.listener.start().expect("restart listener");
let mut reconnect_receiver = zalo.subscribe();
zalo
    .listener
    .request_old_messages(zca_rs::models::ThreadType::User, next_history_cursor(&report))
    .expect("request reconciliation after reconnect");

let reconnect_event = tokio::time::timeout(Duration::from_secs(20), reconnect_receiver.recv())
    .await
    .expect("no listener event after reconnect")
    .expect("listener closed after reconnect");
observe_listener_event(&mut report, &reconnect_event);
```

The executor must confirm from the sanitized report whether the supplied cursor advances history without returning only the same page. If Zalo's cursor semantics differ, update the design spec and subsequent plans before continuing.

- [ ] **Step 4: Re-run the live probe**

Run the same ignored test command.

Expected: PASS after listener restart and at least one post-reconnect event.

## Task 4: Spike Exit Gate

**Files:**
- Modify only if evidence differs: `docs/superpowers/specs/2026-07-31-unified-conversation-design.md`

- [ ] **Step 1: Confirm required evidence**

All statements must be true:

- Direct old-message batches arrive after an explicit request.
- Group history is available through either listener old messages or `get_group_chat_history`.
- Realtime direct and group messages expose stable thread IDs.
- At least one stable provider message ID and client message ID are populated.
- `self_listen = true` captures messages sent by the logged-in account.
- Listener restart produces events and permits reconciliation requests.
- No secret-bearing fields are printed.

- [ ] **Step 2: Run the complete spike test set**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::zalo::probe_test
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
git diff --check
```

Expected: all non-ignored tests pass; ignored live probe is listed but skipped unless explicitly requested.

- [ ] **Step 3: Stop if the live gate has not passed**

Do not start the storage foundation plan based only on synthetic tests. The live probe must pass on a non-production account, or the design must be revised to match observed provider behavior.

