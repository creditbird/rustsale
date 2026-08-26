use super::*;

#[test]
fn test_tray_config_default() {
    let config = TrayConfig::default();
    assert!(config.close_to_tray);
    assert!(!config.dnd_enabled);
}

#[test]
fn test_tray_state_toggle_dnd() {
    let state = TrayState::default();
    assert!(!state.is_dnd_enabled());

    state.set_dnd_enabled(true);
    assert!(state.is_dnd_enabled());

    state.set_dnd_enabled(false);
    assert!(!state.is_dnd_enabled());
}

#[test]
fn test_tray_state_close_to_tray() {
    let state = TrayState::default();
    assert!(state.is_close_to_tray_enabled());

    state.set_close_to_tray(false);
    assert!(!state.is_close_to_tray_enabled());

    state.set_close_to_tray(true);
    assert!(state.is_close_to_tray_enabled());
}
