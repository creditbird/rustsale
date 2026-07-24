use crate::error::ZaloError;
use reqwest::cookie::CookieStore;
use reqwest::header::HeaderValue;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use url::Url;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ZaloCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
}

#[derive(Debug, Default)]
pub struct ZaloCookieJar {
    cookies: Mutex<Vec<ZaloCookie>>,
}

fn parse_set_cookie(cookie_str: &str, default_domain: &str) -> Option<ZaloCookie> {
    let mut parts = cookie_str.split(';');
    let first = parts.next()?;
    let mut eq_parts = first.splitn(2, '=');
    let name = eq_parts.next()?.trim().to_string();
    let value = eq_parts.next()?.trim().to_string();

    let mut domain = default_domain.to_string();
    let mut path = "/".to_string();

    for part in parts {
        let part = part.trim();
        let mut kv = part.splitn(2, '=');
        if let Some(key) = kv.next() {
            let key = key.trim().to_ascii_lowercase();
            let val = kv.next().unwrap_or("").trim().to_string();
            if key == "domain" {
                let mut d = val;
                if d.starts_with('.') {
                    d = d[1..].to_string();
                }
                domain = d;
            } else if key == "path" {
                path = val;
            }
        }
    }

    Some(ZaloCookie {
        name,
        value,
        domain,
        path,
    })
}

fn is_cookie_name_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'!' | b'#'..=b'\'' | b'*' | b'+' | b'-' | b'.' | b'0'..=b'9' | b'A'..=b'Z'
            | b'^'..=b'z' | b'|'
    )
}

fn split_set_cookie_header(header: &str) -> Vec<&str> {
    let bytes = header.as_bytes();
    let mut cookies = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'"' if index == 0 || bytes[index - 1] != b'\\' => {
                in_quotes = !in_quotes;
            }
            b',' if !in_quotes => {
                let mut next = index + 1;
                while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                    next += 1;
                }

                let name_start = next;
                while next < bytes.len() && is_cookie_name_byte(bytes[next]) {
                    next += 1;
                }

                if next > name_start && bytes.get(next) == Some(&b'=') {
                    let cookie = header[start..index].trim();
                    if !cookie.is_empty() {
                        cookies.push(cookie);
                    }
                    start = name_start;
                    index = next;
                }
            }
            _ => {}
        }
        index += 1;
    }

    let cookie = header[start..].trim();
    if !cookie.is_empty() {
        cookies.push(cookie);
    }
    cookies
}

impl ZaloCookieJar {
    pub fn new(cookies: Vec<ZaloCookie>) -> Self {
        Self {
            cookies: Mutex::new(cookies),
        }
    }

    pub fn to_vec(&self) -> Vec<ZaloCookie> {
        let list = self.cookies.lock().unwrap_or_else(|e| e.into_inner());
        list.clone()
    }

    pub fn get_cookie_string(&self, url: &Url) -> String {
        let domain = url.host_str().unwrap_or("");
        let path = url.path();
        let list = self.cookies.lock().unwrap_or_else(|e| e.into_inner());
        let mut cookie_strs = Vec::new();
        for c in list.iter() {
            let domain_matches = c.domain == domain || domain.ends_with(&format!(".{}", c.domain));

            let path_matches = path.starts_with(&c.path);
            if domain_matches && path_matches {
                cookie_strs.push(format!("{}={}", c.name, c.value));
            }
        }
        cookie_strs.join("; ")
    }
}

impl CookieStore for ZaloCookieJar {
    fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
        let default_domain = url.host_str().unwrap_or("zalo.me");
        let mut list = self.cookies.lock().unwrap_or_else(|e| e.into_inner());
        for val in cookie_headers {
            if let Ok(s) = val.to_str() {
                for raw_cookie in split_set_cookie_header(s) {
                    if let Some(cookie) = parse_set_cookie(raw_cookie, default_domain) {
                        if let Some(existing) = list.iter_mut().find(|c| {
                            c.name == cookie.name
                                && c.domain == cookie.domain
                                && c.path == cookie.path
                        }) {
                            existing.value = cookie.value;
                        } else {
                            list.push(cookie);
                        }
                    }
                }
            }
        }
    }

    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        let list = self.cookies.lock().unwrap_or_else(|e| e.into_inner());
        let domain = url.host_str().unwrap_or("");
        let path = url.path();

        let mut cookie_strs = Vec::new();
        for c in list.iter() {
            let domain_matches = c.domain == domain || domain.ends_with(&format!(".{}", c.domain));

            let path_matches = path.starts_with(&c.path);

            if domain_matches && path_matches {
                cookie_strs.push(format!("{}={}", c.name, c.value));
            }
        }

        if cookie_strs.is_empty() {
            None
        } else {
            let joined = cookie_strs.join("; ");
            HeaderValue::from_str(&joined).ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_store_keeps_all_cookies_from_combined_header() {
        let jar = ZaloCookieJar::new(Vec::new());
        let url = Url::parse("https://id.zalo.me/account/checksession").unwrap();
        let header = HeaderValue::from_static(
            "zalo_id=id-value; Path=/; Expires=Wed, 18 Mar 2026 10:00:00 GMT, zpw_sek=session-value; Domain=.zalo.me; Path=/; HttpOnly",
        );

        jar.set_cookies(&mut std::iter::once(&header), &url);

        let mut names: Vec<String> = jar.to_vec().into_iter().map(|cookie| cookie.name).collect();
        names.sort_unstable();
        assert_eq!(names, vec!["zalo_id", "zpw_sek"]);
    }
}

#[derive(Debug, Clone)]
pub struct ImageMetadataGetterResponse {
    pub width: u32,
    pub height: u32,
    pub size: u64,
}

pub type ImageMetadataGetter =
    Arc<dyn Fn(&std::path::Path) -> Result<ImageMetadataGetterResponse, ZaloError> + Send + Sync>;

#[cfg(feature = "image-metadata")]
pub fn default_image_metadata_getter(
    path: &std::path::Path,
) -> Result<ImageMetadataGetterResponse, ZaloError> {
    let size = std::fs::metadata(path)?.len();
    let reader = image::ImageReader::open(path)?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|e| ZaloError::Crypto(e.to_string()))?;
    Ok(ImageMetadataGetterResponse {
        width,
        height,
        size,
    })
}

#[derive(Clone)]
pub struct Options {
    pub self_listen: bool,
    pub check_update: bool,
    pub logging: bool,
    pub api_type: i32,
    pub api_version: i32,
    pub image_metadata_getter: Option<ImageMetadataGetter>,
}

impl std::fmt::Debug for Options {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Options")
            .field("self_listen", &self.self_listen)
            .field("check_update", &self.check_update)
            .field("logging", &self.logging)
            .field("api_type", &self.api_type)
            .field("api_version", &self.api_version)
            .field(
                "image_metadata_getter",
                &self.image_metadata_getter.as_ref().map(|_| "<callback>"),
            )
            .finish()
    }
}

impl Default for Options {
    fn default() -> Self {
        Self {
            self_listen: false,
            check_update: true,
            logging: true,
            api_type: 30,
            api_version: 671,
            image_metadata_getter: None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UploadEventData {
    pub file_url: String,
    pub file_id: String,
}

#[derive(Debug)]
pub struct Context {
    pub uid: Mutex<Option<String>>,
    pub imei: String,
    pub user_agent: String,
    pub language: String,
    pub secret_key: Mutex<Option<String>>,
    pub zpw_service_map: Mutex<HashMap<String, Vec<String>>>,
    pub settings: Mutex<serde_json::Value>,
    pub login_info: Mutex<serde_json::Value>,
    pub extra_ver: Mutex<serde_json::Value>,
    pub cookie_jar: Arc<ZaloCookieJar>,
    pub reqwest_client: reqwest::Client,
    pub upload_callbacks: Mutex<HashMap<String, tokio::sync::oneshot::Sender<UploadEventData>>>,
    pub options: Options,
}

impl Context {
    pub fn new(
        imei: String,
        user_agent: String,
        cookies: Vec<ZaloCookie>,
        options: Options,
    ) -> Result<Self, ZaloError> {
        let cookie_jar = Arc::new(ZaloCookieJar::new(cookies));

        let builder = reqwest::Client::builder()
            .cookie_provider(Arc::clone(&cookie_jar))
            .user_agent(&user_agent);

        let reqwest_client = builder.build()?;

        Ok(Self {
            uid: Mutex::new(None),
            imei,
            user_agent,
            language: "vi".to_string(),
            secret_key: Mutex::new(None),
            zpw_service_map: Mutex::new(HashMap::new()),
            settings: Mutex::new(serde_json::Value::Null),
            login_info: Mutex::new(serde_json::Value::Null),
            extra_ver: Mutex::new(serde_json::Value::Null),
            cookie_jar,
            reqwest_client,
            upload_callbacks: Mutex::new(HashMap::new()),
            options,
        })
    }

    pub fn get_secret_key(&self) -> Option<String> {
        self.secret_key
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_secret_key(&self, key: String) {
        let mut guard = self.secret_key.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(key);
    }

    pub fn make_url(
        &self,
        base_url: &str,
        params: &[(&str, &str)],
        api_version: bool,
    ) -> Result<String, ZaloError> {
        let mut url = Url::parse(base_url)?;
        {
            let mut query = url.query_pairs_mut();
            for (k, v) in params {
                query.append_pair(k, v);
            }
            if api_version {
                query.append_pair("zpw_ver", &self.options.api_version.to_string());
                query.append_pair("zpw_type", &self.options.api_type.to_string());
            }
        }
        Ok(url.into())
    }
}
