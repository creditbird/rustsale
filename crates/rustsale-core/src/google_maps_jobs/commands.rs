use crate::google_maps_jobs::repository::ScrapeJobRepository;
use crate::google_maps_jobs::runner::ScrapeJobRunner;
use crate::google_maps_jobs::types::{
    CreateScrapeJobRequest, ScrapeJob, ScrapeJobStatus, ScrapeMode, ScrapeStage, ScrapeUnit,
    ScrapeUnitStatus, TransitionCause,
};
use crate::host::State;
use crate::scrapers::browser::{BrowserManager, RuntimeStatus};
use crate::scrapers::grid::{generate_initial_cells, BoundingBox, GridLimits};
use anyhow::{anyhow, Context, Result};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionReport {
    pub deleted_jobs: usize,
    pub deleted_profiles: usize,
}

#[derive(Clone)]
pub struct GoogleMapsJobService {
    repository: Arc<ScrapeJobRepository>,
    runner: Arc<ScrapeJobRunner>,
    profile_root: PathBuf,
    retention_backoff: std::time::Duration,
}

/// Attempts (including the first) for retention database calls that hit a lock.
const RETENTION_DB_ATTEMPTS: u32 = 4;
/// First wait between retention attempts; it doubles each retry (0.5 s, 1 s, 2 s).
const RETENTION_BACKOFF: std::time::Duration = std::time::Duration::from_millis(500);

fn is_database_locked(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<rusqlite::Error>(),
            Some(rusqlite::Error::SqliteFailure(failure, _))
                if matches!(
                    failure.code,
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                )
        )
    })
}

pub struct GoogleMapsJobState {
    pub service: Arc<GoogleMapsJobService>,
    pub runtime: Arc<BrowserManager>,
}

impl GoogleMapsJobService {
    pub fn new(
        repository: Arc<ScrapeJobRepository>,
        runner: Arc<ScrapeJobRunner>,
        profile_root: PathBuf,
    ) -> Self {
        Self {
            repository,
            runner,
            profile_root,
            retention_backoff: RETENTION_BACKOFF,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_retention_backoff(mut self, backoff: std::time::Duration) -> Self {
        self.retention_backoff = backoff;
        self
    }

    /// Runs one retention database call, retrying with a doubling pause while
    /// the database is locked by another writer (app start-up shares it with
    /// the Zalo and Facebook sync tasks). Other errors are returned at once.
    async fn retry_when_locked<T, F, Fut>(
        &self,
        operation_name: &'static str,
        mut call: F,
    ) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut delay = self.retention_backoff;
        let mut attempt = 1;
        loop {
            match call().await {
                Err(error) if attempt < RETENTION_DB_ATTEMPTS && is_database_locked(&error) => {
                    tracing::warn!(
                        target: "google_maps_scraper",
                        event = "retention_database_locked",
                        operation = operation_name,
                        attempt,
                        "retention database call hit a lock; retrying"
                    );
                    tokio::time::sleep(delay).await;
                    delay = delay.saturating_mul(2);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    pub async fn create_job(&self, request: CreateScrapeJobRequest) -> Result<ScrapeJob> {
        let keyword = request.keyword.trim();
        if keyword.is_empty() {
            return Err(anyhow!("keyword must not be empty"));
        }
        if keyword.chars().count() > 200 {
            return Err(anyhow!("keyword must not exceed 200 characters"));
        }

        let job_id = format!(
            "gmaps-{}-{:016x}",
            chrono::Utc::now().timestamp_millis(),
            rand::random::<u64>()
        );
        let now = chrono::Utc::now().to_rfc3339();
        let units = plan_initial_units(&job_id, &request, &now)?;
        let job_dir = self.profile_root.join(&job_id);
        let profile_dir = job_dir.join("profile");
        std::fs::create_dir_all(&profile_dir)
            .with_context(|| format!("failed to create profile directory for {job_id}"))?;
        let job = ScrapeJob {
            id: job_id,
            mode: request.mode,
            keyword: keyword.to_string(),
            location: request.location.map(|value| value.trim().to_string()),
            bbox: request.bbox,
            step_km: request.step_km,
            headless: request.headless,
            profile_dir: profile_dir.to_string_lossy().to_string(),
            blocked_url: None,
            status: ScrapeJobStatus::Queued,
            stage: ScrapeStage::Scraping,
            total_units: units.len() as u64,
            processed_units: 0,
            failed_units: 0,
            businesses_discovered: 0,
            businesses_qualified: 0,
            leads_created: 0,
            last_error_kind: None,
            last_error_message: None,
            action_required: None,
            event_sequence: 1,
            version: 0,
            run_generation: 0,
            created_at: now.clone(),
            started_at: None,
            updated_at: now,
            completed_at: None,
            coverage_truncated: false,
        };
        match self.repository.create_job_with_units(job, units).await {
            Ok(job) => Ok(job),
            Err(error) => {
                if let Err(cleanup_error) = std::fs::remove_dir_all(&job_dir) {
                    return Err(anyhow!(
                        "failed to persist Google Maps job: {error:#}; failed to remove profile directory {}: {cleanup_error}",
                        job_dir.display()
                    ));
                }
                Err(error)
            }
        }
    }

    pub async fn get_job(&self, job_id: &str) -> Result<ScrapeJob> {
        self.repository
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("scrape job {job_id} was not found"))
    }

    pub async fn list_jobs(&self) -> Result<Vec<ScrapeJob>> {
        self.repository.list_jobs().await
    }

    pub async fn pause_job(&self, job_id: &str) -> Result<ScrapeJob> {
        self.runner.pause(job_id).await
    }

    pub async fn resume_job(&self, job_id: &str) -> Result<ScrapeJob> {
        let job = self.get_job(job_id).await?;
        if job.status == ScrapeJobStatus::PausedRequiresAction {
            self.runner.confirm_action(job_id).await
        } else {
            self.runner.start(job_id).await
        }
    }

    pub async fn cancel_job(&self, job_id: &str) -> Result<ScrapeJob> {
        self.runner.cancel(job_id).await
    }

    pub async fn retry_failed_units(&self, job_id: &str) -> Result<ScrapeJob> {
        // Refuse before touching the job: transitioning to Queued and resetting
        // units before a start that cannot succeed left the UI reporting an
        // error while the database had already changed (A12).
        if let Some(active) = self.runner.active_job_id().await {
            return Err(anyhow!("scrape job {active} is already active"));
        }
        self.repository.import_ready_results(job_id).await?;
        let job = self.get_job(job_id).await?;
        let queued = self
            .repository
            .transition_job(
                job_id,
                job.version,
                ScrapeJobStatus::Queued,
                TransitionCause::UserRetry,
            )
            .await?;
        self.runner.start(&queued.id).await
    }

    pub async fn retry_email_enrichment(&self, job_id: &str) -> Result<ScrapeJob> {
        self.runner.retry_email_enrichment(job_id).await
    }

    pub async fn delete_job(&self, job_id: &str) -> Result<bool> {
        self.repository.delete_terminal_job(job_id).await
    }

    pub async fn recover_startup(&self) -> Result<usize> {
        self.runner.recover_startup().await
    }

    pub async fn cleanup_retention(&self) -> Result<RetentionReport> {
        self.cleanup_retention_at(chrono::Utc::now()).await
    }

    pub async fn cleanup_retention_at(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<RetentionReport> {
        let profile_dirs = self
            .retry_when_locked("list_retention_profile_dirs", || {
                self.repository.list_retention_profile_dirs(now, 7)
            })
            .await?;
        std::fs::create_dir_all(&self.profile_root).with_context(|| {
            format!(
                "failed to create Google Maps profile root {}",
                self.profile_root.display()
            )
        })?;
        let canonical_root = self.profile_root.canonicalize().with_context(|| {
            format!(
                "failed to canonicalize Google Maps profile root {}",
                self.profile_root.display()
            )
        })?;
        let mut deleted_profiles = 0usize;
        for profile_dir in profile_dirs {
            let profile_dir = PathBuf::from(profile_dir);
            if !profile_dir.exists() {
                continue;
            }
            if profile_dir
                .symlink_metadata()
                .with_context(|| format!("failed to inspect profile {}", profile_dir.display()))?
                .file_type()
                .is_symlink()
            {
                return Err(anyhow!(
                    "refusing to delete symlinked profile {}",
                    profile_dir.display()
                ));
            }
            let canonical_profile = profile_dir.canonicalize().with_context(|| {
                format!("failed to canonicalize profile {}", profile_dir.display())
            })?;
            if !canonical_profile.starts_with(&canonical_root) {
                return Err(anyhow!(
                    "refusing to delete profile outside managed root: {}",
                    profile_dir.display()
                ));
            }
            let job_dir = canonical_profile.parent().map(PathBuf::from);
            std::fs::remove_dir_all(&canonical_profile).with_context(|| {
                format!("failed to delete expired profile {}", profile_dir.display())
            })?;
            if let Some(job_dir) = job_dir.filter(|path| {
                path != &canonical_root
                    && path.starts_with(&canonical_root)
                    && std::fs::read_dir(path)
                        .map(|mut entries| entries.next().is_none())
                        .unwrap_or(false)
            }) {
                std::fs::remove_dir(&job_dir).with_context(|| {
                    format!("failed to delete empty job directory {}", job_dir.display())
                })?;
            }
            deleted_profiles += 1;
        }
        let deleted_jobs = self
            .retry_when_locked("delete_expired_terminal_jobs", || {
                self.repository.delete_expired_terminal_jobs(now, 30)
            })
            .await?;
        Ok(RetentionReport {
            deleted_jobs,
            deleted_profiles,
        })
    }
}

pub async fn get_google_maps_runtime_status(
    state: State<'_, GoogleMapsJobState>,
) -> Result<RuntimeStatus, String> {
    state
        .runtime
        .runtime_status()
        .await
        .map_err(|error| error.to_string())
}

pub async fn prepare_google_maps_runtime(
    state: State<'_, GoogleMapsJobState>,
) -> Result<RuntimeStatus, String> {
    state
        .runtime
        .prepare_runtime()
        .await
        .map_err(|error| error.to_string())
}

pub async fn create_google_maps_scrape_job(
    state: State<'_, GoogleMapsJobState>,
    request: CreateScrapeJobRequest,
) -> Result<ScrapeJob, String> {
    state
        .service
        .create_job(request)
        .await
        .map_err(|error| error.to_string())
}

pub async fn pause_google_maps_scrape_job(
    state: State<'_, GoogleMapsJobState>,
    job_id: String,
) -> Result<ScrapeJob, String> {
    state
        .service
        .pause_job(&job_id)
        .await
        .map_err(|error| error.to_string())
}

pub async fn resume_google_maps_scrape_job(
    state: State<'_, GoogleMapsJobState>,
    job_id: String,
) -> Result<ScrapeJob, String> {
    state
        .service
        .resume_job(&job_id)
        .await
        .map_err(|error| error.to_string())
}

pub async fn cancel_google_maps_scrape_job(
    state: State<'_, GoogleMapsJobState>,
    job_id: String,
) -> Result<ScrapeJob, String> {
    state
        .service
        .cancel_job(&job_id)
        .await
        .map_err(|error| error.to_string())
}

pub async fn retry_google_maps_failed_units(
    state: State<'_, GoogleMapsJobState>,
    job_id: String,
) -> Result<ScrapeJob, String> {
    state
        .service
        .retry_failed_units(&job_id)
        .await
        .map_err(|error| error.to_string())
}

pub async fn retry_google_maps_email_enrichment(
    state: State<'_, GoogleMapsJobState>,
    job_id: String,
) -> Result<ScrapeJob, String> {
    state
        .service
        .retry_email_enrichment(&job_id)
        .await
        .map_err(|error| error.to_string())
}

pub async fn get_google_maps_scrape_job(
    state: State<'_, GoogleMapsJobState>,
    job_id: String,
) -> Result<ScrapeJob, String> {
    state
        .service
        .get_job(&job_id)
        .await
        .map_err(|error| error.to_string())
}

pub async fn list_google_maps_scrape_jobs(
    state: State<'_, GoogleMapsJobState>,
) -> Result<Vec<ScrapeJob>, String> {
    state
        .service
        .list_jobs()
        .await
        .map_err(|error| error.to_string())
}

pub async fn delete_google_maps_scrape_job(
    state: State<'_, GoogleMapsJobState>,
    job_id: String,
) -> Result<bool, String> {
    state
        .service
        .delete_job(&job_id)
        .await
        .map_err(|error| error.to_string())
}

fn plan_initial_units(
    job_id: &str,
    request: &CreateScrapeJobRequest,
    now: &str,
) -> Result<Vec<ScrapeUnit>> {
    match request.mode {
        ScrapeMode::Keyword => {
            let raw_locations: Vec<Option<String>> = if request.districts.is_empty() {
                vec![request.location.clone()]
            } else {
                request
                    .districts
                    .iter()
                    .map(|district| Some(district.trim().to_string()))
                    .collect()
            };
            if raw_locations.len() > 500 {
                return Err(anyhow!("district count exceeds the 500-unit safety limit"));
            }

            let mut seen_keys = std::collections::HashSet::new();
            let mut unique_locations = Vec::with_capacity(raw_locations.len());
            for location in raw_locations {
                let unit_key = format!(
                    "keyword:{}",
                    location.as_deref().unwrap_or("all").to_lowercase()
                );
                if seen_keys.insert(unit_key.clone()) {
                    unique_locations.push((unit_key, location));
                }
            }

            Ok(unique_locations
                .into_iter()
                .enumerate()
                .map(|(index, (unit_key, location))| ScrapeUnit {
                    id: format!("{job_id}-unit-{index:05}"),
                    job_id: job_id.to_string(),
                    unit_key,
                    unit_type: if request.districts.is_empty() {
                        "keyword".to_string()
                    } else {
                        "district".to_string()
                    },
                    payload: json!({"location": location}),
                    status: ScrapeUnitStatus::Pending,
                    depth: 0,
                    attempt_count: 0,
                    next_retry_at: None,
                    last_error_kind: None,
                    last_error_message: None,
                    created_at: now.to_string(),
                    updated_at: now.to_string(),
                    completed_at: None,
                })
                .collect())
        }
        ScrapeMode::Grid => {
            let bbox = request
                .bbox
                .ok_or_else(|| anyhow!("grid mode requires bbox"))?;
            let step_km = request
                .step_km
                .ok_or_else(|| anyhow!("grid mode requires step_km"))?;
            let bounding_box = BoundingBox {
                lat_min: bbox[0],
                lat_max: bbox[1],
                lng_min: bbox[2],
                lng_max: bbox[3],
            };
            let cells = generate_initial_cells(bounding_box, step_km, GridLimits::default())
                .map_err(|error| anyhow!(error))?;
            cells
                .into_iter()
                .enumerate()
                .map(|(index, cell)| {
                    Ok(ScrapeUnit {
                        id: format!("{job_id}-unit-{index:05}"),
                        job_id: job_id.to_string(),
                        unit_key: format!(
                            "grid:{:.7}:{:.7}:{:.7}:{:.7}:{}",
                            cell.bbox.lat_min,
                            cell.bbox.lat_max,
                            cell.bbox.lng_min,
                            cell.bbox.lng_max,
                            cell.depth
                        ),
                        unit_type: "grid".to_string(),
                        payload: serde_json::to_value(&cell)?,
                        status: ScrapeUnitStatus::Pending,
                        depth: cell.depth,
                        attempt_count: 0,
                        next_retry_at: None,
                        last_error_kind: None,
                        last_error_message: None,
                        created_at: now.to_string(),
                        updated_at: now.to_string(),
                        completed_at: None,
                    })
                })
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::google_maps_jobs::events::ScrapeEventPublisher;
    use crate::google_maps_jobs::repository::ScrapeJobRepository;
    use crate::google_maps_jobs::runner::{
        RetryPolicy, ScrapeJobRunner, ScrapeUnitProcessor, UnitExecution,
    };
    use crate::google_maps_jobs::types::{
        CreateScrapeJobRequest, ScrapeJob, ScrapeJobError, ScrapeJobStatus, ScrapeMode, ScrapeUnit,
    };
    use crate::scrapers::browser::{GoogleMapsPageHealth, GoogleMapsPageState};
    use async_trait::async_trait;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use tokio::sync::Notify;

    struct FakeProcessor {
        block: bool,
        executed_units: Mutex<Vec<String>>,
        release: Notify,
    }

    impl FakeProcessor {
        fn new(block: bool) -> Self {
            Self {
                block,
                executed_units: Mutex::new(Vec::new()),
                release: Notify::new(),
            }
        }
    }

    #[async_trait]
    impl ScrapeUnitProcessor for FakeProcessor {
        async fn execute(
            &self,
            _job: &ScrapeJob,
            unit: &ScrapeUnit,
        ) -> Result<UnitExecution, ScrapeJobError> {
            self.executed_units.lock().unwrap().push(unit.id.clone());
            if self.block {
                self.release.notified().await;
            }
            Ok(UnitExecution::default())
        }

        async fn action_health(
            &self,
            _job: &ScrapeJob,
        ) -> Result<GoogleMapsPageHealth, ScrapeJobError> {
            Ok(GoogleMapsPageHealth {
                state: GoogleMapsPageState::Ready,
                url: "https://www.google.com/maps".to_string(),
                title: "Google Maps".to_string(),
            })
        }

        async fn relaunch_for_action(&self, _job: &ScrapeJob) -> Result<(), ScrapeJobError> {
            Ok(())
        }

        async fn shutdown(&self) -> Result<(), ScrapeJobError> {
            self.release.notify_waiters();
            Ok(())
        }
    }

    struct Harness {
        service: GoogleMapsJobService,
        repository: Arc<ScrapeJobRepository>,
        runner: Arc<ScrapeJobRunner>,
        processor: Arc<FakeProcessor>,
        path: PathBuf,
    }

    impl Harness {
        fn new(block: bool) -> Self {
            let path = std::env::temp_dir().join(format!(
                "rustsale-google-maps-command-{}-{}.db",
                std::process::id(),
                rand::random::<u64>()
            ));
            let repository =
                Arc::new(ScrapeJobRepository::new(path.to_string_lossy().to_string()).unwrap());
            let processor = Arc::new(FakeProcessor::new(block));
            let events = Arc::new(ScrapeEventPublisher::new(64));
            let runner = Arc::new(ScrapeJobRunner::new(
                repository.clone(),
                processor.clone(),
                events,
                RetryPolicy::for_tests(),
            ));
            let service = GoogleMapsJobService::new(
                repository.clone(),
                runner.clone(),
                path.with_extension("profiles"),
            );
            Self {
                service,
                repository,
                runner,
                processor,
                path,
            }
        }
    }

    impl Drop for Harness {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_file(self.path.with_extension("db-wal"));
            let _ = std::fs::remove_file(self.path.with_extension("db-shm"));
            let _ = std::fs::remove_dir_all(self.path.with_extension("profiles"));
        }
    }

    fn keyword_request(keyword: &str) -> CreateScrapeJobRequest {
        CreateScrapeJobRequest {
            mode: ScrapeMode::Keyword,
            keyword: keyword.to_string(),
            location: Some("Thành phố Hồ Chí Minh".to_string()),
            bbox: None,
            step_km: None,
            districts: Vec::new(),
            headless: true,
        }
    }

    #[tokio::test]
    async fn google_maps_scrape_job_command_create_persists_queued_job_and_units() {
        let harness = Harness::new(false);

        let created = harness
            .service
            .create_job(keyword_request("cafe"))
            .await
            .unwrap();

        assert_eq!(created.status, ScrapeJobStatus::Queued);
        assert_eq!(created.total_units, 1);
        assert_eq!(created.event_sequence, 1);
        assert_eq!(
            harness
                .repository
                .list_units(&created.id)
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(
            PathBuf::from(created.profile_dir).starts_with(harness.path.with_extension("profiles"))
        );
    }

    #[tokio::test]
    async fn google_maps_scrape_job_command_rejects_invalid_grid_before_execution() {
        let harness = Harness::new(false);
        let request = CreateScrapeJobRequest {
            mode: ScrapeMode::Grid,
            keyword: "cafe".to_string(),
            location: Some("Thành phố Hồ Chí Minh".to_string()),
            bbox: Some([11.0, 10.0, 106.0, 107.0]),
            step_km: Some(1.0),
            districts: Vec::new(),
            headless: true,
        };

        let error = harness.service.create_job(request).await.unwrap_err();

        assert!(error.to_string().contains("minimums"));
        assert!(harness.repository.list_jobs().await.unwrap().is_empty());
        assert!(harness.processor.executed_units.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn create_job_rejects_out_of_range_step() {
        for step_km in [0.05, 150.0] {
            let harness = Harness::new(false);
            let request = CreateScrapeJobRequest {
                mode: ScrapeMode::Grid,
                keyword: "cafe".to_string(),
                location: Some("Thành phố Hồ Chí Minh".to_string()),
                bbox: Some([10.75, 10.80, 106.68, 106.72]),
                step_km: Some(step_km),
                districts: Vec::new(),
                headless: true,
            };

            let error = harness.service.create_job(request).await.unwrap_err();

            assert!(
                error.to_string().contains("step_km"),
                "step {step_km}: {error}"
            );
            assert!(harness.repository.list_jobs().await.unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn google_maps_scrape_job_command_rejects_second_active_job() {
        let harness = Harness::new(true);
        let first = harness
            .service
            .create_job(keyword_request("cafe"))
            .await
            .unwrap();
        let second = harness
            .service
            .create_job(keyword_request("spa"))
            .await
            .unwrap();
        harness.service.resume_job(&first.id).await.unwrap();

        let error = harness.service.resume_job(&second.id).await.unwrap_err();

        assert!(error.to_string().contains("already active"));
        harness.service.cancel_job(&first.id).await.unwrap();
        harness
            .runner
            .wait_for_idle(std::time::Duration::from_secs(1))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn google_maps_scrape_job_command_pause_resume_cancel_enforce_transitions() {
        let harness = Harness::new(true);
        let job = harness
            .service
            .create_job(keyword_request("cafe"))
            .await
            .unwrap();
        harness.service.resume_job(&job.id).await.unwrap();

        let paused = harness.service.pause_job(&job.id).await.unwrap();
        assert_eq!(paused.status, ScrapeJobStatus::Paused);
        let resumed = harness.service.resume_job(&job.id).await.unwrap();
        assert_eq!(resumed.status, ScrapeJobStatus::Running);
        let cancelled = harness.service.cancel_job(&job.id).await.unwrap();
        assert_eq!(cancelled.status, ScrapeJobStatus::Cancelled);
        harness
            .runner
            .wait_for_idle(std::time::Duration::from_secs(1))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn retry_while_other_job_active_leaves_job_unchanged() {
        let harness = Harness::new(true);
        let active = harness
            .service
            .create_job(keyword_request("cafe"))
            .await
            .unwrap();
        let retryable = harness
            .service
            .create_job(keyword_request("spa"))
            .await
            .unwrap();
        harness.service.resume_job(&active.id).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                if !harness.processor.executed_units.lock().unwrap().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("the first job should start executing");

        let running = harness
            .repository
            .transition_job(
                &retryable.id,
                retryable.version,
                ScrapeJobStatus::Running,
                crate::google_maps_jobs::types::TransitionCause::System,
            )
            .await
            .unwrap();
        let unit = harness
            .repository
            .claim_next_unit(&retryable.id)
            .await
            .unwrap()
            .unwrap();
        let failed = harness
            .repository
            .checkpoint_failed_unit(
                &retryable.id,
                &unit.id,
                ScrapeJobError {
                    kind: crate::google_maps_jobs::types::ScrapeErrorKind::SelectorChanged,
                    message: "place listing changed".to_string(),
                    retryable: false,
                },
            )
            .await
            .unwrap();
        let partial = harness
            .repository
            .transition_job(
                &retryable.id,
                failed.version,
                ScrapeJobStatus::Partial,
                crate::google_maps_jobs::types::TransitionCause::System,
            )
            .await
            .unwrap();
        assert_eq!(partial.failed_units, 1);
        assert_eq!(running.failed_units, 0);

        let error = harness
            .service
            .retry_failed_units(&retryable.id)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("already active"));

        let unchanged = harness
            .repository
            .get_job(&retryable.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(unchanged.status, ScrapeJobStatus::Partial);
        assert_eq!(unchanged.failed_units, 1);
        assert_eq!(unchanged.version, partial.version);
        assert_eq!(
            harness
                .repository
                .get_unit(&unit.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            crate::google_maps_jobs::types::ScrapeUnitStatus::Failed
        );

        harness.service.cancel_job(&active.id).await.unwrap();
        harness
            .runner
            .wait_for_idle(std::time::Duration::from_secs(1))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn google_maps_scrape_job_command_resume_interrupted_claims_exact_pending_unit() {
        let harness = Harness::new(false);
        let job = harness
            .service
            .create_job(keyword_request("cafe"))
            .await
            .unwrap();
        let unit_id = harness.repository.list_units(&job.id).await.unwrap()[0]
            .id
            .clone();
        let running = harness.service.resume_job(&job.id).await.unwrap();
        harness
            .repository
            .transition_job(
                &job.id,
                running.version,
                ScrapeJobStatus::Interrupted,
                crate::google_maps_jobs::types::TransitionCause::System,
            )
            .await
            .unwrap();
        harness
            .runner
            .wait_for_idle(std::time::Duration::from_secs(1))
            .await
            .unwrap();

        harness.service.resume_job(&job.id).await.unwrap();
        harness
            .runner
            .wait_for_idle(std::time::Duration::from_secs(1))
            .await
            .unwrap();

        assert!(harness
            .processor
            .executed_units
            .lock()
            .unwrap()
            .contains(&unit_id));
    }

    #[tokio::test]
    async fn google_maps_scrape_job_command_delete_rejects_non_terminal_job() {
        let harness = Harness::new(false);
        let job = harness
            .service
            .create_job(keyword_request("cafe"))
            .await
            .unwrap();

        let error = harness.service.delete_job(&job.id).await.unwrap_err();

        assert!(error.to_string().contains("terminal"));
    }

    #[tokio::test]
    async fn google_maps_scrape_job_command_event_payload_is_job_scoped_and_monotonic() {
        let publisher = ScrapeEventPublisher::new(8);
        let mut receiver = publisher.subscribe();
        let mut job = test_job("job-event");
        job.event_sequence = 1;
        assert!(publisher.publish_job(&job, "queued").await);
        assert!(!publisher.publish_job(&job, "stale").await);
        job.event_sequence = 2;
        assert!(publisher.publish_job(&job, "running").await);

        let first = receiver.recv().await.unwrap();
        let second = receiver.recv().await.unwrap();
        assert_eq!(first.job_id, "job-event");
        assert_eq!((first.sequence, second.sequence), (1, 2));
    }

    #[tokio::test]
    async fn google_maps_scrape_job_command_deduplicates_duplicate_target_paths_without_rollback() {
        let harness = Harness::new(false);
        let request = CreateScrapeJobRequest {
            mode: ScrapeMode::Keyword,
            keyword: "spa".to_string(),
            location: Some("Vietnam".to_string()),
            bbox: None,
            step_km: None,
            districts: vec![
                "Quận 1, Ho Chi Minh City, Vietnam".to_string(),
                "quận 1, ho chi minh city, vietnam".to_string(),
                "Quận 1, Ho Chi Minh City, Vietnam".to_string(),
                "Quận 3, Ho Chi Minh City, Vietnam".to_string(),
                "QUẬN 3, HO CHI MINH CITY, VIETNAM".to_string(),
            ],
            headless: true,
        };

        let created = harness
            .service
            .create_job(request)
            .await
            .expect("job should be created successfully despite duplicate target districts");

        assert_eq!(created.status, ScrapeJobStatus::Queued);
        assert_eq!(created.total_units, 2);

        let units = harness.repository.list_units(&created.id).await.unwrap();
        assert_eq!(units.len(), 2);
        assert_eq!(
            units[0].unit_key,
            "keyword:quận 1, ho chi minh city, vietnam"
        );
        assert_eq!(
            units[0].payload["location"],
            "Quận 1, Ho Chi Minh City, Vietnam"
        );
        assert_eq!(
            units[1].unit_key,
            "keyword:quận 3, ho chi minh city, vietnam"
        );
        assert_eq!(
            units[1].payload["location"],
            "Quận 3, Ho Chi Minh City, Vietnam"
        );
    }

    fn test_job(id: &str) -> ScrapeJob {
        let now = chrono::Utc::now().to_rfc3339();
        ScrapeJob {
            id: id.to_string(),
            mode: ScrapeMode::Keyword,
            keyword: "cafe".to_string(),
            location: None,
            bbox: None,
            step_km: None,
            headless: true,
            profile_dir: format!("/tmp/{id}"),
            blocked_url: None,
            status: ScrapeJobStatus::Queued,
            stage: crate::google_maps_jobs::types::ScrapeStage::Planning,
            total_units: 1,
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
            updated_at: now,
            completed_at: None,
            coverage_truncated: false,
        }
    }
}
