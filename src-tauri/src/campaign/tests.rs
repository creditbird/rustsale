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
             status TEXT NOT NULL,
             error_message TEXT,
             sent_at TEXT NOT NULL,
             session_imei TEXT NOT NULL
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
             created_at TEXT NOT NULL
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
    let lead1 = insert_test_lead(&conn, "Nguyễn Văn A", "Công ty ABC", "0901234567", "a@abc.com");
    let lead2 = insert_test_lead(&conn, "Trần Thị B", "Công ty XYZ", "0907654321", "b@xyz.com");

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
        },
    ];

    let campaign_id = save_unified_campaign_internal(&mut conn, campaign, steps)
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
    let journey1 = crate::db::get_lead_journey_state(&conn, lead1).unwrap().unwrap();
    assert_eq!(journey1.active_campaign_id, Some(campaign_id));
    assert_eq!(journey1.current_stage, "ENROLLED");
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

    let template = "{Kính gửi|Chào anh|Chào bạn} {{name}} từ {{company}}! Số {{phone}} của anh đã được duyệt.";
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
        },
    ];

    let campaign_id = save_unified_campaign_internal(&mut conn, campaign, steps).unwrap();
    let enrolled =
        enroll_leads_into_campaign_internal(&conn, campaign_id, &[lead_id]).unwrap();
    assert_eq!(enrolled, 1);

    let scheduler = CampaignScheduler::with_defaults();
    scheduler.rotator.register_account("ZALO", "IMEI_MAIN");
    scheduler.rotator.register_account("WHATSAPP", "+84933445566");

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
        },
    ];

    let campaign_id = save_unified_campaign_internal(&mut conn, campaign, steps).unwrap();
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
