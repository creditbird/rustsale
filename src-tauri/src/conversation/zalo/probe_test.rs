use super::probe::{observe_listener_event, ZaloProbeReport};
use serde_json::json;
use std::time::Duration;
use zca_rs::context::ZaloCookie;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::{GroupMessage, Message, TGroupMessage, TMessage, UserMessage};
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

#[test]
fn probe_records_packet_errors_without_exposing_payloads() {
    let event = ListenerEvent::PacketError {
        cmd: 501,
        sub_cmd: 1,
        error: "invalid packet payload".to_string(),
    };
    let mut report = ZaloProbeReport::default();

    observe_listener_event(&mut report, &event);

    assert_eq!(report.error_events, 1);
    assert_eq!(report.error_message_lengths, vec![22]);
}

fn next_history_cursor(report: &ZaloProbeReport) -> Option<&str> {
    report.provider_message_ids.last().map(String::as_str)
}

fn next_direct_history_cursor(report: &ZaloProbeReport) -> Option<&str> {
    report
        .direct_provider_message_ids
        .last()
        .map(String::as_str)
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

#[test]
fn direct_history_cursor_ignores_group_message_ids() {
    let direct_event = ListenerEvent::Message(Box::new(Message::User(UserMessage::new(
        "self-uid",
        direct_message("peer-uid", "0", "direct-1"),
    ))));
    let group_event = ListenerEvent::Message(Box::new(Message::Group(GroupMessage::new(
        "self-uid",
        TGroupMessage {
            base: direct_message("group-peer", "group-1", "group-1"),
            mentions: None,
        },
    ))));
    let mut report = ZaloProbeReport::default();

    observe_listener_event(&mut report, &direct_event);
    observe_listener_event(&mut report, &group_event);

    assert_eq!(next_direct_history_cursor(&report), Some("direct-1"));
    assert_eq!(report.group_provider_message_ids, vec!["group-1"]);
}

#[test]
fn sanitized_probe_summary_exposes_counts_and_lengths_only() {
    let event = ListenerEvent::Message(Box::new(Message::User(UserMessage::new(
        "self-uid",
        direct_message("secret-peer-uid", "0", "secret-message-id"),
    ))));
    let mut report = ZaloProbeReport::default();
    observe_listener_event(&mut report, &event);

    let serialized = serde_json::to_string(&report.sanitized_summary()).unwrap();

    assert!(!serialized.contains("secret-peer-uid"));
    assert!(!serialized.contains("secret-message-id"));
    assert!(serialized.contains("providerMessageIdLengths"));
    assert!(serialized.contains("threadIdLengths"));
}

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("missing required environment variable {name}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires a non-production Zalo session and explicit live credentials"]
async fn live_zalo_conversation_probe() {
    crate::install_rustls_crypto_provider();
    let imei = required_env("RUSTSALE_ZALO_PROBE_IMEI");
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
    options.auto_start_listener = false;
    let zalo = Zalo::login(imei, user_agent, cookies, options)
        .await
        .expect("Zalo probe login failed");
    let mut receiver = zalo.subscribe();
    zalo.listener.start().expect("start Zalo probe listener");
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            match receiver.recv().await {
                Ok(ListenerEvent::Connected) => break,
                Ok(ListenerEvent::Error(error)) => {
                    panic!("Zalo probe listener failed before ready: {error}")
                }
                Ok(ListenerEvent::Disconnected { code, reason })
                | Ok(ListenerEvent::Closed { code, reason }) => {
                    panic!("Zalo probe listener closed before ready ({code}): {reason}")
                }
                Ok(_) => {}
                Err(error) => panic!("Zalo probe readiness stream failed: {error}"),
            }
        }
    })
    .await
    .expect("Zalo probe listener readiness timed out");

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
        .ok();

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

    println!(
        "initialProbe={}",
        serde_json::to_string(&report.sanitized_summary())
            .expect("serialize initial sanitized probe report")
    );

    let first_direct_batch_count = report.old_direct_batches;
    let direct_cursor = next_direct_history_cursor(&report)
        .expect("direct history must expose a provider message ID cursor")
        .to_string();
    zalo.listener
        .request_old_messages(zca_rs::models::ThreadType::User, Some(&direct_cursor))
        .expect("request second direct-history page");

    let second_page_deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while report.old_direct_batches == first_direct_batch_count
        && tokio::time::Instant::now() < second_page_deadline
    {
        match tokio::time::timeout(Duration::from_secs(2), receiver.recv()).await {
            Ok(Ok(event)) => observe_listener_event(&mut report, &event),
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped))) => {
                panic!("probe receiver lagged by {skipped} events while waiting for page 2");
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                panic!("probe listener closed while waiting for page 2");
            }
            Err(_) => {}
        }
    }
    assert!(
        report.old_direct_batches > first_direct_batch_count,
        "second direct-history page not observed"
    );

    zalo.listener.stop();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let mut reconnect_receiver = zalo.subscribe();
    zalo.listener.start().expect("restart listener");
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            match reconnect_receiver.recv().await {
                Ok(ListenerEvent::Connected) => break,
                Ok(ListenerEvent::Error(error)) => {
                    panic!("Zalo probe listener failed during reconnect: {error}")
                }
                Ok(ListenerEvent::Disconnected { code, reason })
                | Ok(ListenerEvent::Closed { code, reason }) => {
                    panic!("Zalo probe listener closed during reconnect ({code}): {reason}")
                }
                Ok(_) => {}
                Err(error) => panic!("Zalo probe reconnect stream failed: {error}"),
            }
        }
    })
    .await
    .expect("Zalo probe reconnect readiness timed out");
    let reconnect_batch_count = report.old_direct_batches;
    let reconnect_cursor = next_direct_history_cursor(&report)
        .expect("reconnect history must expose a provider message ID cursor")
        .to_string();
    zalo.listener
        .request_old_messages(zca_rs::models::ThreadType::User, Some(&reconnect_cursor))
        .expect("request reconciliation after reconnect");

    let reconnect_deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while report.old_direct_batches == reconnect_batch_count
        && tokio::time::Instant::now() < reconnect_deadline
    {
        match tokio::time::timeout(Duration::from_secs(2), reconnect_receiver.recv()).await {
            Ok(Ok(event)) => observe_listener_event(&mut report, &event),
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped))) => {
                panic!("probe receiver lagged by {skipped} events after reconnect");
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                panic!("probe listener closed after reconnect");
            }
            Err(_) => {}
        }
    }
    assert!(
        report.old_direct_batches > reconnect_batch_count,
        "direct-history reconciliation not observed after reconnect"
    );

    println!(
        "{}",
        serde_json::to_string_pretty(&report.sanitized_summary())
            .expect("serialize sanitized probe report")
    );
    println!(
        "groupHistory={{groupIdLength:{},available:{},messageCount:{},more:{},lastActionIdLength:{}}}",
        first_group_id.len(),
        group_history.is_some(),
        group_history
            .as_ref()
            .map(|history| history.group_msgs.len())
            .unwrap_or_default(),
        group_history
            .as_ref()
            .map(|history| history.more)
            .unwrap_or_default(),
        group_history
            .as_ref()
            .map(|history| history.last_action_id.len())
            .unwrap_or_default()
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
        report.group_messages >= 1
            || group_history
                .as_ref()
                .is_some_and(|history| !history.group_msgs.is_empty()),
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
    assert_eq!(
        report.error_events, 0,
        "protocol decode errors were observed"
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
