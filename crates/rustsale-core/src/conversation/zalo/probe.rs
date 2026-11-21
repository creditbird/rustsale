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
    pub connected_events: usize,
    pub disconnected_events: usize,
    pub error_events: usize,
    pub error_message_lengths: Vec<usize>,
    pub provider_message_ids: Vec<String>,
    pub direct_provider_message_ids: Vec<String>,
    pub group_provider_message_ids: Vec<String>,
    pub client_message_ids: Vec<String>,
    pub real_message_ids: Vec<String>,
    pub thread_ids: Vec<String>,
    pub occurred_at_timestamps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SanitizedZaloProbeSummary {
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
    pub connected_events: usize,
    pub disconnected_events: usize,
    pub error_events: usize,
    pub error_message_lengths: Vec<usize>,
    pub provider_message_id_lengths: Vec<usize>,
    pub client_message_id_lengths: Vec<usize>,
    pub real_message_id_lengths: Vec<usize>,
    pub thread_id_lengths: Vec<usize>,
    pub occurred_at_count: usize,
    pub earliest_occurred_at: Option<String>,
    pub latest_occurred_at: Option<String>,
}

impl ZaloProbeReport {
    pub fn sanitized_summary(&self) -> SanitizedZaloProbeSummary {
        SanitizedZaloProbeSummary {
            direct_messages: self.direct_messages,
            group_messages: self.group_messages,
            old_direct_batches: self.old_direct_batches,
            old_group_batches: self.old_group_batches,
            reactions: self.reactions,
            undo_events: self.undo_events,
            delivered_events: self.delivered_events,
            seen_events: self.seen_events,
            group_events: self.group_events,
            self_messages: self.self_messages,
            connected_events: self.connected_events,
            disconnected_events: self.disconnected_events,
            error_events: self.error_events,
            error_message_lengths: self.error_message_lengths.clone(),
            provider_message_id_lengths: self
                .provider_message_ids
                .iter()
                .map(String::len)
                .collect(),
            client_message_id_lengths: self.client_message_ids.iter().map(String::len).collect(),
            real_message_id_lengths: self.real_message_ids.iter().map(String::len).collect(),
            thread_id_lengths: self.thread_ids.iter().map(String::len).collect(),
            occurred_at_count: self.occurred_at_timestamps.len(),
            earliest_occurred_at: self.occurred_at_timestamps.iter().min().cloned(),
            latest_occurred_at: self.occurred_at_timestamps.iter().max().cloned(),
        }
    }
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
        if is_group {
            report.group_provider_message_ids.push(base.msg_id.clone());
        } else {
            report.direct_provider_message_ids.push(base.msg_id.clone());
        }
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
        ListenerEvent::Connected => report.connected_events += 1,
        ListenerEvent::Disconnected { .. } | ListenerEvent::Closed { .. } => {
            report.disconnected_events += 1;
        }
        ListenerEvent::Error(error) | ListenerEvent::PacketError { error, .. } => {
            report.error_events += 1;
            report.error_message_lengths.push(error.len());
        }
        ListenerEvent::Typing(_)
        | ListenerEvent::UploadAttachment(_)
        | ListenerEvent::FriendEvent(_) => {}
    }
}
