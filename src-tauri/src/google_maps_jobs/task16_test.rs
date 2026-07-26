use super::events::ScrapeEventPublisher;
use super::repository::ScrapeJobRepository;
use super::runner::{
    FinalizeOutcome, RetryPolicy, ScrapeJobRunner, ScrapeUnitProcessor, UnitExecution,
};
use super::types::{
    ScrapeErrorKind, ScrapeJob, ScrapeJobError, ScrapeJobStatus, ScrapeMode, ScrapeResult,
    ScrapeStage, ScrapeUnit, ScrapeUnitStatus,
};
use crate::db::open_db_connection;
use crate::scrapers::browser::{GoogleMapsPageHealth, GoogleMapsPageState};
use async_trait::async_trait;
use rusqlite::Connection;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Copy)]
enum FinalizeMode {
    Noop,
    Import,
}

struct IntegrationProcessor {
    repository: Arc<ScrapeJobRepository>,
    outcomes: Mutex<VecDeque<Result<UnitExecution, ScrapeJobError>>>,
    calls: Mutex<Vec<String>>,
    finalize_mode: FinalizeMode,
}

impl IntegrationProcessor {
    fn new(
        repository: Arc<ScrapeJobRepository>,
        outcomes: Vec<Result<UnitExecution, ScrapeJobError>>,
        finalize_mode: FinalizeMode,
    ) -> Self {
        Self {
            repository,
            outcomes: Mutex::new(outcomes.into()),
            calls: Mutex::new(Vec::new()),
            finalize_mode,
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl ScrapeUnitProcessor for IntegrationProcessor {
    async fn execute(
        &self,
        _job: &ScrapeJob,
        unit: &ScrapeUnit,
    ) -> Result<UnitExecution, ScrapeJobError> {
        self.calls.lock().unwrap().push(unit.unit_key.clone());
        self.outcomes
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Ok(UnitExecution::default()))
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

    async fn finalize(&self, job: &ScrapeJob) -> Result<FinalizeOutcome, ScrapeJobError> {
        if matches!(self.finalize_mode, FinalizeMode::Import) {
            self.repository
                .set_stage(&job.id, ScrapeStage::EmailEnrichment)
                .await
                .map_err(database_error)?;
            self.repository
                .set_stage(&job.id, ScrapeStage::Importing)
                .await
                .map_err(database_error)?;
            self.repository
                .import_pending_results(&job.id)
                .await
                .map_err(database_error)?;
        }
        Ok(FinalizeOutcome::default())
    }
}

struct TestContext {
    db_path: PathBuf,
    repository: Arc<ScrapeJobRepository>,
}

impl TestContext {
    fn new() -> Self {
        let db_path = std::env::temp_dir().join(format!(
            "rustsale-task16-{}-{}.db",
            std::process::id(),
            rand::random::<u64>()
        ));
        let repository =
            Arc::new(ScrapeJobRepository::new(db_path.to_string_lossy().to_string()).unwrap());
        Self {
            db_path,
            repository,
        }
    }

    fn connection(&self) -> Connection {
        open_db_connection(self.db_path.to_str().unwrap()).unwrap()
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.db_path);
        let _ = std::fs::remove_file(self.db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(self.db_path.with_extension("db-shm"));
    }
}

fn runner(
    repository: Arc<ScrapeJobRepository>,
    processor: Arc<IntegrationProcessor>,
    events: Arc<ScrapeEventPublisher>,
) -> Arc<ScrapeJobRunner> {
    Arc::new(ScrapeJobRunner::new(
        repository,
        processor,
        events,
        RetryPolicy::for_tests(),
    ))
}

fn job(id: &str, mode: ScrapeMode, total_units: u64) -> ScrapeJob {
    let now = chrono::Utc::now().to_rfc3339();
    ScrapeJob {
        id: id.to_string(),
        mode,
        keyword: "cafe".to_string(),
        location: Some("Thành phố Hồ Chí Minh".to_string()),
        bbox: matches!(mode, ScrapeMode::Grid).then_some([10.0, 10.2, 106.0, 106.2]),
        step_km: matches!(mode, ScrapeMode::Grid).then_some(1.0),
        headless: true,
        profile_dir: format!("/tmp/{id}/profile"),
        blocked_url: None,
        status: ScrapeJobStatus::Queued,
        stage: ScrapeStage::Planning,
        total_units,
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
    }
}

fn unit(job_id: &str, id: &str, key: &str, unit_type: &str, depth: u8) -> ScrapeUnit {
    let now = chrono::Utc::now().to_rfc3339();
    ScrapeUnit {
        id: id.to_string(),
        job_id: job_id.to_string(),
        unit_key: key.to_string(),
        unit_type: unit_type.to_string(),
        payload: serde_json::json!({"parent_unit_id": (depth > 0).then_some("parent")}),
        status: ScrapeUnitStatus::Pending,
        depth,
        attempt_count: 0,
        next_retry_at: None,
        last_error_kind: None,
        last_error_message: None,
        created_at: now.clone(),
        updated_at: now,
        completed_at: None,
    }
}

fn importable_result(job_id: &str, unit_id: &str) -> ScrapeResult {
    let now = chrono::Utc::now().to_rfc3339();
    ScrapeResult {
        id: format!("result-{job_id}"),
        job_id: job_id.to_string(),
        source_unit_id: unit_id.to_string(),
        place_id: Some(format!("place-{job_id}")),
        canonical_url: Some("https://www.google.com/maps/place/test".to_string()),
        dedup_key: format!("place:{job_id}"),
        name: "Cafe Production Test".to_string(),
        phone: Some("+84 90 123 4567".to_string()),
        normalized_phone: Some("0901234567".to_string()),
        email: None,
        normalized_email: None,
        address: Some("Quận 1, Thành phố Hồ Chí Minh".to_string()),
        website: None,
        facebook: None,
        zalo: None,
        latitude: Some(10.775),
        longitude: Some(106.7),
        inside_requested_area: Some(true),
        detail_status: "completed".to_string(),
        enrichment_status: "not_found".to_string(),
        import_status: "pending".to_string(),
        last_error_kind: None,
        last_error_message: None,
        first_seen_at: now.clone(),
        last_seen_at: now,
    }
}

fn database_error(error: impl std::fmt::Display) -> ScrapeJobError {
    ScrapeJobError {
        kind: ScrapeErrorKind::Database,
        message: error.to_string(),
        retryable: false,
    }
}

#[tokio::test]
async fn keyword_job_checkpoints_result_and_imports_lead_before_completion() {
    let context = TestContext::new();
    let job_id = "keyword-e2e";
    let unit_id = "keyword-unit";
    context
        .repository
        .create_job_with_units(
            job(job_id, ScrapeMode::Keyword, 1),
            vec![unit(job_id, unit_id, "keyword:cafe:hcm", "keyword", 0)],
        )
        .await
        .unwrap();
    let processor = Arc::new(IntegrationProcessor::new(
        context.repository.clone(),
        vec![Ok(UnitExecution {
            results: vec![importable_result(job_id, unit_id)],
            child_units: Vec::new(),
        })],
        FinalizeMode::Import,
    ));
    let runner = runner(
        context.repository.clone(),
        processor,
        Arc::new(ScrapeEventPublisher::new(32)),
    );

    runner.start(job_id).await.unwrap();
    runner.wait_for_idle(Duration::from_secs(2)).await.unwrap();

    let completed = context.repository.get_job(job_id).await.unwrap().unwrap();
    assert_eq!(completed.status, ScrapeJobStatus::Completed);
    assert_eq!(completed.processed_units, 1);
    assert_eq!(completed.businesses_discovered, 1);
    assert_eq!(completed.businesses_qualified, 1);
    assert_eq!(completed.leads_created, 1);
    let connection = context.connection();
    let lead: (String, String) = connection
        .query_row("SELECT name, phone FROM leads", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(
        lead,
        ("Cafe Production Test".to_string(), "0901234567".to_string())
    );
    let import_status: String = connection
        .query_row(
            "SELECT import_status FROM google_maps_scrape_results WHERE job_id=?1",
            [job_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(import_status, "imported");
}

async fn assert_disconnect_stops_later_unit(mode: ScrapeMode, unit_type: &str) {
    let context = TestContext::new();
    let job_id = format!("disconnect-{unit_type}");
    context
        .repository
        .create_job_with_units(
            job(&job_id, mode, 2),
            vec![
                unit(&job_id, "first", "first", unit_type, 0),
                unit(&job_id, "second", "second", unit_type, 0),
            ],
        )
        .await
        .unwrap();
    let processor = Arc::new(IntegrationProcessor::new(
        context.repository.clone(),
        vec![Err(ScrapeJobError {
            kind: ScrapeErrorKind::Disconnected,
            message: "CDP connection lost".to_string(),
            retryable: false,
        })],
        FinalizeMode::Noop,
    ));
    let runner = runner(
        context.repository.clone(),
        processor.clone(),
        Arc::new(ScrapeEventPublisher::new(32)),
    );

    runner.start(&job_id).await.unwrap();
    runner.wait_for_idle(Duration::from_secs(2)).await.unwrap();

    assert_eq!(processor.calls(), vec!["first"]);
    assert_eq!(
        context
            .repository
            .get_job(&job_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        ScrapeJobStatus::Failed
    );
    assert_eq!(
        context
            .repository
            .get_unit("second")
            .await
            .unwrap()
            .unwrap()
            .status,
        ScrapeUnitStatus::Pending
    );
}

#[tokio::test]
async fn disconnected_deep_scan_and_grid_units_fail_fast() {
    assert_disconnect_stops_later_unit(ScrapeMode::Keyword, "district").await;
    assert_disconnect_stops_later_unit(ScrapeMode::Grid, "grid_cell").await;
}

#[tokio::test]
async fn saturated_grid_checkpoint_persists_only_four_bounded_children() {
    let context = TestContext::new();
    let job_id = "grid-saturation-e2e";
    context
        .repository
        .create_job_with_units(
            job(job_id, ScrapeMode::Grid, 1),
            vec![unit(job_id, "parent", "grid:parent", "grid_cell", 0)],
        )
        .await
        .unwrap();
    let children = (0..4)
        .map(|index| {
            unit(
                job_id,
                &format!("child-{index}"),
                &format!("grid:child:{index}"),
                "grid_cell",
                1,
            )
        })
        .collect::<Vec<_>>();
    let processor = Arc::new(IntegrationProcessor::new(
        context.repository.clone(),
        vec![Ok(UnitExecution {
            results: Vec::new(),
            child_units: children,
        })],
        FinalizeMode::Noop,
    ));
    let runner = runner(
        context.repository.clone(),
        processor.clone(),
        Arc::new(ScrapeEventPublisher::new(32)),
    );

    runner.start(job_id).await.unwrap();
    runner.wait_for_idle(Duration::from_secs(2)).await.unwrap();

    let units = context.repository.list_units(job_id).await.unwrap();
    assert_eq!(units.len(), 5);
    assert_eq!(units.iter().filter(|unit| unit.depth == 1).count(), 4);
    assert!(units.iter().all(|unit| unit.depth <= 1));
    assert_eq!(processor.calls().len(), 5);
    let completed = context.repository.get_job(job_id).await.unwrap().unwrap();
    assert_eq!(completed.status, ScrapeJobStatus::Completed);
    assert_eq!((completed.total_units, completed.processed_units), (5, 5));
}

#[tokio::test]
async fn database_import_failure_fails_job_without_completion_event() {
    let context = TestContext::new();
    let job_id = "import-failure-e2e";
    let unit_id = "import-failure-unit";
    context
        .repository
        .create_job_with_units(
            job(job_id, ScrapeMode::Keyword, 1),
            vec![unit(job_id, unit_id, "keyword:failure", "keyword", 0)],
        )
        .await
        .unwrap();
    context
        .connection()
        .execute_batch(
            "CREATE TRIGGER fail_task16_lead_insert
             BEFORE INSERT ON leads
             BEGIN
               SELECT RAISE(ABORT, 'forced task16 lead insert failure');
             END;",
        )
        .unwrap();
    let processor = Arc::new(IntegrationProcessor::new(
        context.repository.clone(),
        vec![Ok(UnitExecution {
            results: vec![importable_result(job_id, unit_id)],
            child_units: Vec::new(),
        })],
        FinalizeMode::Import,
    ));
    let events = Arc::new(ScrapeEventPublisher::new(32));
    let mut receiver = events.subscribe();
    let runner = runner(context.repository.clone(), processor, events);

    runner.start(job_id).await.unwrap();
    runner.wait_for_idle(Duration::from_secs(2)).await.unwrap();

    let failed = context.repository.get_job(job_id).await.unwrap().unwrap();
    assert_eq!(failed.status, ScrapeJobStatus::Failed);
    assert_eq!(failed.last_error_kind, Some(ScrapeErrorKind::Database));
    let connection = context.connection();
    let lead_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM leads", [], |row| row.get(0))
        .unwrap();
    let import_status: String = connection
        .query_row(
            "SELECT import_status FROM google_maps_scrape_results WHERE job_id=?1",
            [job_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(lead_count, 0);
    assert_eq!(import_status, "pending");
    let mut statuses = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        statuses.push(event.status);
    }
    assert!(!statuses.contains(&ScrapeJobStatus::Completed));
}
