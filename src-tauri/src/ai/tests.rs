use crate::ai::evidence::{EvidenceExtractor, LeadEvidenceContext, ProductEvidence};
use crate::ai::gateway::{LlmConfig, LlmGateway, LlmProvider};
use crate::ai::generator::SmartReplyGenerator;
use crate::ai::intent::{CustomerIntent, IntentClassifier};
use crate::ai::policy::{PolicyDecision, PolicyGate};
use crate::ai::commands::{
    generate_smart_reply_suggestions_internal, get_lead_evidence_context_internal,
    get_pending_ai_proposals_internal, review_ai_proposal_internal,
};
use rusqlite::Connection;

fn setup_test_db() -> Connection {
    crate::db::init_db(":memory:").expect("Failed to init in-memory test database")
}

fn create_mock_lead(conn: &Connection) -> i32 {
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, about, address, currency)
         VALUES ('Nguyễn Văn A', 'Công ty ABC Tech', 'QUALIFIED', 15000000.0, '2026-09-02', 'Sales Rep', 'vana@abc.vn', '0912345678', 'Quan tâm gói Pro', 'Hà Nội', 'VND')",
        [],
    ).unwrap();
    conn.last_insert_rowid() as i32
}

#[test]
fn test_intent_classification_vietnamese_heuristics() {
    // 1. AskingPrice
    let r1 = IntentClassifier::classify_heuristics("Giá bao nhiêu em?");
    assert_eq!(r1.intent, CustomerIntent::AskingPrice);
    assert!(r1.confidence >= 0.85);

    let r2 = IntentClassifier::classify_heuristics("Báo giá cho anh gói doanh nghiệp nhé");
    assert_eq!(r2.intent, CustomerIntent::AskingPrice);

    let r3 = IntentClassifier::classify_heuristics("Gói này bn tiền vậy shop?");
    assert_eq!(r3.intent, CustomerIntent::AskingPrice);

    // 2. ScheduleMeeting
    let r4 = IntentClassifier::classify_heuristics("Mai qua xem hàng được không?");
    assert_eq!(r4.intent, CustomerIntent::ScheduleMeeting);
    assert!(r4.confidence >= 0.85);

    let r5 = IntentClassifier::classify_heuristics("Chiều nay mình ghé shop xem demo nhé");
    assert_eq!(r5.intent, CustomerIntent::ScheduleMeeting);

    let r6 = IntentClassifier::classify_heuristics("Sáng mai hẹn gặp ở văn phòng bên bạn");
    assert_eq!(r6.intent, CustomerIntent::ScheduleMeeting);

    // 3. Complaint
    let r7 = IntentClassifier::classify_heuristics("Hàng lỗi rồi shop ơi");
    assert_eq!(r7.intent, CustomerIntent::Complaint);
    assert!(r7.confidence >= 0.85);

    let r8 = IntentClassifier::classify_heuristics("Sản phẩm bị hỏng rồi, yêu cầu hoàn tiền gấp!");
    assert_eq!(r8.intent, CustomerIntent::Complaint);

    let r9 = IntentClassifier::classify_heuristics("Chất lượng quá tệ, làm ăn lừa đảo");
    assert_eq!(r9.intent, CustomerIntent::Complaint);

    // 4. Unsubscribe
    let r10 = IntentClassifier::classify_heuristics("Đừng gửi tin nhắn nữa");
    assert_eq!(r10.intent, CustomerIntent::Unsubscribe);
    assert!(r10.confidence >= 0.90);

    let r11 = IntentClassifier::classify_heuristics("Hủy nhận tin giúp tôi, phiền quá");
    assert_eq!(r11.intent, CustomerIntent::Unsubscribe);

    let r12 = IntentClassifier::classify_heuristics("Stop spam, xóa số tôi đi");
    assert_eq!(r12.intent, CustomerIntent::Unsubscribe);

    // 5. ProductInquiry
    let r13 = IntentClassifier::classify_heuristics("Bên bạn có mẫu đầm dạ hội màu đen không?");
    assert_eq!(r13.intent, CustomerIntent::ProductInquiry);

    let r14 = IntentClassifier::classify_heuristics("Tư vấn giúp mình thông tin sản phẩm và tính năng đồng bộ Zalo");
    assert_eq!(r14.intent, CustomerIntent::ProductInquiry);

    // 6. GeneralInquiry
    let r15 = IntentClassifier::classify_heuristics("Alo shop ơi");
    assert_eq!(r15.intent, CustomerIntent::GeneralInquiry);

    let r16 = IntentClassifier::classify_heuristics("Chào bạn");
    assert_eq!(r16.intent, CustomerIntent::GeneralInquiry);
}

#[test]
fn test_evidence_extraction_from_db() {
    let conn = setup_test_db();
    let lead_id = create_mock_lead(&conn);

    // Add Deal
    conn.execute(
        "INSERT INTO deals (lead_id, title, value, stage, created_at, currency)
         VALUES (?1, 'Gói Phần Mềm Enterprise', 25000000.0, 'PROPOSAL', '2026-09-02', 'VND')",
        rusqlite::params![lead_id],
    ).unwrap();

    // Add Category
    let cat_id = crate::db::create_category(&conn, "Doanh Nghiệp B2B").unwrap();
    crate::db::add_lead_to_category(&conn, lead_id, cat_id).unwrap();

    // Add Journey State
    let _ = crate::db::get_or_create_lead_journey_state(&conn, lead_id).unwrap();
    crate::db::update_lead_journey_state(
        &conn,
        lead_id,
        "EVALUATION",
        None,
        None,
        None,
        Some("asking_price"),
        Some(0.95),
        Some("Đã gửi demo"),
    ).unwrap();

    // Extract Context
    let context = EvidenceExtractor::extract_lead_context(&conn, lead_id, None)
        .expect("extract_lead_context failed");

    assert_eq!(context.lead_id, lead_id);
    assert_eq!(context.lead_name, "Nguyễn Văn A");
    assert_eq!(context.lead_company, "Công ty ABC Tech");
    assert_eq!(context.deal_stage.as_deref(), Some("PROPOSAL"));
    assert_eq!(context.deal_value, Some(25000000.0));
    assert_eq!(context.journey_stage.as_deref(), Some("EVALUATION"));
    assert!(context.categories.contains(&"Doanh Nghiệp B2B".to_string()));
    assert!(!context.known_products.is_empty());
    assert_eq!(context.known_products[0].name, "Gói Phần Mềm Enterprise");

    // Summary formatting check
    let summary = context.to_context_summary();
    assert!(summary.contains("Nguyễn Văn A"));
    assert!(summary.contains("Công ty ABC Tech"));
    assert!(summary.contains("PROPOSAL"));
}

#[test]
fn test_policy_gate_decisions() {
    let policy_gate = PolicyGate::with_default_config();

    let context = LeadEvidenceContext {
        lead_id: 1,
        lead_name: "Trần Thị B".to_string(),
        lead_company: "Công ty XYZ".to_string(),
        lead_phone: "0988776655".to_string(),
        lead_email: "b@xyz.com".to_string(),
        lead_status: "CONTACTED".to_string(),
        lead_value: 5000000.0,
        lead_currency: "VND".to_string(),
        lead_about: None,
        lead_address: None,
        deal_stage: Some("CONTACTED".to_string()),
        deal_value: Some(5000000.0),
        deal_title: Some("Gói Cơ Bản".to_string()),
        journey_stage: Some("DISCOVERY".to_string()),
        active_campaign_name: None,
        recent_messages: vec![],
        known_products: vec![ProductEvidence {
            name: "Gói Cơ Bản".to_string(),
            price: 5000000.0,
            currency: "VND".to_string(),
            category: None,
            description: None,
        }],
        relationship_status: None,
        categories: vec![],
        verified_channels: vec!["zalo".to_string()],
    };

    // 1. Forbidden / Offensive Word -> REJECT
    let profanity_reply = "Dạ chào anh, đồ lừa đảo này sẽ liên hệ lại anh sau nhé đm";
    let decision = policy_gate.evaluate_proposed_reply(
        profanity_reply,
        &CustomerIntent::GeneralInquiry,
        0.95,
        &context,
    );
    assert!(matches!(decision, PolicyDecision::Reject { .. }));

    // 2. Unsubscribe with pushy promotional content -> REJECT
    let pushy_unsubscribe = "Dạ bên em đang có giảm giá sốc 50% mua ngay kẻo lỡ!";
    let decision2 = policy_gate.evaluate_proposed_reply(
        pushy_unsubscribe,
        &CustomerIntent::Unsubscribe,
        0.95,
        &context,
    );
    assert!(matches!(decision2, PolicyDecision::Reject { .. }));

    // 3. Unsubscribe with polite apology -> APPROVE_AUTO
    let polite_unsubscribe = "Dạ em chào anh/chị, em vô cùng xin lỗi vì đã làm phiền. Em đã ngừng gửi tin nhắn và xóa số khỏi danh sách rồi ạ.";
    let decision3 = policy_gate.evaluate_proposed_reply(
        polite_unsubscribe,
        &CustomerIntent::Unsubscribe,
        0.95,
        &context,
    );
    assert_eq!(decision3, PolicyDecision::ApproveAuto);

    // 4. Complaint intent -> REQUIRE_REVIEW
    let complaint_reply = "Dạ em rất xin lỗi anh, em sẽ chuyển kỹ thuật hỗ trợ ngay ạ.";
    let decision4 = policy_gate.evaluate_proposed_reply(
        complaint_reply,
        &CustomerIntent::Complaint,
        0.95,
        &context,
    );
    assert!(matches!(decision4, PolicyDecision::RequireReview { .. }));

    // 5. Low confidence -> REQUIRE_REVIEW
    let normal_reply = "Dạ em chào anh, bên em có thể hỗ trợ gì cho mình hôm nay ạ?";
    let decision5 = policy_gate.evaluate_proposed_reply(
        normal_reply,
        &CustomerIntent::GeneralInquiry,
        0.60, // Below 0.75 threshold
        &context,
    );
    assert!(matches!(decision5, PolicyDecision::RequireReview { .. }));

    // 6. Normal high confidence general reply -> APPROVE_AUTO
    let decision6 = policy_gate.evaluate_proposed_reply(
        normal_reply,
        &CustomerIntent::GeneralInquiry,
        0.95,
        &context,
    );
    assert_eq!(decision6, PolicyDecision::ApproveAuto);
}

#[tokio::test]
async fn test_smart_reply_generator_and_offline_variants() {
    let policy_gate = PolicyGate::with_default_config();
    let gateway = LlmGateway::new(LlmConfig {
        provider: LlmProvider::Mock,
        ..Default::default()
    });

    let context = LeadEvidenceContext {
        lead_id: 10,
        lead_name: "Lê Hoàng C".to_string(),
        lead_company: "Tập đoàn Minh Khang".to_string(),
        lead_phone: "0909123456".to_string(),
        lead_email: "hoangc@minhkhang.com".to_string(),
        lead_status: "IN_DISCUSSION".to_string(),
        lead_value: 20000000.0,
        lead_currency: "VND".to_string(),
        lead_about: Some("Cần triển khai gấp trong tháng 9".to_string()),
        lead_address: Some("TP.HCM".to_string()),
        deal_stage: Some("NEGOTIATION".to_string()),
        deal_value: Some(20000000.0),
        deal_title: Some("Hợp Đồng Phần Mềm Bán Hàng".to_string()),
        journey_stage: Some("PROPOSAL".to_string()),
        active_campaign_name: Some("Chiến dịch Q3".to_string()),
        recent_messages: vec![],
        known_products: vec![ProductEvidence {
            name: "Hợp Đồng Phần Mềm Bán Hàng".to_string(),
            price: 20000000.0,
            currency: "VND".to_string(),
            category: Some("Phần mềm".to_string()),
            description: None,
        }],
        relationship_status: None,
        categories: vec!["Phần mềm".to_string()],
        verified_channels: vec!["zalo".to_string()],
    };

    let intent = IntentClassifier::classify_heuristics("Báo giá cho anh hợp đồng phần mềm nhé");
    assert_eq!(intent.intent, CustomerIntent::AskingPrice);

    let output = SmartReplyGenerator::generate(
        &context,
        &intent,
        Some("Nhấn mạnh chiết khấu 10% nếu ký trong tuần này"),
        &gateway,
        &policy_gate,
    )
    .await
    .expect("generate failed");

    assert_eq!(output.lead_id, 10);
    assert_eq!(output.variants.len(), 3);
    assert_eq!(output.variants[0].variant_type, "brief");
    assert_eq!(output.variants[1].variant_type, "detailed");
    assert_eq!(output.variants[2].variant_type, "promotional");

    for v in &output.variants {
        assert!(!v.content.is_empty());
        assert!(v.confidence > 0.80);
    }
}

#[tokio::test]
async fn test_ai_commands_end_to_end() {
    let conn = setup_test_db();
    let lead_id = create_mock_lead(&conn);
    let gateway = LlmGateway::from_env();
    let policy_gate = PolicyGate::with_default_config();

    // 1. Test generate_smart_reply_suggestions_internal
    let output = generate_smart_reply_suggestions_internal(
        &conn,
        lead_id,
        None,
        Some("Tư vấn giải pháp cho công ty".to_string()),
        &gateway,
        &policy_gate,
    )
    .await
    .expect("generate_smart_reply_suggestions_internal failed");

    assert_eq!(output.lead_id, lead_id);
    assert_eq!(output.variants.len(), 3);

    // Verify AI evidence log was persisted in DB
    let logs = crate::db::get_ai_evidence_logs_by_lead(&conn, lead_id)
        .expect("get_ai_evidence_logs_by_lead failed");
    assert!(!logs.is_empty());
    assert_eq!(logs[0].action_type, "GENERATE_SMART_REPLY");

    // 2. Test get_lead_evidence_context_internal
    let ctx = get_lead_evidence_context_internal(&conn, lead_id)
        .expect("get_lead_evidence_context_internal failed");
    assert_eq!(ctx.lead_name, "Nguyễn Văn A");

    // 3. Test AI action proposals
    let prop_id = crate::db::create_ai_action_proposal(
        &conn,
        Some(lead_id),
        None,
        "zalo",
        "Chào anh A, em gửi báo giá nhé",
        "Khách hỏi giá gói Pro",
    )
    .expect("create_ai_action_proposal failed");

    let proposals = get_pending_ai_proposals_internal(&conn)
        .expect("get_pending_ai_proposals_internal failed");
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].id, Some(prop_id));

    // 4. Test review_ai_proposal_internal
    review_ai_proposal_internal(&conn, prop_id, "APPROVED", "Trưởng Phòng Sale")
        .expect("review_ai_proposal_internal failed");

    let updated_prop = crate::db::get_ai_action_proposal(&conn, prop_id)
        .expect("get_ai_action_proposal failed")
        .expect("proposal not found");
    assert_eq!(updated_prop.status, "APPROVED");
    assert_eq!(updated_prop.reviewed_by.as_deref(), Some("Trưởng Phòng Sale"));
}
