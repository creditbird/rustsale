//! `read_attachment`: resolves one stored image attachment to bytes for MCP/REST clients.
//!
//! Sources are tried in order: the local file, the provider URL (bounded HTTP GET), then an
//! inline `data:` thumbnail. Provider URLs and local paths never leave this module: errors
//! carry fixed messages only.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use base64::Engine as _;
use tokio::io::AsyncReadExt;
use url::{Host, Url};

use super::error::{ApiError, ApiResult};
use crate::conversation::commands::ConversationState;
use crate::conversation::repository::AttachmentSource;

/// Largest image handed to a client. The bot gets it base64-encoded inside its context, so
/// this is deliberately far below the 50 MiB UI preview limit.
pub const READ_ATTACHMENT_MAX_BYTES: u64 = 8 * 1024 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Image bytes plus the MIME type detected from the content.
#[derive(Debug)]
pub struct AttachmentImage {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

#[derive(Debug)]
pub struct Fetched {
    pub bytes: Vec<u8>,
}

/// Bounded HTTP downloader for provider media URLs.
pub struct MediaFetcher {
    client: Option<reqwest::Client>,
    allow_private_hosts: bool,
    max_bytes: u64,
}

impl MediaFetcher {
    pub fn new(allow_private_hosts: bool, max_bytes: u64) -> Self {
        // Redirects are off: a provider URL must not bounce the request to another host.
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(FETCH_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(|error| {
                tracing::error!(target: "local_api", %error, "media http client unavailable");
            })
            .ok();
        Self {
            client,
            allow_private_hosts,
            max_bytes,
        }
    }

    /// Production settings: public hosts only, `READ_ATTACHMENT_MAX_BYTES`.
    pub fn production() -> Self {
        Self::new(false, READ_ATTACHMENT_MAX_BYTES)
    }

    pub async fn fetch(&self, raw_url: &str) -> ApiResult<Fetched> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| ApiError::download_failed("download client unavailable"))?;
        let url = Url::parse(raw_url)
            .map_err(|_| ApiError::download_failed("attachment link is not a valid URL"))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(ApiError::download_failed("attachment link is not http(s)"));
        }
        if !self.allow_private_hosts && url.host().is_none_or(|host| is_private_host(&host)) {
            return Err(ApiError::download_failed(
                "attachment link points to a private address",
            ));
        }
        let mut response = client.get(url).send().await.map_err(|error| {
            tracing::warn!(target: "local_api", timeout = error.is_timeout(), "attachment download failed");
            ApiError::download_failed("could not reach the attachment server")
        })?;
        let status = response.status();
        if matches!(status.as_u16(), 401 | 403 | 404 | 410) {
            return Err(ApiError::link_expired());
        }
        if !status.is_success() {
            tracing::warn!(target: "local_api", status = status.as_u16(), "attachment server refused");
            return Err(ApiError::download_failed(
                "the attachment server returned an error",
            ));
        }
        if response
            .content_length()
            .is_some_and(|len| len > self.max_bytes)
        {
            return Err(ApiError::too_large(self.max_bytes));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| ApiError::download_failed("the attachment download was interrupted"))?
        {
            if bytes.len() as u64 + chunk.len() as u64 > self.max_bytes {
                return Err(ApiError::too_large(self.max_bytes));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(Fetched { bytes })
    }
}

/// Hosts a provider CDN URL must never resolve to by name or literal (SSRF guard).
fn is_private_host(host: &Host<&str>) -> bool {
    match host {
        Host::Ipv4(ip) => is_private_v4(*ip),
        Host::Ipv6(ip) => is_private_v6(*ip),
        Host::Domain(name) => {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            name == "localhost"
                || name.ends_with(".localhost")
                || name.ends_with(".local")
                || name.ends_with(".internal")
                || name.parse::<IpAddr>().is_ok_and(|ip| match ip {
                    IpAddr::V4(v4) => is_private_v4(v4),
                    IpAddr::V6(v6) => is_private_v6(v6),
                })
        }
    }
}

fn is_private_v4(ip: Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.octets()[0] == 100 && (ip.octets()[1] & 0xc0) == 64 // 100.64.0.0/10
}

fn is_private_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_private_v4(v4);
    }
    ip.is_loopback()
        || ip.is_unspecified()
        || (ip.segments()[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
        || (ip.segments()[0] & 0xffc0) == 0xfe80 // link local fe80::/10
}

/// Image MIME types accepted from content sniffing.
fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    infer::get(bytes)
        .map(|kind| kind.mime_type())
        .filter(|mime| mime.starts_with("image/"))
}

fn decode_data_url(value: &str, max_bytes: u64) -> ApiResult<Option<Vec<u8>>> {
    let Some(rest) = value.strip_prefix("data:image/") else {
        return Ok(None);
    };
    let Some((_, payload)) = rest.split_once(";base64,") else {
        return Ok(None);
    };
    // Base64 inflates by 4/3; reject before decoding so a huge string is never allocated twice.
    if payload.len() as u64 / 4 * 3 > max_bytes.saturating_add(3) {
        return Err(ApiError::too_large(max_bytes));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .map_err(|_| ApiError::download_failed("stored thumbnail is not valid base64"))?;
    if bytes.len() as u64 > max_bytes {
        return Err(ApiError::too_large(max_bytes));
    }
    Ok(Some(bytes))
}

async fn read_local(path: &str, max_bytes: u64) -> ApiResult<Option<Vec<u8>>> {
    let Ok(file) = tokio::fs::File::open(path).await else {
        return Ok(None);
    };
    let Ok(metadata) = file.metadata().await else {
        return Ok(None);
    };
    if !metadata.is_file() {
        return Ok(None);
    }
    if metadata.len() > max_bytes {
        return Err(ApiError::too_large(max_bytes));
    }
    let mut bytes = Vec::new();
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| ApiError::download_failed("could not read the stored file"))?;
    if bytes.len() as u64 > max_bytes {
        return Err(ApiError::too_large(max_bytes));
    }
    Ok(Some(bytes))
}

fn is_http_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    // WhatsApp media URLs are encrypted blobs; they need the app's key flow, not a plain GET.
    (lower.starts_with("http://") || lower.starts_with("https://"))
        && !lower.contains("mmg.whatsapp.net")
}

async fn load_bytes(media: &MediaFetcher, source: &AttachmentSource) -> ApiResult<Vec<u8>> {
    let max = media.max_bytes;
    if let Some(path) = source
        .local_path
        .as_deref()
        .filter(|p| !p.trim().is_empty())
    {
        if let Some(bytes) = read_local(path, max).await? {
            return Ok(bytes);
        }
    }
    if let Some(url) = source.remote_url.as_deref().filter(|u| is_http_url(u)) {
        return Ok(media.fetch(url.trim()).await?.bytes);
    }
    if let Some(thumb) = source.thumbnail_url.as_deref() {
        if let Some(bytes) = decode_data_url(thumb, max)? {
            return Ok(bytes);
        }
    }
    Err(ApiError::download_failed(
        "no downloadable source for this attachment",
    ))
}

/// Loads one stored image attachment. Errors use the typed `read_attachment` codes.
pub async fn read_image(
    state: &ConversationState,
    media: &MediaFetcher,
    attachment_id: i64,
) -> ApiResult<AttachmentImage> {
    let source = state
        .service
        .repository
        .get_attachment_source(attachment_id)?
        .ok_or_else(ApiError::attachment_not_found)?;
    image_from_source(media, &source).await
}

/// Same as `read_image` once the stored source is known (also used by the DB-copy check).
async fn image_from_source(
    media: &MediaFetcher,
    source: &AttachmentSource,
) -> ApiResult<AttachmentImage> {
    if source.media_type != "image" {
        return Err(ApiError::unsupported_kind(&source.media_type));
    }
    if source.size_bytes.is_some_and(|size| size > media.max_bytes) {
        return Err(ApiError::too_large(media.max_bytes));
    }
    let bytes = load_bytes(media, source).await?;
    let mime = image_mime(&bytes)
        .ok_or_else(|| ApiError::download_failed("the attachment is not a readable image"))?;
    Ok(AttachmentImage { bytes, mime })
}

/// Manual check against a COPY of `rustsale.db` (never the live file); downloads for real:
/// `RUSTSALE_ATT_CHECK_DB=/tmp/copy/rustsale.db RUSTSALE_ATT_CHECK_ID=21478 \
///  RUSTSALE_ATT_CHECK_OUT=/tmp/att.jpg cargo test --lib read_attachment_on_db_copy -- --ignored --nocapture`
#[cfg(test)]
mod db_copy_check {
    use super::*;
    use crate::conversation::repository::ConversationRepository;

    #[tokio::test]
    #[ignore = "needs RUSTSALE_ATT_CHECK_DB pointing at a copy of rustsale.db and network"]
    async fn read_attachment_on_db_copy() {
        let (Ok(path), Ok(id), Ok(out)) = (
            std::env::var("RUSTSALE_ATT_CHECK_DB"),
            std::env::var("RUSTSALE_ATT_CHECK_ID"),
            std::env::var("RUSTSALE_ATT_CHECK_OUT"),
        ) else {
            panic!("set RUSTSALE_ATT_CHECK_DB, _ID and _OUT");
        };
        let live = std::env::var("HOME")
            .map(|home| format!("{home}/.local/share/com.rustsale.crm"))
            .unwrap_or_default();
        assert!(
            !path.starts_with(&live),
            "refusing to open the live database; pass a copy"
        );
        let id: i64 = id.trim().parse().expect("numeric attachment id");
        let source = ConversationRepository::new(path)
            .get_attachment_source(id)
            .expect("lookup")
            .expect("attachment exists");
        let image = image_from_source(&MediaFetcher::production(), &source)
            .await
            .map_err(|e| format!("{} {}", e.code, e.message))
            .expect("image");
        std::fs::write(&out, &image.bytes).expect("write output");
        println!("id={id} mime={} bytes={}", image.mime, image.bytes.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn private(host: &str) -> bool {
        let url = Url::parse(&format!("http://{host}/x.png")).expect("valid url");
        url.host().is_none_or(|h| is_private_host(&h))
    }

    #[test]
    fn loopback_private_and_link_local_hosts_are_blocked() {
        for host in [
            "127.0.0.1",
            "10.0.0.5",
            "192.168.1.1",
            "172.16.0.9",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "[::1]",
            "[fd00::1]",
            "[fe80::1]",
            "[::ffff:127.0.0.1]",
            "localhost",
            "api.localhost",
            "printer.local",
            "db.internal",
        ] {
            assert!(private(host), "{host} must be blocked");
        }
    }

    #[test]
    fn public_cdn_hosts_are_allowed() {
        for host in ["photo-stal-36.zdn.vn", "8.8.8.8", "[2606:4700::1111]"] {
            assert!(!private(host), "{host} must be allowed");
        }
    }

    #[test]
    fn non_image_bytes_are_not_images() {
        assert_eq!(image_mime(b"<html>expired</html>"), None);
        assert_eq!(image_mime(b"\x89PNG\r\n\x1a\n rest"), Some("image/png"));
    }

    #[test]
    fn whatsapp_encrypted_urls_are_not_plain_downloads() {
        assert!(!is_http_url("https://mmg.whatsapp.net/v/t62/x.enc"));
        assert!(is_http_url("https://photo-stal-36.zdn.vn/a.jpg"));
        assert!(!is_http_url("file:///etc/passwd"));
    }

    #[test]
    fn inline_thumbnail_over_the_limit_is_too_large() {
        let payload = base64::engine::general_purpose::STANDARD.encode(vec![1u8; 64]);
        let url = format!("data:image/png;base64,{payload}");
        let err = decode_data_url(&url, 16).expect_err("over limit");
        assert_eq!(err.code, "TOO_LARGE");
        assert!(decode_data_url("data:text/plain;base64,AAAA", 16)
            .expect("ignored")
            .is_none());
    }
}
