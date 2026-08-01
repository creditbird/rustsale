use super::normalizer::{normalize_listener_event, ReceiptKind, ZaloMutation};
use serde_json::json;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::{
    DeliveredMessage, EventReactions, GroupDeliveredMessage, GroupMessage, GroupSeenMessage,
    Message, Reaction, SeenMessage, TDeliveredMessage, TGroupDeliveredMessage, TGroupMessage,
    TGroupSeenMessage, TMessage, TReaction, TReactionContent, TReactionMsg, TUndo, TUndoContent,
    TUserSeenMessage, Undo, UserDeliveredMessage, UserMessage, UserSeenMessage,
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
    assert_eq!(norm_undo.provider_message_id, "msg-20");
    assert_eq!(norm_undo.provider_client_message_id, "cli-20");
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
