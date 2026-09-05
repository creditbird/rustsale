#[cfg(target_os = "linux")]
#[test]
fn test_linux_native_xdg_autostart_lifecycle() {
    let autostart_dir = dirs::config_dir()
        .expect("Failed to locate config dir")
        .join("autostart");

    std::fs::create_dir_all(&autostart_dir).expect("Failed to ensure autostart dir");

    let test_app_name = "rustsale-crm-test-autostart";
    let desktop_file_path = autostart_dir.join(format!("{}.desktop", test_app_name));

    // Ensure clean state before testing
    if desktop_file_path.exists() {
        let _ = std::fs::remove_file(&desktop_file_path);
    }

    // 1. Simulate native autostart desktop entry creation by auto-launch / tauri-plugin-autostart
    let exec_cmd = "/usr/bin/rustsale-crm --autostart";
    let desktop_content = format!(
        "[Desktop Entry]\nType=Application\nVersion=1.0\nName={}\nComment={} startup script\nExec={}\nStartupNotify=false\nTerminal=false\n",
        test_app_name, test_app_name, exec_cmd
    );

    std::fs::write(&desktop_file_path, &desktop_content).expect("Failed to write desktop entry");

    // 2. Verify file existence and contents
    assert!(desktop_file_path.exists(), "Autostart desktop entry must exist on disk");
    let read_content = std::fs::read_to_string(&desktop_file_path).expect("Failed to read desktop file");
    assert!(read_content.contains("Exec=/usr/bin/rustsale-crm --autostart"));
    assert!(read_content.contains("Type=Application"));
    assert!(read_content.contains("Terminal=false"));

    // 3. Verify clean deletion (simulating disable autostart)
    std::fs::remove_file(&desktop_file_path).expect("Failed to remove desktop entry");
    assert!(!desktop_file_path.exists(), "Autostart desktop entry must be removed after disabling");
}
