use minisign_verify::{PublicKey, Signature};
use std::fs;
use std::path::PathBuf;

fn find_repo_root() -> PathBuf {
    let mut current = std::env::current_dir().expect("Failed to get current dir");
    loop {
        if current.join("src-tauri").exists() && current.join("latest.json").exists() {
            return current;
        }
        if !current.pop() {
            break;
        }
    }
    panic!("Repository root containing src-tauri and latest.json not found from current working directory");
}

#[test]
fn test_updater_signature_against_public_key() {
    let repo_root = find_repo_root();

    // 1. Load public key from tauri.conf.json
    let tauri_conf_path = repo_root.join("src-tauri/tauri.conf.json");
    let tauri_conf_str = fs::read_to_string(&tauri_conf_path)
        .unwrap_or_else(|_| panic!("Failed to read {:?}", tauri_conf_path));
    let tauri_conf: serde_json::Value =
        serde_json::from_str(&tauri_conf_str).expect("Failed to parse tauri.conf.json");

    let raw_pubkey_str = tauri_conf["plugins"]["updater"]["pubkey"]
        .as_str()
        .expect("Missing plugins.updater.pubkey");

    // Decode base64 public key if needed
    let pubkey_text = if let Ok(decoded) =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, raw_pubkey_str)
    {
        String::from_utf8(decoded).unwrap_or_else(|_| raw_pubkey_str.to_string())
    } else {
        raw_pubkey_str.to_string()
    };

    let public_key = if let Ok(pk) = PublicKey::decode(&pubkey_text) {
        pk
    } else if let Ok(pk) = PublicKey::from_base64(&pubkey_text) {
        pk
    } else {
        let key_line = pubkey_text
            .lines()
            .find(|l| l.starts_with("RW"))
            .expect("No RW line found in pubkey");
        PublicKey::from_base64(key_line).expect("Failed to parse base64 key line")
    };

    // 2. Deterministic cryptographic verification of committed test fixture
    let fixture_path = repo_root.join("src-tauri/tests/fixtures/updater_fixture.txt");
    let sig_path = repo_root.join("src-tauri/tests/fixtures/updater_fixture.txt.sig");

    let fixture_bytes = fs::read(&fixture_path)
        .unwrap_or_else(|_| panic!("Failed to read fixture bytes at {:?}", fixture_path));
    let raw_sig_str = fs::read_to_string(&sig_path)
        .unwrap_or_else(|_| panic!("Failed to read fixture signature at {:?}", sig_path));
    let sig_text = if let Ok(decoded) = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        raw_sig_str.trim(),
    ) {
        String::from_utf8(decoded).unwrap_or_else(|_| raw_sig_str.to_string())
    } else {
        raw_sig_str.to_string()
    };

    let fixture_signature =
        Signature::decode(&sig_text).expect("Failed to decode fixture signature");
    let fixture_verify_result = public_key.verify(&fixture_bytes, &fixture_signature, false);
    assert!(
        fixture_verify_result.is_ok(),
        "Deterministic test fixture signature verification failed: {:?}",
        fixture_verify_result.err()
    );
    println!("✅ Deterministic updater fixture signature verified successfully!");

    // 3. If exact release bundle for current version exists, verify it against latest.json (fail-closed)
    let version = tauri_conf["version"].as_str().unwrap_or("0.1.14");
    let expected_deb_name = format!("RustSale CRM_{}_amd64.deb", version);
    let deb_path = repo_root
        .join("src-tauri/target/release/bundle/deb")
        .join(&expected_deb_name);

    if deb_path.exists() {
        let latest_json_path = repo_root.join("latest.json");
        let latest_json_str = fs::read_to_string(&latest_json_path)
            .unwrap_or_else(|_| panic!("Failed to read {:?}", latest_json_path));
        let latest_json: serde_json::Value =
            serde_json::from_str(&latest_json_str).expect("Failed to parse latest.json");

        let linux_platform = latest_json["platforms"]
            .get("linux-x86_64")
            .expect("Fail-closed: latest.json missing platforms.linux-x86_64 entry while release deb bundle exists");
        let raw_sig_str = linux_platform["signature"]
            .as_str()
            .expect("Fail-closed: latest.json missing signature for linux-x86_64");

        let sig_text = if let Ok(decoded) =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, raw_sig_str)
        {
            String::from_utf8(decoded).unwrap_or_else(|_| raw_sig_str.to_string())
        } else {
            raw_sig_str.to_string()
        };

        let signature = Signature::decode(&sig_text).expect("Failed to decode minisign signature");
        let deb_bytes = fs::read(&deb_path).expect("Failed to read .deb bundle");

        let verify_result = public_key.verify(&deb_bytes, &signature, false);
        assert!(
            verify_result.is_ok(),
            "Release .deb signature verification failed: {:?}",
            verify_result.err()
        );
        println!(
            "✅ Minisign release bundle verification SUCCEEDED for {:?} ({} bytes)",
            deb_path,
            deb_bytes.len()
        );
    } else {
        println!(
            "ℹ️ Release .deb bundle not present in clean checkout - deterministic fixture verified"
        );
    }
}
