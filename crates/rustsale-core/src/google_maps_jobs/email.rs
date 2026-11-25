use crate::google_maps_jobs::types::{ScrapeErrorKind, ScrapeJobError, ScrapeResult};
use async_trait::async_trait;
use futures_util::StreamExt;
use percent_encoding::percent_decode_str;
use regex::Regex;
use reqwest::header::{ACCEPT, CONTENT_LENGTH, CONTENT_TYPE, LOCATION, USER_AGENT};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use url::Url;

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_MAX_REDIRECTS: usize = 3;
const DEFAULT_MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_HTML_PAGES: usize = 5;
/// Everything one website may cost: robots.txt, the homepage and its sub-pages
/// together. Per-request timeouts alone allowed over five minutes for a site
/// that answered slowly (A-U5).
const SITE_DEADLINE: Duration = Duration::from_secs(30);
const ENRICHMENT_USER_AGENT: &str = "RustSaleBot/1.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeHtmlResponse {
    pub final_url: Url,
    pub body: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PinnedHttpRequest {
    pub url: Url,
    pub address: SocketAddr,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub max_body_bytes: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct RawHttpResponse {
    status: u16,
    content_type: Option<String>,
    location: Option<String>,
    body: Vec<u8>,
}

#[cfg(test)]
impl RawHttpResponse {
    fn html(body: &str) -> Self {
        Self::with_content_type("text/html; charset=utf-8", body.as_bytes().to_vec())
    }

    fn html_bytes(body: Vec<u8>) -> Self {
        Self::with_content_type("text/html", body)
    }

    fn text(body: &str) -> Self {
        Self::with_content_type("text/plain; charset=utf-8", body.as_bytes().to_vec())
    }

    fn with_content_type(content_type: &str, body: Vec<u8>) -> Self {
        Self {
            status: 200,
            content_type: Some(content_type.to_string()),
            location: None,
            body,
        }
    }

    fn status(status: u16, content_type: &str) -> Self {
        Self {
            status,
            content_type: Some(content_type.to_string()),
            location: None,
            body: Vec::new(),
        }
    }

    fn redirect(location: &str) -> Self {
        Self {
            status: 302,
            content_type: None,
            location: Some(location.to_string()),
            body: Vec::new(),
        }
    }
}

#[async_trait]
pub(crate) trait DnsResolver: Send + Sync {
    async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, ScrapeJobError>;
}

#[async_trait]
pub(crate) trait HttpTransport: Send + Sync {
    async fn execute(&self, request: PinnedHttpRequest) -> Result<RawHttpResponse, ScrapeJobError>;
}

struct SystemDnsResolver;

#[async_trait]
impl DnsResolver for SystemDnsResolver {
    async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, ScrapeJobError> {
        let addresses = tokio::net::lookup_host((host, port))
            .await
            .map_err(|error| {
                enrichment_error(
                    ScrapeErrorKind::TransientNetwork,
                    format!("failed to resolve enrichment host: {error}"),
                    true,
                )
            })?;
        let mut unique = Vec::new();
        for address in addresses {
            if !unique.contains(&address) {
                unique.push(address);
            }
        }
        Ok(unique)
    }
}

struct ReqwestTransport;

#[async_trait]
impl HttpTransport for ReqwestTransport {
    async fn execute(&self, request: PinnedHttpRequest) -> Result<RawHttpResponse, ScrapeJobError> {
        let host = request.url.host_str().ok_or_else(|| {
            enrichment_error(
                ScrapeErrorKind::InvalidInput,
                "enrichment URL omitted hostname",
                false,
            )
        })?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(request.connect_timeout)
            .timeout(request.request_timeout)
            .resolve(host, request.address)
            .build()
            .map_err(map_reqwest_error)?;
        let response = client
            .get(request.url.clone())
            .header(USER_AGENT, ENRICHMENT_USER_AGENT)
            .header(ACCEPT, "text/html,text/plain;q=0.9")
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let location = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        if let Some(length) = response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<usize>().ok())
        {
            if length > request.max_body_bytes {
                return Err(enrichment_error(
                    ScrapeErrorKind::InvalidResponse,
                    "enrichment response exceeded the 2 MB body limit",
                    false,
                ));
            }
        }
        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(map_reqwest_error)?;
            if body.len().saturating_add(chunk.len()) > request.max_body_bytes {
                return Err(enrichment_error(
                    ScrapeErrorKind::InvalidResponse,
                    "enrichment response exceeded the 2 MB body limit",
                    false,
                ));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(RawHttpResponse {
            status,
            content_type,
            location,
            body,
        })
    }
}

#[derive(Clone)]
pub struct SafeHttpClient {
    connect_timeout: Duration,
    request_timeout: Duration,
    max_redirects: usize,
    max_body_bytes: usize,
    resolver: Arc<dyn DnsResolver>,
    transport: Arc<dyn HttpTransport>,
}

impl SafeHttpClient {
    pub fn new() -> Self {
        Self {
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            max_redirects: DEFAULT_MAX_REDIRECTS,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            resolver: Arc::new(SystemDnsResolver),
            transport: Arc::new(ReqwestTransport),
        }
    }

    #[cfg(test)]
    fn with_backend(resolver: Arc<dyn DnsResolver>, transport: Arc<dyn HttpTransport>) -> Self {
        Self {
            resolver,
            transport,
            ..Self::new()
        }
    }

    pub async fn fetch_html(&self, url: Url) -> Result<SafeHtmlResponse, ScrapeJobError> {
        let response = self.fetch_resource(url, ResourceKind::Html).await?;
        Ok(SafeHtmlResponse {
            final_url: response.final_url,
            body: String::from_utf8_lossy(&response.body).into_owned(),
        })
    }

    async fn fetch_robots(&self, homepage: &Url) -> Result<RobotsRules, ScrapeJobError> {
        let mut robots_url = homepage.clone();
        robots_url.set_path("/robots.txt");
        robots_url.set_query(None);
        robots_url.set_fragment(None);
        let response = self
            .fetch_resource(robots_url, ResourceKind::Robots)
            .await?;
        // RFC 9309: any 4xx on robots.txt means "no rules"; 5xx already
        // surfaced above as a retryable error (B5).
        if (400..500).contains(&response.status) {
            return Ok(RobotsRules::default());
        }
        Ok(RobotsRules::parse(&String::from_utf8_lossy(&response.body)))
    }

    async fn fetch_resource(
        &self,
        initial_url: Url,
        resource_kind: ResourceKind,
    ) -> Result<ValidatedResponse, ScrapeJobError> {
        validate_url(&initial_url)?;
        let initial_site = registrable_site(&initial_url)?;
        let mut current_url = initial_url;
        let mut redirects = 0usize;

        loop {
            validate_url(&current_url)?;
            if registrable_site(&current_url)? != initial_site {
                return Err(enrichment_error(
                    ScrapeErrorKind::InvalidInput,
                    "enrichment redirect left the original registrable domain",
                    false,
                ));
            }
            let address = self.resolve_and_pin(&current_url).await?;
            let response = self
                .transport
                .execute(PinnedHttpRequest {
                    url: current_url.clone(),
                    address,
                    connect_timeout: self.connect_timeout,
                    request_timeout: self.request_timeout,
                    max_body_bytes: self.max_body_bytes,
                })
                .await?;

            if is_redirect(response.status) {
                if redirects >= self.max_redirects {
                    return Err(enrichment_error(
                        ScrapeErrorKind::InvalidResponse,
                        "enrichment redirect limit exceeded",
                        false,
                    ));
                }
                let location = response.location.as_deref().ok_or_else(|| {
                    enrichment_error(
                        ScrapeErrorKind::InvalidResponse,
                        "enrichment redirect omitted Location",
                        false,
                    )
                })?;
                current_url = current_url.join(location).map_err(|error| {
                    enrichment_error(
                        ScrapeErrorKind::InvalidResponse,
                        format!("invalid enrichment redirect URL: {error}"),
                        false,
                    )
                })?;
                redirects += 1;
                continue;
            }

            if resource_kind == ResourceKind::Robots
                && (400..500).contains(&response.status)
                && response.status != 429
            {
                return Ok(ValidatedResponse {
                    final_url: current_url,
                    status: response.status,
                    body: Vec::new(),
                });
            }
            if !(200..300).contains(&response.status) {
                let retryable = response.status == 429 || response.status >= 500;
                return Err(enrichment_error(
                    if retryable {
                        ScrapeErrorKind::TransientNetwork
                    } else {
                        ScrapeErrorKind::InvalidResponse
                    },
                    format!("enrichment HTTP status {}", response.status),
                    retryable,
                ));
            }
            if resource_kind == ResourceKind::Html {
                validate_html_content_type(response.content_type.as_deref(), &response.body)?;
            }
            if response.body.len() > self.max_body_bytes {
                return Err(enrichment_error(
                    ScrapeErrorKind::InvalidResponse,
                    "enrichment response exceeded the 2 MB body limit",
                    false,
                ));
            }
            return Ok(ValidatedResponse {
                final_url: current_url,
                status: response.status,
                body: response.body,
            });
        }
    }

    async fn resolve_and_pin(&self, url: &Url) -> Result<SocketAddr, ScrapeJobError> {
        let host = url.host().ok_or_else(|| {
            enrichment_error(
                ScrapeErrorKind::InvalidInput,
                "enrichment URL omitted hostname",
                false,
            )
        })?;
        let port = url.port_or_known_default().ok_or_else(|| {
            enrichment_error(
                ScrapeErrorKind::InvalidInput,
                "enrichment URL omitted a supported port",
                false,
            )
        })?;
        let addresses = match host {
            url::Host::Ipv4(ip) => vec![SocketAddr::new(IpAddr::V4(ip), port)],
            url::Host::Ipv6(ip) => vec![SocketAddr::new(IpAddr::V6(ip), port)],
            url::Host::Domain(domain) => self.resolver.resolve(domain, port).await?,
        };
        if addresses.is_empty() {
            return Err(enrichment_error(
                ScrapeErrorKind::TransientNetwork,
                "enrichment DNS lookup returned no addresses",
                true,
            ));
        }
        if let Some(address) = addresses
            .iter()
            .find(|address| is_disallowed_destination(address.ip()))
        {
            return Err(enrichment_error(
                ScrapeErrorKind::InvalidInput,
                format!("enrichment destination is not public: {}", address.ip()),
                false,
            ));
        }
        Ok(addresses[0])
    }
}

impl Default for SafeHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResourceKind {
    Html,
    Robots,
}

struct ValidatedResponse {
    final_url: Url,
    status: u16,
    body: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
struct RobotsRules {
    rules: Vec<RobotsRule>,
}

#[derive(Debug, Clone)]
struct RobotsRule {
    allow: bool,
    path: String,
}

impl RobotsRules {
    fn parse(content: &str) -> Self {
        let mut applies = false;
        let mut rules = Vec::new();
        for raw_line in content.lines() {
            let line = raw_line.split('#').next().unwrap_or_default().trim();
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();
            match name.as_str() {
                "user-agent" => applies = value == "*",
                "allow" if applies && !value.is_empty() => rules.push(RobotsRule {
                    allow: true,
                    path: value.to_string(),
                }),
                "disallow" if applies && !value.is_empty() => rules.push(RobotsRule {
                    allow: false,
                    path: value.to_string(),
                }),
                _ => {}
            }
        }
        Self { rules }
    }

    fn allows(&self, url: &Url) -> bool {
        let path = url.path();
        let mut decision: Option<(usize, bool)> = None;
        for rule in &self.rules {
            if path.starts_with(&rule.path) {
                let candidate = (rule.path.len(), rule.allow);
                if decision.is_none_or(|current| {
                    candidate.0 > current.0
                        || (candidate.0 == current.0 && candidate.1 && !current.1)
                }) {
                    decision = Some(candidate);
                }
            }
        }
        decision.map(|(_, allow)| allow).unwrap_or(true)
    }
}

pub struct EmailEnricher {
    client: SafeHttpClient,
    site_deadline: Duration,
}

impl EmailEnricher {
    pub fn new(client: SafeHttpClient) -> Self {
        Self {
            client,
            site_deadline: SITE_DEADLINE,
        }
    }

    /// Looks for a contact email on one website, spending at most
    /// [`SITE_DEADLINE`] on it. A site that does not finish in time is a
    /// retryable network failure, not "no email found".
    pub async fn find_email(&self, homepage: Url) -> Result<Option<String>, ScrapeJobError> {
        let host = homepage.host_str().unwrap_or("website").to_string();
        match tokio::time::timeout(self.site_deadline, self.find_email_on_site(homepage)).await {
            Ok(outcome) => outcome,
            Err(_) => Err(enrichment_error(
                ScrapeErrorKind::TransientNetwork,
                format!(
                    "enrichment of {host} did not finish within {} s",
                    self.site_deadline.as_secs()
                ),
                true,
            )),
        }
    }

    async fn find_email_on_site(&self, homepage: Url) -> Result<Option<String>, ScrapeJobError> {
        let robots = self.client.fetch_robots(&homepage).await?;
        if !robots.allows(&homepage) {
            return Ok(None);
        }
        let homepage_response = self.client.fetch_html(homepage.clone()).await?;
        if let Some(email) = extract_primary_email(&homepage_response.body) {
            return Ok(Some(email));
        }
        let links = discover_relevant_links(
            &homepage_response.final_url,
            &homepage_response.body,
            MAX_HTML_PAGES - 1,
        );
        let mut retryable_failure = None;
        for link in links {
            if !robots.allows(&link) {
                continue;
            }
            // A dead or odd sub-page (404, wrong type) must not discard the
            // pages still to visit; only a retryable failure is reported, and
            // only when nothing was found (B4).
            match self.client.fetch_html(link).await {
                Ok(response) => {
                    if let Some(email) = extract_primary_email(&response.body) {
                        return Ok(Some(email));
                    }
                }
                Err(error) if error.retryable => retryable_failure = Some(error),
                Err(_) => {}
            }
        }
        match retryable_failure {
            Some(error) => Err(error),
            None => Ok(None),
        }
    }
}

impl Default for EmailEnricher {
    fn default() -> Self {
        Self::new(SafeHttpClient::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnrichmentStatus {
    Pending,
    Running,
    Found,
    NotFound,
    RetryWait,
    Failed,
}

impl EnrichmentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Found => "found",
            Self::NotFound => "not_found",
            Self::RetryWait => "retry_wait",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrichmentOutcome {
    pub status: EnrichmentStatus,
    pub email: Option<String>,
    pub error: Option<ScrapeJobError>,
}

impl EnrichmentOutcome {
    pub fn found(email: String) -> Self {
        Self {
            status: EnrichmentStatus::Found,
            email: Some(email),
            error: None,
        }
    }

    pub fn not_found() -> Self {
        Self {
            status: EnrichmentStatus::NotFound,
            email: None,
            error: None,
        }
    }

    pub fn from_error(error: ScrapeJobError) -> Self {
        Self {
            status: if error.retryable {
                EnrichmentStatus::RetryWait
            } else {
                EnrichmentStatus::Failed
            },
            email: None,
            error: Some(error),
        }
    }

    pub fn apply_to_result(&self, result: &mut ScrapeResult) {
        result.enrichment_status = self.status.as_str().to_string();
        result.email = self.email.clone();
        result.normalized_email = self.email.clone();
        result.last_error_kind = self.error.as_ref().map(|error| error.kind);
        result.last_error_message = self.error.as_ref().map(|error| error.message.clone());
    }
}

pub fn extract_primary_email(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let anchor_selector = Selector::parse("a[href]").expect("valid anchor selector");
    for anchor in document.select(&anchor_selector) {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        if !href
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("mailto:"))
        {
            continue;
        }
        let raw = href[7..].split(['?', ',', ';']).next().unwrap_or_default();
        let decoded = percent_decode_str(raw).decode_utf8_lossy();
        if let Some(email) = normalize_email(&decoded) {
            return Some(email);
        }
    }

    let visible_html = strip_non_visible_sections(html);
    let visible_document = Html::parse_document(&visible_html);
    let text = visible_document
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ");
    for candidate in email_regex().find_iter(&text) {
        if let Some(email) = normalize_email(candidate.as_str()) {
            return Some(email);
        }
    }
    None
}

pub fn normalize_email(value: &str) -> Option<String> {
    let value = value
        .trim()
        .trim_matches(|character: char| "<>()[]{}\"',;:".contains(character))
        .to_ascii_lowercase();
    if value.len() > 254 || !value.is_ascii() || value.matches('@').count() != 1 {
        return None;
    }
    let (local, domain) = value.split_once('@')?;
    if local.is_empty()
        || local.len() > 64
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
    {
        return None;
    }
    if !local
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b".!#$%&'*+-=?^_`{|}~".contains(&byte))
    {
        return None;
    }
    let labels = domain.split('.').collect::<Vec<_>>();
    if labels.len() < 2
        || labels.iter().any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return None;
    }
    let tld = labels.last()?;
    if tld.len() < 2 || !tld.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return None;
    }
    if ASSET_TLDS.contains(tld) || is_placeholder_or_tracking_domain(domain) {
        return None;
    }
    if is_automated_local_part(local) {
        return None;
    }
    Some(value)
}

/// File extensions that look like a TLD in names such as `logo@2x.png` (B2).
const ASSET_TLDS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "ico", "bmp", "css", "js", "map", "json",
    "xml", "woff", "woff2", "ttf", "eot",
];

/// Placeholder, documentation and error-tracking domains never reach a person.
const BLOCKED_EMAIL_DOMAINS: &[&str] = &[
    "example.com",
    "example.org",
    "example.net",
    "domain.com",
    "email.com",
    "yourdomain.com",
    "yourcompany.com",
    "test.com",
    "sentry.io",
    "wixpress.com",
];

fn is_placeholder_or_tracking_domain(domain: &str) -> bool {
    BLOCKED_EMAIL_DOMAINS
        .iter()
        .any(|blocked| domain == *blocked || domain.ends_with(&format!(".{blocked}")))
}

/// No-reply mailboxes and long hex hashes (error-tracker ids) are not contacts.
fn is_automated_local_part(local: &str) -> bool {
    let compact = local.replace(['-', '_', '.'], "");
    if ["noreply", "donotreply", "mailerdaemon"]
        .iter()
        .any(|prefix| compact.starts_with(prefix))
    {
        return true;
    }
    local.len() >= 20 && local.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn is_disallowed_destination(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_disallowed_ipv4(address),
        IpAddr::V6(address) => is_disallowed_ipv6(address),
    }
}

fn is_disallowed_ipv4(address: Ipv4Addr) -> bool {
    let [a, b, c, d] = address.octets();
    address.is_unspecified()
        || address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_multicast()
        || address.is_broadcast()
        || a == 0
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 240
        || (a == 169 && b == 254 && c == 169 && d == 254)
}

fn is_disallowed_ipv6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return is_disallowed_ipv4(mapped);
    }
    // NAT64 and 6to4 addresses embed an IPv4 destination. Checking only the
    // IPv6 wrapper let them tunnel a private or loopback target (B13).
    if let Some(embedded) = embedded_ipv4(address) {
        return is_disallowed_ipv4(embedded);
    }
    let segments = address.segments();
    address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] & 0xffc0) == 0xfec0
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
}

/// IPv4 address carried inside a NAT64 (64:ff9b::/96) or 6to4 (2002::/16)
/// wrapper, if the wrapper is one of those well-known formats.
fn embedded_ipv4(address: Ipv6Addr) -> Option<Ipv4Addr> {
    let segments = address.segments();
    if segments[0] == 0x0064
        && segments[1] == 0xff9b
        && segments[2..6].iter().all(|segment| *segment == 0)
    {
        return Some(Ipv4Addr::new(
            (segments[6] >> 8) as u8,
            segments[6] as u8,
            (segments[7] >> 8) as u8,
            segments[7] as u8,
        ));
    }
    if segments[0] == 0x2002 {
        return Some(Ipv4Addr::new(
            (segments[1] >> 8) as u8,
            segments[1] as u8,
            (segments[2] >> 8) as u8,
            segments[2] as u8,
        ));
    }
    None
}

fn validate_url(url: &Url) -> Result<(), ScrapeJobError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(enrichment_error(
            ScrapeErrorKind::InvalidInput,
            "enrichment URL must use HTTP or HTTPS",
            false,
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(enrichment_error(
            ScrapeErrorKind::InvalidInput,
            "enrichment URL must not contain credentials",
            false,
        ));
    }
    if url.host_str().is_none() {
        return Err(enrichment_error(
            ScrapeErrorKind::InvalidInput,
            "enrichment URL omitted hostname",
            false,
        ));
    }
    Ok(())
}

fn registrable_site(url: &Url) -> Result<String, ScrapeJobError> {
    let host = url.host_str().ok_or_else(|| {
        enrichment_error(
            ScrapeErrorKind::InvalidInput,
            "enrichment URL omitted hostname",
            false,
        )
    })?;
    if host.parse::<IpAddr>().is_ok() {
        return Ok(host.to_ascii_lowercase());
    }
    Ok(psl::domain_str(host)
        .unwrap_or(host)
        .trim_end_matches('.')
        .to_ascii_lowercase())
}

/// Real sites often serve HTML as `text/plain`, `application/octet-stream` or
/// with no type at all; accept those when the body itself looks like HTML (B5).
fn validate_html_content_type(
    content_type: Option<&str>,
    body: &[u8],
) -> Result<(), ScrapeJobError> {
    let media_type = content_type
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .map(str::to_ascii_lowercase);
    let sniffable = match media_type.as_deref() {
        None | Some("") | Some("text/plain") | Some("application/octet-stream") => true,
        Some(_) => false,
    };
    if sniffable && body_looks_like_html(body) {
        return Ok(());
    }
    validate_content_type(content_type, ResourceKind::Html)
}

fn body_looks_like_html(body: &[u8]) -> bool {
    let head = &body[..body.len().min(1024)];
    let head = String::from_utf8_lossy(head).to_ascii_lowercase();
    let head = head.trim_start();
    head.starts_with("<!doctype html") || head.starts_with("<html") || head.contains("<body")
}

fn validate_content_type(
    content_type: Option<&str>,
    resource_kind: ResourceKind,
) -> Result<(), ScrapeJobError> {
    let media_type = content_type
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| {
            enrichment_error(
                ScrapeErrorKind::InvalidResponse,
                "enrichment response omitted Content-Type",
                false,
            )
        })?;
    let allowed = match resource_kind {
        ResourceKind::Html => matches!(media_type.as_str(), "text/html" | "application/xhtml+xml"),
        ResourceKind::Robots => matches!(media_type.as_str(), "text/plain" | "text/html"),
    };
    if !allowed {
        return Err(enrichment_error(
            ScrapeErrorKind::InvalidResponse,
            format!("enrichment rejected Content-Type {media_type}"),
            false,
        ));
    }
    Ok(())
}

fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

fn discover_relevant_links(base_url: &Url, html: &str, limit: usize) -> Vec<Url> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("a[href]").expect("valid anchor selector");
    let base_site = registrable_site(base_url).ok();
    let mut ranked = Vec::new();
    let mut seen = HashSet::new();
    for (index, anchor) in document.select(&selector).enumerate() {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        let Ok(mut link) = base_url.join(href) else {
            continue;
        };
        link.set_fragment(None);
        if validate_url(&link).is_err()
            || base_site.as_ref().is_none_or(|site| {
                registrable_site(&link)
                    .map(|candidate| candidate != *site)
                    .unwrap_or(true)
            })
            || !seen.insert(link.as_str().to_string())
        {
            continue;
        }
        let anchor_text = anchor.text().collect::<Vec<_>>().join(" ");
        let haystack = format!("{} {}", link.path(), anchor_text).to_ascii_lowercase();
        let rank = if contains_any(
            &haystack,
            &["contact", "lien-he", "lien_he", "liên hệ", "get-in-touch"],
        ) {
            0
        } else if contains_any(
            &haystack,
            &["about", "gioi-thieu", "gioi_thieu", "giới thiệu"],
        ) {
            1
        } else if contains_any(
            &haystack,
            &["support", "team", "company", "help", "customer-service"],
        ) {
            2
        } else {
            continue;
        };
        ranked.push((rank, index, link));
    }
    ranked.sort_by_key(|(rank, index, _)| (*rank, *index));
    ranked
        .into_iter()
        .take(limit)
        .map(|(_, _, link)| link)
        .collect()
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn strip_non_visible_sections(html: &str) -> String {
    let mut visible = html.to_string();
    for regex in non_visible_regexes() {
        visible = regex.replace_all(&visible, " ").into_owned();
    }
    visible
}

fn non_visible_regexes() -> &'static [Regex; 4] {
    static REGEXES: OnceLock<[Regex; 4]> = OnceLock::new();
    REGEXES.get_or_init(|| {
        [
            Regex::new(r"(?is)<script\b[^>]*>.*?</script\s*>").expect("valid script regex"),
            Regex::new(r"(?is)<style\b[^>]*>.*?</style\s*>").expect("valid style regex"),
            Regex::new(r"(?is)<noscript\b[^>]*>.*?</noscript\s*>").expect("valid noscript regex"),
            Regex::new(r"(?is)<template\b[^>]*>.*?</template\s*>").expect("valid template regex"),
        ]
    })
}

fn email_regex() -> &'static Regex {
    static EMAIL_REGEX: OnceLock<Regex> = OnceLock::new();
    EMAIL_REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)[a-z0-9.!#$%&'*+=?^_`{|}~-]+@[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+",
        )
        .expect("valid email regex")
    })
}

fn enrichment_error(
    kind: ScrapeErrorKind,
    message: impl Into<String>,
    retryable: bool,
) -> ScrapeJobError {
    ScrapeJobError {
        kind,
        message: message.into(),
        retryable,
    }
}

fn map_reqwest_error(error: reqwest::Error) -> ScrapeJobError {
    enrichment_error(
        if error.is_timeout() || error.is_connect() || error.is_request() {
            ScrapeErrorKind::TransientNetwork
        } else {
            ScrapeErrorKind::InvalidResponse
        },
        format!("enrichment HTTP request failed: {error}"),
        error.is_timeout() || error.is_connect() || error.is_request(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::{HashMap, VecDeque};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use url::Url;

    struct FakeResolver {
        answers: Mutex<HashMap<String, Vec<IpAddr>>>,
    }

    impl FakeResolver {
        fn public() -> Self {
            Self {
                answers: Mutex::new(HashMap::new()),
            }
        }

        fn with_answer(self, host: &str, addresses: Vec<IpAddr>) -> Self {
            self.answers
                .lock()
                .unwrap()
                .insert(host.to_string(), addresses);
            self
        }
    }

    #[async_trait]
    impl DnsResolver for FakeResolver {
        async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, ScrapeJobError> {
            let addresses = self
                .answers
                .lock()
                .unwrap()
                .get(host)
                .cloned()
                .unwrap_or_else(|| vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))]);
            Ok(addresses
                .into_iter()
                .map(|address| SocketAddr::new(address, port))
                .collect())
        }
    }

    struct FakeTransport {
        responses: Mutex<HashMap<String, VecDeque<RawHttpResponse>>>,
        requests: Mutex<Vec<PinnedHttpRequest>>,
    }

    impl FakeTransport {
        fn new() -> Self {
            Self {
                responses: Mutex::new(HashMap::new()),
                requests: Mutex::new(Vec::new()),
            }
        }

        fn respond(&self, url: &str, response: RawHttpResponse) {
            self.responses
                .lock()
                .unwrap()
                .entry(url.to_string())
                .or_default()
                .push_back(response);
        }

        fn requested_urls(&self) -> Vec<String> {
            self.requests
                .lock()
                .unwrap()
                .iter()
                .map(|request| request.url.to_string())
                .collect()
        }
    }

    #[async_trait]
    impl HttpTransport for FakeTransport {
        async fn execute(
            &self,
            request: PinnedHttpRequest,
        ) -> Result<RawHttpResponse, ScrapeJobError> {
            self.requests.lock().unwrap().push(request.clone());
            self.responses
                .lock()
                .unwrap()
                .get_mut(request.url.as_str())
                .and_then(VecDeque::pop_front)
                .ok_or_else(|| ScrapeJobError {
                    kind: ScrapeErrorKind::InvalidResponse,
                    message: format!("missing fake response for {}", request.url),
                    retryable: false,
                })
        }
    }

    fn client(resolver: Arc<dyn DnsResolver>, transport: Arc<dyn HttpTransport>) -> SafeHttpClient {
        SafeHttpClient::with_backend(resolver, transport)
    }

    fn url(value: &str) -> Url {
        Url::parse(value).unwrap()
    }

    #[test]
    fn rejects_loopback_private_link_local_multicast_and_metadata_ips() {
        for address in [
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)),
            IpAddr::V4(Ipv4Addr::new(224, 0, 0, 1)),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            "fc00::1".parse().unwrap(),
            "fe80::1".parse().unwrap(),
            "ff02::1".parse().unwrap(),
        ] {
            assert!(is_disallowed_destination(address), "accepted {address}");
        }
        assert!(!is_disallowed_destination(IpAddr::V4(Ipv4Addr::new(
            93, 184, 216, 34
        ))));
    }

    #[test]
    fn blocks_nat64_and_6to4_embedding_private_v4() {
        for address in [
            "64:ff9b::7f00:1",
            "64:ff9b::c0a8:101",
            "2002:c0a8:0101::",
            "2002:7f00:0001::",
        ] {
            assert!(
                is_disallowed_destination(address.parse().unwrap()),
                "accepted {address}"
            );
        }
        for address in ["64:ff9b::5db8:d822", "2002:5db8:d822::"] {
            assert!(
                !is_disallowed_destination(address.parse().unwrap()),
                "rejected public address {address}"
            );
        }
    }

    #[tokio::test]
    async fn rejects_private_redirect_after_public_start() {
        let resolver = Arc::new(FakeResolver::public().with_answer(
            "internal.public.test",
            vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 8))],
        ));
        let transport = Arc::new(FakeTransport::new());
        transport.respond(
            "https://public.test/",
            RawHttpResponse::redirect("https://internal.public.test/contact"),
        );
        let client = client(resolver, transport.clone());

        let error = client
            .fetch_html(url("https://public.test/"))
            .await
            .unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::InvalidInput);
        assert_eq!(transport.requested_urls(), vec!["https://public.test/"]);
    }

    #[tokio::test]
    async fn rejects_the_fourth_redirect() {
        let resolver = Arc::new(FakeResolver::public());
        let transport = Arc::new(FakeTransport::new());
        transport.respond("https://public.test/", RawHttpResponse::redirect("/one"));
        transport.respond("https://public.test/one", RawHttpResponse::redirect("/two"));
        transport.respond(
            "https://public.test/two",
            RawHttpResponse::redirect("/three"),
        );
        transport.respond(
            "https://public.test/three",
            RawHttpResponse::redirect("/four"),
        );
        let client = client(resolver, transport.clone());

        assert!(client
            .fetch_html(url("https://public.test/"))
            .await
            .is_err());
        assert_eq!(transport.requested_urls().len(), 4);
    }

    #[tokio::test]
    async fn rejects_ipv6_loopback_literal_as_non_retryable_invalid_input_without_dns() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingFailingResolver {
            calls: AtomicUsize,
        }

        #[async_trait]
        impl DnsResolver for CountingFailingResolver {
            async fn resolve(
                &self,
                host: &str,
                _port: u16,
            ) -> Result<Vec<SocketAddr>, ScrapeJobError> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                Err(enrichment_error(
                    ScrapeErrorKind::TransientNetwork,
                    format!("unexpected DNS lookup for {host}"),
                    true,
                ))
            }
        }

        let resolver = Arc::new(CountingFailingResolver {
            calls: AtomicUsize::new(0),
        });
        let transport = Arc::new(FakeTransport::new());
        let client = client(resolver.clone(), transport.clone());

        for disallowed in [
            "http://[::1]/",
            "http://[fc00::1]/",
            "http://[::ffff:127.0.0.1]/",
        ] {
            let error = client.fetch_html(url(disallowed)).await.unwrap_err();
            assert_eq!(
                error.kind,
                ScrapeErrorKind::InvalidInput,
                "{disallowed} -> {error:?}"
            );
            assert!(!error.retryable, "{disallowed} -> {error:?}");
        }
        assert_eq!(
            resolver.calls.load(Ordering::SeqCst),
            0,
            "IPv6 literals must be parsed from url::Host without invoking DNS"
        );
        assert!(transport.requested_urls().is_empty());

        // Also verify default `SystemDnsResolver` path returns non-retryable InvalidInput.
        let default_error = SafeHttpClient::new()
            .fetch_html(url("http://[::1]/"))
            .await
            .unwrap_err();
        assert_eq!(default_error.kind, ScrapeErrorKind::InvalidInput);
        assert!(!default_error.retryable);
    }

    #[tokio::test]
    async fn rejects_non_http_and_credentialed_urls_before_transport() {
        let resolver = Arc::new(FakeResolver::public());
        let transport = Arc::new(FakeTransport::new());
        let client = client(resolver, transport.clone());

        assert!(client.fetch_html(url("file:///etc/passwd")).await.is_err());
        assert!(client
            .fetch_html(url("https://user:secret@public.test/"))
            .await
            .is_err());
        assert!(transport.requested_urls().is_empty());
    }

    #[tokio::test]
    async fn pins_the_validated_dns_address_for_the_request() {
        let resolver = Arc::new(FakeResolver::public().with_answer(
            "public.test",
            vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))],
        ));
        let transport = Arc::new(FakeTransport::new());
        transport.respond(
            "https://public.test/",
            RawHttpResponse::html("<html></html>"),
        );
        let client = client(resolver, transport.clone());

        client
            .fetch_html(url("https://public.test/"))
            .await
            .unwrap();

        assert_eq!(
            transport.requests.lock().unwrap()[0].address.ip(),
            IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))
        );
    }

    #[tokio::test]
    async fn reqwest_transport_honors_the_validated_pinned_address() {
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0u8; 4096];
            let read = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.to_ascii_lowercase().contains("host: public.test"));
            let body = "<html><body>sales@company.test</body></html>";
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });
        let transport = ReqwestTransport;

        let response = transport
            .execute(PinnedHttpRequest {
                url: url(&format!("http://public.test:{}/", address.port())),
                address,
                connect_timeout: Duration::from_secs(1),
                request_timeout: Duration::from_secs(2),
                max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            })
            .await
            .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.content_type.as_deref(), Some("text/html"));
        assert!(String::from_utf8(response.body)
            .unwrap()
            .contains("sales@company.test"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn stays_on_same_registrable_domain() {
        let resolver = Arc::new(FakeResolver::public());
        let transport = Arc::new(FakeTransport::new());
        transport.respond(
            "https://www.company.co.uk/",
            RawHttpResponse::redirect("https://attacker.co.uk/contact"),
        );
        let client = client(resolver, transport.clone());

        let error = client
            .fetch_html(url("https://www.company.co.uk/"))
            .await
            .unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::InvalidInput);
        assert_eq!(transport.requested_urls().len(), 1);
    }

    #[tokio::test]
    async fn stops_after_five_html_pages() {
        let resolver = Arc::new(FakeResolver::public());
        let transport = Arc::new(FakeTransport::new());
        transport.respond(
            "https://company.test/robots.txt",
            RawHttpResponse::text("User-agent: *\nDisallow:"),
        );
        let links = (1..=6)
            .map(|index| format!(r#"<a href="/contact-{index}">Contact {index}</a>"#))
            .collect::<String>();
        transport.respond(
            "https://company.test/",
            RawHttpResponse::html(&format!("<html><body>{links}</body></html>")),
        );
        for index in 1..=6 {
            transport.respond(
                &format!("https://company.test/contact-{index}"),
                RawHttpResponse::html("<html><body>No email</body></html>"),
            );
        }
        let enricher = EmailEnricher::new(client(resolver, transport.clone()));

        assert_eq!(
            enricher
                .find_email(url("https://company.test/"))
                .await
                .unwrap(),
            None
        );

        let html_requests = transport
            .requested_urls()
            .into_iter()
            .filter(|requested| !requested.ends_with("/robots.txt"))
            .count();
        assert_eq!(html_requests, 5);
    }

    #[tokio::test]
    async fn rejects_non_html_and_body_over_two_megabytes() {
        let resolver = Arc::new(FakeResolver::public());
        let transport = Arc::new(FakeTransport::new());
        transport.respond(
            "https://company.test/file.pdf",
            RawHttpResponse::with_content_type("application/pdf", b"pdf".to_vec()),
        );
        transport.respond(
            "https://company.test/large",
            RawHttpResponse::html_bytes(vec![b'a'; 2 * 1024 * 1024 + 1]),
        );
        let client = client(resolver, transport);

        assert!(client
            .fetch_html(url("https://company.test/file.pdf"))
            .await
            .is_err());
        assert!(client
            .fetch_html(url("https://company.test/large"))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn respects_robots_disallow() {
        let resolver = Arc::new(FakeResolver::public());
        let transport = Arc::new(FakeTransport::new());
        transport.respond(
            "https://company.test/robots.txt",
            RawHttpResponse::text("User-agent: *\nDisallow: /contact"),
        );
        transport.respond(
            "https://company.test/",
            RawHttpResponse::html(r#"<html><body><a href="/contact">Contact</a></body></html>"#),
        );
        transport.respond(
            "https://company.test/contact",
            RawHttpResponse::html(
                r#"<html><body><a href="mailto:sales@company.test">Email</a></body></html>"#,
            ),
        );
        let enricher = EmailEnricher::new(client(resolver, transport.clone()));

        assert_eq!(
            enricher
                .find_email(url("https://company.test/"))
                .await
                .unwrap(),
            None
        );
        assert!(!transport
            .requested_urls()
            .contains(&"https://company.test/contact".to_string()));
    }

    #[test]
    fn extracts_first_mailto_before_visible_text() {
        let html = r#"
            <html><body>
                Visible first: visible@company.test
                <a href="mailto:mailto@company.test?subject=Hello">Email us</a>
                <a href="mailto:second@company.test">Second</a>
            </body></html>
        "#;

        assert_eq!(
            extract_primary_email(html).as_deref(),
            Some("mailto@company.test")
        );
    }

    #[test]
    fn extracts_first_visible_email_when_no_mailto_exists() {
        let html = "<html><body>First@Company.Test then second@company.test</body></html>";

        assert_eq!(
            extract_primary_email(html).as_deref(),
            Some("first@company.test")
        );
    }

    #[test]
    fn filters_example_and_malformed_addresses() {
        for candidate in [
            "test@example.com",
            "test@example.org",
            "missing-at.company.test",
            ".leading@company.test",
            "double..dot@company.test",
            "name@localhost",
            "name@-company.test",
        ] {
            assert_eq!(normalize_email(candidate), None, "accepted {candidate}");
        }
        assert_eq!(
            normalize_email(" Sales@Company.Test ").as_deref(),
            Some("sales@company.test")
        );
    }

    /// B2: asset file names, tracking/placeholder domains and no-reply
    /// addresses are not contact emails; a plain contact address still is.
    /// B4: a dead sub-page must not discard the remaining pages.
    #[tokio::test]
    async fn find_email_continues_after_dead_subpage() {
        let resolver = Arc::new(FakeResolver::public());
        let transport = Arc::new(FakeTransport::new());
        transport.respond(
            "https://company.test/robots.txt",
            RawHttpResponse::status(404, "text/plain"),
        );
        transport.respond(
            "https://company.test/",
            RawHttpResponse::html(
                r#"<html><body><a href="/lien-he">Liên hệ</a><a href="/about">About</a></body></html>"#,
            ),
        );
        transport.respond(
            "https://company.test/lien-he",
            RawHttpResponse::status(404, "text/html"),
        );
        transport.respond(
            "https://company.test/about",
            RawHttpResponse::html(
                r#"<html><body><a href="mailto:sales@company.test">Email</a></body></html>"#,
            ),
        );
        let enricher = EmailEnricher::new(client(resolver, transport));

        assert_eq!(
            enricher
                .find_email(url("https://company.test/"))
                .await
                .unwrap()
                .as_deref(),
            Some("sales@company.test")
        );
    }

    /// B4: a retryable sub-page failure still surfaces when nothing is found.
    #[tokio::test]
    async fn find_email_reports_retryable_subpage_failure_when_nothing_found() {
        let resolver = Arc::new(FakeResolver::public());
        let transport = Arc::new(FakeTransport::new());
        transport.respond(
            "https://company.test/robots.txt",
            RawHttpResponse::status(404, "text/plain"),
        );
        transport.respond(
            "https://company.test/",
            RawHttpResponse::html(r#"<html><body><a href="/contact">Contact</a></body></html>"#),
        );
        transport.respond(
            "https://company.test/contact",
            RawHttpResponse::status(503, "text/html"),
        );
        let enricher = EmailEnricher::new(client(resolver, transport));

        let error = enricher
            .find_email(url("https://company.test/"))
            .await
            .unwrap_err();
        assert!(error.retryable);
    }

    /// B5: robots.txt answering 401/403 means "no rules" (RFC 9309), not an
    /// enrichment failure.
    #[tokio::test]
    async fn robots_403_is_treated_as_allow_all() {
        let resolver = Arc::new(FakeResolver::public());
        let transport = Arc::new(FakeTransport::new());
        transport.respond(
            "https://company.test/robots.txt",
            RawHttpResponse::status(403, "text/html"),
        );
        transport.respond(
            "https://company.test/",
            RawHttpResponse::html(
                r#"<html><body><a href="mailto:sales@company.test">Email</a></body></html>"#,
            ),
        );
        let enricher = EmailEnricher::new(client(resolver, transport));

        assert_eq!(
            enricher
                .find_email(url("https://company.test/"))
                .await
                .unwrap()
                .as_deref(),
            Some("sales@company.test")
        );
    }

    /// B5: a site serving its HTML as text/plain or octet-stream is still read.
    #[tokio::test]
    async fn html_served_with_wrong_content_type_is_still_read() {
        let resolver = Arc::new(FakeResolver::public());
        let transport = Arc::new(FakeTransport::new());
        transport.respond(
            "https://company.test/robots.txt",
            RawHttpResponse::status(404, "text/plain"),
        );
        transport.respond(
            "https://company.test/",
            RawHttpResponse::with_content_type(
                "application/octet-stream",
                br#"<!DOCTYPE html><html><body><a href="mailto:sales@company.test">Email</a></body></html>"#
                    .to_vec(),
            ),
        );
        let enricher = EmailEnricher::new(client(resolver, transport));

        assert_eq!(
            enricher
                .find_email(url("https://company.test/"))
                .await
                .unwrap()
                .as_deref(),
            Some("sales@company.test")
        );
    }

    #[test]
    fn email_extraction_rejects_asset_names_and_placeholder_domains() {
        for candidate in [
            "logo@2x.png",
            "banner@3x.webp",
            "a/b@site.vn",
            "x@sentry.io",
            "abc@o123.ingest.sentry.io",
            "name@domain.com",
            "name@email.com",
            "name@yourdomain.com",
            "name@test.com",
            "noreply@congty.vn",
            "no-reply@congty.vn",
            "donotreply@congty.vn",
            "0123456789abcdef0123456789abcdef@sentry.wixpress.com",
            "0123456789abcdef0123456789abcdef@congty.vn",
        ] {
            assert_eq!(normalize_email(candidate), None, "accepted {candidate}");
        }
        assert_eq!(
            normalize_email("lienhe@congty.vn").as_deref(),
            Some("lienhe@congty.vn")
        );
        let html = "<html><body>Ảnh logo@2x.png rồi liên hệ lienhe@congty.vn</body></html>";
        assert_eq!(
            extract_primary_email(html).as_deref(),
            Some("lienhe@congty.vn")
        );
        let path_html =
            "<html><body>Xem https://shop.example/contact/info@shop.example</body></html>";
        assert_eq!(
            extract_primary_email(path_html).as_deref(),
            Some("info@shop.example")
        );
    }

    #[test]
    fn maps_enrichment_outcomes_to_retryable_statuses() {
        assert_eq!(
            EnrichmentOutcome::found("sales@company.test".to_string()).status,
            EnrichmentStatus::Found
        );
        assert_eq!(
            EnrichmentOutcome::not_found().status,
            EnrichmentStatus::NotFound
        );
        assert_eq!(
            EnrichmentOutcome::from_error(ScrapeJobError {
                kind: ScrapeErrorKind::TransientNetwork,
                message: "reset".to_string(),
                retryable: true,
            })
            .status,
            EnrichmentStatus::RetryWait
        );
        assert_eq!(
            EnrichmentOutcome::from_error(ScrapeJobError {
                kind: ScrapeErrorKind::InvalidInput,
                message: "blocked destination".to_string(),
                retryable: false,
            })
            .status,
            EnrichmentStatus::Failed
        );
    }
}

/// A-U5: the time one website may take is bounded, measured in virtual time.
#[cfg(test)]
mod site_deadline {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct PublicDns;
    #[async_trait]
    impl DnsResolver for PublicDns {
        async fn resolve(&self, _host: &str, port: u16) -> Result<Vec<SocketAddr>, ScrapeJobError> {
            Ok(vec![SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)),
                port,
            )])
        }
    }

    /// Every request takes `delay`; `redirects` extra hops precede every HTML page except robots.
    struct SlowSite {
        delay: Duration,
        redirects: usize,
        seen: Mutex<HashMap<String, usize>>,
        count: Mutex<usize>,
    }
    #[async_trait]
    impl HttpTransport for SlowSite {
        async fn execute(
            &self,
            request: PinnedHttpRequest,
        ) -> Result<RawHttpResponse, ScrapeJobError> {
            *self.count.lock().unwrap() += 1;
            tokio::time::sleep(self.delay.min(request.request_timeout)).await;
            let path = request.url.path().to_string();
            if path == "/robots.txt" {
                return Ok(RawHttpResponse {
                    status: 404,
                    content_type: Some("text/plain".into()),
                    location: None,
                    body: vec![],
                });
            }
            let hop = {
                let mut seen = self.seen.lock().unwrap();
                let n = seen.entry(path.clone()).or_insert(0);
                *n += 1;
                *n
            };
            if hop <= self.redirects {
                return Ok(RawHttpResponse::redirect(&format!("{path}?h={hop}")));
            }
            // After the redirects the same path (with query) serves the page.
            let _ = &request.url;
            let page = if path == "/" {
                "<html><body><a href='/contact'>Contact</a><a href='/lien-he'>Liên hệ</a><a href='/about'>About us</a><a href='/gioi-thieu'>Giới thiệu</a></body></html>"
            } else {
                "<html><body>nothing here</body></html>"
            };
            Ok(RawHttpResponse::html(page))
        }
    }

    async fn run(
        delay_s: u64,
        redirects: usize,
    ) -> (u64, usize, Result<Option<String>, ScrapeJobError>) {
        let site = Arc::new(SlowSite {
            delay: Duration::from_secs(delay_s),
            redirects,
            seen: Mutex::new(HashMap::new()),
            count: Mutex::new(0),
        });
        let client = SafeHttpClient::with_backend(Arc::new(PublicDns), site.clone());
        let enricher = EmailEnricher::new(client);
        let started = tokio::time::Instant::now();
        let outcome = enricher
            .find_email(Url::parse("https://slow.example.test/").unwrap())
            .await;
        let elapsed = started.elapsed().as_secs();
        let requests = *site.count.lock().unwrap();
        (elapsed, requests, outcome)
    }

    #[tokio::test(start_paused = true)]
    async fn a_healthy_site_is_not_affected_by_the_deadline() {
        let (elapsed, requests, outcome) = run(1, 0).await;
        assert_eq!(outcome.unwrap(), None);
        assert_eq!(requests, 6, "robots.txt, the homepage and four sub-pages");
        assert!(elapsed < 10, "took {elapsed}s");
    }

    #[tokio::test(start_paused = true)]
    async fn a_slow_site_is_cut_off_at_the_per_website_deadline() {
        // Without a deadline the three slow scenarios took 154 s and 315 s.
        for (delay_s, redirects) in [(14, 0), (14, 1), (15, 3)] {
            let (elapsed, _, outcome) = run(delay_s, redirects).await;
            assert!(
                elapsed <= 30,
                "{delay_s}s per request with {redirects} redirects took {elapsed}s"
            );
            let error = outcome.expect_err("a site that cannot finish in time is not 'no email'");
            assert_eq!(error.kind, ScrapeErrorKind::TransientNetwork);
            assert!(error.retryable, "a later run may find the site faster");
            assert!(error.message.contains("30 s"), "{}", error.message);
        }
    }
}

/// Helpers that let other modules' tests drive an enricher against slow,
/// fake websites without touching the network.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Counts how many requests are in flight at the same moment.
    #[derive(Default)]
    pub(crate) struct ConcurrencyProbe {
        in_flight: AtomicUsize,
        peak: AtomicUsize,
    }

    impl ConcurrencyProbe {
        pub(crate) fn peak(&self) -> usize {
            self.peak.load(Ordering::SeqCst)
        }
    }

    struct PublicDns;
    #[async_trait]
    impl DnsResolver for PublicDns {
        async fn resolve(&self, _host: &str, port: u16) -> Result<Vec<SocketAddr>, ScrapeJobError> {
            Ok(vec![SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)),
                port,
            )])
        }
    }

    /// Every request takes `delay`; robots.txt is missing and the homepage has no links,
    /// so one website costs exactly two requests.
    struct SlowSites {
        delay: Duration,
        probe: Arc<ConcurrencyProbe>,
    }
    #[async_trait]
    impl HttpTransport for SlowSites {
        async fn execute(
            &self,
            request: PinnedHttpRequest,
        ) -> Result<RawHttpResponse, ScrapeJobError> {
            let now = self.probe.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.probe.peak.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(self.delay).await;
            self.probe.in_flight.fetch_sub(1, Ordering::SeqCst);
            if request.url.path() == "/robots.txt" {
                return Ok(RawHttpResponse::status(404, "text/plain"));
            }
            Ok(RawHttpResponse::html(
                "<html><body>no contact</body></html>",
            ))
        }
    }

    pub(crate) fn enricher_with_slow_sites(
        delay: Duration,
        probe: Arc<ConcurrencyProbe>,
    ) -> EmailEnricher {
        let client =
            SafeHttpClient::with_backend(Arc::new(PublicDns), Arc::new(SlowSites { delay, probe }));
        EmailEnricher::new(client)
    }
}
