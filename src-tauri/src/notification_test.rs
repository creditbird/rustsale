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
    assert!(manager.set_focus(Some(42), true, Some(1)).accepted);

    // Suppressed because user is focused on conversation 42
    assert!(manager.should_suppress(42, false));

    // Not suppressed for conversation 99
    assert!(!manager.should_suppress(99, false));

    // When window loses focus, conversation 42 is no longer suppressed
    assert!(manager.set_focus(Some(42), false, Some(2)).accepted);
    assert!(!manager.should_suppress(42, false));
}

#[test]
fn test_focus_sequence_monotonic_ordering() {
    let manager = NotificationManager::default();

    // Call 1 at seq 10 sets focus to conv 10
    let res1 = manager.set_focus(Some(10), true, Some(10));
    assert!(res1.accepted);
    assert_eq!(res1.backend_seq, 10);
    assert_eq!(manager.get_focus_state(), (Some(10), true, 10));

    // Call 2 at seq 20 sets focus to conv 20
    let res2 = manager.set_focus(Some(20), true, Some(20));
    assert!(res2.accepted);
    assert_eq!(res2.backend_seq, 20);
    assert_eq!(manager.get_focus_state(), (Some(20), true, 20));

    // Out-of-order delayed Call 3 at seq 15 arrives later -> REJECTED, returns backend_seq = 20
    let res3 = manager.set_focus(Some(15), true, Some(15));
    assert!(!res3.accepted);
    assert_eq!(res3.backend_seq, 20);
    // Backend focus state remains at conv 20
    assert_eq!(manager.get_focus_state(), (Some(20), true, 20));

    // Duplicate Call at seq 20 -> REJECTED
    let res_dup = manager.set_focus(Some(999), false, Some(20));
    assert!(!res_dup.accepted);
    assert_eq!(res_dup.backend_seq, 20);
    assert_eq!(manager.get_focus_state(), (Some(20), true, 20));

    // Newer Call 4 at seq 25 -> ACCEPTED
    let res4 = manager.set_focus(Some(25), false, Some(25));
    assert!(res4.accepted);
    assert_eq!(res4.backend_seq, 25);
    assert_eq!(manager.get_focus_state(), (Some(25), false, 25));
}

#[test]
fn test_concurrent_atomic_focus_updates() {
    use std::sync::Arc;
    use std::thread;

    let manager = Arc::new(NotificationManager::default());
    let mut handles = Vec::new();

    // Spawn 10 concurrent threads with different sequence numbers and conversations
    for i in 1..=10 {
        let mgr = Arc::clone(&manager);
        handles.push(thread::spawn(move || {
            let seq = (i * 10) as u64;
            mgr.set_focus(Some(i as i64), true, Some(seq));
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Because highest sequence submitted was 100 (i=10), final state must have seq 100 and conv 10
    let (active_conv, is_focused, final_seq) = manager.get_focus_state();
    assert_eq!(final_seq, 100);
    assert_eq!(active_conv, Some(10));
    assert!(is_focused);
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
fn test_rate_limiting_debounces_rapid_burst_and_prunes() {
    let manager = NotificationManager::default();

    // First message should be allowed
    assert!(manager.check_and_record_rate_limit(100));

    // Immediate second message should be rate-limited
    assert!(!manager.check_and_record_rate_limit(100));

    // Different conversation should be allowed
    assert!(manager.check_and_record_rate_limit(200));

    // Populate 70 expired entries (> 3 seconds old)
    let past_time = Instant::now()
        .checked_sub(Duration::from_secs(10))
        .unwrap();
    {
        let mut map = manager.last_notified.lock().unwrap();
        for i in 1..=70 {
            map.insert(5000 + i, past_time);
        }
        assert!(map.len() >= 72);
    }

    // Trigger pruning via next rate-limit check (size > 64 threshold)
    assert!(manager.check_and_record_rate_limit(9999));

    // Expired entries must be purged, leaving only active recent entries (100, 200, 9999)
    let final_len = manager.last_notified.lock().unwrap().len();
    assert_eq!(final_len, 3);
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

    let plugin_res = show_plugin_notification(&app.handle(), "Fallback Title", "Fallback Body");
    assert!(
        plugin_res.is_ok(),
        "show_plugin_notification should return Ok: {:?}",
        plugin_res
    );
}
