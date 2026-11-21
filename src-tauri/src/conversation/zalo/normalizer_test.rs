use super::normalizer::{normalize_listener_event, ReceiptKind, ZaloMutation};
use serde_json::json;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::{
    DeliveredMessage, EventReactions, GroupDeliveredMessage, GroupMessage, GroupSeenMessage,
    Message, Reaction, SeenMessage, TDeliveredMessage, TGroupDeliveredMessage, TGroupMessage,
    TGroupSeenMessage, TMention, TMessage, TPropertyExt, TReaction, TReactionContent, TReactionMsg,
    TUndo, TUndoContent, TUserSeenMessage, Undo, UserDeliveredMessage, UserMessage,
    UserSeenMessage,
};

fn base_message(uid_from: &str, id_to: &str, msg_id: &str, content: serde_json::Value) -> TMessage {
    serde_json::from_value(json!({
        "actionId": "action-1",
        "msgId": msg_id,
        "cliMsgId": format!("client-{msg_id}"),
        "msgType": "webchat",
        "uidFrom": uid_from,
        "idTo": id_to,
        "dName": "Probe Sender",
        "ts": "1785456000000",
        "status": 1,
        "content": content,
        "notify": "",
        "ttl": 0,
        "userId": "",
        "uin": "",
        "propertyExt": null,
        "paramsExt": {"countUnread": 1, "containType": 0, "platformType": 1},
        "cmd": 501,
        "st": 0,
        "at": 0,
        "realMsgId": format!("real-{msg_id}"),
        "quote": null
    }))
    .unwrap()
}

#[test]
fn inbound_direct_message_links_peer_as_sender_and_participant() {
    let event = ListenerEvent::Message(Box::new(Message::User(UserMessage::new(
        "self-uid",
        base_message("peer-uid", "0", "message-1", json!("hello")),
    ))));

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Message(message) = &mutations[0] else {
        panic!("expected message mutation");
    };
    assert_eq!(message.external_thread_id, "peer-uid");
    assert_eq!(
        message.direction,
        crate::conversation::types::MessageDirection::Inbound
    );
    assert!(message.sender_is_participant);
    assert_eq!(
        message
            .participant
            .as_ref()
            .map(|identity| identity.external_id.as_str()),
        Some("peer-uid")
    );
}

#[test]
fn outbound_direct_message_does_not_use_account_name_for_peer_identity() {
    let event = ListenerEvent::Message(Box::new(Message::User(UserMessage::new(
        "self-uid",
        base_message("0", "peer-uid", "message-2", json!("hello")),
    ))));

    let mutations = normalize_listener_event("imei-a", Some("Probe Sender"), event).unwrap();
    let ZaloMutation::Message(message) = &mutations[0] else {
        panic!("expected message mutation");
    };

    assert_eq!(message.external_thread_id, "peer-uid");
    assert_eq!(
        message.direction,
        crate::conversation::types::MessageDirection::Outbound
    );
    assert!(!message.sender_is_participant);
    assert_eq!(
        message
            .participant
            .as_ref()
            .map(|identity| identity.external_id.as_str()),
        Some("peer-uid")
    );
    assert_eq!(
        message
            .participant
            .as_ref()
            .and_then(|identity| identity.display_name.as_deref()),
        None
    );
}

#[test]
fn outbound_group_message_does_not_assign_customer_sender_identity() {
    let raw = TGroupMessage {
        base: base_message("0", "group-1", "message-2", json!("hello group")),
        mentions: None,
    };
    let event =
        ListenerEvent::Message(Box::new(Message::Group(GroupMessage::new("self-uid", raw))));

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Message(message) = &mutations[0] else {
        panic!("expected message mutation");
    };
    assert_eq!(message.external_thread_id, "group-1");
    assert_eq!(
        message.direction,
        crate::conversation::types::MessageDirection::Outbound
    );
    assert!(!message.sender_is_participant);
    assert!(message.participant.is_none());
}

#[test]
fn numeric_quote_ids_preserve_quote_owner_identity() {
    let raw: TMessage = serde_json::from_value(json!({
        "actionId": "action-quote",
        "msgId": "message-quote",
        "cliMsgId": "client-message-quote",
        "msgType": "webchat",
        "uidFrom": "peer-uid",
        "idTo": "group-quote",
        "dName": "Probe Sender",
        "ts": "1785456000000",
        "status": 1,
        "content": "reply",
        "notify": "",
        "ttl": 0,
        "userId": "",
        "uin": "",
        "propertyExt": null,
        "paramsExt": {"countUnread": 1, "containType": 0, "platformType": 1},
        "cmd": 501,
        "st": 0,
        "at": 0,
        "realMsgId": "real-message-quote",
        "quote": {
            "ownerId": "quote-owner-uid",
            "cliMsgId": 12345,
            "globalMsgId": 67890,
            "cliMsgType": 1,
            "ts": 1785455000000_u64,
            "msg": "quoted",
            "attach": "",
            "fromD": "Quote Owner",
            "ttl": 0
        }
    }))
    .unwrap();
    let event = ListenerEvent::Message(Box::new(Message::Group(GroupMessage::new(
        "self-uid",
        TGroupMessage {
            base: raw,
            mentions: None,
        },
    ))));

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Message(message) = &mutations[0] else {
        panic!("expected message mutation");
    };
    let reply = message.reply.as_ref().expect("quote must normalize");
    assert_eq!(reply.provider_message_id, "67890");
    assert_eq!(reply.provider_client_message_id, "12345");
    assert_eq!(reply.sender_external_id, "quote-owner-uid");
}

#[test]
fn reaction_event_normalizes_correctly() {
    let reaction_data = TReaction {
        action_id: "act-1".to_string(),
        msg_id: "msg-10".to_string(),
        cli_msg_id: "cli-10".to_string(),
        msg_type: "1".to_string(),
        uid_from: "peer-uid".to_string(),
        id_to: "0".to_string(),
        d_name: Some("Peer".to_string()),
        content: TReactionContent {
            r_msg: vec![TReactionMsg {
                g_msg_id: "gmsg-10".to_string(),
                c_msg_id: "cmsg-10".to_string(),
                msg_type: 1,
            }],
            r_icon: EventReactions::Heart,
            r_type: 1,
            source: 1,
        },
        ts: "1785456000000".to_string(),
        ttl: 0,
    };
    let reaction = Reaction::new("self-uid", reaction_data, false);
    let event = ListenerEvent::Reaction(reaction);

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    assert_eq!(mutations.len(), 1);
    let ZaloMutation::Reaction(norm_reaction) = &mutations[0] else {
        panic!("expected reaction mutation");
    };
    assert_eq!(norm_reaction.provider_account_key, "imei-a");
    assert_eq!(norm_reaction.external_thread_id, "peer-uid");
    assert_eq!(norm_reaction.provider_event_id.as_deref(), Some("msg-10"));
    assert!(!norm_reaction.is_group);
    assert_eq!(norm_reaction.provider_message_id, "gmsg-10");
    assert_eq!(norm_reaction.provider_client_message_id, "cmsg-10");
    assert_eq!(norm_reaction.actor_external_id, "peer-uid");
    assert!(!norm_reaction.removed);
}

#[test]
fn haha_reaction_type_zero_is_addition() {
    let reaction_data = TReaction {
        action_id: "act-haha".to_string(),
        msg_id: "event-haha".to_string(),
        cli_msg_id: "cli-haha".to_string(),
        msg_type: "1".to_string(),
        uid_from: "self-uid".to_string(),
        id_to: "peer-uid".to_string(),
        d_name: Some("Self".to_string()),
        content: TReactionContent {
            r_msg: vec![TReactionMsg {
                g_msg_id: "gmsg-haha".to_string(),
                c_msg_id: "cmsg-haha".to_string(),
                msg_type: 1,
            }],
            r_icon: EventReactions::Haha,
            r_type: 0,
            source: 6,
        },
        ts: "1786269310941".to_string(),
        ttl: 0,
    };
    let event = ListenerEvent::Reaction(Reaction::new("self-uid", reaction_data, false));

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Reaction(normalized) = &mutations[0] else {
        panic!("expected reaction mutation");
    };

    assert_eq!(normalized.reaction, ":>");
    assert!(!normalized.removed);
}

#[test]
fn empty_reaction_icon_marks_removal_even_with_non_negative_type() {
    let reaction_data = TReaction {
        action_id: "act-empty-remove".to_string(),
        msg_id: "event-empty-remove".to_string(),
        cli_msg_id: "cli-empty-remove".to_string(),
        msg_type: "1".to_string(),
        uid_from: "self-uid".to_string(),
        id_to: "peer-uid".to_string(),
        d_name: Some("Self".to_string()),
        content: TReactionContent {
            r_msg: vec![TReactionMsg {
                g_msg_id: "gmsg-empty-remove".to_string(),
                c_msg_id: "cmsg-empty-remove".to_string(),
                msg_type: 1,
            }],
            r_icon: EventReactions::Unknown(String::new()),
            r_type: 0,
            source: 6,
        },
        ts: "1786269311941".to_string(),
        ttl: 0,
    };
    let event = ListenerEvent::Reaction(Reaction::new("self-uid", reaction_data, false));

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Reaction(normalized) = &mutations[0] else {
        panic!("expected reaction mutation");
    };

    assert_eq!(normalized.reaction, "");
    assert!(normalized.removed);
}

#[test]
fn negative_reaction_type_marks_removal() {
    let reaction_data = TReaction {
        action_id: "act-remove".to_string(),
        msg_id: "event-remove".to_string(),
        cli_msg_id: "cli-remove".to_string(),
        msg_type: "1".to_string(),
        uid_from: "self-uid".to_string(),
        id_to: "peer-uid".to_string(),
        d_name: Some("Self".to_string()),
        content: TReactionContent {
            r_msg: vec![TReactionMsg {
                g_msg_id: "gmsg-remove".to_string(),
                c_msg_id: "cmsg-remove".to_string(),
                msg_type: 1,
            }],
            r_icon: EventReactions::Unknown(String::new()),
            r_type: -1,
            source: 6,
        },
        ts: "1786269311941".to_string(),
        ttl: 0,
    };
    let event = ListenerEvent::Reaction(Reaction::new("self-uid", reaction_data, false));

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Reaction(normalized) = &mutations[0] else {
        panic!("expected reaction mutation");
    };

    assert!(normalized.removed);
}

#[test]
fn unknown_reaction_icon_preserves_raw_provider_value() {
    let reaction_data: TReaction = serde_json::from_value(json!({
        "actionId": "act-unknown-reaction",
        "msgId": "event-unknown-reaction",
        "cliMsgId": "cli-unknown-reaction",
        "msgType": "1",
        "uidFrom": "peer-uid",
        "idTo": "0",
        "dName": "Peer",
        "content": {
            "rMsg": [{
                "gMsgID": "gmsg-unknown-reaction",
                "cMsgID": "cmsg-unknown-reaction",
                "msgType": 1
            }],
            "rIcon": "/:new-haha",
            "rType": 1,
            "source": 1
        },
        "ts": "1785456000000",
        "ttl": 0
    }))
    .unwrap();
    let event = ListenerEvent::Reaction(Reaction::new("self-uid", reaction_data, false));

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Reaction(normalized) = &mutations[0] else {
        panic!("expected reaction mutation");
    };

    assert_eq!(normalized.reaction, "/:new-haha");
}

#[test]
fn undo_event_normalizes_correctly() {
    let undo_data = TUndo {
        action_id: "act-2".to_string(),
        msg_id: "msg-20".to_string(),
        cli_msg_id: "cli-20".to_string(),
        msg_type: "1".to_string(),
        uid_from: "peer-uid".to_string(),
        id_to: "0".to_string(),
        d_name: "Peer".to_string(),
        ts: "1785456000000".to_string(),
        status: 1,
        content: TUndoContent {
            global_msg_id: 20,
            cli_msg_id: 200,
            delete_msg: 1,
            src_id: 100,
            dest_id: 200,
        },
        notify: "".to_string(),
        ttl: 0,
        user_id: "".to_string(),
        uin: "".to_string(),
        cmd: 0,
        st: 0,
        at: 0,
        real_msg_id: "real-20".to_string(),
    };
    let undo = Undo::new("self-uid", undo_data, false);
    let event = ListenerEvent::Undo(Box::new(undo));

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    assert_eq!(mutations.len(), 1);
    let ZaloMutation::Undo(norm_undo) = &mutations[0] else {
        panic!("expected undo mutation");
    };
    assert_eq!(norm_undo.provider_account_key, "imei-a");
    assert_eq!(norm_undo.external_thread_id, "peer-uid");
    assert_eq!(norm_undo.provider_message_id, "20");
    assert_eq!(norm_undo.provider_client_message_id, "200");
}

#[test]
fn delivered_and_seen_receipts_normalize_correctly() {
    let delivered_user = UserDeliveredMessage::new(TDeliveredMessage {
        msg_id: "msg-30".to_string(),
        seen: 0,
        delivered_uids: vec!["peer-uid".to_string()],
        seen_uids: vec![],
        real_msg_id: "real-30".to_string(),
        m_s_ts: 1785456000000,
    });
    let delivered_group = GroupDeliveredMessage::new(
        "self-uid",
        TGroupDeliveredMessage {
            base: TDeliveredMessage {
                msg_id: "msg-31".to_string(),
                seen: 0,
                delivered_uids: vec!["member-1".to_string(), "member-2".to_string()],
                seen_uids: vec![],
                real_msg_id: "real-31".to_string(),
                m_s_ts: 1785456000000,
            },
            group_id: "group-1".to_string(),
        },
    );
    let event_delivered = ListenerEvent::DeliveredMessages(vec![
        DeliveredMessage::User(delivered_user),
        DeliveredMessage::Group(delivered_group),
    ]);

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event_delivered).unwrap();
    assert_eq!(mutations.len(), 2);
    let ZaloMutation::Receipt(receipt1) = &mutations[0] else {
        panic!("expected receipt mutation");
    };
    assert_eq!(receipt1.kind, ReceiptKind::Delivered);
    assert_eq!(receipt1.provider_message_id, "msg-30");

    let seen_user = UserSeenMessage::new(TUserSeenMessage {
        id_to: "peer-uid".to_string(),
        msg_id: "msg-32".to_string(),
        real_msg_id: "real-32".to_string(),
    });
    let seen_group = GroupSeenMessage::new(
        "self-uid",
        TGroupSeenMessage {
            msg_id: "msg-33".to_string(),
            group_id: "group-1".to_string(),
            seen_uids: vec!["member-1".to_string()],
        },
    );
    let event_seen = ListenerEvent::SeenMessages(vec![
        SeenMessage::User(seen_user),
        SeenMessage::Group(seen_group),
    ]);

    let mutations_seen = normalize_listener_event("imei-a", Some("Sale 01"), event_seen).unwrap();
    assert_eq!(mutations_seen.len(), 2);
    let ZaloMutation::Receipt(receipt2) = &mutations_seen[0] else {
        panic!("expected receipt mutation");
    };
    assert_eq!(receipt2.kind, ReceiptKind::Read);
    assert_eq!(receipt2.provider_message_id, "msg-32");

    // Delivered message with seen=1 should normalize to ReceiptKind::Read
    let delivered_seen = UserDeliveredMessage::new(TDeliveredMessage {
        msg_id: "".to_string(),
        seen: 1,
        delivered_uids: vec!["peer-uid".to_string()],
        seen_uids: vec!["peer-uid".to_string()],
        real_msg_id: "real-seen-34".to_string(),
        m_s_ts: 1785456000000,
    });
    let event_del_seen =
        ListenerEvent::DeliveredMessages(vec![DeliveredMessage::User(delivered_seen)]);
    let mutations_del_seen =
        normalize_listener_event("imei-a", Some("Sale 01"), event_del_seen).unwrap();
    assert_eq!(mutations_del_seen.len(), 1);
    let ZaloMutation::Receipt(receipt3) = &mutations_del_seen[0] else {
        panic!("expected receipt mutation");
    };
    assert_eq!(receipt3.kind, ReceiptKind::Read);
    assert_eq!(receipt3.provider_message_id, "real-seen-34");

    // Delivered message with m_s_ts=0 must NOT produce 1970 timestamp
    let delivered_zero_ts = UserDeliveredMessage::new(TDeliveredMessage {
        msg_id: "0".to_string(),
        seen: 0,
        delivered_uids: vec!["peer-uid".to_string()],
        seen_uids: vec![],
        real_msg_id: "real-del-35".to_string(),
        m_s_ts: 0,
    });
    let event_zero_ts =
        ListenerEvent::DeliveredMessages(vec![DeliveredMessage::User(delivered_zero_ts)]);
    let mutations_zero_ts =
        normalize_listener_event("imei-a", Some("Sale 01"), event_zero_ts).unwrap();
    let ZaloMutation::Receipt(receipt4) = &mutations_zero_ts[0] else {
        panic!("expected receipt mutation");
    };
    assert_eq!(receipt4.kind, ReceiptKind::Delivered);
    assert_eq!(receipt4.provider_message_id, "real-del-35");
    assert!(!receipt4.occurred_at.contains("1970"));

    // Delivered message with seen=1 and empty delivered_uids must extract thread_id from seen_uids
    let delivered_active_chat = UserDeliveredMessage::new(TDeliveredMessage {
        msg_id: "msg-active-36".to_string(),
        seen: 1,
        delivered_uids: vec![],
        seen_uids: vec!["active-client-uid".to_string()],
        real_msg_id: "".to_string(),
        m_s_ts: 0,
    });
    let event_active_chat =
        ListenerEvent::DeliveredMessages(vec![DeliveredMessage::User(delivered_active_chat)]);
    let mutations_active_chat =
        normalize_listener_event("imei-a", Some("Sale 01"), event_active_chat).unwrap();
    let ZaloMutation::Receipt(receipt5) = &mutations_active_chat[0] else {
        panic!("expected receipt mutation");
    };
    assert_eq!(receipt5.kind, ReceiptKind::Read);
    assert_eq!(receipt5.external_thread_id, "active-client-uid");
    assert_eq!(
        receipt5.recipient_external_ids,
        vec!["active-client-uid".to_string()]
    );
    assert!(!receipt5.occurred_at.contains("1970"));
}

#[test]
fn receipt_normalization_rejects_empty_zero_and_missing_thread_ids() {
    // 1. Delivery with empty msg_id and real_msg_id
    let empty_msg_del = UserDeliveredMessage::new(TDeliveredMessage {
        msg_id: "".to_string(),
        seen: 0,
        delivered_uids: vec!["peer-1".to_string()],
        seen_uids: vec![],
        real_msg_id: "".to_string(),
        m_s_ts: 0,
    });
    let event1 = ListenerEvent::DeliveredMessages(vec![DeliveredMessage::User(empty_msg_del)]);
    let mut1 = normalize_listener_event("imei-a", Some("Sale 01"), event1).unwrap();
    assert!(mut1.is_empty());

    // 2. Delivery with "0" msg_id and "0" real_msg_id
    let zero_msg_del = UserDeliveredMessage::new(TDeliveredMessage {
        msg_id: "0".to_string(),
        seen: 0,
        delivered_uids: vec!["peer-1".to_string()],
        seen_uids: vec![],
        real_msg_id: "0".to_string(),
        m_s_ts: 0,
    });
    let event2 = ListenerEvent::DeliveredMessages(vec![DeliveredMessage::User(zero_msg_del)]);
    let mut2 = normalize_listener_event("imei-a", Some("Sale 01"), event2).unwrap();
    assert!(mut2.is_empty());

    // 3. Delivery with valid msg_id but empty thread/recipients
    let mut no_thread_del = UserDeliveredMessage::new(TDeliveredMessage {
        msg_id: "msg-valid-1".to_string(),
        seen: 0,
        delivered_uids: vec![],
        seen_uids: vec![],
        real_msg_id: "".to_string(),
        m_s_ts: 0,
    });
    no_thread_del.thread_id = "".to_string();
    let event3 = ListenerEvent::DeliveredMessages(vec![DeliveredMessage::User(no_thread_del)]);
    let mut3 = normalize_listener_event("imei-a", Some("Sale 01"), event3).unwrap();
    assert!(mut3.is_empty());

    // 4. Seen with empty msg_id and real_msg_id
    let empty_seen = UserSeenMessage::new(TUserSeenMessage {
        id_to: "peer-1".to_string(),
        msg_id: "".to_string(),
        real_msg_id: "".to_string(),
    });
    let event4 = ListenerEvent::SeenMessages(vec![SeenMessage::User(empty_seen)]);
    let mut4 = normalize_listener_event("imei-a", Some("Sale 01"), event4).unwrap();
    assert!(mut4.is_empty());

    // 5. Seen with empty thread_id (id_to)
    let mut no_thread_seen = UserSeenMessage::new(TUserSeenMessage {
        id_to: "".to_string(),
        msg_id: "msg-valid-1".to_string(),
        real_msg_id: "".to_string(),
    });
    no_thread_seen.thread_id = "".to_string();
    let event5 = ListenerEvent::SeenMessages(vec![SeenMessage::User(no_thread_seen)]);
    let mut5 = normalize_listener_event("imei-a", Some("Sale 01"), event5).unwrap();
    assert!(mut5.is_empty());
}

#[test]
fn receipt_normalization_mixed_batch_preserves_valid_and_drops_invalid() {
    let valid_del = UserDeliveredMessage::new(TDeliveredMessage {
        msg_id: "msg-valid-100".to_string(),
        seen: 0,
        delivered_uids: vec!["peer-1".to_string()],
        seen_uids: vec![],
        real_msg_id: "".to_string(),
        m_s_ts: 1785456000000,
    });
    let empty_del = UserDeliveredMessage::new(TDeliveredMessage {
        msg_id: "".to_string(),
        seen: 0,
        delivered_uids: vec!["peer-1".to_string()],
        seen_uids: vec![],
        real_msg_id: "".to_string(),
        m_s_ts: 0,
    });
    let zero_del = UserDeliveredMessage::new(TDeliveredMessage {
        msg_id: "0".to_string(),
        seen: 0,
        delivered_uids: vec!["peer-1".to_string()],
        seen_uids: vec![],
        real_msg_id: "0".to_string(),
        m_s_ts: 0,
    });

    let batch = ListenerEvent::DeliveredMessages(vec![
        DeliveredMessage::User(valid_del),
        DeliveredMessage::User(empty_del),
        DeliveredMessage::User(zero_del),
    ]);

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), batch).unwrap();
    assert_eq!(mutations.len(), 1);
    let ZaloMutation::Receipt(receipt) = &mutations[0] else {
        panic!("expected receipt");
    };
    assert_eq!(receipt.provider_message_id, "msg-valid-100");
    assert_eq!(receipt.external_thread_id, "peer-1");
}

#[test]
fn disconnect_and_error_events_emit_reconcile_required() {
    let event = ListenerEvent::Disconnected {
        code: 1006,
        reason: "abnormal".to_string(),
    };
    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    assert_eq!(mutations.len(), 1);
    let ZaloMutation::ReconcileRequired { scope } = mutations[0] else {
        panic!("expected reconcile");
    };
    assert_eq!(scope, "all");
}

#[test]
fn packet_error_event_emits_reconcile_required() {
    let event = ListenerEvent::PacketError {
        cmd: 501,
        sub_cmd: 1,
        error: "invalid packet payload".to_string(),
    };

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    assert_eq!(mutations.len(), 1);
    let ZaloMutation::ReconcileRequired { scope } = mutations[0] else {
        panic!("expected reconcile");
    };
    assert_eq!(scope, "all");
}

#[test]
fn photo_message_extracts_caption_text_content() {
    let mut base = base_message(
        "peer-uid",
        "0",
        "photo-msg-1",
        json!({
            "url": "https://cdn.example.com/photo.jpg",
            "caption": "Chào tháng 8 may mắn"
        }),
    );
    base.msg_type = "chat.photo".to_string();

    let event = ListenerEvent::Message(Box::new(Message::User(UserMessage::new("self-uid", base))));

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Message(message) = &mutations[0] else {
        panic!("expected message mutation");
    };
    assert_eq!(message.content_type, "image");
    assert_eq!(
        message.text_content.as_deref(),
        Some("Chào tháng 8 may mắn")
    );
}

#[test]
fn mention_inbound_group_message_normalizes_mentions() {
    let mut base = base_message(
        "sender-uid",
        "0",
        "group-msg-mentions-1",
        json!({
            "text": "Xin chào @Nguyễn Văn A nhé",
            "mentions": [
                {
                    "target_uid": "user-302",
                    "utf16_offset": 9,
                    "utf16_len": 13,
                    "mention_text": "@Nguyễn Văn A"
                }
            ]
        }),
    );
    base.msg_type = "chat.text".to_string();

    let raw = TGroupMessage {
        base,
        mentions: None,
    };
    let group_msg = GroupMessage::new("group-thread-100", raw);

    let event = ListenerEvent::Message(Box::new(Message::Group(group_msg)));
    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Message(message) = &mutations[0] else {
        panic!("expected message mutation");
    };

    assert_eq!(message.mentions.len(), 1);
    assert_eq!(message.mentions[0].external_id.as_deref(), Some("user-302"));
    assert_eq!(message.mentions[0].display_text, "@Nguyễn Văn A");
    assert_eq!(message.mentions[0].start_utf16, 9);
    assert_eq!(message.mentions[0].length_utf16, 13);
}

#[test]
fn mention_inbound_group_message_uses_top_level_sdk_mentions() {
    let mut base = base_message(
        "sender-uid",
        "0",
        "group-msg-sdk-mentions",
        json!("@Nga Nguyễn B B Tourist"),
    );
    base.msg_type = "chat.text".to_string();

    let raw = TGroupMessage {
        base,
        mentions: Some(vec![TMention {
            uid: "5054685013624794058".to_string(),
            pos: 0,
            len: 13,
            r#type: 0,
        }]),
    };
    let group_msg = GroupMessage::new("group-thread-100", raw);

    let event = ListenerEvent::Message(Box::new(Message::Group(group_msg)));
    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Message(message) = &mutations[0] else {
        panic!("expected message mutation");
    };

    assert_eq!(message.mentions.len(), 1);
    assert_eq!(
        message.mentions[0].external_id.as_deref(),
        Some("5054685013624794058")
    );
    assert_eq!(message.mentions[0].display_text, "@Nga Nguyễn B");
    assert_eq!(message.mentions[0].start_utf16, 0);
    assert_eq!(message.mentions[0].length_utf16, 13);
    assert_eq!(
        message
            .raw_payload
            .as_ref()
            .and_then(|payload| payload.get("mentions"))
            .and_then(|mentions| mentions.get(0))
            .and_then(|mention| mention.get("uid"))
            .and_then(serde_json::Value::as_str),
        Some("5054685013624794058")
    );
}

#[test]
fn mention_inbound_group_message_normalizes_numeric_uid_and_property_ext() {
    let mut base = base_message(
        "sender-uid",
        "0",
        "group-msg-mentions-numeric",
        json!("@Quang Thái bác check lại coi"),
    );
    base.msg_type = "chat.text".to_string();
    base.property_ext = Some(TPropertyExt {
        color: -1,
        size: -1,
        r#type: 1,
        sub_type: 0,
        ext: "{\"mentions\":[{\"uid\":481940120149,\"pos\":0,\"len\":11,\"display_text\":\"@Quang Thái\"}]}".to_string(),
    });

    let raw = TGroupMessage {
        base,
        mentions: None,
    };
    let group_msg = GroupMessage::new("group-thread-100", raw);

    let event = ListenerEvent::Message(Box::new(Message::Group(group_msg)));
    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Message(message) = &mutations[0] else {
        panic!("expected message mutation");
    };

    assert_eq!(message.mentions.len(), 1);
    assert_eq!(
        message.mentions[0].external_id.as_deref(),
        Some("481940120149")
    );
    assert_eq!(message.mentions[0].display_text, "@Quang Thái");
    assert_eq!(message.mentions[0].start_utf16, 0);
    assert_eq!(message.mentions[0].length_utf16, 11);
}

#[test]
fn sticker_message_normalizes_correctly_with_url_variants() {
    let mut base = base_message(
        "peer-uid",
        "0",
        "sticker-msg-1",
        json!({
            "id": 12345,
            "cateId": 678,
            "type": 1,
            "spriteUrl": "https://stc-laban.zdn.vn/sticker/sprite.png"
        }),
    );
    base.msg_type = "chat.sticker".to_string();

    let event = ListenerEvent::Message(Box::new(Message::User(UserMessage::new("self-uid", base))));

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Message(message) = &mutations[0] else {
        panic!("expected message mutation");
    };
    assert_eq!(message.content_type, "sticker");
    assert_eq!(message.attachments.len(), 1);
    assert_eq!(
        message.attachments[0].remote_url.as_deref(),
        Some("https://stc-laban.zdn.vn/sticker/sprite.png")
    );
}

#[test]
fn sticker_message_with_stringified_json_content_normalizes_attachment() {
    let mut base = base_message(
        "peer-uid",
        "0",
        "sticker-msg-2",
        json!("{\"id\":999,\"stickerUrl\":\"https://stc-laban.zdn.vn/sticker/sample.webp\"}"),
    );
    base.msg_type = "chat.sticker".to_string();

    let event = ListenerEvent::Message(Box::new(Message::User(UserMessage::new("self-uid", base))));

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Message(message) = &mutations[0] else {
        panic!("expected message mutation");
    };
    assert_eq!(message.content_type, "sticker");
    assert_eq!(message.attachments.len(), 1);
    assert_eq!(
        message.attachments[0].remote_url.as_deref(),
        Some("https://stc-laban.zdn.vn/sticker/sample.webp")
    );
}

#[test]
fn self_reaction_in_direct_chat_resolves_to_peer_thread_id() {
    let reaction_data = TReaction {
        action_id: "act-self-react".to_string(),
        msg_id: "event-self-react".to_string(),
        cli_msg_id: "cli-self-react".to_string(),
        msg_type: "1".to_string(),
        uid_from: "self-uid".to_string(),
        id_to: "peer-uid".to_string(),
        d_name: Some("Self".to_string()),
        content: TReactionContent {
            r_msg: vec![TReactionMsg {
                g_msg_id: "gmsg-self-react".to_string(),
                c_msg_id: "cmsg-self-react".to_string(),
                msg_type: 1,
            }],
            r_icon: EventReactions::Heart,
            r_type: 1,
            source: 6,
        },
        ts: "1786269310941".to_string(),
        ttl: 0,
    };
    let event = ListenerEvent::Reaction(Reaction::new("self-uid", reaction_data, false));

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Reaction(normalized) = &mutations[0] else {
        panic!("expected reaction mutation");
    };

    assert_eq!(normalized.external_thread_id, "peer-uid");
}

#[test]
fn undo_event_end_to_end_resolves_and_tombstones_message_in_db() {
    use crate::conversation::migration::migrate;
    use crate::conversation::repository::ConversationRepository;
    use crate::conversation::types::MessageStatus;
    use crate::db::open_db_connection;

    let path = std::env::temp_dir().join(format!(
        "rustsale-undo-e2e-test-{}.db",
        rand::random::<u64>()
    ));
    let conn = open_db_connection(path.to_str().unwrap()).unwrap();
    conn.execute(
        "CREATE TABLE leads (id INTEGER PRIMARY KEY, name TEXT NOT NULL, phone TEXT, zalo_uid TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE IF NOT EXISTS zalo_sessions (
            imei TEXT PRIMARY KEY,
            user_agent TEXT NOT NULL,
            cookies_json TEXT NOT NULL,
            language TEXT NOT NULL,
            display_name TEXT,
            avatar TEXT
        )",
        [],
    )
    .unwrap();
    migrate(&conn).unwrap();
    drop(conn);

    let repo = ConversationRepository::new(path.to_string_lossy().to_string());

    // 1. Inbound message 1 from peer: msgId="88888", cliMsgId="client-88888"
    let msg1_data = base_message("peer-uid", "0", "88888", json!("Xin chao shop"));
    let event1 = ListenerEvent::Message(Box::new(Message::User(UserMessage::new(
        "self-uid", msg1_data,
    ))));
    let mutations1 = normalize_listener_event("imei-e2e", Some("Sale Agent"), event1).unwrap();
    assert_eq!(mutations1.len(), 1);
    let ZaloMutation::Message(norm_msg1) = mutations1.into_iter().next().unwrap() else {
        panic!("expected message mutation");
    };
    let applied1 = repo.apply_normalized_message(&norm_msg1).unwrap();

    // 2. Inbound message 2 from peer (control message): msgId="88889", cliMsgId="client-88889"
    let msg2_data = base_message("peer-uid", "0", "88889", json!("Tin nhan khong bi thu hoi"));
    let event2 = ListenerEvent::Message(Box::new(Message::User(UserMessage::new(
        "self-uid", msg2_data,
    ))));
    let mutations2 = normalize_listener_event("imei-e2e", Some("Sale Agent"), event2).unwrap();
    let ZaloMutation::Message(norm_msg2) = mutations2.into_iter().next().unwrap() else {
        panic!("expected message mutation");
    };
    let applied2 = repo.apply_normalized_message(&norm_msg2).unwrap();

    // Verify initial states
    let msg1_before = repo
        .get_message_by_id(applied1.message_id)
        .unwrap()
        .unwrap();
    assert_eq!(msg1_before.status, MessageStatus::Received);
    assert!(msg1_before.removed_at.is_none());

    // 3. Receive real Undo event recalling message 1
    // Notice: undo control packet msg_id is "999999" (packet ID), but content.global_msg_id is 88888
    let undo_data = TUndo {
        action_id: "act-undo-1".to_string(),
        msg_id: "999999".to_string(), // packet ID
        cli_msg_id: "cli-packet-999999".to_string(),
        msg_type: "undo".to_string(),
        uid_from: "peer-uid".to_string(),
        id_to: "self-uid".to_string(),
        d_name: "Peer".to_string(),
        ts: "1785456000000".to_string(),
        status: 1,
        content: TUndoContent {
            global_msg_id: 88888,
            cli_msg_id: 0,
            delete_msg: 1,
            src_id: 100,
            dest_id: 200,
        },
        notify: "".to_string(),
        ttl: 0,
        user_id: "".to_string(),
        uin: "".to_string(),
        cmd: 0,
        st: 0,
        at: 0,
        real_msg_id: "real-undo-1".to_string(),
    };
    let undo_event = ListenerEvent::Undo(Box::new(Undo::new("self-uid", undo_data, false)));
    let undo_mutations =
        normalize_listener_event("imei-e2e", Some("Sale Agent"), undo_event).unwrap();
    assert_eq!(undo_mutations.len(), 1);
    let ZaloMutation::Undo(norm_undo) = undo_mutations.into_iter().next().unwrap() else {
        panic!("expected undo mutation");
    };

    // 4. Apply undo to repository
    let affected_msg_id = repo.apply_undo(&norm_undo).unwrap();
    assert_eq!(affected_msg_id, Some(applied1.message_id));

    // 5. Verify message 1 is now removed/tombstoned
    let msg1_after = repo
        .get_message_by_id(applied1.message_id)
        .unwrap()
        .unwrap();
    assert_eq!(msg1_after.status, MessageStatus::Removed);
    assert!(msg1_after.removed_at.is_some());

    // 6. Verify message 2 remains unaffected
    let msg2_after = repo
        .get_message_by_id(applied2.message_id)
        .unwrap()
        .unwrap();
    assert_eq!(msg2_after.status, MessageStatus::Received);
    assert!(msg2_after.removed_at.is_none());

    let _ = std::fs::remove_file(path);
}

#[test]
fn complete_outbox_job_updates_provider_client_message_id_when_different_from_queued() {
    use crate::conversation::migration::migrate;
    use crate::conversation::repository::ConversationRepository;
    use crate::conversation::types::{MessageStatus, SendConversationMessageRequest};
    use crate::db::open_db_connection;

    let path = std::env::temp_dir().join(format!(
        "rustsale-outbox-cli-test-{}.db",
        rand::random::<u64>()
    ));
    let conn = open_db_connection(path.to_str().unwrap()).unwrap();
    conn.execute(
        "CREATE TABLE leads (id INTEGER PRIMARY KEY, name TEXT NOT NULL, phone TEXT, zalo_uid TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE IF NOT EXISTS zalo_sessions (
            imei TEXT PRIMARY KEY,
            user_agent TEXT NOT NULL,
            cookies_json TEXT NOT NULL,
            language TEXT NOT NULL,
            display_name TEXT,
            avatar TEXT
        )",
        [],
    )
    .unwrap();
    migrate(&conn).unwrap();
    drop(conn);

    let repo = ConversationRepository::new(path.to_string_lossy().to_string());

    // Setup an initial normalized message to establish account and conversation
    let msg_data = base_message("peer-thread", "0", "11111", json!("Initial message"));
    let event = ListenerEvent::Message(Box::new(Message::User(UserMessage::new(
        "self-uid", msg_data,
    ))));
    let mutations = normalize_listener_event("imei-outbox", Some("Account"), event).unwrap();
    let ZaloMutation::Message(norm_msg) = mutations.into_iter().next().unwrap() else {
        panic!("expected message");
    };
    let applied = repo.apply_normalized_message(&norm_msg).unwrap();

    // Enqueue outbound message
    let req = SendConversationMessageRequest {
        conversation_id: applied.conversation_id,
        channel_account_id: applied.channel_account_id,
        text: "Outbound message to be sent".to_string(),
        reply_to_message_id: None,
    };
    let (queued_msg, _delta) = repo
        .enqueue_outbound_message(&req, "local-req-1", "job-outbox-1", "2026-08-18T10:00:00Z")
        .unwrap();
    assert_eq!(queued_msg.status, MessageStatus::Queued);
    let initial_cli_id = queued_msg.provider_client_message_id.clone().unwrap();

    // Complete outbox job where provider returns a different cli_msg_id (e.g. from attachment upload response)
    let provider_returned_cli_id = "provider-actual-cli-789012";
    let provider_msg_id = "server-msg-999999";
    repo.complete_outbox_job(
        "job-outbox-1",
        provider_msg_id,
        provider_returned_cli_id,
        "2026-08-18T10:01:00Z",
    )
    .unwrap();

    let msg_after = repo.get_message_by_id(queued_msg.id).unwrap().unwrap();
    assert_eq!(msg_after.status, MessageStatus::Sent);
    assert_eq!(
        msg_after.provider_message_id.as_deref(),
        Some(provider_msg_id)
    );
    assert_eq!(
        msg_after.provider_client_message_id.as_deref(),
        Some(provider_returned_cli_id)
    );
    assert_ne!(
        msg_after.provider_client_message_id.as_deref(),
        Some(initial_cli_id.as_str())
    );

    let _ = std::fs::remove_file(path);
}

fn normalized_content(
    content: serde_json::Value,
) -> crate::conversation::repository::NormalizedMessage {
    let event = ListenerEvent::Message(Box::new(Message::User(UserMessage::new(
        "self-uid",
        base_message("peer-uid", "0", "message-rtf", content),
    ))));
    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Message(message) = mutations.into_iter().next().unwrap() else {
        panic!("expected message mutation");
    };
    message
}

#[test]
fn formatted_rtf_text_keeps_its_title_as_plain_text() {
    let title = "QUOC ANH // 900814405E--- ACTR\nCHO XE ĐI --- FULL SAILUN";
    let message = normalized_content(json!({
        "action": "rtf",
        "childnumber": 0,
        "description": "",
        "href": "",
        "params": "{\"styles\":[{\"start\":64,\"len\":11,\"st\":\"b\"}],\"ver\":0}",
        "thumb": "",
        "title": title,
        "type": ""
    }));
    assert_eq!(message.content_type, "text");
    assert_eq!(message.text_content.as_deref(), Some(title));
}

#[test]
fn empty_description_does_not_hide_title_for_pin_notices() {
    let message = normalized_content(json!({
        "action": "msginfo.actionlist",
        "description": "",
        "title": "Vận Tải Quốc Anh đã ghim tin nhắn x",
        "params": "{}"
    }));
    assert_eq!(message.content_type, "unsupported");
    assert_eq!(
        message.text_content.as_deref(),
        Some("Vận Tải Quốc Anh đã ghim tin nhắn x")
    );
}

#[test]
fn non_rtf_webchat_object_stays_unsupported() {
    let message = normalized_content(json!({ "action": "something.else", "title": "t" }));
    assert_eq!(message.content_type, "unsupported");
}
