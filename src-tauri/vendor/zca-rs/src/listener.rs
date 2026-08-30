//! Real-time WebSocket connection listener.
//!
//! Handles connections, automatic retry on close, rotatable endpoints,
//! heartbeat pings, encrypted payload decoding, and event routing.

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use url::Url;

use crate::context::{Context, UploadEventData};
use crate::crypto::decode_event_data;
use crate::error::ZaloError;
use crate::models::{
    get_friend_event_type, get_group_event_type, initialize_friend_event, initialize_group_event,
    DeliveredMessage, FriendEvent, GroupDeliveredMessage, GroupEvent, GroupMessage,
    GroupSeenMessage, GroupTyping, Message as ModelMessage, Reaction, SeenMessage, ThreadType,
    Typing, Undo, UserDeliveredMessage, UserMessage, UserSeenMessage, UserTyping,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    ManualClosure,
    AbnormalClosure,
    DuplicateConnection,
    KickConnection,
    Other(u16),
}

impl From<u16> for CloseReason {
    fn from(code: u16) -> Self {
        match code {
            1000 => CloseReason::ManualClosure,
            1006 => CloseReason::AbnormalClosure,
            3000 => CloseReason::DuplicateConnection,
            3003 => CloseReason::KickConnection,
            other => CloseReason::Other(other),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "data")]
pub enum ListenerEvent {
    Connected,
    Disconnected {
        code: u16,
        reason: String,
    },
    Closed {
        code: u16,
        reason: String,
    },
    Error(String),
    PacketError {
        cmd: u16,
        sub_cmd: u8,
        error: String,
    },
    Typing(Typing),
    Message(Box<ModelMessage>),
    OldMessages {
        messages: Vec<ModelMessage>,
        thread_type: ThreadType,
    },
    SeenMessages(Vec<SeenMessage>),
    DeliveredMessages(Vec<DeliveredMessage>),
    Reaction(Reaction),
    OldReactions {
        reactions: Vec<Reaction>,
        is_group: bool,
    },
    UploadAttachment(UploadEventData),
    Undo(Box<Undo>),
    FriendEvent(FriendEvent),
    GroupEvent(GroupEvent),
}

#[derive(Debug)]
pub struct OutgoingFrame {
    pub version: u8,
    pub cmd: u16,
    pub sub_cmd: u8,
    pub data: Value,
    pub require_id: bool,
}

struct RetryState {
    count: u32,
    max: u32,
    times: Vec<u64>,
}

#[derive(Debug)]
pub struct Listener {
    ctx: Arc<Context>,
    urls: Vec<String>,
    event_tx: broadcast::Sender<ListenerEvent>,
    active_outgoing_tx: Mutex<Option<mpsc::Sender<OutgoingFrame>>>,
    shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,
}

impl Listener {
    pub fn new(ctx: Arc<Context>, urls: Vec<String>) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            ctx,
            urls,
            event_tx,
            active_outgoing_tx: Mutex::new(None),
            shutdown_tx: Mutex::new(None),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ListenerEvent> {
        self.event_tx.subscribe()
    }

    pub fn start(&self) -> Result<(), ZaloError> {
        let mut shutdown_guard = self.shutdown_tx.lock().unwrap_or_else(|e| e.into_inner());
        if shutdown_guard.is_some() {
            return Err(ZaloError::Crypto("Listener already started".to_string()));
        }

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        *shutdown_guard = Some(shutdown_tx);

        let (outgoing_tx, outgoing_rx) = mpsc::channel(100);
        let mut outgoing_guard = self
            .active_outgoing_tx
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *outgoing_guard = Some(outgoing_tx);

        let ctx = Arc::clone(&self.ctx);
        let urls = self.urls.clone();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            connection_loop(ctx, urls, event_tx, outgoing_rx, shutdown_rx).await;
        });

        Ok(())
    }

    pub fn stop(&self) {
        let mut shutdown_guard = self.shutdown_tx.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(tx) = shutdown_guard.take() {
            let _ = tx.send(());
        }
        let mut outgoing_guard = self
            .active_outgoing_tx
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *outgoing_guard = None;
    }

    pub fn send_ws(
        &self,
        version: u8,
        cmd: u16,
        sub_cmd: u8,
        data: Value,
        require_id: bool,
    ) -> Result<(), ZaloError> {
        let guard = self
            .active_outgoing_tx
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(tx) = guard.as_ref() {
            tx.try_send(OutgoingFrame {
                version,
                cmd,
                sub_cmd,
                data,
                require_id,
            })
            .map_err(|e| ZaloError::PayloadFormat(format!("Failed to queue outgoing frame: {}", e)))
        } else {
            Err(ZaloError::Crypto("Listener not running".to_string()))
        }
    }

    pub fn request_old_messages(
        &self,
        thread_type: ThreadType,
        last_msg_id: Option<&str>,
    ) -> Result<(), ZaloError> {
        let (cmd, data) = old_messages_request_frame(thread_type, last_msg_id);
        self.send_ws(1, cmd, 1, data, true)
    }

    pub fn request_old_reactions(
        &self,
        thread_type: ThreadType,
        last_msg_id: Option<&str>,
    ) -> Result<(), ZaloError> {
        let cmd = match thread_type {
            ThreadType::User => 610,
            ThreadType::Group => 611,
        };
        let data = serde_json::json!({
            "first": true,
            "lastId": last_msg_id,
            "preIds": []
        });
        self.send_ws(1, cmd, 1, data, true)
    }
}

fn old_messages_request_frame(thread_type: ThreadType, last_msg_id: Option<&str>) -> (u16, Value) {
    let command = match thread_type {
        ThreadType::User => 510,
        ThreadType::Group => 511,
    };
    let payload = serde_json::json!({
        "first": true,
        "lastId": last_msg_id,
        "preIds": []
    });
    (command, payload)
}

async fn connection_loop(
    ctx: Arc<Context>,
    urls: Vec<String>,
    event_tx: broadcast::Sender<ListenerEvent>,
    mut outgoing_rx: mpsc::Receiver<OutgoingFrame>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    let mut rotate_count = 0;
    let mut id_counter = 0u64;
    let self_listen = ctx.options.self_listen;

    let mut retry_states = HashMap::new();
    {
        let settings = ctx.settings.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(socket_settings) = settings.get("features").and_then(|v| v.get("socket")) {
            if let Some(retries_obj) = socket_settings.get("retries").and_then(|v| v.as_object()) {
                for (code_str, val) in retries_obj {
                    if let Ok(code) = code_str.parse::<i64>() {
                        let max = val.get("max").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        let times = if let Some(t_arr) = val.get("times").and_then(|v| v.as_array())
                        {
                            t_arr.iter().filter_map(|v| v.as_u64()).collect()
                        } else if let Some(t_num) = val.get("times").and_then(|v| v.as_u64()) {
                            vec![t_num]
                        } else {
                            vec![1000]
                        };
                        retry_states.insert(
                            code,
                            RetryState {
                                count: 0,
                                max,
                                times,
                            },
                        );
                    }
                }
            }
        }
    }

    'main: loop {
        let base_url = match urls.get(rotate_count) {
            Some(u) => u.clone(),
            None => {
                if !urls.is_empty() {
                    urls.first().cloned().unwrap_or_default()
                } else {
                    let _ =
                        event_tx.send(ListenerEvent::Error("No WebSocket URLs provided".into()));
                    break;
                }
            }
        };

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let ws_url = match ctx.make_url(&base_url, &[("t", &now_ms.to_string())], true) {
            Ok(u) => u,
            Err(e) => {
                let _ = event_tx.send(ListenerEvent::Error(format!(
                    "Failed to make WS URL: {}",
                    e
                )));
                break;
            }
        };

        log::info!("Connecting to WebSocket: {}", ws_url);

        let url_parsed = match Url::parse(&ws_url) {
            Ok(u) => u,
            Err(e) => {
                let _ = event_tx.send(ListenerEvent::Error(format!("Invalid WS URL: {}", e)));
                break;
            }
        };

        let chat_url = match Url::parse("https://chat.zalo.me") {
            Ok(u) => u,
            Err(e) => {
                let _ = event_tx.send(ListenerEvent::Error(format!("Invalid chat URL: {}", e)));
                break;
            }
        };

        let cookie_header = ctx.cookie_jar.get_cookie_string(&chat_url);

        let request = match http::Request::builder()
            .method("GET")
            .uri(&ws_url)
            .header(
                "Host",
                url_parsed.host_str().unwrap_or("wscan.chat.zalo.me"),
            )
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                tokio_tungstenite::tungstenite::handshake::client::generate_key(),
            )
            .header("Origin", "https://chat.zalo.me")
            .header("User-Agent", &ctx.user_agent)
            .header("Cookie", &cookie_header)
            .header("Accept-Encoding", "gzip, deflate, br, zstd")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Cache-Control", "no-cache")
            .header("Pragma", "no-cache")
            .body(())
        {
            Ok(req) => req,
            Err(e) => {
                let _ = event_tx.send(ListenerEvent::Error(format!(
                    "Failed to build request: {}",
                    e
                )));
                break;
            }
        };

        let ws_connect = tokio_tungstenite::connect_async(request);

        let ws_stream = tokio::select! {
            res = ws_connect => {
                match res {
                    Ok((stream, _)) => stream,
                    Err(e) => {
                        let _ = event_tx.send(ListenerEvent::Error(format!("WebSocket connection failed: {}", e)));
                        let code = 1006;
                        if handle_reconnect_or_exit(code, &ctx, &urls, &mut rotate_count, &mut retry_states, &event_tx).await {
                            continue 'main;
                        } else {
                            break 'main;
                        }
                    }
                }
            }
            _ = &mut shutdown_rx => {
                log::info!("Listener connection aborted by shutdown signal");
                break 'main;
            }
        };

        let (mut ws_write, mut ws_read) = ws_stream.split();
        let mut cipher_key: Option<String> = None;
        let (ping_tx, mut ping_rx) = mpsc::channel::<()>(1);
        let mut ping_interval_task: Option<tokio::task::JoinHandle<()>> = None;

        let (write_tx, mut write_rx) = mpsc::channel::<Vec<u8>>(32);

        let write_forward_task = tokio::spawn(async move {
            while let Some(data) = write_rx.recv().await {
                if let Err(e) = ws_write.send(WsMessage::Binary(data)).await {
                    log::error!("Error writing binary message: {}", e);
                    break;
                }
            }
        });

        let mut connection_shutdown = false;

        while !connection_shutdown {
            tokio::select! {
                msg_res = ws_read.next() => {
                    match msg_res {
                        Some(Ok(WsMessage::Binary(data))) => {
                            if data.len() < 4 {
                                continue;
                            }
                            let version = data.first().copied().unwrap_or(0);
                            let cmd = u16::from_le_bytes([
                                data.get(1).copied().unwrap_or(0),
                                data.get(2).copied().unwrap_or(0),
                            ]);
                            let sub_cmd = data.get(3).copied().unwrap_or(0);

                            let Some(body) = data.get(4..) else {
                                continue;
                            };
                            let decoded_str = match String::from_utf8(body.to_vec()) {
                                Ok(s) => s,
                                Err(_) => continue,
                            };

                            if decoded_str.is_empty() {
                                continue;
                            }

                            let parsed: Value = match serde_json::from_str(&decoded_str) {
                                Ok(v) => v,
                                Err(_) => continue,
                            };

                            if version == 1 && cmd == 1 && sub_cmd == 1 {
                                if let Some(key) = parsed.get("key").and_then(|v| v.as_str()) {
                                    cipher_key = Some(key.to_string());
                                    let _ = event_tx.send(ListenerEvent::Connected);

                                    let ping_tx_clone = ping_tx.clone();
                                    let ping_interval = {
                                        let settings = ctx.settings.lock().unwrap_or_else(|e| e.into_inner());
                                        settings.get("features")
                                            .and_then(|v| v.get("socket"))
                                            .and_then(|v| v.get("ping_interval"))
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(20000)
                                    };

                                    if let Some(task) = ping_interval_task.take() {
                                        task.abort();
                                    }

                                    ping_interval_task = Some(tokio::spawn(async move {
                                        loop {
                                            tokio::time::sleep(Duration::from_millis(ping_interval)).await;
                                            if ping_tx_clone.send(()).await.is_err() {
                                                break;
                                            }
                                        }
                                    }));
                                }
                            }

                            if version == 1 && cmd == 3000 && sub_cmd == 0 {
                                log::error!("Another connection is opened, closing this one");
                                let _ = event_tx.send(ListenerEvent::Disconnected { code: 3000, reason: "Duplicate Connection".into() });
                                break 'main;
                            }

                            if let Err(e) = handle_incoming_packet(version, cmd, sub_cmd, parsed, cipher_key.as_deref(), &ctx, self_listen, &event_tx).await {
                                log::error!("Error handling incoming packet: {:?}", e);
                                let _ = event_tx.send(ListenerEvent::PacketError {
                                    cmd,
                                    sub_cmd,
                                    error: e.to_string(),
                                });
                            }
                        }
                        Some(Ok(WsMessage::Close(frame))) => {
                            let code = frame.as_ref().map(|f| u16::from(f.code)).unwrap_or(1000);
                            let reason = frame.as_ref().map(|f| f.reason.to_string()).unwrap_or_default();
                            let _ = event_tx.send(ListenerEvent::Disconnected { code, reason: reason.clone() });

                            connection_shutdown = true;
                            if handle_reconnect_or_exit(code, &ctx, &urls, &mut rotate_count, &mut retry_states, &event_tx).await {
                                // Will restart connection loop
                            } else {
                                let _ = event_tx.send(ListenerEvent::Closed { code, reason });
                                break 'main;
                            }
                        }
                        Some(Err(e)) => {
                            let _ = event_tx.send(ListenerEvent::Error(format!("Read error: {}", e)));
                            connection_shutdown = true;
                            let code = 1006;
                            if handle_reconnect_or_exit(code, &ctx, &urls, &mut rotate_count, &mut retry_states, &event_tx).await {
                                // Will restart connection loop
                            } else {
                                break 'main;
                            }
                        }
                        None => {
                            let _ = event_tx.send(ListenerEvent::Disconnected { code: 1006, reason: "Connection closed by peer".into() });
                            connection_shutdown = true;
                            let code = 1006;
                            if handle_reconnect_or_exit(code, &ctx, &urls, &mut rotate_count, &mut retry_states, &event_tx).await {
                                // Will restart connection loop
                            } else {
                                break 'main;
                            }
                        }
                        _ => {}
                    }
                }

                outgoing_frame = outgoing_rx.recv() => {
                    if let Some(mut frame) = outgoing_frame {
                        if frame.require_id {
                            if let Some(obj) = frame.data.as_object_mut() {
                                id_counter += 1;
                                obj.insert("req_id".to_string(), Value::String(format!("req_{}", id_counter)));
                            }
                        }

                        let data_str = match serde_json::to_string(&frame.data) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let data_bytes = data_str.as_bytes();

                        let mut buf = Vec::with_capacity(4 + data_bytes.len());
                        buf.push(frame.version);
                        buf.extend_from_slice(&frame.cmd.to_le_bytes());
                        buf.push(frame.sub_cmd);
                        buf.extend_from_slice(data_bytes);

                        let _ = write_tx.send(buf).await;
                    }
                }

                _ = ping_rx.recv() => {
                    let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
                    let payload = serde_json::json!({ "eventId": now_ms });
                    let data_str = match serde_json::to_string(&payload) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    let data_bytes = data_str.as_bytes();

                    let mut buf = Vec::with_capacity(4 + data_bytes.len());
                    buf.push(1);
                    buf.extend_from_slice(&2u16.to_le_bytes());
                    buf.push(1);
                    buf.extend_from_slice(data_bytes);

                    let _ = write_tx.send(buf).await;
                }

                _ = &mut shutdown_rx => {
                    log::info!("Listener connection loop received shutdown signal");
                    break 'main;
                }
            }
        }

        if let Some(task) = ping_interval_task {
            task.abort();
        }
        write_forward_task.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::old_messages_request_frame;
    use crate::models::ThreadType;

    #[test]
    fn group_history_request_is_account_wide_command_511() {
        let (command, payload) = old_messages_request_frame(ThreadType::Group, Some("cursor-1"));

        assert_eq!(command, 511);
        assert_eq!(
            payload.get("lastId").and_then(serde_json::Value::as_str),
            Some("cursor-1")
        );
        assert!(payload.get("groupId").is_none());
        assert!(payload.get("grid").is_none());
    }
}

async fn handle_reconnect_or_exit(
    code: u16,
    ctx: &Context,
    urls: &[String],
    rotate_count: &mut usize,
    retry_states: &mut HashMap<i64, RetryState>,
    event_tx: &broadcast::Sender<ListenerEvent>,
) -> bool {
    let (close_and_retry_codes, rotate_error_codes) = {
        let settings = ctx.settings.lock().unwrap_or_else(|e| e.into_inner());
        let socket_settings = settings.get("features").and_then(|v| v.get("socket"));

        let close_and_retry_codes: Vec<i64> = socket_settings
            .and_then(|v| v.get("close_and_retry_codes"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|val| val.as_i64()).collect())
            .unwrap_or_default();

        let rotate_error_codes: Vec<i64> = socket_settings
            .and_then(|v| v.get("rotate_error_codes"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|val| val.as_i64()).collect())
            .unwrap_or_default();
        (close_and_retry_codes, rotate_error_codes)
    };

    let code_i64 = code as i64;

    if !close_and_retry_codes.contains(&code_i64) {
        return false;
    }

    let state = match retry_states.get_mut(&code_i64) {
        Some(s) => s,
        None => return false,
    };

    if state.count >= state.max {
        return false;
    }

    state.count += 1;
    let idx = (state.count - 1) as usize;
    let retry_time = if idx < state.times.len() {
        state.times.get(idx).copied().unwrap_or(1000)
    } else {
        state.times.last().copied().unwrap_or(1000)
    };

    let _ = event_tx.send(ListenerEvent::Error(format!(
        "Retry for code {} in {}ms ({}/{})",
        code, retry_time, state.count, state.max
    )));

    if rotate_error_codes.contains(&code_i64) && *rotate_count < urls.len().saturating_sub(1) {
        *rotate_count += 1;
    }

    tokio::time::sleep(Duration::from_millis(retry_time)).await;
    true
}

#[expect(clippy::too_many_arguments)]
async fn handle_incoming_packet(
    version: u8,
    cmd: u16,
    sub_cmd: u8,
    parsed: Value,
    cipher_key: Option<&str>,
    ctx: &Context,
    self_listen: bool,
    event_tx: &broadcast::Sender<ListenerEvent>,
) -> Result<(), ZaloError> {
    let uid = {
        let guard = ctx.uid.lock().unwrap_or_else(|e| e.into_inner());
        guard.clone().unwrap_or_default()
    };

    if version == 1 && cmd == 501 && sub_cmd == 0 {
        let decoded = decode_event_data(&parsed, cipher_key)?;
        if let Some(msgs) = decoded.get("msgs").and_then(|v| v.as_array()) {
            for msg_val in msgs {
                let content = msg_val.get("content");
                let is_undo = content
                    .map(|c| c.get("deleteMsg").is_some())
                    .unwrap_or(false);

                if is_undo {
                    let undo_data: crate::models::event::TUndo =
                        serde_json::from_value(msg_val.clone())?;
                    let undo = Undo::new(&uid, undo_data, false);
                    if undo.is_self && !self_listen {
                        continue;
                    }
                    let _ = event_tx.send(ListenerEvent::Undo(Box::new(undo)));
                } else {
                    let msg_data: crate::models::message::TMessage =
                        serde_json::from_value(msg_val.clone())?;
                    let user_msg = UserMessage::new(&uid, msg_data);
                    if user_msg.is_self && !self_listen {
                        continue;
                    }
                    let _ = event_tx.send(ListenerEvent::Message(Box::new(ModelMessage::User(
                        user_msg,
                    ))));
                }
            }
        }
    } else if version == 1 && cmd == 521 && sub_cmd == 0 {
        let decoded = decode_event_data(&parsed, cipher_key)?;
        if let Some(group_msgs) = decoded.get("groupMsgs").and_then(|v| v.as_array()) {
            for msg_val in group_msgs {
                let content = msg_val.get("content");
                let is_undo = content
                    .map(|c| c.get("deleteMsg").is_some())
                    .unwrap_or(false);

                if is_undo {
                    let undo_data: crate::models::event::TUndo =
                        serde_json::from_value(msg_val.clone())?;
                    let undo = Undo::new(&uid, undo_data, true);
                    if undo.is_self && !self_listen {
                        continue;
                    }
                    let _ = event_tx.send(ListenerEvent::Undo(Box::new(undo)));
                } else {
                    let msg_data: crate::models::message::TGroupMessage =
                        serde_json::from_value(msg_val.clone())?;
                    let group_msg = GroupMessage::new(&uid, msg_data);
                    if group_msg.is_self && !self_listen {
                        continue;
                    }
                    let _ = event_tx.send(ListenerEvent::Message(Box::new(ModelMessage::Group(
                        group_msg,
                    ))));
                }
            }
        }
    } else if version == 1 && cmd == 601 && sub_cmd == 0 {
        let decoded = decode_event_data(&parsed, cipher_key)?;
        if let Some(controls) = decoded.get("controls").and_then(|v| v.as_array()) {
            for control_val in controls {
                let act_type = control_val
                    .get("content")
                    .and_then(|v| v.get("act_type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if act_type == "file_done" {
                    let file_id = control_val
                        .get("content")
                        .and_then(|v| v.get("fileId"))
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            _ => "".to_string(),
                        })
                        .unwrap_or_default();
                    let file_url = control_val
                        .get("content")
                        .and_then(|v| v.get("data"))
                        .and_then(|v| v.get("url"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let data = UploadEventData {
                        file_url,
                        file_id: file_id.clone(),
                    };

                    if !file_id.is_empty() {
                        let mut callbacks = ctx
                            .upload_callbacks
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        if let Some(sender) = callbacks.remove(&file_id) {
                            let _ = sender.send(data.clone());
                        }
                    }

                    let _ = event_tx.send(ListenerEvent::UploadAttachment(data));
                } else if act_type == "group" {
                    let act = control_val
                        .get("content")
                        .and_then(|v| v.get("act"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if act == "join_reject" {
                        continue;
                    }
                    let data_val = control_val
                        .get("content")
                        .and_then(|v| v.get("data"))
                        .cloned()
                        .unwrap_or(Value::Null);
                    let parsed_data = if let Some(s) = data_val.as_str() {
                        serde_json::from_str(s).unwrap_or(data_val)
                    } else {
                        data_val
                    };

                    let group_event = initialize_group_event(
                        &uid,
                        parsed_data,
                        get_group_event_type(act),
                        act.to_string(),
                    );
                    if group_event.is_self && !self_listen {
                        continue;
                    }
                    let _ = event_tx.send(ListenerEvent::GroupEvent(group_event));
                } else if act_type == "fr" {
                    let act = control_val
                        .get("content")
                        .and_then(|v| v.get("act"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if act == "req" {
                        continue;
                    }
                    let data_val = control_val
                        .get("content")
                        .and_then(|v| v.get("data"))
                        .cloned()
                        .unwrap_or(Value::Null);
                    let mut parsed_data = if let Some(s) = data_val.as_str() {
                        serde_json::from_str(s).unwrap_or(data_val)
                    } else {
                        data_val
                    };

                    if let Some(obj) = parsed_data.as_object_mut() {
                        if let Some(topic) = obj.get_mut("topic") {
                            if let Some(topic_obj) = topic.as_object_mut() {
                                if let Some(params_val) = topic_obj.get_mut("params") {
                                    if let Some(params_str) = params_val.as_str() {
                                        if let Ok(parsed_params) =
                                            serde_json::from_str::<Value>(params_str)
                                        {
                                            *params_val = parsed_params;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    let friend_event =
                        initialize_friend_event(&uid, parsed_data, get_friend_event_type(act));
                    if friend_event.is_self && !self_listen {
                        continue;
                    }
                    let _ = event_tx.send(ListenerEvent::FriendEvent(friend_event));
                }
            }
        }
    } else if cmd == 612 {
        let decoded = decode_event_data(&parsed, cipher_key)?;
        if let Some(reacts) = decoded.get("reacts").and_then(|v| v.as_array()) {
            for react_val in reacts {
                let mut react_clone = react_val.clone();
                if let Some(content_str) = react_clone.get("content").and_then(|v| v.as_str()) {
                    if let Ok(content_parsed) = serde_json::from_str::<Value>(content_str) {
                        if let Some(obj) = react_clone.as_object_mut() {
                            obj.insert("content".to_string(), content_parsed);
                        }
                    }
                }
                let react_data: crate::models::event::TReaction =
                    serde_json::from_value(react_clone)?;
                let reaction = Reaction::new(&uid, react_data, false);
                if reaction.is_self && !self_listen {
                    continue;
                }
                let _ = event_tx.send(ListenerEvent::Reaction(reaction));
            }
        }
        if let Some(react_groups) = decoded.get("reactGroups").and_then(|v| v.as_array()) {
            for react_val in react_groups {
                let mut react_clone = react_val.clone();
                if let Some(content_str) = react_clone.get("content").and_then(|v| v.as_str()) {
                    if let Ok(content_parsed) = serde_json::from_str::<Value>(content_str) {
                        if let Some(obj) = react_clone.as_object_mut() {
                            obj.insert("content".to_string(), content_parsed);
                        }
                    }
                }
                let react_data: crate::models::event::TReaction =
                    serde_json::from_value(react_clone)?;
                let reaction = Reaction::new(&uid, react_data, true);
                if reaction.is_self && !self_listen {
                    continue;
                }
                let _ = event_tx.send(ListenerEvent::Reaction(reaction));
            }
        }
    } else if cmd == 610 || cmd == 611 {
        let decoded = decode_event_data(&parsed, cipher_key)?;
        let is_group = cmd == 611;
        let key_name = if is_group { "reactGroups" } else { "reacts" };
        if let Some(reacts) = decoded.get(key_name).and_then(|v| v.as_array()) {
            let mut reactions = Vec::new();
            for react_val in reacts {
                let mut react_clone = react_val.clone();
                if let Some(content_str) = react_clone.get("content").and_then(|v| v.as_str()) {
                    if let Ok(content_parsed) = serde_json::from_str::<Value>(content_str) {
                        if let Some(obj) = react_clone.as_object_mut() {
                            obj.insert("content".to_string(), content_parsed);
                        }
                    }
                }
                let react_data: crate::models::event::TReaction =
                    serde_json::from_value(react_clone)?;
                reactions.push(Reaction::new(&uid, react_data, is_group));
            }
            let _ = event_tx.send(ListenerEvent::OldReactions {
                reactions,
                is_group,
            });
        }
    } else if cmd == 510 && sub_cmd == 1 {
        let decoded = decode_event_data(&parsed, cipher_key)?;
        if let Some(msgs) = decoded.get("msgs").and_then(|v| v.as_array()) {
            let mut messages = Vec::new();
            for msg_val in msgs {
                let msg_data: crate::models::message::TMessage =
                    serde_json::from_value(msg_val.clone())?;
                messages.push(ModelMessage::User(UserMessage::new(&uid, msg_data)));
            }
            let _ = event_tx.send(ListenerEvent::OldMessages {
                messages,
                thread_type: ThreadType::User,
            });
        }
    } else if cmd == 511 && sub_cmd == 1 {
        let decoded = decode_event_data(&parsed, cipher_key)?;
        if let Some(group_msgs) = decoded.get("groupMsgs").and_then(|v| v.as_array()) {
            let mut messages = Vec::new();
            for msg_val in group_msgs {
                let msg_data: crate::models::message::TGroupMessage =
                    serde_json::from_value(msg_val.clone())?;
                messages.push(ModelMessage::Group(GroupMessage::new(&uid, msg_data)));
            }
            let _ = event_tx.send(ListenerEvent::OldMessages {
                messages,
                thread_type: ThreadType::Group,
            });
        }
    } else if cmd == 602 && sub_cmd == 0 {
        let decoded = decode_event_data(&parsed, cipher_key)?;
        if let Some(actions) = decoded.get("actions").and_then(|v| v.as_array()) {
            for action_val in actions {
                let act_type = action_val
                    .get("act_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if act_type == "typing" {
                    let act = action_val.get("act").and_then(|v| v.as_str()).unwrap_or("");
                    let data_str = action_val
                        .get("data")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let wrapped_json = format!("{{{}}}", data_str);
                    if act == "typing" {
                        let typing_data: crate::models::event::TTyping =
                            serde_json::from_str(&wrapped_json)?;
                        let _ = event_tx.send(ListenerEvent::Typing(Typing::User(
                            UserTyping::new(typing_data),
                        )));
                    } else if act == "gtyping" {
                        let typing_data: crate::models::event::TGroupTyping =
                            serde_json::from_str(&wrapped_json)?;
                        let _ = event_tx.send(ListenerEvent::Typing(Typing::Group(
                            GroupTyping::new(typing_data),
                        )));
                    }
                }
            }
        }
    } else if cmd == 502 && sub_cmd == 0 {
        let decoded = decode_event_data(&parsed, cipher_key)?;
        let mut delivered_events = Vec::new();
        if let Some(delivereds) = decoded.get("delivereds").and_then(|v| v.as_array()) {
            for del_val in delivereds {
                match serde_json::from_value::<crate::models::event::TDeliveredMessage>(del_val.clone()) {
                    Ok(del_data) if del_data.is_valid() => {
                        delivered_events.push(DeliveredMessage::User(UserDeliveredMessage::new(del_data)));
                    }
                    Ok(_) => {
                        log::warn!("Ignored semantically invalid TDeliveredMessage in cmd 502");
                    }
                    Err(e) => {
                        log::warn!("Failed to parse TDeliveredMessage in cmd 502: {}", e);
                    }
                }
            }
        }
        if !delivered_events.is_empty() {
            let _ = event_tx.send(ListenerEvent::DeliveredMessages(delivered_events));
        }

        let mut seen_events = Vec::new();
        if let Some(seens) = decoded.get("seens").and_then(|v| v.as_array()) {
            for seen_val in seens {
                match serde_json::from_value::<crate::models::event::TUserSeenMessage>(seen_val.clone()) {
                    Ok(seen_data) if seen_data.is_valid() => {
                        seen_events.push(SeenMessage::User(UserSeenMessage::new(seen_data)));
                    }
                    Ok(_) => {
                        log::warn!("Ignored semantically invalid TUserSeenMessage in cmd 502");
                    }
                    Err(e) => {
                        log::warn!("Failed to parse TUserSeenMessage in cmd 502: {}", e);
                    }
                }
            }
        }
        if !seen_events.is_empty() {
            let _ = event_tx.send(ListenerEvent::SeenMessages(seen_events));
        }
    } else if cmd == 522 && sub_cmd == 0 {
        let decoded = decode_event_data(&parsed, cipher_key)?;
        let mut delivered_events = Vec::new();
        if let Some(delivereds) = decoded.get("delivereds").and_then(|v| v.as_array()) {
            for del_val in delivereds {
                match serde_json::from_value::<crate::models::event::TGroupDeliveredMessage>(del_val.clone()) {
                    Ok(del_data) if del_data.is_valid() => {
                        let event = GroupDeliveredMessage::new(&uid, del_data);
                        if event.is_self && !self_listen {
                            continue;
                        }
                        delivered_events.push(DeliveredMessage::Group(event));
                    }
                    Ok(_) => {
                        log::warn!("Ignored semantically invalid TGroupDeliveredMessage in cmd 522");
                    }
                    Err(e) => {
                        log::warn!("Failed to parse TGroupDeliveredMessage in cmd 522: {}", e);
                    }
                }
            }
        }
        if !delivered_events.is_empty() {
            let _ = event_tx.send(ListenerEvent::DeliveredMessages(delivered_events));
        }

        let mut seen_events = Vec::new();
        if let Some(seens) = decoded.get("groupSeens").and_then(|v| v.as_array()) {
            for seen_val in seens {
                match serde_json::from_value::<crate::models::event::TGroupSeenMessage>(seen_val.clone()) {
                    Ok(seen_data) if seen_data.is_valid() => {
                        let event = GroupSeenMessage::new(&uid, seen_data);
                        if event.is_self && !self_listen {
                            continue;
                        }
                        seen_events.push(SeenMessage::Group(event));
                    }
                    Ok(_) => {
                        log::warn!("Ignored semantically invalid TGroupSeenMessage in cmd 522");
                    }
                    Err(e) => {
                        log::warn!("Failed to parse TGroupSeenMessage in cmd 522: {}", e);
                    }
                }
            }
        }
        if !seen_events.is_empty() {
            let _ = event_tx.send(ListenerEvent::SeenMessages(seen_events));
        }
    }

    Ok(())
}
