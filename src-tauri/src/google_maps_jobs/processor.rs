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
    candidate_matches_requested_area, GoogleMapsScraper, PlaceCandidate, SearchPageResult,
};
use crate::scrapers::grid::{
    radius_to_zoom, should_subdivide, subdivide_cell, BoundingBox, GridCell, GridLimits,
};
use async_trait::async_trait;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

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
        if active.is_none() {
            let manager =
                Arc::new(BrowserManager::new(1).with_profile_dir(PathBuf::from(&job.profile_dir)));
            *active = Some(ActiveBrowser {
                job_id: job.id.clone(),
                manager,
            });
        }
        Ok(active
            .as_ref()
            .expect("active browser initialized")
            .manager
            .clone())
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
        let page = match unit.unit_type.as_str() {
            "keyword" | "district" => {
                let location = unit
                    .payload
                    .get("location")
                    .and_then(|value| value.as_str());
                scraper.search_page(&job.keyword, location).await
            }
            "grid" => {
                let cell: GridCell =
                    serde_json::from_value(unit.payload.clone()).map_err(|error| {
                        invalid_response(format!("invalid grid unit payload: {error}"))
                    })?;
                scraper
                    .search_region_page(
                        &job.keyword,
                        cell.center_lat,
                        cell.center_lng,
                        radius_to_zoom(cell.radius_m),
                    )
                    .await
            }
            other => Err(invalid_response(format!(
                "unsupported scrape unit type {other}"
            ))),
        };
        let page = match page {
            Ok(page) => page,
            Err(error) => {
                let error = capture_scrape_diagnostics(&scraper.session, job, unit, error).await;
                if error.retryable {
                    if let Err(close_error) = manager.close().await {
                        eprintln!(
                            "failed to close browser after retryable scrape error: {close_error}"
                        );
                    }
                }
                return Err(error);
            }
        };
        if let Some(kind) = page.blocked_reason {
            return Err(capture_scrape_diagnostics(
                &scraper.session,
                job,
                unit,
                ScrapeJobError {
                    kind,
                    message: "Google Maps requires user action".to_string(),
                    retryable: false,
                },
            )
            .await);
        }
        self.page_to_execution(job, unit, page).await
    }

    async fn page_to_execution(
        &self,
        job: &ScrapeJob,
        unit: &ScrapeUnit,
        page: SearchPageResult,
    ) -> Result<UnitExecution, ScrapeJobError> {
        if let Some(kind) = page.blocked_reason {
            return Err(ScrapeJobError {
                kind,
                message: "Google Maps requires user action".to_string(),
                retryable: false,
            });
        }
        let existing = self
            .repository
            .list_result_dedup_keys(&job.id)
            .await
            .map_err(database_error)?
            .into_iter()
            .collect::<HashSet<_>>();
        let page_keys = page
            .places
            .iter()
            .map(candidate_dedup_key)
            .collect::<HashSet<_>>();
        let new_unique = page_keys
            .iter()
            .filter(|key| !existing.contains(*key))
            .count();
        let new_unique_ratio = if page_keys.is_empty() {
            0.0
        } else {
            new_unique as f64 / page_keys.len() as f64
        };
        let results = page
            .places
            .into_iter()
            .enumerate()
            .map(|(index, candidate)| candidate_to_result(job, unit, candidate, index))
            .collect();
        let child_units = if unit.unit_type == "grid" {
            let cell: GridCell = serde_json::from_value(unit.payload.clone())
                .map_err(|error| invalid_response(format!("invalid grid payload: {error}")))?;
            plan_grid_children(
                job,
                unit,
                &cell,
                page.raw_listing_count,
                page.reached_end,
                new_unique_ratio,
            )?
        } else {
            Vec::new()
        };
        Ok(UnitExecution {
            results,
            child_units,
        })
    }

    async fn finalize_job(&self, job: &ScrapeJob) -> Result<FinalizeOutcome, ScrapeJobError> {
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
                Some(url) => match enricher.find_email(url).await {
                    Ok(Some(email)) => EnrichmentOutcome::found(email),
                    Ok(None) => EnrichmentOutcome::not_found(),
                    Err(error) => EnrichmentOutcome::from_error(error),
                },
                None => EnrichmentOutcome::from_error(invalid_response(
                    "enrichment website URL is invalid",
                )),
            };
            unresolved |= outcome.error.is_some();
            self.repository
                .save_enrichment_outcome(&job.id, &claimed.id, outcome)
                .await
                .map_err(database_error)?;
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
        let blocked_url = job
            .blocked_url
            .as_deref()
            .ok_or_else(|| invalid_response("action-required job omitted blocked URL"))?;
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

    async fn finalize(&self, job: &ScrapeJob) -> Result<FinalizeOutcome, ScrapeJobError> {
        self.finalize_job(job).await
    }
}

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
    let enrichment_status = if normalized_phone.is_none() && candidate.website.is_some() {
        "pending"
    } else {
        "not_found"
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
        import_status: "pending".to_string(),
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
        return format!("url:{}", candidate.canonical_url.trim().to_lowercase());
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

fn plan_grid_children(
    job: &ScrapeJob,
    unit: &ScrapeUnit,
    cell: &GridCell,
    raw_listing_count: usize,
    reached_end: bool,
    new_unique_ratio: f64,
) -> Result<Vec<ScrapeUnit>, ScrapeJobError> {
    if !should_subdivide(
        cell,
        raw_listing_count,
        55,
        reached_end,
        new_unique_ratio,
        job.total_units as usize,
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

        let children = plan_grid_children(&job, &unit, &cell, 60, false, 1.0).unwrap();

        assert_eq!(children.len(), 4);
        assert!(children.iter().all(|child| child.depth == 1));
        assert!(children.iter().all(|child| child.job_id == job.id));
        assert!(children
            .iter()
            .all(|child| child.payload["parent_unit_id"] == unit.id));
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
