use serde::Serialize;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::Message;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZaloProbeReport {
    pub direct_messages: usize,
    pub group_messages: usize,
    pub old_direct_batches: usize,
    pub old_group_batches: usize,
    pub reactions: usize,
    pub undo_events: usize,
    pub delivered_events: usize,
    pub seen_events: usize,
    pub group_events: usize,
    pub self_messages: usize,
    pub provider_message_ids: Vec<String>,
    pub client_message_ids: Vec<String>,
    pub real_message_ids: Vec<String>,
    pub thread_ids: Vec<String>,
    pub occurred_at_timestamps: Vec<String>,
}

fn observe_message(report: &mut ZaloProbeReport, message: &Message) {
    let (base, thread_id, is_self, is_group) = match message {
        Message::User(message) => (&message.data, &message.thread_id, message.is_self, false),
        Message::Group(message) => (
            &message.data.base,
            &message.thread_id,
            message.is_self,
            true,
        ),
    };

    if is_group {
        report.group_messages += 1;
    } else {
        report.direct_messages += 1;
    }
    if is_self {
        report.self_messages += 1;
    }
    if !base.msg_id.is_empty() {
        report.provider_message_ids.push(base.msg_id.clone());
    }
    if !base.cli_msg_id.is_empty() {
        report.client_message_ids.push(base.cli_msg_id.clone());
    }
    if !base.real_msg_id.is_empty() {
        report.real_message_ids.push(base.real_msg_id.clone());
    }
    if !thread_id.is_empty() {
        report.thread_ids.push(thread_id.clone());
    }
    if !base.ts.is_empty() {
        if let Ok(ts_ms) = base.ts.parse::<i64>() {
            let rfc = chrono::DateTime::from_timestamp_millis(ts_ms)
                .unwrap_or_else(chrono::Utc::now)
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            report.occurred_at_timestamps.push(rfc);
        }
    }
}

pub fn observe_listener_event(report: &mut ZaloProbeReport, event: &ListenerEvent) {
    match event {
        ListenerEvent::Message(message) => observe_message(report, message),
        ListenerEvent::OldMessages {
            messages,
            thread_type,
        } => {
            match thread_type {
                zca_rs::models::ThreadType::User => report.old_direct_batches += 1,
                zca_rs::models::ThreadType::Group => report.old_group_batches += 1,
            }
            for message in messages {
                observe_message(report, message);
            }
        }
        ListenerEvent::Reaction(_) | ListenerEvent::OldReactions { .. } => {
            report.reactions += 1;
        }
        ListenerEvent::Undo(_) => report.undo_events += 1,
        ListenerEvent::DeliveredMessages(events) => report.delivered_events += events.len(),
        ListenerEvent::SeenMessages(events) => report.seen_events += events.len(),
        ListenerEvent::GroupEvent(_) => report.group_events += 1,
        ListenerEvent::Connected
        | ListenerEvent::Disconnected { .. }
        | ListenerEvent::Closed { .. }
        | ListenerEvent::Error(_)
        | ListenerEvent::Typing(_)
        | ListenerEvent::UploadAttachment(_)
        | ListenerEvent::FriendEvent(_) => {}
    }
}
