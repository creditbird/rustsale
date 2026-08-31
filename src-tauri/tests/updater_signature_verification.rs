use minisign_verify::{PublicKey, Signature};
use std::fs;
use std::path::Path;

#[test]
fn test_updater_signature_against_public_key() {
    // 1. Load public key from tauri.conf.json
    let tauri_conf_str = fs::read_to_string("tauri.conf.json")
        .or_else(|_| fs::read_to_string("src-tauri/tauri.conf.json"))
        .expect("Failed to read tauri.conf.json");
    let tauri_conf: serde_json::Value =
        serde_json::from_str(&tauri_conf_str).expect("Failed to parse tauri.conf.json");

    let raw_pubkey_str = tauri_conf["plugins"]["updater"]["pubkey"]
        .as_str()
        .expect("Missing plugins.updater.pubkey");

    // Check if raw_pubkey_str is base64 encoded text (e.g. "untrusted comment: ...")
    let pubkey_text = if let Ok(decoded) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, raw_pubkey_str) {
        String::from_utf8(decoded).unwrap_or_else(|_| raw_pubkey_str.to_string())
    } else {
        raw_pubkey_str.to_string()
    };

    println!("Pubkey text: {:?}", pubkey_text);

    // Parse public key either from full string or key line
    let public_key = if let Ok(pk) = PublicKey::decode(&pubkey_text) {
        pk
    } else if let Ok(pk) = PublicKey::from_base64(&pubkey_text) {
        pk
    } else {
        // Find line starting with RW
        let key_line = pubkey_text.lines().find(|l| l.starts_with("RW")).expect("No RW line found in pubkey");
        PublicKey::from_base64(key_line).expect("Failed to parse base64 key line")
    };

    // 2. Load latest.json and signature
    let latest_json_str = fs::read_to_string("../latest.json")
        .or_else(|_| fs::read_to_string("latest.json"))
        .expect("Failed to read latest.json");
    let latest_json: serde_json::Value =
        serde_json::from_str(&latest_json_str).expect("Failed to parse latest.json");

    let linux_platform = &latest_json["platforms"]["linux-x86_64"];
    let raw_sig_str = linux_platform["signature"]
        .as_str()
        .expect("Missing signature in linux-x86_64 platform");

    // Decode signature if base64 encoded string
    let sig_text = if let Ok(decoded) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, raw_sig_str) {
        String::from_utf8(decoded).unwrap_or_else(|_| raw_sig_str.to_string())
    } else {
        raw_sig_str.to_string()
    };

    println!("Signature text:\n{}", sig_text);

    let signature = Signature::decode(&sig_text).expect("Failed to decode minisign signature");

    // 3. Locate the built .deb bundle
    let deb_path = if Path::new("target/release/bundle/deb/RustSale CRM_0.1.14_amd64.deb").exists() {
        "target/release/bundle/deb/RustSale CRM_0.1.14_amd64.deb"
    } else if Path::new("src-tauri/target/release/bundle/deb/RustSale CRM_0.1.14_amd64.deb").exists() {
        "src-tauri/target/release/bundle/deb/RustSale CRM_0.1.14_amd64.deb"
    } else {
        "../src-tauri/target/release/bundle/deb/RustSale CRM_0.1.14_amd64.deb"
    };

    let deb_bytes = fs::read(deb_path).expect("Failed to read .deb bundle");

    // 4. Verify signature cryptographically
    let verify_result = public_key.verify(&deb_bytes, &signature, false);
    println!("Verify result (allow_legacy=false): {:?}", verify_result);
    assert!(
        verify_result.is_ok(),
        "Minisign cryptographic verification failed: {:?}",
        verify_result.err()
    );

    println!(
        "✅ Minisign cryptographic verification SUCCEEDED for {} ({} bytes)",
        deb_path,
        deb_bytes.len()
    );
}
