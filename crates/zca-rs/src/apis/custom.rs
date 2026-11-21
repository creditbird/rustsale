//! Escape hatch for calling Zalo endpoints that are not yet wrapped by a
//! dedicated method.
//!
//! This mirrors the `custom` factory in `zca-js`, which simply hands the caller
//! the session-bound building blocks (`makeURL`, `encodeAES`, `request`,
//! `resolve`). Here those are exposed as methods on [`Zalo`] so you can:
//!
//! 1. build a service URL from the logged-in `zpw_service_map`,
//! 2. encrypt request params with the session secret key,
//! 3. fire a raw request, and
//! 4. decrypt / unwrap the response envelope.
//!
//! ```no_run
//! # use zca_rs::Zalo;
//! # async fn demo(zalo: &Zalo) -> Result<(), zca_rs::ZaloError> {
//! let params = serde_json::json!({ "imei": "..." });
//! let result: serde_json::Value = zalo
//!     .custom_request(
//!         reqwest::Method::GET,
//!         "profile",
//!         "api/social/some-endpoint",
//!         params,
//!         true,
//!     )
//!     .await?;
//! # Ok(())
//! # }
//! ```

use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::apis::Zalo;
use crate::error::ZaloError;

impl Zalo {
    /// Encrypt arbitrary params with the current session secret key (AES-CBC),
    /// returning the base64 string Zalo expects in the `params` field.
    pub fn custom_encode_params<T: Serialize>(&self, data: &T) -> Result<String, ZaloError> {
        let secret_key = self
            .ctx
            .get_secret_key()
            .ok_or_else(|| ZaloError::Crypto("Context missing secret_key".to_string()))?;
        let stringified = serde_json::to_string(data)?;
        crate::crypto::aes::encode_aes(&secret_key, stringified.as_bytes())
    }

    /// Decrypt an encrypted `data` payload string returned by Zalo using the
    /// current session secret key, parsed into the requested type.
    pub fn custom_decode_response<T: DeserializeOwned>(
        &self,
        encrypted: &str,
    ) -> Result<T, ZaloError> {
        let secret_key = self
            .ctx
            .get_secret_key()
            .ok_or_else(|| ZaloError::Crypto("Context missing secret_key".to_string()))?;
        let decrypted = crate::crypto::aes::decode_aes(&secret_key, encrypted)?;
        Ok(serde_json::from_str(&decrypted)?)
    }

    /// Build a fully-qualified URL for a service entry from the session
    /// `zpw_service_map` (or an absolute URL passed through as-is), appending
    /// `zpw_ver`/`zpw_type` like the official client.
    pub fn custom_make_url(
        &self,
        service: &str,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<String, ZaloError> {
        let service_url = self.get_service_url(service, path)?;
        self.ctx.make_url(&service_url, query, true)
    }

    /// Call an arbitrary Zalo endpoint, reusing the same encrypt/sign/decrypt
    /// pipeline as the built-in methods.
    ///
    /// This is a thin alias over [`Zalo::request_api`] for discoverability;
    /// `service` may be a key in the `zpw_service_map` (e.g. `"profile"`,
    /// `"group"`) or an absolute `https://` URL.
    pub async fn custom_request<Req, Resp>(
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
        self.request_api(method, service, path, data, is_encrypted)
            .await
    }

    /// Like [`Zalo::custom_request`] but POSTs the encrypted params as a form
    /// body (matching endpoints that expect `application/x-www-form-urlencoded`).
    pub async fn custom_request_form<Req, Resp>(
        &self,
        service: &str,
        path: &str,
        data: Req,
    ) -> Result<Resp, ZaloError>
    where
        Req: Serialize,
        Resp: DeserializeOwned,
    {
        self.request_api_form(service, path, data).await
    }

    /// Lowest-level escape hatch: send a request and return the full, raw
    /// response envelope as a [`serde_json::Value`] without unwrapping `data`
    /// or attempting decryption. Use when an endpoint's envelope differs from
    /// the standard `{ error_code, data }` shape.
    pub async fn custom_request_raw<Req: Serialize>(
        &self,
        method: Method,
        service: &str,
        path: &str,
        data: Req,
        encrypt_params: bool,
    ) -> Result<Value, ZaloError> {
        let service_url = self.get_service_url(service, path)?;

        let url = if encrypt_params {
            let encrypted = self.custom_encode_params(&data)?;
            self.ctx
                .make_url(&service_url, &[("params", &encrypted)], true)?
        } else {
            let val = serde_json::to_value(&data)?;
            let mut query_params = Vec::new();
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

        Ok(response.json().await?)
    }
}
