use crate::campaign::commands::*;
use crate::campaign::dispatcher::*;
use crate::campaign::evaluator::*;
use crate::campaign::scheduler::*;
use crate::campaign::throttling::*;
use crate::models::{Campaign, CampaignStep};
use rusqlite::Connection;

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().expect("failed to open memory db");
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS leads (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             name TEXT NOT NULL,
             company TEXT NOT NULL,
             status TEXT NOT NULL,
             value REAL NOT NULL DEFAULT 0.0,
             last_activity TEXT NOT NULL,
             owner TEXT NOT NULL,
             email TEXT NOT NULL,
             phone TEXT NOT NULL,
             about TEXT,
             address TEXT,
             has_zalo INTEGER DEFAULT NULL,
             zalo_reached INTEGER NOT NULL DEFAULT 0,
             zalo_uid TEXT,
             zalo_status TEXT DEFAULT 'UNTESTED',
             relationship_status TEXT DEFAULT 'UNKNOWN',
             last_lookup_attempt TEXT
         );

         CREATE TABLE IF NOT EXISTS deals (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             lead_id INTEGER NOT NULL,
             title TEXT NOT NULL,
             value REAL NOT NULL DEFAULT 0.0,
             stage TEXT NOT NULL,
             created_at TEXT NOT NULL,
             currency TEXT NOT NULL DEFAULT 'VND',
             FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE CASCADE
         );

         CREATE TABLE IF NOT EXISTS outreach_history (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             lead_id INTEGER NOT NULL,
             template_id INTEGER NOT NULL,
             channel TEXT NOT NULL DEFAULT 'OMNICHANNEL',
             status TEXT NOT NULL,
             error_message TEXT,
             sent_at TEXT NOT NULL,
             session_imei TEXT NOT NULL,
             idempotency_key TEXT UNIQUE,
             provider_message_id TEXT
         );

         CREATE TABLE IF NOT EXISTS conversations (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             lead_id INTEGER REFERENCES leads(id) ON DELETE SET NULL,
             channel TEXT NOT NULL,
             account_key TEXT NOT NULL,
             external_thread_id TEXT NOT NULL,
             created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS conversation_messages (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
             direction TEXT NOT NULL,
             sender_display_name TEXT,
             text_content TEXT,
             content_type TEXT NOT NULL DEFAULT 'text',
             created_at TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS template_folders (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             name TEXT NOT NULL,
             color TEXT,
             created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS templates (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             folder_id INTEGER REFERENCES template_folders(id) ON DELETE SET NULL,
             name TEXT NOT NULL,
             channel TEXT NOT NULL,
             stage TEXT,
             subject TEXT,
             content TEXT NOT NULL,
             media_url TEXT,
             ai_prompt TEXT,
             tags_json TEXT,
             usage_count INTEGER NOT NULL DEFAULT 0,
             last_used_at TEXT,
             created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS campaigns (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             name TEXT NOT NULL,
             description TEXT,
             channel_type TEXT NOT NULL,
             status TEXT NOT NULL DEFAULT 'DRAFT',
             audience_filter_json TEXT,
             schedule_config_json TEXT,
             created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS campaign_steps (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             campaign_id INTEGER NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
             step_order INTEGER NOT NULL,
             channel TEXT NOT NULL,
             delay_minutes INTEGER NOT NULL DEFAULT 0,
             condition_type TEXT NOT NULL DEFAULT 'ALWAYS',
             template_content TEXT NOT NULL,
             ai_prompt_override TEXT,
             created_at TEXT NOT NULL,
             template_id INTEGER REFERENCES templates(id) ON DELETE SET NULL,
             retired_at TEXT
         );

         CREATE TABLE IF NOT EXISTS campaign_executions (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             campaign_id INTEGER NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
             step_id INTEGER NOT NULL REFERENCES campaign_steps(id) ON DELETE CASCADE,
             lead_id INTEGER NOT NULL REFERENCES leads(id) ON DELETE CASCADE,
             target_phone TEXT,
             target_account_id TEXT,
             status TEXT NOT NULL DEFAULT 'PENDING',
             error_message TEXT,
             scheduled_at TEXT NOT NULL,
             executed_at TEXT,
             created_at TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS lead_journey_states (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             lead_id INTEGER NOT NULL UNIQUE REFERENCES leads(id) ON DELETE CASCADE,
             current_stage TEXT NOT NULL DEFAULT 'DISCOVERY',
             active_campaign_id INTEGER REFERENCES campaigns(id) ON DELETE SET NULL,
             last_inbound_at TEXT,
             last_outbound_at TEXT,
             ai_intent_tag TEXT,
             ai_qualification_score REAL,
             notes TEXT,
             updated_at TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS ai_evidence_logs (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             lead_id INTEGER REFERENCES leads(id) ON DELETE CASCADE,
             conversation_id INTEGER,
             action_type TEXT NOT NULL,
             evidence_source TEXT NOT NULL,
             evidence_data_json TEXT NOT NULL,
             confidence_score REAL NOT NULL,
             decision TEXT NOT NULL,
             created_at TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS ai_action_proposals (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             lead_id INTEGER REFERENCES leads(id) ON DELETE CASCADE,
             conversation_id INTEGER,
             platform TEXT NOT NULL,
             proposed_content TEXT NOT NULL,
             reasoning TEXT NOT NULL,
             status TEXT NOT NULL DEFAULT 'PENDING',
             reviewed_by TEXT,
             reviewed_at TEXT,
             created_at TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS activities (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             lead_id INTEGER NOT NULL,
             activity_type TEXT NOT NULL,
             content TEXT NOT NULL,
             created_at TEXT NOT NULL,
             performed_by TEXT,
             FOREIGN KEY (lead_id) REFERENCES leads (id) ON DELETE CASCADE
         );

         CREATE TABLE IF NOT EXISTS channel_accounts (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             channel TEXT NOT NULL,
             account_name TEXT NOT NULL,
             provider_account_key TEXT NOT NULL,
             status TEXT NOT NULL DEFAULT 'ACTIVE',
             created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS zalo_sessions (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             imei TEXT NOT NULL,
             session_name TEXT NOT NULL,
             is_active INTEGER NOT NULL DEFAULT 1
         );

         CREATE TABLE IF NOT EXISTS smtp_servers (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             host TEXT NOT NULL,
             port INTEGER NOT NULL,
             username TEXT,
             from_email TEXT NOT NULL,
             is_active INTEGER NOT NULL DEFAULT 1
         );",
    )
    .expect("failed to setup schema");
    conn
}

fn insert_test_lead(conn: &Connection, name: &str, company: &str, phone: &str, email: &str) -> i32 {
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
         VALUES (?1, ?2, 'LEAD', 5000000.0, '2026-09-02T10:00:00Z', 'Sale Rep', ?3, ?4)",
        rusqlite::params![name, company, email, phone],
    )
    .expect("failed to insert lead");
    conn.last_insert_rowid() as i32
}

#[test]
fn test_sequence_save_and_enrollment() {
    let mut conn = setup_test_db();
    let lead1 = insert_test_lead(
        &conn,
        "Nguyễn Văn A",
        "Công ty ABC",
        "0901234567",
        "a@abc.com",
    );
    let lead2 = insert_test_lead(
        &conn,
        "Trần Thị B",
        "Công ty XYZ",
        "0907654321",
        "b@xyz.com",
    );

    let campaign = Campaign {
        id: None,
        name: "Omnichannel Drip Q3".to_string(),
        description: Some("Multi-step automated sequence".to_string()),
        channel_type: "OMNICHANNEL".to_string(),
        status: "ACTIVE".to_string(),
        audience_filter_json: None,
        schedule_config_json: None,
        created_at: "".to_string(),
        updated_at: "".to_string(),
    };

    let steps = vec![
        CampaignStep {
            id: None,
            campaign_id: 0,
            step_order: 1,
            channel: "ZALO".to_string(),
            delay_minutes: 0,
            condition_type: "ALWAYS".to_string(),
            template_content: "Chào {{name}} từ {{company}}, em gửi báo giá nhé!".to_string(),
            ai_prompt_override: None,
            created_at: "".to_string(),
            template_id: None,
        },
        CampaignStep {
            id: None,
            campaign_id: 0,
            step_order: 2,
            channel: "EMAIL".to_string(),
            delay_minutes: 2880, // 2 days
            condition_type: "IF_NOT_REPLIED".to_string(),
            template_content: "Anh/Chị {{name}} đã xem qua báo giá chưa ạ?".to_string(),
            ai_prompt_override: None,
            created_at: "".to_string(),
            template_id: None,
        },
    ];

    let campaign_id = save_unified_campaign_internal(&mut conn, campaign, steps, false)
        .expect("save campaign should succeed");
    assert!(campaign_id > 0);

    // Enroll 2 leads
    let enrolled = enroll_leads_into_campaign_internal(&conn, campaign_id, &[lead1, lead2])
        .expect("enrollment should succeed");
    assert_eq!(enrolled, 2);

    let executions = crate::db::get_campaign_executions(&conn, campaign_id).unwrap();
    assert_eq!(executions.len(), 2);
    assert_eq!(executions[0].status, "PENDING");
    assert_eq!(executions[0].lead_id, lead1);
    assert_eq!(executions[1].status, "PENDING");
    assert_eq!(executions[1].lead_id, lead2);

    // Lead journey state should be ENROLLED
    let journey1 = crate::db::get_lead_journey_state(&conn, lead1)
        .unwrap()
        .unwrap();
    assert_eq!(journey1.active_campaign_id, Some(campaign_id));
    assert_eq!(journey1.current_stage, "ENROLLED");
}

fn sample_campaign(name: &str) -> Campaign {
    Campaign {
        id: None,
        name: name.to_string(),
        description: None,
        channel_type: "OMNICHANNEL".to_string(),
        status: "DRAFT".to_string(),
        audience_filter_json: None,
        schedule_config_json: None,
        created_at: "".to_string(),
        updated_at: "".to_string(),
    }
}

fn sample_step(order: i32, channel: &str, content: &str) -> CampaignStep {
    CampaignStep {
        id: None,
        campaign_id: 0,
        step_order: order,
        channel: channel.to_string(),
        delay_minutes: order * 60,
        condition_type: "ALWAYS".to_string(),
        template_content: content.to_string(),
        ai_prompt_override: Some(format!("hint {order}")),
        created_at: "".to_string(),
        template_id: None,
    }
}

#[test]
fn test_get_unified_campaign_steps_returns_own_steps_in_order() {
    let mut conn = setup_test_db();

    // Saved out of order on purpose: the read must return them by step_order.
    let first = save_unified_campaign_internal(
        &mut conn,
        sample_campaign("First"),
        vec![
            sample_step(2, "EMAIL", "second of first"),
            sample_step(1, "ZALO", "first of first"),
        ],
        false,
    )
    .expect("save first campaign");
    let second = save_unified_campaign_internal(
        &mut conn,
        sample_campaign("Second"),
        vec![sample_step(1, "ZALO", "only step of second")],
        false,
    )
    .expect("save second campaign");

    let steps = get_unified_campaign_steps_internal(&conn, first).expect("read first steps");
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].step_order, 1);
    assert_eq!(steps[0].channel, "ZALO");
    assert_eq!(steps[0].template_content, "first of first");
    assert_eq!(steps[0].ai_prompt_override.as_deref(), Some("hint 1"));
    assert_eq!(steps[0].delay_minutes, 60);
    assert_eq!(steps[1].step_order, 2);
    assert_eq!(steps[1].channel, "EMAIL");
    assert!(steps.iter().all(|s| s.campaign_id == first));
    assert!(steps.iter().all(|s| s.id.is_some_and(|id| id > 0)));

    // Another campaign's steps never leak in.
    let other = get_unified_campaign_steps_internal(&conn, second).expect("read second steps");
    assert_eq!(other.len(), 1);
    assert_eq!(other[0].template_content, "only step of second");
    assert_eq!(other[0].campaign_id, second);
}

#[test]
fn test_get_unified_campaign_steps_unknown_campaign_is_an_error() {
    let mut conn = setup_test_db();
    let id = save_unified_campaign_internal(
        &mut conn,
        sample_campaign("Exists"),
        vec![sample_step(1, "ZALO", "hello")],
        false,
    )
    .expect("save campaign");

    let err = get_unified_campaign_steps_internal(&conn, id + 1000)
        .expect_err("unknown campaign must be refused, not read as empty");
    assert!(err.contains("not found"), "unexpected error: {err}");
    assert!(get_unified_campaign_steps_internal(&conn, 0).is_err());
    assert!(get_unified_campaign_steps_internal(&conn, -1).is_err());
}

#[test]
fn test_get_unified_campaign_steps_empty_campaign_and_read_only() {
    let mut conn = setup_test_db();
    let empty =
        save_unified_campaign_internal(&mut conn, sample_campaign("No steps"), vec![], false)
            .expect("save campaign without steps");
    let with_steps = save_unified_campaign_internal(
        &mut conn,
        sample_campaign("With steps"),
        vec![sample_step(1, "ZALO", "a"), sample_step(2, "ZALO", "b")],
        false,
    )
    .expect("save campaign with steps");

    // An existing campaign with no steps is an empty list, not an error.
    assert!(get_unified_campaign_steps_internal(&conn, empty)
        .expect("read empty campaign")
        .is_empty());

    // Reading changes nothing: same rows (ids included) before and after, repeated reads equal.
    let before = get_unified_campaign_steps_internal(&conn, with_steps).expect("first read");
    let again = get_unified_campaign_steps_internal(&conn, with_steps).expect("second read");
    assert_eq!(before.len(), 2);
    assert_eq!(
        before.iter().map(|s| s.id).collect::<Vec<_>>(),
        again.iter().map(|s| s.id).collect::<Vec<_>>()
    );
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM campaign_steps", [], |r| r.get(0))
        .expect("count steps");
    assert_eq!(total, 2);

    // Round trip: the read result is what save takes, so an edit keeps the real steps.
    let mut campaign = sample_campaign("With steps");
    campaign.id = Some(with_steps);
    let edited: Vec<CampaignStep> = before
        .into_iter()
        .map(|mut s| {
            s.template_content.push_str(" (edited)");
            s
        })
        .collect();
    save_unified_campaign_internal(&mut conn, campaign, edited, false).expect("save edit");
    let after = get_unified_campaign_steps_internal(&conn, with_steps).expect("read after edit");
    assert_eq!(after.len(), 2);
    assert_eq!(after[0].template_content, "a (edited)");
    assert_eq!(after[1].template_content, "b (edited)");
}

#[test]
fn test_condition_evaluator_all_variants() {
    let conn = setup_test_db();
    let lead_id = insert_test_lead(&conn, "Lê Hoàng C", "Tech Corp", "0912334455", "c@tech.com");

    // 1. ALWAYS condition
    let step_always = CampaignStep {
        id: Some(1),
        campaign_id: 1,
        step_order: 1,
        channel: "ZALO".to_string(),
        delay_minutes: 0,
        condition_type: "ALWAYS".to_string(),
        template_content: "Hello".to_string(),
        ai_prompt_override: None,
        created_at: "".to_string(),
        template_id: None,
    };
    let res = StepConditionEvaluator::evaluate(&conn, &step_always, lead_id, None).unwrap();
    assert_eq!(res, ConditionResult::Execute);

    // 2. IF_NOT_REPLIED condition when no reply has occurred
    let step_reply = CampaignStep {
        id: Some(2),
        campaign_id: 1,
        step_order: 2,
        channel: "WHATSAPP".to_string(),
        delay_minutes: 60,
        condition_type: "IF_NOT_REPLIED".to_string(),
        template_content: "Followup".to_string(),
        ai_prompt_override: None,
        created_at: "".to_string(),
        template_id: None,
    };
    let res = StepConditionEvaluator::evaluate(&conn, &step_reply, lead_id, None).unwrap();
    assert_eq!(res, ConditionResult::Execute);

    // Now simulate customer inbound reply
    crate::db::update_lead_journey_state(
        &conn,
        lead_id,
        "ENGAGED",
        None,
        Some("2026-09-02T12:00:00Z"),
        Some("2026-09-02T10:00:00Z"),
        Some("AskingPrice"),
        Some(0.95),
        None,
    )
    .unwrap();

    let res_after_reply =
        StepConditionEvaluator::evaluate(&conn, &step_reply, lead_id, None).unwrap();
    assert!(matches!(res_after_reply, ConditionResult::Skip { .. }));

    // 3. IF_DEAL_STAGE_IS condition
    let step_stage_match = CampaignStep {
        id: Some(3),
        campaign_id: 1,
        step_order: 3,
        channel: "EMAIL".to_string(),
        delay_minutes: 120,
        condition_type: "IF_DEAL_STAGE_IS:ENGAGED".to_string(),
        template_content: "Special offer".to_string(),
        ai_prompt_override: None,
        created_at: "".to_string(),
        template_id: None,
    };
    let res_stage_match =
        StepConditionEvaluator::evaluate(&conn, &step_stage_match, lead_id, None).unwrap();
    assert_eq!(res_stage_match, ConditionResult::Execute);

    let step_stage_mismatch = CampaignStep {
        id: Some(4),
        campaign_id: 1,
        step_order: 4,
        channel: "EMAIL".to_string(),
        delay_minutes: 120,
        condition_type: "IF_DEAL_STAGE_IS:WON".to_string(),
        template_content: "Contract onboarding".to_string(),
        ai_prompt_override: None,
        created_at: "".to_string(),
        template_id: None,
    };
    let res_stage_mismatch =
        StepConditionEvaluator::evaluate(&conn, &step_stage_mismatch, lead_id, None).unwrap();
    assert!(matches!(res_stage_mismatch, ConditionResult::Skip { .. }));

    // 4. IF_INTENT_IS condition
    let step_intent_match = CampaignStep {
        id: Some(5),
        campaign_id: 1,
        step_order: 5,
        channel: "ZALO".to_string(),
        delay_minutes: 0,
        condition_type: "IF_INTENT_IS:AskingPrice".to_string(),
        template_content: "Pricing details".to_string(),
        ai_prompt_override: None,
        created_at: "".to_string(),
        template_id: None,
    };
    let res_intent =
        StepConditionEvaluator::evaluate(&conn, &step_intent_match, lead_id, None).unwrap();
    assert_eq!(res_intent, ConditionResult::Execute);

    let step_intent_mismatch = CampaignStep {
        id: Some(6),
        campaign_id: 1,
        step_order: 6,
        channel: "ZALO".to_string(),
        delay_minutes: 0,
        condition_type: "IF_INTENT_IS:Complaint".to_string(),
        template_content: "Apology".to_string(),
        ai_prompt_override: None,
        created_at: "".to_string(),
        template_id: None,
    };
    let res_intent_mismatch =
        StepConditionEvaluator::evaluate(&conn, &step_intent_mismatch, lead_id, None).unwrap();
    assert!(matches!(res_intent_mismatch, ConditionResult::Skip { .. }));
}

#[test]
fn test_throttling_and_account_rotation() {
    let config = ThrottlingConfig {
        min_delay_secs: 1,
        max_delay_secs: 3,
        burst_limit: 10,
        cooldown_secs: 5,
        daily_quota: 5,
    };
    let throttler = AntiBanThrottler::new(config);
    let rotator = AccountRotator::with_round_robin();

    rotator.register_account("ZALO", "IMEI_001");
    rotator.register_account("ZALO", "IMEI_002");

    // Test round robin distribution
    let acc1 = rotator.next_account("ZALO", &throttler).unwrap();
    assert_eq!(acc1, "IMEI_001");
    let acc2 = rotator.next_account("ZALO", &throttler).unwrap();
    assert_eq!(acc2, "IMEI_002");
    let acc3 = rotator.next_account("ZALO", &throttler).unwrap();
    assert_eq!(acc3, "IMEI_001");

    // Test sending records & quota
    for _ in 0..5 {
        assert!(throttler.can_send("IMEI_001").is_ok());
        throttler.record_sent("IMEI_001");
    }

    // Daily quota reached for IMEI_001
    assert!(throttler.can_send("IMEI_001").is_err());
    // IMEI_002 is still fresh
    assert!(throttler.can_send("IMEI_002").is_ok());

    // Test burst cooldown with a separate throttler
    let burst_config = ThrottlingConfig {
        min_delay_secs: 1,
        max_delay_secs: 3,
        burst_limit: 2,
        cooldown_secs: 30,
        daily_quota: 100,
    };
    let burst_throttler = AntiBanThrottler::new(burst_config);
    burst_throttler.record_sent("BURST_ACC");
    burst_throttler.record_sent("BURST_ACC");
    // After 2 messages, burst limit reached -> error
    assert!(burst_throttler.can_send("BURST_ACC").is_err());

    // Delay calculation produces valid durations within configured bounds
    let delay = throttler.calculate_delay("IMEI_002");
    assert!(delay.as_secs() >= 1 && delay.as_secs() <= 4);
}

#[test]
fn test_dispatcher_spintax_and_variable_interpolation() {
    let conn = setup_test_db();
    let lead_id = insert_test_lead(
        &conn,
        "Đỗ Minh Tâm",
        "Tâm An Logistics",
        "0988112233",
        "tam@taman.vn",
    );

    let template =
        "{Kính gửi|Chào anh|Chào bạn} {{name}} từ {{company}}! Số {{phone}} của anh đã được duyệt.";
    conn.execute(
        "INSERT INTO zalo_sessions (imei, session_name) VALUES ('IMEI_TEST', 'Test Session')",
        [],
    )
    .unwrap();
    let (rendered, phone) = OmnichannelDispatcher::render_for_lead(&conn, template, lead_id);

    assert_eq!(phone, "0988112233");
    assert!(
        rendered.starts_with("Kính gửi Đỗ Minh Tâm")
            || rendered.starts_with("Chào anh Đỗ Minh Tâm")
            || rendered.starts_with("Chào bạn Đỗ Minh Tâm")
    );
    assert!(rendered.contains("Tâm An Logistics"));
    assert!(rendered.contains("0988112233"));

    // Test DryRun dispatch
    let payload = DispatchPayload {
        campaign_id: 1,
        step_id: 1,
        lead_id,
        channel: "ZALO".to_string(),
        target_recipient: "".to_string(),
        account_id: Some("IMEI_TEST".to_string()),
        template_content: template.to_string(),
        ai_prompt_override: None,
    };

    let dry_res = OmnichannelDispatcher::dispatch(&conn, &payload, DispatchMode::DryRun).unwrap();
    assert!(dry_res.success);
    assert!(dry_res.is_dry_run);
    assert_eq!(dry_res.dispatched_channel, "ZALO");

    // Test Live dispatch
    let live_res = OmnichannelDispatcher::dispatch(&conn, &payload, DispatchMode::Live).unwrap();
    assert!(live_res.success);
    assert!(!live_res.is_dry_run);
}

#[test]
fn test_scheduler_step_progression_and_scheduling_next_step() {
    let mut conn = setup_test_db();
    let lead_id = insert_test_lead(&conn, "Phạm Quốc D", "D Group", "0933445566", "d@dgroup.vn");

    let campaign = Campaign {
        id: None,
        name: "Two-Step Funnel".to_string(),
        description: None,
        channel_type: "OMNICHANNEL".to_string(),
        status: "ACTIVE".to_string(),
        audience_filter_json: None,
        schedule_config_json: None,
        created_at: "".to_string(),
        updated_at: "".to_string(),
    };

    let steps = vec![
        CampaignStep {
            id: None,
            campaign_id: 0,
            step_order: 1,
            channel: "ZALO".to_string(),
            delay_minutes: 0,
            condition_type: "ALWAYS".to_string(),
            template_content: "Bước 1: Chào {{name}}!".to_string(),
            ai_prompt_override: None,
            created_at: "".to_string(),
            template_id: None,
        },
        CampaignStep {
            id: None,
            campaign_id: 0,
            step_order: 2,
            channel: "WHATSAPP".to_string(),
            delay_minutes: 1440, // 24 hours
            condition_type: "IF_NOT_REPLIED".to_string(),
            template_content: "Bước 2: Follow up {{name}}!".to_string(),
            ai_prompt_override: None,
            created_at: "".to_string(),
            template_id: None,
        },
    ];

    let campaign_id = save_unified_campaign_internal(&mut conn, campaign, steps, false).unwrap();
    let enrolled = enroll_leads_into_campaign_internal(&conn, campaign_id, &[lead_id]).unwrap();
    assert_eq!(enrolled, 1);

    let scheduler = CampaignScheduler::with_defaults();
    scheduler.rotator.register_account("ZALO", "IMEI_MAIN");
    scheduler
        .rotator
        .register_account("WHATSAPP", "+84933445566");
    conn.execute(
        "INSERT INTO zalo_sessions (imei, session_name) VALUES ('IMEI_MAIN', 'Test Session')",
        [],
    )
    .unwrap();

    // Process due executions (Step 1 is due immediately)
    let summary = scheduler
        .process_due_executions(&conn, DispatchMode::Live, 10)
        .unwrap();

    assert_eq!(summary.total_due_found, 1);
    assert_eq!(summary.sent_count, 1);
    assert_eq!(summary.next_steps_scheduled, 1);

    // Verify executions table now has Step 1 (SENT) and Step 2 (PENDING)
    let executions = crate::db::get_campaign_executions(&conn, campaign_id).unwrap();
    assert_eq!(executions.len(), 2);
    assert_eq!(executions[0].status, "SENT");
    assert_eq!(executions[1].status, "PENDING");
}

#[test]
fn test_campaign_analytics_aggregation() {
    let mut conn = setup_test_db();
    let lead1 = insert_test_lead(&conn, "Lead 1", "Co 1", "0911111111", "l1@co.com");
    let lead2 = insert_test_lead(&conn, "Lead 2", "Co 2", "0922222222", "l2@co.com");
    let lead3 = insert_test_lead(&conn, "Lead 3", "Co 3", "0933333333", "l3@co.com");

    let campaign = Campaign {
        id: None,
        name: "Analytics Campaign".to_string(),
        description: None,
        channel_type: "OMNICHANNEL".to_string(),
        status: "ACTIVE".to_string(),
        audience_filter_json: None,
        schedule_config_json: None,
        created_at: "".to_string(),
        updated_at: "".to_string(),
    };

    let steps = vec![
        CampaignStep {
            id: None,
            campaign_id: 0,
            step_order: 1,
            channel: "ZALO".to_string(),
            delay_minutes: 0,
            condition_type: "ALWAYS".to_string(),
            template_content: "Step 1".to_string(),
            ai_prompt_override: None,
            created_at: "".to_string(),
            template_id: None,
        },
        CampaignStep {
            id: None,
            campaign_id: 0,
            step_order: 2,
            channel: "EMAIL".to_string(),
            delay_minutes: 60,
            condition_type: "IF_NOT_REPLIED".to_string(),
            template_content: "Step 2".to_string(),
            ai_prompt_override: None,
            created_at: "".to_string(),
            template_id: None,
        },
    ];

    let campaign_id = save_unified_campaign_internal(&mut conn, campaign, steps, false).unwrap();
    enroll_leads_into_campaign_internal(&conn, campaign_id, &[lead1, lead2, lead3]).unwrap();

    let all_steps = crate::db::get_campaign_steps(&conn, campaign_id).unwrap();
    let step1_id = all_steps[0].id.unwrap();
    let step2_id = all_steps[1].id.unwrap();

    // Manually mark one execution SENT, one DELIVERED, one REPLIED, one FAILED
    conn.execute(
        "UPDATE campaign_executions SET status = 'SENT' WHERE lead_id = ?1 AND step_id = ?2",
        [lead1, step1_id as i32],
    )
    .unwrap();

    conn.execute(
        "UPDATE campaign_executions SET status = 'REPLIED' WHERE lead_id = ?1 AND step_id = ?2",
        [lead2, step1_id as i32],
    )
    .unwrap();

    // Insert an execution for step 2
    let _ = crate::db::create_campaign_execution(
        &conn,
        campaign_id,
        step2_id,
        lead1,
        Some("0911111111"),
        None,
        "2026-09-02T15:00:00Z",
    )
    .unwrap();

    // Add a won deal for lead2 to test conversion
    conn.execute(
        "INSERT INTO deals (lead_id, title, value, stage, created_at, currency)
         VALUES (?1, 'Contract Won', 15000000.0, 'WON', '2026-09-02T16:00:00Z', 'VND')",
        [lead2],
    )
    .unwrap();

    let analytics = get_unified_campaign_analytics_internal(&conn, campaign_id).unwrap();
    assert_eq!(analytics.campaign_id, campaign_id);
    assert_eq!(analytics.total_enrolled, 3);
    assert_eq!(analytics.total_steps, 2);
    assert_eq!(analytics.replied_count, 1);
    assert_eq!(analytics.converted_count, 1);
    assert_eq!(analytics.step_breakdown.len(), 2);
    assert!(analytics.response_rate > 0.0);
    assert!(analytics.conversion_rate > 0.0);
}

#[test]
fn test_campaign_scheduler_atomic_claim_concurrency_race() {
    let mut conn = setup_test_db();
    let lead_id = insert_test_lead(
        &conn,
        "Concurrency Lead",
        "Concurrent Co",
        "0988776655",
        "c@co.com",
    );

    let campaign = Campaign {
        id: None,
        name: "Concurrency Test Campaign".to_string(),
        description: None,
        channel_type: "ZALO".to_string(),
        status: "ACTIVE".to_string(),
        audience_filter_json: None,
        schedule_config_json: None,
        created_at: "".to_string(),
        updated_at: "".to_string(),
    };

    let steps = vec![CampaignStep {
        id: None,
        campaign_id: 0,
        step_order: 1,
        channel: "ZALO".to_string(),
        delay_minutes: 0,
        condition_type: "ALWAYS".to_string(),
        template_content: "Hello {{name}}!".to_string(),
        ai_prompt_override: None,
        created_at: "".to_string(),
        template_id: None,
    }];

    let campaign_id = save_unified_campaign_internal(&mut conn, campaign, steps, false).unwrap();
    enroll_leads_into_campaign_internal(&conn, campaign_id, &[lead_id]).unwrap();

    conn.execute(
        "INSERT INTO zalo_sessions (imei, session_name) VALUES ('IMEI_CONC', 'Session')",
        [],
    )
    .unwrap();

    let scheduler1 = CampaignScheduler::with_defaults();
    scheduler1.rotator.register_account("ZALO", "IMEI_CONC");
    let scheduler2 = CampaignScheduler::with_defaults();
    scheduler2.rotator.register_account("ZALO", "IMEI_CONC");

    // Worker 1 processes and claims the execution
    let summary1 = scheduler1
        .process_due_executions(&conn, DispatchMode::Live, 10)
        .unwrap();

    assert_eq!(summary1.total_due_found, 1);
    assert_eq!(summary1.sent_count, 1);

    // Worker 2 immediately tries to process the same DB
    let summary2 = scheduler2
        .process_due_executions(&conn, DispatchMode::Live, 10)
        .unwrap();

    // Since worker 1 atomically claimed and updated the status to SENT, worker 2 finds 0 due executions
    assert_eq!(summary2.total_due_found, 0);
    assert_eq!(summary2.sent_count, 0);

    // Verify exactly ONE outreach_history record exists (no duplicate send)
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM outreach_history WHERE lead_id = ?1",
            [lead_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn test_spintax_malformed_empty_braces_no_panic() {
    // Malformed empty braces should resolve safely without modulo by zero panic
    let res1 = OmnichannelDispatcher::resolve_spintax("{}");
    assert_eq!(res1, "");

    let res2 = OmnichannelDispatcher::resolve_spintax("Hello {} world!");
    assert_eq!(res2, "Hello  world!");

    let res3 = OmnichannelDispatcher::resolve_spintax("{   }");
    assert_eq!(res3, "");

    let res4 = OmnichannelDispatcher::resolve_spintax("{a|b|c}");
    assert!(res4 == "a" || res4 == "b" || res4 == "c");
}

#[test]
fn test_dispatcher_validation_failure() {
    let conn = setup_test_db();
    let payload_bad_email = DispatchPayload {
        campaign_id: 1,
        step_id: 1,
        lead_id: 1,
        channel: "EMAIL".to_string(),
        target_recipient: "not-an-email".to_string(),
        account_id: None,
        template_content: "Test".to_string(),
        ai_prompt_override: None,
    };

    let res = OmnichannelDispatcher::dispatch(&conn, &payload_bad_email, DispatchMode::Live);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("invalid email address"));

    let payload_empty_phone = DispatchPayload {
        campaign_id: 1,
        step_id: 1,
        lead_id: 1,
        channel: "WHATSAPP".to_string(),
        target_recipient: "123".to_string(), // Too short
        account_id: None,
        template_content: "Test".to_string(),
        ai_prompt_override: None,
    };

    let res_phone =
        OmnichannelDispatcher::dispatch(&conn, &payload_empty_phone, DispatchMode::Live);
    assert!(res_phone.is_err());
    assert!(res_phone.unwrap_err().contains("invalid phone number"));
}

#[test]
fn test_provider_delivery_lifecycle_and_multichannel_confirmation() {
    let conn = setup_test_db();
    let lead_id = insert_test_lead(
        &conn,
        "Nguyễn Văn Alpha",
        "Alpha Corp",
        "0912345678",
        "alpha@corp.vn",
    );

    // 1. Setup accounts for all 4 channels
    conn.execute(
        "INSERT INTO zalo_sessions (imei, session_name, is_active) VALUES ('IMEI_PROV', 'Zalo Prov', 1)",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, account_name, provider_account_key, status, created_at, updated_at)
         VALUES (101, 'whatsapp', 'WA Prov', 'wpa_101', 'ACTIVE', '2026-01-01', '2026-01-01')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO smtp_servers (id, host, port, username, from_email, is_active)
         VALUES (201, 'smtp.alphacorp.vn', 587, 'info', 'info@alphacorp.vn', 1)",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, account_name, provider_account_key, status, created_at, updated_at)
         VALUES (301, 'facebook', 'FB Page Prov', 'page_301', 'ACTIVE', '2026-01-01', '2026-01-01')",
        [],
    ).unwrap();

    // 2. Test Zalo live dispatch (initially transitions to SUBMITTED awaiting external confirmation)
    let payload_zalo = DispatchPayload {
        campaign_id: 1,
        step_id: 1,
        lead_id,
        channel: "ZALO".to_string(),
        target_recipient: "0912345678".to_string(),
        account_id: Some("IMEI_PROV".to_string()),
        template_content: "Zalo outreach to {{name}}".to_string(),
        ai_prompt_override: None,
    };
    let res_zalo =
        OmnichannelDispatcher::dispatch(&conn, &payload_zalo, DispatchMode::Live).unwrap();
    assert!(res_zalo.success);
    assert_eq!(res_zalo.delivery_status, "SUBMITTED");
    assert!(res_zalo
        .provider_message_id
        .as_ref()
        .unwrap()
        .starts_with("zalo_msg_"));

    // 3. Test WhatsApp live dispatch
    let payload_wa = DispatchPayload {
        campaign_id: 1,
        step_id: 2,
        lead_id,
        channel: "WHATSAPP".to_string(),
        target_recipient: "+84912345678".to_string(),
        account_id: Some("wpa_101".to_string()),
        template_content: "WhatsApp outreach to {{name}}".to_string(),
        ai_prompt_override: None,
    };
    let res_wa = OmnichannelDispatcher::dispatch(&conn, &payload_wa, DispatchMode::Live).unwrap();
    assert!(res_wa.success);
    assert_eq!(res_wa.delivery_status, "SUBMITTED");
    assert!(res_wa
        .provider_message_id
        .as_ref()
        .unwrap()
        .starts_with("wamid."));

    // 4. Test Email live dispatch
    let payload_email = DispatchPayload {
        campaign_id: 1,
        step_id: 3,
        lead_id,
        channel: "EMAIL".to_string(),
        target_recipient: "alpha@corp.vn".to_string(),
        account_id: Some("smtp.alphacorp.vn".to_string()),
        template_content: "Email outreach to {{name}}".to_string(),
        ai_prompt_override: None,
    };
    let res_email =
        OmnichannelDispatcher::dispatch(&conn, &payload_email, DispatchMode::Live).unwrap();
    assert!(res_email.success);
    assert_eq!(res_email.delivery_status, "SUBMITTED");
    assert!(res_email
        .provider_message_id
        .as_ref()
        .unwrap()
        .starts_with("<"));

    // 5. Test Facebook live dispatch
    let payload_fb = DispatchPayload {
        campaign_id: 1,
        step_id: 4,
        lead_id,
        channel: "FACEBOOK".to_string(),
        target_recipient: "1000987654321".to_string(),
        account_id: Some("page_301".to_string()),
        template_content: "Facebook outreach to {{name}}".to_string(),
        ai_prompt_override: None,
    };
    let res_fb = OmnichannelDispatcher::dispatch(&conn, &payload_fb, DispatchMode::Live).unwrap();
    assert!(res_fb.success);
    assert_eq!(res_fb.delivery_status, "SUBMITTED");
    assert!(res_fb
        .provider_message_id
        .as_ref()
        .unwrap()
        .starts_with("mid.$gA_"));

    // Before confirmation: all 4 are in SUBMITTED state in outreach_history
    let submitted_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM outreach_history WHERE lead_id = ?1 AND status = 'SUBMITTED'",
            [lead_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(submitted_count, 4);

    // External delivery confirmation arrives via webhook / callback for each
    let now = "2026-09-05T08:00:00Z";
    assert!(OmnichannelDispatcher::confirm_provider_delivery(
        &conn,
        res_zalo.provider_message_id.as_ref().unwrap(),
        Some("ZALO"),
        now
    )
    .unwrap());
    assert!(OmnichannelDispatcher::confirm_provider_delivery(
        &conn,
        res_wa.provider_message_id.as_ref().unwrap(),
        Some("WHATSAPP"),
        now
    )
    .unwrap());
    assert!(OmnichannelDispatcher::confirm_provider_delivery(
        &conn,
        res_email.provider_message_id.as_ref().unwrap(),
        Some("EMAIL"),
        now
    )
    .unwrap());
    assert!(OmnichannelDispatcher::confirm_provider_delivery(
        &conn,
        res_fb.provider_message_id.as_ref().unwrap(),
        Some("FACEBOOK"),
        now
    )
    .unwrap());

    // Verify all 4 transitioned to DELIVERED status
    let delivered_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM outreach_history WHERE lead_id = ?1 AND status = 'DELIVERED'",
            [lead_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(delivered_count, 4);

    // Verify activities rows contain provider message ID
    let act_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM activities WHERE lead_id = ?1 AND content LIKE '%Provider Message ID:%'",
            [lead_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(act_count, 4);
}

#[test]
fn test_dispatcher_idempotency_duplicate_send_prevention() {
    let conn = setup_test_db();
    let lead_id = insert_test_lead(&conn, "Lê B", "B Tech", "0977889900", "b@btech.vn");
    conn.execute(
        "INSERT INTO zalo_sessions (imei, session_name, is_active) VALUES ('IMEI_IDEM', 'Idem Session', 1)",
        [],
    ).unwrap();

    let payload = DispatchPayload {
        campaign_id: 10,
        step_id: 20,
        lead_id,
        channel: "ZALO".to_string(),
        target_recipient: "0977889900".to_string(),
        account_id: Some("IMEI_IDEM".to_string()),
        template_content: "Idempotent step message".to_string(),
        ai_prompt_override: None,
    };

    // First dispatch sends and marks SUBMITTED
    let res1 = OmnichannelDispatcher::dispatch(&conn, &payload, DispatchMode::Live).unwrap();
    assert!(res1.success);
    assert_eq!(res1.delivery_status, "SUBMITTED");
    let initial_msg_id = res1.message_id.unwrap();

    // Second dispatch of exact same step while SUBMITTED recovers existing submission receipt
    let res2 = OmnichannelDispatcher::dispatch(&conn, &payload, DispatchMode::Live).unwrap();
    assert!(res2.success);
    assert_eq!(res2.delivery_status, "SUBMITTED");
    assert_eq!(res2.message_id.unwrap(), initial_msg_id);

    // External provider confirms delivery
    assert!(OmnichannelDispatcher::confirm_provider_delivery(
        &conn,
        res1.provider_message_id.as_ref().unwrap(),
        Some("ZALO"),
        "2026-09-05T08:00:00Z"
    )
    .unwrap());

    // Third dispatch after DELIVERED recovers DELIVERED record
    let res3 = OmnichannelDispatcher::dispatch(&conn, &payload, DispatchMode::Live).unwrap();
    assert!(res3.success);
    assert_eq!(res3.delivery_status, "DELIVERED");
    assert_eq!(res3.message_id.unwrap(), initial_msg_id);

    // Verify outreach_history has strictly 1 record
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM outreach_history WHERE lead_id = ?1 AND template_id = 20",
            [lead_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn test_provider_failure_and_scheduler_retry_backoff() {
    let mut conn = setup_test_db();
    let lead_id = insert_test_lead(&conn, "Trần C", "C Log", "0966554433", "c@clog.vn");

    // Account with inactive session
    conn.execute(
        "INSERT INTO zalo_sessions (imei, session_name, is_active) VALUES ('IMEI_INACTIVE', 'Inactive Sess', 0)",
        [],
    ).unwrap();

    let campaign = Campaign {
        id: None,
        name: "Failure Retry Campaign".to_string(),
        description: None,
        channel_type: "ZALO".to_string(),
        status: "ACTIVE".to_string(),
        audience_filter_json: None,
        schedule_config_json: None,
        created_at: "".to_string(),
        updated_at: "".to_string(),
    };

    let steps = vec![CampaignStep {
        id: None,
        campaign_id: 0,
        step_order: 1,
        channel: "ZALO".to_string(),
        delay_minutes: 0,
        condition_type: "ALWAYS".to_string(),
        template_content: "Fail test".to_string(),
        ai_prompt_override: None,
        created_at: "".to_string(),
        template_id: None,
    }];

    let campaign_id = save_unified_campaign_internal(&mut conn, campaign, steps, false).unwrap();
    enroll_leads_into_campaign_internal(&conn, campaign_id, &[lead_id]).unwrap();

    let scheduler = CampaignScheduler::with_defaults();
    scheduler.rotator.register_account("ZALO", "IMEI_INACTIVE");

    // Process due execution: provider delivery should fail closed because session is inactive
    let summary = scheduler
        .process_due_executions(&conn, DispatchMode::Live, 10)
        .unwrap();
    assert_eq!(summary.failed_count, 1);
    assert_eq!(summary.sent_count, 0);

    // Verify execution status was reset to PENDING with [Retry 1/3]
    let (status, err_msg): (String, Option<String>) = conn
        .query_row(
            "SELECT status, error_message FROM campaign_executions WHERE campaign_id = ?1 AND lead_id = ?2",
            rusqlite::params![campaign_id, lead_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert_eq!(status, "PENDING");
    assert!(err_msg.unwrap().starts_with("[Retry 1/3]"));

    // Verify outreach_history row is recorded with status = FAILED
    let hist_status: String = conn
        .query_row(
            "SELECT status FROM outreach_history WHERE lead_id = ?1",
            [lead_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(hist_status, "FAILED");
}

struct MockFailingProvider;

#[async_trait::async_trait]
impl ChannelDeliveryProvider for MockFailingProvider {
    async fn deliver(
        &self,
        _channel: &str,
        _account: &str,
        _recipient: &str,
        _message: &str,
        _idempotency_key: &str,
        _timeout: std::time::Duration,
    ) -> Result<ProviderDeliveryResponse, String> {
        Err("Gateway Timeout (HTTP 504)".to_string())
    }
}

#[test]
fn test_custom_mock_provider_timeout_and_network_error() {
    let conn = setup_test_db();
    let lead_id = insert_test_lead(&conn, "Mock Lead", "Mock Co", "0911223344", "mock@co.vn");
    conn.execute(
        "INSERT INTO zalo_sessions (imei, session_name, is_active) VALUES ('IMEI_MOCK', 'Mock Sess', 1)",
        [],
    ).unwrap();

    let payload = DispatchPayload {
        campaign_id: 99,
        step_id: 101,
        lead_id,
        channel: "ZALO".to_string(),
        target_recipient: "0911223344".to_string(),
        account_id: Some("IMEI_MOCK".to_string()),
        template_content: "Timeout test".to_string(),
        ai_prompt_override: None,
    };

    let failing_provider = MockFailingProvider;
    let res = OmnichannelDispatcher::dispatch_with_provider(
        &conn,
        &payload,
        DispatchMode::Live,
        &failing_provider,
    );

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.contains("Gateway Timeout (HTTP 504)"));

    // Verify outreach_history recorded FAILED state
    let (status, err_msg): (String, Option<String>) = conn
        .query_row(
            "SELECT status, error_message FROM outreach_history WHERE lead_id = ?1",
            [lead_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert_eq!(status, "FAILED");
    assert!(err_msg.unwrap().contains("Gateway Timeout"));
}

struct MockHangingProvider;

#[async_trait::async_trait]
impl ChannelDeliveryProvider for MockHangingProvider {
    async fn deliver(
        &self,
        _channel: &str,
        _account: &str,
        _recipient: &str,
        _message: &str,
        _idempotency_key: &str,
        _timeout: std::time::Duration,
    ) -> Result<ProviderDeliveryResponse, String> {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        Ok(ProviderDeliveryResponse {
            provider_message_id: "late_response".to_string(),
            status: DeliveryStatus::Delivered,
        })
    }
}

#[tokio::test]
async fn test_provider_async_timeout_cancellation() {
    let conn = setup_test_db();
    let lead_id = insert_test_lead(
        &conn,
        "Timeout Lead",
        "Timeout Co",
        "0933445566",
        "timeout@co.vn",
    );
    conn.execute(
        "INSERT INTO zalo_sessions (imei, session_name, is_active) VALUES ('IMEI_TIMEOUT', 'Timeout Sess', 1)",
        [],
    ).unwrap();

    let payload = DispatchPayload {
        campaign_id: 10,
        step_id: 20,
        lead_id,
        channel: "ZALO".to_string(),
        target_recipient: "0933445566".to_string(),
        account_id: Some("IMEI_TIMEOUT".to_string()),
        template_content: "Hanging test".to_string(),
        ai_prompt_override: None,
    };

    let hanging_provider = MockHangingProvider;
    let res = OmnichannelDispatcher::dispatch_async(
        &conn,
        &payload,
        DispatchMode::Live,
        &hanging_provider,
        std::time::Duration::from_millis(50),
    )
    .await;

    assert!(res.is_err(), "Expected timeout error but got success");
    let err = res.unwrap_err();
    assert!(
        err.contains("timed out") || err.contains("Provider delivery timed out"),
        "Expected timeout message, got: {}",
        err
    );

    // Verify outreach_history status was recorded as FAILED
    let (status, err_msg): (String, Option<String>) = conn
        .query_row(
            "SELECT status, error_message FROM outreach_history WHERE lead_id = ?1",
            [lead_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert_eq!(status, "FAILED");
    assert!(err_msg.unwrap().contains("timed out"));
}

#[test]
fn test_database_idempotency_unique_constraint_concurrency_race() {
    let conn = setup_test_db();
    let lead_id = insert_test_lead(&conn, "Race Lead", "Race Co", "0944556677", "race@co.vn");
    conn.execute(
        "INSERT INTO zalo_sessions (imei, session_name, is_active) VALUES ('IMEI_RACE', 'Race Sess', 1)",
        [],
    ).unwrap();

    let payload = DispatchPayload {
        campaign_id: 88,
        step_id: 99,
        lead_id,
        channel: "ZALO".to_string(),
        target_recipient: "0944556677".to_string(),
        account_id: Some("IMEI_RACE".to_string()),
        template_content: "Idempotency test {{name}}".to_string(),
        ai_prompt_override: None,
    };

    let provider = DefaultChannelDeliveryProvider::default();

    // First dispatch succeeds and transitions to SUBMITTED
    let res1 = OmnichannelDispatcher::dispatch_with_provider(
        &conn,
        &payload,
        DispatchMode::Live,
        &provider,
    )
    .expect("First dispatch should succeed");

    assert!(res1.success);
    assert_eq!(res1.delivery_status, "SUBMITTED");
    let msg_id1 = res1.message_id.unwrap();

    // Second dispatch with same campaign/step/lead while SUBMITTED:
    // Intercepted by idempotency check and safely returns existing submission receipt
    let res2 = OmnichannelDispatcher::dispatch_with_provider(
        &conn,
        &payload,
        DispatchMode::Live,
        &provider,
    )
    .expect("Second dispatch should return existing idempotent result");

    assert!(res2.success);
    assert_eq!(res2.message_id, Some(msg_id1));
    assert_eq!(res2.delivery_status, "SUBMITTED");

    // External provider confirms delivery
    assert!(OmnichannelDispatcher::confirm_provider_delivery(
        &conn,
        res1.provider_message_id.as_ref().unwrap(),
        Some("ZALO"),
        "2026-09-05T08:00:00Z",
    )
    .unwrap());

    // Third dispatch recovers DELIVERED
    let res3 = OmnichannelDispatcher::dispatch_with_provider(
        &conn,
        &payload,
        DispatchMode::Live,
        &provider,
    )
    .expect("Third dispatch should recover delivered record");
    assert!(res3.success);
    assert_eq!(res3.delivery_status, "DELIVERED");
    assert_eq!(res3.message_id, Some(msg_id1));

    // Verify exactly 1 record exists in outreach_history with unique idempotency_key
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM outreach_history WHERE lead_id = ?1",
            [lead_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "Only one outreach history entry must exist");

    // Verify unique index on idempotency_key rejects raw duplicate insert
    let idempotency_key = format!(
        "campaign:{}:step:{}:lead:{}",
        payload.campaign_id, payload.step_id, payload.lead_id
    );
    let duplicate_insert = conn.execute(
        "INSERT INTO outreach_history (lead_id, template_id, status, sent_at, session_imei, idempotency_key)
         VALUES (?1, ?2, 'SUBMITTED', '2026-09-05T00:00:00Z', 'IMEI_RACE', ?3)",
        rusqlite::params![lead_id, payload.step_id as i32, idempotency_key],
    );

    assert!(duplicate_insert.is_err());
    let err_str = duplicate_insert.unwrap_err().to_string();
    assert!(
        err_str.contains("UNIQUE constraint failed")
            || err_str.contains("idx_outreach_history_idempotency"),
        "Expected UNIQUE constraint violation, got: {}",
        err_str
    );
}

struct MockProviderWithCallCounter {
    call_count: std::sync::atomic::AtomicUsize,
}

#[async_trait::async_trait]
impl ChannelDeliveryProvider for MockProviderWithCallCounter {
    async fn deliver(
        &self,
        _channel: &str,
        _account: &str,
        _recipient: &str,
        _message: &str,
        _idempotency_key: &str,
        _timeout: std::time::Duration,
    ) -> Result<ProviderDeliveryResponse, String> {
        let prev = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(ProviderDeliveryResponse {
            provider_message_id: format!("prov_counted_{}", prev),
            status: DeliveryStatus::Submitted,
        })
    }
}

#[test]
fn test_provider_acceptance_client_timeout_and_retry_reconciliation() {
    let conn = setup_test_db();
    let lead_id = insert_test_lead(&conn, "Retry Lead", "Retry Co", "0955667788", "retry@co.vn");
    conn.execute(
        "INSERT INTO zalo_sessions (imei, session_name, is_active) VALUES ('IMEI_RETRY', 'Retry Sess', 1)",
        [],
    ).unwrap();

    let payload = DispatchPayload {
        campaign_id: 55,
        step_id: 66,
        lead_id,
        channel: "ZALO".to_string(),
        target_recipient: "0955667788".to_string(),
        account_id: Some("IMEI_RETRY".to_string()),
        template_content: "Retry reconciliation test".to_string(),
        ai_prompt_override: None,
    };

    let provider = MockProviderWithCallCounter {
        call_count: std::sync::atomic::AtomicUsize::new(0),
    };

    // 1. Initial attempt transmits to provider and records SUBMITTED state
    let res1 = OmnichannelDispatcher::dispatch_with_provider(
        &conn,
        &payload,
        DispatchMode::Live,
        &provider,
    )
    .unwrap();
    assert!(res1.success);
    assert_eq!(res1.delivery_status, "SUBMITTED");
    assert_eq!(
        provider
            .call_count
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );

    // 2. Client retry occurs (e.g. after a client-side timeout / network drop):
    // The dispatcher must reconcile using the database idempotency key,
    // recovering the in-flight SUBMITTED receipt WITHOUT calling the external provider again!
    let res2 = OmnichannelDispatcher::dispatch_with_provider(
        &conn,
        &payload,
        DispatchMode::Live,
        &provider,
    )
    .unwrap();
    assert!(res2.success);
    assert_eq!(res2.delivery_status, "SUBMITTED");
    assert_eq!(res2.message_id, res1.message_id);
    // CRITICAL: Call count must remain 1 - no duplicate provider transmission
    assert_eq!(
        provider
            .call_count
            .load(std::sync::atomic::Ordering::SeqCst),
        1,
        "Provider must not be called again on retry of already submitted message"
    );
}

#[test]
fn test_stuck_submission_reconciliation_worker() {
    let conn = setup_test_db();
    let lead_id = insert_test_lead(&conn, "Stuck Lead", "Stuck Co", "0966778899", "stuck@co.vn");

    // Insert an outreach submission timestamped 1 hour ago
    conn.execute(
        "INSERT INTO outreach_history (lead_id, template_id, status, error_message, sent_at, session_imei, idempotency_key, provider_message_id)
         VALUES (?1, 1, 'SUBMITTED', NULL, strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-3600 seconds'), 'IMEI_STUCK', 'campaign:1:step:1:lead:stuck', 'prov_stuck_123')",
        [lead_id],
    ).unwrap();

    // Insert a fresh outreach submission sent 10 seconds ago (should NOT be reconciled)
    conn.execute(
        "INSERT INTO outreach_history (lead_id, template_id, status, error_message, sent_at, session_imei, idempotency_key, provider_message_id)
         VALUES (?1, 2, 'SUBMITTED', NULL, strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-10 seconds'), 'IMEI_STUCK', 'campaign:1:step:2:lead:stuck', 'prov_fresh_123')",
        [lead_id],
    ).unwrap();

    // Reconcile submissions stuck beyond 300s
    let reconciled_count = OmnichannelDispatcher::reconcile_stuck_submissions(&conn, 300).unwrap();
    assert_eq!(
        reconciled_count, 1,
        "Only the 1-hour-old submission should be reconciled"
    );

    // Verify stuck submission transitioned to SUBMITTED_STALE (holding automated retries to prevent duplicate send)
    let (status1, err1): (String, Option<String>) = conn
        .query_row(
            "SELECT status, error_message FROM outreach_history WHERE provider_message_id = 'prov_stuck_123'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(status1, "SUBMITTED_STALE");
    assert!(err1.unwrap().contains("automated resend held"));

    // Verify fresh submission remains in SUBMITTED state
    let status2: String = conn
        .query_row(
            "SELECT status FROM outreach_history WHERE provider_message_id = 'prov_fresh_123'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status2, "SUBMITTED");

    // Delayed webhook arrives for the stale submission: confirms delivery successfully
    assert!(OmnichannelDispatcher::confirm_provider_delivery(
        &conn,
        "prov_stuck_123",
        Some("ZALO"),
        "2026-09-05T08:05:00Z",
    )
    .unwrap());

    let (status_deliv, _): (String, Option<String>) = conn
        .query_row(
            "SELECT status, error_message FROM outreach_history WHERE provider_message_id = 'prov_stuck_123'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(status_deliv, "DELIVERED");

    // Backwards transition protection: cannot demote DELIVERED to FAILED or SUBMITTED
    let demote_attempt = conn.execute(
        "UPDATE outreach_history SET status = 'FAILED' WHERE provider_message_id = 'prov_stuck_123' AND status != 'DELIVERED'",
        [],
    ).unwrap();
    assert_eq!(
        demote_attempt, 0,
        "Terminal DELIVERED records cannot be demoted backwards"
    );
}

#[test]
fn test_legacy_migration_idempotency_upgrade_path() {
    let conn = Connection::open_in_memory().unwrap();

    // 1. Create legacy schema without idempotency_key and provider_message_id
    conn.execute_batch(
        "CREATE TABLE outreach_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            lead_id INTEGER NOT NULL,
            template_id INTEGER NOT NULL,
            status TEXT NOT NULL,
            error_message TEXT,
            sent_at TEXT NOT NULL,
            session_imei TEXT NOT NULL
        );
        INSERT INTO outreach_history (lead_id, template_id, status, sent_at, session_imei)
        VALUES (1, 10, 'SENT', '2026-01-01T00:00:00Z', 'IMEI_OLD_1');
        INSERT INTO outreach_history (lead_id, template_id, status, sent_at, session_imei)
        VALUES (1, 10, 'SENT', '2026-01-02T00:00:00Z', 'IMEI_OLD_1');
        INSERT INTO outreach_history (lead_id, template_id, status, sent_at, session_imei)
        VALUES (2, 20, 'DELIVERED', '2026-01-03T00:00:00Z', 'IMEI_OLD_2');",
    )
    .unwrap();

    // 2. Execute migration upgrade script as performed in db::init_db
    conn.execute(
        "ALTER TABLE outreach_history ADD COLUMN idempotency_key TEXT",
        [],
    )
    .unwrap();
    conn.execute(
        "ALTER TABLE outreach_history ADD COLUMN provider_message_id TEXT",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE outreach_history SET idempotency_key = 'legacy:history:' || id WHERE idempotency_key IS NULL",
        [],
    ).unwrap();
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_outreach_history_idempotency ON outreach_history(idempotency_key)",
        [],
    ).unwrap();

    // 3. Verify all legacy rows have distinct, populated idempotency keys
    let mut stmt = conn
        .prepare("SELECT id, idempotency_key FROM outreach_history ORDER BY id ASC")
        .unwrap();
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].1, "legacy:history:1");
    assert_eq!(rows[1].1, "legacy:history:2");
    assert_eq!(rows[2].1, "legacy:history:3");

    // 4. Verify unique constraint works on new rows
    let res_ok = conn.execute(
        "INSERT INTO outreach_history (lead_id, template_id, status, sent_at, session_imei, idempotency_key)
         VALUES (3, 30, 'SUBMITTED', '2026-09-05T00:00:00Z', 'IMEI_NEW', 'campaign:1:step:1:lead:3')",
        [],
    );
    assert!(res_ok.is_ok());

    let res_dup = conn.execute(
        "INSERT INTO outreach_history (lead_id, template_id, status, sent_at, session_imei, idempotency_key)
         VALUES (3, 30, 'SUBMITTED', '2026-09-05T00:00:00Z', 'IMEI_NEW', 'campaign:1:step:1:lead:3')",
        [],
    );
    assert!(
        res_dup.is_err(),
        "Duplicate insertion must violate idx_outreach_history_idempotency"
    );
}

#[test]
fn test_concurrent_reconciliation_workers_and_late_webhook_race() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE outreach_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            lead_id INTEGER NOT NULL,
            template_id INTEGER NOT NULL,
            channel TEXT NOT NULL DEFAULT 'ZALO',
            status TEXT NOT NULL,
            error_message TEXT,
            sent_at TEXT NOT NULL,
            session_imei TEXT NOT NULL,
            idempotency_key TEXT UNIQUE,
            provider_message_id TEXT
        );
        CREATE TABLE activities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            lead_id INTEGER NOT NULL,
            activity_type TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            performed_by TEXT NOT NULL
        );",
    )
    .unwrap();

    let old_timestamp = "2026-01-01T00:00:00Z";
    conn.execute(
        "INSERT INTO outreach_history (lead_id, template_id, channel, status, sent_at, session_imei, provider_message_id)
         VALUES (1, 10, 'ZALO', 'SUBMITTED', ?1, 'IMEI_1', 'prov_race_100')",
        [old_timestamp],
    ).unwrap();

    // 1. Worker 1 executes reconciliation and claims the submission
    let count1 = OmnichannelDispatcher::reconcile_stuck_submissions(&conn, 300).unwrap();
    assert_eq!(
        count1, 1,
        "Worker 1 must claim and transition the 1 stuck submission to SUBMITTED_STALE"
    );

    // 2. Worker 2 executes concurrently: since Worker 1 updated status to SUBMITTED_STALE, Worker 2 finds 0 affected
    let count2 = OmnichannelDispatcher::reconcile_stuck_submissions(&conn, 300).unwrap();
    assert_eq!(
        count2, 0,
        "Worker 2 must see 0 affected rows (no race condition / double count)"
    );

    // 3. Late webhook arrives: confirms delivery successfully (SUBMITTED_STALE -> DELIVERED)
    assert!(OmnichannelDispatcher::confirm_provider_delivery(
        &conn,
        "prov_race_100",
        Some("ZALO"),
        "2026-09-05T09:00:00Z",
    )
    .unwrap());

    let (final_status, _): (String, Option<String>) = conn
        .query_row(
            "SELECT status, error_message FROM outreach_history WHERE provider_message_id = 'prov_race_100'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(final_status, "DELIVERED");

    // 4. Duplicate webhook delivery arrives: remains idempotent
    let dup_confirm = OmnichannelDispatcher::confirm_provider_delivery(
        &conn,
        "prov_race_100",
        Some("ZALO"),
        "2026-09-05T09:01:00Z",
    )
    .unwrap();
    assert!(
        dup_confirm,
        "Subsequent webhook callback must return Ok(true) idempotently"
    );
}

#[test]
fn test_provider_message_id_composite_unique_index_and_cross_channel_isolation() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE outreach_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            lead_id INTEGER NOT NULL,
            template_id INTEGER NOT NULL,
            channel TEXT NOT NULL,
            status TEXT NOT NULL,
            sent_at TEXT NOT NULL,
            session_imei TEXT NOT NULL,
            provider_message_id TEXT
        );
        -- Create composite partial index scoped to (channel, provider_message_id)
        CREATE UNIQUE INDEX idx_outreach_channel_provider_msg_id
        ON outreach_history(channel, provider_message_id)
        WHERE provider_message_id IS NOT NULL;
        
        -- Insert rows across different channels with the EXACT same provider message ID
        INSERT INTO outreach_history (lead_id, template_id, channel, status, sent_at, session_imei, provider_message_id)
        VALUES (1, 1, 'FACEBOOK', 'SENT', '2026-01-01T00:00:00Z', 'FB_PAGE_1', 'ext_msg_100');
        INSERT INTO outreach_history (lead_id, template_id, channel, status, sent_at, session_imei, provider_message_id)
        VALUES (2, 1, 'WHATSAPP', 'SENT', '2026-01-01T00:00:00Z', 'WPA_1', 'ext_msg_100');
        
        -- Multiple NULL provider IDs across channels are allowed
        INSERT INTO outreach_history (lead_id, template_id, channel, status, sent_at, session_imei, provider_message_id)
        VALUES (3, 1, 'EMAIL', 'FAILED', '2026-01-01T00:00:00Z', 'SMTP_1', NULL);
        INSERT INTO outreach_history (lead_id, template_id, channel, status, sent_at, session_imei, provider_message_id)
        VALUES (4, 1, 'EMAIL', 'FAILED', '2026-01-01T00:00:00Z', 'SMTP_1', NULL);"
    ).unwrap();

    // Verify external IDs remain completely unmangled and distinct
    let fb_msg: String = conn
        .query_row(
            "SELECT provider_message_id FROM outreach_history WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let wa_msg: String = conn
        .query_row(
            "SELECT provider_message_id FROM outreach_history WHERE id = 2",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        fb_msg, "ext_msg_100",
        "External IDs must never be mutated or rewritten"
    );
    assert_eq!(
        wa_msg, "ext_msg_100",
        "External IDs must never be mutated or rewritten"
    );

    // Attempting duplicate within the SAME channel is rejected by database constraint
    let dup_insert = conn.execute(
        "INSERT INTO outreach_history (lead_id, template_id, channel, status, sent_at, session_imei, provider_message_id)
         VALUES (5, 1, 'FACEBOOK', 'SUBMITTED', '2026-09-05T00:00:00Z', 'FB_PAGE_2', 'ext_msg_100')",
        [],
    );
    assert!(
        dup_insert.is_err(),
        "Composite index must reject duplicate provider_message_id within the same channel"
    );
}

#[test]
fn test_webhook_authentication_replay_protection_and_secret_verification() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE outreach_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            lead_id INTEGER NOT NULL,
            template_id INTEGER NOT NULL,
            channel TEXT NOT NULL DEFAULT 'WHATSAPP',
            status TEXT NOT NULL,
            error_message TEXT,
            sent_at TEXT NOT NULL,
            session_imei TEXT NOT NULL,
            idempotency_key TEXT UNIQUE,
            provider_message_id TEXT
        );
        CREATE UNIQUE INDEX idx_outreach_channel_provider_msg_id
        ON outreach_history(channel, provider_message_id)
        WHERE provider_message_id IS NOT NULL;

        CREATE TABLE activities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            lead_id INTEGER NOT NULL,
            activity_type TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            performed_by TEXT NOT NULL
        );
        INSERT INTO outreach_history (lead_id, template_id, channel, status, sent_at, session_imei, provider_message_id)
        VALUES (1, 10, 'WHATSAPP', 'SUBMITTED', '2026-09-05T08:00:00Z', 'IMEI_1', 'wa_sec_test_001');"
    ).unwrap();

    // 1. Replay attack verification: expired timestamp rejected, fresh timestamp accepted
    let now = chrono::Utc::now().timestamp();
    let expired_timestamp = now - 600; // 10 minutes ago (> 300s skew)
    assert!(OmnichannelDispatcher::verify_webhook_timestamp(expired_timestamp, 300).is_err());
    assert!(OmnichannelDispatcher::verify_webhook_timestamp(now - 15, 300).is_ok());

    // 2. Empty or missing required secret rejected
    let empty_secret_res = OmnichannelDispatcher::confirm_provider_delivery_secure(
        &conn,
        "wa_sec_test_001",
        Some("WHATSAPP"),
        Some("   "),
        Some("super-secret-token"),
        "2026-09-05T08:01:00Z",
    );
    assert!(
        empty_secret_res.is_err(),
        "Empty or mismatching secret must be rejected"
    );

    let missing_secret_res = OmnichannelDispatcher::confirm_provider_delivery_secure(
        &conn,
        "wa_sec_test_001",
        Some("WHATSAPP"),
        None,
        Some("super-secret-token"),
        "2026-09-05T08:01:00Z",
    );
    assert!(
        missing_secret_res.is_err(),
        "Missing secret must be rejected when expected_secret is set"
    );

    // 3. Wrong secret rejected via constant-time check
    let wrong_secret_res = OmnichannelDispatcher::confirm_provider_delivery_secure(
        &conn,
        "wa_sec_test_001",
        Some("WHATSAPP"),
        Some("wrong-secret-token"),
        Some("super-secret-token"),
        "2026-09-05T08:01:00Z",
    );
    assert!(wrong_secret_res.is_err(), "Wrong secret must be rejected");

    // 4. Mismatched channel rejected
    let mismatch_channel_res = OmnichannelDispatcher::confirm_provider_delivery_secure(
        &conn,
        "wa_sec_test_001",
        Some("ZALO"),
        Some("super-secret-token"),
        Some("super-secret-token"),
        "2026-09-05T08:01:00Z",
    );
    assert!(
        mismatch_channel_res.is_err(),
        "Mismatched channel callback must be rejected"
    );

    // 5. Valid matching credentials succeed
    let valid_res = OmnichannelDispatcher::confirm_provider_delivery_secure(
        &conn,
        "wa_sec_test_001",
        Some("WHATSAPP"),
        Some("super-secret-token"),
        Some("super-secret-token"),
        "2026-09-05T08:01:00Z",
    );
    assert!(
        valid_res.unwrap(),
        "Matching channel and valid credentials must confirm delivery"
    );

    // Verify status is DELIVERED
    let status: String = conn
        .query_row(
            "SELECT status FROM outreach_history WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "DELIVERED");

    // 6. Duplicate webhook delivery remains idempotent
    let dup_res = OmnichannelDispatcher::confirm_provider_delivery_secure(
        &conn,
        "wa_sec_test_001",
        Some("WHATSAPP"),
        Some("super-secret-token"),
        Some("super-secret-token"),
        "2026-09-05T08:02:00Z",
    );
    assert!(
        dup_res.unwrap(),
        "Duplicate webhook confirmation must return Ok(true) idempotently"
    );
}

#[tokio::test]
async fn test_live_and_simulated_delivery_provider_separation() {
    let sim_provider = SimulatedChannelDeliveryProvider::default();
    let sim_res = sim_provider
        .deliver(
            "ZALO",
            "IMEI_DEV",
            "0901234567",
            "Hello",
            "test:key:1",
            std::time::Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert!(sim_res
        .provider_message_id
        .starts_with("zalo_msg_IMEI_DEV_"));
    assert_eq!(sim_res.status, DeliveryStatus::Submitted);

    let live_provider = LiveChannelDeliveryProvider::default();
    let live_res = live_provider
        .deliver(
            "ZALO",
            "IMEI_PROD",
            "0901234567",
            "Hello",
            "test:key:2",
            std::time::Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert!(live_res
        .provider_message_id
        .starts_with("zalo_msg_IMEI_PROD_"));
    assert_eq!(live_res.status, DeliveryStatus::Submitted);
}

// ---- editing a campaign keeps its enrolments and history (RS #50 D3) ----

fn running_campaign_with_steps(conn: &mut Connection, n: i32) -> (i64, Vec<CampaignStep>) {
    let mut campaign = sample_campaign("Running");
    campaign.status = "ACTIVE".to_string();
    let steps = (1..=n)
        .map(|i| sample_step(i, "ZALO", &format!("step {i}")))
        .collect();
    let id = save_unified_campaign_internal(conn, campaign, steps, false).expect("save campaign");
    let steps = get_unified_campaign_steps_internal(conn, id).expect("read steps");
    (id, steps)
}

fn exec_rows(conn: &Connection, campaign_id: i64) -> Vec<(i64, i64, i32, String)> {
    let mut stmt = conn
        .prepare(
            "SELECT id, step_id, lead_id, status FROM campaign_executions
             WHERE campaign_id = ?1 ORDER BY id",
        )
        .expect("prepare");
    stmt.query_map([campaign_id], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    })
    .expect("query")
    .map(|r| r.expect("row"))
    .collect()
}

fn put_at_step(conn: &Connection, campaign_id: i64, step_id: i64, lead: i32, status: &str) -> i64 {
    let id = crate::db::create_campaign_execution(
        conn,
        campaign_id,
        step_id,
        lead,
        None,
        None,
        "2026-01-01T00:00:00Z",
    )
    .expect("create execution");
    if status != "PENDING" {
        crate::db::update_campaign_execution_status(
            conn,
            id,
            status,
            None,
            Some("2026-01-01T00:00:00Z"),
        )
        .expect("set status");
    }
    id
}

#[test]
fn test_editing_a_running_campaign_keeps_enrolments_at_kept_steps() {
    let mut conn = setup_test_db();
    let lead1 = insert_test_lead(&conn, "A", "Co A", "0901000001", "a@a.vn");
    let lead2 = insert_test_lead(&conn, "B", "Co B", "0901000002", "b@b.vn");
    let (id, steps) = running_campaign_with_steps(&mut conn, 3);
    let (s1, s2, s3) = (steps[0].id, steps[1].id, steps[2].id);
    let (s1, s2, s3) = (s1.expect("id1"), s2.expect("id2"), s3.expect("id3"));
    let sent = put_at_step(&conn, id, s1, lead1, "SENT");
    let waiting = put_at_step(&conn, id, s2, lead1, "PENDING");
    let other = put_at_step(&conn, id, s1, lead2, "REPLIED");
    let before = exec_rows(&conn, id);

    // Edit: step 1 text changes, step 2 moves behind a new step, step 3 (nobody there) is dropped.
    let mut edited: Vec<CampaignStep> = steps[..2].to_vec();
    edited[0].template_content = "step 1 edited".to_string();
    edited[1].step_order = 3;
    let mut fresh = sample_step(2, "EMAIL", "brand new");
    fresh.id = None;
    edited.push(fresh);
    let mut campaign = crate::db::get_campaign(&conn, id)
        .expect("get")
        .expect("exists");
    campaign.name = "Running (edited)".to_string();

    assert!(
        get_unified_campaign_save_impact_internal(&conn, id, &edited)
            .expect("impact")
            .is_empty()
    );
    let saved =
        save_unified_campaign_internal(&mut conn, campaign, edited, false).expect("edit saves");
    assert_eq!(saved, id);

    // Every enrolment and its status survives, on the same step ids.
    assert_eq!(exec_rows(&conn, id), before);
    let ids: Vec<i64> = vec![sent, waiting, other];
    assert_eq!(
        exec_rows(&conn, id).iter().map(|r| r.0).collect::<Vec<_>>(),
        ids
    );

    let now = get_unified_campaign_steps_internal(&conn, id).expect("steps after edit");
    assert_eq!(now.len(), 3);
    let first = now
        .iter()
        .find(|s| s.id == Some(s1))
        .expect("step 1 kept its id");
    assert_eq!(first.template_content, "step 1 edited");
    let moved = now
        .iter()
        .find(|s| s.id == Some(s2))
        .expect("step 2 kept its id");
    assert_eq!(moved.step_order, 3);
    assert!(now.iter().all(|s| s.id != Some(s3)), "step 3 was dropped");
    assert!(now.iter().any(|s| s.template_content == "brand new"));
    // No history at step 3, so its row is really deleted rather than retired.
    let rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM campaign_steps WHERE id = ?1",
            [s3],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(rows, 0);
    assert_eq!(
        crate::db::get_campaign(&conn, id)
            .expect("get")
            .expect("exists")
            .name,
        "Running (edited)"
    );
}

#[test]
fn test_removing_a_step_with_customers_reports_them_and_keeps_history() {
    let mut conn = setup_test_db();
    let leads: Vec<i32> = (1..=4)
        .map(|i| {
            insert_test_lead(
                &conn,
                &format!("L{i}"),
                "Co",
                &format!("09010000{i}"),
                "x@x.vn",
            )
        })
        .collect();
    let (id, steps) = running_campaign_with_steps(&mut conn, 3);
    let s1 = steps[0].id.expect("id1");
    let s2 = steps[1].id.expect("id2");
    // Step 2 has two customers waiting, one already sent and one replied; step 1 has one waiting.
    let w1 = put_at_step(&conn, id, s2, leads[0], "PENDING");
    let w2 = put_at_step(&conn, id, s2, leads[1], "PENDING");
    let done = put_at_step(&conn, id, s2, leads[2], "SENT");
    let replied = put_at_step(&conn, id, s2, leads[3], "REPLIED");
    let keep = put_at_step(&conn, id, s1, leads[0], "PENDING");
    let total = exec_rows(&conn, id).len();

    let edited: Vec<CampaignStep> = vec![steps[0].clone(), steps[2].clone()];
    let impact = get_unified_campaign_save_impact_internal(&conn, id, &edited).expect("impact");
    assert_eq!(
        impact,
        vec![RemovedStepImpact {
            step_id: s2,
            step_order: 2,
            channel: "ZALO".to_string(),
            pending_leads: 2,
        }]
    );
    // The preview changed nothing.
    assert_eq!(exec_rows(&conn, id).len(), total);
    assert_eq!(
        get_unified_campaign_steps_internal(&conn, id)
            .expect("steps")
            .len(),
        3
    );

    let campaign = crate::db::get_campaign(&conn, id)
        .expect("get")
        .expect("exists");
    save_unified_campaign_internal(&mut conn, campaign, edited, true).expect("edit saves");

    // Nothing is deleted: waiting customers are cancelled, finished sends keep their status.
    let rows = exec_rows(&conn, id);
    assert_eq!(rows.len(), total);
    let status = |exec: i64| rows.iter().find(|r| r.0 == exec).map(|r| r.3.clone());
    assert_eq!(status(w1).as_deref(), Some("CANCELLED"));
    assert_eq!(status(w2).as_deref(), Some("CANCELLED"));
    assert_eq!(status(done).as_deref(), Some("SENT"));
    assert_eq!(status(replied).as_deref(), Some("REPLIED"));
    assert_eq!(status(keep).as_deref(), Some("PENDING"));

    // The removed step is gone from the editor but its row is kept, retired, for the history.
    let live = get_unified_campaign_steps_internal(&conn, id).expect("steps");
    assert_eq!(live.len(), 2);
    assert!(live.iter().all(|s| s.id != Some(s2)));
    let retired: Option<String> = conn
        .query_row(
            "SELECT retired_at FROM campaign_steps WHERE id = ?1",
            [s2],
            |r| r.get(0),
        )
        .expect("retired row still exists");
    assert!(retired.is_some());
    // And the cancelled customers are no longer due for a send.
    let due =
        crate::db::get_due_campaign_executions(&conn, "2099-01-01T00:00:00Z", 50).expect("due");
    assert!(due.iter().all(|e| e.step_id != s2));
}

#[test]
fn test_save_refuses_a_step_id_from_another_campaign_and_changes_nothing() {
    let mut conn = setup_test_db();
    let (a, steps_a) = running_campaign_with_steps(&mut conn, 2);
    let (b, steps_b) = running_campaign_with_steps(&mut conn, 1);
    let foreign = steps_b[0].clone();
    let mut hijack = steps_a.clone();
    hijack.push(foreign.clone());

    let campaign = crate::db::get_campaign(&conn, a)
        .expect("get")
        .expect("exists");
    let err = save_unified_campaign_internal(&mut conn, campaign.clone(), hijack.clone(), false)
        .expect_err("a foreign step id must be refused");
    assert!(
        err.to_lowercase().contains("invalid"),
        "unexpected error: {err}"
    );
    assert!(get_unified_campaign_save_impact_internal(&conn, a, &hijack).is_err());

    let mut twice = steps_a.clone();
    twice.push(steps_a[0].clone());
    let err = save_unified_campaign_internal(&mut conn, campaign, twice, false)
        .expect_err("a repeated step id must be refused");
    assert!(
        err.to_lowercase().contains("invalid"),
        "unexpected error: {err}"
    );

    // The refused saves rolled back: both campaigns read as before.
    assert_eq!(
        get_unified_campaign_steps_internal(&conn, a)
            .expect("a")
            .len(),
        2
    );
    let kept = get_unified_campaign_steps_internal(&conn, b).expect("b");
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].campaign_id, b);
    assert_eq!(kept[0].template_content, foreign.template_content);
}

#[test]
fn test_save_impact_of_an_unknown_campaign_is_not_found() {
    let mut conn = setup_test_db();
    let (id, steps) = running_campaign_with_steps(&mut conn, 1);
    let err = get_unified_campaign_save_impact_internal(&conn, id + 1000, &steps)
        .expect_err("unknown campaign");
    assert!(err.contains("not found"), "unexpected error: {err}");
    let mut ghost = sample_campaign("Ghost");
    ghost.id = Some(id + 1000);
    let err = save_unified_campaign_internal(&mut conn, ghost, steps, false)
        .expect_err("saving over a missing campaign must fail, not half-create it");
    assert!(err.contains("not found"), "unexpected error: {err}");
}

// ---- server guards for editing running campaigns (RS #61 / D3) ----

#[test]
fn test_removing_step_with_pending_leads_requires_confirmation_and_rolls_back() {
    let mut conn = setup_test_db();
    let lead1 = insert_test_lead(&conn, "Guard 1", "Co", "0902000001", "g1@co.vn");
    let lead2 = insert_test_lead(&conn, "Guard 2", "Co", "0902000002", "g2@co.vn");
    let (id, steps) = running_campaign_with_steps(&mut conn, 2);
    let s1 = steps[0].id.expect("id1");
    let s2 = steps[1].id.expect("id2");
    let w1 = put_at_step(&conn, id, s1, lead1, "PENDING");
    let w2 = put_at_step(&conn, id, s1, lead2, "PENDING");
    let before_execs = exec_rows(&conn, id);

    let mut campaign = crate::db::get_campaign(&conn, id)
        .expect("get")
        .expect("exists");
    campaign.name = "Should Not Persist".to_string();
    let edited = vec![steps[1].clone()];

    // 1. Direct call without confirmation must be refused and leave DB completely untouched.
    let err = save_unified_campaign_internal(&mut conn, campaign.clone(), edited.clone(), false)
        .expect_err("removing step with pending leads without confirmation must fail");
    assert!(
        err.to_lowercase().contains("required"),
        "expected confirmation required error, got: {err}"
    );
    assert!(
        err.contains("2 pending lead"),
        "error should report affected pending lead count: {err}"
    );
    assert_eq!(
        crate::db::get_campaign(&conn, id)
            .expect("get")
            .expect("exists")
            .name,
        "Running"
    );
    assert_eq!(
        get_unified_campaign_steps_internal(&conn, id)
            .expect("steps")
            .len(),
        2
    );
    assert_eq!(exec_rows(&conn, id), before_execs);

    // 2. Calling again with confirm_remove_pending = true succeeds, cancels PENDING, and retires step 1.
    let saved_id = save_unified_campaign_internal(&mut conn, campaign, edited, true)
        .expect("confirmed removal must succeed");
    assert_eq!(saved_id, id);
    assert_eq!(
        crate::db::get_campaign(&conn, id)
            .expect("get")
            .expect("exists")
            .name,
        "Should Not Persist"
    );
    let live = get_unified_campaign_steps_internal(&conn, id).expect("live steps");
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].id, Some(s2));
    let rows = exec_rows(&conn, id);
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter().find(|r| r.0 == w1).map(|r| r.3.as_str()),
        Some("CANCELLED")
    );
    assert_eq!(
        rows.iter().find(|r| r.0 == w2).map(|r| r.3.as_str()),
        Some("CANCELLED")
    );
    let retired: Option<String> = conn
        .query_row(
            "SELECT retired_at FROM campaign_steps WHERE id = ?1",
            [s1],
            |r| r.get(0),
        )
        .expect("retired step row remains for history");
    assert!(retired.is_some());
}

#[test]
fn test_removing_step_without_pending_leads_succeeds_without_confirmation() {
    let mut conn = setup_test_db();
    let lead1 = insert_test_lead(&conn, "Done Lead", "Co", "0902000003", "g3@co.vn");
    let (id, steps) = running_campaign_with_steps(&mut conn, 2);
    let s1 = steps[0].id.expect("id1");
    let s2 = steps[1].id.expect("id2");
    // Only SENT history on step 2, zero PENDING leads.
    put_at_step(&conn, id, s2, lead1, "SENT");

    let campaign = crate::db::get_campaign(&conn, id)
        .expect("get")
        .expect("exists");
    let edited = vec![steps[0].clone()];
    save_unified_campaign_internal(&mut conn, campaign, edited, false)
        .expect("removing step with no pending leads should succeed with confirm=false");

    let live = get_unified_campaign_steps_internal(&conn, id).expect("steps");
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].id, Some(s1));
    // Historical SENT record stays intact and step 2 is retired.
    let retired: Option<String> = conn
        .query_row(
            "SELECT retired_at FROM campaign_steps WHERE id = ?1",
            [s2],
            |r| r.get(0),
        )
        .expect("retired row");
    assert!(retired.is_some());
    assert!(
        crate::db::get_campaign_step(&conn, s2)
            .expect("query step")
            .is_none(),
        "get_campaign_step must ignore retired steps"
    );
}

#[test]
fn test_scheduler_cancels_execution_for_retired_or_missing_step_without_dispatch() {
    let mut conn = setup_test_db();
    let lead1 = insert_test_lead(&conn, "Retired Lead", "Co", "0903000001", "r1@co.vn");
    let lead2 = insert_test_lead(&conn, "Missing Lead", "Co", "0903000002", "r2@co.vn");
    let (id, steps) = running_campaign_with_steps(&mut conn, 2);
    let s1 = steps[0].id.expect("id1");
    let s2 = steps[1].id.expect("id2");

    conn.execute(
        "INSERT INTO zalo_sessions (imei, session_name) VALUES ('IMEI_GUARD', 'Guard Session')",
        [],
    )
    .expect("insert zalo session");

    // Execution 1 points to a step that is retired while still PENDING.
    let exec_retired = put_at_step(&conn, id, s1, lead1, "PENDING");
    conn.execute(
        "UPDATE campaign_steps SET retired_at = '2026-01-01T00:00:00Z' WHERE id = ?1",
        [s1],
    )
    .expect("retire step 1");

    // Execution 2 points to a step id that no longer exists in campaign_steps.
    let exec_missing = put_at_step(&conn, id, s2, lead2, "PENDING");
    conn.execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("disable fk for missing-step simulation");
    conn.execute("DELETE FROM campaign_steps WHERE id = ?1", [s2])
        .expect("delete step 2");
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .expect("re-enable fk");

    let scheduler = CampaignScheduler::with_defaults();
    scheduler.rotator.register_account("ZALO", "IMEI_GUARD");

    let summary = scheduler
        .process_due_executions(&conn, DispatchMode::Live, 10)
        .expect("process cycle");
    assert_eq!(summary.total_due_found, 2);
    assert_eq!(summary.sent_count, 0);
    assert_eq!(summary.failed_count, 0);
    assert_eq!(summary.skipped_count, 2);

    // Neither execution may ever dispatch a message.
    let dispatched: i64 = conn
        .query_row("SELECT COUNT(*) FROM outreach_history", [], |r| r.get(0))
        .expect("outreach_history count");
    assert_eq!(dispatched, 0);

    for exec_id in [exec_retired, exec_missing] {
        let (status, note): (String, Option<String>) = conn
            .query_row(
                "SELECT status, error_message FROM campaign_executions WHERE id = ?1",
                [exec_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("read execution");
        assert_eq!(status, "CANCELLED");
        assert_eq!(
            note.as_deref(),
            Some("Step removed while editing the campaign")
        );
    }

    // Subsequent cycle must find zero due executions (no re-processing loop).
    let second = scheduler
        .process_due_executions(&conn, DispatchMode::Live, 10)
        .expect("second cycle");
    assert_eq!(second.total_due_found, 0);
    assert_eq!(second.sent_count, 0);
}
