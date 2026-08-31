use crate::conversation::types::Channel;
use std::collections::HashMap;
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_notification::NotificationExt;

const RATE_LIMIT_WINDOW: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct NotificationSettings {
    pub enabled: bool,
    pub sound_enabled: bool,
    pub privacy_mode: bool,
    pub mute_groups: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            sound_enabled: true,
            privacy_mode: false,
            mute_groups: false,
        }
    }
}

pub fn format_notification_title(
    channel: Channel,
    sender_name: &str,
    is_group: bool,
    group_name: Option<&str>,
) -> String {
    let channel_tag = match channel {
        Channel::Zalo => "[Zalo]",
        Channel::Messenger => "[Facebook]",
        Channel::WhatsApp => "[WhatsApp]",
        Channel::Email => "[Email]",
    };

    if is_group {
        if let Some(g_name) = group_name {
            format!("{channel_tag} {g_name} ({sender_name})")
        } else {
            format!("{channel_tag} Nhóm ({sender_name})")
        }
    } else {
        format!("{channel_tag} {sender_name}")
    }
}

pub fn format_notification_body(raw_text: &str, content_type: &str, privacy_mode: bool) -> String {
    if privacy_mode {
        return "Bạn có tin nhắn mới".to_string();
    }

    match content_type {
        "image" => "[Hình ảnh]".to_string(),
        "sticker" => "[Nhãn dán]".to_string(),
        "file" => "[Tệp đính kèm]".to_string(),
        "audio" => "[Tin nhắn thoại]".to_string(),
        "video" => "[Video]".to_string(),
        _ => {
            let trimmed = raw_text.trim();
            if trimmed.is_empty() {
                "Bạn có tin nhắn mới".to_string()
            } else if trimmed.chars().count() > 120 {
                let truncated: String = trimmed.chars().take(117).collect();
                format!("{truncated}...")
            } else {
                trimmed.to_string()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FocusSyncResult {
    pub accepted: bool,
    pub backend_seq: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FocusRecord {
    pub active_conversation_id: Option<i64>,
    pub is_focused: bool,
    pub seq: u64,
}

pub struct NotificationManager {
    settings: RwLock<NotificationSettings>,
    focus_state: Mutex<FocusRecord>,
    last_notified: Mutex<HashMap<i64, Instant>>,
    tray_state: RwLock<Option<crate::tray::TrayState>>,
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self {
            settings: RwLock::new(NotificationSettings::default()),
            focus_state: Mutex::new(FocusRecord::default()),
            last_notified: Mutex::new(HashMap::new()),
            tray_state: RwLock::new(None),
        }
    }
}

impl NotificationManager {
    pub fn new(settings: NotificationSettings) -> Self {
        Self {
            settings: RwLock::new(settings),
            focus_state: Mutex::new(FocusRecord::default()),
            last_notified: Mutex::new(HashMap::new()),
            tray_state: RwLock::new(None),
        }
    }

    pub fn set_tray_state(&self, tray_state: crate::tray::TrayState) {
        *self.tray_state.write().unwrap() = Some(tray_state);
    }

    pub fn get_settings(&self) -> NotificationSettings {
        self.settings.read().unwrap().clone()
    }

    pub fn update_settings(&self, new_settings: NotificationSettings) {
        *self.settings.write().unwrap() = new_settings;
    }

    pub fn set_focus(&self, active_conversation_id: Option<i64>, is_focused: bool, seq: Option<u64>) -> FocusSyncResult {
        let mut lock = self.focus_state.lock().unwrap();
        if let Some(s) = seq {
            if s <= lock.seq {
                return FocusSyncResult {
                    accepted: false,
                    backend_seq: lock.seq,
                };
            }
            lock.seq = s;
        } else {
            lock.seq = lock.seq.saturating_add(1);
        }
        lock.active_conversation_id = active_conversation_id;
        lock.is_focused = is_focused;
        FocusSyncResult {
            accepted: true,
            backend_seq: lock.seq,
        }
    }

    pub fn get_focus_state(&self) -> (Option<i64>, bool, u64) {
        let lock = self.focus_state.lock().unwrap();
        (lock.active_conversation_id, lock.is_focused, lock.seq)
    }

    pub fn should_suppress(&self, conversation_id: i64, is_group: bool) -> bool {
        let settings = self.settings.read().unwrap();
        if !settings.enabled {
            return true;
        }

        if let Some(ref tray) = *self.tray_state.read().unwrap() {
            if tray.is_dnd_enabled() {
                return true;
            }
        }

        if settings.mute_groups && is_group {
            return true;
        }

        let (active_id, is_focused, _) = self.get_focus_state();
        if is_focused && active_id == Some(conversation_id) {
            return true;
        }

        false
    }

    pub fn check_and_record_rate_limit(&self, conversation_id: i64) -> bool {
        let now = Instant::now();
        let mut map = self.last_notified.lock().unwrap();

        // Prune expired entries when map size grows to prevent memory leaks
        if map.len() > 64 {
            map.retain(|_, &mut time| now.duration_since(time) <= RATE_LIMIT_WINDOW * 2);
        }

        if let Some(last_time) = map.get(&conversation_id) {
            if now.duration_since(*last_time) < RATE_LIMIT_WINDOW {
                return false;
            }
        }

        map.insert(conversation_id, now);
        true
    }

    pub fn notify_inbound<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        conversation_id: i64,
        lead_id: Option<i64>,
        channel: Channel,
        sender_name: &str,
        raw_text: &str,
        content_type: &str,
        is_group: bool,
        group_name: Option<&str>,
    ) {
        if self.should_suppress(conversation_id, is_group) {
            return;
        }

        if !self.check_and_record_rate_limit(conversation_id) {
            return;
        }

        let settings = self.get_settings();
        let title = format_notification_title(channel, sender_name, is_group, group_name);
        let body = format_notification_body(raw_text, content_type, settings.privacy_mode);

        if let Err(error) = dispatch_desktop_notification(app, &title, &body) {
            tracing::warn!(target: "notification", %error, "Failed to deliver inbound desktop notification");
        } else {
            let payload = serde_json::json!({
                "conversationId": conversation_id,
                "leadId": lead_id,
                "channel": channel,
                "sound": settings.sound_enabled
            });
            let _ = app.emit("desktop-notification-sent", payload);
        }
    }
}

pub fn show_plugin_notification<R: Runtime>(
    app: &AppHandle<R>,
    title: &str,
    body: &str,
) -> Result<(), String> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|err| format!("Desktop notification delivery failed: {err}"))
}

#[cfg(target_os = "linux")]
pub fn execute_linux_notify_send(title: &str, body: &str) -> Result<bool, String> {
    let mut child = std::process::Command::new("notify-send")
        .arg(title)
        .arg(body)
        .arg("-a")
        .arg("RustSale CRM")
        .arg("-i")
        .arg("rustsale-crm")
        .arg("-e")
        .arg("-u")
        .arg("normal")
        .arg("-t")
        .arg("3500")
        .arg("-h")
        .arg("string:category:im.received")
        .spawn()
        .map_err(|e| format!("notify-send spawn failed: {e}"))?;

    let start = Instant::now();
    let timeout = Duration::from_millis(1500);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(true);
                } else {
                    tracing::warn!(target: "notification", %status, "notify-send exited non-zero");
                    return Ok(false);
                }
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    tracing::warn!(target: "notification", "notify-send timed out after 1.5s; killing process");
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("notify-send timed out after 1.5s".to_string());
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(err) => {
                tracing::warn!(target: "notification", %err, "notify-send try_wait error; killing process");
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("notify-send try_wait error: {err}"));
            }
        }
    }
}

pub fn dispatch_desktop_notification<R: Runtime>(
    app: &AppHandle<R>,
    title: &str,
    body: &str,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        dispatch_notification_strategy(
            || execute_linux_notify_send(title, body),
            || show_plugin_notification(app, title, body),
        )
    }

    #[cfg(not(target_os = "linux"))]
    {
        show_plugin_notification(app, title, body)
    }
}

pub fn dispatch_notification_strategy<FPrimary, FFallback>(
    mut primary_runner: FPrimary,
    mut fallback_runner: FFallback,
) -> Result<(), String>
where
    FPrimary: FnMut() -> Result<bool, String>,
    FFallback: FnMut() -> Result<(), String>,
{
    match primary_runner() {
        Ok(true) => Ok(()),
        Ok(false) | Err(_) => fallback_runner(),
    }
}

#[tauri::command]
pub fn get_notification_settings(
    state: tauri::State<'_, std::sync::Arc<NotificationManager>>,
) -> Result<NotificationSettings, String> {
    Ok(state.get_settings())
}

#[tauri::command]
pub fn update_notification_settings(
    settings: NotificationSettings,
    state: tauri::State<'_, std::sync::Arc<NotificationManager>>,
) -> Result<(), String> {
    state.update_settings(settings);
    Ok(())
}

#[tauri::command]
pub fn set_active_conversation_focus(
    active_conversation_id: Option<i64>,
    is_focused: bool,
    seq: Option<u64>,
    state: tauri::State<'_, std::sync::Arc<NotificationManager>>,
) -> Result<FocusSyncResult, String> {
    Ok(state.set_focus(active_conversation_id, is_focused, seq))
}

#[tauri::command]
pub fn test_desktop_notification<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, std::sync::Arc<NotificationManager>>,
) -> Result<(), String> {
    let settings = state.get_settings();
    let body = if settings.privacy_mode {
        "Bạn có tin nhắn mới (Chế độ riêng tư)".to_string()
    } else {
        "Đây là thông báo thử nghiệm từ RustSale CRM. Tính năng hoạt động hoàn hảo!".to_string()
    };

    dispatch_desktop_notification(&app, "[RustSale] Thông báo thử nghiệm", &body)?;

    let _ = app.emit(
        "desktop-notification-sent",
        serde_json::json!({
            "test": true,
            "sound": settings.sound_enabled
        }),
    );

    Ok(())
}

#[cfg(test)]
#[path = "notification_test.rs"]
mod notification_test;
