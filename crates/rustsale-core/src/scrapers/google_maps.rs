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

/// The result of reading a search page without visiting any place detail.
///
/// Detail traversal is a separate, individually checkpointed step. Keeping the
/// two apart is what stops a block part-way through a listing from discarding
/// every place already scraped, and stops a detail-level fault from being
/// reported as a search-level empty result.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SearchListing {
    pub raw_listing_count: usize,
    pub reached_end: bool,
    pub blocked_reason: Option<ScrapeErrorKind>,
    pub place_urls: Vec<String>,
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
    /// The page has no `div[role="feed"]`: the scroll fell back to the window.
    #[serde(default)]
    feed_missing: bool,
    /// The feed shows a loading indicator, so an unchanged size is not evidence of the end.
    #[serde(default)]
    loading: bool,
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

/// Consecutive identical `(height, place_count)` observations that establish an
/// exhausted feed. With the baseline observation this is four evaluations.
const STABLE_FEED_OBSERVATIONS: u32 = 3;

pub struct GoogleMapsScraper {
    pub session: SessionGuard,
    pub base_url: String,
    pub min_delay_ms: u64,
    pub max_delay_ms: u64,
    ready_attempts: usize,
    ready_delay_ms: u64,
    /// Extra readiness polls granted to a non-empty page that is not ready yet (slow render).
    slow_render_extra_attempts: usize,
    /// Short settle window granted when a `/maps/place/` page initially reports
    /// `PlaceUnavailable`, so a slow real place can finish hydrating its `<h1>`
    /// without stalling 20 s on every withdrawn listing.
    place_unavailable_attempts: usize,
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
            slow_render_extra_attempts: 20,
            place_unavailable_attempts: 4,
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
            slow_render_extra_attempts: 0,
            place_unavailable_attempts: 1,
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

    pub fn search_url(&self, keyword: &str, location: Option<&str>) -> String {
        let query = if let Some(location) = location {
            format!("{keyword} {location}")
        } else {
            keyword.to_string()
        };
        let encoded = urlencoding::encode(&query);
        format!("{}/search/{}?hl=en", self.base_url, encoded)
    }

    pub fn search_region_url(&self, keyword: &str, lat: f64, lng: f64, zoom: u8) -> String {
        let encoded_keyword = urlencoding::encode(keyword);
        format!(
            "{}/search/{}/@{},{},{}z?hl=en",
            self.base_url, encoded_keyword, lat, lng, zoom
        )
    }

    pub async fn search_page(
        &self,
        keyword: &str,
        location: Option<&str>,
    ) -> Result<SearchPageResult, ScrapeJobError> {
        let url = self.search_url(keyword, location);
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
        let url = self.search_region_url(keyword, lat, lng, zoom);
        self.scrape_url_typed(&url).await
    }

    pub async fn scrape_url(&self, url: &str) -> AnyResult<Vec<BusinessInfo>> {
        Ok(legacy_businesses(self.scrape_url_typed(url).await?)?)
    }

    /// Reads a search page down to its place URLs and stops there.
    ///
    /// This never navigates to a place, so a block encountered here is
    /// unambiguously a search-level block and the caller can checkpoint each
    /// place independently afterwards.
    pub async fn collect_search_listing(&self, url: &str) -> Result<SearchListing, ScrapeJobError> {
        self.session.navigate(url).await?;
        let health = self.wait_for_page_health().await?;
        if let Some(listing) = terminal_listing_state(health.state.clone())? {
            return Ok(listing);
        }

        if health.url.contains("/maps/place/") || url.contains("/maps/place/") {
            let canonical = canonicalize_place_url(&health.url)
                .or_else(|| canonicalize_place_url(url))
                .ok_or_else(|| {
                    scrape_error(
                        ScrapeErrorKind::InvalidResponse,
                        "Google Maps redirected to a place without a usable canonical URL",
                        false,
                    )
                })?;
            return Ok(SearchListing {
                raw_listing_count: 1,
                reached_end: true,
                blocked_reason: None,
                place_urls: vec![canonical],
            });
        }

        let (reached_end, observed_count) = self.scroll_feed().await?;
        let mut place_urls = self.collect_place_urls().await?;
        place_urls.sort();
        place_urls.dedup();
        Ok(SearchListing {
            raw_listing_count: observed_count.max(place_urls.len()),
            reached_end,
            blocked_reason: None,
            place_urls,
        })
    }

    /// Visits exactly one place detail page.
    ///
    /// Every failure is scoped to this place: a non-ready page is reported with
    /// a place-level error kind rather than being folded into the search-level
    /// terminal states.
    pub async fn scrape_place(&self, place_url: &str) -> Result<PlaceCandidate, ScrapeJobError> {
        self.session
            .navigate(&place_navigation_url(place_url))
            .await?;
        self.delay().await;
        let state = self.wait_for_page_state().await?;
        if state != GoogleMapsPageState::Ready {
            return Err(place_state_error(state, place_url));
        }
        self.extract_current_place().await
    }

    pub async fn scrape_url_typed(&self, url: &str) -> Result<SearchPageResult, ScrapeJobError> {
        let listing = self.collect_search_listing(url).await?;
        if listing.blocked_reason.is_some() {
            return Ok(SearchPageResult {
                raw_listing_count: listing.raw_listing_count,
                reached_end: listing.reached_end,
                blocked_reason: listing.blocked_reason,
                places: Vec::new(),
            });
        }

        let mut places = Vec::with_capacity(listing.place_urls.len());
        for place_url in &listing.place_urls {
            places.push(self.scrape_place(place_url).await?);
        }

        Ok(SearchPageResult {
            raw_listing_count: listing.raw_listing_count,
            reached_end: listing.reached_end,
            blocked_reason: None,
            places: deduplicate_candidates(places),
        })
    }

    async fn wait_for_page_state(&self) -> Result<GoogleMapsPageState, ScrapeJobError> {
        Ok(self.wait_for_page_health().await?.state)
    }

    async fn wait_for_page_health(&self) -> Result<GoogleMapsPageHealth, ScrapeJobError> {
        let blank_limit = self.ready_attempts.max(1);
        let place_unavailable_limit = self.place_unavailable_attempts.max(1);
        // A blank page is retried by the runner after `blank_limit` polls. A non-empty page
        // that still shows no known selector may just be rendering slowly, so it gets a
        // longer window before it is reported as `SelectorChanged`. A `/maps/place/` page
        // reporting `PlaceUnavailable` gets only a short settle window so slow real places
        // can hydrate `<h1>` without polling 20 s per dead listing.
        let total_attempts = blank_limit.saturating_add(self.slow_render_extra_attempts);
        let mut place_unavailable_seen = 0usize;
        for attempt in 0..total_attempts {
            let health = self.session.health_snapshot().await?;
            match health.state {
                GoogleMapsPageState::PlaceUnavailable => {
                    place_unavailable_seen += 1;
                    if place_unavailable_seen >= place_unavailable_limit
                        || attempt + 1 >= total_attempts
                    {
                        return Ok(health);
                    }
                }
                GoogleMapsPageState::BlankPage | GoogleMapsPageState::SelectorChanged => {
                    place_unavailable_seen = 0;
                    let limit = match health.state {
                        GoogleMapsPageState::BlankPage => blank_limit,
                        _ => total_attempts,
                    };
                    if attempt + 1 >= limit {
                        return Err(match health.state {
                            GoogleMapsPageState::BlankPage => scrape_error(
                                ScrapeErrorKind::TransientNetwork,
                                "Google Maps rendered a blank page after navigation",
                                true,
                            ),
                            _ => scrape_error(
                                ScrapeErrorKind::SelectorChanged,
                                "Google Maps page did not expose a known result or place selector",
                                false,
                            ),
                        });
                    }
                }
                _ => return Ok(health),
            }
            tokio::time::sleep(Duration::from_millis(self.ready_delay_ms)).await;
        }
        Err(scrape_error(
            ScrapeErrorKind::SelectorChanged,
            "Google Maps readiness probe exhausted",
            false,
        ))
    }

    /// Scrolls the result feed until it is exhausted, and reports whether
    /// exhaustion was actually established.
    ///
    /// Exhaustion is proven either by the end-of-list sentinel and the result
    /// cap, both reported as `is_end` by the page script, or by observing the
    /// same `(height, place_count)` pair `STABLE_FEED_OBSERVATIONS` times in a
    /// row. Relying on the sentinel alone made a localized string the only
    /// evidence of a finished feed, so a missing sentinel reported an exhausted
    /// cell as unfinished and `should_subdivide` fanned it out needlessly.
    ///
    /// Running out of scroll attempts while either value is still changing is a
    /// different outcome: the feed is not known to be exhausted, so this
    /// reports `false`.
    async fn scroll_feed(&self) -> Result<(bool, usize), ScrapeJobError> {
        let mut last_snapshot = None;
        let mut stable_observations = 0;
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
            if info.feed_missing {
                return Err(scrape_error(
                    ScrapeErrorKind::SelectorChanged,
                    "Google Maps result feed is missing after the page reported ready",
                    false,
                ));
            }
            observed_count = observed_count.max(info.place_count);
            if info.is_end {
                reached_end = true;
                break;
            }
            let snapshot = (info.height, info.place_count);
            if info.loading {
                last_snapshot = Some(snapshot);
                stable_observations = 0;
            } else if last_snapshot == Some(snapshot) {
                stable_observations += 1;
                if stable_observations >= STABLE_FEED_OBSERVATIONS {
                    reached_end = true;
                    break;
                }
            } else {
                last_snapshot = Some(snapshot);
                stable_observations = 0;
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
        let name = clean_place_name(&raw.name);
        if name.is_empty() {
            // The page loaded and the selectors still work, but there is no
            // place on it: the listing was withdrawn or never existed.
            return Err(scrape_error(
                ScrapeErrorKind::PlaceUnavailable,
                "Google Maps place no longer exists (the page has no place name)",
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

/// Classifies a *search page* state before any place is visited.
///
/// `EmptyResults` here is a genuine "this query has no results" answer. The
/// place-level equivalent is deliberately handled by [`place_state_error`],
/// because a place that fails to render says nothing about the search.
fn terminal_listing_state(
    state: GoogleMapsPageState,
) -> Result<Option<SearchListing>, ScrapeJobError> {
    let blocked_reason = match state {
        GoogleMapsPageState::Ready => return Ok(None),
        GoogleMapsPageState::EmptyResults | GoogleMapsPageState::PlaceUnavailable => {
            return Ok(Some(SearchListing {
                raw_listing_count: 0,
                reached_end: true,
                blocked_reason: None,
                place_urls: Vec::new(),
            }));
        }
        GoogleMapsPageState::Captcha => ScrapeErrorKind::Captcha,
        GoogleMapsPageState::ConsentRequired => ScrapeErrorKind::ConsentRequired,
        GoogleMapsPageState::RateLimited => ScrapeErrorKind::RateLimited,
        GoogleMapsPageState::BlankPage => {
            return Err(scrape_error(
                ScrapeErrorKind::TransientNetwork,
                "Google Maps rendered a blank search page",
                true,
            ));
        }
        GoogleMapsPageState::SelectorChanged => {
            return Err(scrape_error(
                ScrapeErrorKind::SelectorChanged,
                "Google Maps selectors changed",
                false,
            ));
        }
    };
    Ok(Some(SearchListing {
        raw_listing_count: 0,
        reached_end: false,
        blocked_reason: Some(blocked_reason),
        place_urls: Vec::new(),
    }))
}

/// Classifies a non-ready *place detail* state.
///
/// A place that reports `PlaceUnavailable` or `EmptyResults` was withdrawn or
/// never existed: it is a non-retryable place-scoped fault and must never be
/// folded into the search-level empty result or count as selector drift.
fn place_state_error(state: GoogleMapsPageState, place_url: &str) -> ScrapeJobError {
    match state {
        GoogleMapsPageState::Captcha => scrape_error(
            ScrapeErrorKind::Captcha,
            format!("Google Maps blocked the place detail page {place_url}"),
            false,
        ),
        GoogleMapsPageState::ConsentRequired => scrape_error(
            ScrapeErrorKind::ConsentRequired,
            format!("Google Maps requires consent before loading {place_url}"),
            false,
        ),
        GoogleMapsPageState::RateLimited => scrape_error(
            ScrapeErrorKind::RateLimited,
            format!("Google Maps rate limited the place detail page {place_url}"),
            true,
        ),
        GoogleMapsPageState::BlankPage => scrape_error(
            ScrapeErrorKind::TransientNetwork,
            format!("Google Maps rendered a blank place detail page {place_url}"),
            true,
        ),
        GoogleMapsPageState::SelectorChanged => scrape_error(
            ScrapeErrorKind::SelectorChanged,
            format!("Google Maps place detail selectors changed on {place_url}"),
            false,
        ),
        GoogleMapsPageState::EmptyResults | GoogleMapsPageState::PlaceUnavailable => scrape_error(
            ScrapeErrorKind::PlaceUnavailable,
            format!("Google Maps place no longer exists on {place_url}"),
            false,
        ),
        GoogleMapsPageState::Ready => scrape_error(
            ScrapeErrorKind::InvalidResponse,
            format!("Google Maps place detail page {place_url} did not render a place panel"),
            true,
        ),
    }
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
    if !candidate.canonical_url.trim().is_empty() {
        return place_identity(&candidate.canonical_url);
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

/// Place pages must carry the same language hint as search pages. Canonical
/// URLs intentionally drop every query parameter, so the hint has to be added
/// again when the detail page is opened (C18).
fn place_navigation_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.contains("hl=") {
        return trimmed.to_string();
    }
    let separator = if trimmed.contains('?') { '&' } else { '?' };
    format!("{trimmed}{separator}hl=en")
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

/// Extracts a place's coordinates from its URL.
///
/// `!3d`/`!4d` carry the place's own position and are preferred. `@lat,lng` is
/// only the map viewport centre, which depends on the grid cell the place was
/// reached from, so using it first misqualified places near a bbox edge.
fn coordinates_from_url(url: &str) -> (Option<f64>, Option<f64>) {
    let latitude = marker_number(url, "!3d");
    let longitude = marker_number(url, "!4d");
    if latitude.is_some() && longitude.is_some() {
        return (latitude, longitude);
    }

    if let Some(position) = url.find('@') {
        let coordinates = &url[position + 1..];
        let mut parts = coordinates.split(',');
        let viewport_latitude = parts.next().and_then(|value| value.parse::<f64>().ok());
        let viewport_longitude = parts.next().and_then(|value| value.parse::<f64>().ok());
        if viewport_latitude.is_some() && viewport_longitude.is_some() {
            return (viewport_latitude, viewport_longitude);
        }
    }
    (latitude, longitude)
}

/// The name slug of a place URL. Useful as a label, never as an identity:
/// `/maps/place/Starbucks` is every Starbucks.
fn place_url_slug(url: &str) -> String {
    let trimmed = url.trim();
    let without_viewport = trimmed.split("/@").next().unwrap_or(trimmed);
    let without_data = without_viewport
        .split("/data=")
        .next()
        .unwrap_or(without_viewport);
    without_data.trim_end_matches('/').to_lowercase()
}

/// Removes the `/@lat,lng,zoom` segment while keeping everything after it.
///
/// Returns `None` when nothing survives the removal, which means the viewport
/// was the only thing distinguishing this URL from another place's. Callers must
/// then keep the viewport rather than fall back to the slug.
fn url_without_viewport(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let start = trimmed.find("/@")?;
    let rest = &trimmed[start + 2..];
    let end = rest.find('/')?;
    let stripped = format!("{}{}", &trimmed[..start], &rest[end..]);
    let stripped = stripped.trim_end_matches('/').to_lowercase();
    (stripped != place_url_slug(trimmed)).then_some(stripped)
}

/// The durable identity of a place, derived from its URL.
///
/// One function serves the place unit key, the listing novelty key, and the
/// persisted result dedup key, so a place is scheduled once and stored once.
/// Precedence:
///
/// 1. the `!1s` feature id, Google's own stable identifier for the place;
/// 2. the authoritative `!3d`/`!4d` coordinates, which separate two branches of
///    a chain that share a name slug;
/// 3. the URL with only the viewport segment removed.
///
/// The viewport is never part of the identity, because the same place reached
/// from two grid cells carries two different viewports. The slug alone is never
/// the identity either: collapsing onto it silently discards businesses, since
/// duplicate place units are ignored on insert.
pub fn place_identity(url: &str) -> String {
    place_identity_aliases(url)
        .into_iter()
        .next()
        .unwrap_or_else(|| format!("url:{}", url.trim().to_lowercase()))
}

/// Every identity the same place may legitimately be listed under, strongest
/// first.
///
/// A place can appear with its feature id in one listing and with only its
/// coordinates in another. Those are aliases of one business, so planning
/// compares *all* of them against what is already known; using the primary key
/// alone would schedule the same place twice and inflate subdivision novelty.
pub fn place_identity_aliases(url: &str) -> Vec<String> {
    let mut aliases = Vec::new();
    if let Some(place_id) = place_id_from_url(url) {
        aliases.push(format!("place:{place_id}"));
    }
    if let (Some(latitude), Some(longitude)) =
        (marker_number(url, "!3d"), marker_number(url, "!4d"))
    {
        aliases.push(format!(
            "geo:{}:{latitude:.7}:{longitude:.7}",
            place_url_slug(url)
        ));
    }
    if aliases.is_empty() {
        // No feature id and no authoritative coordinates. Drop the viewport only
        // if something else remains to tell this place apart; otherwise keep the
        // whole URL, because a duplicate unit is recoverable and a silently
        // merged business is not.
        aliases.push(match url_without_viewport(url) {
            Some(stripped) => format!("url:{stripped}"),
            None => format!("url:{}", url.trim().to_lowercase()),
        });
    }
    aliases
}

fn marker_number(url: &str, marker: &str) -> Option<f64> {
    let start = url.find(marker)? + marker.len();
    let value: String = url[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit() || matches!(character, '-' | '+' | '.'))
        .collect();
    value.parse().ok()
}

pub fn place_id_from_url(url: &str) -> Option<String> {
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
        // Icon glyphs of Maps buttons live in the Unicode private-use area.
        let without_glyphs: String = value
            .chars()
            .filter(|ch| !('\u{E000}'..='\u{F8FF}').contains(ch))
            .collect();
        let trimmed = without_glyphs.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

/// Some listings space a brand one glyph at a time ("r ê u . coffee") even
/// though the sign reads as one word. Keep the listing's characters, drop the
/// private-use icons, and rejoin only names that are dominated by
/// single-character tokens so ordinary names ("Phúc Long Coffee & Tea") keep
/// their real word spacing.
fn clean_place_name(value: &str) -> String {
    let without_glyphs: String = value
        .chars()
        .filter(|ch| !('\u{E000}'..='\u{F8FF}').contains(ch))
        .collect();
    let collapsed = collapse_whitespace(&without_glyphs);
    rejoin_glyph_spaced_name(&collapsed)
}

fn collapse_whitespace(value: &str) -> String {
    let mut collapsed = String::with_capacity(value.len());
    let mut pending_space = false;
    for ch in value.chars() {
        if ch.is_whitespace() {
            pending_space = !collapsed.is_empty();
        } else {
            if pending_space {
                collapsed.push(' ');
                pending_space = false;
            }
            collapsed.push(ch);
        }
    }
    collapsed
}

fn rejoin_glyph_spaced_name(value: &str) -> String {
    let tokens: Vec<&str> = value.split(' ').filter(|token| !token.is_empty()).collect();
    if tokens.len() < 4 {
        return value.to_string();
    }
    let single_tokens = tokens.iter().filter(|token| is_single_glyph(token)).count();
    // A real multi-word name has at most one short token ("&", "A"), not
    // three quarters of the name.
    if single_tokens * 4 <= tokens.len() * 3 {
        return value.to_string();
    }
    let mut joined = String::with_capacity(value.len());
    for (index, token) in tokens.iter().enumerate() {
        if index == 0 {
            joined.push_str(token);
            continue;
        }
        let previous = tokens[index - 1];
        let token_starts_with_mark = token.chars().next().is_some_and(is_combining_mark);
        let follows_name_punctuation =
            is_single_glyph(previous) && previous.chars().last().is_some_and(is_name_punctuation);
        let attached = token_starts_with_mark
            || (is_single_glyph(previous) && is_single_glyph(token))
            || follows_name_punctuation;
        if !attached {
            joined.push(' ');
        }
        joined.push_str(token);
    }
    joined
}

fn is_single_glyph(token: &str) -> bool {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if is_combining_mark(first) {
        return chars.next().is_none();
    }
    chars.all(is_combining_mark)
}

fn is_combining_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{0300}'..='\u{036F}'
            | '\u{1AB0}'..='\u{1AFF}'
            | '\u{1DC0}'..='\u{1DFF}'
            | '\u{20D0}'..='\u{20FF}'
            | '\u{FE20}'..='\u{FE2F}'
    )
}

fn is_name_punctuation(ch: char) -> bool {
    matches!(ch, '.' | ',' | '·' | '-' | '_' | '\'' | '’')
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
            place_count: placeCount,
            loading: !!feed.querySelector('[role="progressbar"], [aria-busy="true"]')
        };
    }
    window.scrollBy(0, 1500);
    return {
        height: Math.max(0, Math.floor(document.body?.scrollHeight || 0)),
        is_end: false,
        place_count: document.querySelectorAll('a[href*="/maps/place/"]').length,
        feed_missing: true
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
        // Only buttons/links of the info panel that announce themselves as the phone
        // action. Free text elsewhere on the page (related places, reviews) is ignored.
        for (const element of Array.from(document.querySelectorAll('button[aria-label], a[aria-label]'))) {
            const label = (element.getAttribute('aria-label') || '').trim();
            const labelMatch = label.match(/^(?:phone|điện thoại)\s*:\s*(.+)$/i);
            if (!labelMatch) continue;
            const candidate = labelMatch[1].trim();
            if (/^(?:\+84|0)\d{9,10}$/.test(candidate.replace(/[\s.-]/g, ''))) {
                phone = candidate;
                break;
            }
        }
    }
    // Prefer the button's aria-label ("Address: ..."): textContent also carries the icon
    // glyph. The obfuscated Google class names are not used because they change per deploy.
    const addressItem = document.querySelector('[data-item-id="address"]');
    const addressLabel = (addressItem?.getAttribute('aria-label') || '')
        .replace(/^\s*(?:address|địa chỉ)\s*:\s*/i, '').trim();
    const address = addressLabel ||
        text(addressItem).replace(/[\uE000-\uF8FF]/g, '').trim();
    const authority = document.querySelector('[data-item-id="authority"]');
    // Only the official-website button counts: any other external link on a
    // place page (booking, ads, reviews) is not the business's own site.
    const website = authority?.getAttribute('href') || text(authority);
    let facebook = '';
    let zalo = '';
    for (const link of Array.from(document.querySelectorAll('a[href]'))) {
        const href = link.getAttribute('href') || '';
        if (!facebook && href.includes('facebook.com')) facebook = href;
        if (!zalo && href.includes('zalo.me')) zalo = href;
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
        latitude: dataLatitude ? Number(dataLatitude[1]) :
            (coordinateMatch ? Number(coordinateMatch[1]) : null),
        longitude: dataLongitude ? Number(dataLongitude[1]) :
            (coordinateMatch ? Number(coordinateMatch[2]) : null)
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
        navigated: Mutex<Vec<String>>,
    }

    impl FakeBrowserAutomation {
        fn new(steps: Vec<Step>) -> Self {
            Self {
                steps: Mutex::new(steps.into()),
                navigated: Mutex::new(Vec::new()),
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

        async fn navigate(&self, url: &str, _timeout: Duration) -> Result<(), ScrapeJobError> {
            self.navigated.lock().unwrap().push(url.to_string());
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
    async fn blank_page_returns_retryable_transient_network() {
        let (scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::BlankPage)),
        ])
        .await;

        let error = scraper.search_page("cafe", None).await.unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::TransientNetwork);
        assert!(error.retryable);
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
            // The redirect target is now scraped as its own place visit rather
            // than extracted in place, so it navigates and re-checks health.
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::Ready)),
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

    /// Google Maps lists this brand with one space between every glyph
    /// ("r ê u . coffee"), while the storefront reads "rêu.coffee". The place
    /// detail extraction must store the readable name, not the markup spacing.
    #[tokio::test]
    async fn place_detail_repairs_glyph_spaced_google_maps_name() {
        let place_url = "https://www.google.com/maps/place/r%C3%AAu.coffee/@10.767,106.696,17z";
        let (scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::Ready)),
            Step::Evaluate(Ok(json!({
                "placeId": "place-1",
                "canonicalUrl": place_url,
                "name": "r ê u . coffee",
                "phone": null,
                "address": "12 Le Loi, District 1",
                "website": null,
                "facebook": null,
                "zalo": null,
                "latitude": 10.767,
                "longitude": 106.696
            }))),
        ])
        .await;

        let place = scraper.scrape_place(place_url).await.unwrap();

        assert_eq!(place.name, "rêu.coffee");
        automation.assert_exhausted();
    }

    /// C10: a place page that loads but carries no place name was withdrawn or
    /// never existed. It must say so, instead of blaming the selectors.
    #[tokio::test]
    async fn place_page_without_a_name_reports_place_unavailable() {
        let place_url = "https://www.google.com/maps/place/?q=place_id:ChIJgone";
        let (scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::Ready)),
            Step::Evaluate(Ok(json!({
                "placeId": null,
                "canonicalUrl": place_url,
                "name": "  ",
                "phone": null,
                "address": null,
                "website": null,
                "facebook": null,
                "zalo": null,
                "latitude": null,
                "longitude": null
            }))),
        ])
        .await;

        let error = scraper.scrape_place(place_url).await.unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::PlaceUnavailable);
        assert!(!error.retryable);
        assert!(error.message.contains("no longer exists"), "{error:?}");
        automation.assert_exhausted();
    }

    /// Feed exhaustion must be inferred from a stable `(height, place_count)`
    /// observation, not only from the localized end-of-list sentinel. When the
    /// sentinel is absent the scraper still knows the feed stopped producing
    /// listings, and reporting `reached_end = false` makes `should_subdivide`
    /// fan a demonstrably exhausted cell out into four children.
    #[tokio::test]
    async fn stable_feed_without_end_sentinel_reports_reached_end() {
        let scroll = json!({"height": 1000, "is_end": false, "place_count": 1});
        let place_url = "https://www.google.com/maps/place/Cafe-One";
        let (scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::Ready)),
            Step::Evaluate(Ok(scroll.clone())),
            Step::Evaluate(Ok(scroll.clone())),
            Step::Evaluate(Ok(scroll.clone())),
            Step::Evaluate(Ok(scroll)),
            Step::Evaluate(Ok(json!([place_url]))),
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::Ready)),
            Step::Evaluate(Ok(json!({
                "placeId": "place-1",
                "canonicalUrl": place_url,
                "name": "Cafe One",
                "phone": "0901234567",
                "address": "1 Le Loi, District 1",
                "website": null,
                "facebook": null,
                "zalo": null,
                "latitude": 10.77,
                "longitude": 106.69
            }))),
        ])
        .await;

        let result = scraper.search_page("cafe", None).await.unwrap();

        assert_eq!(result.raw_listing_count, 1);
        assert_eq!(result.places.len(), 1);
        assert!(
            result.reached_end,
            "a feed whose height and place count stopped changing is exhausted"
        );
        automation.assert_exhausted();
    }

    /// A non-empty page that needs longer than the base readiness window to render its feed
    /// must not be reported as `SelectorChanged` (non-retryable, stops the job).
    #[tokio::test]
    async fn slow_render_is_retried_before_selector_changed() {
        let (mut scraper, automation) = scripted_scraper(vec![
            Step::Health(Ok(GoogleMapsPageState::SelectorChanged)),
            Step::Health(Ok(GoogleMapsPageState::SelectorChanged)),
            Step::Health(Ok(GoogleMapsPageState::SelectorChanged)),
            Step::Health(Ok(GoogleMapsPageState::Ready)),
        ])
        .await;
        scraper.ready_attempts = 2;
        scraper.slow_render_extra_attempts = 4;

        let state = scraper.wait_for_page_state().await.unwrap();

        assert_eq!(state, GoogleMapsPageState::Ready);
        automation.assert_exhausted();
    }

    /// A feed that is still loading (spinner shown) and has not grown for a few polls is a slow
    /// network, not the end of the list.
    #[tokio::test]
    async fn slow_feed_load_is_not_reported_as_end() {
        let loading = json!({"height": 1000, "is_end": false, "place_count": 1, "loading": true});
        let urls: Vec<String> = (1..=5)
            .map(|index| format!("https://www.google.com/maps/place/Cafe-{index}"))
            .collect();
        let (scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::Ready)),
            Step::Evaluate(Ok(
                json!({"height": 1000, "is_end": false, "place_count": 1}),
            )),
            Step::Evaluate(Ok(loading.clone())),
            Step::Evaluate(Ok(loading.clone())),
            Step::Evaluate(Ok(loading)),
            Step::Evaluate(Ok(
                json!({"height": 2400, "is_end": true, "place_count": 5}),
            )),
            Step::Evaluate(Ok(json!(urls))),
        ])
        .await;

        let result = scraper
            .collect_search_listing("https://www.google.com/maps/search/cafe")
            .await
            .unwrap();

        assert_eq!(result.raw_listing_count, 5);
        assert_eq!(result.place_urls.len(), 5);
        assert!(result.reached_end);
        automation.assert_exhausted();
    }

    /// A page that passed the readiness probe but has no result feed is not an exhausted
    /// list: reporting `reached_end` with zero places silently completes the unit empty.
    #[tokio::test]
    async fn missing_feed_after_ready_is_selector_changed_not_exhausted() {
        let no_feed =
            json!({"height": 900, "is_end": false, "place_count": 0, "feed_missing": true});
        let (scraper, _automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::Ready)),
            Step::Evaluate(Ok(no_feed.clone())),
            Step::Evaluate(Ok(no_feed.clone())),
            Step::Evaluate(Ok(no_feed.clone())),
            Step::Evaluate(Ok(no_feed)),
            Step::Evaluate(Ok(json!([]))),
        ])
        .await;

        let error = scraper.search_page("cafe", None).await.unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::SelectorChanged);
    }

    /// Collecting a search page must stop at the listing. Visiting details in
    /// the same unit is what made a mid-traversal block discard every place
    /// already scraped; details are now separate units with their own
    /// checkpoints. `assert_exhausted` is the actual assertion here: any extra
    /// browser call would be an unscripted step.
    #[tokio::test]
    async fn collecting_a_search_page_does_not_visit_place_details() {
        let first = "https://www.google.com/maps/place/Cafe-One";
        let second = "https://www.google.com/maps/place/Cafe-Two";
        let (scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::Ready)),
            Step::Evaluate(Ok(json!({"height": 900, "is_end": true, "place_count": 2}))),
            Step::Evaluate(Ok(json!([first, second]))),
        ])
        .await;

        let listing = scraper
            .collect_search_listing("https://www.google.com/maps/search/cafe?hl=en")
            .await
            .unwrap();

        assert_eq!(listing.raw_listing_count, 2);
        assert!(listing.reached_end);
        assert_eq!(listing.blocked_reason, None);
        assert_eq!(
            listing.place_urls,
            vec![first.to_string(), second.to_string()]
        );
        automation.assert_exhausted();
    }

    #[tokio::test]
    async fn place_navigation_url_keeps_hl_param() {
        let place_url = "https://www.google.com/maps/place/Cafe-Two";
        let (scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::Ready)),
            Step::Evaluate(Ok(json!({
                "placeId": "place-2",
                "canonicalUrl": place_url,
                "name": "Cafe Two",
                "phone": null,
                "address": null,
                "website": null,
                "facebook": null,
                "zalo": null,
                "latitude": null,
                "longitude": null
            }))),
        ])
        .await;

        scraper.scrape_place(place_url).await.unwrap();

        let navigated = automation.navigated.lock().unwrap();
        let last = navigated.last().expect("place page was not opened");
        assert!(
            last.contains("hl=en"),
            "place URL dropped the language hint: {last}"
        );
        drop(navigated);
        automation.assert_exhausted();
    }

    /// A non-ready state on a place detail page is a detail-level fault. It must
    /// never be reported as the search-level `EmptyResults` outcome, which used
    /// to rewrite the real listing count to zero and let a truncated traversal
    /// checkpoint as a successful unit.
    #[tokio::test]
    async fn place_detail_empty_results_is_a_place_scoped_fault() {
        let (scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::EmptyResults)),
        ])
        .await;

        let error = scraper
            .scrape_place("https://www.google.com/maps/place/Cafe-Two")
            .await
            .unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::PlaceUnavailable);
        assert!(
            !error.retryable,
            "a place reporting empty results does not exist and must not retry"
        );
        automation.assert_exhausted();
    }

    /// C10: a slow real place may briefly expose an empty `<h1>` while hydrating.
    /// The short settle window must let it become `Ready` before failing.
    #[tokio::test]
    async fn slow_real_place_hydrating_past_place_unavailable_succeeds() {
        let place_url = "https://www.google.com/maps/place/Cafe+Sam/@10.77,106.69,17z";
        let (mut scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::PlaceUnavailable)),
            Step::Health(Ok(GoogleMapsPageState::PlaceUnavailable)),
            Step::Health(Ok(GoogleMapsPageState::Ready)),
            Step::Evaluate(Ok(json!({
                "placeId": "place-sam",
                "canonicalUrl": place_url,
                "name": "Cafe Sam",
                "phone": "0901234567",
                "address": "1 Le Loi",
                "website": null,
                "facebook": null,
                "zalo": null,
                "latitude": 10.77,
                "longitude": 106.69
            }))),
        ])
        .await;
        scraper.ready_attempts = 20;
        scraper.slow_render_extra_attempts = 20;
        scraper.place_unavailable_attempts = 4;

        let place = scraper.scrape_place(place_url).await.unwrap();

        assert_eq!(place.name, "Cafe Sam");
        automation.assert_exhausted();
    }

    /// C10 end-to-end through the real sidecar JSON-lines wire contract and
    /// `ScrapeJobRunner`: three dead `/maps/place/` pages with
    /// `max_consecutive_failures = 2` must settle briefly (4 polls, not 40),
    /// map to non-retryable `PlaceUnavailable`, and finish 3/3 instead of
    /// aborting the job at 2/3.
    #[tokio::test]
    async fn dead_places_over_sidecar_protocol_finish_all_units_without_long_polling() {
        use crate::google_maps_jobs::events::ScrapeEventPublisher;
        use crate::google_maps_jobs::repository::ScrapeJobRepository;
        use crate::google_maps_jobs::runner::{
            FailureTolerance, RetryPolicy, ScrapeJobRunner, ScrapeUnitProcessor, UnitExecution,
        };
        use crate::google_maps_jobs::types::{
            ScrapeJob, ScrapeJobStatus, ScrapeMode, ScrapeStage, ScrapeUnit, ScrapeUnitStatus,
        };
        use crate::scrapers::browser::{BrowserSupervisor, ProcessControl, SupervisorTimeouts};
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::sync::Notify;

        struct NoopProcessControl {
            exited: AtomicBool,
            notify: Notify,
        }

        #[async_trait]
        impl ProcessControl for NoopProcessControl {
            fn pid(&self) -> Option<u32> {
                Some(9001)
            }
            async fn wait(&self) -> std::io::Result<()> {
                while !self.exited.load(Ordering::SeqCst) {
                    self.notify.notified().await;
                }
                Ok(())
            }
            async fn terminate(&self) -> std::io::Result<()> {
                self.exited.store(true, Ordering::SeqCst);
                self.notify.notify_waiters();
                Ok(())
            }
            async fn kill(&self) -> std::io::Result<()> {
                self.terminate().await
            }
        }

        let (client_io, sidecar_io) = tokio::io::duplex(32 * 1024);
        let (client_reader, client_writer) = tokio::io::split(client_io);
        let (sidecar_reader, mut sidecar_writer) = tokio::io::split(sidecar_io);
        let control = Arc::new(NoopProcessControl {
            exited: AtomicBool::new(false),
            notify: Notify::new(),
        });
        let supervisor = Arc::new(BrowserSupervisor::from_io(
            BufReader::new(client_reader),
            client_writer,
            control,
            SupervisorTimeouts {
                request: Duration::from_secs(5),
                shutdown: Duration::from_millis(200),
                exit_grace: Duration::from_millis(200),
            },
        ));
        let health_calls = Arc::new(AtomicUsize::new(0));
        let health_calls_sidecar = health_calls.clone();
        let evaluate_calls = Arc::new(AtomicUsize::new(0));
        let evaluate_calls_sidecar = evaluate_calls.clone();

        let sidecar_task = tokio::spawn(async move {
            let mut reader = BufReader::new(sidecar_reader);
            let mut current_url = String::new();
            let mut line = String::new();
            loop {
                line.clear();
                let read = reader.read_line(&mut line).await.unwrap_or(0);
                if read == 0 {
                    break;
                }
                let request: Value = serde_json::from_str(&line).unwrap();
                let id = request["id"].clone();
                let command = request["command"].as_str().unwrap_or("");
                let result = match command {
                    "runtime_status" | "ensure_runtime" => json!({ "installed": true }),
                    "launch" => json!({ "launched": true }),
                    "navigate" => {
                        current_url = request["payload"]["url"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string();
                        json!({ "url": current_url, "title": "Google Maps" })
                    }
                    "health_check" => {
                        health_calls_sidecar.fetch_add(1, Ordering::SeqCst);
                        // First two places return the explicit sidecar
                        // `place_unavailable` state (empty <h1> / can't find on
                        // `/maps/place/`); the third exercises the defensive
                        // `empty_results` fallback mapping.
                        let state = if current_url.contains("ChIJgone3") {
                            "empty_results"
                        } else {
                            "place_unavailable"
                        };
                        json!({
                            "state": state,
                            "url": current_url,
                            "title": "Google Maps"
                        })
                    }
                    "evaluate" => {
                        evaluate_calls_sidecar.fetch_add(1, Ordering::SeqCst);
                        json!({ "value": null })
                    }
                    "shutdown" => json!({ "stopped": true }),
                    other => panic!("unexpected sidecar command: {other}"),
                };
                let response = json!({ "id": id, "ok": true, "result": result });
                sidecar_writer
                    .write_all(format!("{response}\n").as_bytes())
                    .await
                    .unwrap();
                sidecar_writer.flush().await.unwrap();
                if command == "shutdown" {
                    break;
                }
            }
        });

        struct SidecarPlaceProcessor {
            manager: Arc<BrowserManager>,
            visited: Mutex<Vec<String>>,
        }

        #[async_trait]
        impl ScrapeUnitProcessor for SidecarPlaceProcessor {
            async fn execute(
                &self,
                _job: &ScrapeJob,
                unit: &ScrapeUnit,
            ) -> Result<UnitExecution, ScrapeJobError> {
                self.visited.lock().unwrap().push(unit.unit_key.clone());
                let session = self.manager.acquire().await?;
                let mut scraper = GoogleMapsScraper::new(session);
                scraper.min_delay_ms = 0;
                scraper.max_delay_ms = 0;
                scraper.ready_delay_ms = 0;
                let place_url = unit
                    .payload
                    .get("place_url")
                    .and_then(Value::as_str)
                    .expect("place_url payload");
                let _ = scraper.scrape_place(place_url).await?;
                Ok(UnitExecution::default())
            }

            async fn action_health(
                &self,
                _job: &ScrapeJob,
            ) -> Result<GoogleMapsPageHealth, ScrapeJobError> {
                Ok(GoogleMapsPageHealth {
                    state: GoogleMapsPageState::Ready,
                    url: String::new(),
                    title: String::new(),
                })
            }

            async fn relaunch_for_action(&self, _job: &ScrapeJob) -> Result<(), ScrapeJobError> {
                Ok(())
            }

            async fn shutdown(&self) -> Result<(), ScrapeJobError> {
                Ok(())
            }
        }

        let manager = Arc::new(BrowserManager::with_automation(1, supervisor));
        manager.start(true).await.unwrap();
        let processor = Arc::new(SidecarPlaceProcessor {
            manager: manager.clone(),
            visited: Mutex::new(Vec::new()),
        });

        let db_path = std::env::temp_dir().join(format!(
            "rustsale-c10-sidecar-{}-{}.db",
            std::process::id(),
            rand::random::<u64>()
        ));
        let repo =
            Arc::new(ScrapeJobRepository::new(db_path.to_string_lossy().to_string()).unwrap());
        let now = chrono::Utc::now().to_rfc3339();
        let job = ScrapeJob {
            id: "c10-dead-places".to_string(),
            mode: ScrapeMode::Keyword,
            keyword: "cafe".to_string(),
            location: Some("District 1".to_string()),
            bbox: None,
            step_km: None,
            headless: true,
            profile_dir: "/tmp/rustsale-c10-test-profile".to_string(),
            blocked_url: None,
            status: ScrapeJobStatus::Queued,
            stage: ScrapeStage::Scraping,
            total_units: 3,
            processed_units: 0,
            failed_units: 0,
            businesses_discovered: 0,
            businesses_qualified: 0,
            leads_created: 0,
            last_error_kind: None,
            last_error_message: None,
            action_required: None,
            event_sequence: 0,
            version: 0,
            run_generation: 0,
            created_at: now.clone(),
            started_at: None,
            updated_at: now.clone(),
            completed_at: None,
            coverage_truncated: false,
        };
        let units = ["ChIJgone1", "ChIJgone2", "ChIJgone3"]
            .into_iter()
            .enumerate()
            .map(|(index, place_id)| ScrapeUnit {
                id: format!("u{}", index + 1),
                job_id: job.id.clone(),
                unit_key: format!("p{}", index + 1),
                unit_type: "place".to_string(),
                payload: json!({
                    "place_url": format!("https://www.google.com/maps/place/?q=place_id:{place_id}")
                }),
                status: ScrapeUnitStatus::Pending,
                depth: 0,
                attempt_count: 0,
                next_retry_at: None,
                last_error_kind: None,
                last_error_message: None,
                created_at: now.clone(),
                updated_at: now.clone(),
                completed_at: None,
            })
            .collect();
        repo.create_job_with_units(job.clone(), units)
            .await
            .unwrap();

        let runner = Arc::new(ScrapeJobRunner::new(
            repo.clone(),
            processor.clone(),
            Arc::new(ScrapeEventPublisher::new(64)),
            RetryPolicy {
                max_retries: 3,
                base_delay: Duration::from_millis(2),
                failure_tolerance: FailureTolerance {
                    max_consecutive_failures: 2,
                    ..FailureTolerance::default()
                },
            },
        ));

        runner.start(&job.id).await.unwrap();
        runner.wait_for_idle(Duration::from_secs(5)).await.unwrap();

        assert_eq!(
            processor.visited.lock().unwrap().as_slice(),
            ["p1", "p2", "p3"],
            "all 3 dead places must be visited even with max_consecutive_failures=2"
        );
        let stored_job = repo.get_job(&job.id).await.unwrap().unwrap();
        assert_eq!(stored_job.status, ScrapeJobStatus::Partial);
        assert_eq!(stored_job.processed_units, 3);
        assert_eq!(stored_job.failed_units, 3);
        for stored_unit in repo.list_units(&job.id).await.unwrap() {
            assert_eq!(stored_unit.status, ScrapeUnitStatus::Failed);
            assert_eq!(
                stored_unit.attempt_count, 1,
                "PlaceUnavailable must not retry"
            );
            assert_eq!(
                stored_unit.last_error_kind,
                Some(ScrapeErrorKind::PlaceUnavailable)
            );
        }
        // 4 settle polls for each `place_unavailable` place + 1 immediate poll
        // for `empty_results` = 9 polls total (vs 80+ polls under 20 s SelectorChanged).
        assert_eq!(health_calls.load(Ordering::SeqCst), 9);
        assert_eq!(
            evaluate_calls.load(Ordering::SeqCst),
            0,
            "extract script must not run on an unavailable place"
        );

        let _ = manager.close().await;
        let _ = sidecar_task.await;
        let _ = std::fs::remove_file(&db_path);
    }

    /// A block on one place must surface as that block, so the runner can pause
    /// for action without the places already checkpointed being re-scraped.
    #[tokio::test]
    async fn place_detail_captcha_surfaces_as_captcha() {
        let (scraper, automation) = scripted_scraper(vec![
            Step::Navigate(Ok(())),
            Step::Health(Ok(GoogleMapsPageState::Captcha)),
        ])
        .await;

        let error = scraper
            .scrape_place("https://www.google.com/maps/place/Cafe-Two")
            .await
            .unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::Captcha);
        automation.assert_exhausted();
    }

    /// `!3d`/`!4d` are the place's own coordinates; `@lat,lng` is only the map
    /// viewport centre, which varies with the grid cell the place was reached
    /// from. Spatial qualification must use the authoritative pair.
    #[test]
    fn place_coordinates_prefer_the_authoritative_data_markers() {
        let url = "https://www.google.com/maps/place/Cafe/@10.5000000,106.5000000,17z/data=!3m1!4b1!4m6!3m5!1s0x317529abcd:0x1234!8m2!3d10.7654321!4d106.7654321";

        let (latitude, longitude) = coordinates_from_url(url);

        assert_eq!(latitude, Some(10.7654321));
        assert_eq!(longitude, Some(106.7654321));
    }

    /// The same place reached from two grid cells yields two different viewport
    /// URLs, so identity must survive the viewport changing.
    #[test]
    fn place_identity_is_stable_across_entry_viewports() {
        let from_cell_a = "https://www.google.com/maps/place/Cafe+Sam/@10.7000000,106.7000000,17z/data=!4m6!3m5!1s0xCCC!8m2!3d10.7654321!4d106.7654321";
        let from_cell_b = "https://www.google.com/maps/place/Cafe+Sam/@10.7900000,106.6900000,15z/data=!4m6!3m5!1s0xCCC!8m2!3d10.7654321!4d106.7654321";

        assert_eq!(place_identity(from_cell_a), place_identity(from_cell_b));
    }

    /// The name slug is not an identity: `/maps/place/Starbucks` is every
    /// Starbucks. Collapsing two branches onto one key silently discards a
    /// business, because the place unit insert ignores duplicates.
    #[test]
    fn place_identity_distinguishes_branches_that_share_a_name_slug() {
        let district_one = "https://www.google.com/maps/place/Starbucks/@10.7000000,106.7000000,17z/data=!4m6!3m5!1s0xAAA!8m2!3d10.7710000!4d106.6980000";
        let district_three = "https://www.google.com/maps/place/Starbucks/@10.7900000,106.6900000,17z/data=!4m6!3m5!1s0xBBB!8m2!3d10.8010000!4d106.7120000";

        assert_ne!(place_identity(district_one), place_identity(district_three));
    }

    /// Without a feature id the authoritative coordinates still separate two
    /// branches, and still survive a viewport change.
    #[test]
    fn place_identity_falls_back_to_authoritative_coordinates() {
        let district_one = "https://www.google.com/maps/place/Starbucks/@10.7000000,106.7000000,17z/data=!8m2!3d10.7710000!4d106.6980000";
        let district_three = "https://www.google.com/maps/place/Starbucks/@10.7900000,106.6900000,17z/data=!8m2!3d10.8010000!4d106.7120000";
        let district_one_zoomed_out = "https://www.google.com/maps/place/Starbucks/@10.6000000,106.6000000,12z/data=!8m2!3d10.7710000!4d106.6980000";

        assert_ne!(place_identity(district_one), place_identity(district_three));
        assert_eq!(
            place_identity(district_one),
            place_identity(district_one_zoomed_out)
        );
    }

    /// With neither a feature id nor coordinates the URL still must not be
    /// reduced to its slug; only the viewport segment may be dropped.
    #[test]
    fn place_identity_never_reduces_to_the_bare_name_slug() {
        let first =
            "https://www.google.com/maps/place/Starbucks/@10.7,106.7,17z/data=!4m2!3m1!9sfirst";
        let second =
            "https://www.google.com/maps/place/Starbucks/@10.7,106.7,17z/data=!4m2!3m1!9ssecond";

        assert_ne!(place_identity(first), place_identity(second));
    }

    /// When the viewport is the *only* thing distinguishing two URLs, dropping
    /// it leaves the bare slug and merges two businesses. With no discriminator
    /// left, identity must stay conservative: a duplicate unit is recoverable,
    /// a silently discarded business is not.
    #[test]
    fn place_identity_keeps_the_viewport_when_it_is_the_only_discriminator() {
        let district_one =
            "https://www.google.com/maps/place/Starbucks/@10.7710000,106.6980000,17z";
        let district_three =
            "https://www.google.com/maps/place/Starbucks/@10.8010000,106.7120000,17z";

        assert_ne!(place_identity(district_one), place_identity(district_three));
    }

    /// The same place can be listed with and without its feature id. Those are
    /// aliases of one business, and planning must recognise the second form as
    /// already known rather than scheduling it twice.
    #[test]
    fn place_identity_aliases_link_the_feature_id_and_coordinate_forms() {
        let with_feature_id = "https://www.google.com/maps/place/Cafe+Sam/@10.70,106.70,17z/data=!4m6!3m5!1s0xCCC!8m2!3d10.7654321!4d106.7654321";
        let without_feature_id = "https://www.google.com/maps/place/Cafe+Sam/@10.79,106.69,15z/data=!8m2!3d10.7654321!4d106.7654321";

        let with_aliases = place_identity_aliases(with_feature_id);
        let without_aliases = place_identity_aliases(without_feature_id);

        assert_eq!(
            with_aliases.first().unwrap(),
            &place_identity(with_feature_id)
        );
        assert!(
            with_aliases
                .iter()
                .any(|alias| without_aliases.contains(alias)),
            "the two listings of one business must share at least one alias"
        );
    }

    /// The persisted result key and the place unit key must be the same
    /// identity, otherwise a place can be scheduled once and stored twice.
    #[test]
    fn candidate_dedup_key_agrees_with_place_identity() {
        let url = "https://www.google.com/maps/place/Cafe+Sam/@10.70,106.70,17z/data=!4m6!3m5!1s0xCCC!8m2!3d10.77!4d106.69";
        let candidate = PlaceCandidate {
            place_id: None,
            canonical_url: url.to_string(),
            name: "Cafe Sam".to_string(),
            phone: None,
            address: None,
            website: None,
            facebook: None,
            zalo: None,
            latitude: None,
            longitude: None,
        };

        assert_eq!(candidate_dedup_key(&candidate), place_identity(url));
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
    #[ignore = "needs real cloak-runtime sidecar; run with --include-ignored"]
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

    #[tokio::test]
    #[ignore = "needs real cloak-runtime sidecar; run with --include-ignored"]
    async fn real_sidecar_dead_place_empty_h1_reports_place_unavailable() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut buf = [0; 4096];
                    if let Ok(n) = socket.read(&mut buf).await {
                        let request = String::from_utf8_lossy(&buf[..n]);
                        let body = if request.contains("ChIJnotfoundtext") {
                            r#"<!doctype html><html><body><div role="main"><h1 class="sr-only"> </h1><button aria-label="Zoom in">+</button><p>Google Maps can't find ChIJnotfoundtext</p></div></body></html>"#
                        } else {
                            r#"<!doctype html><html><body><div role="main"><h1 class="sr-only"> </h1><button aria-label="Zoom in">+</button></div></body></html>"#
                        };
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = socket.write_all(response.as_bytes()).await;
                    }
                });
            }
        });

        let manager = BrowserManager::new(1);
        manager.start(true).await.expect("Failed to start browser");
        let session = manager.acquire().await.expect("Failed to acquire session");
        let mut scraper = GoogleMapsScraper::new(session);
        scraper.min_delay_ms = 0;
        scraper.max_delay_ms = 0;

        for place_id in ["ChIJdoesnotexist", "ChIJnotfoundtext"] {
            let url = format!("http://127.0.0.1:{port}/maps/place/?q=place_id:{place_id}");
            let error = scraper.scrape_place(&url).await.unwrap_err();
            assert_eq!(error.kind, ScrapeErrorKind::PlaceUnavailable, "{error:?}");
            assert!(!error.retryable, "{error:?}");
        }

        let _ = manager.close().await;
    }
}

#[cfg(test)]
mod extract_script_tests {
    use super::EXTRACT_PLACE_SCRIPT;
    use serde_json::{json, Value};
    use std::io::Write;
    use std::process::{Command, Stdio};

    /// Evaluates the place-extraction script against a tiny fake DOM (no real
    /// browser needed). Elements are `{tag, attrs, text}`.
    fn run_extract(elements: Value) -> Value {
        let harness = format!(
            r#"
const elements = {elements};
const matches = (el, sel) => sel.split(',').some((raw) => {{
  const s = raw.trim();
  let m;
  if ((m = s.match(/^\[data-item-id="(.+)"\]$/))) return el.attrs['data-item-id'] === m[1];
  if ((m = s.match(/^\[data-item-id\^="(.+)"\]$/))) return (el.attrs['data-item-id'] || '').startsWith(m[1]);
  if ((m = s.match(/^a\[href\^="(.+)"\]$/))) return el.tag === 'a' && (el.attrs.href || '').startsWith(m[1]);
  if (s === 'a[href]') return el.tag === 'a' && el.attrs.href !== undefined;
  if ((m = s.match(/^\[data-item-id="(.+)"\]\[aria-label\]$/))) return el.attrs['data-item-id'] === m[1] && el.attrs['aria-label'] !== undefined;
  if ((m = s.match(/^(\w+)\[aria-label\]$/))) return el.tag === m[1] && el.attrs['aria-label'] !== undefined;
  if (s.startsWith('.')) return false;
  return el.tag === s;
}});
const wrap = (el) => el && ({{
  textContent: el.text || '',
  getAttribute: (name) => (name in el.attrs ? el.attrs[name] : null),
}});
globalThis.window = {{ location: {{ href: 'https://www.google.com/maps/place/Fixture/@10.0,106.0,17z/data=!1sfixture!3d10.0!4d106.0' }} }};
globalThis.document = {{
  querySelector: (sel) => wrap(elements.find((el) => matches(el, sel))),
  querySelectorAll: (sel) => elements.filter((el) => matches(el, sel)).map(wrap),
}};
console.log(JSON.stringify({script}));
"#,
            elements = elements,
            script = EXTRACT_PLACE_SCRIPT.trim(),
        );
        let runtime = ["node", "bun"]
            .into_iter()
            .find(|name| {
                Command::new(name)
                    .arg("--version")
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .is_ok_and(|status| status.success())
            })
            .expect("extract script tests need node or bun on PATH");
        let mut child = Command::new(runtime)
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to spawn script runtime");
        child
            .stdin
            .take()
            .expect("stdin is piped")
            .write_all(harness.as_bytes())
            .expect("failed to write harness");
        let output = child.wait_with_output().expect("script runtime failed");
        assert!(output.status.success(), "script harness failed");
        serde_json::from_slice(&output.stdout).expect("script printed JSON")
    }

    #[test]
    fn google_maps_script_does_not_use_arbitrary_external_link_as_website() {
        let result = run_extract(json!([
            {"tag": "h1", "attrs": {}, "text": "Cafe Fixture"},
            {"tag": "a", "attrs": {"href": "https://booking.example/reserve/123"}, "text": "Reserve"},
            {"tag": "a", "attrs": {"href": "https://delivery.example/menu"}, "text": "Order"}
        ]));
        assert_eq!(result["name"], "Cafe Fixture");
        assert_eq!(result["website"], Value::Null);
    }

    #[test]
    fn phone_fallback_ignores_numbers_outside_info_panel() {
        // A suggestion card for another place shows its own number as plain text.
        let result = run_extract(json!([
            {"tag": "h1", "attrs": {}, "text": "Cafe Fixture"},
            {"tag": "div", "attrs": {}, "text": "0912 345 678"},
            {"tag": "span", "attrs": {}, "text": "0987654321"}
        ]));
        assert_eq!(result["phone"], Value::Null);
    }

    #[test]
    fn phone_fallback_reads_the_labelled_phone_button() {
        let result = run_extract(json!([
            {"tag": "h1", "attrs": {}, "text": "Cafe Fixture"},
            {"tag": "div", "attrs": {}, "text": "0912 345 678"},
            {"tag": "button", "attrs": {"aria-label": "Phone: 0901 234 567"}, "text": "0901 234 567"}
        ]));
        assert_eq!(result["phone"], "0901 234 567");
    }

    #[test]
    fn clean_optional_strips_private_use_glyphs() {
        assert_eq!(
            super::clean_optional(Some("\u{e0c8} 12 Le Loi\u{f8ff} ".to_string())),
            Some("12 Le Loi".to_string())
        );
        assert_eq!(super::clean_optional(Some("\u{e0c8}".to_string())), None);
    }

    #[test]
    fn clean_place_name_rejoins_maps_glyph_spacing() {
        assert_eq!(
            super::clean_place_name("\u{e0c8} r ê u . coffee"),
            "rêu.coffee"
        );
    }

    #[test]
    fn clean_place_name_keeps_ordinary_word_spacing() {
        assert_eq!(
            super::clean_place_name("Phúc Long Coffee & Tea"),
            "Phúc Long Coffee & Tea"
        );
    }

    #[test]
    fn clean_place_name_keeps_decomposed_vietnamese_glyphs_together() {
        assert_eq!(
            super::clean_place_name("Phu\u{0301}c Long Coffee & Tea"),
            "Phu\u{0301}c Long Coffee & Tea"
        );
        assert_eq!(
            super::clean_place_name("r e\u{0302} u . coffee"),
            "re\u{0302}u.coffee"
        );
    }

    #[test]
    fn address_prefers_aria_label_without_icon_glyph() {
        let result = run_extract(json!([
            {"tag": "h1", "attrs": {}, "text": "Cafe Fixture"},
            {"tag": "button", "attrs": {"data-item-id": "address", "aria-label": "Address: 12 Le Loi, District 1"}, "text": "\u{e0c8}\n12 Le Loi, District 1"}
        ]));
        assert_eq!(result["address"], "12 Le Loi, District 1");
    }

    #[test]
    fn address_aria_label_prefix_is_removed_in_vietnamese_too() {
        let result = run_extract(json!([
            {"tag": "h1", "attrs": {}, "text": "Cafe Fixture"},
            {"tag": "button", "attrs": {"data-item-id": "address", "aria-label": "Địa chỉ: 12 Lê Lợi, Quận 1"}, "text": "12 Lê Lợi, Quận 1"}
        ]));
        assert_eq!(result["address"], "12 Lê Lợi, Quận 1");
    }

    #[test]
    fn google_maps_script_keeps_the_authority_website() {
        let result = run_extract(json!([
            {"tag": "h1", "attrs": {}, "text": "Cafe Fixture"},
            {"tag": "a", "attrs": {"data-item-id": "authority", "href": "https://cafe.example"}, "text": "cafe.example"},
            {"tag": "a", "attrs": {"href": "https://booking.example/reserve/123"}, "text": "Reserve"}
        ]));
        assert_eq!(result["website"], "https://cafe.example");
    }
}
