use crate::google_maps_jobs::email::{EmailEnricher, EnrichmentOutcome};
use crate::google_maps_jobs::events::ScrapeEventPublisher;
use crate::google_maps_jobs::importer::normalize_vn_mobile;
use crate::google_maps_jobs::repository::ScrapeJobRepository;
use crate::google_maps_jobs::runner::{FinalizeOutcome, ScrapeUnitProcessor, UnitExecution};
use crate::google_maps_jobs::types::{
    ScrapeErrorKind, ScrapeJob, ScrapeJobError, ScrapeResult, ScrapeStage, ScrapeUnit,
    ScrapeUnitStatus,
};
use crate::scrapers::browser::{BrowserManager, BrowserSession, GoogleMapsPageHealth};
use crate::scrapers::google_maps::{
    candidate_matches_requested_area, place_identity, place_identity_aliases, GoogleMapsScraper,
    PlaceCandidate, SearchListing,
};
use crate::scrapers::grid::{
    grid_budget_exhausted, radius_to_zoom, should_subdivide, subdivide_cell, BoundingBox, GridCell,
    GridLimits,
};

use async_trait::async_trait;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use url::Url;

/// Raw listings on one Google Maps search page above which the page is assumed
/// to be truncated by ranking rather than by the absence of further results.
const GRID_SATURATION_THRESHOLD: usize = 55;

struct ActiveBrowser {
    job_id: String,
    manager: Arc<BrowserManager>,
}

pub struct GoogleMapsUnitProcessor {
    repository: Arc<ScrapeJobRepository>,
    events: Arc<ScrapeEventPublisher>,
    active_browser: Mutex<Option<ActiveBrowser>>,
}

impl GoogleMapsUnitProcessor {
    pub fn new(repository: Arc<ScrapeJobRepository>, events: Arc<ScrapeEventPublisher>) -> Self {
        Self {
            repository,
            events,
            active_browser: Mutex::new(None),
        }
    }

    async fn browser_for(&self, job: &ScrapeJob) -> Result<Arc<BrowserManager>, ScrapeJobError> {
        let mut active = self.active_browser.lock().await;
        if active
            .as_ref()
            .is_some_and(|active| active.job_id != job.id)
        {
            if let Some(previous) = active.take() {
                previous.manager.close().await?;
            }
        }
        if let Some(existing) = active.as_ref() {
            return Ok(existing.manager.clone());
        }
        let manager =
            Arc::new(BrowserManager::new(1).with_profile_dir(PathBuf::from(&job.profile_dir)));
        *active = Some(ActiveBrowser {
            job_id: job.id.clone(),
            manager: manager.clone(),
        });
        Ok(manager)
    }

    async fn execute_search(
        &self,
        job: &ScrapeJob,
        unit: &ScrapeUnit,
    ) -> Result<UnitExecution, ScrapeJobError> {
        let manager = self.browser_for(job).await?;
        if unit.attempt_count > 1 || manager.acquire().await.is_err() {
            manager.start(job.headless).await?;
        }
        let session = manager.acquire().await?;
        let scraper = GoogleMapsScraper::new(session);

        if unit.unit_type == "place" {
            let place_url = unit
                .payload
                .get("place_url")
                .and_then(|value| value.as_str())
                .ok_or_else(|| invalid_response("place unit payload omitted place_url"))?;
            let candidate = match scraper.scrape_place(place_url).await {
                Ok(candidate) => candidate,
                Err(error) => {
                    return Err(self
                        .handle_scrape_error(&scraper, &manager, job, unit, error)
                        .await)
                }
            };
            return Ok(UnitExecution {
                results: vec![candidate_to_result(job, unit, candidate, 0)],
                child_units: Vec::new(),
                budget_exhausted: false,
            });
        }

        let url = match unit.unit_type.as_str() {
            "keyword" | "district" => {
                let location = unit
                    .payload
                    .get("location")
                    .and_then(|value| value.as_str());
                scraper.search_url(&job.keyword, location)
            }
            "grid" => {
                let cell: GridCell =
                    serde_json::from_value(unit.payload.clone()).map_err(|error| {
                        invalid_response(format!("invalid grid unit payload: {error}"))
                    })?;
                scraper.search_region_url(
                    &job.keyword,
                    cell.center_lat,
                    cell.center_lng,
                    radius_to_zoom(cell.radius_m),
                )
            }
            other => {
                return Err(invalid_response(format!(
                    "unsupported scrape unit type {other}"
                )))
            }
        };

        let listing = match scraper.collect_search_listing(&url).await {
            Ok(listing) => listing,
            Err(error) => {
                return Err(self
                    .handle_scrape_error(&scraper, &manager, job, unit, error)
                    .await)
            }
        };
        if let Some(kind) = listing.blocked_reason {
            return Err(capture_scrape_diagnostics(
                &scraper.session,
                job,
                unit,
                ScrapeJobError {
                    kind,
                    message: "Google Maps requires user action".to_string(),
                    // A throttle is transient and is retried with backoff before
                    // a human is asked to intervene; a CAPTCHA never is.
                    retryable: kind == ScrapeErrorKind::RateLimited,
                },
            )
            .await);
        }
        self.listing_to_execution(job, unit, listing).await
    }

    async fn handle_scrape_error(
        &self,
        scraper: &GoogleMapsScraper,
        manager: &BrowserManager,
        job: &ScrapeJob,
        unit: &ScrapeUnit,
        error: ScrapeJobError,
    ) -> ScrapeJobError {
        let error = capture_scrape_diagnostics(&scraper.session, job, unit, error).await;
        if error.retryable {
            if let Err(close_error) = manager.close().await {
                eprintln!("failed to close browser after retryable scrape error: {close_error}");
            }
        }
        error
    }

    async fn listing_to_execution(
        &self,
        job: &ScrapeJob,
        unit: &ScrapeUnit,
        listing: SearchListing,
    ) -> Result<UnitExecution, ScrapeJobError> {
        // Novelty is measured against work already *planned*, not only work
        // already finished. A grid child can be claimed before its place
        // siblings run, so counting a scheduled-but-unscraped place as new would
        // make the low-yield heuristic subdivide a cell it has already covered.
        let mut existing = self
            .repository
            .list_result_dedup_keys(&job.id)
            .await
            .map_err(database_error)?
            .into_iter()
            .collect::<HashSet<_>>();
        existing.extend(
            self.repository
                .list_place_unit_keys(&job.id)
                .await
                .map_err(database_error)?,
        );
        // The search unit completes before any detail is fetched, so a place URL
        // has to carry the same identity the extracted candidate will.
        let page_keys = listing
            .place_urls
            .iter()
            .map(|url| place_identity(url))
            .collect::<HashSet<_>>();
        let new_unique = listing
            .place_urls
            .iter()
            .filter(|url| {
                !place_identity_aliases(url)
                    .iter()
                    .any(|alias| existing.contains(alias))
            })
            .map(|url| place_identity(url))
            .collect::<HashSet<_>>()
            .len();
        let new_unique_ratio = if page_keys.is_empty() {
            0.0
        } else {
            new_unique as f64 / page_keys.len() as f64
        };

        let mut child_units = plan_place_children(job, unit, &listing.place_urls, &existing)?;
        let mut budget_exhausted = false;
        if unit.unit_type == "grid" {
            let cell: GridCell = serde_json::from_value(unit.payload.clone())
                .map_err(|error| invalid_response(format!("invalid grid payload: {error}")))?;
            let grid_cells = self
                .repository
                .count_units_by_type(&job.id, "grid")
                .await
                .map_err(database_error)? as usize;
            child_units.extend(plan_grid_children(
                job,
                unit,
                &cell,
                listing.raw_listing_count,
                listing.reached_end,
                new_unique_ratio,
                grid_cells,
            )?);
            budget_exhausted = grid_budget_exhausted(
                &cell,
                listing.raw_listing_count,
                GRID_SATURATION_THRESHOLD,
                listing.reached_end,
                new_unique_ratio,
                grid_cells,
                GridLimits::default(),
            );
        }

        Ok(UnitExecution {
            results: Vec::new(),
            child_units,
            budget_exhausted,
        })
    }

    async fn finalize_job(
        &self,
        job: &ScrapeJob,
        cancellation: &CancellationToken,
    ) -> Result<FinalizeOutcome, ScrapeJobError> {
        let enrichment_stage = self
            .repository
            .set_stage(&job.id, ScrapeStage::EmailEnrichment)
            .await
            .map_err(database_error)?;
        self.events
            .publish_job(&enrichment_stage, "Email enrichment started")
            .await;
        let pending = self
            .repository
            .list_results_for_enrichment(&job.id)
            .await
            .map_err(database_error)?;
        let enricher = EmailEnricher::default();
        let mut unresolved = false;
        for result in pending {
            if cancellation.is_cancelled() {
                return Ok(FinalizeOutcome::default());
            }
            let Some(claimed) = self
                .repository
                .claim_result_for_enrichment(&job.id, &result.id)
                .await
                .map_err(database_error)?
            else {
                continue;
            };
            let outcome = match claimed
                .website
                .as_deref()
                .and_then(|value| Url::parse(value).ok())
            {
                Some(url) => {
                    let found = tokio::select! {
                        _ = cancellation.cancelled() => None,
                        found = enricher.find_email(url) => Some(found),
                    };
                    let Some(found) = found else {
                        // Paused or cancelled mid-request: hand the result
                        // back so a later run enriches it instead of leaving
                        // it stuck at `running` (A6).
                        self.repository
                            .release_enrichment_claim(&job.id, &claimed.id)
                            .await
                            .map_err(database_error)?;
                        return Ok(FinalizeOutcome::default());
                    };
                    match found {
                        Ok(Some(email)) => EnrichmentOutcome::found(email),
                        Ok(None) => EnrichmentOutcome::not_found(),
                        Err(error) => EnrichmentOutcome::from_error(error),
                    }
                }
                None => EnrichmentOutcome::from_error(invalid_response(
                    "enrichment website URL is invalid",
                )),
            };
            unresolved |= enrichment_is_unresolved(&outcome);
            self.repository
                .save_enrichment_outcome(&job.id, &claimed.id, outcome)
                .await
                .map_err(database_error)?;
        }

        if cancellation.is_cancelled() {
            return Ok(FinalizeOutcome::default());
        }
        let import_stage = self
            .repository
            .set_stage(&job.id, ScrapeStage::Importing)
            .await
            .map_err(database_error)?;
        self.events
            .publish_job(&import_stage, "Lead import started")
            .await;
        self.repository
            .import_pending_results(&job.id)
            .await
            .map_err(database_error)?;
        Ok(FinalizeOutcome {
            partial: unresolved,
        })
    }
}

#[async_trait]
impl ScrapeUnitProcessor for GoogleMapsUnitProcessor {
    async fn execute(
        &self,
        job: &ScrapeJob,
        unit: &ScrapeUnit,
    ) -> Result<UnitExecution, ScrapeJobError> {
        self.execute_search(job, unit).await
    }

    async fn action_health(&self, job: &ScrapeJob) -> Result<GoogleMapsPageHealth, ScrapeJobError> {
        let manager = self.browser_for(job).await?;
        manager.acquire().await?.health_snapshot().await
    }

    async fn relaunch_for_action(&self, job: &ScrapeJob) -> Result<(), ScrapeJobError> {
        // The URL can be missing when the page could not be probed before the
        // browser closed (A1); reopen Maps so the user can still resolve it.
        let blocked_url = job.blocked_url.as_deref().unwrap_or(DEFAULT_ACTION_URL);
        self.browser_for(job)
            .await?
            .relaunch_for_action(blocked_url)
            .await
    }

    async fn shutdown(&self) -> Result<(), ScrapeJobError> {
        if let Some(active) = self.active_browser.lock().await.as_ref() {
            active.manager.close().await?;
        }
        Ok(())
    }

    async fn finalize(
        &self,
        job: &ScrapeJob,
        cancellation: &CancellationToken,
    ) -> Result<FinalizeOutcome, ScrapeJobError> {
        self.finalize_job(job, cancellation).await
    }
}

/// Only an enrichment that can still succeed on retry keeps the job partial.
/// A permanent failure (dead site, 403, DNS) cannot be fixed by retrying (B9).
fn enrichment_is_unresolved(outcome: &EnrichmentOutcome) -> bool {
    outcome.error.as_ref().is_some_and(|error| error.retryable)
}

const DEFAULT_ACTION_URL: &str = "https://www.google.com/maps";

async fn capture_scrape_diagnostics(
    session: &BrowserSession,
    job: &ScrapeJob,
    unit: &ScrapeUnit,
    mut error: ScrapeJobError,
) -> ScrapeJobError {
    if !matches!(
        error.kind,
        ScrapeErrorKind::Captcha
            | ScrapeErrorKind::ConsentRequired
            | ScrapeErrorKind::RateLimited
            | ScrapeErrorKind::SelectorChanged
    ) {
        return error;
    }

    let profile_dir = PathBuf::from(&job.profile_dir);
    let job_dir = profile_dir
        .parent()
        .map(PathBuf::from)
        .unwrap_or(profile_dir);
    let diagnostic_dir = job_dir.join("diagnostics");
    if let Err(capture_error) = std::fs::create_dir_all(&diagnostic_dir) {
        error.message.push_str(&format!(
            "; diagnostic_capture_error=failed to create {}: {capture_error}",
            diagnostic_dir.display()
        ));
        return error;
    }

    let unit_component: String = unit
        .id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let error_component = format!("{:?}", error.kind).to_ascii_lowercase();
    let screenshot_path = diagnostic_dir.join(format!(
        "{unit_component}-attempt-{}-{error_component}.png",
        unit.attempt_count
    ));

    match session.health_snapshot().await {
        Ok(health) if !health.url.trim().is_empty() => error
            .message
            .push_str(&format!("; diagnostic_url={}", health.url)),
        Ok(_) => {}
        Err(capture_error) => error
            .message
            .push_str(&format!("; diagnostic_url_error={}", capture_error.message)),
    }
    match session.screenshot(&screenshot_path).await {
        Ok(()) => error.message.push_str(&format!(
            "; diagnostic_screenshot={}",
            screenshot_path.display()
        )),
        Err(capture_error) => error.message.push_str(&format!(
            "; diagnostic_screenshot_error={}",
            capture_error.message
        )),
    }
    error
}

fn candidate_to_result(
    job: &ScrapeJob,
    unit: &ScrapeUnit,
    candidate: PlaceCandidate,
    index: usize,
) -> ScrapeResult {
    let now = chrono::Utc::now().to_rfc3339();
    let dedup_key = candidate_dedup_key(&candidate);
    let normalized_phone = candidate.phone.as_deref().and_then(normalize_vn_mobile);
    let inside_requested_area = Some(job.bbox.is_none_or(|bbox| {
        candidate_matches_requested_area(
            &candidate,
            BoundingBox {
                lat_min: bbox[0],
                lat_max: bbox[1],
                lng_min: bbox[2],
                lng_max: bbox[3],
            },
            job.location.as_deref(),
        )
    }));
    // Outside the requested area is a decision, not unfinished work. Leaving
    // these rows at `pending` stranded them forever and made the gap between
    // discovered and imported unattributable. Both queues carry the same
    // terminal reason so neither shows work that will never be done.
    let outside_requested_area = inside_requested_area == Some(false);
    let enrichment_status = if outside_requested_area {
        "skipped_outside_area"
    } else if normalized_phone.is_none() && candidate.website.is_some() {
        "pending"
    } else {
        "not_found"
    };
    let import_status = if outside_requested_area {
        "skipped_outside_area"
    } else {
        "pending"
    };
    ScrapeResult {
        id: format!("{}-result-{index:05}", unit.id),
        job_id: job.id.clone(),
        source_unit_id: unit.id.clone(),
        place_id: candidate.place_id,
        canonical_url: (!candidate.canonical_url.trim().is_empty())
            .then_some(candidate.canonical_url),
        dedup_key,
        name: candidate.name,
        phone: candidate.phone,
        normalized_phone,
        email: None,
        normalized_email: None,
        address: candidate.address,
        website: candidate.website,
        facebook: candidate.facebook,
        zalo: candidate.zalo,
        latitude: candidate.latitude,
        longitude: candidate.longitude,
        inside_requested_area,
        detail_status: "completed".to_string(),
        enrichment_status: enrichment_status.to_string(),
        import_status: import_status.to_string(),
        last_error_kind: None,
        last_error_message: None,
        first_seen_at: now.clone(),
        last_seen_at: now,
    }
}

fn candidate_dedup_key(candidate: &PlaceCandidate) -> String {
    if let Some(place_id) = candidate.place_id.as_deref() {
        return format!("place:{place_id}");
    }
    if !candidate.canonical_url.trim().is_empty() {
        return place_identity(&candidate.canonical_url);
    }
    if let Some(phone) = candidate.phone.as_deref().and_then(normalize_vn_mobile) {
        return format!("phone:{phone}");
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

/// Turns the place URLs of one search page into individually checkpointed
/// units.
///
/// Each place then carries its own attempt count, retry schedule, and
/// completion record, so a block or a malformed listing costs exactly one
/// place instead of the whole search page's work.
fn plan_place_children(
    job: &ScrapeJob,
    unit: &ScrapeUnit,
    place_urls: &[String],
    known: &HashSet<String>,
) -> Result<Vec<ScrapeUnit>, ScrapeJobError> {
    let now = chrono::Utc::now().to_rfc3339();
    Ok(place_urls
        .iter()
        .filter(|place_url| {
            // Compared against every alias, not just the primary key: the same
            // business can be listed with its feature id in one cell and with
            // only its coordinates in another.
            !place_identity_aliases(place_url)
                .iter()
                .any(|alias| known.contains(alias))
        })
        .enumerate()
        .map(|(index, place_url)| ScrapeUnit {
            id: format!("{}-place-{index:05}", unit.id),
            job_id: job.id.clone(),
            unit_key: place_identity(place_url),
            unit_type: "place".to_string(),
            payload: serde_json::json!({"place_url": place_url}),
            status: ScrapeUnitStatus::Pending,
            depth: unit.depth,
            attempt_count: 0,
            next_retry_at: None,
            last_error_kind: None,
            last_error_message: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            completed_at: None,
        })
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn plan_grid_children(
    job: &ScrapeJob,
    unit: &ScrapeUnit,
    cell: &GridCell,
    raw_listing_count: usize,
    reached_end: bool,
    new_unique_ratio: f64,
    grid_cells: usize,
) -> Result<Vec<ScrapeUnit>, ScrapeJobError> {
    if !should_subdivide(
        cell,
        raw_listing_count,
        GRID_SATURATION_THRESHOLD,
        reached_end,
        new_unique_ratio,
        grid_cells,
        GridLimits::default(),
    ) {
        return Ok(Vec::new());
    }
    let now = chrono::Utc::now().to_rfc3339();
    subdivide_cell(cell, new_unique_ratio)
        .into_iter()
        .enumerate()
        .map(|(index, mut child)| {
            child.parent_unit_id = Some(unit.id.clone());
            Ok(ScrapeUnit {
                id: format!("{}-child-{}-{index}", unit.id, child.depth),
                job_id: job.id.clone(),
                unit_key: format!(
                    "grid:{:.7}:{:.7}:{:.7}:{:.7}:{}",
                    child.bbox.lat_min,
                    child.bbox.lat_max,
                    child.bbox.lng_min,
                    child.bbox.lng_max,
                    child.depth
                ),
                unit_type: "grid".to_string(),
                payload: serde_json::to_value(&child)
                    .map_err(|error| invalid_response(format!("invalid child grid: {error}")))?,
                status: ScrapeUnitStatus::Pending,
                depth: child.depth,
                attempt_count: 0,
                next_retry_at: None,
                last_error_kind: None,
                last_error_message: None,
                created_at: now.clone(),
                updated_at: now.clone(),
                completed_at: None,
            })
        })
        .collect()
}

fn invalid_response(message: impl Into<String>) -> ScrapeJobError {
    ScrapeJobError {
        kind: ScrapeErrorKind::InvalidResponse,
        message: message.into(),
        retryable: false,
    }
}

fn database_error(error: impl std::fmt::Display) -> ScrapeJobError {
    ScrapeJobError {
        kind: ScrapeErrorKind::Database,
        message: error.to_string(),
        retryable: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::google_maps_jobs::types::{ScrapeJobStatus, ScrapeMode, ScrapeStage};
    use crate::scrapers::browser::{
        BrowserAutomation, BrowserSession, GoogleMapsPageHealth, GoogleMapsPageState,
        LaunchRequest, RuntimeStatus,
    };
    use crate::scrapers::google_maps::PlaceCandidate;
    use crate::scrapers::grid::{BoundingBox, GridCell};
    use serde_json::Value;
    use std::path::Path;
    use std::time::Duration;

    /// B9: a permanent enrichment error must not keep the job partial forever.
    #[test]
    fn finalize_job_completes_when_only_permanent_enrichment_errors() {
        let permanent = EnrichmentOutcome::from_error(ScrapeJobError {
            kind: ScrapeErrorKind::InvalidResponse,
            message: "enrichment HTTP status 403".to_string(),
            retryable: false,
        });
        let transient = EnrichmentOutcome::from_error(ScrapeJobError {
            kind: ScrapeErrorKind::TransientNetwork,
            message: "connection reset".to_string(),
            retryable: true,
        });

        assert!(!enrichment_is_unresolved(&permanent));
        assert!(enrichment_is_unresolved(&transient));
        assert!(!enrichment_is_unresolved(&EnrichmentOutcome::not_found()));
    }

    struct DiagnosticAutomation {
        screenshot_paths: std::sync::Mutex<Vec<PathBuf>>,
    }

    #[async_trait]
    impl BrowserAutomation for DiagnosticAutomation {
        async fn runtime_status(&self) -> Result<RuntimeStatus, ScrapeJobError> {
            unreachable!()
        }

        async fn ensure_runtime(&self) -> Result<RuntimeStatus, ScrapeJobError> {
            unreachable!()
        }

        async fn launch(&self, _request: LaunchRequest) -> Result<(), ScrapeJobError> {
            unreachable!()
        }

        async fn navigate(&self, _url: &str, _timeout: Duration) -> Result<(), ScrapeJobError> {
            unreachable!()
        }

        async fn evaluate(
            &self,
            _script: &str,
            _timeout: Duration,
        ) -> Result<Value, ScrapeJobError> {
            unreachable!()
        }

        async fn screenshot(&self, path: &Path) -> Result<(), ScrapeJobError> {
            std::fs::write(path, b"diagnostic").unwrap();
            self.screenshot_paths
                .lock()
                .unwrap()
                .push(path.to_path_buf());
            Ok(())
        }

        async fn health_check(&self) -> Result<GoogleMapsPageState, ScrapeJobError> {
            Ok(GoogleMapsPageState::Ready)
        }

        async fn health_snapshot(&self) -> Result<GoogleMapsPageHealth, ScrapeJobError> {
            Ok(GoogleMapsPageHealth {
                state: GoogleMapsPageState::Captcha,
                url: "https://www.google.com/maps/search/coffee".to_string(),
                title: "Google Maps".to_string(),
            })
        }

        async fn shutdown(&self) -> Result<(), ScrapeJobError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn captcha_and_selector_failures_capture_url_and_screenshot() {
        let root = std::env::temp_dir().join(format!(
            "rustsale-google-maps-diagnostic-{}",
            rand::random::<u64>()
        ));
        let profile_dir = root.join("job-processor").join("profile");
        std::fs::create_dir_all(&profile_dir).unwrap();
        let mut job = test_job(None);
        job.profile_dir = profile_dir.to_string_lossy().to_string();
        let unit = test_unit("keyword", serde_json::json!({"location": "HCM"}));
        let automation = Arc::new(DiagnosticAutomation {
            screenshot_paths: std::sync::Mutex::new(Vec::new()),
        });
        let session = BrowserSession::new(automation.clone());

        for kind in [ScrapeErrorKind::Captcha, ScrapeErrorKind::SelectorChanged] {
            let captured = capture_scrape_diagnostics(
                &session,
                &job,
                &unit,
                ScrapeJobError {
                    kind,
                    message: "Google Maps requires diagnostics".to_string(),
                    retryable: false,
                },
            )
            .await;

            assert!(captured
                .message
                .contains("https://www.google.com/maps/search/coffee"));
            assert!(captured.message.contains("diagnostic_screenshot="));
        }

        let screenshots = automation.screenshot_paths.lock().unwrap().clone();
        assert_eq!(screenshots.len(), 2);
        assert!(screenshots.iter().all(|path| path.exists()));
        assert!(screenshots
            .iter()
            .all(|path| path.parent() == Some(root.join("job-processor/diagnostics").as_path())));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn processor_maps_candidate_to_auditable_result() {
        let job = test_job(Some([10.0, 11.0, 106.0, 107.0]));
        let unit = test_unit("keyword", serde_json::json!({"location": "HCM"}));
        let candidate = PlaceCandidate {
            place_id: Some("place-1".to_string()),
            canonical_url: "https://www.google.com/maps/place/one".to_string(),
            name: "Cafe One".to_string(),
            phone: Some("+84 90 123 4567".to_string()),
            address: Some("Quận 1, HCM".to_string()),
            website: Some("https://cafe.test".to_string()),
            facebook: None,
            zalo: None,
            latitude: Some(10.5),
            longitude: Some(106.5),
        };

        let result = candidate_to_result(&job, &unit, candidate, 0);

        assert_eq!(result.dedup_key, "place:place-1");
        assert_eq!(result.normalized_phone.as_deref(), Some("0901234567"));
        assert_eq!(result.inside_requested_area, Some(true));
        assert_eq!(result.import_status, "pending");
    }

    #[test]
    fn processor_marks_keyword_results_inside_unbounded_text_search_area() {
        let job = test_job(None);
        let unit = test_unit(
            "keyword",
            serde_json::json!({"location": "Ho Chi Minh City"}),
        );
        let candidate = PlaceCandidate {
            place_id: Some("place-keyword".to_string()),
            canonical_url: "https://www.google.com/maps/place/keyword".to_string(),
            name: "Dien Lanh HK".to_string(),
            phone: Some("0918321212".to_string()),
            address: Some("Ho Chi Minh City, Vietnam".to_string()),
            website: None,
            facebook: None,
            zalo: None,
            latitude: Some(10.8),
            longitude: Some(106.7),
        };

        let result = candidate_to_result(&job, &unit, candidate, 0);

        assert_eq!(result.normalized_phone.as_deref(), Some("0918321212"));
        assert_eq!(result.inside_requested_area, Some(true));
    }

    #[test]
    fn processor_plans_four_bounded_children_for_saturated_grid_cell() {
        let job = test_job(Some([10.0, 11.0, 106.0, 107.0]));
        let cell = GridCell {
            bbox: BoundingBox {
                lat_min: 10.0,
                lat_max: 10.1,
                lng_min: 106.0,
                lng_max: 106.1,
            },
            center_lat: 10.05,
            center_lng: 106.05,
            step_lat_deg: 0.1,
            step_lng_deg: 0.1,
            radius_m: 7_800.0,
            depth: 0,
            parent_unit_id: None,
            consecutive_low_yield_levels: 0,
        };
        let unit = test_unit("grid", serde_json::to_value(&cell).unwrap());

        let children = plan_grid_children(&job, &unit, &cell, 60, false, 1.0, 1).unwrap();

        assert_eq!(children.len(), 4);
        assert!(children.iter().all(|child| child.depth == 1));
        assert!(children.iter().all(|child| child.job_id == job.id));
        assert!(children
            .iter()
            .all(|child| child.payload["parent_unit_id"] == unit.id));
    }

    /// A search unit yields place *units*, not places. Until every one of them
    /// is claimed and completed the job cannot finish, which is what makes a
    /// block part-way through a listing survivable: the places already done are
    /// already checkpointed.
    #[test]
    fn search_unit_emits_one_place_child_per_listing() {
        let job = test_job(None);
        let unit = test_unit("keyword", serde_json::json!({"location": "HCM"}));
        let urls = vec![
            "https://www.google.com/maps/place/Cafe-One".to_string(),
            "https://www.google.com/maps/place/Cafe-Two".to_string(),
        ];

        let children = plan_place_children(&job, &unit, &urls, &HashSet::new()).unwrap();

        assert_eq!(children.len(), 2);
        assert!(children.iter().all(|child| child.unit_type == "place"));
        assert!(children.iter().all(|child| child.job_id == job.id));
        assert!(children
            .iter()
            .all(|child| child.status == ScrapeUnitStatus::Pending));
        assert_eq!(
            children
                .iter()
                .map(|child| child.payload["place_url"].as_str().unwrap().to_string())
                .collect::<Vec<_>>(),
            urls
        );
    }

    /// `max_total_cells` is a budget of grid cells. Once every place detail is
    /// its own unit, `total_units` is dominated by place units, so spending the
    /// cell budget on them pauses a job that has barely subdivided.
    #[test]
    fn place_units_do_not_consume_the_grid_cell_budget() {
        let mut job = test_job(Some([10.0, 11.0, 106.0, 107.0]));
        job.total_units = 24_999;
        let cell = GridCell {
            bbox: BoundingBox {
                lat_min: 10.0,
                lat_max: 10.1,
                lng_min: 106.0,
                lng_max: 106.1,
            },
            center_lat: 10.05,
            center_lng: 106.05,
            step_lat_deg: 0.1,
            step_lng_deg: 0.1,
            radius_m: 7_800.0,
            depth: 0,
            parent_unit_id: None,
            consecutive_low_yield_levels: 0,
        };
        let unit = test_unit("grid", serde_json::to_value(&cell).unwrap());

        let children = plan_grid_children(&job, &unit, &cell, 60, false, 1.0, 10).unwrap();

        assert_eq!(
            children.len(),
            4,
            "only 10 grid cells exist, so the cell budget is nowhere near spent"
        );
    }

    /// A place already known under any of its aliases must not be scheduled
    /// again, even when the new listing carries a different identity form.
    #[test]
    fn a_place_already_known_under_an_alias_is_not_scheduled_again() {
        let job = test_job(None);
        let unit = test_unit("keyword", serde_json::json!({"location": "HCM"}));
        let without_feature_id = "https://www.google.com/maps/place/Cafe+Sam/@10.79,106.69,15z/data=!8m2!3d10.7654321!4d106.7654321";
        let with_feature_id = "https://www.google.com/maps/place/Cafe+Sam/@10.70,106.70,17z/data=!4m6!3m5!1s0xCCC!8m2!3d10.7654321!4d106.7654321";
        let known = place_identity_aliases(without_feature_id)
            .into_iter()
            .collect::<HashSet<_>>();

        let children =
            plan_place_children(&job, &unit, &[with_feature_id.to_string()], &known).unwrap();

        assert!(
            children.is_empty(),
            "the coordinate alias already identifies this business"
        );
    }

    /// Two place URLs that differ only by the grid cell they were reached from
    /// are the same business, so they must collapse to one persisted row.
    #[test]
    fn processor_dedup_key_is_stable_across_entry_viewports() {
        let candidate = |url: &str| PlaceCandidate {
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

        assert_eq!(
            candidate_dedup_key(&candidate(
                "https://www.google.com/maps/place/Cafe+Sam/@10.70,106.70,17z/data=!4m2"
            )),
            candidate_dedup_key(&candidate(
                "https://www.google.com/maps/place/Cafe+Sam/@10.79,106.69,15z/data=!4m2"
            ))
        );
    }

    /// A candidate outside the requested area is a decision, not unfinished
    /// work. Leaving it at `pending` stranded it forever and made the gap
    /// between discovered and imported unattributable.
    #[test]
    fn candidate_outside_the_requested_area_is_marked_skipped_not_pending() {
        let job = test_job(Some([10.0, 11.0, 106.0, 107.0]));
        let unit = test_unit("place", serde_json::json!({"place_url": "https://x"}));
        let candidate = PlaceCandidate {
            place_id: Some("place-outside".to_string()),
            canonical_url: "https://www.google.com/maps/place/outside".to_string(),
            name: "Da Nang Cafe".to_string(),
            phone: Some("0901234567".to_string()),
            address: Some("Da Nang".to_string()),
            website: None,
            facebook: None,
            zalo: None,
            latitude: Some(16.0544),
            longitude: Some(108.2022),
        };

        let result = candidate_to_result(&job, &unit, candidate, 0);

        assert_eq!(result.inside_requested_area, Some(false));
        assert_eq!(result.import_status, "skipped_outside_area");
        assert_eq!(
            result.enrichment_status, "skipped_outside_area",
            "enrichment is never attempted for this row, so leaving it pending is a lie"
        );
    }

    fn test_job(bbox: Option<[f64; 4]>) -> ScrapeJob {
        let now = chrono::Utc::now().to_rfc3339();
        ScrapeJob {
            id: "job-processor".to_string(),
            mode: if bbox.is_some() {
                ScrapeMode::Grid
            } else {
                ScrapeMode::Keyword
            },
            keyword: "cafe".to_string(),
            location: Some("HCM".to_string()),
            bbox,
            step_km: Some(1.0),
            headless: true,
            profile_dir: "/tmp/job-processor/profile".to_string(),
            blocked_url: None,
            status: ScrapeJobStatus::Running,
            stage: ScrapeStage::Scraping,
            total_units: 1,
            processed_units: 0,
            failed_units: 0,
            businesses_discovered: 0,
            businesses_qualified: 0,
            leads_created: 0,
            last_error_kind: None,
            last_error_message: None,
            action_required: None,
            event_sequence: 1,
            version: 1,
            run_generation: 0,
            created_at: now.clone(),
            started_at: Some(now.clone()),
            updated_at: now,
            completed_at: None,
            coverage_truncated: false,
        }
    }

    fn test_unit(unit_type: &str, payload: serde_json::Value) -> ScrapeUnit {
        let now = chrono::Utc::now().to_rfc3339();
        ScrapeUnit {
            id: "unit-1".to_string(),
            job_id: "job-processor".to_string(),
            unit_key: "unit-key".to_string(),
            unit_type: unit_type.to_string(),
            payload,
            status: crate::google_maps_jobs::types::ScrapeUnitStatus::Running,
            depth: 0,
            attempt_count: 1,
            next_retry_at: None,
            last_error_kind: None,
            last_error_message: None,
            created_at: now.clone(),
            updated_at: now,
            completed_at: None,
        }
    }
}
