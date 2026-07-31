# Zalo Outreach Conversation Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist every successful Zalo Outreach delivery as one outbound message in the correct direct provider conversation, using the actual IMEI and Zalo UID, without creating groups or sending the same lead once per selected session.

**Architecture:** Keep provider sending in the existing Outreach functions for the first integration. A focused transaction helper in `conversation/outreach.rs` upserts the Zalo account, linked identity, direct conversation, participant, and outbound message inside the same SQLite transaction as the existing lead/activity/outreach-history success updates. Provider responses retain message IDs before persistence; compact Conversation invalidations publish only after commit.

**Tech Stack:** Rust, rusqlite transactions, Tauri managed state/events, vendored `zca-rs`, existing Outreach mock tests, Cargo test tooling.

---

## Preconditions and Risk Gates

- Complete the storage and Zalo sync child plans first.
- Re-run GitNexus impact before implementation:

```text
mcp__gitnexus.impact({repo: "RustSale_v2", target: "send_zalo_outreach_internal", direction: "upstream", maxDepth: 3, includeTests: true})
mcp__gitnexus.impact({repo: "RustSale_v2", target: "send_zalo_campaign_internal", direction: "upstream", maxDepth: 3, includeTests: true})
mcp__gitnexus.impact({repo: "RustSale_v2", target: "start_zalo_outreach", direction: "upstream", maxDepth: 3, includeTests: true})
```

- Current known risk: `send_zalo_outreach_internal` LOW with four direct dependents; `send_zalo_campaign_internal` MEDIUM with five direct dependents; `start_zalo_outreach` LOW.
- Do not route Outreach through the durable composer outbox in this phase. Existing campaign timing, rate-limit handling, template rendering, attachment sending, and verification remain in place.
- Never create `ConversationKind::Group` from Outreach data.
- Use only `bun`/`bunx` for frontend commands and Cargo for Rust.

## Task 1: Add Provider Receipt and Outreach Persistence Contracts

**Files:**
- Create: `src-tauri/src/conversation/outreach.rs`
- Create: `src-tauri/src/conversation/outreach_test.rs`
- Modify: `src-tauri/src/conversation/mod.rs`

- [ ] **Step 1: Write failing provider receipt tests**

Create `outreach_test.rs` and cover:

```rust
#[test]
fn provider_receipt_prefers_text_message_id() {
    let receipt = ZaloProviderReceipt::from_response(SendMessageResponse {
        message: Some(SendMessageResult { msg_id: 42 }),
        attachment: vec![SendMessageResult { msg_id: 99 }],
    });
    assert_eq!(receipt.primary_message_id.as_deref(), Some("42"));
    assert_eq!(receipt.attachment_message_ids, vec!["99"]);
}

#[test]
fn provider_receipt_uses_first_attachment_when_text_is_absent() {
    let receipt = ZaloProviderReceipt::from_response(SendMessageResponse {
        message: None,
        attachment: vec![SendMessageResult { msg_id: 99 }],
    });
    assert_eq!(receipt.primary_message_id.as_deref(), Some("99"));
}
```

Also assert an empty provider response returns `primary_message_id = None` but preserves the local request ID for deduplication.

- [ ] **Step 2: Run the test and verify failure**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::outreach_test::provider_receipt
```

Expected: FAIL because the module and types do not exist.

- [ ] **Step 3: Define the contracts**

Create `outreach.rs`:

```rust
use crate::conversation::repository::AppliedMessage;
use rusqlite::Transaction;
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZaloProviderReceipt {
    pub primary_message_id: Option<String>,
    pub attachment_message_ids: Vec<String>,
}

impl ZaloProviderReceipt {
    pub fn from_response(response: zca_rs::models::SendMessageResponse) -> Self {
        let text_message_id = response.message.map(|message| message.msg_id.to_string());
        let attachment_message_ids = response
            .attachment
            .into_iter()
            .map(|message| message.msg_id.to_string())
            .collect::<Vec<_>>();
        let primary_message_id = text_message_id
            .or_else(|| attachment_message_ids.first().cloned());
        Self {
            primary_message_id,
            attachment_message_ids,
        }
    }

    pub fn mock(local_request_id: &str) -> Self {
        Self {
            primary_message_id: Some(format!("mock-{local_request_id}")),
            attachment_message_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OutreachConversationRecord {
    pub lead_id: i64,
    pub provider_account_key: String,
    pub account_display_name: Option<String>,
    pub external_user_id: String,
    pub user_display_name: Option<String>,
    pub user_avatar_url: Option<String>,
    pub template_id: i64,
    pub source: String,
    pub text: String,
    pub local_request_id: String,
    pub provider_receipt: ZaloProviderReceipt,
    pub sent_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedOutreachMessage {
    pub applied: AppliedMessage,
    pub lead_id: i64,
}
```

Add to `conversation/mod.rs`:

```rust
pub mod outreach;

#[cfg(test)]
mod outreach_test;
```

- [ ] **Step 4: Run provider receipt tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::outreach_test::provider_receipt
```

Expected: PASS.

## Task 2: Persist Successful Outreach in One Transaction

**Files:**
- Modify: `src-tauri/src/conversation/outreach.rs`
- Modify: `src-tauri/src/conversation/outreach_test.rs`

- [ ] **Step 1: Write failing transaction tests**

Build a temporary database with the Conversation migration and a lead. Add exact tests:

```rust
#[test]
fn outreach_success_creates_direct_conversation_and_no_group() {
    let applied = record_outreach_success_in_transaction(&tx, &record()).unwrap();
    assert!(!applied.applied.duplicate);
    assert_eq!(count(&tx, "channel_conversations", "kind = 'direct'"), 1);
    assert_eq!(count(&tx, "channel_conversations", "kind = 'group'"), 0);
}

#[test]
fn same_provider_message_id_is_idempotent() {
    let first = record_outreach_success_in_transaction(&tx, &record()).unwrap();
    let second = record_outreach_success_in_transaction(&tx, &record()).unwrap();
    assert_eq!(first.applied.message_id, second.applied.message_id);
    assert!(second.applied.duplicate);
}

#[test]
fn same_lead_through_two_imeis_has_two_provider_conversations() {
    let mut first = record();
    first.provider_account_key = "imei-a".to_string();
    first.local_request_id = "request-a".to_string();
    first.provider_receipt.primary_message_id = Some("message-a".to_string());
    let mut second = first.clone();
    second.provider_account_key = "imei-b".to_string();
    second.local_request_id = "request-b".to_string();
    second.provider_receipt.primary_message_id = Some("message-b".to_string());

    record_outreach_success_in_transaction(&tx, &first).unwrap();
    record_outreach_success_in_transaction(&tx, &second).unwrap();

    assert_eq!(count(&tx, "channel_conversations", "kind = 'direct'"), 2);
    assert_eq!(count(&tx, "channel_identities", "lead_id = 7"), 1);
}

#[test]
fn rolled_back_crm_transaction_leaves_no_conversation_rows() {
    let path = temp_database_path();
    let mut connection = initialized_connection(&path);
    let tx = connection.transaction().unwrap();
    record_outreach_success_in_transaction(&tx, &record()).unwrap();
    tx.rollback().unwrap();
    drop(connection);

    let reopened = open_db_connection(path.to_str().unwrap()).unwrap();
    assert_eq!(count(&reopened, "channel_conversations", "1 = 1"), 0);
    assert_eq!(count(&reopened, "messages", "1 = 1"), 0);
}
```

Also verify:

- `channel_identities.lead_id` equals the Outreach lead;
- `channel_accounts.provider_account_key` equals the actual IMEI;
- `messages.direction = 'outbound'`, `status = 'sent'`;
- `messages.content_json` contains `templateId`, `source`, and all attachment message IDs;
- the participant row links only the customer identity, not the local account.

- [ ] **Step 2: Run the transaction tests and verify failure**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::outreach_test
```

Expected: FAIL because the transaction helper does not exist.

- [ ] **Step 3: Implement the transaction helper**

Add:

```rust
pub fn record_outreach_success_in_transaction(
    tx: &Transaction<'_>,
    record: &OutreachConversationRecord,
) -> rusqlite::Result<AppliedOutreachMessage>;
```

The helper performs these exact operations without opening a second connection:

1. Upsert `channel_accounts` with `channel = 'zalo'` and `provider_account_key = record.provider_account_key`.
2. Upsert `channel_identities` with namespace `zalo_uid`, the successful `external_user_id`, `lead_id`, display name, and avatar.
3. Upsert `channel_conversations` using `(channel_account_id, external_user_id, 'direct')`.
4. Upsert the customer participant as role `member`.
5. Find an existing message by `(conversation_id, provider_message_id)` when a provider ID exists; otherwise find by `local_request_id`.
6. Insert one logical outbound message when no duplicate exists.
7. Return `AppliedOutreachMessage` with the same `AppliedMessage` shape used by realtime ingestion.

Use this exact content payload:

```rust
let content = json!({
    "text": record.text,
    "outreach": {
        "templateId": record.template_id,
        "source": record.source,
        "providerAttachmentMessageIds": record.provider_receipt.attachment_message_ids,
    }
});
```

The inserted message fields are:

```text
direction = outbound
content_type = text when no attachment IDs exist, otherwise template
text_content = rendered Outreach text
provider_message_id = primary provider ID
local_request_id = generated before provider send
provider_timestamp = sent_at
ingested_at = sent_at
status = sent
```

The helper never accepts a conversation kind parameter; it always writes `direct`.

- [ ] **Step 4: Run transaction tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::outreach_test
```

Expected: PASS.

## Task 3: Publish Conversation Invalidation After Commit

**Files:**
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/service_test.rs`

- [ ] **Step 1: Write a failing event test**

Add a service test that subscribes to `ConversationEventPublisher`, calls the new method, and asserts:

```rust
assert_eq!(event.conversation_id, applied.applied.conversation_id);
assert_eq!(event.lead_id, Some(applied.lead_id));
assert_eq!(event.channel, Channel::Zalo);
assert_eq!(event.reason, ConversationInvalidationReason::Message);
assert_eq!(event.message_id, Some(applied.applied.message_id));
```

- [ ] **Step 2: Implement the service method**

Add:

```rust
pub fn publish_outreach_success(&self, applied: &AppliedOutreachMessage) {
    if applied.applied.duplicate {
        return;
    }
    self.events.publish(ConversationInvalidatedEvent {
        conversation_id: applied.applied.conversation_id,
        lead_id: Some(applied.lead_id),
        channel: Channel::Zalo,
        reason: ConversationInvalidationReason::Message,
        message_id: Some(applied.applied.message_id),
    });
}
```

Call this method only after the surrounding SQLite transaction commits.

- [ ] **Step 3: Run service tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::service_test
```

Expected: PASS.

## Task 4: Integrate Single-Session Zalo Outreach Success Paths

**Files:**
- Modify: `src-tauri/src/commands.rs:1579-2341`
- Modify: `src-tauri/src/commands_test.rs`

- [ ] **Step 1: Extend mock Outreach tests**

After `test_send_zalo_outreach_mock` succeeds, assert:

```sql
SELECT COUNT(*)
FROM channel_conversations
WHERE kind = 'direct';
```

equals the number of successful leads, and:

```sql
SELECT COUNT(*)
FROM channel_conversations
WHERE kind = 'group';
```

equals zero. Assert failed/not-found leads have no message rows.

Add an assertion that the message content contains the rendered lead name and `source = 'zalo_outreach'`.

- [ ] **Step 2: Run the focused test and verify failure**

```bash
cargo test --manifest-path src-tauri/Cargo.toml test_send_zalo_outreach_mock
```

Expected: FAIL because the success path does not persist Conversation rows.

- [ ] **Step 3: Preserve provider receipts in the real send branch**

Generate before each provider send:

```rust
let local_request_id = format!("outreach-{}", rand::random::<u128>());
let conversation_sent_at = crate::conversation::types::utc_now();
```

Change the real result branch from `Ok(_)` to:

```rust
Ok(response) => {
    let provider_receipt =
        crate::conversation::outreach::ZaloProviderReceipt::from_response(response);
    // Existing success transaction follows.
}
```

The mock branch uses `ZaloProviderReceipt::mock(&local_request_id)`.

- [ ] **Step 4: Insert Conversation persistence into existing success transactions**

Inside each existing success transaction, after the lead/activity/history statements and before `tx.commit()`, call:

```rust
let applied_outreach = crate::conversation::outreach::record_outreach_success_in_transaction(
    &tx,
    &crate::conversation::outreach::OutreachConversationRecord {
        lead_id: i64::from(*lead_id),
        provider_account_key: imei.clone(),
        account_display_name: None,
        external_user_id: user.uid.clone(),
        user_display_name: Some(lead_name.clone()),
        user_avatar_url: None,
        template_id: i64::from(template_id),
        source: "zalo_outreach".to_string(),
        text: formatted_content.clone(),
        local_request_id: local_request_id.clone(),
        provider_receipt,
        sent_at: conversation_sent_at.clone(),
    },
)?;
```

For the mock branch, use a deterministic mock UID derived from sanitized phone:

```rust
let external_user_id = format!("mock-uid-{sanitized_phone}");
```

After commit, when `app_handle` exists, resolve `ConversationState` and call `state.service.publish_outreach_success(&applied_outreach)`.

- [ ] **Step 5: Handle post-send persistence failures explicitly**

If the provider send succeeded but the success transaction fails:

- keep the delivery counted as provider-sent;
- emit an Outreach log containing `Delivery succeeded but local persistence failed`;
- do not call the provider again automatically;
- do not insert a partial Conversation row outside the failed transaction.

This avoids duplicate messages after a database fault.

- [ ] **Step 6: Run single-session regression tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml test_send_zalo_outreach_mock
cargo test --manifest-path src-tauri/Cargo.toml test_send_zalo_outreach_burst_limit
```

Expected: PASS.

## Task 5: Ensure Multi-Session Start Sends Each Lead Once

**Files:**
- Modify: `src-tauri/src/commands.rs:1461-1497`
- Modify: `src-tauri/src/commands_test.rs`

- [ ] **Step 1: Write a failing pure partition test**

Extract and test:

```rust
#[test]
fn selected_sessions_partition_leads_without_duplication() {
    let assignments = partition_outreach_leads(
        &["imei-a".to_string(), "imei-b".to_string()],
        &[1, 2, 3, 4, 5],
    );
    assert_eq!(assignments[0], ("imei-a".to_string(), vec![1, 3, 5]));
    assert_eq!(assignments[1], ("imei-b".to_string(), vec![2, 4]));
    let all = assignments
        .into_iter()
        .flat_map(|(_, lead_ids)| lead_ids)
        .collect::<Vec<_>>();
    assert_eq!(all.len(), 5);
    assert_eq!(all.iter().collect::<std::collections::HashSet<_>>().len(), 5);
}
```

Also assert empty IMEIs return an error and duplicate lead IDs are deduplicated while preserving first-seen order.

- [ ] **Step 2: Implement deterministic partitioning**

Add:

```rust
pub fn partition_outreach_leads(
    imeis: &[String],
    lead_ids: &[i32],
) -> Result<Vec<(String, Vec<i32>)>, String>;
```

Deduplicate IMEIs and lead IDs, reject an empty IMEI set, then assign each lead round-robin to exactly one session.

- [ ] **Step 3: Update `start_zalo_outreach` orchestration**

Replace the current nested behavior where every IMEI receives the full `lead_ids` list. Call `partition_outreach_leads`, then invoke `send_zalo_outreach_internal` once per non-empty partition.

Add an internal progress scope so partition calls report one global run:

```rust
#[derive(Debug, Clone, Copy)]
pub struct OutreachProgressScope {
    pub processed_offset: usize,
    pub total_leads: usize,
    pub emit_complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZaloOutreachBatchResult {
    pub total_sent: usize,
}
```

Change the internal signature to:

```rust
pub async fn send_zalo_outreach_internal(
    app_handle: Option<&tauri::AppHandle>,
    conn_mutex: &std::sync::Mutex<rusqlite::Connection>,
    imei: String,
    lead_ids: Vec<i32>,
    template_id: i32,
    options: ZaloOutreachRunOptions,
    progress_scope: OutreachProgressScope,
) -> Result<ZaloOutreachBatchResult, String>;
```

The function uses `(processed_offset + local_index + 1) / total_leads` for progress and emits `outreach-complete` only for the final partition. Return `ZaloOutreachBatchResult { total_sent }` so the wrapper can aggregate the final sent count.

- [ ] **Step 4: Update direct callers and tests**

Update:

- `send_zalo_outreach` to pass a single full-run scope and map the batch result to `()`;
- `start_zalo_outreach` to pass partition scopes;
- `test_send_zalo_outreach_mock` and burst-limit tests to assert `result.total_sent`;
- progress event tests to assert one final complete event.

- [ ] **Step 5: Run the focused tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml partition_outreach_leads
cargo test --manifest-path src-tauri/Cargo.toml test_send_zalo_outreach_
```

Expected: PASS and no lead is sent once per selected IMEI.

## Task 6: Integrate Rotating Zalo Campaign Success Paths

**Files:**
- Modify: `src-tauri/src/commands.rs:2494-2982`
- Modify: `src-tauri/src/commands_test.rs`

- [ ] **Step 1: Extend campaign tests**

Add assertions to the existing mock campaign tests:

1. Successful leads create direct conversations and zero groups.
2. `channel_accounts.provider_account_key` equals the `current_imei` that actually sent the lead.
3. When `mock-fail-imei` is evicted and the next session succeeds, the message records the succeeding IMEI.
4. `mock-send-fail` creates no outbound Conversation message.
5. Multiple leads create one direct conversation per `(successful IMEI, Zalo UID)` pair.

- [ ] **Step 2: Run campaign tests and verify failure**

```bash
cargo test --manifest-path src-tauri/Cargo.toml test_send_zalo_campaign_
```

Expected: existing assertions pass, new Conversation assertions fail.

- [ ] **Step 3: Preserve the campaign provider response**

Change:

```rust
let send_result: Result<(), zca_rs::ZaloError>
```

to:

```rust
let local_request_id = format!("campaign-{}", rand::random::<u128>());
let send_result: Result<crate::conversation::outreach::ZaloProviderReceipt, zca_rs::ZaloError>
```

The real branch maps `SendMessageResponse` with `ZaloProviderReceipt::from_response`; the mock success branch returns `ZaloProviderReceipt::mock(&local_request_id)`.

- [ ] **Step 4: Make each campaign success transaction atomic**

On `Ok(provider_receipt)`, open one mutable DB guard and transaction. Inside it:

1. Insert the existing `outreach_history` success row.
2. Update `leads.zalo_reached`, `has_zalo`, and `last_activity`.
3. Call `record_outreach_success_in_transaction` with `provider_account_key = current_imei`, `external_user_id = uid`, `source = 'zalo_campaign'`, and the rendered content.
4. Commit.
5. Increment `total_sent` and publish invalidation after commit.

Do not call `log_outreach_attempt` on a separate connection for successful sends because that would break atomicity.

- [ ] **Step 5: Preserve failure and rotation semantics**

Keep failed attempts in `outreach_history`, leave `zalo_reached = 0`, and rotate/evict the same sessions as before. Conversation persistence runs only in the successful branch and therefore cannot create phantom messages for lookup/send failures.

- [ ] **Step 6: Run all campaign regressions**

```bash
cargo test --manifest-path src-tauri/Cargo.toml test_send_zalo_campaign_mock
cargo test --manifest-path src-tauri/Cargo.toml test_send_zalo_campaign_continues_after_lead_send_failure
cargo test --manifest-path src-tauri/Cargo.toml test_send_zalo_campaign_eviction_on_lookup_failure
cargo test --manifest-path src-tauri/Cargo.toml test_send_zalo_campaign_exhausted_sessions
```

Expected: PASS.

## Task 7: Verify Timeline Semantics and Deduplication

**Files:**
- Modify: `src-tauri/src/conversation/outreach_test.rs`
- Modify: `src-tauri/src/commands_test.rs`

- [ ] **Step 1: Add end-to-end repository assertions**

After persisting Outreach through command tests, instantiate `ConversationRepository` and assert:

```rust
let page = repository
    .get_contact_timeline(lead_id, &[Channel::Zalo], None, 50)
    .unwrap();
assert_eq!(page.items.len(), expected_successful_sends);
assert!(page.items.iter().all(|message| message.direction == MessageDirection::Outbound));
```

For one lead contacted through two IMEIs, assert the inbox has one `lead:{lead_id}` customer row with two `reply_targets` and one Zalo badge.

- [ ] **Step 2: Add replay/idempotency test**

Persist the same provider response twice through the transaction helper, then apply a matching realtime self-message with the same provider message ID. Assert one logical message remains. If the realtime event enriches content/status, it updates the existing row instead of inserting another.

- [ ] **Step 3: Run Conversation and Outreach tests together**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::
cargo test --manifest-path src-tauri/Cargo.toml test_send_zalo_
```

Expected: PASS.

## Task 8: Full Verification and Scope Audit

**Files:**
- No new files

- [ ] **Step 1: Run the full Rust suite**

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS, including existing Zalo login/session verification and all Outreach tests.

- [ ] **Step 2: Run formatting and frontend regressions**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
bun test
bun run check
git diff --check
```

Expected: PASS.

- [ ] **Step 3: Run manual Outreach smoke verification**

With two non-production Zalo personal sessions and three test leads:

1. Launch one Outreach run selecting both IMEIs.
2. Confirm each lead receives one direct message, not one message per IMEI.
3. Confirm zero Zalo groups are created.
4. Confirm Conversation shows one customer row per lead.
5. Confirm the message account label matches the actual IMEI used.
6. Confirm one lead sent through a second IMEI later shows two reply targets but one unified timeline.
7. Restart the app and confirm successfully persisted messages are not duplicated.

- [ ] **Step 4: Verify graph scope**

Run:

```text
mcp__gitnexus.detect_changes({repo: "RustSale_v2", scope: "all"})
```

Expected: the new Conversation Outreach helper, Conversation service event method, targeted Outreach internals, their direct callers, and tests only. Stop if unrelated Google Maps, Email Outreach, or CRM flows appear.
