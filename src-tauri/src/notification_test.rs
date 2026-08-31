use super::*;
use tauri::Manager;

#[test]
fn test_notification_settings_default() {
    let settings = NotificationSettings::default();
    assert!(settings.enabled);
    assert!(settings.sound_enabled);
    assert!(!settings.privacy_mode);
    assert!(!settings.mute_groups);
}

#[test]
fn test_format_notification_content_normal() {
    let title = format_notification_title(
        crate::conversation::types::Channel::Zalo,
        "Nguyễn Văn A",
        false,
        None,
    );
    assert_eq!(title, "[Zalo] Nguyễn Văn A");

    let body = format_notification_body("Xin chào, tôi cần tư vấn báo giá", "text", false);
    assert_eq!(body, "Xin chào, tôi cần tư vấn báo giá");
}

#[test]
fn test_format_notification_content_privacy_mode() {
    let title = format_notification_title(
        crate::conversation::types::Channel::Zalo,
        "Nguyễn Văn A",
        false,
        None,
    );
    assert_eq!(title, "[Zalo] Nguyễn Văn A");

    let body = format_notification_body("Nội dung nhạy cảm 123", "text", true);
    assert_eq!(body, "Bạn có tin nhắn mới");
}

#[test]
fn test_format_notification_content_group() {
    let title = format_notification_title(
        crate::conversation::types::Channel::Zalo,
        "Nguyễn Văn A",
        true,
        Some("Nhóm Đối Tác"),
    );
    assert_eq!(title, "[Zalo] Nhóm Đối Tác (Nguyễn Văn A)");
}

#[test]
fn test_format_notification_content_media() {
    let body_image = format_notification_body("", "image", false);
    assert_eq!(body_image, "[Hình ảnh]");

    let body_sticker = format_notification_body("", "sticker", false);
    assert_eq!(body_sticker, "[Nhãn dán]");

    let body_file = format_notification_body("", "file", false);
    assert_eq!(body_file, "[Tệp đính kèm]");
}

#[test]
fn test_should_suppress_notification_when_focused() {
    let manager = NotificationManager::default();
    manager.set_focus(Some(42), true);

    // Suppressed because user is focused on conversation 42
    assert!(manager.should_suppress(42, false));

    // Not suppressed for conversation 99
    assert!(!manager.should_suppress(99, false));

    // When window loses focus, conversation 42 is no longer suppressed
    manager.set_focus(Some(42), false);
    assert!(!manager.should_suppress(42, false));
}

#[test]
fn test_should_suppress_notification_when_dnd_or_disabled() {
    let manager = NotificationManager::default();
    let tray_state = crate::tray::TrayState::new(true, false);
    manager.set_tray_state(tray_state.clone());

    // Default: not suppressed
    assert!(!manager.should_suppress(1, false));

    // When Tray DND is enabled: must suppress direct and group chats
    tray_state.set_dnd_enabled(true);
    assert!(manager.should_suppress(1, false));
    assert!(manager.should_suppress(1, true));

    // When Tray DND is toggled off: resumed
    tray_state.set_dnd_enabled(false);
    assert!(!manager.should_suppress(1, false));

    // Disabled setting
    manager.update_settings(NotificationSettings {
        enabled: false,
        sound_enabled: true,
        privacy_mode: false,
        mute_groups: false,
    });
    assert!(manager.should_suppress(1, false));

    // Re-enable, but mute groups
    manager.update_settings(NotificationSettings {
        enabled: true,
        sound_enabled: true,
        privacy_mode: false,
        mute_groups: true,
    });
    assert!(!manager.should_suppress(1, false)); // Direct chat is not suppressed
    assert!(manager.should_suppress(1, true)); // Group chat is suppressed
}

#[test]
fn test_rate_limiting_debounces_rapid_burst() {
    let manager = NotificationManager::default();

    // First message should be allowed
    assert!(manager.check_and_record_rate_limit(100));

    // Immediate second message should be rate-limited
    assert!(!manager.check_and_record_rate_limit(100));

    // Different conversation should be allowed
    assert!(manager.check_and_record_rate_limit(200));
}

#[test]
fn test_dispatch_desktop_notification_and_test_command() {
    let app = tauri::test::mock_builder()
        .plugin(tauri_plugin_notification::init())
        .manage(std::sync::Arc::new(NotificationManager::default()))
        .build(tauri::generate_context!())
        .expect("failed to build mock app");
    let state = app.state::<std::sync::Arc<NotificationManager>>();

    let test_res = test_desktop_notification(app.handle().clone(), state);
    assert!(
        test_res.is_ok(),
        "test_desktop_notification should return Ok: {:?}",
        test_res
    );

    let direct_res = dispatch_desktop_notification(&app.handle(), "Test Title", "Test Body");
    assert!(
        direct_res.is_ok(),
        "dispatch_desktop_notification should return Ok: {:?}",
        direct_res
    );
}
