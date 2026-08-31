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

pub struct NotificationManager {
    settings: RwLock<NotificationSettings>,
    focus_state: RwLock<(Option<i64>, bool)>,
    last_notified: Mutex<HashMap<i64, Instant>>,
    tray_state: RwLock<Option<crate::tray::TrayState>>,
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self {
            settings: RwLock::new(NotificationSettings::default()),
            focus_state: RwLock::new((None, false)),
            last_notified: Mutex::new(HashMap::new()),
            tray_state: RwLock::new(None),
        }
    }
}

impl NotificationManager {
    pub fn new(settings: NotificationSettings) -> Self {
        Self {
            settings: RwLock::new(settings),
            focus_state: RwLock::new((None, false)),
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

    pub fn set_focus(&self, active_conversation_id: Option<i64>, is_focused: bool) {
        *self.focus_state.write().unwrap() = (active_conversation_id, is_focused);
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

        if is_group && settings.mute_groups {
            return true;
        }

        let (focused_conv_id, is_window_focused) = *self.focus_state.read().unwrap();
        if is_window_focused && focused_conv_id == Some(conversation_id) {
            return true;
        }

        false
    }

    pub fn check_and_record_rate_limit(&self, conversation_id: i64) -> bool {
        let now = Instant::now();
        let mut map = self.last_notified.lock().unwrap();

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

fn show_plugin_notification<R: Runtime>(
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

pub fn dispatch_desktop_notification<R: Runtime>(
    app: &AppHandle<R>,
    title: &str,
    body: &str,
) -> Result<(), String> {
    let title_owned = title.to_string();
    let body_owned = body.to_string();
    let app_handle = app.clone();

    #[cfg(target_os = "linux")]
    {
        let spawn_res = std::thread::Builder::new()
            .name("notification-dispatch".to_string())
            .spawn(move || {
                let notify_send_res = std::process::Command::new("notify-send")
                    .arg(&title_owned)
                    .arg(&body_owned)
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
                    .status();

                match notify_send_res {
                    Ok(status) if status.success() => {}
                    Ok(status) => {
                        tracing::warn!(target: "notification", %status, "notify-send exited with non-zero status; falling back to tauri-plugin-notification");
                        if let Err(err) = show_plugin_notification(&app_handle, &title_owned, &body_owned) {
                            tracing::warn!(target: "notification", %err, "Fallback tauri-plugin-notification failed");
                        }
                    }
                    Err(spawn_err) => {
                        tracing::debug!(target: "notification", %spawn_err, "notify-send spawn failed; falling back to tauri-plugin-notification");
                        if let Err(err) = show_plugin_notification(&app_handle, &title_owned, &body_owned) {
                            tracing::warn!(target: "notification", %err, "Fallback tauri-plugin-notification failed");
                        }
                    }
                }
            });

        if let Err(err) = spawn_res {
            tracing::warn!(target: "notification", %err, "Failed to spawn notification thread; falling back to plugin");
            return show_plugin_notification(app, title, body);
        }

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    {
        show_plugin_notification(&app_handle, &title_owned, &body_owned)
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
    state: tauri::State<'_, std::sync::Arc<NotificationManager>>,
) -> Result<(), String> {
    state.set_focus(active_conversation_id, is_focused);
    Ok(())
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
