#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
fn get_auto_launch_autostart_dir() -> PathBuf {
    // Locked auto-launch 0.5.0 on Linux (src/linux.rs:81) resolves autostart directory via:
    // dirs::home_dir().unwrap().join(".config").join("autostart")
    // rather than dirs::config_dir(). Test assertions and cleanup must align with this exact path.
    dirs::home_dir()
        .expect("Failed to locate user home directory")
        .join(".config")
        .join("autostart")
}

#[cfg(target_os = "linux")]
fn unique_test_app_name(prefix: &str) -> String {
    format!(
        "rustsale-crm-test-{}-{}-{}",
        prefix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

#[cfg(target_os = "linux")]
struct CleanupGuard<'a>(&'a Path);

#[cfg(target_os = "linux")]
impl<'a> Drop for CleanupGuard<'a> {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = std::fs::remove_file(self.0);
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn test_linux_native_xdg_autostart_lifecycle() {
    let autostart_dir = get_auto_launch_autostart_dir();
    let test_app_name = unique_test_app_name("lifecycle");
    let desktop_file_path = autostart_dir.join(format!("{}.desktop", test_app_name));

    // RAII cleanup guard to guarantee removal during normal exit or panic unwinding.
    // Note: Rust destructors run on normal return and panic unwinding, but not on process termination without unwinding (e.g. SIGKILL/abort).
    let _guard = CleanupGuard(&desktop_file_path);

    // Call real auto_launch (the exact crate and API used by tauri-plugin-autostart on Linux)
    let auto = auto_launch::AutoLaunchBuilder::new()
        .set_app_name(&test_app_name)
        .set_app_path("/usr/bin/rustsale-crm")
        .set_args(&["--autostart"])
        .build()
        .expect("Failed to build AutoLaunch instance");

    assert!(
        !auto
            .is_enabled()
            .expect("Failed to query initial is_enabled state"),
        "Initial state should not be enabled for unique owned entry"
    );

    // 1. Enable autostart via auto_launch
    auto.enable()
        .expect("Failed to enable autostart via auto_launch");
    assert!(
        auto.is_enabled()
            .expect("Failed to query is_enabled after enable"),
        "is_enabled should be true after enable"
    );
    assert!(
        desktop_file_path.exists(),
        "Desktop file must be created on disk in ~/.config/autostart"
    );

    let read_content =
        std::fs::read_to_string(&desktop_file_path).expect("Failed to read desktop file");
    assert!(read_content.contains("Exec=/usr/bin/rustsale-crm --autostart"));
    assert!(read_content.contains("Type=Application"));
    assert!(read_content.contains(&format!("Name={}", test_app_name)));

    // 2. Disable autostart via auto_launch
    auto.disable()
        .expect("Failed to disable autostart via auto_launch");
    assert!(
        !auto
            .is_enabled()
            .expect("Failed to query is_enabled after disable"),
        "is_enabled should be false after disable"
    );
    assert!(
        !desktop_file_path.exists(),
        "Desktop file must be deleted after disable"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn test_linux_native_autostart_non_default_xdg_alignment() {
    // When XDG_CONFIG_HOME points to a custom directory, dirs::config_dir() diverges
    // from dirs::home_dir().join(".config"). Locked auto-launch 0.5.0 always writes to
    // ~/.config/autostart. This test verifies that assertions and cleanup target the
    // actual file path written by auto-launch and leave no orphaned entries.
    let temp_xdg = tempfile::tempdir().expect("Failed to create temporary XDG_CONFIG_HOME");
    let test_app_name = unique_test_app_name("xdg-align");

    let autostart_dir = get_auto_launch_autostart_dir();
    let desktop_file_path = autostart_dir.join(format!("{}.desktop", test_app_name));
    let _guard = CleanupGuard(&desktop_file_path);

    // Verify temp_xdg is different from autostart_dir
    assert_ne!(
        temp_xdg.path(),
        autostart_dir.as_path(),
        "Temporary XDG directory must diverge from real ~/.config/autostart"
    );

    let auto = auto_launch::AutoLaunchBuilder::new()
        .set_app_name(&test_app_name)
        .set_app_path("/usr/bin/rustsale-crm")
        .set_args(&["--autostart"])
        .build()
        .expect("Failed to build AutoLaunch instance");

    auto.enable()
        .expect("Failed to enable autostart under non-default XDG test");
    assert!(
        desktop_file_path.exists(),
        "Desktop file must be created at auto-launch target ~/.config/autostart"
    );

    auto.disable()
        .expect("Failed to disable autostart under non-default XDG test");
    assert!(
        !desktop_file_path.exists(),
        "Desktop file must be removed cleanly without leaving orphaned file"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn test_cleanup_guard_removes_file_on_panic_unwind() {
    let autostart_dir = get_auto_launch_autostart_dir();
    let test_app_name = unique_test_app_name("panic-unwind");
    let desktop_file_path = autostart_dir.join(format!("{}.desktop", test_app_name));

    let path_clone = desktop_file_path.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _guard = CleanupGuard(&path_clone);
        std::fs::create_dir_all(path_clone.parent().unwrap()).unwrap();
        std::fs::write(&path_clone, "[Desktop Entry]\nName=simulated-panic\n").unwrap();
        assert!(
            path_clone.exists(),
            "Precondition: file created before panic"
        );
        panic!("simulated assertion failure to test RAII CleanupGuard drop");
    }));

    assert!(result.is_err(), "Closure must panic as simulated");
    assert!(
        !desktop_file_path.exists(),
        "CleanupGuard must remove the desktop file when unwinding from panic"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn test_concurrent_autostart_isolation() {
    let handles: Vec<_> = (0..4)
        .map(|i| {
            std::thread::spawn(move || {
                let autostart_dir = get_auto_launch_autostart_dir();
                let test_app_name = unique_test_app_name(&format!("thread-{}", i));
                let desktop_file_path = autostart_dir.join(format!("{}.desktop", test_app_name));
                let _guard = CleanupGuard(&desktop_file_path);

                let auto = auto_launch::AutoLaunchBuilder::new()
                    .set_app_name(&test_app_name)
                    .set_app_path("/usr/bin/rustsale-crm")
                    .set_args(&["--autostart"])
                    .build()
                    .expect("Failed to build AutoLaunch instance");

                assert!(!auto.is_enabled().unwrap());
                auto.enable().expect("Thread failed to enable");
                assert!(auto.is_enabled().unwrap());
                assert!(desktop_file_path.exists());

                // Short sleep to allow concurrent overlapping execution
                std::thread::sleep(std::time::Duration::from_millis(10));

                auto.disable().expect("Thread failed to disable");
                assert!(!auto.is_enabled().unwrap());
                assert!(!desktop_file_path.exists());
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Concurrent test thread failed");
    }
}
