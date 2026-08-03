use super::phase2a_probe::{
    classify_forward_fixture, classify_reaction_fixture, extract_mentions_fixture,
    generate_phase2a_probe_summary, utf16_slice,
};
use serde_json::Value;

#[test]
fn heart_fixture_is_classified_as_add() {
    let raw = include_str!("./fixtures/phase2a/reaction_add_heart.json");
    let val: Value = serde_json::from_str(raw).unwrap();
    let reaction = classify_reaction_fixture(&val).unwrap();

    assert_eq!(reaction.emoji, "❤️");
    assert_eq!(reaction.code, "/:heart");
    assert!(!reaction.removed);
    assert_eq!(reaction.actor_id, "user-101");
}

#[test]
fn haha_fixture_is_not_classified_as_remove() {
    let raw = include_str!("./fixtures/phase2a/reaction_add_haha.json");
    let val: Value = serde_json::from_str(raw).unwrap();
    let reaction = classify_reaction_fixture(&val).unwrap();

    assert_eq!(reaction.emoji, "😆");
    assert_eq!(reaction.code, "/-haha");
    assert!(!reaction.removed);
    assert_eq!(reaction.actor_id, "user-101");
}

#[test]
fn remove_fixture_identifies_exact_reaction() {
    let raw = include_str!("./fixtures/phase2a/reaction_remove_heart.json");
    let val: Value = serde_json::from_str(raw).unwrap();
    let reaction = classify_reaction_fixture(&val).unwrap();

    assert_eq!(reaction.emoji, "❤️");
    assert_eq!(reaction.code, "/:heart");
    assert!(reaction.removed);
    assert_eq!(reaction.actor_id, "user-101");
}

#[test]
fn multi_active_fixture_preserves_other_reaction() {
    let raw = include_str!("./fixtures/phase2a/reaction_multi_active.json");
    let val: Value = serde_json::from_str(raw).unwrap();
    let active_reactions = val.get("active_reactions").unwrap().as_array().unwrap();

    assert_eq!(active_reactions.len(), 2);
    let rx1 = classify_reaction_fixture(&active_reactions[0]).unwrap();
    let rx2 = classify_reaction_fixture(&active_reactions[1]).unwrap();

    assert_eq!(rx1.actor_id, "user-101");
    assert_eq!(rx1.emoji, "❤️");
    assert!(!rx1.removed);

    assert_eq!(rx2.actor_id, "user-102");
    assert_eq!(rx2.emoji, "😆");
    assert!(!rx2.removed);
}

#[test]
fn old_reactions_fixture_round_trips_actor_state() {
    let raw = include_str!("./fixtures/phase2a/reaction_old_reactions.json");
    let val: Value = serde_json::from_str(raw).unwrap();
    let reactions_by_msg = val.get("reactions_by_msg").unwrap().as_object().unwrap();

    assert!(reactions_by_msg.contains_key("msg-101"));
    assert!(reactions_by_msg.contains_key("msg-102"));

    let msg101_rxs = reactions_by_msg["msg-101"].as_array().unwrap();
    let rx101 = classify_reaction_fixture(&msg101_rxs[0]).unwrap();
    assert_eq!(rx101.actor_id, "user-101");
    assert_eq!(rx101.emoji, "❤️");

    let msg102_rxs = reactions_by_msg["msg-102"].as_array().unwrap();
    let rx102 = classify_reaction_fixture(&msg102_rxs[0]).unwrap();
    assert_eq!(rx102.actor_id, "user-102");
    assert_eq!(rx102.emoji, "👍");
}

#[test]
fn forward_fixture_exposes_provider_and_client_ids() {
    let raw = include_str!("./fixtures/phase2a/forward_text_echo.json");
    let val: Value = serde_json::from_str(raw).unwrap();
    let forward = classify_forward_fixture(&val).unwrap();

    assert_eq!(forward.provider_msg_id, "msg-fwd-001");
    assert_eq!(forward.client_msg_id, "cli-fwd-001");
    assert_eq!(forward.content_type, "text");
    assert_eq!(forward.original_msg_id.as_deref(), Some("msg-orig-888"));
    assert!(forward.is_forwarded);
}

#[test]
fn forward_matrix_classifies_every_content_type() {
    let raw = include_str!("./fixtures/phase2a/forward_content_matrix.json");
    let val: Value = serde_json::from_str(raw).unwrap();
    let items = val.as_array().unwrap();

    let mut classified_types = Vec::new();
    for item in items {
        let fwd = classify_forward_fixture(item).unwrap();
        assert!(fwd.is_forwarded);
        assert!(!fwd.provider_msg_id.is_empty());
        assert!(!fwd.client_msg_id.is_empty());
        classified_types.push(fwd.content_type);
    }

    assert_eq!(
        classified_types,
        vec!["text", "photo", "video", "file", "sticker", "link"]
    );
}

#[test]
fn mention_fixture_uses_utf16_offsets() {
    let raw = include_str!("./fixtures/phase2a/mention_vietnamese_emoji.json");
    let val: Value = serde_json::from_str(raw).unwrap();
    let mentions = extract_mentions_fixture(&val).unwrap();

    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].target_uid, "user-201");
    assert_eq!(mentions[0].utf16_offset, 5);
    assert_eq!(mentions[0].utf16_len, 13);
    assert_eq!(mentions[0].mention_text.as_deref(), Some("@Nguyễn Văn A"));
}

#[test]
fn mention_matrix_covers_required_unicode_cases() {
    let raw = include_str!("./fixtures/phase2a/mention_utf16_matrix.json");
    let val: Value = serde_json::from_str(raw).unwrap();
    let cases = val.as_array().unwrap();

    for case_item in cases {
        let case_name = case_item["case_name"].as_str().unwrap();
        let text = case_item["text"].as_str().unwrap();
        let offset = case_item["utf16_offset"].as_u64().unwrap() as usize;
        let len = case_item["utf16_len"].as_u64().unwrap() as usize;
        let expected = case_item["expected_mention_text"].as_str().unwrap();

        let sliced = utf16_slice(text, offset, len);
        assert_eq!(
            sliced.as_deref(),
            Some(expected),
            "Failed unicode case: {}",
            case_name
        );
    }
}

#[test]
fn phase2a_probe_fixtures_are_sanitized() {
    let fixtures = vec![
        serde_json::from_str::<Value>(include_str!("./fixtures/phase2a/reaction_add_heart.json"))
            .unwrap(),
        serde_json::from_str::<Value>(include_str!("./fixtures/phase2a/reaction_add_haha.json"))
            .unwrap(),
        serde_json::from_str::<Value>(include_str!(
            "./fixtures/phase2a/reaction_remove_heart.json"
        ))
        .unwrap(),
        serde_json::from_str::<Value>(include_str!(
            "./fixtures/phase2a/reaction_multi_active.json"
        ))
        .unwrap(),
        serde_json::from_str::<Value>(include_str!(
            "./fixtures/phase2a/reaction_old_reactions.json"
        ))
        .unwrap(),
        serde_json::from_str::<Value>(include_str!("./fixtures/phase2a/forward_text_echo.json"))
            .unwrap(),
        serde_json::from_str::<Value>(include_str!(
            "./fixtures/phase2a/forward_content_matrix.json"
        ))
        .unwrap(),
        serde_json::from_str::<Value>(include_str!(
            "./fixtures/phase2a/mention_vietnamese_emoji.json"
        ))
        .unwrap(),
        serde_json::from_str::<Value>(include_str!("./fixtures/phase2a/mention_utf16_matrix.json"))
            .unwrap(),
    ];

    let summary = generate_phase2a_probe_summary(&fixtures);
    assert!(
        summary.sanitized,
        "One or more fixtures contain sensitive keys or un-sanitized phone numbers!"
    );
    assert_eq!(summary.total_fixtures, 9);
}
