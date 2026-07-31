use super::probe::{observe_listener_event, ZaloProbeReport};
use serde_json::json;
use std::time::Duration;
use zca_rs::context::ZaloCookie;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::{Message, TMessage, UserMessage};
use zca_rs::{Options, Zalo};

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

fn next_history_cursor(report: &ZaloProbeReport) -> Option<&str> {
    report.provider_message_ids.last().map(String::as_str)
}

#[test]
fn history_cursor_uses_last_provider_message_id() {
    let report = ZaloProbeReport {
        provider_message_ids: vec!["msg-1".to_string(), "msg-2".to_string()],
        occurred_at_timestamps: vec![
            "2026-07-31T12:05:00.000Z".to_string(),
            "2026-07-31T12:01:00.000Z".to_string(),
        ],
        ..ZaloProbeReport::default()
    };

    assert_eq!(next_history_cursor(&report), Some("msg-2"));
    assert_eq!(
        report
            .occurred_at_timestamps
            .iter()
            .min()
            .map(String::as_str),
        Some("2026-07-31T12:01:00.000Z")
    );
}

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("missing required environment variable {name}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_zalo_conversation_probe() {
    let imei = match std::env::var("RUSTSALE_ZALO_PROBE_IMEI") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            println!("Skipping live Zalo probe: RUSTSALE_ZALO_PROBE_IMEI not set in environment");
            return;
        }
    };
    let user_agent = required_env("RUSTSALE_ZALO_PROBE_USER_AGENT");
    let cookies: Vec<ZaloCookie> =
        serde_json::from_str(&required_env("RUSTSALE_ZALO_PROBE_COOKIES_JSON"))
            .expect("cookies JSON must match Vec<ZaloCookie>");
    let timeout_seconds = std::env::var("RUSTSALE_ZALO_PROBE_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(90);

    let mut options = Options::default();
    options.self_listen = true;
    let zalo = Zalo::login(imei, user_agent, cookies, options)
        .await
        .expect("Zalo probe login failed");
    let mut receiver = zalo.subscribe();

    zalo.listener
        .request_old_messages(zca_rs::models::ThreadType::User, None)
        .expect("request direct history");
    zalo.listener
        .request_old_messages(zca_rs::models::ThreadType::Group, None)
        .expect("request group history");
    zalo.listener
        .request_old_reactions(zca_rs::models::ThreadType::User, None)
        .expect("request direct reactions");
    zalo.listener
        .request_old_reactions(zca_rs::models::ThreadType::Group, None)
        .expect("request group reactions");

    let groups = zalo.get_all_groups().await.expect("get all groups");
    let first_group_id = groups
        .grid_ver_map
        .keys()
        .next()
        .cloned()
        .expect("probe account must belong to at least one group");
    let group_history = zalo
        .get_group_chat_history(&first_group_id, Some(50))
        .await
        .expect("get group history");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_seconds);
    let mut report = ZaloProbeReport::default();
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(2), receiver.recv()).await {
            Ok(Ok(event)) => observe_listener_event(&mut report, &event),
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped))) => {
                panic!("probe receiver lagged by {skipped} events");
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                panic!("probe listener closed");
            }
            Err(_) => {}
        }
    }

    if let Some(cursor) = next_history_cursor(&report) {
        zalo.listener
            .request_old_messages(zca_rs::models::ThreadType::User, Some(cursor))
            .expect("request second direct-history page");
    }

    zalo.listener.stop();
    tokio::time::sleep(Duration::from_secs(2)).await;
    zalo.listener.start().expect("restart listener");
    let mut reconnect_receiver = zalo.subscribe();
    zalo.listener
        .request_old_messages(
            zca_rs::models::ThreadType::User,
            next_history_cursor(&report),
        )
        .expect("request reconciliation after reconnect");

    let reconnect_event = tokio::time::timeout(Duration::from_secs(20), reconnect_receiver.recv())
        .await
        .expect("no listener event after reconnect")
        .expect("listener closed after reconnect");
    observe_listener_event(&mut report, &reconnect_event);

    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("serialize sanitized probe report")
    );
    println!(
        "groupHistory={{groupIdLength:{},messageCount:{},more:{},lastActionIdLength:{}}}",
        first_group_id.len(),
        group_history.group_msgs.len(),
        group_history.more,
        group_history.last_action_id.len()
    );

    assert!(
        report.old_direct_batches >= 1,
        "direct history batch not observed"
    );
    assert!(
        report.old_group_batches >= 1,
        "group history batch not observed"
    );
    assert!(report.direct_messages >= 1, "no direct message observed");
    assert!(
        report.group_messages >= 1 || !group_history.group_msgs.is_empty(),
        "no group message observed"
    );
    assert!(
        !report.provider_message_ids.is_empty(),
        "provider message IDs were empty"
    );
    assert!(
        !report.client_message_ids.is_empty(),
        "client message IDs were empty"
    );
}

#[tokio::test]
async fn synthetic_zalo_conversation_probe_execution() {
    let mut report = ZaloProbeReport::default();

    // Page 1: Direct live message + Page 1 History
    let direct_ev = ListenerEvent::Message(Box::new(Message::User(UserMessage::new(
        "self-uid",
        direct_message("peer-uid", "0", "msg-102"),
    ))));
    let old_ev_1 = ListenerEvent::OldMessages {
        messages: vec![
            Message::User(UserMessage::new(
                "self-uid",
                direct_message("peer-uid", "0", "msg-101"),
            )),
            Message::User(UserMessage::new(
                "self-uid",
                direct_message("peer-uid", "0", "msg-100"),
            )),
        ],
        thread_type: zca_rs::models::ThreadType::User,
    };

    observe_listener_event(&mut report, &direct_ev);
    observe_listener_event(&mut report, &old_ev_1);

    assert_eq!(report.direct_messages, 3);
    assert_eq!(report.old_direct_batches, 1);
    assert_eq!(next_history_cursor(&report), Some("msg-100"));

    // Page 2: Multi-page continuation requested with cursor "msg-100"
    let old_ev_2 = ListenerEvent::OldMessages {
        messages: vec![
            Message::User(UserMessage::new(
                "self-uid",
                direct_message("peer-uid", "0", "msg-099"),
            )),
            Message::User(UserMessage::new(
                "self-uid",
                direct_message("peer-uid", "0", "msg-098"),
            )),
        ],
        thread_type: zca_rs::models::ThreadType::User,
    };

    observe_listener_event(&mut report, &old_ev_2);

    assert_eq!(report.direct_messages, 5);
    assert_eq!(report.old_direct_batches, 2);
    // Continuation cursor now advances to "msg-098" (last ID in second batch)
    assert_eq!(next_history_cursor(&report), Some("msg-098"));

    // Verify Reconnect & Re-sync event handling
    let reconnect_ev = ListenerEvent::Disconnected {
        reason: "WS Reset".to_string(),
        code: 1006,
    };
    observe_listener_event(&mut report, &reconnect_ev);
    assert_eq!(report.occurred_at_timestamps.len(), 5);
}
