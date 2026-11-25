use super::campaign::{
    add_to_suppression_list, cancel_campaign, claim_next_recipients, create_campaign, get_campaign,
    is_in_quiet_hours, is_suppressed, pause_campaign, populate_recipients,
    recover_interrupted_campaign, refresh_campaign_counters, resume_campaign, start_campaign,
    update_recipient_status, CampaignRecipientInput, CreateCampaignRequest, RecipientEvaluation,
    WhatsAppCampaignEngine,
};
use rusqlite::Connection;
use std::collections::HashMap;

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute("CREATE TABLE leads (id INTEGER PRIMARY KEY)", [])
        .unwrap();
    conn.execute_batch(include_str!("../schema.sql")).unwrap();
    super::migration::apply_whatsapp_migrations(&conn).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at)
         VALUES (1, 'whatsapp', 'wa-test-1', '2026-08-26T00:00:00Z', '2026-08-26T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at)
         VALUES (2, 'whatsapp', 'wa-test-2', '2026-08-26T00:00:00Z', '2026-08-26T00:00:00Z')",
        [],
    )
    .unwrap();

    conn
}

#[test]
fn test_campaign_skips_suppressed_contact() {
    let conn = setup_test_db();
    add_to_suppression_list(&conn, "whatsapp", "84901234567", "opt_out", "manual").unwrap();

    let supp_reason = is_suppressed(&conn, "whatsapp", "84901234567").unwrap();
    assert_eq!(supp_reason, Some("opt_out".to_string()));
    assert_eq!(
        is_suppressed(&conn, "whatsapp", "84900000000").unwrap(),
        None
    );

    let engine = WhatsAppCampaignEngine::new();

    let result = engine.evaluate_recipient(&conn, "84901234567", "Hello [name]", &HashMap::new());
    assert_eq!(
        result,
        RecipientEvaluation::Skipped {
            reason: "opt_out_suppressed".to_string()
        }
    );

    // Also checks with leading '+' or local 090... format
    let result_plus =
        engine.evaluate_recipient(&conn, "+84901234567", "Hello [name]", &HashMap::new());
    assert_eq!(
        result_plus,
        RecipientEvaluation::Skipped {
            reason: "opt_out_suppressed".to_string()
        }
    );

    let result_local =
        engine.evaluate_recipient(&conn, "0901234567", "Hello [name]", &HashMap::new());
    assert_eq!(
        result_local,
        RecipientEvaluation::Skipped {
            reason: "opt_out_suppressed".to_string()
        }
    );

    // Non-suppressed contact is eligible
    let eligible = engine.evaluate_recipient(
        &conn,
        "0912345678",
        "Xin chào [name]",
        &[("name".to_string(), "Bình".to_string())]
            .into_iter()
            .collect(),
    );
    assert_eq!(
        eligible,
        RecipientEvaluation::Eligible {
            phone_e164: "84912345678".to_string(),
            jid: "84912345678@s.whatsapp.net".to_string(),
            rendered_text: "Xin chào Bình".to_string(),
        }
    );
}

#[test]
fn test_is_in_quiet_hours_cases() {
    // Midnight wrap-around: 22:00 -> 07:30
    assert!(is_in_quiet_hours("22:00", "07:30", "22:00"));
    assert!(is_in_quiet_hours("22:00", "07:30", "23:45"));
    assert!(is_in_quiet_hours("22:00", "07:30", "00:00"));
    assert!(is_in_quiet_hours("22:00", "07:30", "04:30"));
    assert!(is_in_quiet_hours("22:00", "07:30", "07:29"));
    assert!(!is_in_quiet_hours("22:00", "07:30", "07:30"));
    assert!(!is_in_quiet_hours("22:00", "07:30", "08:00"));
    assert!(!is_in_quiet_hours("22:00", "07:30", "12:00"));
    assert!(!is_in_quiet_hours("22:00", "07:30", "21:59"));

    // Day-time quiet hours: 12:00 -> 13:30
    assert!(is_in_quiet_hours("12:00", "13:30", "12:00"));
    assert!(is_in_quiet_hours("12:00", "13:30", "12:45"));
    assert!(!is_in_quiet_hours("12:00", "13:30", "13:30"));
    assert!(!is_in_quiet_hours("12:00", "13:30", "11:59"));
    assert!(!is_in_quiet_hours("12:00", "13:30", "14:00"));

    // Invalid format returns false gracefully
    assert!(!is_in_quiet_hours("invalid", "07:30", "12:00"));
}

#[test]
fn test_campaign_lifecycle_and_counters() {
    let conn = setup_test_db();

    // Add one suppressed number
    add_to_suppression_list(&conn, "whatsapp", "84909999999", "spam_report", "manual").unwrap();

    let req = CreateCampaignRequest {
        name: "Test Campaign".to_string(),
        template_id: None,
        assigned_account_ids: vec![1, 2],
        rate_limit_min_delay_secs: Some(10),
        rate_limit_max_delay_secs: Some(25),
        burst_limit_per_batch: Some(30),
        burst_pause_secs: Some(50),
        quiet_hours_enabled: Some(true),
        quiet_hours_start: Some("22:00".to_string()),
        quiet_hours_end: Some("07:30".to_string()),
        timezone: Some("Asia/Ho_Chi_Minh".to_string()),
    };

    let campaign_id = create_campaign(&conn, &req).unwrap();
    assert!(campaign_id > 0);

    // Populate recipients
    let recipients = vec![
        CampaignRecipientInput {
            lead_id: None,
            recipient_phone_raw: "0901234567".to_string(),
            template: Some("{Xin chào|Chào bạn} [name]".to_string()),
            lead_vars: Some(
                [("name".to_string(), "Khách A".to_string())]
                    .into_iter()
                    .collect(),
            ),
        },
        CampaignRecipientInput {
            lead_id: None,
            recipient_phone_raw: "0908765432".to_string(),
            template: Some("Hello [name]".to_string()),
            lead_vars: Some(
                [("name".to_string(), "Khách B".to_string())]
                    .into_iter()
                    .collect(),
            ),
        },
        CampaignRecipientInput {
            lead_id: None,
            recipient_phone_raw: "0909999999".to_string(), // Suppressed!
            template: Some("Hello".to_string()),
            lead_vars: None,
        },
        CampaignRecipientInput {
            lead_id: None,
            recipient_phone_raw: "invalid_phone".to_string(), // Invalid!
            template: Some("Hello".to_string()),
            lead_vars: None,
        },
    ];

    let count = populate_recipients(&conn, campaign_id, &recipients, None).unwrap();
    assert_eq!(count, 4);

    let counters = refresh_campaign_counters(&conn, campaign_id).unwrap();
    assert_eq!(counters.total_recipients, 4);
    assert_eq!(counters.skipped_count, 2); // 1 suppressed + 1 invalid
    assert_eq!(counters.sent_count, 0);

    // Claim next pending batch
    let claimed = claim_next_recipients(&conn, campaign_id, 10, Some(1)).unwrap();
    assert_eq!(claimed.len(), 2);
    assert_eq!(claimed[0].status, "running");
    assert_eq!(claimed[0].assigned_channel_account_id, Some(1));

    // Mark first as sent, second as failed
    update_recipient_status(&conn, claimed[0].id, "sent", None, None, None, None).unwrap();
    update_recipient_status(
        &conn,
        claimed[1].id,
        "failed",
        None,
        Some("Network timeout"),
        None,
        None,
    )
    .unwrap();

    let counters_after = refresh_campaign_counters(&conn, campaign_id).unwrap();
    assert_eq!(counters_after.sent_count, 1);
    assert_eq!(counters_after.failed_count, 1);
    assert_eq!(counters_after.skipped_count, 2);

    // Pause and Resume
    pause_campaign(&conn, campaign_id).unwrap();
    let status: String = conn
        .query_row(
            "SELECT status FROM whatsapp_outreach_campaigns WHERE id = ?",
            [campaign_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "paused");

    resume_campaign(&conn, campaign_id).unwrap();
    let status_resumed: String = conn
        .query_row(
            "SELECT status FROM whatsapp_outreach_campaigns WHERE id = ?",
            [campaign_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status_resumed, "queued");

    // Cancel campaign
    cancel_campaign(&conn, campaign_id).unwrap();
    let status_cancelled: String = conn
        .query_row(
            "SELECT status FROM whatsapp_outreach_campaigns WHERE id = ?",
            [campaign_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status_cancelled, "cancelled");
}

#[test]
fn test_interrupted_campaign_recovery() {
    let conn = setup_test_db();
    let req = CreateCampaignRequest {
        name: "Crash Recovery Test".to_string(),
        template_id: None,
        assigned_account_ids: vec![1],
        rate_limit_min_delay_secs: None,
        rate_limit_max_delay_secs: None,
        burst_limit_per_batch: None,
        burst_pause_secs: None,
        quiet_hours_enabled: None,
        quiet_hours_start: None,
        quiet_hours_end: None,
        timezone: None,
    };
    let campaign_id = create_campaign(&conn, &req).unwrap();

    let recipients = vec![
        CampaignRecipientInput {
            lead_id: None,
            recipient_phone_raw: "0911111111".to_string(),
            template: Some("Hello".to_string()),
            lead_vars: None,
        },
        CampaignRecipientInput {
            lead_id: None,
            recipient_phone_raw: "0922222222".to_string(),
            template: Some("Hello".to_string()),
            lead_vars: None,
        },
    ];
    populate_recipients(&conn, campaign_id, &recipients, None).unwrap();

    // Claim so both are in 'running'
    let claimed = claim_next_recipients(&conn, campaign_id, 10, Some(1)).unwrap();
    assert_eq!(claimed.len(), 2);
    assert_eq!(claimed[0].status, "running");

    // Simulate interrupted execution and recover
    let recovered = recover_interrupted_campaign(&conn, campaign_id).unwrap();
    assert_eq!(recovered, 2);

    // Both should now be 'pending' and unassigned, ready to be reclaimed
    let reclaimed = claim_next_recipients(&conn, campaign_id, 10, Some(2)).unwrap();
    assert_eq!(reclaimed.len(), 2);
    assert_eq!(reclaimed[0].assigned_channel_account_id, Some(2));
}

#[test]
fn test_create_campaign_rejects_blank_name_and_invalid_accounts() {
    let conn = setup_test_db();

    let blank_name = CreateCampaignRequest {
        name: "   ".to_string(),
        template_id: None,
        assigned_account_ids: vec![1],
        rate_limit_min_delay_secs: None,
        rate_limit_max_delay_secs: None,
        burst_limit_per_batch: None,
        burst_pause_secs: None,
        quiet_hours_enabled: None,
        quiet_hours_start: None,
        quiet_hours_end: None,
        timezone: None,
    };
    let blank_err = create_campaign(&conn, &blank_name).unwrap_err().to_string();
    assert!(
        blank_err.contains("Invalid campaign") && blank_err.contains("must not be empty"),
        "expected invalid_request error for blank name, got: {blank_err}"
    );

    for bad_accounts in [vec![], vec![0], vec![-1, 0], vec![1, 0], vec![1, -1]] {
        let req = CreateCampaignRequest {
            name: "No Accounts".to_string(),
            template_id: None,
            assigned_account_ids: bad_accounts,
            rate_limit_min_delay_secs: None,
            rate_limit_max_delay_secs: None,
            burst_limit_per_batch: None,
            burst_pause_secs: None,
            quiet_hours_enabled: None,
            quiet_hours_start: None,
            quiet_hours_end: None,
            timezone: None,
        };
        let err = create_campaign(&conn, &req).unwrap_err().to_string();
        assert!(
            err.contains("Invalid campaign") && err.contains("required"),
            "expected invalid_request error for bad accounts, got: {err}"
        );
    }
}

#[test]
fn test_start_and_resume_reject_campaign_without_valid_account() {
    let conn = setup_test_db();

    // Seed draft/paused campaigns with no valid assigned account (e.g. legacy rows).
    for (status, accounts_json) in [
        ("draft", "[]"),
        ("draft", "[0]"),
        ("draft", "[1, 0]"),
        ("paused", "[]"),
        ("paused", "[1, 0]"),
    ] {
        conn.execute(
            "INSERT INTO whatsapp_outreach_campaigns (
                name, status, assigned_account_ids_json, created_at, updated_at
             ) VALUES ('Legacy Empty', ?1, ?2, '2026-11-25T00:00:00Z', '2026-11-25T00:00:00Z')",
            rusqlite::params![status, accounts_json],
        )
        .unwrap();
        let campaign_id = conn.last_insert_rowid();

        let err = if status == "paused" {
            resume_campaign(&conn, campaign_id).unwrap_err().to_string()
        } else {
            start_campaign(&conn, campaign_id).unwrap_err().to_string()
        };
        assert!(
            err.contains("Invalid campaign") && err.contains("required"),
            "expected invalid_request wording when starting/resuming without valid account, got: {err}"
        );

        let record = get_campaign(&conn, campaign_id).unwrap();
        assert_eq!(record.status, status);
    }

    // Valid campaign starts cleanly.
    let valid_id = create_campaign(
        &conn,
        &CreateCampaignRequest {
            name: "Valid WA".to_string(),
            template_id: None,
            assigned_account_ids: vec![1],
            rate_limit_min_delay_secs: None,
            rate_limit_max_delay_secs: None,
            burst_limit_per_batch: None,
            burst_pause_secs: None,
            quiet_hours_enabled: None,
            quiet_hours_start: None,
            quiet_hours_end: None,
            timezone: None,
        },
    )
    .unwrap();
    start_campaign(&conn, valid_id).unwrap();
    assert_eq!(get_campaign(&conn, valid_id).unwrap().status, "queued");

    // Missing campaign returns not-found row error.
    let missing_err = start_campaign(&conn, 999_999).unwrap_err();
    assert!(matches!(missing_err, rusqlite::Error::QueryReturnedNoRows));
}
