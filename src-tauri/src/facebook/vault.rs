//! Local Facebook Page token vault: AES-256-GCM authenticated encryption with
//! a master key held in the OS Keyring. Tokens never exist on disk in
//! plaintext; the keyring stores only the base64-encoded 32-byte master key.
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::prelude::*;
use keyring::Entry;
use rusqlite::{params, Connection, OptionalExtension};
use anyhow::{anyhow, Result};

const KEYRING_SERVICE: &str = "rustsale";
const KEYRING_USER: &str = "facebook_vault_key";

fn keyring_entry() -> Result<Entry> {
    Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| anyhow!("Failed to access OS keyring: {}", e))
}

/// Loads (or lazily creates) the 32-byte vault master key from the OS keyring.
pub fn get_master_key() -> Result<[u8; 32]> {
    let entry = keyring_entry()?;
    match entry.get_password() {
        Ok(secret) => {
            let mut key = [0u8; 32];
            let decoded = BASE64_STANDARD
                .decode(secret.trim())
                .map_err(|e| anyhow!("Invalid base64 in keyring secret: {}", e))?;
            if decoded.len() != 32 {
                return Err(anyhow!("Keyring master key length is not 32 bytes"));
            }
            key.copy_from_slice(&decoded);
            Ok(key)
        }
        Err(keyring::Error::NoEntry) => {
            let mut key = [0u8; 32];
            rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut key);
            let encoded = BASE64_STANDARD.encode(key);
            entry
                .set_password(&encoded)
                .map_err(|e| anyhow!("Failed to persist master key to OS keyring: {}", e))?;
            Ok(key)
        }
        Err(e) => Err(anyhow!("Keyring access failed: {}", e)),
    }
}

/// AES-256-GCM round trip over a caller-provided key (unit-testable core).
pub fn encrypt_with_key(key: &[u8; 32], plain_token: &str) -> Result<String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| anyhow!("AES-256-GCM init error: {}", e))?;
    let mut nonce_bytes = [0u8; 12];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plain_token.as_bytes())
        .map_err(|e| anyhow!("Token encryption error: {}", e))?;
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    Ok(BASE64_STANDARD.encode(combined))
}

pub fn decrypt_with_key(key: &[u8; 32], ciphertext_b64: &str) -> Result<String> {
    let raw = BASE64_STANDARD
        .decode(ciphertext_b64.trim())
        .map_err(|e| anyhow!("Base64 decode failed: {}", e))?;
    if raw.len() < 12 {
        return Err(anyhow!("Ciphertext payload too short for nonce"));
    }
    let (nonce_bytes, encrypted_payload) = raw.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| anyhow!("AES-256-GCM cipher init error: {}", e))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let decrypted_bytes = cipher
        .decrypt(nonce, encrypted_payload)
        .map_err(|e| anyhow!("AES-256-GCM decryption failed: {}", e))?;
    String::from_utf8(decrypted_bytes)
        .map_err(|e| anyhow!("Decrypted token is not valid UTF-8: {}", e))
}

/// Encrypts using the keyring-backed master key.
pub fn encrypt_token(plain_token: &str) -> Result<String> {
    encrypt_with_key(&get_master_key()?, plain_token)
}

pub fn decrypt_token(ciphertext_b64: &str) -> Result<String> {
    decrypt_with_key(&get_master_key()?, ciphertext_b64)
}

/// Persists an encrypted Page access token keyed by page_id (upsert).
pub fn encrypt_and_upsert_page_token_with_key(
    conn: &Connection,
    key: &[u8; 32],
    channel_account_id: i64,
    page_id: &str,
    plain_token: &str,
    granted_scopes: &[String],
) -> Result<()> {
    let encrypted_ciphertext = encrypt_with_key(key, plain_token)?;
    let scopes_json =
        serde_json::to_string(granted_scopes).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "INSERT INTO facebook_page_tokens (channel_account_id, page_id, access_token_encrypted, token_status, granted_scopes_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'valid', ?4, datetime('now'), datetime('now'))
         ON CONFLICT(page_id) DO UPDATE SET
             channel_account_id = EXCLUDED.channel_account_id,
             access_token_encrypted = EXCLUDED.access_token_encrypted,
             token_status = 'valid',
             granted_scopes_json = EXCLUDED.granted_scopes_json,
             updated_at = datetime('now')",
        params![channel_account_id, page_id, encrypted_ciphertext, scopes_json],
    )
    .map_err(|e| anyhow!("Failed to upsert page token in vault: {}", e))?;
    Ok(())
}

pub fn encrypt_and_upsert_page_token(
    conn: &Connection,
    channel_account_id: i64,
    page_id: &str,
    plain_token: &str,
    granted_scopes: &[String],
) -> Result<()> {
    encrypt_and_upsert_page_token_with_key(
        conn,
        &get_master_key()?,
        channel_account_id,
        page_id,
        plain_token,
        granted_scopes,
    )
}

/// Retrieves and decrypts the currently-valid Page token.
pub fn get_valid_page_token_with_key(
    conn: &Connection,
    key: &[u8; 32],
    channel_account_id: i64,
    page_id: &str,
) -> Result<String> {
    let encrypted_token: Option<String> = conn
        .query_row(
            "SELECT access_token_encrypted FROM facebook_page_tokens
             WHERE channel_account_id = ?1 AND page_id = ?2 AND token_status = 'valid'",
            params![channel_account_id, page_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| {
            anyhow!(
                "DB query failed for channel_account_id {} page_id {}: {}",
                channel_account_id,
                page_id,
                e
            )
        })?;

    let ciphertext = match encrypted_token {
        Some(t) if !t.is_empty() => t,
        _ => {
            return Err(anyhow!(
                "No valid active token found for channel_account_id: {} page_id: {}",
                channel_account_id,
                page_id
            ))
        }
    };

    decrypt_with_key(key, &ciphertext)
}

pub fn get_valid_page_token(
    conn: &Connection,
    channel_account_id: i64,
    page_id: &str,
) -> Result<String> {
    get_valid_page_token_with_key(conn, &get_master_key()?, channel_account_id, page_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [7u8; 32];

    #[test]
    fn encrypt_decrypt_roundtrip_preserves_token() {
        let token = "EAAx9f8example_page_token_🙂";
        let sealed = encrypt_with_key(&KEY, token).unwrap();
        assert_ne!(sealed, token);
        assert!(!sealed.contains("example"));
        assert_eq!(decrypt_with_key(&KEY, &sealed).unwrap(), token);
    }

    #[test]
    fn unique_nonces_produce_distinct_ciphertexts() {
        let a = encrypt_with_key(&KEY, "same-token").unwrap();
        let b = encrypt_with_key(&KEY, "same-token").unwrap();
        assert_ne!(a, b, "random nonce must randomize ciphertext");
    }

    #[test]
    fn tampered_ciphertext_fails_authentication() {
        let sealed = encrypt_with_key(&KEY, "secret-token").unwrap();
        let mut raw = BASE64_STANDARD.decode(sealed).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0xFF;
        let tampered = BASE64_STANDARD.encode(raw);
        assert!(decrypt_with_key(&KEY, &tampered).is_err());
    }

    #[test]
    fn wrong_key_fails_authentication() {
        let sealed = encrypt_with_key(&KEY, "secret-token").unwrap();
        let other_key = [9u8; 32];
        assert!(decrypt_with_key(&other_key, &sealed).is_err());
    }

    #[test]
    fn short_payload_is_rejected() {
        let short = BASE64_STANDARD.encode([0u8; 4]);
        assert!(decrypt_with_key(&KEY, &short).is_err());
    }

    #[test]
    fn vault_upsert_then_get_roundtrips_and_rotates() {
        // Minimal standalone table (no FK) keeps this unit isolated from the
        // full conversation schema; SQL mirrors the production upsert path.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE facebook_page_tokens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_account_id INTEGER NOT NULL,
                page_id TEXT NOT NULL UNIQUE,
                access_token_encrypted TEXT NOT NULL,
                token_status TEXT NOT NULL DEFAULT 'valid',
                granted_scopes_json TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
        .unwrap();

        encrypt_and_upsert_page_token_with_key(&conn, &KEY, 10, "page-1", "token-v1", &["pages_messaging".to_string()]).unwrap();
        assert_eq!(
            get_valid_page_token_with_key(&conn, &KEY, 10, "page-1").unwrap(),
            "token-v1"
        );

        // Rotation: upserting again replaces ciphertext + keeps status valid.
        encrypt_and_upsert_page_token_with_key(&conn, &KEY, 10, "page-1", "token-v2", &["pages_messaging".to_string(), "pages_manage_posts".to_string()]).unwrap();
        assert_eq!(
            get_valid_page_token_with_key(&conn, &KEY, 10, "page-1").unwrap(),
            "token-v2"
        );

        // Missing page yields a clear error.
        assert!(get_valid_page_token_with_key(&conn, &KEY, 10, "page-missing").is_err());

        // Invalidated tokens are not returned.
        conn.execute(
            "UPDATE facebook_page_tokens SET token_status='revoked' WHERE page_id='page-1'",
            [],
        )
        .unwrap();
        assert!(get_valid_page_token_with_key(&conn, &KEY, 10, "page-1").is_err());
    }
}
