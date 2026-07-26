use crate::commands::{
    legacy_start_google_maps_scrape, legacy_stop_google_maps_scrape, LegacyGoogleMapsScrapeRequest,
};
use crate::db::open_db_connection;
use crate::google_maps_jobs::commands::GoogleMapsJobService;
use crate::google_maps_jobs::diagnostics::redact_sensitive_fields;
use crate::google_maps_jobs::events::ScrapeEventPublisher;
use crate::google_maps_jobs::repository::ScrapeJobRepository;
use crate::google_maps_jobs::runner::{
    FinalizeOutcome, RetryPolicy, ScrapeJobRunner, ScrapeUnitProcessor, UnitExecution,
};
use crate::google_maps_jobs::types::{
    CreateScrapeJobRequest, ScrapeJob, ScrapeJobError, ScrapeJobStatus, ScrapeMode, ScrapeStage,
    ScrapeUnit, ScrapeUnitStatus,
};
use crate::scrapers::browser::{GoogleMapsPageHealth, GoogleMapsPageState};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::params;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Notify;

struct FakeProcessor {
    started: Notify,
    gate: Option<Arc<Notify>>,
}

#[async_trait]
impl ScrapeUnitProcessor for FakeProcessor {
    async fn execute(
        &self,
        _job: &ScrapeJob,
        _unit: &ScrapeUnit,
    ) -> Result<UnitExecution, ScrapeJobError> {
        self.started.notify_waiters();
        if let Some(gate) = &self.gate {
            gate.notified().await;
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
        Ok(())
    }

    async fn finalize(&self, _job: &ScrapeJob) -> Result<FinalizeOutcome, ScrapeJobError> {
        Ok(FinalizeOutcome::default())
    }
}

struct Harness {
    root: PathBuf,
    db_path: PathBuf,
    repository: Arc<ScrapeJobRepository>,
    events: Arc<ScrapeEventPublisher>,
    runner: Arc<ScrapeJobRunner>,
    service: Arc<GoogleMapsJobService>,
    processor: Arc<FakeProcessor>,
    gate: Option<Arc<Notify>>,
}

impl Harness {
    fn new(block: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "rustsale-google-maps-task15-{}",
            rand::random::<u64>()
        ));
        let db_path = root.join("rustsale.db");
        std::fs::create_dir_all(&root).unwrap();
        let repository =
            Arc::new(ScrapeJobRepository::new(db_path.to_string_lossy().to_string()).unwrap());
        let events = Arc::new(ScrapeEventPublisher::new(64));
        let gate = block.then(|| Arc::new(Notify::new()));
        let processor = Arc::new(FakeProcessor {
            started: Notify::new(),
            gate: gate.clone(),
        });
        let runner = Arc::new(ScrapeJobRunner::new(
            repository.clone(),
            processor.clone(),
            events.clone(),
            RetryPolicy::for_tests(),
        ));
        let service = Arc::new(GoogleMapsJobService::new(
            repository.clone(),
            runner.clone(),
            root.join("profiles"),
        ));
        Self {
            root,
            db_path,
            repository,
            events,
            runner,
            service,
            processor,
            gate,
        }
    }

    fn profile_path(&self, job_id: &str) -> PathBuf {
        self.root.join("profiles").join(job_id).join("profile")
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn legacy_request() -> LegacyGoogleMapsScrapeRequest {
    LegacyGoogleMapsScrapeRequest {
        keyword: "coffee".to_string(),
        mode: "text".to_string(),
        location: Some("Quận 1".to_string()),
        bbox: None,
        step_km: None,
        districts: None,
        headless: Some(true),
    }
}

fn old_job(
    id: &str,
    status: ScrapeJobStatus,
    profile_dir: &Path,
    completed_at: Option<&str>,
) -> ScrapeJob {
    ScrapeJob {
        id: id.to_string(),
        mode: ScrapeMode::Keyword,
        keyword: "coffee".to_string(),
        location: Some("Hà Nội".to_string()),
        bbox: None,
        step_km: None,
        headless: true,
        profile_dir: profile_dir.to_string_lossy().to_string(),
        blocked_url: (status == ScrapeJobStatus::PausedRequiresAction)
            .then(|| "https://www.google.com/maps".to_string()),
        status,
        stage: ScrapeStage::Scraping,
        total_units: 1,
        processed_units: usize::from(status.is_terminal()) as u64,
        failed_units: 0,
        businesses_discovered: 0,
        businesses_qualified: 0,
        leads_created: 0,
        last_error_kind: None,
        last_error_message: None,
        action_required: (status == ScrapeJobStatus::PausedRequiresAction)
            .then(|| "captcha".to_string()),
        event_sequence: 1,
        version: 0,
        run_generation: 0,
        created_at: "2026-06-01T00:00:00Z".to_string(),
        started_at: Some("2026-06-01T00:00:01Z".to_string()),
        updated_at: completed_at.unwrap_or("2026-06-01T00:00:00Z").to_string(),
        completed_at: completed_at.map(str::to_string),
    }
}

fn old_unit(job_id: &str) -> ScrapeUnit {
    ScrapeUnit {
        id: format!("{job_id}-unit"),
        job_id: job_id.to_string(),
        unit_key: "keyword:coffee".to_string(),
        unit_type: "keyword".to_string(),
        payload: json!({"location": "Hà Nội"}),
        status: ScrapeUnitStatus::Completed,
        depth: 0,
        attempt_count: 1,
        next_retry_at: None,
        last_error_kind: None,
        last_error_message: None,
        created_at: "2026-06-01T00:00:00Z".to_string(),
        updated_at: "2026-06-01T00:01:00Z".to_string(),
        completed_at: Some("2026-06-01T00:01:00Z".to_string()),
    }
}

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-07-26T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

#[tokio::test]
async fn invalid_grid_request_does_not_leave_a_profile_directory() {
    let harness = Harness::new(false);
    let profile_root = harness.root.join("profiles");

    let error = harness
        .service
        .create_job(CreateScrapeJobRequest {
            mode: ScrapeMode::Grid,
            keyword: "coffee".to_string(),
            location: Some("Hà Nội".to_string()),
            bbox: Some([21.0, 20.0, 105.0, 106.0]),
            step_km: Some(2.0),
            districts: Vec::new(),
            headless: true,
        })
        .await
        .unwrap_err();

    assert!(!error.to_string().is_empty());
    let profile_count = if profile_root.exists() {
        std::fs::read_dir(profile_root).unwrap().count()
    } else {
        0
    };
    assert_eq!(profile_count, 0);
}

#[tokio::test]
async fn job_persistence_failure_removes_the_new_profile_directory() {
    let harness = Harness::new(false);
    std::fs::remove_file(&harness.db_path).unwrap();
    std::fs::create_dir(&harness.db_path).unwrap();
    let profile_root = harness.root.join("profiles");

    let error = harness
        .service
        .create_job(CreateScrapeJobRequest {
            mode: ScrapeMode::Keyword,
            keyword: "coffee".to_string(),
            location: Some("Hà Nội".to_string()),
            bbox: None,
            step_km: None,
            districts: Vec::new(),
            headless: true,
        })
        .await
        .unwrap_err();

    assert!(!error.to_string().is_empty());
    let profile_count = if profile_root.exists() {
        std::fs::read_dir(profile_root).unwrap().count()
    } else {
        0
    };
    assert_eq!(profile_count, 0);
}

#[tokio::test]
async fn legacy_google_maps_start_delegates_to_durable_create_and_resume() {
    let harness = Harness::new(true);
    let started = harness.processor.started.notified();

    let job = legacy_start_google_maps_scrape(harness.service.as_ref(), legacy_request())
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), started)
        .await
        .unwrap();

    assert_eq!(job.mode, ScrapeMode::Keyword);
    assert_eq!(job.keyword, "coffee");
    assert_eq!(job.status, ScrapeJobStatus::Running);
    assert!(harness.repository.get_job(&job.id).await.unwrap().is_some());
    harness.gate.as_ref().unwrap().notify_waiters();
}

#[tokio::test]
async fn legacy_google_maps_stop_cancels_the_active_durable_job() {
    let harness = Harness::new(true);
    let started = harness.processor.started.notified();
    let running = legacy_start_google_maps_scrape(harness.service.as_ref(), legacy_request())
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), started)
        .await
        .unwrap();

    let cancelled = legacy_stop_google_maps_scrape(harness.service.as_ref())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(cancelled.id, running.id);
    assert_eq!(cancelled.status, ScrapeJobStatus::Cancelled);
    harness.gate.as_ref().unwrap().notify_waiters();
}

#[tokio::test]
async fn legacy_google_maps_wrapper_never_emits_duplicate_terminal_event() {
    let harness = Harness::new(false);
    let mut receiver = harness.events.subscribe();
    let job = legacy_start_google_maps_scrape(harness.service.as_ref(), legacy_request())
        .await
        .unwrap();
    harness
        .runner
        .wait_for_idle(std::time::Duration::from_secs(2))
        .await
        .unwrap();

    let mut terminal_count = 0;
    while let Ok(event) = receiver.try_recv() {
        if event.job_id == job.id && event.status.is_terminal() {
            terminal_count += 1;
        }
    }
    assert_eq!(terminal_count, 1);
}

#[tokio::test]
async fn retention_deletes_terminal_profile_after_seven_days_but_keeps_job_history() {
    let harness = Harness::new(false);
    let profile = harness.profile_path("profile-old");
    let job_dir = profile.parent().unwrap().to_path_buf();
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::write(profile.join("Preferences"), b"cached").unwrap();
    harness
        .repository
        .create_job(old_job(
            "profile-old",
            ScrapeJobStatus::Completed,
            &profile,
            Some("2026-07-18T00:00:00Z"),
        ))
        .await
        .unwrap();

    let report = harness.service.cleanup_retention_at(now()).await.unwrap();

    assert_eq!(report.deleted_profiles, 1);
    assert!(!profile.exists());
    assert!(!job_dir.exists());
    assert!(harness
        .repository
        .get_job("profile-old")
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn retention_never_deletes_captcha_action_required_profile() {
    let harness = Harness::new(false);
    let profile = harness.profile_path("captcha-job");
    std::fs::create_dir_all(&profile).unwrap();
    harness
        .repository
        .create_job(old_job(
            "captcha-job",
            ScrapeJobStatus::PausedRequiresAction,
            &profile,
            None,
        ))
        .await
        .unwrap();

    let report = harness.service.cleanup_retention_at(now()).await.unwrap();

    assert_eq!(report.deleted_profiles, 0);
    assert!(profile.exists());
}

#[tokio::test]
async fn retention_deletes_job_units_and_results_after_thirty_days() {
    let harness = Harness::new(false);
    let profile = harness.profile_path("expired-job");
    std::fs::create_dir_all(&profile).unwrap();
    harness
        .repository
        .create_job_with_units(
            old_job(
                "expired-job",
                ScrapeJobStatus::Completed,
                &profile,
                Some("2026-06-20T00:00:00Z"),
            ),
            vec![old_unit("expired-job")],
        )
        .await
        .unwrap();
    let conn = open_db_connection(harness.db_path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO google_maps_scrape_results (
            id, job_id, source_unit_id, dedup_key, name, detail_status,
            enrichment_status, import_status, first_seen_at, last_seen_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            "expired-result",
            "expired-job",
            "expired-job-unit",
            "place:expired",
            "Expired Business",
            "completed",
            "not_found",
            "skipped",
            "2026-06-20T00:00:00Z",
            "2026-06-20T00:00:00Z"
        ],
    )
    .unwrap();
    drop(conn);

    let report = harness.service.cleanup_retention_at(now()).await.unwrap();

    assert_eq!(report.deleted_jobs, 1);
    let conn = open_db_connection(harness.db_path.to_str().unwrap()).unwrap();
    let unit_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM google_maps_scrape_units WHERE job_id='expired-job'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let result_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM google_maps_scrape_results WHERE job_id='expired-job'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((unit_count, result_count), (0, 0));
}

#[tokio::test]
async fn retention_profile_validation_failure_keeps_expired_job_for_retry() {
    let harness = Harness::new(false);
    let outside_profile = harness.root.join("outside-profile");
    std::fs::create_dir_all(&outside_profile).unwrap();
    harness
        .repository
        .create_job(old_job(
            "unsafe-profile-job",
            ScrapeJobStatus::Completed,
            &outside_profile,
            Some("2026-06-20T00:00:00Z"),
        ))
        .await
        .unwrap();

    let error = harness
        .service
        .cleanup_retention_at(now())
        .await
        .unwrap_err();

    assert!(error.to_string().contains("outside managed root"));
    assert!(outside_profile.exists());
    assert!(harness
        .repository
        .get_job("unsafe-profile-job")
        .await
        .unwrap()
        .is_some());
}

#[test]
fn redaction_removes_license_cookie_and_local_storage_values() {
    let redacted = redact_sensitive_fields(json!({
        "license_key": "license-secret",
        "cookies": [{"value": "cookie-secret"}],
        "nested": {
            "localStorage": "storage-secret",
            "safe": "visible"
        }
    }));
    let encoded = serde_json::to_string(&redacted).unwrap();

    assert!(!encoded.contains("license-secret"));
    assert!(!encoded.contains("cookie-secret"));
    assert!(!encoded.contains("storage-secret"));
    assert!(encoded.contains("visible"));
    assert!(encoded.contains("[REDACTED]"));
}
