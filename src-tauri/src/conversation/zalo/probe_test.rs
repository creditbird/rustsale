use super::probe::{observe_listener_event, ZaloProbeReport};
use serde_json::json;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::{Message, TMessage, UserMessage};

fn direct_message(uid_from: &str, id_to: &str, msg_id: &str) -> TMessage {
    serde_json::from_value(json!({
        "actionId": "action-1",
        "msgId": msg_id,
        "cliMsgId": "client-1",
        "msgType": "webchat",
        "uidFrom": uid_from,
        "idTo": id_to,
        "dName": "Probe Sender",
        "ts": "1785456000000",
        "status": 1,
        "content": "hello",
        "notify": "",
        "ttl": 0,
        "userId": "",
        "uin": "",
        "propertyExt": null,
        "paramsExt": {
            "countUnread": 1,
            "containType": 0,
            "platformType": 1
        },
        "cmd": 501,
        "st": 0,
        "at": 0,
        "realMsgId": "real-1",
        "quote": null
    }))
    .unwrap()
}

#[test]
fn probe_summary_records_sanitized_direct_identifiers() {
    let event = ListenerEvent::Message(Box::new(Message::User(UserMessage::new(
        "self-uid",
        direct_message("peer-uid", "0", "message-1"),
    ))));
    let mut report = ZaloProbeReport::default();

    observe_listener_event(&mut report, &event);

    assert_eq!(report.direct_messages, 1);
    assert_eq!(report.group_messages, 0);
    assert_eq!(report.provider_message_ids, vec!["message-1"]);
    assert_eq!(report.thread_ids, vec!["peer-uid"]);
    assert_eq!(report.self_messages, 0);
}
