use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::context::{Context, Options, ZaloCookie};
use crate::error::ZaloError;
use crate::listener::{Listener, ListenerEvent};

pub mod auth;
pub mod catalog;
pub mod custom;
pub mod friend;
pub mod group;
pub mod message;
pub mod misc;
pub mod poll;
pub mod profile;
pub mod reminder;
pub mod sticker;
pub mod upload;

fn new_qr_login_context(options: Options) -> Result<(String, Arc<Context>), ZaloError> {
    let user_agent =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0"
            .to_string();
    let imei = crate::crypto::generate_zalo_uuid(&user_agent);
    let context = Arc::new(Context::new(imei.clone(), user_agent, Vec::new(), options)?);

    Ok((imei, context))
}

fn json_value_type(value: Option<&Value>) -> &'static str {
    match value {
        None => "missing",
        Some(Value::Null) => "null",
        Some(Value::Bool(_)) => "boolean",
        Some(Value::Number(_)) => "number",
        Some(Value::String(_)) => "string",
        Some(Value::Array(_)) => "array",
        Some(Value::Object(_)) => "object",
    }
}

fn sorted_json_keys(value: Option<&Value>) -> String {
    let Some(object) = value.and_then(Value::as_object) else {
        return String::new();
    };

    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    keys.join(",")
}

fn login_response_summary(response: &Value) -> String {
    let data = response.get("data");
    let error_code = response
        .get("error_code")
        .map(Value::to_string)
        .unwrap_or_else(|| "missing".to_string());
    let error_message = response
        .get("error_message")
        .and_then(Value::as_str)
        .map(|message| message.replace(['\r', '\n'], " "))
        .unwrap_or_else(|| "missing".to_string());

    format!(
        "top_level_keys=[{}], data_type={}, data_keys=[{}], error_code={}, error_message={}",
        sorted_json_keys(Some(response)),
        json_value_type(data),
        sorted_json_keys(data),
        error_code,
        error_message
    )
}

fn extract_login_info(response: &Value) -> Result<&Value, ZaloError> {
    let error_code = response
        .get("error_code")
        .and_then(Value::as_i64)
        .unwrap_or(0);

    if error_code != 0 {
        eprintln!(
            "Zalo login response shape: {}",
            login_response_summary(response)
        );
        let message = response
            .get("error_message")
            .and_then(Value::as_str)
            .unwrap_or("Zalo login failed");
        return Err(ZaloError::ApiError {
            message: message.to_string(),
            code: Some(error_code as i32),
        });
    }

    response
        .get("data")
        .filter(|data| data.is_object())
        .ok_or_else(|| {
            eprintln!(
                "Zalo login response shape: {}",
                login_response_summary(response)
            );
            ZaloError::ApiError {
                message: "Login failed: missing data field".to_string(),
                code: None,
            }
        })
}

fn cookie_matches_endpoint(cookie: &ZaloCookie, domain: &str, path: &str) -> bool {
    (cookie.domain == domain || domain.ends_with(&format!(".{}", cookie.domain)))
        && path.starts_with(&cookie.path)
}

fn login_cookie_candidates(
    cookies: &[ZaloCookie],
    request_domain: &str,
    request_path: &str,
) -> Vec<Vec<ZaloCookie>> {
    let mut session_key_indexes: Vec<usize> = cookies
        .iter()
        .enumerate()
        .filter(|(_, cookie)| {
            cookie.name == "zpw_sek"
                && cookie_matches_endpoint(cookie, request_domain, request_path)
        })
        .map(|(index, _)| index)
        .collect();

    if session_key_indexes.len() <= 1 {
        return vec![cookies.to_vec()];
    }

    session_key_indexes.sort_unstable_by(|left, right| {
        cookies[*right]
            .domain
            .len()
            .cmp(&cookies[*left].domain.len())
            .then_with(|| cookies[*right].path.len().cmp(&cookies[*left].path.len()))
            .then_with(|| left.cmp(right))
    });

    session_key_indexes
        .into_iter()
        .map(|selected_index| {
            cookies
                .iter()
                .enumerate()
                .filter(|(index, cookie)| {
                    cookie.name != "zpw_sek"
                        || !cookie_matches_endpoint(cookie, request_domain, request_path)
                        || *index == selected_index
                })
                .map(|(_, cookie)| cookie.clone())
                .collect()
        })
        .collect()
}

fn is_session_key_error(error: &ZaloError) -> bool {
    matches!(
        error,
        ZaloError::ApiError {
            code: Some(102),
            ..
        }
    )
}

fn login_cookie_summary(ctx: &Context) -> String {
    let cookies = ctx.cookie_jar.to_vec();
    let mut all: Vec<String> = cookies
        .iter()
        .map(|cookie| format!("{}@{}{}", cookie.name, cookie.domain, cookie.path))
        .collect();
    all.sort_unstable();

    let request_domain = "wpa.chat.zalo.me";
    let request_path = "/";
    let sent_cookies: Vec<&ZaloCookie> = cookies
        .iter()
        .filter(|cookie| cookie_matches_endpoint(cookie, request_domain, request_path))
        .collect();
    let sent_to_wpa = sent_cookies
        .iter()
        .map(|cookie| format!("{}@{}", cookie.name, cookie.domain))
        .collect::<Vec<_>>()
        .join(",");

    let zpw_sek: Vec<&ZaloCookie> = sent_cookies
        .into_iter()
        .filter(|cookie| cookie.name == "zpw_sek")
        .collect();
    let first_zpw_value = zpw_sek.first().map(|cookie| cookie.value.as_str());
    let zpw_sek_metadata = zpw_sek
        .iter()
        .map(|cookie| {
            format!(
                "{}(len={},same_as_first={})",
                cookie.domain,
                cookie.value.len(),
                first_zpw_value == Some(cookie.value.as_str())
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "all=[{}], sent_to_wpa=[{}], zpw_sek=[{}]",
        all.join(","),
        sent_to_wpa,
        zpw_sek_metadata
    )
}

#[derive(Debug)]
pub struct Zalo {
    pub ctx: Arc<Context>,
    pub listener: Arc<Listener>,
}

impl Zalo {
    pub fn new(ctx: Arc<Context>, ws_urls: Vec<String>) -> Self {
        let listener = Arc::new(Listener::new(Arc::clone(&ctx), ws_urls));
        Self { ctx, listener }
    }

    pub async fn login(
        imei: String,
        user_agent: String,
        cookies: Vec<ZaloCookie>,
        options: Options,
    ) -> Result<Self, ZaloError> {
        let cookie_candidates = login_cookie_candidates(&cookies, "wpa.chat.zalo.me", "/");
        let candidate_count = cookie_candidates.len();

        for (index, candidate_cookies) in cookie_candidates.into_iter().enumerate() {
            let result = Self::login_once(
                imei.clone(),
                user_agent.clone(),
                candidate_cookies,
                options.clone(),
            )
            .await;

            if matches!(&result, Err(error) if is_session_key_error(error))
                && index + 1 < candidate_count
            {
                eprintln!(
                    "Zalo rejected session key candidate {}/{}; trying the next scoped cookie",
                    index + 1,
                    candidate_count
                );
                continue;
            }

            return result;
        }

        Err(ZaloError::PayloadFormat(
            "No Zalo login cookie candidates were available".to_string(),
        ))
    }

    async fn login_once(
        imei: String,
        user_agent: String,
        cookies: Vec<ZaloCookie>,
        options: Options,
    ) -> Result<Self, ZaloError> {
        let ctx = Arc::new(Context::new(imei, user_agent, cookies, options)?);

        // 1. Fetch login info
        let login_res = auth::login_call(&ctx, true).await?;
        let login_info = extract_login_info(&login_res).map_err(|error| {
            eprintln!("Zalo login cookie metadata: {}", login_cookie_summary(&ctx));
            error
        })?;

        // 2. Fetch server info
        let server_info = auth::get_server_info_call(&ctx, true).await?;

        // Extract values
        let zpw_enk = login_info
            .get("zpw_enk")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                eprintln!(
                    "Zalo login response shape: {}",
                    login_response_summary(&login_res)
                );
                eprintln!("Zalo login cookie metadata: {}", login_cookie_summary(&ctx));
                ZaloError::PayloadFormat("Missing zpw_enk in login response".to_string())
            })?;

        let uid = login_info
            .get("uid")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        // Service map
        let mut service_map = HashMap::new();
        if let Some(map_obj) = login_info
            .get("zpw_service_map_v3")
            .and_then(|v| v.as_object())
        {
            for (k, v) in map_obj {
                if let Some(arr) = v.as_array() {
                    let urls: Vec<String> = arr
                        .iter()
                        .filter_map(|url_val| url_val.as_str().map(|s| s.to_string()))
                        .collect();
                    service_map.insert(k.clone(), urls);
                }
            }
        }

        // WS URLs
        let mut ws_urls = Vec::new();
        if let Some(arr) = login_info.get("zpw_ws").and_then(|v| v.as_array()) {
            for v in arr {
                if let Some(s) = v.as_str() {
                    ws_urls.push(s.to_string());
                }
            }
        }

        // Settings (fallback settings / setttings)
        let settings = server_info
            .get("setttings")
            .or_else(|| server_info.get("settings"))
            .cloned()
            .unwrap_or(Value::Null);

        let extra_ver = server_info.get("extra_ver").cloned().unwrap_or(Value::Null);

        // Store session in context
        ctx.set_secret_key(zpw_enk.to_string());
        {
            let mut uid_guard = ctx.uid.lock().unwrap_or_else(|e| e.into_inner());
            *uid_guard = Some(uid);
        }
        {
            let mut sm_guard = ctx
                .zpw_service_map
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *sm_guard = service_map;
        }
        {
            let mut settings_guard = ctx.settings.lock().unwrap_or_else(|e| e.into_inner());
            *settings_guard = settings;
        }
        {
            let mut li_guard = ctx.login_info.lock().unwrap_or_else(|e| e.into_inner());
            *li_guard = login_info.clone();
        }
        {
            let mut ev_guard = ctx.extra_ver.lock().unwrap_or_else(|e| e.into_inner());
            *ev_guard = extra_ver;
        }

        let zalo = Zalo::new(ctx, ws_urls);
        if zalo.ctx.options.auto_start_listener {
            zalo.listener.start()?;
        }

        Ok(zalo)
    }

    pub async fn login_qr<F>(options: Options, callback: F) -> Result<Self, ZaloError>
    where
        F: Fn(auth::LoginQREvent) -> auth::QRCallbackResult + Send + Sync + 'static,
    {
        // 1. Load temporary context for authentication
        let (imei, ctx) = new_qr_login_context(options.clone())?;

        // 2. Perform QR scan flow
        let cookies = auth::login_qr_flow(&ctx, callback).await?;

        // 3. Login using the newly acquired credentials
        Zalo::login(imei, ctx.user_agent.clone(), cookies, options).await
    }

    pub fn get_service_url(&self, service: &str, path: &str) -> Result<String, ZaloError> {
        if service.starts_with("http://") || service.starts_with("https://") {
            let base = if service.ends_with('/') {
                format!("{}{}", service, path.trim_start_matches('/'))
            } else {
                format!("{}/{}", service, path.trim_start_matches('/'))
            };
            return Ok(base);
        }

        let service_map = self
            .ctx
            .zpw_service_map
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let host = service_map
            .get(service)
            .and_then(|hosts| hosts.first())
            .ok_or_else(|| ZaloError::Crypto(format!("Service host not found for: {}", service)))?;

        let base = if host.ends_with('/') {
            format!("{}{}", host, path.trim_start_matches('/'))
        } else {
            format!("{}/{}", host, path.trim_start_matches('/'))
        };
        Ok(base)
    }

    pub async fn request_api<Req, Resp>(
        &self,
        method: Method,
        service: &str,
        path: &str,
        data: Req,
        is_encrypted: bool,
    ) -> Result<Resp, ZaloError>
    where
        Req: Serialize,
        Resp: DeserializeOwned,
    {
        let service_url = self.get_service_url(service, path)?;

        let url = if is_encrypted {
            let secret_key = self
                .ctx
                .get_secret_key()
                .ok_or_else(|| ZaloError::Crypto("Context missing secret_key".to_string()))?;
            let stringified = serde_json::to_string(&data)?;
            let encrypted_params =
                crate::crypto::aes::encode_aes(&secret_key, stringified.as_bytes())?;
            self.ctx
                .make_url(&service_url, &[("params", &encrypted_params)], true)?
        } else {
            let mut query_params = Vec::new();
            let val = serde_json::to_value(&data)?;
            if let Some(obj) = val.as_object() {
                for (k, v) in obj {
                    let v_str = match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    query_params.push((k.clone(), v_str));
                }
            }
            let query_slices: Vec<(&str, &str)> = query_params
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            self.ctx.make_url(&service_url, &query_slices, true)?
        };

        let response = self
            .ctx
            .reqwest_client
            .request(method, &url)
            .header("Accept", "application/json, text/plain, */*")
            .header("Origin", "https://chat.zalo.me")
            .header("Referer", "https://chat.zalo.me/")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(ZaloError::ApiError {
                message: format!("HTTP error: {}", response.status()),
                code: Some(response.status().as_u16() as i32),
            });
        }

        let response_json: Value = response.json().await?;

        let error_code = response_json
            .get("error_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if error_code != 0 {
            let msg = response_json
                .get("error_message")
                .and_then(|v| v.as_str())
                .unwrap_or("API error");
            return Err(ZaloError::ApiError {
                message: msg.to_string(),
                code: Some(error_code as i32),
            });
        }

        let final_value = if is_encrypted {
            let secret_key = self
                .ctx
                .get_secret_key()
                .ok_or_else(|| ZaloError::Crypto("Context missing secret_key".to_string()))?;
            let encrypted_data = response_json
                .get("data")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ZaloError::PayloadFormat("Missing data field in encrypted response".to_string())
                })?;
            let decrypted_str = crate::crypto::aes::decode_aes(&secret_key, encrypted_data)?;
            let decrypted_json: Value = serde_json::from_str(&decrypted_str)?;

            let inner_error_code = decrypted_json
                .get("error_code")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if inner_error_code != 0 {
                let msg = decrypted_json
                    .get("error_message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Inner API error");
                return Err(ZaloError::ApiError {
                    message: msg.to_string(),
                    code: Some(inner_error_code as i32),
                });
            }

            decrypted_json.get("data").cloned().unwrap_or(Value::Null)
        } else {
            response_json.get("data").cloned().unwrap_or(Value::Null)
        };

        let result: Resp = serde_json::from_value(final_value)?;
        Ok(result)
    }

    pub async fn request_api_form<Req, Resp>(
        &self,
        service: &str,
        path: &str,
        data: Req,
    ) -> Result<Resp, ZaloError>
    where
        Req: Serialize,
        Resp: DeserializeOwned,
    {
        self.request_api_form_with_query(service, path, &[], data)
            .await
    }

    pub async fn request_api_form_with_query<Req, Resp>(
        &self,
        service: &str,
        path: &str,
        query: &[(&str, &str)],
        data: Req,
    ) -> Result<Resp, ZaloError>
    where
        Req: Serialize,
        Resp: DeserializeOwned,
    {
        let service_url = self.get_service_url(service, path)?;

        let secret_key = self
            .ctx
            .get_secret_key()
            .ok_or_else(|| ZaloError::Crypto("Context missing secret_key".to_string()))?;
        let stringified = serde_json::to_string(&data)?;
        let encrypted_params = crate::crypto::aes::encode_aes(&secret_key, stringified.as_bytes())?;

        let url = self.ctx.make_url(&service_url, query, true)?;

        let params = [("params", &encrypted_params)];

        let response = self
            .ctx
            .reqwest_client
            .post(&url)
            .header("Accept", "application/json, text/plain, */*")
            .header("Origin", "https://chat.zalo.me")
            .header("Referer", "https://chat.zalo.me/")
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(ZaloError::ApiError {
                message: format!("HTTP error: {}", response.status()),
                code: Some(response.status().as_u16() as i32),
            });
        }

        let response_json: Value = response.json().await?;
        let error_code = response_json
            .get("error_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if error_code != 0 {
            let msg = response_json
                .get("error_message")
                .and_then(|v| v.as_str())
                .unwrap_or("API error");
            return Err(ZaloError::ApiError {
                message: msg.to_string(),
                code: Some(error_code as i32),
            });
        }

        let encrypted_data = response_json
            .get("data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ZaloError::PayloadFormat("Missing data field in encrypted response".to_string())
            })?;
        let decrypted_str = crate::crypto::aes::decode_aes(&secret_key, encrypted_data)?;
        let decrypted_json: Value = serde_json::from_str(&decrypted_str)?;

        let inner_error_code = decrypted_json
            .get("error_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if inner_error_code != 0 {
            let msg = decrypted_json
                .get("error_message")
                .and_then(|v| v.as_str())
                .unwrap_or("Inner API error");
            return Err(ZaloError::ApiError {
                message: msg.to_string(),
                code: Some(inner_error_code as i32),
            });
        }

        let final_value = decrypted_json.get("data").cloned().unwrap_or(Value::Null);
        let result: Resp = serde_json::from_value(final_value)?;
        Ok(result)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ListenerEvent> {
        self.listener.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_login_context_reuses_the_device_imei() -> Result<(), ZaloError> {
        let (imei, context) = new_qr_login_context(Options::default())?;

        assert!(!imei.trim().is_empty());
        assert_eq!(context.imei, imei);
        Ok(())
    }

    #[test]
    fn login_response_summary_excludes_sensitive_values() {
        let response = serde_json::json!({
            "error_code": 201,
            "error_message": "Session is not ready",
            "data": {
                "uid": "hidden-user-id",
                "zpw_enk": "hidden-secret",
                "zpw_ws": ["wss://hidden.example"]
            }
        });

        let summary = login_response_summary(&response);

        assert!(summary.contains("top_level_keys=[data,error_code,error_message]"));
        assert!(summary.contains("data_type=object"));
        assert!(summary.contains("data_keys=[uid,zpw_enk,zpw_ws]"));
        assert!(summary.contains("error_code=201"));
        assert!(summary.contains("error_message=Session is not ready"));
        assert!(!summary.contains("hidden-user-id"));
        assert!(!summary.contains("hidden-secret"));
        assert!(!summary.contains("hidden.example"));
    }

    #[test]
    fn extract_login_info_preserves_upstream_session_error() {
        let response = serde_json::json!({
            "error_code": 102,
            "error_message": "The session key is invalid",
            "data": null
        });

        let error = extract_login_info(&response).unwrap_err();

        match error {
            ZaloError::ApiError { message, code } => {
                assert_eq!(message, "The session key is invalid");
                assert_eq!(code, Some(102));
            }
            other => panic!("expected API error, got {other:?}"),
        }
    }

    #[test]
    fn login_cookie_summary_excludes_cookie_values() -> Result<(), ZaloError> {
        let context = Context::new(
            "test-imei".to_string(),
            "test-user-agent".to_string(),
            vec![
                ZaloCookie {
                    name: "zpw_sek".to_string(),
                    value: "hidden-session-key".to_string(),
                    domain: "chat.zalo.me".to_string(),
                    path: "/".to_string(),
                },
                ZaloCookie {
                    name: "zalo_id".to_string(),
                    value: "hidden-id-cookie".to_string(),
                    domain: "id.zalo.me".to_string(),
                    path: "/".to_string(),
                },
            ],
            Options::default(),
        )?;

        let summary = login_cookie_summary(&context);

        assert!(summary.contains("all=[zalo_id@id.zalo.me/,zpw_sek@chat.zalo.me/]"));
        assert!(summary.contains("sent_to_wpa=[zpw_sek@chat.zalo.me]"));
        assert!(summary.contains("zpw_sek=[chat.zalo.me(len=18,same_as_first=true)]"));
        assert!(!summary.contains("hidden-session-key"));
        assert!(!summary.contains("hidden-id-cookie"));
        Ok(())
    }

    #[test]
    fn login_cookie_candidates_try_each_matching_session_key_separately() {
        let cookies = vec![
            ZaloCookie {
                name: "zpw_sek".to_string(),
                value: "parent-key".to_string(),
                domain: "zalo.me".to_string(),
                path: "/".to_string(),
            },
            ZaloCookie {
                name: "zpw_sek".to_string(),
                value: "specific-key".to_string(),
                domain: "chat.zalo.me".to_string(),
                path: "/".to_string(),
            },
            ZaloCookie {
                name: "zpw_sek".to_string(),
                value: "other-tld-key".to_string(),
                domain: "chat.zalo.gg".to_string(),
                path: "/".to_string(),
            },
            ZaloCookie {
                name: "zpsid".to_string(),
                value: "session-id".to_string(),
                domain: "zalo.me".to_string(),
                path: "/".to_string(),
            },
        ];

        let candidates = login_cookie_candidates(&cookies, "wpa.chat.zalo.me", "/");

        assert_eq!(candidates.len(), 2);
        let matching_domains = |candidate: &[ZaloCookie]| {
            candidate
                .iter()
                .filter(|cookie| {
                    cookie.name == "zpw_sek"
                        && (cookie.domain == "wpa.chat.zalo.me"
                            || "wpa.chat.zalo.me".ends_with(&format!(".{}", cookie.domain)))
                })
                .map(|cookie| cookie.domain.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            matching_domains(&candidates[0]),
            vec!["chat.zalo.me".to_string()]
        );
        assert_eq!(
            matching_domains(&candidates[1]),
            vec!["zalo.me".to_string()]
        );
        assert!(candidates.iter().all(|candidate| candidate
            .iter()
            .any(|cookie| { cookie.domain == "chat.zalo.gg" && cookie.value == "other-tld-key" })));
    }
}
