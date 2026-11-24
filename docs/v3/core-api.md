# RS: `rustsale-core` API (P0b)

Status: written at the end of P0b (branch `feat/nsh-v3-core`). The code was **moved, not changed**:
every v2 command keeps its name, argument names, return type and behaviour. This file is the
contract that `native/` (the `#[native_module("rustsale/native")]` bridge, P1) and the TSX
stores build on. Sections 7-8 list what is *not* yet TSX-ready; those are P1 work, not P0b.

## 1. Layout

```
Cargo.toml                    workspace: members = [crates/rustsale-core, src-tauri]
                              exclude = [crates/zca-rs]  (see section 9, deviation 1)
crates/rustsale-core/         all v2 logic, no tauri (cargo tree: 0 tauri crates)
crates/zca-rs/                moved from src-tauri/vendor/zca-rs, source unchanged
src-tauri/                    TEMPORARY v2 shell: adapter only (host.rs, tray_ui.rs, wrappers/)
```

Core modules: `administrative_catalog, ai, campaign, commands, conversation (zalo, whatsapp,
facebook), db, events, facebook, google_maps_jobs, host, lead_importer, local_api, models,
notification, scrapers, tray`.

## 2. Boot: service hub

```rust
let core = Core::start(CoreConfig { data_dir, local_api_port: None }, ports, &tokio_handle)?;   // Result<Core, CoreError>
// local_api_port: Some(8797) overrides RUSTSALE_LOCAL_API_PORT (both v3 dev shells pass 8797; revert the Tauri one to None at cutover, §12)
let app: &AppHandle = core.handle();                                      // the "service hub"
```

`Core::start` does everything v2's Tauri `setup` did, in the same order: tray/notification
state, data dir, logging guard, SQLite open + migrations (+ debug seed), notification and tray
settings, administrative catalog, Google Maps job service + event forwarder + retention +
startup recovery, conversation service and supervisors (Zalo, Facebook, WhatsApp), local API
(axum, `127.0.0.1`, port from `RUSTSALE_LOCAL_API_PORT`), event forwarders, 4 notification
workers, outbox recovery, Zalo `start_all`, Facebook sync loop (60 s), WhatsApp run loop,
Facebook scheduler thread (30 s).

- `CoreError` (thiserror): `DataDir`, `NonUtf8Path`, `Logging`, `Database`, `Catalog`,
  `GoogleMaps`, `Thread`. v2 `expect(...)` calls on the startup path became these variants.
- The shell (Tauri adapter today, NSH app later) owns the window/tray and calls
  `apply_boot_window_policy(app, has_autostart_arg, tray_ready)` after `start`.
- `AppHandle` is a registry of managed states (`manage<T>`, `state<T>() -> Result<State<T>, HostError>`,
  `try_state<T>() -> Option`) plus the `Ports` bundle and `emit(event, payload)`.
  **Difference from Tauri:** `state::<T>()` returns `Result` (Tauri panicked when missing).
- Managed states exposed to commands: `DbState`, `ConversationState`, `GoogleMapsJobState`,
  `AdministrativeCatalogState`, `Arc<NotificationManager>`, `TrayState`, and the supervisors.

## 3. Ports (everything Tauri used to provide)

| Port | Methods | Used by core for | NSH implementation (P2) |
|---|---|---|---|
| `EventSink` | `emit(event: &str, payload: serde_json::Value)` | every event in section 6 | native module subscriptions |
| `DesktopNotifier` | `show(title, body)` | desktop notifications | NSH desktop notification capability |
| `FileDialog` | `pick_file(&PickFileRequest)`, `save_file(&SaveFileRequest)` (blocking) | lead import/export, save image, template download | NSH file dialog |
| `WindowPort` | `has_main_window`, `is_main_visible`, `show_main`, `hide_main`, `exit(code)` | tray actions, close-to-tray, boot policy | NSH window API |
| `TrayPort` | `init()` | tray creation after boot | NSH tray |
| `OpenerPort` | `open_url`, `reveal_path` | open links / show file | NSH opener |
| `UpdaterPort` (async) | `check() -> Option<UpdateInfo>`, `download_and_install()`, `restart()` | auto update | NSH updater |
| `AutostartPort` | `is_enabled()`, `set_enabled(bool)` | autostart toggle | NSH autostart |

`Ports` bundles the eight (`Arc<dyn ...>`), `Ports::noop()` / `NoopPorts` returns
`PortError::Unsupported` or does nothing (used by tests and headless runs). Errors:
`PortError { Unsupported, Failed(String) }`. Test doubles `MockApp`, `MockEvents`,
`MockNotifier` live behind `cfg(any(test, feature = "test-support"))`.
Streaming command arguments that were `tauri::ipc::Channel<T>` are `host::Channel<T>`
(`new(send_fn)`, `noop()`, `send(msg)`).

Tray menu actions are in `rustsale_core::tray::actions` (`open_app, mark_all_read, toggle_dnd,
quit, toggle_main_window, should_hide_on_close`); drawing the menu is the shell's job.

## 4. Command surface

177 commands (157 `String` errors, 18 typed `ConversationCommandError`, 2 infallible), all `pub fn` / `pub async fn` in core taking `State<'_, X>` (looked up from
`AppHandle`) and serde arguments, returning `Result<T, E>` with serde types. The Tauri
adapter in `src-tauri/src/wrappers/` is **generated** (`scripts/gen_tauri_wrappers.py`) and
only injects states; it is deleted at cutover. Argument names below are the JS names v2's
Svelte UI uses (verified 177/177 against the v2 sources at 2535767).

Conventions to keep when exporting to TSX:
- argument and return types are serde structs/enums already (camelCase where the v2 UI needed it);
- sync commands touch SQLite on the caller thread: the native module must run them on a
  worker (`spawn_blocking`), never on the UI thread;
- `Channel<T>` arguments: `execute_lead_import(on_progress)` and similar; export as a
  subscription/stream, not as a callback argument.

### 4.1 Full command list

#### `administrative_catalog_commands` (7)

| Command | async | Args (JS names) | Returns |
|---|---|---|---|
| `list_administrative_countries` | n | — | `Vec<AdministrativeCountry>` |
| `get_administrative_country_status` | y | `country_code: String` | `Result<AdministrativeCountryStatus, String>` |
| `load_administrative_country` | y | `country_code: String`, `force_refresh: bool` | `Result<AdministrativeCountryStatus, String>` |
| `list_administrative_areas` | y | `country_code: String`, `parent_id: Option<i64>`, `limit: usize` | `Result<Vec<AdministrativeAreaSummary>, String>` |
| `search_administrative_areas` | y | `country_code: String`, `query: String`, `limit: usize` | `Result<Vec<AdministrativeAreaSummary>, String>` |
| `resolve_administrative_bounds` | y | `geoname_id: i64` | `Result<AdministrativeBounds, String>` |
| `geocode_bounding_box` | y | `query: String`, `country_code: Option<String>` | `Result<Option<[f64; 4]>, String>` |

#### `ai_commands` (5)

| Command | async | Args (JS names) | Returns |
|---|---|---|---|
| `classify_customer_intent` | y | `text: String` | `Result<IntentResult, String>` |
| `generate_smart_reply_suggestions` | y | `lead_id: i32`, `conversation_id: Option<i64>`, `custom_prompt: Option<String>` | `Result<SmartReplyOutput, String>` |
| `get_pending_ai_proposals` | n | — | `Result<Vec<AiActionProposal>, String>` |
| `review_ai_proposal` | n | `proposal_id: i64`, `status: String`, `reviewer: String` | `Result<(), String>` |
| `get_lead_evidence_context` | n | `lead_id: i32` | `Result<LeadEvidenceContext, String>` |

#### `campaign_commands` (7 + v3-only `get_unified_campaign_steps`)

| Command | async | Args (JS names) | Returns |
|---|---|---|---|
| `start_unified_campaign` | n | `campaign_id: i64` | `Result<(), String>` |
| `pause_unified_campaign` | n | `campaign_id: i64` | `Result<(), String>` |
| `enroll_leads_into_campaign` | n | `campaign_id: i64`, `lead_ids: Vec<i32>` | `Result<usize, String>` |
| `get_unified_campaign_analytics` | n | `campaign_id: i64` | `Result<CampaignAnalytics, String>` |
| `get_unified_campaign_steps` | n | `campaign_id: i64` | `Result<Vec<CampaignStep>, String>` (v3-only; `Err` if the campaign does not exist) |
| `get_all_unified_campaigns` | n | — | `Result<Vec<Campaign>, String>` |
| `save_unified_campaign` | n | `campaign: Campaign`, `steps: Vec<CampaignStep>` | `Result<i64, String>` |
| `delete_unified_campaign` | n | `campaign_id: i64` | `Result<(), String>` |

#### `commands` (74)

| Command | async | Args (JS names) | Returns |
|---|---|---|---|
| `get_leads` | n | — | `Result<Vec<Lead>, String>` |
| `get_lead_details` | n | `id: i32` | `Result<Lead, String>` |
| `add_lead` | n | `name: String`, `company: String`, `status: String`, `value: f64`, `owner: String`, `email: String`, `phone: String`, `about: Option<String>`, `address: Option<String>`, `currency: Option<String>` | `Result<i32, String>` |
| `update_lead_status` | n | `id: i32`, `status: String` | `Result<(), String>` |
| `get_deals` | n | — | `Result<Vec<Deal>, String>` |
| `create_deal` | n | `lead_id: i32`, `title: String`, `value: f64`, `stage: String`, `currency: Option<String>` | `Result<Deal, String>` |
| `update_deal_stage` | n | `id: i32`, `stage: String` | `Result<(), String>` |
| `delete_deal` | n | `id: i32` | `Result<(), String>` |
| `get_activities` | n | `lead_id: i32` | `Result<Vec<Activity>, String>` |
| `get_recent_activities` | n | `limit: Option<i32>` | `Result<Vec<Activity>, String>` |
| `add_activity` | n | `lead_id: i32`, `activity_type: String`, `content: String`, `performed_by: String` | `Result<(), String>` |
| `update_lead` | n | `id: i32`, `name: String`, `company: String`, `status: String`, `value: f64`, `owner: String`, `email: String`, `phone: String`, `about: Option<String>`, `address: Option<String>`, `currency: Option<String>` | `Result<(), String>` |
| `delete_lead` | n | `id: i32` | `Result<(), String>` |
| `get_currency_settings` | n | — | `Result<CurrencySettings, String>` |
| `update_currency_settings` | n | `base_currency: String`, `exchange_rate: f64` | `Result<CurrencySettings, String>` |
| `start_google_maps_scrape` | y | `keyword: String`, `mode: String`, `location: Option<String>`, `bbox: Option<[f64; 4]>`, `step_km: Option<f64>`, `districts: Option<Vec<String>>`, `headless: Option<bool>` | `Result<(), String>` |
| `stop_google_maps_scrape` | y | — | `Result<(), String>` |
| `get_preset_provinces` | n | — | `Result<serde_json::Value, String>` |
| `plan_grid_crawl` | n | `bbox: [f64; 4]`, `step_km: f64` | `Result<GridPlan, String>` |
| `start_zalo_qr_login` | y | — | `Result<(), String>` |
| `get_managed_zalo_accounts` | n | — | `Result<Vec<ManagedZaloAccount>, String>` |
| `get_managed_smtp_accounts` | n | — | `Result<Vec<ManagedSmtpAccount>, String>` |
| `get_managed_facebook_accounts` | n | — | `Result<Vec<ManagedFacebookAccount>, String>` |
| `save_managed_facebook_account` | y | `request: SaveFacebookAccountRequest` | `Result<ManagedFacebookAccount, String>` |
| `fetch_facebook_pages_from_token` | y | `token: String`, `app_id: Option<String>` | `Result<Vec<DiscoveredFacebookPage>, String>` |
| `delete_facebook_account` | n | `page_id: String` | `Result<(), String>` |
| `delete_zalo_session` | y | `imei: String` | `Result<(), String>` |
| `verify_zalo_session` | y | `imei: String` | `Result<ZaloSessionStatus, String>` |
| `verify_all_zalo_sessions` | y | — | `Result<Vec<ZaloSessionStatus>, String>` |
| `get_outreach_templates` | n | — | `Result<Vec<OutreachTemplate>, String>` |
| `create_outreach_template` | n | `name: String`, `content: String`, `image_url: Option<String>` | `Result<i32, String>` |
| `update_outreach_template` | n | `id: i32`, `name: String`, `content: String`, `image_url: Option<String>` | `Result<(), String>` |
| `delete_outreach_template` | n | `id: i32` | `Result<(), String>` |
| `send_zalo_outreach` | y | `imei: String`, `lead_ids: Vec<i32>`, `template_id: i32` | `Result<(), String>` |
| `start_zalo_outreach` | y | `imeis: Vec<String>`, `template_id: i32`, `lead_ids: Vec<i32>`, `enable_burst_limit: Option<bool>`, `burst_max_messages: Option<u32>`, `burst_delay_seconds: Option<u64>`, `inter_message_delay_seconds: Option<u64>` | `Result<(), String>` |
| `get_categories` | n | — | `Result<Vec<Category>, String>` |
| `get_categories_for_lead` | n | `lead_id: i32` | `Result<Vec<Category>, String>` |
| `create_category` | n | `name: String` | `Result<i32, String>` |
| `delete_category` | n | `id: i32` | `Result<(), String>` |
| `add_leads_to_category` | n | `lead_ids: Vec<i32>`, `category_id: i32` | `Result<(), String>` |
| `remove_leads_from_category` | n | `lead_ids: Vec<i32>`, `category_id: i32` | `Result<(), String>` |
| `get_leads_by_category` | n | `category_id: i32` | `Result<Vec<Lead>, String>` |
| `send_zalo_campaign` | y | `category_id: i32`, `template_id: i32`, `session_imeis: Vec<String>` | `Result<(), String>` |
| `get_campaign_leads` | y | `category_id: i32`, `segmentation: String`, `filter_template_id: Option<i32>`, `channel: Option<String>` | `Result<Vec<Lead>, String>` |
| `get_outreach_leads_page` | n | `category_id: Option<i32>`, `segmentation: String`, `filter_template_id: Option<i32>`, `channel: String`, `search: Option<String>`, `filter_verified_zalo: bool`, `filter_unreached: bool`, `filter_mobile_only: bool`, `cursor: Option<i32>`, `limit: u32` | `Result<OutreachLeadsPage, String>` |
| `resolve_outreach_lead_ids` | n | `category_id: Option<i32>`, `segmentation: String`, `filter_template_id: Option<i32>`, `channel: String`, `search: Option<String>`, `filter_verified_zalo: bool`, `filter_unreached: bool`, `filter_mobile_only: bool`, `excluded_lead_ids: Vec<i32>` | `Result<Vec<i32>, String>` |
| `save_smtp_server` | n | `server: SmtpServer` | `Result<i32, String>` |
| `delete_smtp_server` | n | `id: i32` | `Result<(), String>` |
| `test_smtp_connection` | y | `host: String`, `port: i32` | `Result<String, String>` |
| `get_email_templates` | n | — | `Result<Vec<EmailTemplate>, String>` |
| `create_email_template` | n | `name: String`, `subject: String`, `content: String` | `Result<i32, String>` |
| `update_email_template` | n | `id: i32`, `name: String`, `subject: String`, `content: String` | `Result<(), String>` |
| `delete_email_template` | n | `id: i32` | `Result<(), String>` |
| `get_templates` | n | `filter: Option<rustsale_core::models::TemplateFilter>` | `Result<Vec<TemplateRecord>, String>` |
| `get_template_by_id` | n | `id: i64` | `Result<Option<TemplateRecord>, String>` |
| `create_template` | n | `input: rustsale_core::models::CreateTemplateInput` | `Result<i64, String>` |
| `update_template` | n | `input: rustsale_core::models::UpdateTemplateInput` | `Result<(), String>` |
| `delete_template` | n | `id: i64` | `Result<(), String>` |
| `get_template_folders` | n | — | `Result<Vec<TemplateFolderRecord>, String>` |
| `create_template_folder` | n | `name: String`, `color: Option<String>` | `Result<i64, String>` |
| `delete_template_folder` | n | `id: i64` | `Result<(), String>` |
| `increment_template_usage` | n | `id: i64` | `Result<(), String>` |
| `send_email_outreach` | y | `smtp_id: i32`, `template_id: i32`, `lead_ids: Vec<i32>` | `Result<(), String>` |
| `get_group_members_unrestricted` | n | `conversation_id: i64` | `Result<Vec<GroupMemberDetail>, String>` |
| `batch_import_group_members_to_leads` | n | `request: BatchImportGroupMembersRequest` | `Result<BatchImportGroupMembersResult, String>` |
| `sync_and_deep_scan_group_members` | y | `conversation_id: i64` | `Result<GroupMemberScanResult, String>` |
| `get_hardware_id` | n | — | `Result<HardwareInfo, String>` |
| `export_leads_csv_file` | y | `default_filename: String`, `content: String` | `Result<Option<String>, String>` |
| `pick_lead_import_file` | y | — | `Result<Option<String>, String>` |
| `inspect_lead_import_file` | y | `file_path: String`, `sheet_name: Option<String>` | `Result<ImportInspection, String>` |
| `execute_lead_import` | y | `options: rustsale_core::lead_importer::types::LeadImportOptions`, `on_progress: tauri::ipc::Channel<rustsale_core::lead_importer::types::ImportProgress>` | `Result<ImportSummaryResult, String>` |
| `download_lead_import_template` | y | `format: String` | `Result<Option<String>, String>` |
| `save_image_file` | y | `default_filename: Option<String>`, `url: Option<String>`, `base64_data: Option<String>` | `Result<Option<String>, String>` |
| `install_portable_linux_update` | y | `version: String` | `Result<bool, String>` |

#### `conversation_commands` (55)

| Command | async | Args (JS names) | Returns |
|---|---|---|---|
| `list_conversation_inbox` | n | — | `Result<Vec<InboxItem>, String>` |
| `mark_conversation_read` | n | `key: String` | `Result<u64, String>` |
| `get_contact_timeline` | n | `lead_id: i64`, `active_channels: Vec<Channel>`, `before: Option<TimelineCursor>`, `limit: u32` | `Result<TimelinePage, String>` |
| `get_conversation_timeline` | n | `conversation_id: i64`, `before: Option<TimelineCursor>`, `limit: u32` | `Result<TimelinePage, String>` |
| `get_identity_timeline` | n | `identity_id: i64`, `before: Option<TimelineCursor>`, `limit: u32` | `Result<TimelinePage, String>` |
| `link_conversation_identity` | n | `request: LinkIdentityRequest` | `Result<(), String>` |
| `unlink_conversation_identity` | n | `request: UnlinkIdentityRequest` | `Result<(), String>` |
| `unlink_conversation_lead` | n | `request: UnlinkLeadRequest` | `Result<(), String>` |
| `relink_conversation_lead` | n | `request: RelinkLeadRequest` | `Result<(), String>` |
| `send_conversation_message` | n | `request: SendConversationMessageRequest` | `Result<ConversationMessage, String>` |
| `get_conversation_runtime_status` | y | — | `Result<ConversationRuntimeStatus, String>` |
| `restart_conversation_account` | y | `provider_account_key: String` | `Result<(), String>` |
| `delete_conversation` | n | `request: DeleteConversationRequest` | `Result<(), String>` |
| `delete_conversations_batch` | n | `requests: Vec<DeleteConversationRequest>` | `Result<(), String>` |
| `start_direct_conversation` | n | `request: StartDirectConversationRequest` | `Result<ConversationMessage, String>` |
| `send_conversation_batch` | n | `request: SendConversationBatchRequest` | `Result<SendConversationBatchResult, ConversationCommandError>` |
| `stage_conversation_attachment_paths` | y | `conversation_id: i64`, `paths: Vec<String>` | `Result<Vec<StagedAttachmentResponse>, ConversationCommandError>` |
| `stage_conversation_clipboard_image` | y | `request: StageConversationClipboardImageRequest` | `Result<StagedAttachmentResponse, ConversationCommandError>` |
| `discard_conversation_staged_attachment` | y | `staged_attachment_id: String` | `Result<(), ConversationCommandError>` |
| `search_zalo_stickers` | y | `conversation_id: i64`, `query: String`, `limit: u32` | `Result< Vec<ZaloStickerReference>, ConversationCommandError, >` |
| `get_zalo_sticker_detail` | y | `conversation_id: i64`, `sticker_id: u32` | `Result< Option<ZaloStickerReference>, ConversationCommandError, >` |
| `recall_conversation_message` | y | `request: rustsale_core::conversation::types::RecallMessageRequest` | `Result<ConversationMessage, ConversationCommandError>` |
| `list_conversation_quick_templates` | n | `request: ListConversationQuickTemplatesRequest` | `Result<Vec<ConversationQuickTemplate>, ConversationCommandError>` |
| `create_conversation_quick_template` | n | `request: CreateConversationQuickTemplateRequest` | `Result<ConversationQuickTemplate, ConversationCommandError>` |
| `update_conversation_quick_template` | n | `request: UpdateConversationQuickTemplateRequest` | `Result<ConversationQuickTemplate, ConversationCommandError>` |
| `delete_conversation_quick_template` | n | `request: DeleteConversationQuickTemplateRequest` | `Result<(), ConversationCommandError>` |
| `mark_conversation_quick_template_used` | n | `request: MarkConversationQuickTemplateUsedRequest` | `Result<(), ConversationCommandError>` |
| `set_conversation_reaction` | y | `request: SetConversationReactionRequest` | `Result<ConversationReactionMutationResult, ConversationCommandError>` |
| `forward_conversation_message` | n | `request: ForwardConversationMessageRequest` | `Result<ForwardConversationMessageResult, ConversationCommandError>` |
| `get_conversation_forward_operation` | n | `client_operation_id: String` | `Result<ForwardConversationMessageResult, ConversationCommandError>` |
| `retry_conversation_forward_target` | n | `request: RetryConversationForwardTargetRequest` | `Result<ConversationForwardTargetResult, ConversationCommandError>` |
| `list_conversation_mention_candidates` | n | `request: ListConversationMentionCandidatesRequest` | `Result<Vec<ConversationMentionCandidate>, ConversationCommandError>` |
| `pin_conversation_message` | n | `request: PinMessageRequest` | `Result<PinnedMessage, String>` |
| `unpin_conversation_message` | n | `request: UnpinMessageRequest` | `Result<(), String>` |
| `list_conversation_pinned_messages` | n | `conversation_id: i64` | `Result<Vec<PinnedMessage>, String>` |
| `sync_facebook_conversations` | y | `page_id: Option<String>` | `Result<usize, String>` |
| `ingest_facebook_webhook_events` | n | `page_id: String`, `events: Vec<serde_json::Value>` | `Result<usize, String>` |
| `start_whatsapp_qr_login` | y | `attempt_id: String` | `Result<(), String>` |
| `cancel_whatsapp_qr_login` | n | `attempt_id: String` | `Result<(), String>` |
| `get_managed_whatsapp_accounts` | n | — | `Result<Vec<ConnectedWhatsAppAccount>, String>` |
| `disconnect_whatsapp_account` | y | `channel_account_id: i64` | `Result<(), String>` |
| `create_whatsapp_campaign` | n | `req: rustsale_core::conversation::whatsapp::campaign::CreateCampaignRequest` | `Result<i64, String>` |
| `populate_whatsapp_campaign_recipients` | n | `campaign_id: i64`, `recipients: Vec<rustsale_core::conversation::whatsapp::campaign::CampaignRecipientInput>`, `template: Option<String>` | `Result<usize, String>` |
| `start_whatsapp_campaign` | y | `campaign_id: i64` | `Result<(), String>` |
| `pause_whatsapp_campaign` | n | `campaign_id: i64` | `Result<(), String>` |
| `resume_whatsapp_campaign` | n | `campaign_id: i64` | `Result<(), String>` |
| `cancel_whatsapp_campaign` | n | `campaign_id: i64` | `Result<(), String>` |
| `get_whatsapp_campaign` | n | `campaign_id: i64` | `Result<WhatsAppCampaignRecord, String>` |
| `list_whatsapp_campaigns` | n | — | `Result<Vec<WhatsAppCampaignRecord>, String>` |
| `get_whatsapp_campaign_recipients` | n | `campaign_id: i64`, `limit: Option<u32>`, `offset: Option<u32>` | `Result<Vec<CampaignRecipientRecord>, String>` |
| `sync_zalo_group_chat_history` | y | `conversation_id: i64` | `Result<usize, String>` |
| `sync_whatsapp_conversations` | y | — | `Result<usize, String>` |
| `download_conversation_attachment` | y | `attachment_id: i64` | `Result<ConversationAttachment, ConversationCommandError>` |
| `resume_conversation_deltas` | n | `after_sequence: u64`, `limit: Option<usize>` | `Result<Vec<ConversationMessageDeltaEvent>, String>` |
| `get_conversation_high_water_mark` | n | — | `Result<ConversationHighWaterMark, String>` |

#### `facebook_commands` (10)

| Command | async | Args (JS names) | Returns |
|---|---|---|---|
| `schedule_facebook_post` | n | `request: CreateFacebookPostRequest` | `Result<i64, String>` |
| `publish_facebook_post_now` | n | `post_id: i64` | `Result<bool, String>` |
| `list_facebook_scheduled_posts` | n | `page_id: Option<String>` | `Result<Vec<FacebookScheduledPostDto>, String>` |
| `cancel_facebook_scheduled_post` | n | `post_id: i64` | `Result<bool, String>` |
| `create_comment_automation` | n | `req: CreateCommentAutomationRequest` | `Result<i64, String>` |
| `list_comment_automations` | n | `page_id: Option<String>` | `Result<Vec<CommentAutomationDto>, String>` |
| `update_comment_automation` | n | `req: UpdateCommentAutomationRequest` | `Result<bool, String>` |
| `delete_comment_automation` | n | `id: i64` | `Result<bool, String>` |
| `list_comment_logs` | n | `page_id: Option<String>`, `limit: Option<i64>` | `Result<Vec<CommentLogDto>, String>` |
| `get_facebook_page_health` | n | `page_id: String` | `Result<PageHealthDto, String>` |

#### `google_maps_jobs_commands` (11)

| Command | async | Args (JS names) | Returns |
|---|---|---|---|
| `get_google_maps_runtime_status` | y | — | `Result<RuntimeStatus, String>` |
| `prepare_google_maps_runtime` | y | — | `Result<RuntimeStatus, String>` |
| `create_google_maps_scrape_job` | y | `request: CreateScrapeJobRequest` | `Result<ScrapeJob, String>` |
| `pause_google_maps_scrape_job` | y | `job_id: String` | `Result<ScrapeJob, String>` |
| `resume_google_maps_scrape_job` | y | `job_id: String` | `Result<ScrapeJob, String>` |
| `cancel_google_maps_scrape_job` | y | `job_id: String` | `Result<ScrapeJob, String>` |
| `retry_google_maps_failed_units` | y | `job_id: String` | `Result<ScrapeJob, String>` |
| `retry_google_maps_email_enrichment` | y | `job_id: String` | `Result<ScrapeJob, String>` |
| `get_google_maps_scrape_job` | y | `job_id: String` | `Result<ScrapeJob, String>` |
| `list_google_maps_scrape_jobs` | y | — | `Result<Vec<ScrapeJob>, String>` |
| `delete_google_maps_scrape_job` | y | `job_id: String` | `Result<bool, String>` |

#### `notification` (4)

| Command | async | Args (JS names) | Returns |
|---|---|---|---|
| `get_notification_settings` | n | — | `Result<NotificationSettings, String>` |
| `update_notification_settings` | n | `settings: NotificationSettings` | `Result<(), String>` |
| `set_active_conversation_focus` | n | `active_conversation_id: Option<i64>`, `is_focused: bool`, `seq: Option<u64>` | `Result<FocusSyncResult, String>` |
| `test_desktop_notification` | y | — | `Result<(), String>` |

#### `root` (1)

| Command | async | Args (JS names) | Returns |
|---|---|---|---|
| `greet` | n | `name: &str` | `String` |

#### `tray` (3)

| Command | async | Args (JS names) | Returns |
|---|---|---|---|
| `get_tray_state` | n | — | `Result<TrayConfig, String>` |
| `set_close_to_tray` | n | `enabled: bool` | `Result<(), String>` |
| `set_tray_dnd` | n | `enabled: bool` | `Result<(), String>` |

## 5. Commands that are hard to export as-is

| Command(s) | Why | Plan for `native/` |
|---|---|---|
| `export_leads_csv_file`, `pick_lead_import_file`, `download_lead_import_template`, `save_image_file` | open a **blocking** native dialog through `FileDialog` | call on a blocking worker; the dialog is shown by the NSH port |
| `execute_lead_import` | streams progress through `Channel` and also emits `lead-import-progress` | export as async stream |
| `start_zalo_qr_login`, `start_whatsapp_qr_login` | long running; progress arrives only as events | return after start; QR/state through subscriptions |
| `start_zalo_outreach`, `send_*_outreach`, `send_zalo_campaign`, `start_google_maps_scrape` | long running jobs, progress only via events | same |
| `send_zalo_campaign`, `get_campaign_leads`, `start_google_maps_scrape`, `stop_google_maps_scrape`, `verify_all_zalo_sessions` | legacy, no v2 UI | kept in core, **not exported** to TSX (see ARCHITECTURE.md queue) |
| `ConversationCommandError` commands (conversation module) | already typed errors | see section 7 |

## 6. Events (core -> UI)

All events go through `EventSink::emit(name, serde_json::Value)`. In TSX each one becomes a
subscription on the native module. Payloads are serde types in core.

| Event | Payload | Source |
|---|---|---|
| `conversation-invalidated` | `ConversationInvalidatedEvent { conversation_id, lead_id, channel, reason, message_id }`; on broadcast lag `{ reason: "ReconcileRequired", skipped }`. Also emitted by `mark_conversation_read` (one per conversation whose unread count changed, `reason: "metadata"`, `message_id: null`; none when no message changed). The UI listens to this name, not to `conversation-updated`, which does not exist | conversation/events.rs |
| `conversation-message-delta` | `ConversationMessageDeltaEvent { sequence, conversation_id, lead_id, channel, message, emitted_at_ms }` | conversation/events.rs |
| `conversation-delta-lagged` | `{ skipped }` -> UI must call `resume_conversation_deltas(after_sequence, limit)` | conversation/events.rs |
| `google-maps-scrape-job-event` | scrape job event (`google_maps_jobs::types`) | google_maps_jobs/events.rs |
| `zalo-qr-code` / `zalo-qr-scanned` / `zalo-qr-expired` / `zalo-qr-declined` / `zalo-qr-success` / `zalo-qr-error` | `{ image_base64, token, code }` / `{ avatar, display_name }` / null / null / `{ imei }` / `{ message }` | commands.rs |
| `whatsapp-qr-code` / `whatsapp-login-success` / `whatsapp-login-error` | `{ attempt_id, qr_raw, expires_in_sec, sequence }` / `{ attempt_id, account_id, account_key, phone_number, push_name }` / `{ attempt_id, error }` | conversation/whatsapp/login.rs |
| `outreach-progress` / `outreach-log` / `outreach-complete` | `{ progress, status, message }` / `{ message }` / `{ total_sent, message }` | Zalo outreach |
| `email-outreach-progress` / `-log` / `-complete` | same shapes | email outreach |
| `campaign-progress` / `campaign-log` / `campaign-complete` | same shapes (`total_sent: i32`) | legacy campaign |
| `lead-import-progress` | import progress struct | lead_importer |
| `desktop-notification-sent` | `{ test, sound }` | notification.rs |
| `tray-mark-all-read` | `null` | tray |
| `tray-dnd-changed` | `bool` | tray |

Delivery rule to keep: events are best effort (`emit` errors are logged, never fatal). Anything
the UI must not miss is recovered by a command: `resume_conversation_deltas`,
`get_conversation_high_water_mark`, `get_google_maps_scrape_job`.

## 7. Errors: current state and plan for stable codes

Current (v2, unchanged): 157 commands return `Result<T, String>`, 2 are infallible (`list_administrative_countries`, `greet`); the 18 conversation commands
that use `ConversationCommandError` return `{ code, message }` with `code` a snake_case
`ConversationErrorCode` (`capability_denied, account_disconnected, stage_expired,
provider_auth, provider_rate_limited, delivery_unknown, invalid_request, provider_failure,
forward_*`, `reaction_*`, `template_*`, `mention_*`, ...).

Plan (done in `native/` in P1, so P0b stays behaviour-neutral): the bridge exposes one error
type `CoreApiError { code: String, message: String }` to TSX.
- `ConversationCommandError` -> its existing `code`.
- `String` errors -> `code = "internal"` by default, mapped to `invalid_request`, `not_found`,
  `unavailable` by a small prefix/keyword table that lives in `native/` and has tests;
  new core code returns typed errors (thiserror) instead of `String`, module by module, as
  screens are ported.
- UI shows `message` only for display; logic branches on `code`.

## 8. Large lists: paging that exists vs gaps

> **Status (P1, commit 50f5b95):** every gap in the right-hand column is closed by a NEW command; the v2 commands stay for the Tauri adapter. See section 14.

TSX must never receive a whole large list. Sorting/filtering stays in Rust.

| Data | Exists today | Gap (P1 core work) |
|---|---|---|
| Messages | `get_conversation_timeline / get_contact_timeline / get_identity_timeline(before: TimelineCursor, limit)` -> `TimelinePage { items, next_cursor, snapshot_sequence }`. Keyset paging backwards in time: a page is the newest `limit` messages older than `before`, **listed oldest first** (last item = newest). `next_cursor` is the oldest message of a **full** page (`{ occurred_at, message_id }` in core, `{ occurredAt, messageId }` in JS) and goes back as `before`; it is `null` when the page is short (nothing older). Older pages are prepended. v2 relies on this order; do not reverse it in the UI | none for reading; needs "jump to message" window |
| Conversation inbox | `list_conversation_inbox` returns **all** `InboxItem` | cursor/limit + filter/sort (unread, channel, search) |
| Leads | `get_outreach_leads_page(category, segmentation, channel, search, filters...)` -> `OutreachLeadsPage` (paged, filtered in SQL); `get_leads`, `get_leads_by_category` return **all** rows | `get_leads_page(filter, sort, cursor/limit)` for the leads table (4.6k rows target < 100 ms) |
| Google Maps places (scrape results) | **no** UI-facing command; repository has `list_results_for_import / list_results_for_enrichment` (whole job) | `list_google_maps_results(job_id, filter, sort, cursor, limit)` + total counts |
| Google Maps jobs | `list_google_maps_scrape_jobs` returns all (small) | ok |
| Logs | `list_comment_logs(page_id, limit)`, `get_recent_activities(limit)` (limit only); outreach/campaign logs are events only | cursor paging for comment/activity logs; persisted campaign log with cursor if the UI shows history |
| WhatsApp campaign recipients | `get_whatsapp_campaign_recipients(campaign_id, limit, offset)` | ok (offset; switch to cursor if large) |
| Administrative areas | `list_/search_administrative_areas(..., limit)` | ok |

## 9. Deviations from the brief (need Nakroth's approval)

1. **`crates/zca-rs` is excluded from the workspace** (`exclude`), not a member. Its `lib.rs`
   has `#![deny(clippy::unwrap_used, expect_used, panic, indexing_slicing, ...)]` which the
   vendored code itself violates (v2 never linted it: it was a path dependency). Making it a
   member would force edits to a crate that must stay byte-identical. As a result
   `cargo clippy/test --workspace` skip it; its own tests run with
   `cargo test --manifest-path crates/zca-rs/Cargo.toml` (31 + 1 passed). It is still built as a
   dependency of core.
2. `cargo fmt --all` would reformat 3 zca-rs files; use plain `cargo fmt` (workspace members only).
3. 751 MB of build artifacts tracked by git under `src-tauri/vendor/zca-rs/target/` were
   removed from the index (`git rm`); `crates/zca-rs/target/` is now ignored.
4. Two tests that read `docs/superpowers/...` use `../../docs/...` because the crate moved
   two levels down. `scrapers/browser.rs` also looks in `../../src-tauri/binaries` for the sidecar.
5. `AppHandle::state::<T>()` returns `Result` (v2/Tauri panicked); startup `expect`s are `CoreError`.
6. Not run by hand (no GUI session in this task): tray, native dialogs, updater, close-to-tray,
   autostart through the Tauri adapter. They compile and are wired to the same core functions;
   Alice's QA must cover them on v2-style `cargo tauri dev` (the shell binds 8797 by itself, no env
   var needed) and the `com.rustsale.crm.v3-dev` data dir.

## 10. Exporting to TSX (`#[native_module("rustsale/native")]`)

- One module per domain (`leads`, `conversations`, `maps`, `campaigns`, `settings`, ...), each
  method = one core command above; `async` methods return a promise; state lookups stay in
  Rust (the module owns `Core`).
- Events: one subscription per event name in section 6; the bridge converts
  `serde_json::Value` into the typed TSX object once, on the Rust side.
- Lists: only the paged/windowed APIs (section 8) are exposed to `List`; add the missing ones
  in core first.
- Errors: section 7. Blocking commands: run on `spawn_blocking` inside the bridge.
- Google Maps scraper: all 11 commands, `scrapers/*`, `cloak-runtime` sidecar and their tests are
  moved untouched (149 scraper/maps tests identical to v2). Bug fixes from
  `docs/v3/scraper-audit.md` are P1-S, not part of this move.

## 11. Verification (P0b)

See the report to Nakroth: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`,
`cargo test --workspace` (1021 + 4 + 1 passed, 4 ignored: same test set as v2), `cargo tree -p
rustsale-core | grep -i tauri` empty.

Fixed during the P1 parity pass: `list_comment_logs_impl` built `LIMIT ?2` even when the optional
page filter was absent, so the single bound parameter left `?1` unset and SQLite rejected the
query. Without a page filter it now uses `LIMIT ?1`; `NativeRest.test.tsx` calls
`Facebook.commentLogs(null, 10)` to cover it.

## 12. Dev identity and cutover checklist (RS P0c)

v3 must never share state with the production v2 app while both exist on one machine. Until
cutover the build identifies itself as a separate app:

| Item | v3 dev value | Revert at cutover |
| --- | --- | --- |
| `tauri.conf.json` `identifier` | `com.rustsale.crm.v3-dev` (data dir, D-Bus name, single-instance lock) | `com.rustsale.crm` |
| `productName` | `RustSale CRM v3 dev` (autostart `.desktop`, bundle names) | `RustSale CRM` |
| `plugins.updater.endpoints` | `https://updates.v3-dev.invalid/latest.json` (never resolves; a dev build cannot self-update to a production release) | the real `latest.json` URL |
| OS keyring service and vault lock dir (`facebook/vault.rs`) | `rustsale-v3-dev` | `rustsale`, so migrated Facebook tokens still decrypt |
| Local API port (`src-tauri/src/lib.rs` `DEV_LOCAL_API_PORT`, passed as `CoreConfig.local_api_port`; wins over `RUSTSALE_LOCAL_API_PORT` and `.env`) | `8797` (production v2 owns 8787, so a forgotten env var cannot collide) | `None`, so the core falls back to `RUSTSALE_LOCAL_API_PORT`, else 8787 |
| Zalo webhook allowlist default | `<data_dir>/zalo-webhook-allowlist.csv` only; no `HOME` fallback, so no data dir means nobody is reported | unchanged |

Test-only helpers that open a database refuse any path under `~/.local/share/com.rustsale.crm`
(kept on purpose: it is a guard, not a default).

Cutover blockers that live outside the core:

- `scrapers/browser.rs` still looks for the `cloak-runtime` sidecar in `../../src-tauri/binaries`.
  That path must change **before** `src-tauri` is deleted at cutover, or the scraper loses its
  browser.
- The Linux self-update installer in `commands.rs` writes `~/.local/bin/rustsale-crm`. The bin name
  is still the production one; rename it together with the identifier if v3 dev is ever installed
  as a package.
- `MockWhatsAppClient` is `#[cfg(test)]` only (module `whatsapp::client::mock_client`).

## 13. Facebook Messenger sync: retry pacing

`facebook/backoff.rs` paces polls per channel account: first retry after 30 s, doubling to a 15 min
cap; throttled pages (HTTP 429 or Graph codes 4/17/32/613) wait at least 5 min. Auth failures still
mark the account `invalid` (it leaves the connected list until reconnected). The UI gets an
`AccountStatus` invalidation once when an account turns unhealthy and once when it recovers;
`channel_accounts.last_error` holds the text, including storage failures. The "sync now" command
bypasses the pacing. Only the first failure of a streak, a changed error, or every 20th repeat is
logged at `error`; the rest are `debug`.

## 14. P1 status: paging and the native module (`feat/nsh-v3-native`)

**Paging (core, new commands, v2 ones untouched).** Keyset cursors: an opaque JSON string tagged
with sort and direction, a unique tie-break key so the order is total. A page is
`{ items, total, nextCursor }` (camelCase). `limit` 0 means 50, max 200. Errors are
`PageError { InvalidCursor, InvalidRequest, Database, Internal }`; a cursor made for another sort
is rejected, never silently wrong.

| New command | Replaces for the UI | TSX export |
|---|---|---|
| `get_leads_page`, `get_leads_by_category_page` | `get_leads`, `get_leads_by_category` | `Leads.page`, `Leads.byCategoryPage` |
| `list_conversation_inbox_page` | `list_conversation_inbox` | `Conversations.inboxPage` |
| `list_google_maps_results_page` | (none, new) | `GoogleMaps.resultsPage` |
| `get_activities_page` | `get_activities` | `Leads.activitiesPage` |
| `list_comment_logs_page` | `list_comment_logs` | not needed for parity; `Facebook.commentLogs` wraps the v2 command |

**Native crate `native/`** (`nsh-app-rustsale`, NSH 0.6.1 mixed mode, own `[workspace]`;
`import { ... } from "rustsale/native"`). Modules: `Host`, `Leads`, `LeadsMore`, `Deals`,
`Categories`, `Conversations`, `ConversationActions`, `GoogleMaps`, `Templates`, `Settings`,
`Campaigns`, `Zalo`, `Facebook`, `Ai`, `ImportLeads`, `Util`, `Desktop`, `GoogleMapsMore` and
`ConversationNetwork`. Status per command is in `feature-parity.md` section 2 (column
"Trạng thái v3"): 171 of 177 exported, 5 legacy commands deliberately not exported, 1 via a port
(updater), 0 pending. Commands that need a network or a real channel account are wrapped; the
headless tests call them only with missing ids or invalid input, or not at all, and the parity
file marks the ones no test exercises ("bọc, chưa gọi thử").

- **Errors:** every export returns `Result<_, CoreApiError { code, message }>`; `ConversationCommandError`
  keeps its code, `PageError` maps to `invalid_cursor` / `invalid_request` / `internal`, `String`
  errors sorted by a small keyword table (tested).
- **Boot:** `Host.start(dataDir?)` starts the core with real NSH ports. It refuses any path with a
  `com.rustsale.crm` component or `..`, refuses a configured Zalo webhook URL, uses the
  `com.rustsale.crm.v3-dev` folder by default and local API port 8797.
- **Ports -> NSH** (core port -> `Request` on a tokio channel -> a pump on the engine thread):
  events -> `Host.onEvent`; notifications -> `show_notification`; dialogs -> `pick_paths` /
  `pick_save_path`; opener -> `open_url` / `reveal_path`; tray -> `tray()` + menu; window ->
  show/hide/focus/`quit`; autostart -> `launch_at_login` / `set_launch_at_login`.
- **Updater** follows the packaged-app API `nsh/updater` (release builds use the native target of
  `nsh package`, gpui is dev only): the core asks, TSX answers through `Host.onUpdaterRequest` /
  `Host.answerUpdater`. Without a TSX listener the port returns `Unsupported`.
- **Not real noops, but degraded:** autostart returns `Unsupported` outside a packaged app; the
  window exit code is dropped (NSH `quit` has none); core log files are not written because the
  host's logger is inherited (`init_structured_logging_or_inherit`); tray, notifications and
  dialogs need a desktop host (headless tests only prove the plumbing).
- **Wire naming:** v2 models (Lead, CurrencySettings, TrayConfig, NotificationSettings,
  templates, SMTP) are snake_case; conversation types, paging queries and `Page` are camelCase.
  Results are `unknown` in the generated TS (`JsonValue`), so TSX casts to its own types.
- **Not exported on purpose:** `send_zalo_campaign`, `get_campaign_leads`,
  `start_google_maps_scrape`, `stop_google_maps_scrape`, `verify_all_zalo_sessions`. A Rust test
  (`legacy_commands_are_not_exported_to_tsx`) checks the generated declarations, and another
  (`every_export_has_a_doc_comment`) checks `registry().undocumented()`.
- **Build gotcha:** NSH re-resolves its own generated workspace under `native/.nsh/rust`;
  `zip` is pinned to `=2.4.2` in `native/Cargo.toml` because calamine 0.26 does not build with 2.6.
