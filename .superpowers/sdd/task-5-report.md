# Task 5 Report: WhatsApp Supervisor & Outbox Service Seam

## 1. Overview
Task 5 delivers the WhatsApp Supervisor and concrete Outbox worker seam for WhatsApp in `RustSale_v2`. It enables full lifecycle management of WhatsApp client handles, reliable background outbox job dispatching (handling text, media, quote replies, reactions, and forwards), error taxonomy handling with retry backoff, circuit-breaker isolation on auth revocation, suppression list insertion on invalid recipients, and ambiguous timeout non-retry safety (REQ-WA-06, REQ-WA-07).

---

## 2. Files Created and Modified

1. **`src-tauri/src/conversation/whatsapp/client.rs`** (Created)
   - `WhatsAppClient`: Async trait defining `send_text_message`, `send_media_message`, `send_reaction`, `is_connected`.
   - `WhatsAppSendResponse`: Structured response with `provider_message_id` and timestamp.
   - `WhatsAppErrorKind` & `WhatsAppError`: Error classification (`AuthFailure`, `RateLimited`, `InvalidRecipient`, `AmbiguousTimeout`, `NetworkTransient`, `Permanent`).
   - `categorize_whatsapp_error`: Robust error classifier analyzing error strings and domain types.
   - `MockWhatsAppClient`: Deterministic mock client capturing dispatched messages, media, reactions, and supporting error injection and disconnection states for unit and integration testing.

2. **`src-tauri/src/conversation/whatsapp/supervisor.rs`** (Created)
   - `WhatsAppSupervisor`: Manages in-memory client handle registry per channel account, background polling runner (`run_loop`), and single-account sync/drain logic (`sync_account_internal`).
   - `ConnectedWhatsAppAccount`: Model representing active WhatsApp accounts from `channel_accounts` and `whatsapp_sessions`.
   - `get_connected_accounts`: Filters enabled WhatsApp accounts with `connected` or `synced` status.

3. **`src-tauri/src/conversation/whatsapp/supervisor_test.rs`** (Created)
   - `test_supervisor_lifecycle_and_client_registration`: Verifies client registration, retrieval, deregistration, and cancellation token stopping.
   - `test_supervisor_get_connected_accounts`: Verifies account discovery filtering for enabled/connected status vs disabled/disconnected/other channels.
   - `test_supervisor_sync_account_drains_outbox`: Tests end-to-end outbox drainage via `sync_account_internal`.
   - `test_supervisor_sync_account_handles_disconnected_client`: Verifies safety when client handle is disconnected.
   - `test_supervisor_run_loop_stops_on_cancellation`: Verifies background loop gracefully shuts down on cancellation.

4. **`src-tauri/src/conversation/service.rs`** (Modified)
   - Implemented `ConversationService::process_next_whatsapp_outbox_job(&self, channel_account_id: i64, client: &dyn WhatsAppClient) -> Result<bool>`.
   - JID resolution: automatically formats numbers to `<phone>@s.whatsapp.net` or preserves existing chat JIDs (`@g.us`, `@lid`).
   - Quote reply resolution: extracts stanza IDs, participant JID, and timestamps from parent messages.
   - Content type dispatching: supports `text`, `media` (image/video/audio/document/file/sticker), `reaction`, and `forward`.
   - Error handling:
     - `AuthFailure`: marks job failed with `ConversationErrorCode::ProviderAuth`, sets `channel_accounts.connection_status = 'invalid'`, sets `whatsapp_sessions.is_valid = 0`, publishes `AccountStatus` invalidation, and halts outbox queue drain (circuit breaker).
     - `InvalidRecipient`: marks job failed with `ConversationErrorCode::ProviderFailure`, adds recipient phone to `contact_suppression_list` with reason `invalid_number`.
     - `AmbiguousTimeout`: marks job failed with `ConversationErrorCode::DeliveryUnknown` without automatic retry (REQ-WA-07).
     - `RateLimited` / `NetworkTransient`: schedules outbox retry with exponential backoff if `attempt_count < 3`.
     - `Permanent`: fails job with `ConversationErrorCode::ProviderFailure`.

5. **`src-tauri/src/conversation/service_test.rs`** (Modified)
   - Added unit test suite covering:
     - `test_process_whatsapp_outbox_job_success`: Text message dispatch and job completion.
     - `test_process_whatsapp_outbox_job_with_quote_reply`: Quote extraction and linkage.
     - `test_process_whatsapp_outbox_job_media`: Media payload extraction (URL and caption).
     - `test_process_whatsapp_outbox_job_reaction`: Emoji reaction dispatch.
     - `test_process_whatsapp_outbox_job_retryable_error`: Retry scheduling and attempt counter.
     - `test_process_whatsapp_outbox_job_auth_error_invalidates_account`: Account invalidation, session update, and queue halt.
     - `test_process_whatsapp_outbox_job_ambiguous_timeout_does_not_retry`: Delivery unknown without immediate retry.
     - `test_process_whatsapp_outbox_job_invalid_recipient_suppresses`: Suppression list auto-insertion.

6. **`src-tauri/src/conversation/whatsapp/mod.rs`** (Modified)
   - Exported `client` and `supervisor` modules, and registered `supervisor_test`.

---

## 3. Test & Verification Evidence

### 3.1 WhatsApp Module Tests
```
running 37 tests
test conversation::whatsapp::login_test::test_start_qr_login_public_api ... ok
test conversation::whatsapp::login_test::test_login_attempt_qr_emission ... ok
test conversation::whatsapp::login_test::test_login_attempt_cancellation ... ok
test conversation::whatsapp::login_test::test_multiple_concurrent_login_attempts_isolation ... ok
test conversation::whatsapp::login_test::test_login_attempt_error_cleanup ... ok
test conversation::whatsapp::normalizer_test::test_normalizer_attachments_parsing ... ok
test conversation::whatsapp::normalizer_test::test_normalizer_formatted_phone_numbers_in_crm ... ok
test conversation::whatsapp::normalizer_test::test_normalizer_idempotent_ingestion ... ok
test conversation::whatsapp::login_test::test_finalize_login_upsert_existing_account ... ok
test conversation::whatsapp::migration_test::test_whatsapp_sessions_constraints ... ok
test conversation::whatsapp::migration_test::test_contact_suppression_list_constraints ... ok
test conversation::whatsapp::migration_test::test_whatsapp_outreach_campaigns_and_recipients_constraints ... ok
test conversation::whatsapp::login_test::test_login_attempt_finalize_success ... ok
test conversation::whatsapp::migration_test::test_whatsapp_migrations_idempotency ... ok
test conversation::whatsapp::normalizer_test::test_normalizer_normalize_message_model_conversion ... ok
test conversation::whatsapp::normalizer_test::test_normalizer_inbound_direct_message_parsing ... ok
test conversation::whatsapp::normalizer_test::test_normalizer_inbound_group_message_parsing ... ok
test conversation::whatsapp::normalizer_test::test_normalizer_leaves_ambiguous_leads_unlinked ... ok
test conversation::whatsapp::normalizer_test::test_normalizer_leaves_unknown_phone_unlinked ... ok
test conversation::whatsapp::phone_test::test_invalid_phones ... ok
test conversation::whatsapp::phone_test::test_jid_formatting ... ok
test conversation::whatsapp::phone_test::test_jid_to_phone ... ok
test conversation::whatsapp::phone_test::test_normalize_international_phones ... ok
test conversation::whatsapp::phone_test::test_normalize_vietnamese_phones ... ok
test conversation::whatsapp::phone_test::test_parse_raw_jid ... ok
test conversation::whatsapp::normalizer_test::test_normalizer_links_single_matching_lead ... ok
test conversation::whatsapp::normalizer_test::test_normalizer_links_single_matching_lead_with_domestic_format ... ok
test conversation::whatsapp::normalizer_test::test_normalizer_outbound_direct_and_group_messages ... ok
test conversation::whatsapp::normalizer_test::test_normalizer_quote_reply_parsing ... ok
test conversation::whatsapp::normalizer_test::test_normalizer_pending_reaction_replayed_when_message_arrives ... ok
test conversation::whatsapp::normalizer_test::test_normalizer_reaction_and_receipt_ingestion ... ok
test conversation::whatsapp::migration_test::test_conversation_migrate_applies_whatsapp_migrations ... ok
test conversation::whatsapp::supervisor_test::test_supervisor_sync_account_handles_disconnected_client ... ok
test conversation::whatsapp::supervisor_test::test_supervisor_lifecycle_and_client_registration ... ok
test conversation::whatsapp::supervisor_test::test_supervisor_get_connected_accounts ... ok
test conversation::whatsapp::supervisor_test::test_supervisor_sync_account_drains_outbox ... ok
test conversation::whatsapp::supervisor_test::test_supervisor_run_loop_stops_on_cancellation ... ok

test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; finished in 0.18s
```

### 3.2 Full Workspace Test Suite
```
test result: ok. 721 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 17.09s
```

---

## 4. Status
- **Status**: DONE
- **Next Task**: Task 6: WhatsApp Campaign Engine, Suppression List & Spintax
