#[cfg(target_os = "linux")]
#[test]
fn test_linux_native_xdg_autostart_lifecycle() {
    let test_app_name = "rustsale-crm-test-autostart-integration";
    let autostart_dir = dirs::config_dir()
        .expect("Failed to locate config dir")
        .join("autostart");
    let desktop_file_path = autostart_dir.join(format!("{}.desktop", test_app_name));

    // RAII cleanup guard to guarantee removal even if an assertion panics
    struct CleanupGuard<'a>(&'a std::path::Path);
    impl<'a> Drop for CleanupGuard<'a> {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.0);
        }
    }
    let _guard = CleanupGuard(&desktop_file_path);

    // Ensure clean state before test
    if desktop_file_path.exists() {
        let _ = std::fs::remove_file(&desktop_file_path);
    }

    // Call real auto_launch (the exact crate and API used by tauri-plugin-autostart on Linux)
    let auto = auto_launch::AutoLaunchBuilder::new()
        .set_app_name(test_app_name)
        .set_app_path("/usr/bin/rustsale-crm")
        .set_args(&["--autostart"])
        .build()
        .expect("Failed to build AutoLaunch instance");

    assert!(!auto.is_enabled().expect("Failed to query initial is_enabled state"));

    // 1. Enable autostart via auto_launch
    auto.enable().expect("Failed to enable autostart via auto_launch");
    assert!(auto.is_enabled().expect("Failed to query is_enabled after enable"));
    assert!(desktop_file_path.exists(), "Desktop file must be created on disk in ~/.config/autostart");

    let read_content = std::fs::read_to_string(&desktop_file_path).expect("Failed to read desktop file");
    assert!(read_content.contains("Exec=/usr/bin/rustsale-crm --autostart"));
    assert!(read_content.contains("Type=Application"));

    // 2. Disable autostart via auto_launch
    auto.disable().expect("Failed to disable autostart via auto_launch");
    assert!(!auto.is_enabled().expect("Failed to query is_enabled after disable"));
    assert!(!desktop_file_path.exists(), "Desktop file must be deleted after disable");
}
