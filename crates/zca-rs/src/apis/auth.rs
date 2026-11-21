use crate::context::{Context, ZaloCookie};
use crate::crypto::{decrypt_resp, get_sign_key, ParamsEncryptor};
use crate::error::ZaloError;
use base64::Engine;
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub enum LoginQREvent {
    QRCodeGenerated {
        code: String,
        image_base64: String,
        token: String,
    },
    QRCodeExpired,
    QRCodeScanned {
        avatar: String,
        display_name: String,
    },
    QRCodeDeclined,
    GotLoginInfo {
        cookie: Vec<ZaloCookie>,
        imei: String,
        user_agent: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QRCallbackResult {
    Continue,
    Cancel,
}

pub fn get_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn encode_aes_plain(key: &str, plaintext: &str) -> Result<String, ZaloError> {
    let iv = [0u8; 16];
    let encrypted = crate::crypto::aes::aes_cbc_encrypt(key.as_bytes(), &iv, plaintext.as_bytes())?;
    Ok(base64::prelude::BASE64_STANDARD.encode(encrypted))
}

async fn get_encrypt_param(
    ctx: &Context,
    encrypt_params: bool,
    req_type: &str,
) -> Result<(serde_json::Map<String, Value>, Option<String>), ZaloError> {
    let ts = get_timestamp_ms();
    let data = serde_json::json!({
        "computer_name": "Web",
        "imei": ctx.imei,
        "language": ctx.language,
        "ts": ts,
    });

    let mut params = serde_json::Map::new();
    let mut enk = None;

    if encrypt_params {
        let encryptor = ParamsEncryptor::new(ctx.options.api_type, &ctx.imei, ts)?;
        let stringified = data.to_string();
        let encrypt_key = encryptor.get_encrypt_key()?;
        let encoded_data = encode_aes_plain(encrypt_key, &stringified)?;

        enk = Some(encrypt_key.to_string());

        if let Some(enc_params_val) = encryptor.get_params() {
            if let Some(obj) = enc_params_val.as_object() {
                for (k, v) in obj {
                    params.insert(k.clone(), v.clone());
                }
            }
        }
        params.insert("params".to_string(), Value::String(encoded_data));
    } else {
        params.insert(
            "computer_name".to_string(),
            Value::String("Web".to_string()),
        );
        params.insert("imei".to_string(), Value::String(ctx.imei.clone()));
        params.insert("language".to_string(), Value::String(ctx.language.clone()));
        params.insert("ts".to_string(), Value::Number(ts.into()));
    }

    params.insert(
        "type".to_string(),
        Value::Number(ctx.options.api_type.into()),
    );
    params.insert(
        "client_version".to_string(),
        Value::Number(ctx.options.api_version.into()),
    );

    let signkey_val = if req_type == "getserverinfo" {
        serde_json::json!({
            "imei": ctx.imei,
            "type": ctx.options.api_type,
            "client_version": ctx.options.api_version,
            "computer_name": "Web"
        })
    } else {
        Value::Object(params.clone())
    };

    let signkey = get_sign_key(req_type, &signkey_val);
    params.insert("signkey".to_string(), Value::String(signkey));

    Ok((params, enk))
}

pub async fn login_call(ctx: &Context, encrypt_params: bool) -> Result<Value, ZaloError> {
    let (mut encrypted_params, enk) =
        get_encrypt_param(ctx, encrypt_params, "getlogininfo").await?;
    encrypted_params.insert("nretry".to_string(), Value::Number(0.into()));

    let mut query_items = Vec::new();
    for (k, v) in &encrypted_params {
        let v_str = match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        query_items.push((k.clone(), v_str));
    }

    let query_slices: Vec<(&str, &str)> = query_items
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let url = ctx.make_url(
        "https://wpa.chat.zalo.me/api/login/getLoginInfo",
        &query_slices,
        true,
    )?;

    let response = ctx
        .reqwest_client
        .get(&url)
        .header("Accept", "application/json, text/plain, */*")
        .header("Origin", "https://chat.zalo.me")
        .header("Referer", "https://chat.zalo.me/")
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(ZaloError::ApiError {
            message: format!("Failed to fetch login info: {}", response.status()),
            code: Some(response.status().as_u16() as i32),
        });
    }

    let val: Value = response.json().await?;

    if let Some(enk_str) = enk {
        let raw_data = val.get("data").and_then(|v| v.as_str()).ok_or_else(|| {
            ZaloError::PayloadFormat("Missing data field in login response".to_string())
        })?;

        let decrypted = decrypt_resp(&enk_str, raw_data)?;
        Ok(decrypted)
    } else {
        Ok(val)
    }
}

pub async fn get_server_info_call(ctx: &Context, encrypt_params: bool) -> Result<Value, ZaloError> {
    let (encrypted_params, _) = get_encrypt_param(ctx, encrypt_params, "getserverinfo").await?;
    let signkey = encrypted_params
        .get("signkey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ZaloError::PayloadFormat("Missing signkey in server info request".to_string())
        })?;

    let query_params = [
        ("imei", ctx.imei.as_str()),
        ("type", &ctx.options.api_type.to_string()),
        ("client_version", &ctx.options.api_version.to_string()),
        ("computer_name", "Web"),
        ("signkey", signkey),
    ];

    let url = ctx.make_url(
        "https://wpa.chat.zalo.me/api/login/getServerInfo",
        &query_params,
        false,
    )?;

    let response = ctx
        .reqwest_client
        .get(&url)
        .header("Accept", "application/json, text/plain, */*")
        .header("Origin", "https://chat.zalo.me")
        .header("Referer", "https://chat.zalo.me/")
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(ZaloError::ApiError {
            message: format!("Failed to fetch server info: {}", response.status()),
            code: Some(response.status().as_u16() as i32),
        });
    }

    let val: Value = response.json().await?;
    let data = val.get("data").ok_or_else(|| {
        let msg = val
            .get("error_message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown server info error");
        ZaloError::ApiError {
            message: msg.to_string(),
            code: val
                .get("error_code")
                .and_then(|v| v.as_i64())
                .map(|n| n as i32),
        }
    })?;

    Ok(data.clone())
}

fn extract_login_version(html: &str) -> Option<String> {
    let prefix = "https://stc-zlogin.zdn.vn/main-";
    let start_idx = html.find(prefix)?;
    let start_version = start_idx + prefix.len();
    let rest = &html[start_version..];
    let end_idx = rest.find(".js")?;
    Some(rest[..end_idx].to_string())
}

async fn load_login_page(ctx: &Context) -> Result<String, ZaloError> {
    let response = ctx.reqwest_client.get("https://id.zalo.me/account?continue=https%3A%2F%2Fchat.zalo.me%2F")
        .header("accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7")
        .header("accept-language", "vi-VN,vi;q=0.9,fr-FR;q=0.8,fr;q=0.7,en-US;q=0.6,en;q=0.5")
        .header("Referer", "https://chat.zalo.me/")
        .send().await?;

    let html = response.text().await?;
    extract_login_version(&html)
        .ok_or_else(|| ZaloError::Crypto("Cannot get API login version from HTML".to_string()))
}

async fn post_form_zlogin(
    ctx: &Context,
    url: &str,
    form_data: &[(&str, &str)],
) -> Result<Value, ZaloError> {
    let mut form = HashMap::new();
    for &(k, v) in form_data {
        form.insert(k, v);
    }

    let response = ctx
        .reqwest_client
        .post(url)
        .header("accept", "*/*")
        .header(
            "accept-language",
            "vi-VN,vi;q=0.9,fr-FR;q=0.8,fr;q=0.7,en-US;q=0.6,en;q=0.5",
        )
        .header(
            "sec-ch-ua",
            r#""Chromium";v="130", "Google Chrome";v="130", "Not?A_Brand";v="99""#,
        )
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", r#""Windows""#)
        .header("sec-fetch-dest", "empty")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-site", "same-origin")
        .header(
            "Referer",
            "https://id.zalo.me/account?continue=https%3A%2F%2Fzalo.me%2Fpc",
        )
        .form(&form)
        .send()
        .await?;

    let val: Value = response.json().await?;
    Ok(val)
}

async fn waiting_scan(ctx: &Context, version: &str, code: &str) -> Result<Value, ZaloError> {
    loop {
        let val = post_form_zlogin(
            ctx,
            "https://id.zalo.me/account/authen/qr/waiting-scan",
            &[
                ("code", code),
                ("continue", "https://chat.zalo.me/"),
                ("v", version),
            ],
        )
        .await?;

        let error_code = val.get("error_code").and_then(|v| v.as_i64()).unwrap_or(0);
        if error_code == 8 {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            continue;
        }

        if error_code != 0 {
            let msg = val
                .get("error_message")
                .and_then(|v| v.as_str())
                .unwrap_or("Waiting scan failed");
            return Err(ZaloError::ApiError {
                message: msg.to_string(),
                code: Some(error_code as i32),
            });
        }

        return Ok(val);
    }
}

async fn waiting_confirm(ctx: &Context, version: &str, code: &str) -> Result<Value, ZaloError> {
    loop {
        let val = post_form_zlogin(
            ctx,
            "https://id.zalo.me/account/authen/qr/waiting-confirm",
            &[
                ("code", code),
                ("gToken", ""),
                ("gAction", "CONFIRM_QR"),
                ("continue", "https://chat.zalo.me/"),
                ("v", version),
            ],
        )
        .await?;

        let error_code = val.get("error_code").and_then(|v| v.as_i64()).unwrap_or(0);
        if error_code == 8 {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            continue;
        }

        return Ok(val);
    }
}

async fn check_session(ctx: &Context) -> Result<(), ZaloError> {
    ctx.reqwest_client
        .get("https://id.zalo.me/account/checksession?continue=https%3A%2F%2Fchat.zalo.me%2Findex.html")
        .header("accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7")
        .header("accept-language", "vi-VN,vi;q=0.9,fr-FR;q=0.8,fr;q=0.7,en-US;q=0.6,en;q=0.5")
        .header("sec-ch-ua", r#""Chromium";v="130", "Google Chrome";v="130", "Not?A_Brand";v="99""#)
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", r#""Windows""#)
        .header("sec-fetch-dest", "document")
        .header("sec-fetch-mode", "navigate")
        .header("sec-fetch-site", "same-origin")
        .header("upgrade-insecure-requests", "1")
        .header("Referer", "https://id.zalo.me/account?continue=https%3A%2F%2Fchat.zalo.me%2F")
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

pub async fn login_qr_flow<F>(ctx: &Context, callback: F) -> Result<Vec<ZaloCookie>, ZaloError>
where
    F: Fn(LoginQREvent) -> QRCallbackResult + Send + Sync + 'static,
{
    let login_version = load_login_page(ctx).await?;

    post_form_zlogin(
        ctx,
        "https://id.zalo.me/account/logininfo",
        &[("continue", "https://zalo.me/pc"), ("v", &login_version)],
    )
    .await?;

    post_form_zlogin(
        ctx,
        "https://id.zalo.me/account/verify-client",
        &[
            ("type", "device"),
            ("continue", "https://zalo.me/pc"),
            ("v", &login_version),
        ],
    )
    .await?;

    let qr_gen_result = post_form_zlogin(
        ctx,
        "https://id.zalo.me/account/authen/qr/generate",
        &[("continue", "https://zalo.me/pc"), ("v", &login_version)],
    )
    .await?;

    let qr_data = qr_gen_result.get("data").ok_or_else(|| {
        ZaloError::PayloadFormat("QRCode generation failed to return data".to_string())
    })?;

    let code = qr_data
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let image_base64 = qr_data
        .get("image")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .replace("data:image/png;base64,", "");
    let token = qr_data
        .get("token")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let cb_res = callback(LoginQREvent::QRCodeGenerated {
        code: code.clone(),
        image_base64: image_base64.clone(),
        token: token.clone(),
    });

    if cb_res == QRCallbackResult::Cancel {
        return Err(ZaloError::LoginQRAborted);
    }

    let scan_result = waiting_scan(ctx, &login_version, &code).await?;
    let scan_data = scan_result.get("data").ok_or_else(|| {
        ZaloError::PayloadFormat("Waiting scan failed to return data".to_string())
    })?;

    let avatar = scan_data
        .get("avatar")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let display_name = scan_data
        .get("display_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let cb_res = callback(LoginQREvent::QRCodeScanned {
        avatar,
        display_name,
    });

    if cb_res == QRCallbackResult::Cancel {
        return Err(ZaloError::LoginQRAborted);
    }

    let confirm_result = waiting_confirm(ctx, &login_version, &code).await?;
    let confirm_error_code = confirm_result
        .get("error_code")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    if confirm_error_code == -13 {
        let _ = callback(LoginQREvent::QRCodeDeclined);
        return Err(ZaloError::LoginQRDeclined);
    } else if confirm_error_code != 0 {
        let msg = confirm_result
            .get("error_message")
            .and_then(|v| v.as_str())
            .unwrap_or("Confirm failed");
        return Err(ZaloError::ApiError {
            message: msg.to_string(),
            code: Some(confirm_error_code as i32),
        });
    }

    check_session(ctx).await?;

    let cookies = ctx.cookie_jar.to_vec();

    let _ = callback(LoginQREvent::GotLoginInfo {
        cookie: cookies.clone(),
        imei: ctx.imei.clone(),
        user_agent: ctx.user_agent.clone(),
    });

    Ok(cookies)
}

pub fn save_qr_code_to_file(filepath: &str, image_base64: &str) -> Result<(), ZaloError> {
    let data = base64::prelude::BASE64_STANDARD
        .decode(image_base64)
        .map_err(|e| ZaloError::Crypto(e.to_string()))?;
    std::fs::write(filepath, data)?;
    Ok(())
}
