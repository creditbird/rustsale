use crate::google_maps_jobs::types::{ScrapeErrorKind, ScrapeJobError};
use crate::scrapers::browser::{GoogleMapsPageHealth, GoogleMapsPageState, SessionGuard};
use crate::scrapers::grid::BoundingBox;
use anyhow::Result as AnyResult;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct BusinessInfo {
    pub name: String,
    pub phone: String,
    pub address: Option<String>,
    pub website: Option<String>,
    pub facebook: Option<String>,
    pub zalo: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SearchPageResult {
    pub raw_listing_count: usize,
    pub reached_end: bool,
    pub blocked_reason: Option<ScrapeErrorKind>,
    pub places: Vec<PlaceCandidate>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PlaceCandidate {
    pub place_id: Option<String>,
    pub canonical_url: String,
    pub name: String,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub website: Option<String>,
    pub facebook: Option<String>,
    pub zalo: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ScrollInfo {
    height: u64,
    is_end: bool,
    place_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPlaceCandidate {
    place_id: Option<String>,
    canonical_url: String,
    name: String,
    phone: Option<String>,
    address: Option<String>,
    website: Option<String>,
    facebook: Option<String>,
    zalo: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
}

pub struct GoogleMapsScraper {
    pub session: SessionGuard,
    pub base_url: String,
    pub min_delay_ms: u64,
    pub max_delay_ms: u64,
    ready_attempts: usize,
    ready_delay_ms: u64,
    max_scroll_attempts: usize,
    scroll_delay_ms: u64,
}

impl GoogleMapsScraper {
    pub fn new(session: SessionGuard) -> Self {
        Self {
            session,
            base_url: "https://www.google.com/maps".to_string(),
            min_delay_ms: 8000,
            max_delay_ms: 15000,
            ready_attempts: 20,
            ready_delay_ms: 500,
            max_scroll_attempts: 30,
            scroll_delay_ms: 800,
        }
    }

    pub fn with_base_url(session: SessionGuard, base_url: String) -> Self {
        Self {
            session,
            base_url,
            min_delay_ms: 10,
            max_delay_ms: 50,
            ready_attempts: 1,
            ready_delay_ms: 0,
            max_scroll_attempts: 5,
            scroll_delay_ms: 5,
        }
    }

    async fn delay(&self) {
        if self.min_delay_ms >= self.max_delay_ms {
            tokio::time::sleep(Duration::from_millis(self.min_delay_ms)).await;
        } else {
            let delay = rand::thread_rng().gen_range(self.min_delay_ms..=self.max_delay_ms);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
    }

    pub async fn search(
        &self,
        keyword: &str,
        location: Option<&str>,
    ) -> AnyResult<Vec<BusinessInfo>> {
        Ok(legacy_businesses(
            self.search_page(keyword, location).await?,
        )?)
    }

    pub async fn search_page(
        &self,
        keyword: &str,
        location: Option<&str>,
    ) -> Result<SearchPageResult, ScrapeJobError> {
        let query = if let Some(location) = location {
            format!("{keyword} {location}")
        } else {
            keyword.to_string()
        };
        let encoded = urlencoding::encode(&query);
        let url = format!("{}/search/{}?hl=en", self.base_url, encoded);
        self.scrape_url_typed(&url).await
    }

    pub async fn search_region(
        &self,
        keyword: &str,
        lat: f64,
        lng: f64,
        zoom: u8,
    ) -> AnyResult<Vec<BusinessInfo>> {
        Ok(legacy_businesses(
            self.search_region_page(keyword, lat, lng, zoom).await?,
        )?)
    }

    pub async fn search_region_page(
        &self,
        keyword: &str,
        lat: f64,
        lng: f64,
        zoom: u8,
    ) -> Result<SearchPageResult, ScrapeJobError> {
        let encoded_keyword = urlencoding::encode(keyword);
        let url = format!(
            "{}/search/{}/@{},{},{}z?hl=en",
            self.base_url, encoded_keyword, lat, lng, zoom
        );
        self.scrape_url_typed(&url).await
    }

    pub async fn scrape_url(&self, url: &str) -> AnyResult<Vec<BusinessInfo>> {
        Ok(legacy_businesses(self.scrape_url_typed(url).await?)?)
    }

    pub async fn scrape_url_typed(&self, url: &str) -> Result<SearchPageResult, ScrapeJobError> {
        self.session.navigate(url).await?;
        let initial_health = self.wait_for_page_health().await?;
        if let Some(result) = terminal_page_state(initial_health.state.clone(), 0, Vec::new())? {
            return Ok(result);
        }

        if initial_health.url.contains("/maps/place/") || url.contains("/maps/place/") {
            let place = self.extract_current_place().await?;
            return Ok(SearchPageResult {
                raw_listing_count: 1,
                reached_end: true,
                blocked_reason: None,
                places: vec![place],
            });
        }

        let (reached_end, observed_count) = self.scroll_feed().await?;
        let mut place_urls = self.collect_place_urls().await?;
        place_urls.sort();
        place_urls.dedup();
        let raw_listing_count = observed_count.max(place_urls.len());
        let mut places = Vec::with_capacity(place_urls.len());

        for place_url in place_urls {
            self.session.navigate(&place_url).await?;
            self.delay().await;
            let state = self.wait_for_page_state().await?;
            if state != GoogleMapsPageState::Ready {
                let mut result = terminal_page_state(state, raw_listing_count, places)?
                    .ok_or_else(|| {
                        scrape_error(
                            ScrapeErrorKind::InvalidResponse,
                            "non-ready Google Maps state was not terminal",
                            false,
                        )
                    })?;
                result.reached_end = false;
                return Ok(result);
            }
            places.push(self.extract_current_place().await?);
        }

        Ok(SearchPageResult {
            raw_listing_count,
            reached_end,
            blocked_reason: None,
            places: deduplicate_candidates(places),
        })
    }

    async fn wait_for_page_state(&self) -> Result<GoogleMapsPageState, ScrapeJobError> {
        Ok(self.wait_for_page_health().await?.state)
    }

    async fn wait_for_page_health(&self) -> Result<GoogleMapsPageHealth, ScrapeJobError> {
        for attempt in 0..self.ready_attempts.max(1) {
            let health = self.session.health_snapshot().await?;
            if health.state != GoogleMapsPageState::SelectorChanged {
                return Ok(health);
            }
            if attempt + 1 == self.ready_attempts.max(1) {
                return Err(scrape_error(
                    ScrapeErrorKind::SelectorChanged,
                    "Google Maps page did not expose a known result or place selector",
                    false,
                ));
            }
            tokio::time::sleep(Duration::from_millis(self.ready_delay_ms)).await;
        }
        Err(scrape_error(
            ScrapeErrorKind::SelectorChanged,
            "Google Maps readiness probe exhausted",
            false,
        ))
    }

    async fn scroll_feed(&self) -> Result<(bool, usize), ScrapeJobError> {
        let mut last_height = None;
        let mut no_change_count = 0;
        let mut reached_end = false;
        let mut observed_count = 0;

        for _ in 0..self.max_scroll_attempts.max(1) {
            let value = self.session.evaluate(SCROLL_FEED_SCRIPT).await?;
            let info: ScrollInfo = serde_json::from_value(value).map_err(|error| {
                scrape_error(
                    ScrapeErrorKind::InvalidResponse,
                    format!("invalid Google Maps scroll response: {error}"),
                    false,
                )
            })?;
            observed_count = observed_count.max(info.place_count);
            if info.is_end {
                reached_end = true;
                break;
            }
            if last_height == Some(info.height) {
                no_change_count += 1;
                if no_change_count >= 3 {
                    break;
                }
            } else {
                last_height = Some(info.height);
                no_change_count = 0;
            }
            tokio::time::sleep(Duration::from_millis(self.scroll_delay_ms)).await;
        }

        Ok((reached_end, observed_count))
    }

    async fn collect_place_urls(&self) -> Result<Vec<String>, ScrapeJobError> {
        let value = self.session.evaluate(COLLECT_PLACE_URLS_SCRIPT).await?;
        let urls: Vec<String> = serde_json::from_value(value).map_err(|error| {
            scrape_error(
                ScrapeErrorKind::InvalidResponse,
                format!("invalid Google Maps place URL response: {error}"),
                false,
            )
        })?;
        Ok(urls
            .into_iter()
            .filter_map(|url| canonicalize_place_url(&url))
            .collect())
    }

    async fn extract_current_place(&self) -> Result<PlaceCandidate, ScrapeJobError> {
        let value = self.session.evaluate(EXTRACT_PLACE_SCRIPT).await?;
        let raw: RawPlaceCandidate = serde_json::from_value(value).map_err(|error| {
            scrape_error(
                ScrapeErrorKind::InvalidResponse,
                format!("invalid Google Maps place detail response: {error}"),
                false,
            )
        })?;
        let name = raw.name.trim().to_string();
        if name.is_empty() {
            return Err(scrape_error(
                ScrapeErrorKind::SelectorChanged,
                "Google Maps place detail omitted the name",
                false,
            ));
        }
        let canonical_url = canonicalize_place_url(&raw.canonical_url).ok_or_else(|| {
            scrape_error(
                ScrapeErrorKind::InvalidResponse,
                "Google Maps place detail omitted a valid canonical URL",
                false,
            )
        })?;
        let (url_latitude, url_longitude) = coordinates_from_url(&canonical_url);
        Ok(PlaceCandidate {
            place_id: clean_optional(raw.place_id).or_else(|| place_id_from_url(&canonical_url)),
            canonical_url,
            name,
            phone: clean_optional(raw.phone),
            address: clean_optional(raw.address),
            website: clean_optional(raw.website),
            facebook: clean_optional(raw.facebook),
            zalo: clean_optional(raw.zalo),
            latitude: raw.latitude.or(url_latitude),
            longitude: raw.longitude.or(url_longitude),
        })
    }
}

pub fn candidate_matches_requested_area(
    candidate: &PlaceCandidate,
    bbox: BoundingBox,
    requested_location: Option<&str>,
) -> bool {
    match (candidate.latitude, candidate.longitude) {
        (Some(latitude), Some(longitude)) => {
            latitude.is_finite()
                && longitude.is_finite()
                && latitude >= bbox.lat_min
                && latitude <= bbox.lat_max
                && longitude >= bbox.lng_min
                && longitude <= bbox.lng_max
        }
        _ => candidate
            .address
            .as_deref()
            .zip(requested_location)
            .is_some_and(|(address, location)| deterministic_location_match(address, location)),
    }
}

fn deterministic_location_match(address: &str, requested_location: &str) -> bool {
    let address_tokens: HashSet<String> = location_tokens(address).into_iter().collect();
    let requested_tokens = location_tokens(requested_location);
    !requested_tokens.is_empty()
        && requested_tokens
            .iter()
            .all(|token| address_tokens.contains(token))
}

fn location_tokens(value: &str) -> Vec<String> {
    value
        .to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn terminal_page_state(
    state: GoogleMapsPageState,
    raw_listing_count: usize,
    places: Vec<PlaceCandidate>,
) -> Result<Option<SearchPageResult>, ScrapeJobError> {
    let blocked_reason = match state {
        GoogleMapsPageState::Ready => return Ok(None),
        GoogleMapsPageState::EmptyResults => {
            return Ok(Some(SearchPageResult {
                raw_listing_count: 0,
                reached_end: true,
                blocked_reason: None,
                places,
            }));
        }
        GoogleMapsPageState::Captcha => ScrapeErrorKind::Captcha,
        GoogleMapsPageState::ConsentRequired => ScrapeErrorKind::ConsentRequired,
        GoogleMapsPageState::RateLimited => ScrapeErrorKind::RateLimited,
        GoogleMapsPageState::SelectorChanged => {
            return Err(scrape_error(
                ScrapeErrorKind::SelectorChanged,
                "Google Maps selectors changed",
                false,
            ));
        }
    };
    Ok(Some(SearchPageResult {
        raw_listing_count,
        reached_end: false,
        blocked_reason: Some(blocked_reason),
        places,
    }))
}

fn legacy_businesses(result: SearchPageResult) -> Result<Vec<BusinessInfo>, ScrapeJobError> {
    if let Some(kind) = result.blocked_reason {
        return Err(scrape_error(
            kind,
            "Google Maps requires user action before scraping can continue",
            false,
        ));
    }
    Ok(result
        .places
        .into_iter()
        .filter_map(|place| {
            let phone = place.phone?.trim().to_string();
            if phone.is_empty() {
                return None;
            }
            Some(BusinessInfo {
                name: place.name,
                phone,
                address: place.address,
                website: place.website,
                facebook: place.facebook,
                zalo: place.zalo,
            })
        })
        .collect())
}

fn deduplicate_candidates(candidates: Vec<PlaceCandidate>) -> Vec<PlaceCandidate> {
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|candidate| seen.insert(candidate_dedup_key(candidate)))
        .collect()
}

fn candidate_dedup_key(candidate: &PlaceCandidate) -> String {
    if let Some(place_id) = &candidate.place_id {
        return format!("place:{place_id}");
    }
    if !candidate.canonical_url.is_empty() {
        return format!("url:{}", candidate.canonical_url.to_lowercase());
    }
    if let Some(phone) = &candidate.phone {
        let digits: String = phone.chars().filter(char::is_ascii_digit).collect();
        if !digits.is_empty() {
            return format!("phone:{digits}");
        }
    }
    format!(
        "name-address:{}|{}",
        candidate.name.trim().to_lowercase(),
        candidate
            .address
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_lowercase()
    )
}

fn canonicalize_place_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return None;
    }
    let without_fragment = trimmed.split('#').next().unwrap_or(trimmed);
    let canonical = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    if !canonical.contains("/maps/place/") {
        return None;
    }
    Some(canonical.trim_end_matches('/').to_string())
}

fn coordinates_from_url(url: &str) -> (Option<f64>, Option<f64>) {
    if let Some(position) = url.find('@') {
        let coordinates = &url[position + 1..];
        let mut parts = coordinates.split(',');
        let latitude = parts.next().and_then(|value| value.parse::<f64>().ok());
        let longitude = parts.next().and_then(|value| value.parse::<f64>().ok());
        if latitude.is_some() && longitude.is_some() {
            return (latitude, longitude);
        }
    }
    let latitude = marker_number(url, "!3d");
    let longitude = marker_number(url, "!4d");
    (latitude, longitude)
}

fn marker_number(url: &str, marker: &str) -> Option<f64> {
    let start = url.find(marker)? + marker.len();
    let value: String = url[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit() || matches!(character, '-' | '+' | '.'))
        .collect();
    value.parse().ok()
}

fn place_id_from_url(url: &str) -> Option<String> {
    if let Some(start) = url.find("!1s") {
        let value: String = url[start + 3..]
            .chars()
            .take_while(|character| *character != '!')
            .collect();
        return clean_optional(Some(value));
    }
    None
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn scrape_error(
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

const SCROLL_FEED_SCRIPT: &str = r#"
(() => {
    const feed = document.querySelector('div[role="feed"]');
    if (feed) {
        feed.scrollTop = feed.scrollHeight;
        const text = feed.textContent || "";
        const isEnd = text.includes("You've reached the end of the list.") ||
            text.includes("kết thúc danh sách") ||
            text.includes("reached the end");
        const placeCount = feed.querySelectorAll('a[href*="/maps/place/"]').length;
        return {
            height: Math.max(0, Math.floor(feed.scrollHeight)),
            is_end: isEnd || placeCount >= 120,
            place_count: placeCount
        };
    }
    window.scrollBy(0, 1500);
    return {
        height: Math.max(0, Math.floor(document.body?.scrollHeight || 0)),
        is_end: false,
        place_count: document.querySelectorAll('a[href*="/maps/place/"]').length
    };
})()
"#;

const COLLECT_PLACE_URLS_SCRIPT: &str = r#"
(() => Array.from(document.querySelectorAll('a[href*="/maps/place/"]'))
    .map((anchor) => anchor.href)
    .filter((href) => typeof href === 'string' && href.length > 0))()
"#;

const EXTRACT_PLACE_SCRIPT: &str = r#"
(() => {
    const text = (element) => element?.textContent?.trim() || '';
    const canonicalUrl = window.location.href;
    const name = text(document.querySelector('h1'));
    const phoneItem = document.querySelector('[data-item-id^="phone:tel:"]');
    const telLink = document.querySelector('a[href^="tel:"]');
    let phone = phoneItem?.getAttribute('data-item-id')?.replace('phone:tel:', '').trim() ||
        telLink?.getAttribute('href')?.replace('tel:', '').trim() || '';
    if (!phone) {
        const elements = Array.from(document.querySelectorAll('button, a, div, span'));
        for (const element of elements) {
            const candidate = text(element);
            const cleaned = candidate.replace(/[\s.-]/g, '');
            if (/^(?:\+84|0)\d{9,10}$/.test(cleaned)) {
                phone = candidate;
                break;
            }
        }
    }
    const address = text(document.querySelector('[data-item-id="address"]')) ||
        text(document.querySelector('.rogA2c'));
    const authority = document.querySelector('[data-item-id="authority"]');
    let website = authority?.getAttribute('href') || text(authority);
    let facebook = '';
    let zalo = '';
    for (const link of Array.from(document.querySelectorAll('a[href]'))) {
        const href = link.getAttribute('href') || '';
        if (!facebook && href.includes('facebook.com')) facebook = href;
        if (!zalo && href.includes('zalo.me')) zalo = href;
        if (!website && href.startsWith('http') &&
            !href.includes('google.') && !href.includes('gstatic.com') &&
            !href.includes('facebook.com') && !href.includes('zalo.me')) {
            website = href;
        }
    }
    const coordinateMatch = canonicalUrl.match(/@(-?\d+(?:\.\d+)?),(-?\d+(?:\.\d+)?)/);
    const dataLatitude = canonicalUrl.match(/!3d(-?\d+(?:\.\d+)?)/);
    const dataLongitude = canonicalUrl.match(/!4d(-?\d+(?:\.\d+)?)/);
    const placeIdMatch = canonicalUrl.match(/!1s([^!]+)/);
    return {
        placeId: placeIdMatch?.[1] || null,
        canonicalUrl,
        name,
        phone: phone || null,
        address: address || null,
        website: website || null,
        facebook: facebook || null,
        zalo: zalo || null,
        latitude: coordinateMatch ? Number(coordinateMatch[1]) :
            (dataLatitude ? Number(dataLatitude[1]) : null),
        longitude: coordinateMatch ? Number(coordinateMatch[2]) :
            (dataLongitude ? Number(dataLongitude[1]) : null)
    };
})()
"#;

#[cfg(test)]
mod typed_tests {
    use super::*;
    use crate::google_maps_jobs::types::{ScrapeErrorKind, ScrapeJobError};
    use crate::scrapers::browser::{
        BrowserAutomation, BrowserManager, GoogleMapsPageHealth, GoogleMapsPageState,
        LaunchRequest, RuntimeStatus,
    };
    use crate::scrapers::grid::BoundingBox;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::collections::VecDeque;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    enum Step {
        Navigate(Result<(), ScrapeJobError>),
        Health(Result<GoogleMapsPageState, ScrapeJobError>),
        HealthSnapshot(Result<GoogleMapsPageHealth, ScrapeJobError>),
        Evaluate(Result<Value, ScrapeJobError>),
    }

    struct FakeBrowserAutomation {
        steps: Mutex<VecDeque<Step>>,
    }

    impl FakeBrowserAutomation {
        fn new(steps: Vec<Step>) -> Self {
            Self {
                steps: Mutex::new(steps.into()),
            }
        }

        fn pop(&self) -> Step {
            self.steps
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected browser call")
        }

        fn assert_exhausted(&self) {
            assert!(self.steps.lock().unwrap().is_empty());
        }
    }

    #[async_trait]
    impl BrowserAutomation for FakeBrowserAutomation {
        async fn runtime_status(&self) -> Result<RuntimeStatus, ScrapeJobError> {
            Ok(RuntimeStatus::installed_for_test())
        }

        async fn ensure_runtime(&self) -> Result<RuntimeStatus, ScrapeJobError> {
            Ok(RuntimeStatus::installed_for_test())
        }

        async fn launch(&self, _request: LaunchRequest) -> Result<(), ScrapeJobError> {
            Ok(())
        }

        async fn navigate(&self, _url: &str, _timeout: Duration) -> Result<(), ScrapeJobError> {
            match self.pop() {
                Step::Navigate(result) => result,
                _ => panic!("expected navigate call"),
            }
        }

        async fn evaluate(
            &self,
            _script: &str,
            _timeout: Duration,
        ) -> Result<Value, ScrapeJobError> {
            match self.pop() {
                Step::Evaluate(result) => result,
                _ => panic!("expected evaluate call"),
            }
        }

        async fn screenshot(&self, _path: &Path) -> Result<(), ScrapeJobError> {
            Ok(())
        }

        async fn health_check(&self) -> Result<GoogleMapsPageState, ScrapeJobError> {
            match self.pop() {
                Step::Health(result) => result,
                Step::HealthSnapshot(result) => result.map(|health| health.state),
                _ => panic!("expected health-check call"),
            }
        }

        async fn health_snapshot(&self) -> Result<GoogleMapsPageHealth, ScrapeJobError> {
            match self.pop() {
                Step::Health(result) => result.map(|state| GoogleMapsPageHealth {
                    state,
                    url: String::new(),
                    title: String::new(),
                }),
                Step::HealthSnapshot(result) => result,
                _ => panic!("expected health-snapshot call"),
            }
        }

        async fn shutdown(&self) -> Result<(), ScrapeJobError> {
            Ok(())
        }
    }

    async fn scripted_scraper(steps: Vec<Step>) -> (GoogleMapsScraper, Arc<FakeBrowserAutomation>) {
        let automation = Arc::new(FakeBrowserAutomation::new(steps));
        let manager = BrowserManager::with_automation(1, automation.clone());
        manager.start(true).await.unwrap();
        let session = manager.acquire().await.unwrap();
        let mut scraper =
            GoogleMapsScraper::with_base_url(session, "https://www.google.com/maps".to_string());
        scraper.min_delay_ms = 0;
        scraper.max_delay_ms = 0;
        (scraper, automation)
    }

    fn fault(kind: ScrapeErrorKind, message: &str, retryable: bool) -> ScrapeJobError {
        ScrapeJobError {
            kind,
            message: message.to_string(),
            retryable,
        }
    }

    #[tokio::test]
    async fn disconnect_during_initial_navigation_returns_disconnected() {
        let (scraper, automation) = scripted_scraper(vec![Step::Navigate(Err(fault(
            ScrapeErrorKind::Disconnected,
            "connection lost",
            true,
        )))])
        .await;

        let error = scraper.search_page("cafe", None).await.unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::Disconnected);
        automation.assert_exhausted();
    }

    #[tokio::test]
    async fn disconnect_during_load_probe_is_not_converted_to_false() {
        let (scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Err(fault(
                ScrapeErrorKind::Disconnected,
                "health probe disconnected",
                true,
            ))),
        ])
        .await;

        let error = scraper.search_page("cafe", None).await.unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::Disconnected);
        automation.assert_exhausted();
    }

    #[tokio::test]
    async fn disconnect_during_scroll_is_not_converted_to_null() {
        let (scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::Ready)),
            Step::Evaluate(Err(fault(
                ScrapeErrorKind::Disconnected,
                "scroll disconnected",
                true,
            ))),
        ])
        .await;

        let error = scraper.search_page("cafe", None).await.unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::Disconnected);
        automation.assert_exhausted();
    }

    #[tokio::test]
    async fn disconnect_during_place_navigation_stops_detail_traversal() {
        let (scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::Ready)),
            Step::Evaluate(Ok(json!({"height": 100, "is_end": true, "place_count": 1}))),
            Step::Evaluate(Ok(json!(["https://www.google.com/maps/place/Cafe-One"]))),
            Step::Navigate(Err(fault(
                ScrapeErrorKind::Disconnected,
                "place navigation disconnected",
                true,
            ))),
        ])
        .await;

        let error = scraper.search_page("cafe", None).await.unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::Disconnected);
        automation.assert_exhausted();
    }

    #[tokio::test]
    async fn selector_change_returns_selector_changed() {
        let (scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::SelectorChanged)),
        ])
        .await;

        let error = scraper.search_page("cafe", None).await.unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::SelectorChanged);
        automation.assert_exhausted();
    }

    #[tokio::test]
    async fn legitimate_empty_page_returns_reached_end_with_zero_listings() {
        let (scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::EmptyResults)),
        ])
        .await;

        let result = scraper.search_page("missing", None).await.unwrap();

        assert_eq!(result.raw_listing_count, 0);
        assert!(result.reached_end);
        assert!(result.places.is_empty());
        assert_eq!(result.blocked_reason, None);
        automation.assert_exhausted();
    }

    #[tokio::test]
    async fn search_redirected_to_direct_place_extracts_listing() {
        let final_url = "https://www.google.com/maps/place/Cuc-Gach-Quan/@10.7901,106.6902,17z";
        let (scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::HealthSnapshot(Ok(GoogleMapsPageHealth {
                state: GoogleMapsPageState::Ready,
                url: final_url.to_string(),
                title: "Cuc Gach Quan - Google Maps".to_string(),
            })),
            Step::Evaluate(Ok(json!({
                "placeId": "place-1",
                "canonicalUrl": final_url,
                "name": "Cuc Gach Quan",
                "phone": "0901234567",
                "address": "10 Dang Tat, Quan 1, Ho Chi Minh City",
                "website": "https://example.com",
                "facebook": null,
                "zalo": null,
                "latitude": 10.7901,
                "longitude": 106.6902
            }))),
        ])
        .await;

        let result = scraper
            .search_page("Cuc Gach Quan", Some("Quan 1, Ho Chi Minh City"))
            .await
            .unwrap();

        assert_eq!(result.raw_listing_count, 1);
        assert!(result.reached_end);
        assert_eq!(result.places.len(), 1);
        assert_eq!(result.places[0].name, "Cuc Gach Quan");
        automation.assert_exhausted();
    }

    #[test]
    fn outside_bbox_result_is_retained_but_not_qualified() {
        let candidate = PlaceCandidate {
            place_id: Some("place-1".to_string()),
            canonical_url: "https://www.google.com/maps/place/Outside".to_string(),
            name: "Outside Cafe".to_string(),
            phone: Some("0901234567".to_string()),
            address: Some("Da Nang".to_string()),
            website: None,
            facebook: None,
            zalo: None,
            latitude: Some(16.0544),
            longitude: Some(108.2022),
        };
        let result = SearchPageResult {
            raw_listing_count: 1,
            reached_end: true,
            blocked_reason: None,
            places: vec![candidate.clone()],
        };

        assert_eq!(result.places.len(), 1);
        assert!(!candidate_matches_requested_area(
            &candidate,
            BoundingBox {
                lat_min: 10.7,
                lat_max: 10.9,
                lng_min: 106.5,
                lng_max: 106.9,
            },
            Some("Ho Chi Minh")
        ));
    }

    #[test]
    fn missing_coordinate_result_requires_deterministic_text_location_match() {
        let matching = PlaceCandidate {
            place_id: None,
            canonical_url: "https://www.google.com/maps/place/District-One".to_string(),
            name: "District One Cafe".to_string(),
            phone: None,
            address: Some("123 Le Loi, District 1, Ho Chi Minh".to_string()),
            website: Some("https://example.test".to_string()),
            facebook: None,
            zalo: None,
            latitude: None,
            longitude: None,
        };
        let mismatch = PlaceCandidate {
            address: Some("123 Le Loi, District 10, Ho Chi Minh".to_string()),
            ..matching.clone()
        };
        let bbox = BoundingBox {
            lat_min: 10.7,
            lat_max: 10.9,
            lng_min: 106.5,
            lng_max: 106.9,
        };

        assert!(candidate_matches_requested_area(
            &matching,
            bbox,
            Some("District 1")
        ));
        assert!(!candidate_matches_requested_area(
            &mismatch,
            bbox,
            Some("District 1")
        ));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrapers::browser::BrowserManager;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn spawn_mock_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0; 4096];
                    if let Ok(n) = socket.read(&mut buf).await {
                        let request = String::from_utf8_lossy(&buf[..n]);

                        let response = if request.contains("/search") {
                            let body = format!(
                                r#"<html>
                                <body>
                                  <div role="feed">
                                    <a href="http://127.0.0.1:{}/maps/place/Cafe+One">Cafe One</a>
                                    <a href="http://127.0.0.1:{}/maps/place/Cafe+Two">Cafe Two</a>
                                  </div>
                                </body>
                                </html>"#,
                                port, port
                            );
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                                body.len(), body
                            )
                        } else if request.contains("/maps/place/Cafe+One") {
                            let body = r#"<html>
                            <body>
                              <h1>Cafe One</h1>
                              <button data-item-id="phone:tel:+84987654321">Phone Button</button>
                              <button data-item-id="address">123 Le Loi St, Dist 1, HCMC</button>
                              <a data-item-id="authority" href="https://cafeone.com">Website</a>
                              <a href="https://facebook.com/cafeone">Facebook</a>
                              <a href="https://zalo.me/cafeone">Zalo</a>
                            </body>
                            </html>"#;
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                                body.len(), body
                            )
                        } else if request.contains("/maps/place/Cafe+Two") {
                            let body = r#"<html>
                            <body>
                              <h1>Cafe Two</h1>
                              <button data-item-id="address">456 Nguyen Hue St, Dist 1, HCMC</button>
                            </body>
                            </html>"#;
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                                body.len(), body
                            )
                        } else {
                            let body = "Not Found";
                            format!(
                                "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                                body.len(), body
                            )
                        };
                        let _ = socket.write_all(response.as_bytes()).await;
                    }
                });
            }
        });

        format!("http://127.0.0.1:{}", port)
    }

    #[tokio::test]
    async fn test_google_maps_scraper_search() {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let manager = BrowserManager::new(1).with_port(port);
        manager.start(true).await.expect("Failed to start browser");

        let session = manager.acquire().await.expect("Failed to acquire session");
        let mock_base = spawn_mock_server().await;

        let scraper = GoogleMapsScraper::with_base_url(session, mock_base);
        let results = scraper.search("cafe", Some("District 1")).await;

        assert!(results.is_ok(), "search failed: {:?}", results.err());
        let results = results.unwrap();

        assert_eq!(results.len(), 1);
        let first = &results[0];
        assert_eq!(first.name, "Cafe One");
        assert_eq!(first.phone, "+84987654321");
        assert_eq!(
            first.address,
            Some("123 Le Loi St, Dist 1, HCMC".to_string())
        );
        assert_eq!(first.website, Some("https://cafeone.com".to_string()));
        assert_eq!(
            first.facebook,
            Some("https://facebook.com/cafeone".to_string())
        );
        assert_eq!(first.zalo, Some("https://zalo.me/cafeone".to_string()));

        let _ = manager.close().await;
    }
}
