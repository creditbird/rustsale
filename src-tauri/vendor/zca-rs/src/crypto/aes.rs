use crate::error::ZaloError;
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit};
use aes_gcm::aead::consts::U16;
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::AesGcm;
use base64::prelude::*;
use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use md5::{Digest, Md5};
use std::io::Read;

type Aes256Gcm16 = AesGcm<aes::Aes256, U16>;
type Aes128Gcm16 = AesGcm<aes::Aes128, U16>;

/// Decodes AES-GCM encrypted data using 16-byte IV, 16-byte AAD and dynamic key length.
pub fn aes_gcm_decrypt(
    key: &[u8],
    iv: &[u8],
    aad: &[u8],
    ciphertext_with_tag: &[u8],
) -> Result<Vec<u8>, ZaloError> {
    let payload = Payload {
        msg: ciphertext_with_tag,
        aad,
    };
    match key.len() {
        16 => {
            let cipher =
                Aes128Gcm16::new_from_slice(key).map_err(|e| ZaloError::Crypto(e.to_string()))?;
            let nonce = aes_gcm::Nonce::<U16>::from_slice(iv);
            let decrypted = cipher
                .decrypt(nonce, payload)
                .map_err(|e| ZaloError::Crypto(e.to_string()))?;
            Ok(decrypted)
        }
        32 => {
            let cipher =
                Aes256Gcm16::new_from_slice(key).map_err(|e| ZaloError::Crypto(e.to_string()))?;
            let nonce = aes_gcm::Nonce::<U16>::from_slice(iv);
            let decrypted = cipher
                .decrypt(nonce, payload)
                .map_err(|e| ZaloError::Crypto(e.to_string()))?;
            Ok(decrypted)
        }
        other => Err(ZaloError::Crypto(format!(
            "Unsupported GCM key size: {}",
            other
        ))),
    }
}

/// Encrypts plaintext using AES-CBC with PKCS7 padding.
pub fn aes_cbc_encrypt(key: &[u8], iv: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, ZaloError> {
    match key.len() {
        16 => {
            let encryptor = cbc::Encryptor::<aes::Aes128>::new_from_slices(key, iv)
                .map_err(|e| ZaloError::Crypto(e.to_string()))?;
            let mut buf = vec![0u8; plaintext.len() + 16];
            if let Some(sub) = buf.get_mut(..plaintext.len()) {
                sub.copy_from_slice(plaintext);
            }
            let ct_len = encryptor
                .encrypt_padded_mut::<Pkcs7>(&mut buf, plaintext.len())
                .map_err(|e| ZaloError::Crypto(e.to_string()))?
                .len();
            buf.truncate(ct_len);
            Ok(buf)
        }
        32 => {
            let encryptor = cbc::Encryptor::<aes::Aes256>::new_from_slices(key, iv)
                .map_err(|e| ZaloError::Crypto(e.to_string()))?;
            let mut buf = vec![0u8; plaintext.len() + 16];
            if let Some(sub) = buf.get_mut(..plaintext.len()) {
                sub.copy_from_slice(plaintext);
            }
            let ct_len = encryptor
                .encrypt_padded_mut::<Pkcs7>(&mut buf, plaintext.len())
                .map_err(|e| ZaloError::Crypto(e.to_string()))?
                .len();
            buf.truncate(ct_len);
            Ok(buf)
        }
        other => Err(ZaloError::Crypto(format!(
            "Unsupported CBC key size: {}",
            other
        ))),
    }
}

/// Decrypts ciphertext using AES-CBC with PKCS7 padding.
pub fn aes_cbc_decrypt(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, ZaloError> {
    match key.len() {
        16 => {
            let decryptor = cbc::Decryptor::<aes::Aes128>::new_from_slices(key, iv)
                .map_err(|e| ZaloError::Crypto(e.to_string()))?;
            let mut buf = ciphertext.to_vec();
            let pt = decryptor
                .decrypt_padded_mut::<Pkcs7>(&mut buf)
                .map_err(|e| ZaloError::Crypto(e.to_string()))?;
            Ok(pt.to_vec())
        }
        32 => {
            let decryptor = cbc::Decryptor::<aes::Aes256>::new_from_slices(key, iv)
                .map_err(|e| ZaloError::Crypto(e.to_string()))?;
            let mut buf = ciphertext.to_vec();
            let pt = decryptor
                .decrypt_padded_mut::<Pkcs7>(&mut buf)
                .map_err(|e| ZaloError::Crypto(e.to_string()))?;
            Ok(pt.to_vec())
        }
        other => Err(ZaloError::Crypto(format!(
            "Unsupported CBC key size: {}",
            other
        ))),
    }
}

/// Decompress Zlib payload.
pub fn zlib_decompress(data: &[u8]) -> Result<Vec<u8>, ZaloError> {
    fn read_all(mut decoder: impl Read) -> std::io::Result<Vec<u8>> {
        let mut result = Vec::new();
        decoder.read_to_end(&mut result)?;
        Ok(result)
    }

    if let Ok(result) = read_all(ZlibDecoder::new(data)) {
        return Ok(result);
    }
    if let Ok(result) = read_all(GzDecoder::new(data)) {
        return Ok(result);
    }

    let mut result = Vec::new();
    DeflateDecoder::new(data)
        .read_to_end(&mut result)
        .map_err(|e| ZaloError::Decompression(e.to_string()))?;
    Ok(result)
}

/// Encrypts parameter payload using Base64 key and CBC mode.
pub fn encode_aes(secret_key: &str, data: &[u8]) -> Result<String, ZaloError> {
    let key = BASE64_STANDARD
        .decode(secret_key)
        .map_err(|e| ZaloError::Crypto(e.to_string()))?;
    let iv = [0u8; 16];
    let encrypted = aes_cbc_encrypt(&key, &iv, data)?;
    Ok(BASE64_STANDARD.encode(encrypted))
}

/// Decrypts response payload using Base64 key and CBC mode.
pub fn decode_aes(secret_key: &str, data: &str) -> Result<String, ZaloError> {
    let key = BASE64_STANDARD
        .decode(secret_key)
        .map_err(|e| ZaloError::Crypto(e.to_string()))?;
    let iv = [0u8; 16];

    // URL decode the ciphertext if needed
    let decoded_url = urlencoding::decode(data)
        .map_err(|e: std::string::FromUtf8Error| ZaloError::Crypto(e.to_string()))?;
    let ciphertext = BASE64_STANDARD
        .decode(decoded_url.as_ref())
        .map_err(|e| ZaloError::Crypto(e.to_string()))?;

    let decrypted = aes_cbc_decrypt(&key, &iv, &ciphertext)?;
    String::from_utf8(decrypted).map_err(|e| ZaloError::Crypto(e.to_string()))
}

/// Decrypts response payload using plain string UTF-8 key and CBC mode.
pub fn decrypt_resp(key: &str, data: &str) -> Result<serde_json::Value, ZaloError> {
    let key_bytes = key.as_bytes();
    let iv = [0u8; 16];

    let decoded_url = urlencoding::decode(data)
        .map_err(|e: std::string::FromUtf8Error| ZaloError::Crypto(e.to_string()))?;
    let ciphertext = BASE64_STANDARD
        .decode(decoded_url.as_ref())
        .map_err(|e| ZaloError::Crypto(e.to_string()))?;

    let decrypted = aes_cbc_decrypt(key_bytes, &iv, &ciphertext)?;
    let s = String::from_utf8(decrypted).map_err(|e| ZaloError::Crypto(e.to_string()))?;
    serde_json::from_str(&s).map_err(Into::into)
}

/// Helper structure to process characters (even/odd separation).
fn process_str(s: &str) -> (Vec<char>, Vec<char>) {
    let mut even = Vec::new();
    let mut odd = Vec::new();
    for (i, c) in s.chars().enumerate() {
        if i % 2 == 0 {
            even.push(c);
        } else {
            odd.push(c);
        }
    }
    (even, odd)
}

/// ParamsEncryptor implements Zalo's custom CBC parameter key derivation.
#[derive(Debug)]
pub struct ParamsEncryptor {
    zcid: Option<String>,
    zcid_ext: String,
    enc_ver: String,
    encrypt_key: Option<String>,
}

impl ParamsEncryptor {
    pub fn new(api_type: i32, imei: &str, first_launch_time: u64) -> Result<Self, ZaloError> {
        let mut slf = Self {
            zcid: None,
            zcid_ext: Self::random_string(6, 12),
            enc_ver: "v2".to_string(),
            encrypt_key: None,
        };
        slf.create_zcid(api_type, imei, first_launch_time)?;
        slf.create_encrypt_key(0)?;
        Ok(slf)
    }

    pub fn get_encrypt_key(&self) -> Result<&str, ZaloError> {
        self.encrypt_key
            .as_deref()
            .ok_or_else(|| ZaloError::Crypto("Encrypt key not created yet".to_string()))
    }

    pub fn get_params(&self) -> Option<serde_json::Value> {
        self.zcid.as_ref().map(|zcid| {
            serde_json::json!({
                "zcid": zcid,
                "zcid_ext": self.zcid_ext,
                "enc_ver": self.enc_ver,
            })
        })
    }

    fn create_zcid(
        &mut self,
        api_type: i32,
        imei: &str,
        first_launch_time: u64,
    ) -> Result<(), ZaloError> {
        let msg = format!("{},{},{}", api_type, imei, first_launch_time);
        // Key is "3FC4F0D2AB50057BCE0D90D9187A22B1" (32 bytes)
        let key = b"3FC4F0D2AB50057BCE0D90D9187A22B1";
        let iv = [0u8; 16];
        let encrypted = aes_cbc_encrypt(key, &iv, msg.as_bytes())?;

        // Hex encode and make uppercase
        let mut hex_str = String::new();
        for byte in encrypted {
            hex_str.push_str(&format!("{:02X}", byte));
        }
        self.zcid = Some(hex_str);
        Ok(())
    }

    fn create_encrypt_key(&mut self, attempt: i32) -> Result<(), ZaloError> {
        let zcid = self
            .zcid
            .as_deref()
            .ok_or_else(|| ZaloError::Crypto("zcid is null".to_string()))?;

        let mut hasher = Md5::new();
        hasher.update(self.zcid_ext.as_bytes());
        let n = format!("{:X}", hasher.finalize());

        let (n_even, _) = process_str(&n);
        let (z_even, z_odd) = process_str(zcid);

        if n_even.is_empty() || z_even.is_empty() || z_odd.is_empty() {
            if attempt < 3 {
                return self.create_encrypt_key(attempt + 1);
            }
            return Err(ZaloError::Crypto(
                "Failed to process even/odd strings".to_string(),
            ));
        }

        let n_8: String = n_even.iter().take(8).collect();
        let z_12: String = z_even.iter().take(12).collect();

        let mut z_odd_rev = z_odd.clone();
        z_odd_rev.reverse();
        let zo_12: String = z_odd_rev.iter().take(12).collect();

        self.encrypt_key = Some(format!("{}{}{}", n_8, z_12, zo_12));
        Ok(())
    }

    fn random_string(min: usize, max: usize) -> String {
        // We generate a pseudo-random hex string using UUID v4.
        let u = uuid::Uuid::new_v4();
        let s = format!("{:x}", u).replace('-', "");
        let len = if max > min {
            let diff = max - min;
            let offset = (u.as_u128() % (diff as u128 + 1)) as usize;
            min + offset
        } else {
            min
        };
        s.chars().take(len).collect()
    }
}

/// Decrypts websocket event payload and returns the inner `data` object
/// (mirrors the JS `decodeEventData(...).data` access pattern).
pub fn decode_event_data(
    parsed: &serde_json::Value,
    cipher_key: Option<&str>,
) -> Result<serde_json::Value, ZaloError> {
    // 1. If parsed already has target payload arrays at the top level:
    if parsed.get("delivereds").is_some()
        || parsed.get("seens").is_some()
        || parsed.get("msgs").is_some()
        || parsed.get("groupSeens").is_some()
        || parsed.get("controls").is_some()
    {
        return Ok(parsed.clone());
    }

    // 2. If parsed["data"] is already a JSON object / array directly:
    if let Some(inner_obj) = parsed.get("data") {
        if inner_obj.is_object() || inner_obj.is_array() {
            if let Some(data_field) = inner_obj.get("data") {
                if data_field.is_object() || data_field.is_array() {
                    return Ok(data_field.clone());
                }
            }
            return Ok(inner_obj.clone());
        }
    }

    let raw_data = parsed
        .get("data")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ZaloError::PayloadFormat("Missing data field".to_string()))?;

    let encrypt_type = parsed
        .get("encrypt")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    if encrypt_type == 0 {
        let parsed_inner: serde_json::Value = serde_json::from_str(raw_data)?;
        if let Some(data_field) = parsed_inner.get("data") {
            if data_field.is_object() || data_field.is_array() {
                return Ok(data_field.clone());
            }
        }
        return Ok(parsed_inner);
    }

    let decoded_buffer = if encrypt_type == 1 {
        BASE64_STANDARD
            .decode(raw_data)
            .map_err(|e| ZaloError::Crypto(e.to_string()))?
    } else {
        let decoded_url = urlencoding::decode(raw_data)
            .map_err(|e: std::string::FromUtf8Error| ZaloError::Crypto(e.to_string()))?;
        BASE64_STANDARD
            .decode(decoded_url.as_ref())
            .map_err(|e| ZaloError::Crypto(e.to_string()))?
    };

    let decrypted_buffer = if encrypt_type != 1 {
        let key_str = cipher_key
            .ok_or_else(|| ZaloError::Crypto("Missing cipher key for GCM".to_string()))?;
        let key_bytes = BASE64_STANDARD
            .decode(key_str)
            .map_err(|e| ZaloError::Crypto(e.to_string()))?;

        if decoded_buffer.len() < 48 {
            return Err(ZaloError::Crypto("Invalid GCM payload length".to_string()));
        }

        let iv = decoded_buffer
            .get(0..16)
            .ok_or_else(|| ZaloError::Crypto("Invalid GCM iv length".to_string()))?;
        let aad = decoded_buffer
            .get(16..32)
            .ok_or_else(|| ZaloError::Crypto("Invalid GCM aad length".to_string()))?;
        let ciphertext = decoded_buffer
            .get(32..)
            .ok_or_else(|| ZaloError::Crypto("Invalid GCM ciphertext length".to_string()))?;

        aes_gcm_decrypt(&key_bytes, iv, aad, ciphertext)?
    } else {
        decoded_buffer
    };

    let decompressed = if encrypt_type == 3 {
        decrypted_buffer
    } else {
        zlib_decompress(&decrypted_buffer)?
    };

    let s = String::from_utf8(decompressed).map_err(|e| ZaloError::Crypto(e.to_string()))?;

    let parsed_inner: serde_json::Value = serde_json::from_str(&s)?;
    if let Some(data_field) = parsed_inner.get("data") {
        if data_field.is_object() || data_field.is_array() {
            return Ok(data_field.clone());
        }
    }
    Ok(parsed_inner)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
    use flate2::Compression;
    use std::io::Write;

    fn compressed_payload<E>(mut encoder: E) -> Vec<u8>
    where
        E: Write + FinishEncoder,
    {
        encoder.write_all(b"zalo-history-payload").unwrap();
        encoder.finish_encoder().unwrap()
    }

    trait FinishEncoder {
        fn finish_encoder(self) -> std::io::Result<Vec<u8>>;
    }

    impl FinishEncoder for ZlibEncoder<Vec<u8>> {
        fn finish_encoder(self) -> std::io::Result<Vec<u8>> {
            self.finish()
        }
    }

    impl FinishEncoder for GzEncoder<Vec<u8>> {
        fn finish_encoder(self) -> std::io::Result<Vec<u8>> {
            self.finish()
        }
    }

    impl FinishEncoder for DeflateEncoder<Vec<u8>> {
        fn finish_encoder(self) -> std::io::Result<Vec<u8>> {
            self.finish()
        }
    }

    #[test]
    fn websocket_decompression_accepts_zlib_gzip_and_raw_deflate() {
        let payloads = [
            compressed_payload(ZlibEncoder::new(Vec::new(), Compression::default())),
            compressed_payload(GzEncoder::new(Vec::new(), Compression::default())),
            compressed_payload(DeflateEncoder::new(Vec::new(), Compression::default())),
        ];

        for payload in payloads {
            assert_eq!(zlib_decompress(&payload).unwrap(), b"zalo-history-payload");
        }
    }

    #[test]
    fn decode_event_data_unwraps_inner_data_for_plaintext() {
        // encrypt == 0: `data` is a JSON string wrapping the real payload under `.data`.
        // Regression test for the listener bug where handlers read `.msgs` directly
        // instead of `.data.msgs`.
        let inner = serde_json::json!({ "data": { "msgs": [{ "id": "1" }] } });
        let parsed = serde_json::json!({
            "data": inner.to_string(),
            "encrypt": 0,
        });

        let out = decode_event_data(&parsed, None).expect("should decode");
        let msgs = out.get("msgs").and_then(|v| v.as_array());
        assert!(
            msgs.is_some(),
            "decoded payload must expose msgs at top level"
        );
        assert_eq!(msgs.unwrap().len(), 1);

        // Variant 1: un-nested plaintext delivery payload
        let plain_del = serde_json::json!({ "delivereds": [{ "msgId": "123" }] });
        let parsed_del = serde_json::json!({
            "data": plain_del.to_string(),
            "encrypt": 0,
        });
        let out_del = decode_event_data(&parsed_del, None).expect("should decode un-nested");
        assert!(out_del.get("delivereds").is_some());

        // Variant 2: already parsed direct JSON object
        let direct = serde_json::json!({
            "data": { "delivereds": [{ "msgId": "456" }] }
        });
        let out_direct = decode_event_data(&direct, None).expect("should decode direct object");
        assert!(out_direct.get("delivereds").is_some());

        // Variant 3: top level delivereds array
        let top_level = serde_json::json!({
            "delivereds": [{ "msgId": "789" }]
        });
        let out_top = decode_event_data(&top_level, None).expect("should decode top-level");
        assert!(out_top.get("delivereds").is_some());
    }
}
