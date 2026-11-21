use crate::db::{init_db, open_db_connection};
use crate::google_maps_jobs::email::{normalize_email, EnrichmentOutcome, EnrichmentStatus};
use crate::google_maps_jobs::types::{
    ScrapeJob, ScrapeJobError, ScrapeJobStatus, ScrapeResult, ScrapeStage, ScrapeUnit,
    TransitionCause,
};
use anyhow::{anyhow, Context, Result};
// Every transaction below reads before it writes. `BEGIN IMMEDIATE` takes the
// write lock up front so `busy_timeout` can wait for it; a DEFERRED transaction
// would instead fail the upgrade with SQLITE_BUSY_SNAPSHOT (517), which is not
// retryable and surfaced as a spurious "database is locked".
use rusqlite::{
    params, types::Type, Connection, OptionalExtension, Row, Transaction, TransactionBehavior,
};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

const JOB_COLUMNS: &str = "id, mode, keyword, location, bbox_json, step_km, headless, profile_dir, blocked_url, status, stage, total_units, processed_units, failed_units, businesses_discovered, businesses_qualified, leads_created, last_error_kind, last_error_message, action_required, event_sequence, version, run_generation, created_at, started_at, updated_at, completed_at, coverage_truncated";
const UNIT_COLUMNS: &str = "id, job_id, unit_key, unit_type, payload_json, status, depth, attempt_count, next_retry_at, last_error_kind, last_error_message, created_at, updated_at, completed_at";
const RESULT_COLUMNS: &str = "id, job_id, source_unit_id, place_id, canonical_url, dedup_key, name, phone, normalized_phone, email, normalized_email, address, website, facebook, zalo, latitude, longitude, inside_requested_area, detail_status, enrichment_status, import_status, last_error_kind, last_error_message, first_seen_at, last_seen_at";

#[derive(Clone)]
pub struct ScrapeJobRepository {
    db_path: Arc<String>,
}

impl ScrapeJobRepository {
    pub fn new(db_path: String) -> Result<Self> {
        drop(init_db(&db_path).context("failed to initialize scraper repository schema")?);
        Ok(Self {
            db_path: Arc::new(db_path),
        })
    }

    async fn with_connection<T, F>(&self, operation: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(Connection) -> Result<T> + Send + 'static,
    {
        let db_path = self.db_path.clone();
        tokio::task::spawn_blocking(move || {
            let conn = open_db_connection(db_path.as_str())
                .context("failed to open scraper repository connection")?;
            operation(conn)
        })
        .await
        .context("scraper repository worker panicked")?
    }

    pub async fn create_job(&self, job: ScrapeJob) -> Result<ScrapeJob> {
        if job.profile_dir.trim().is_empty() {
            return Err(anyhow!("scrape job profile_dir must not be empty"));
        }
        self.with_connection(move |conn| {
            let bbox_json = job
                .bbox
                .map(|bbox| serde_json::to_string(&bbox))
                .transpose()?;
            conn.execute(
                "INSERT INTO google_maps_scrape_jobs (
                    id, mode, keyword, location, bbox_json, step_km, headless, profile_dir,
                    blocked_url, status, stage,
                    total_units, processed_units, failed_units, businesses_discovered,
                    businesses_qualified, leads_created, last_error_kind, last_error_message,
                    action_required, event_sequence, version, run_generation, created_at,
                    started_at, updated_at, completed_at
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                    ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25,
                    ?26, ?27
                 )",
                params![
                    job.id,
                    enum_to_text(&job.mode)?,
                    job.keyword,
                    job.location,
                    bbox_json,
                    job.step_km,
                    i64::from(job.headless),
                    job.profile_dir,
                    job.blocked_url,
                    enum_to_text(&job.status)?,
                    enum_to_text(&job.stage)?,
                    job.total_units as i64,
                    job.processed_units as i64,
                    job.failed_units as i64,
                    job.businesses_discovered as i64,
                    job.businesses_qualified as i64,
                    job.leads_created as i64,
                    job.last_error_kind
                        .map(|kind| enum_to_text(&kind))
                        .transpose()?,
                    job.last_error_message,
                    job.action_required,
                    job.event_sequence as i64,
                    job.version as i64,
                    job.run_generation as i64,
                    job.created_at,
                    job.started_at,
                    job.updated_at,
                    job.completed_at,
                ],
            )?;
            load_job(&conn, &job.id)?.ok_or_else(|| anyhow!("created job was not found"))
        })
        .await
    }

    pub async fn create_job_with_units(
        &self,
        job: ScrapeJob,
        units: Vec<ScrapeUnit>,
    ) -> Result<ScrapeJob> {
        if job.profile_dir.trim().is_empty() {
            return Err(anyhow!("scrape job profile_dir must not be empty"));
        }
        if job.total_units != units.len() as u64 {
            return Err(anyhow!(
                "scrape job total_units does not match planned units"
            ));
        }
        let job_id = job.id.clone();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            insert_job(&tx, &job)?;
            for unit in &units {
                if unit.job_id != job_id {
                    return Err(anyhow!("planned unit belongs to a different job"));
                }
                insert_unit(&tx, unit, false)?;
            }
            let created = load_job(&tx, &job_id)?
                .ok_or_else(|| anyhow!("created scrape job was not found"))?;
            tx.commit()?;
            Ok(created)
        })
        .await
    }

    pub async fn pause_job(&self, job_id: &str) -> Result<ScrapeJob> {
        let job_id = job_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let current = load_job(&tx, &job_id)?
                .ok_or_else(|| anyhow!("scrape job {job_id} was not found"))?;
            if current.status != ScrapeJobStatus::Running {
                return Err(anyhow!("only a running scrape job can be paused"));
            }
            let now = chrono::Utc::now().to_rfc3339();
            tx.execute(
                "UPDATE google_maps_scrape_units
                 SET status='pending', next_retry_at=NULL, updated_at=?1
                 WHERE job_id=?2 AND status='running'",
                params![now, job_id],
            )?;
            let changed = tx.execute(
                "UPDATE google_maps_scrape_jobs
                 SET status='paused', updated_at=?1, action_required=NULL,
                     event_sequence=event_sequence + 1, version=version + 1
                 WHERE id=?2 AND status='running' AND version=?3",
                params![now, job_id, current.version as i64],
            )?;
            if changed != 1 {
                return Err(anyhow!("scrape job changed while pausing"));
            }
            let paused = load_job(&tx, &job_id)?
                .ok_or_else(|| anyhow!("paused scrape job was not found"))?;
            tx.commit()?;
            Ok(paused)
        })
        .await
    }

    pub async fn set_stage(&self, job_id: &str, stage: ScrapeStage) -> Result<ScrapeJob> {
        let job_id = job_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let now = chrono::Utc::now().to_rfc3339();
            let changed = tx.execute(
                "UPDATE google_maps_scrape_jobs
                 SET stage=?1, updated_at=?2, event_sequence=event_sequence + 1,
                     version=version + 1
                 WHERE id=?3 AND status='running'",
                params![enum_to_text(&stage)?, now, job_id],
            )?;
            if changed != 1 {
                return Err(anyhow!("only a running scrape job can change stage"));
            }
            let job = load_job(&tx, &job_id)?
                .ok_or_else(|| anyhow!("staged scrape job was not found"))?;
            tx.commit()?;
            Ok(job)
        })
        .await
    }

    pub async fn reopen_for_email_retry(&self, job_id: &str) -> Result<ScrapeJob> {
        let job_id = job_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let current = load_job(&tx, &job_id)?
                .ok_or_else(|| anyhow!("scrape job {job_id} was not found"))?;
            if current.status != ScrapeJobStatus::Partial {
                return Err(anyhow!("email enrichment retry requires a partial job"));
            }
            let reset = tx.execute(
                "UPDATE google_maps_scrape_results
                 SET enrichment_status='pending', last_error_kind=NULL,
                     last_error_message=NULL
                 WHERE job_id=?1 AND enrichment_status IN ('retry_wait', 'failed')",
                params![job_id],
            )?;
            if reset == 0 {
                return Err(anyhow!(
                    "scrape job has no failed email enrichment to retry"
                ));
            }
            let now = chrono::Utc::now().to_rfc3339();
            let changed = tx.execute(
                "UPDATE google_maps_scrape_jobs
                 SET status='running', stage='email_enrichment', completed_at=NULL,
                     updated_at=?1, last_error_kind=NULL, last_error_message=NULL,
                     event_sequence=event_sequence + 1, version=version + 1
                 WHERE id=?2 AND status='partial' AND version=?3",
                params![now, job_id, current.version as i64],
            )?;
            if changed != 1 {
                return Err(anyhow!(
                    "scrape job changed while reopening email enrichment"
                ));
            }
            let running = load_job(&tx, &job_id)?
                .ok_or_else(|| anyhow!("reopened scrape job was not found"))?;
            tx.commit()?;
            Ok(running)
        })
        .await
    }

    pub async fn list_result_dedup_keys(&self, job_id: &str) -> Result<Vec<String>> {
        let job_id = job_id.to_string();
        self.with_connection(move |conn| {
            let mut statement =
                conn.prepare("SELECT dedup_key FROM google_maps_scrape_results WHERE job_id=?1")?;
            let rows = statement.query_map(params![job_id], |row| row.get(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into)
        })
        .await
    }

    pub async fn claim_result_for_enrichment(
        &self,
        job_id: &str,
        result_id: &str,
    ) -> Result<Option<ScrapeResult>> {
        let job_id = job_id.to_string();
        let result_id = result_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let changed = tx.execute(
                "UPDATE google_maps_scrape_results
                 SET enrichment_status='running', last_error_kind=NULL,
                     last_error_message=NULL
                 WHERE id=?1 AND job_id=?2
                   AND enrichment_status IN ('pending', 'retry_wait')",
                params![result_id, job_id],
            )?;
            if changed == 0 {
                tx.commit()?;
                return Ok(None);
            }
            if changed != 1 {
                return Err(anyhow!("enrichment claim changed multiple results"));
            }
            let result = load_result(&tx, &result_id)?
                .ok_or_else(|| anyhow!("claimed enrichment result was not found"))?;
            tx.commit()?;
            Ok(Some(result))
        })
        .await
    }

    pub async fn import_pending_results(
        &self,
        job_id: &str,
    ) -> Result<crate::google_maps_jobs::importer::ImportSummary> {
        let job_id = job_id.to_string();
        self.with_connection(move |mut conn| {
            crate::google_maps_jobs::importer::import_pending_results(&mut conn, &job_id)
                .map_err(anyhow::Error::from)
        })
        .await
    }

    pub async fn import_ready_results(
        &self,
        job_id: &str,
    ) -> Result<crate::google_maps_jobs::importer::ImportSummary> {
        let job_id = job_id.to_string();
        self.with_connection(move |mut conn| {
            crate::google_maps_jobs::importer::import_ready_results(&mut conn, &job_id)
                .map_err(anyhow::Error::from)
        })
        .await
    }

    pub async fn get_job(&self, job_id: &str) -> Result<Option<ScrapeJob>> {
        let job_id = job_id.to_string();
        self.with_connection(move |conn| load_job(&conn, &job_id))
            .await
    }

    pub async fn list_jobs(&self) -> Result<Vec<ScrapeJob>> {
        self.with_connection(move |conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {JOB_COLUMNS} FROM google_maps_scrape_jobs ORDER BY created_at DESC, id DESC"
            ))?;
            let rows = stmt.query_map([], job_from_row)?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
        })
        .await
    }

    pub async fn get_unit(&self, unit_id: &str) -> Result<Option<ScrapeUnit>> {
        let unit_id = unit_id.to_string();
        self.with_connection(move |conn| load_unit(&conn, &unit_id))
            .await
    }

    pub async fn list_units(&self, job_id: &str) -> Result<Vec<ScrapeUnit>> {
        let job_id = job_id.to_string();
        self.with_connection(move |conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {UNIT_COLUMNS} FROM google_maps_scrape_units
                 WHERE job_id=?1 ORDER BY created_at ASC, id ASC"
            ))?;
            let rows = stmt.query_map(params![job_id], unit_from_row)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into)
        })
        .await
    }

    pub async fn transition_job(
        &self,
        job_id: &str,
        expected_version: u64,
        to: ScrapeJobStatus,
        cause: TransitionCause,
    ) -> Result<ScrapeJob> {
        let job_id = job_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let current = load_job(&tx, &job_id)?
                .ok_or_else(|| anyhow!("scrape job {job_id} was not found"))?;
            if current.version != expected_version {
                return Err(anyhow!(
                    "stale job version: expected {expected_version}, found {}",
                    current.version
                ));
            }
            if !is_valid_transition(current.status, to, cause) {
                return Err(anyhow!(
                    "invalid transition from {:?} to {:?} with {:?}",
                    current.status,
                    to,
                    cause
                ));
            }

            let now = chrono::Utc::now().to_rfc3339();
            let mut reset_count = 0usize;
            let mut generation_increment = 0i64;
            if matches!(
                current.status,
                ScrapeJobStatus::Partial | ScrapeJobStatus::Failed
            ) && to == ScrapeJobStatus::Queued
                && cause == TransitionCause::UserRetry
            {
                reset_count = tx.execute(
                    "UPDATE google_maps_scrape_units
                     SET status='pending', next_retry_at=NULL, last_error_kind=NULL,
                         last_error_message=NULL, completed_at=NULL, updated_at=?1
                     WHERE job_id=?2 AND status='failed'
                       AND last_error_kind IN (
                           'disconnected', 'navigation_timeout', 'evaluation_timeout',
                           'transient_network', 'browser_startup', 'runtime_unavailable',
                           'selector_changed', 'database'
                       )",
                    params![now, job_id],
                )?;
                generation_increment = 1;
            }

            let to_text = enum_to_text(&to)?;
            let terminal = i64::from(to.is_terminal());
            let changed = tx.execute(
                "UPDATE google_maps_scrape_jobs
                 SET status=?1,
                     updated_at=?2,
                     started_at=CASE
                         WHEN ?1='running' THEN COALESCE(started_at, ?2)
                         ELSE started_at
                     END,
                     completed_at=CASE
                         WHEN ?3=1 THEN ?2
                         WHEN ?1 IN ('queued', 'running') THEN NULL
                         ELSE completed_at
                     END,
                     action_required=CASE
                         WHEN ?1 IN ('queued', 'running') THEN NULL
                         ELSE action_required
                     END,
                     last_error_kind=CASE
                         WHEN ?1 IN ('queued', 'running') THEN NULL
                         ELSE last_error_kind
                     END,
                     last_error_message=CASE
                         WHEN ?1 IN ('queued', 'running') THEN NULL
                         ELSE last_error_message
                     END,
                     processed_units=max(processed_units - ?4, 0),
                     failed_units=max(failed_units - ?4, 0),
                     event_sequence=event_sequence + 1,
                     version=version + 1,
                     run_generation=run_generation + ?5
                 WHERE id=?6 AND version=?7",
                params![
                    to_text,
                    now,
                    terminal,
                    reset_count as i64,
                    generation_increment,
                    job_id,
                    expected_version as i64,
                ],
            )?;
            if changed != 1 {
                return Err(anyhow!("stale job version during transition"));
            }
            let updated =
                load_job(&tx, &job_id)?.ok_or_else(|| anyhow!("transitioned job was not found"))?;
            tx.commit()?;
            Ok(updated)
        })
        .await
    }

    pub async fn recover_interrupted_jobs(&self) -> Result<usize> {
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let now = chrono::Utc::now().to_rfc3339();
            tx.execute(
                "UPDATE google_maps_scrape_units
                 SET status='pending', next_retry_at=NULL, updated_at=?1
                 WHERE status='running'
                   AND job_id IN (
                       SELECT id FROM google_maps_scrape_jobs WHERE status='running'
                   )",
                params![now],
            )?;
            let changed = tx.execute(
                "UPDATE google_maps_scrape_jobs
                 SET status='interrupted', updated_at=?1, completed_at=NULL,
                     event_sequence=event_sequence + 1, version=version + 1
                 WHERE status='running'",
                params![now],
            )?;
            tx.commit()?;
            Ok(changed)
        })
        .await
    }

    pub async fn insert_units(&self, job_id: &str, units: Vec<ScrapeUnit>) -> Result<usize> {
        let job_id = job_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            for unit in &units {
                if unit.job_id != job_id {
                    return Err(anyhow!("unit {} belongs to a different job", unit.id));
                }
                insert_unit(&tx, unit, false)?;
            }
            let changed = tx.execute(
                "UPDATE google_maps_scrape_jobs
                 SET total_units=total_units + ?1, version=version + 1, updated_at=?2
                 WHERE id=?3 AND status NOT IN ('partial', 'completed', 'failed', 'cancelled')",
                params![units.len() as i64, chrono::Utc::now().to_rfc3339(), job_id],
            )?;
            if changed != 1 {
                return Err(anyhow!("scrape job was not found while inserting units"));
            }
            tx.commit()?;
            Ok(units.len())
        })
        .await
    }

    /// Counts units of one type.
    ///
    /// The grid safety budget is a budget of *cells*, so it must be measured
    /// against grid units alone. `total_units` also carries one unit per place
    /// detail and would exhaust the budget after a single populated cell.
    pub async fn count_units_by_type(&self, job_id: &str, unit_type: &str) -> Result<u64> {
        let job_id = job_id.to_string();
        let unit_type = unit_type.to_string();
        self.with_connection(move |conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM google_maps_scrape_units WHERE job_id=?1 AND unit_type=?2",
                params![job_id, unit_type],
                |row| row.get(0),
            )?;
            Ok(count.max(0) as u64)
        })
        .await
    }

    /// Records that the cell budget cut this job's coverage short.
    ///
    /// The flag is durable and never cleared, so resuming past a budget pause
    /// cannot report full coverage of an area the job stopped subdividing.
    pub async fn mark_coverage_truncated(&self, job_id: &str) -> Result<()> {
        let job_id = job_id.to_string();
        self.with_connection(move |conn| {
            conn.execute(
                "UPDATE google_maps_scrape_jobs SET coverage_truncated=1, updated_at=?1
                 WHERE id=?2",
                params![chrono::Utc::now().to_rfc3339(), job_id],
            )?;
            Ok(())
        })
        .await
    }

    /// Finished place units, split into completed and failed.
    ///
    /// The failure ratio has to be measured over place units alone. The job-wide
    /// counters cannot serve: `checkpoint_failed_unit` increments
    /// `processed_units` *and* `failed_units` for the same unit, so
    /// `failed / (processed + failed)` can never exceed one half and any
    /// threshold above that is unreachable.
    pub async fn count_place_unit_outcomes(&self, job_id: &str) -> Result<(u64, u64)> {
        let job_id = job_id.to_string();
        self.with_connection(move |conn| {
            let (completed, failed): (i64, i64) = conn.query_row(
                "SELECT
                     COUNT(*) FILTER (WHERE status='completed'),
                     COUNT(*) FILTER (WHERE status='failed')
                 FROM google_maps_scrape_units
                 WHERE job_id=?1 AND unit_type='place'",
                params![job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            Ok((completed.max(0) as u64, failed.max(0) as u64))
        })
        .await
    }

    /// The identities of every place already scheduled as a unit.
    ///
    /// Subdivision novelty must be measured against work already planned, not
    /// only against work already finished: a grid child can be claimed before
    /// its place siblings, and counting an unscraped-but-scheduled place as new
    /// makes the low-yield heuristic subdivide again.
    pub async fn list_place_unit_keys(&self, job_id: &str) -> Result<Vec<String>> {
        let job_id = job_id.to_string();
        self.with_connection(move |conn| {
            let mut statement = conn.prepare(
                "SELECT unit_key FROM google_maps_scrape_units
                 WHERE job_id=?1 AND unit_type='place'",
            )?;
            let rows = statement.query_map(params![job_id], |row| row.get(0))?;
            rows.collect::<rusqlite::Result<Vec<String>>>()
                .map_err(Into::into)
        })
        .await
    }

    pub async fn claim_next_unit(&self, job_id: &str) -> Result<Option<ScrapeUnit>> {
        let job_id = job_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let now = chrono::Utc::now().to_rfc3339();
            let unit_id: Option<String> = tx
                .query_row(
                    "SELECT id FROM google_maps_scrape_units
                     WHERE job_id=?1
                       AND (
                           status='pending'
                           OR (status='retry_scheduled' AND (next_retry_at IS NULL OR next_retry_at <= ?2))
                       )
                     ORDER BY created_at ASC, id ASC
                     LIMIT 1",
                    params![job_id, now],
                    |row| row.get(0),
                )
                .optional()?;
            let Some(unit_id) = unit_id else {
                tx.commit()?;
                return Ok(None);
            };
            let changed = tx.execute(
                "UPDATE google_maps_scrape_units
                 SET status='running', attempt_count=attempt_count + 1,
                     next_retry_at=NULL, updated_at=?1
                 WHERE id=?2 AND job_id=?3
                   AND status IN ('pending', 'retry_scheduled')",
                params![now, unit_id, job_id],
            )?;
            if changed != 1 {
                return Err(anyhow!("unit claim lost a concurrency race"));
            }
            let unit = load_unit(&tx, &unit_id)?
                .ok_or_else(|| anyhow!("claimed unit was not found"))?;
            tx.commit()?;
            Ok(Some(unit))
        })
        .await
    }

    pub async fn schedule_retry(
        &self,
        unit_id: &str,
        error: ScrapeJobError,
        next_retry_at: String,
    ) -> Result<ScrapeUnit> {
        if !error.retryable {
            return Err(anyhow!(
                "non-retryable errors cannot be scheduled for retry"
            ));
        }
        let unit_id = unit_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let changed = tx.execute(
                "UPDATE google_maps_scrape_units
                 SET status='retry_scheduled', next_retry_at=?1, last_error_kind=?2,
                     last_error_message=?3, updated_at=?4
                 WHERE id=?5 AND status='running'",
                params![
                    next_retry_at,
                    enum_to_text(&error.kind)?,
                    error.message,
                    chrono::Utc::now().to_rfc3339(),
                    unit_id,
                ],
            )?;
            if changed != 1 {
                return Err(anyhow!("only a running unit can be scheduled for retry"));
            }
            let unit =
                load_unit(&tx, &unit_id)?.ok_or_else(|| anyhow!("scheduled unit was not found"))?;
            tx.commit()?;
            Ok(unit)
        })
        .await
    }

    pub async fn checkpoint_completed_unit(
        &self,
        job_id: &str,
        unit_id: &str,
        results: Vec<ScrapeResult>,
        child_units: Vec<ScrapeUnit>,
    ) -> Result<ScrapeJob> {
        let job_id = job_id.to_string();
        let unit_id = unit_id.to_string();
        let should_import_ready = !results.is_empty();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            for result in &results {
                if result.job_id != job_id || result.source_unit_id != unit_id {
                    return Err(anyhow!(
                        "checkpoint result belongs to a different job or unit"
                    ));
                }
                upsert_result(&tx, result)?;
            }

            let mut inserted_children = 0usize;
            for child in &child_units {
                if child.job_id != job_id {
                    return Err(anyhow!("child unit belongs to a different job"));
                }
                inserted_children += insert_unit(&tx, child, true)?;
            }

            let now = chrono::Utc::now().to_rfc3339();
            let changed = tx.execute(
                "UPDATE google_maps_scrape_units
                 SET status='completed', completed_at=?1, updated_at=?1,
                     next_retry_at=NULL, last_error_kind=NULL, last_error_message=NULL
                 WHERE id=?2 AND job_id=?3 AND status='running'",
                params![now, unit_id, job_id],
            )?;
            if changed != 1 {
                return Err(anyhow!("checkpoint requires a running unit"));
            }

            let changed = tx.execute(
                "UPDATE google_maps_scrape_jobs
                 SET total_units=total_units + ?1,
                     processed_units=processed_units + 1,
                     businesses_discovered=(
                         SELECT COUNT(*) FROM google_maps_scrape_results WHERE job_id=?2
                     ),
                     event_sequence=event_sequence + 1,
                     version=version + 1,
                     updated_at=?3
                 WHERE id=?2 AND status='running'",
                params![inserted_children as i64, job_id, now],
            )?;
            if changed != 1 {
                return Err(anyhow!("checkpoint requires a running job"));
            }
            tx.commit()?;
            if should_import_ready {
                crate::google_maps_jobs::importer::import_ready_results(&mut conn, &job_id)
                    .map_err(anyhow::Error::from)?;
            }
            load_job(&conn, &job_id)?.ok_or_else(|| anyhow!("checkpointed job was not found"))
        })
        .await
    }

    pub async fn checkpoint_failed_unit(
        &self,
        job_id: &str,
        unit_id: &str,
        error: ScrapeJobError,
    ) -> Result<ScrapeJob> {
        let job_id = job_id.to_string();
        let unit_id = unit_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let now = chrono::Utc::now().to_rfc3339();
            let error_kind = enum_to_text(&error.kind)?;
            let changed = tx.execute(
                "UPDATE google_maps_scrape_units
                 SET status='failed', completed_at=?1, updated_at=?1,
                     last_error_kind=?2, last_error_message=?3
                 WHERE id=?4 AND job_id=?5 AND status='running'",
                params![now, error_kind, error.message, unit_id, job_id],
            )?;
            if changed != 1 {
                return Err(anyhow!("failure checkpoint requires a running unit"));
            }
            let changed = tx.execute(
                "UPDATE google_maps_scrape_jobs
                 SET processed_units=processed_units + 1,
                     failed_units=failed_units + 1,
                     last_error_kind=?1,
                     last_error_message=?2,
                     event_sequence=event_sequence + 1,
                     version=version + 1,
                     updated_at=?3
                 WHERE id=?4 AND status='running'",
                params![enum_to_text(&error.kind)?, error.message, now, job_id],
            )?;
            if changed != 1 {
                return Err(anyhow!("failure checkpoint requires a running job"));
            }
            let job = load_job(&tx, &job_id)?
                .ok_or_else(|| anyhow!("failed job checkpoint was not found"))?;
            tx.commit()?;
            Ok(job)
        })
        .await
    }

    pub async fn pause_for_action(
        &self,
        job_id: &str,
        unit_id: &str,
        error: ScrapeJobError,
        action_required: String,
        blocked_url: Option<String>,
    ) -> Result<ScrapeJob> {
        let job_id = job_id.to_string();
        let unit_id = unit_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let now = chrono::Utc::now().to_rfc3339();
            let error_kind = enum_to_text(&error.kind)?;
            let changed = tx.execute(
                "UPDATE google_maps_scrape_units
                 SET status='pending', next_retry_at=NULL, updated_at=?1,
                     last_error_kind=?2, last_error_message=?3
                 WHERE id=?4 AND job_id=?5 AND status='running'",
                params![now, error_kind, error.message, unit_id, job_id],
            )?;
            if changed != 1 {
                return Err(anyhow!("pause requires a running unit"));
            }
            let changed = tx.execute(
                "UPDATE google_maps_scrape_jobs
                 SET status='paused_requires_action', last_error_kind=?1,
                     last_error_message=?2, action_required=?3, blocked_url=?4, updated_at=?5,
                     completed_at=NULL, event_sequence=event_sequence + 1,
                     version=version + 1
                 WHERE id=?6 AND status='running'",
                params![
                    enum_to_text(&error.kind)?,
                    error.message,
                    action_required,
                    blocked_url,
                    now,
                    job_id
                ],
            )?;
            if changed != 1 {
                return Err(anyhow!("pause requires a running job"));
            }
            let job = load_job(&tx, &job_id)?.ok_or_else(|| anyhow!("paused job was not found"))?;
            tx.commit()?;
            Ok(job)
        })
        .await
    }

    pub async fn cancel_job(&self, job_id: &str) -> Result<ScrapeJob> {
        let job_id = job_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let current = load_job(&tx, &job_id)?
                .ok_or_else(|| anyhow!("scrape job {job_id} was not found"))?;
            if current.status == ScrapeJobStatus::Cancelled {
                tx.commit()?;
                return Ok(current);
            }
            if !is_valid_transition(
                current.status,
                ScrapeJobStatus::Cancelled,
                TransitionCause::UserAction,
            ) {
                return Err(anyhow!("job cannot be cancelled from {:?}", current.status));
            }
            let now = chrono::Utc::now().to_rfc3339();
            tx.execute(
                "UPDATE google_maps_scrape_units
                 SET status='cancelled', completed_at=?1, updated_at=?1,
                     next_retry_at=NULL
                 WHERE job_id=?2 AND status IN ('pending', 'running', 'retry_scheduled')",
                params![now, job_id],
            )?;
            let changed = tx.execute(
                "UPDATE google_maps_scrape_jobs
                 SET status='cancelled', completed_at=?1, updated_at=?1,
                     action_required=NULL, event_sequence=event_sequence + 1,
                     version=version + 1
                 WHERE id=?2 AND version=?3",
                params![now, job_id, current.version as i64],
            )?;
            if changed != 1 {
                return Err(anyhow!("cancel lost a job version race"));
            }
            let job =
                load_job(&tx, &job_id)?.ok_or_else(|| anyhow!("cancelled job was not found"))?;
            tx.commit()?;
            Ok(job)
        })
        .await
    }

    pub async fn complete_job_if_exhausted(&self, job_id: &str) -> Result<Option<ScrapeJob>> {
        let job_id = job_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let open_units: i64 = tx.query_row(
                "SELECT COUNT(*) FROM google_maps_scrape_units
                 WHERE job_id=?1 AND status IN ('pending', 'running', 'retry_scheduled')",
                params![job_id],
                |row| row.get(0),
            )?;
            if open_units != 0 {
                tx.commit()?;
                return Ok(None);
            }
            // Failed units mean the job did not cover everything: never report
            // it as a clean completion (A2).
            let failed_units: i64 = tx.query_row(
                "SELECT COUNT(*) FROM google_maps_scrape_units
                 WHERE job_id=?1 AND status='failed'",
                params![job_id],
                |row| row.get(0),
            )?;
            let final_status = if failed_units > 0 {
                "partial"
            } else {
                "completed"
            };
            let now = chrono::Utc::now().to_rfc3339();
            let changed = tx.execute(
                "UPDATE google_maps_scrape_jobs
                 SET status=?3, completed_at=?1, updated_at=?1,
                     action_required=NULL, event_sequence=event_sequence + 1,
                     version=version + 1
                 WHERE id=?2 AND status='running'",
                params![now, job_id, final_status],
            )?;
            if changed != 1 {
                tx.commit()?;
                return Ok(None);
            }
            let job =
                load_job(&tx, &job_id)?.ok_or_else(|| anyhow!("completed job was not found"))?;
            tx.commit()?;
            Ok(Some(job))
        })
        .await
    }

    pub async fn fail_job(&self, job_id: &str, error: ScrapeJobError) -> Result<ScrapeJob> {
        let job_id = job_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let current = load_job(&tx, &job_id)?
                .ok_or_else(|| anyhow!("scrape job {job_id} was not found"))?;
            if current.status != ScrapeJobStatus::Running {
                tx.commit()?;
                return Ok(current);
            }
            let now = chrono::Utc::now().to_rfc3339();
            let failed_now = tx.execute(
                "UPDATE google_maps_scrape_units
                 SET status='failed', completed_at=?1, updated_at=?1,
                     last_error_kind=?2, last_error_message=?3
                 WHERE job_id=?4 AND status='running'",
                params![now, enum_to_text(&error.kind)?, error.message, job_id],
            )?;
            // Units failed here must be counted, otherwise a later user retry
            // subtracts from counters that never included them (A2).
            let changed = tx.execute(
                "UPDATE google_maps_scrape_jobs
                 SET status='failed', completed_at=?1, updated_at=?1,
                     last_error_kind=?2, last_error_message=?3,
                     processed_units=processed_units + ?5,
                     failed_units=failed_units + ?5,
                     event_sequence=event_sequence + 1, version=version + 1
                 WHERE id=?4 AND status='running'",
                params![
                    now,
                    enum_to_text(&error.kind)?,
                    error.message,
                    job_id,
                    failed_now as i64
                ],
            )?;
            if changed != 1 {
                return Err(anyhow!("failed job transition lost a state race"));
            }
            let job = load_job(&tx, &job_id)?.ok_or_else(|| anyhow!("failed job was not found"))?;
            tx.commit()?;
            Ok(job)
        })
        .await
    }

    pub async fn upsert_results(&self, job_id: &str, results: Vec<ScrapeResult>) -> Result<usize> {
        let job_id = job_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            for result in &results {
                if result.job_id != job_id {
                    return Err(anyhow!("result belongs to a different job"));
                }
                upsert_result(&tx, result)?;
            }
            let changed = tx.execute(
                "UPDATE google_maps_scrape_jobs
                 SET businesses_discovered=(
                         SELECT COUNT(*) FROM google_maps_scrape_results WHERE job_id=?1
                     ),
                     event_sequence=event_sequence + 1,
                     version=version + 1,
                     updated_at=?2
                 WHERE id=?1",
                params![job_id, chrono::Utc::now().to_rfc3339()],
            )?;
            if changed != 1 {
                return Err(anyhow!("scrape job was not found while upserting results"));
            }
            tx.commit()?;
            Ok(results.len())
        })
        .await
    }

    pub async fn list_results_for_enrichment(&self, job_id: &str) -> Result<Vec<ScrapeResult>> {
        let job_id = job_id.to_string();
        self.with_connection(move |conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {RESULT_COLUMNS} FROM google_maps_scrape_results
                 WHERE job_id=?1
                   AND (normalized_phone IS NULL OR trim(normalized_phone)='')
                   AND website IS NOT NULL AND trim(website)<>''
                   AND enrichment_status IN ('pending', 'retry_wait')
                   AND import_status='pending'
                 ORDER BY first_seen_at ASC, id ASC"
            ))?;
            let rows = stmt.query_map(params![job_id], result_from_row)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into)
        })
        .await
    }

    pub async fn claim_next_result_for_enrichment(
        &self,
        job_id: &str,
    ) -> Result<Option<ScrapeResult>> {
        let job_id = job_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let result_id: Option<String> = tx
                .query_row(
                    "SELECT id FROM google_maps_scrape_results
                     WHERE job_id=?1
                       AND (normalized_phone IS NULL OR trim(normalized_phone)='')
                       AND website IS NOT NULL AND trim(website)<>''
                       AND enrichment_status IN ('pending', 'retry_wait')
                     ORDER BY first_seen_at ASC, id ASC
                     LIMIT 1",
                    params![job_id],
                    |row| row.get(0),
                )
                .optional()?;
            let Some(result_id) = result_id else {
                tx.commit()?;
                return Ok(None);
            };
            let changed = tx.execute(
                "UPDATE google_maps_scrape_results
                 SET enrichment_status='running', last_error_kind=NULL,
                     last_error_message=NULL
                 WHERE id=?1 AND job_id=?2
                   AND enrichment_status IN ('pending', 'retry_wait')",
                params![result_id, job_id],
            )?;
            if changed != 1 {
                return Err(anyhow!("enrichment claim lost a result state race"));
            }
            let result = load_result(&tx, &result_id)?
                .ok_or_else(|| anyhow!("claimed enrichment result was not found"))?;
            tx.commit()?;
            Ok(Some(result))
        })
        .await
    }

    pub async fn save_enrichment_outcome(
        &self,
        job_id: &str,
        result_id: &str,
        outcome: EnrichmentOutcome,
    ) -> Result<ScrapeResult> {
        if matches!(
            outcome.status,
            EnrichmentStatus::Pending | EnrichmentStatus::Running
        ) {
            return Err(anyhow!("enrichment outcome must be quiescent"));
        }
        let normalized_email = outcome.email.as_deref().and_then(normalize_email);
        if outcome.status == EnrichmentStatus::Found && normalized_email.is_none() {
            return Err(anyhow!("found enrichment outcome requires a valid email"));
        }
        if outcome.status != EnrichmentStatus::Found && outcome.email.is_some() {
            return Err(anyhow!(
                "non-found enrichment outcome must not contain email"
            ));
        }
        let job_id = job_id.to_string();
        let result_id = result_id.to_string();
        let status = outcome.status.as_str().to_string();
        let error_kind = outcome
            .error
            .as_ref()
            .map(|error| enum_to_text(&error.kind))
            .transpose()?;
        let error_message = outcome.error.map(|error| error.message);
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let changed = tx.execute(
                "UPDATE google_maps_scrape_results
                 SET email=?1, normalized_email=?1, enrichment_status=?2,
                     last_error_kind=?3, last_error_message=?4
                 WHERE id=?5 AND job_id=?6 AND enrichment_status='running'",
                params![
                    normalized_email,
                    status,
                    error_kind,
                    error_message,
                    result_id,
                    job_id
                ],
            )?;
            if changed != 1 {
                return Err(anyhow!("enrichment checkpoint requires a running result"));
            }
            let changed = tx.execute(
                "UPDATE google_maps_scrape_jobs
                 SET event_sequence=event_sequence + 1, version=version + 1,
                     updated_at=?1
                 WHERE id=?2",
                params![chrono::Utc::now().to_rfc3339(), job_id],
            )?;
            if changed != 1 {
                return Err(anyhow!("enrichment checkpoint job was not found"));
            }
            let result = load_result(&tx, &result_id)?
                .ok_or_else(|| anyhow!("enriched result was not found"))?;
            tx.commit()?;
            Ok(result)
        })
        .await
    }

    pub async fn list_results_for_import(&self, job_id: &str) -> Result<Vec<ScrapeResult>> {
        let job_id = job_id.to_string();
        self.with_connection(move |conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {RESULT_COLUMNS} FROM google_maps_scrape_results
                 WHERE job_id=?1
                   AND import_status='pending'
                   AND inside_requested_area=1
                   AND (
                       (normalized_phone IS NOT NULL AND trim(normalized_phone)<>'')
                       OR (normalized_email IS NOT NULL AND trim(normalized_email)<>'')
                   )
                 ORDER BY first_seen_at ASC, id ASC"
            ))?;
            let rows = stmt.query_map(params![job_id], result_from_row)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into)
        })
        .await
    }

    pub async fn delete_terminal_job(&self, job_id: &str) -> Result<bool> {
        let job_id = job_id.to_string();
        self.with_connection(move |mut conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let job = load_job(&tx, &job_id)?
                .ok_or_else(|| anyhow!("scrape job {job_id} was not found"))?;
            if !job.status.is_terminal() {
                return Err(anyhow!("only terminal jobs can be deleted"));
            }
            let deleted = tx.execute(
                "DELETE FROM google_maps_scrape_jobs WHERE id=?1",
                params![job_id],
            )?;
            tx.commit()?;
            Ok(deleted == 1)
        })
        .await
    }

    pub async fn list_retention_profile_dirs(
        &self,
        now: chrono::DateTime<chrono::Utc>,
        profile_retention_days: i64,
    ) -> Result<Vec<String>> {
        if profile_retention_days <= 0 {
            return Err(anyhow!("profile retention period must be positive"));
        }
        let profile_cutoff = (now - chrono::Duration::days(profile_retention_days)).to_rfc3339();
        self.with_connection(move |conn| {
            let mut statement = conn.prepare(
                "SELECT DISTINCT profile_dir
                 FROM google_maps_scrape_jobs
                 WHERE status IN ('partial', 'completed', 'failed', 'cancelled')
                   AND action_required IS NULL
                   AND trim(profile_dir) <> ''
                   AND julianday(COALESCE(completed_at, updated_at)) < julianday(?1)",
            )?;
            let rows = statement.query_map(params![profile_cutoff], |row| row.get(0))?;
            rows.collect::<rusqlite::Result<Vec<String>>>()
                .map_err(Into::into)
        })
        .await
    }

    pub async fn delete_expired_terminal_jobs(
        &self,
        now: chrono::DateTime<chrono::Utc>,
        job_retention_days: i64,
    ) -> Result<usize> {
        if job_retention_days <= 0 {
            return Err(anyhow!("job retention period must be positive"));
        }
        let job_cutoff = (now - chrono::Duration::days(job_retention_days)).to_rfc3339();
        self.with_connection(move |conn| {
            conn.execute(
                "DELETE FROM google_maps_scrape_jobs
                 WHERE status IN ('partial', 'completed', 'failed', 'cancelled')
                   AND completed_at IS NOT NULL
                   AND julianday(completed_at) < julianday(?1)",
                params![job_cutoff],
            )
            .map_err(Into::into)
        })
        .await
    }
}

pub fn is_valid_transition(
    from: ScrapeJobStatus,
    to: ScrapeJobStatus,
    cause: TransitionCause,
) -> bool {
    use ScrapeJobStatus::*;

    if matches!(from, Partial | Failed) && to == Queued {
        return cause == TransitionCause::UserRetry;
    }

    matches!(
        (from, to),
        (Queued, Running)
            | (Queued, Cancelled)
            | (Running, Paused)
            | (Running, PausedRequiresAction)
            | (Running, PausedLimitReached)
            | (Running, Interrupted)
            | (Running, Partial)
            | (Running, Completed)
            | (Running, Failed)
            | (Running, Cancelled)
            | (Paused, Running)
            | (Paused, Cancelled)
            | (PausedRequiresAction, Running)
            | (PausedRequiresAction, Cancelled)
            | (PausedLimitReached, Running)
            | (PausedLimitReached, Cancelled)
            | (Interrupted, Running)
            | (Interrupted, Cancelled)
    )
}

fn enum_to_text<T: Serialize>(value: &T) -> Result<String> {
    let encoded = serde_json::to_string(value)?;
    Ok(encoded.trim_matches('"').to_string())
}

fn insert_job(conn: &Connection, job: &ScrapeJob) -> Result<usize> {
    let bbox_json = job
        .bbox
        .map(|bbox| serde_json::to_string(&bbox))
        .transpose()?;
    conn.execute(
        "INSERT INTO google_maps_scrape_jobs (
            id, mode, keyword, location, bbox_json, step_km, headless, profile_dir,
            blocked_url, status, stage, total_units, processed_units, failed_units,
            businesses_discovered, businesses_qualified, leads_created, last_error_kind,
            last_error_message, action_required, event_sequence, version, run_generation,
            created_at, started_at, updated_at, completed_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
            ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27
         )",
        params![
            job.id,
            enum_to_text(&job.mode)?,
            job.keyword,
            job.location,
            bbox_json,
            job.step_km,
            i64::from(job.headless),
            job.profile_dir,
            job.blocked_url,
            enum_to_text(&job.status)?,
            enum_to_text(&job.stage)?,
            job.total_units as i64,
            job.processed_units as i64,
            job.failed_units as i64,
            job.businesses_discovered as i64,
            job.businesses_qualified as i64,
            job.leads_created as i64,
            job.last_error_kind
                .map(|kind| enum_to_text(&kind))
                .transpose()?,
            job.last_error_message,
            job.action_required,
            job.event_sequence as i64,
            job.version as i64,
            job.run_generation as i64,
            job.created_at,
            job.started_at,
            job.updated_at,
            job.completed_at,
        ],
    )
    .map_err(Into::into)
}

fn enum_from_text<T: DeserializeOwned>(value: String, column: usize) -> rusqlite::Result<T> {
    serde_json::from_value(serde_json::Value::String(value)).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(column, Type::Text, Box::new(error))
    })
}

fn optional_enum_from_text<T: DeserializeOwned>(
    value: Option<String>,
    column: usize,
) -> rusqlite::Result<Option<T>> {
    value.map(|value| enum_from_text(value, column)).transpose()
}

fn job_from_row(row: &Row<'_>) -> rusqlite::Result<ScrapeJob> {
    let bbox_json: Option<String> = row.get(4)?;
    let bbox = bbox_json
        .map(|value| {
            serde_json::from_str(&value).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(4, Type::Text, Box::new(error))
            })
        })
        .transpose()?;
    Ok(ScrapeJob {
        id: row.get(0)?,
        mode: enum_from_text(row.get(1)?, 1)?,
        keyword: row.get(2)?,
        location: row.get(3)?,
        bbox,
        step_km: row.get(5)?,
        headless: row.get::<_, i64>(6)? != 0,
        profile_dir: row.get(7)?,
        blocked_url: row.get(8)?,
        status: enum_from_text(row.get(9)?, 9)?,
        stage: enum_from_text(row.get(10)?, 10)?,
        total_units: row.get::<_, i64>(11)? as u64,
        processed_units: row.get::<_, i64>(12)? as u64,
        failed_units: row.get::<_, i64>(13)? as u64,
        businesses_discovered: row.get::<_, i64>(14)? as u64,
        businesses_qualified: row.get::<_, i64>(15)? as u64,
        leads_created: row.get::<_, i64>(16)? as u64,
        last_error_kind: optional_enum_from_text(row.get(17)?, 17)?,
        last_error_message: row.get(18)?,
        action_required: row.get(19)?,
        event_sequence: row.get::<_, i64>(20)? as u64,
        version: row.get::<_, i64>(21)? as u64,
        run_generation: row.get::<_, i64>(22)? as u64,
        created_at: row.get(23)?,
        started_at: row.get(24)?,
        updated_at: row.get(25)?,
        completed_at: row.get(26)?,
        coverage_truncated: row.get::<_, i64>(27)? != 0,
    })
}

fn unit_from_row(row: &Row<'_>) -> rusqlite::Result<ScrapeUnit> {
    let payload_json: String = row.get(4)?;
    let payload = serde_json::from_str(&payload_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, Type::Text, Box::new(error))
    })?;
    Ok(ScrapeUnit {
        id: row.get(0)?,
        job_id: row.get(1)?,
        unit_key: row.get(2)?,
        unit_type: row.get(3)?,
        payload,
        status: enum_from_text(row.get(5)?, 5)?,
        depth: row.get::<_, i64>(6)? as u8,
        attempt_count: row.get::<_, i64>(7)? as u32,
        next_retry_at: row.get(8)?,
        last_error_kind: optional_enum_from_text(row.get(9)?, 9)?,
        last_error_message: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
        completed_at: row.get(13)?,
    })
}

fn result_from_row(row: &Row<'_>) -> rusqlite::Result<ScrapeResult> {
    Ok(ScrapeResult {
        id: row.get(0)?,
        job_id: row.get(1)?,
        source_unit_id: row.get(2)?,
        place_id: row.get(3)?,
        canonical_url: row.get(4)?,
        dedup_key: row.get(5)?,
        name: row.get(6)?,
        phone: row.get(7)?,
        normalized_phone: row.get(8)?,
        email: row.get(9)?,
        normalized_email: row.get(10)?,
        address: row.get(11)?,
        website: row.get(12)?,
        facebook: row.get(13)?,
        zalo: row.get(14)?,
        latitude: row.get(15)?,
        longitude: row.get(16)?,
        inside_requested_area: row.get::<_, Option<i64>>(17)?.map(|value| value != 0),
        detail_status: row.get(18)?,
        enrichment_status: row.get(19)?,
        import_status: row.get(20)?,
        last_error_kind: optional_enum_from_text(row.get(21)?, 21)?,
        last_error_message: row.get(22)?,
        first_seen_at: row.get(23)?,
        last_seen_at: row.get(24)?,
    })
}

fn load_job(conn: &Connection, job_id: &str) -> Result<Option<ScrapeJob>> {
    conn.query_row(
        &format!("SELECT {JOB_COLUMNS} FROM google_maps_scrape_jobs WHERE id=?1"),
        params![job_id],
        job_from_row,
    )
    .optional()
    .map_err(Into::into)
}

fn load_unit(conn: &Connection, unit_id: &str) -> Result<Option<ScrapeUnit>> {
    conn.query_row(
        &format!("SELECT {UNIT_COLUMNS} FROM google_maps_scrape_units WHERE id=?1"),
        params![unit_id],
        unit_from_row,
    )
    .optional()
    .map_err(Into::into)
}

fn load_result(conn: &Connection, result_id: &str) -> Result<Option<ScrapeResult>> {
    conn.query_row(
        &format!("SELECT {RESULT_COLUMNS} FROM google_maps_scrape_results WHERE id=?1"),
        params![result_id],
        result_from_row,
    )
    .optional()
    .map_err(Into::into)
}

fn insert_unit(tx: &Transaction<'_>, unit: &ScrapeUnit, ignore_duplicate: bool) -> Result<usize> {
    let insert_mode = if ignore_duplicate {
        "INSERT OR IGNORE"
    } else {
        "INSERT"
    };
    let sql = format!(
        "{insert_mode} INTO google_maps_scrape_units (
            id, job_id, unit_key, unit_type, payload_json, status, depth, attempt_count,
            next_retry_at, last_error_kind, last_error_message, created_at, updated_at, completed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
    );
    tx.execute(
        &sql,
        params![
            unit.id,
            unit.job_id,
            unit.unit_key,
            unit.unit_type,
            serde_json::to_string(&unit.payload)?,
            enum_to_text(&unit.status)?,
            i64::from(unit.depth),
            i64::from(unit.attempt_count),
            unit.next_retry_at,
            unit.last_error_kind
                .map(|kind| enum_to_text(&kind))
                .transpose()?,
            unit.last_error_message,
            unit.created_at,
            unit.updated_at,
            unit.completed_at,
        ],
    )
    .map_err(Into::into)
}

fn upsert_result(tx: &Transaction<'_>, result: &ScrapeResult) -> Result<usize> {
    tx.execute(
        "INSERT INTO google_maps_scrape_results (
            id, job_id, source_unit_id, place_id, canonical_url, dedup_key, name, phone,
            normalized_phone, email, normalized_email, address, website, facebook, zalo,
            latitude, longitude, inside_requested_area, detail_status, enrichment_status,
            import_status, last_error_kind, last_error_message, first_seen_at, last_seen_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
            ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25
         )
         ON CONFLICT(job_id, dedup_key) DO UPDATE SET
            source_unit_id=excluded.source_unit_id,
            place_id=COALESCE(excluded.place_id, google_maps_scrape_results.place_id),
            canonical_url=COALESCE(excluded.canonical_url, google_maps_scrape_results.canonical_url),
            name=excluded.name,
            phone=COALESCE(excluded.phone, google_maps_scrape_results.phone),
            normalized_phone=COALESCE(excluded.normalized_phone, google_maps_scrape_results.normalized_phone),
            email=COALESCE(excluded.email, google_maps_scrape_results.email),
            normalized_email=COALESCE(excluded.normalized_email, google_maps_scrape_results.normalized_email),
            address=COALESCE(excluded.address, google_maps_scrape_results.address),
            website=COALESCE(excluded.website, google_maps_scrape_results.website),
            facebook=COALESCE(excluded.facebook, google_maps_scrape_results.facebook),
            zalo=COALESCE(excluded.zalo, google_maps_scrape_results.zalo),
            latitude=COALESCE(excluded.latitude, google_maps_scrape_results.latitude),
            longitude=COALESCE(excluded.longitude, google_maps_scrape_results.longitude),
            inside_requested_area=COALESCE(excluded.inside_requested_area, google_maps_scrape_results.inside_requested_area),
            detail_status=excluded.detail_status,
            enrichment_status=excluded.enrichment_status,
            import_status=excluded.import_status,
            last_error_kind=excluded.last_error_kind,
            last_error_message=excluded.last_error_message,
            last_seen_at=excluded.last_seen_at",
        params![
            result.id,
            result.job_id,
            result.source_unit_id,
            result.place_id,
            result.canonical_url,
            result.dedup_key,
            result.name,
            result.phone,
            result.normalized_phone,
            result.email,
            result.normalized_email,
            result.address,
            result.website,
            result.facebook,
            result.zalo,
            result.latitude,
            result.longitude,
            result.inside_requested_area.map(i64::from),
            result.detail_status,
            result.enrichment_status,
            result.import_status,
            result.last_error_kind
                .map(|kind| enum_to_text(&kind))
                .transpose()?,
            result.last_error_message,
            result.first_seen_at,
            result.last_seen_at,
        ],
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_db_connection;
    use crate::google_maps_jobs::email::{EnrichmentOutcome, EnrichmentStatus};
    use crate::google_maps_jobs::types::{
        ScrapeErrorKind, ScrapeJob, ScrapeJobError, ScrapeJobStatus, ScrapeMode, ScrapeResult,
        ScrapeStage, ScrapeUnit, ScrapeUnitStatus, TransitionCause,
    };
    use std::path::PathBuf;

    struct TestRepository {
        repository: ScrapeJobRepository,
        path: PathBuf,
    }

    impl TestRepository {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "rustsale-google-maps-repository-{}.db",
                rand::random::<u64>()
            ));
            let repository = ScrapeJobRepository::new(path.to_string_lossy().to_string()).unwrap();
            Self { repository, path }
        }

        fn connection(&self) -> rusqlite::Connection {
            open_db_connection(self.path.to_str().unwrap()).unwrap()
        }
    }

    impl Drop for TestRepository {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_file(self.path.with_extension("db-wal"));
            let _ = std::fs::remove_file(self.path.with_extension("db-shm"));
        }
    }

    fn sample_job(status: ScrapeJobStatus) -> ScrapeJob {
        ScrapeJob {
            id: "job-1".to_string(),
            mode: ScrapeMode::Keyword,
            keyword: "coffee".to_string(),
            location: Some("Hà Nội".to_string()),
            bbox: None,
            step_km: None,
            headless: true,
            profile_dir: "/tmp/rustsale-google-maps/job-1/profile".to_string(),
            blocked_url: None,
            status,
            stage: ScrapeStage::Planning,
            total_units: 0,
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
            created_at: "2026-07-26T00:00:00Z".to_string(),
            started_at: None,
            updated_at: "2026-07-26T00:00:00Z".to_string(),
            completed_at: status
                .is_terminal()
                .then(|| "2026-07-26T00:01:00Z".to_string()),
            coverage_truncated: false,
        }
    }

    fn sample_unit(id: &str, key: &str, status: ScrapeUnitStatus) -> ScrapeUnit {
        ScrapeUnit {
            id: id.to_string(),
            job_id: "job-1".to_string(),
            unit_key: key.to_string(),
            unit_type: "keyword".to_string(),
            payload: serde_json::json!({"query": "coffee"}),
            status,
            depth: 0,
            attempt_count: 0,
            next_retry_at: None,
            last_error_kind: None,
            last_error_message: None,
            created_at: "2026-07-26T00:00:00Z".to_string(),
            updated_at: "2026-07-26T00:00:00Z".to_string(),
            completed_at: None,
        }
    }

    fn sample_result() -> ScrapeResult {
        ScrapeResult {
            id: "result-1".to_string(),
            job_id: "job-1".to_string(),
            source_unit_id: "unit-1".to_string(),
            place_id: Some("place-1".to_string()),
            canonical_url: Some("https://www.google.com/maps/place/place-1".to_string()),
            dedup_key: "place:place-1".to_string(),
            name: "Coffee Shop".to_string(),
            phone: Some("0901234567".to_string()),
            normalized_phone: Some("84901234567".to_string()),
            email: None,
            normalized_email: None,
            address: Some("Hà Nội".to_string()),
            website: None,
            facebook: None,
            zalo: None,
            latitude: Some(21.0285),
            longitude: Some(105.8542),
            inside_requested_area: Some(true),
            detail_status: "completed".to_string(),
            enrichment_status: "skipped".to_string(),
            import_status: "pending".to_string(),
            last_error_kind: None,
            last_error_message: None,
            first_seen_at: "2026-07-26T00:00:00Z".to_string(),
            last_seen_at: "2026-07-26T00:00:00Z".to_string(),
        }
    }

    /// A2: units failed by `fail_job` must be counted, a user retry must be
    /// able to reset them (kind `database`), and a job with failed units must
    /// never be reported as cleanly completed.
    #[tokio::test]
    async fn fail_job_counts_failed_units_and_retry_resets_them() {
        let context = TestRepository::new();
        let mut job = sample_job(ScrapeJobStatus::Running);
        job.total_units = 1;
        context.repository.create_job(job).await.unwrap();
        context
            .repository
            .insert_units(
                "job-1",
                vec![sample_unit(
                    "unit-1",
                    "keyword:coffee",
                    ScrapeUnitStatus::Running,
                )],
            )
            .await
            .unwrap();

        let failed = context
            .repository
            .fail_job(
                "job-1",
                ScrapeJobError {
                    kind: ScrapeErrorKind::Database,
                    message: "scrape runner failed".to_string(),
                    retryable: false,
                },
            )
            .await
            .unwrap();
        assert_eq!(failed.failed_units, 1);
        assert_eq!(failed.processed_units, 1);

        let queued = context
            .repository
            .transition_job(
                "job-1",
                failed.version,
                ScrapeJobStatus::Queued,
                TransitionCause::UserRetry,
            )
            .await
            .unwrap();
        let units = context.repository.list_units("job-1").await.unwrap();
        assert_eq!(units[0].status, ScrapeUnitStatus::Pending);
        assert_eq!(queued.failed_units, 0);
        assert_eq!(queued.processed_units, 0);
    }

    #[tokio::test]
    async fn job_with_failed_unit_never_completes() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Running))
            .await
            .unwrap();
        context
            .repository
            .insert_units(
                "job-1",
                vec![sample_unit(
                    "unit-1",
                    "keyword:coffee",
                    ScrapeUnitStatus::Failed,
                )],
            )
            .await
            .unwrap();

        let job = context
            .repository
            .complete_job_if_exhausted("job-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(job.status, ScrapeJobStatus::Partial);
    }

    #[tokio::test]
    async fn terminal_job_rejects_later_transition() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Completed))
            .await
            .unwrap();

        let error = context
            .repository
            .transition_job(
                "job-1",
                0,
                ScrapeJobStatus::Running,
                TransitionCause::System,
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("invalid transition"));
        assert_eq!(
            context
                .repository
                .get_job("job-1")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeJobStatus::Completed
        );
    }

    #[tokio::test]
    async fn startup_recovery_marks_running_jobs_interrupted() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Running))
            .await
            .unwrap();
        context
            .repository
            .insert_units(
                "job-1",
                vec![sample_unit(
                    "unit-1",
                    "keyword:coffee",
                    ScrapeUnitStatus::Running,
                )],
            )
            .await
            .unwrap();

        assert_eq!(
            context.repository.recover_interrupted_jobs().await.unwrap(),
            1
        );
        assert_eq!(
            context
                .repository
                .get_job("job-1")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeJobStatus::Interrupted
        );
        let unit_status: String = context
            .connection()
            .query_row(
                "SELECT status FROM google_maps_scrape_units WHERE id='unit-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unit_status, "pending");
    }

    #[tokio::test]
    async fn optimistic_version_rejects_stale_transition() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Queued))
            .await
            .unwrap();

        let error = context
            .repository
            .transition_job(
                "job-1",
                4,
                ScrapeJobStatus::Running,
                TransitionCause::System,
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("stale job version"));
    }

    #[tokio::test]
    async fn user_retry_reopens_partial_and_increments_generation() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Running))
            .await
            .unwrap();
        let mut unit = sample_unit("unit-1", "keyword:coffee", ScrapeUnitStatus::Failed);
        unit.last_error_kind = Some(ScrapeErrorKind::TransientNetwork);
        context
            .repository
            .insert_units("job-1", vec![unit])
            .await
            .unwrap();
        context
            .repository
            .transition_job(
                "job-1",
                1,
                ScrapeJobStatus::Partial,
                TransitionCause::System,
            )
            .await
            .unwrap();

        assert!(context
            .repository
            .transition_job("job-1", 2, ScrapeJobStatus::Queued, TransitionCause::System,)
            .await
            .is_err());

        let job = context
            .repository
            .transition_job(
                "job-1",
                2,
                ScrapeJobStatus::Queued,
                TransitionCause::UserRetry,
            )
            .await
            .unwrap();
        assert_eq!(job.status, ScrapeJobStatus::Queued);
        assert_eq!(job.run_generation, 1);
        assert_eq!(job.version, 3);

        let unit_status: String = context
            .connection()
            .query_row(
                "SELECT status FROM google_maps_scrape_units WHERE id='unit-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unit_status, "pending");
    }

    #[tokio::test]
    async fn user_retry_reopens_failed_selector_unit_without_resetting_completed_work() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Running))
            .await
            .unwrap();
        context
            .repository
            .insert_units(
                "job-1",
                vec![
                    sample_unit(
                        "unit-completed",
                        "keyword:completed",
                        ScrapeUnitStatus::Running,
                    ),
                    sample_unit("unit-failed", "keyword:failed", ScrapeUnitStatus::Running),
                ],
            )
            .await
            .unwrap();
        context
            .repository
            .checkpoint_completed_unit("job-1", "unit-completed", Vec::new(), Vec::new())
            .await
            .unwrap();
        let checkpoint = context
            .repository
            .checkpoint_failed_unit(
                "job-1",
                "unit-failed",
                ScrapeJobError {
                    kind: ScrapeErrorKind::SelectorChanged,
                    message: "selector changed".to_string(),
                    retryable: false,
                },
            )
            .await
            .unwrap();
        let failed = context
            .repository
            .transition_job(
                "job-1",
                checkpoint.version,
                ScrapeJobStatus::Failed,
                TransitionCause::System,
            )
            .await
            .unwrap();

        let queued = context
            .repository
            .transition_job(
                "job-1",
                failed.version,
                ScrapeJobStatus::Queued,
                TransitionCause::UserRetry,
            )
            .await
            .unwrap();

        assert_eq!(queued.status, ScrapeJobStatus::Queued);
        assert_eq!(queued.processed_units, 1);
        assert_eq!(queued.failed_units, 0);
        assert_eq!(queued.run_generation, 1);
        let statuses = context
            .connection()
            .prepare("SELECT id, status FROM google_maps_scrape_units ORDER BY id")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            statuses,
            vec![
                ("unit-completed".to_string(), "completed".to_string()),
                ("unit-failed".to_string(), "pending".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn duplicate_unit_batch_rolls_back_job_counter() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Queued))
            .await
            .unwrap();

        let result = context
            .repository
            .insert_units(
                "job-1",
                vec![
                    sample_unit("unit-1", "keyword:coffee", ScrapeUnitStatus::Pending),
                    sample_unit("unit-2", "keyword:coffee", ScrapeUnitStatus::Pending),
                ],
            )
            .await;
        assert!(result.is_err());

        let conn = context.connection();
        let unit_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM google_maps_scrape_units", [], |row| {
                row.get(0)
            })
            .unwrap();
        let total_units: i64 = conn
            .query_row(
                "SELECT total_units FROM google_maps_scrape_jobs WHERE id='job-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unit_count, 0);
        assert_eq!(total_units, 0);
    }

    #[tokio::test]
    async fn terminal_job_rejects_new_units() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Completed))
            .await
            .unwrap();

        let result = context
            .repository
            .insert_units(
                "job-1",
                vec![sample_unit(
                    "unit-1",
                    "keyword:coffee",
                    ScrapeUnitStatus::Pending,
                )],
            )
            .await;
        assert!(result.is_err());
        let unit_count: i64 = context
            .connection()
            .query_row("SELECT COUNT(*) FROM google_maps_scrape_units", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(unit_count, 0);
    }

    #[tokio::test]
    async fn checkpoint_rolls_back_results_when_unit_update_fails() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Running))
            .await
            .unwrap();
        context
            .repository
            .insert_units(
                "job-1",
                vec![sample_unit(
                    "unit-1",
                    "keyword:coffee",
                    ScrapeUnitStatus::Pending,
                )],
            )
            .await
            .unwrap();
        let before_job = context.repository.get_job("job-1").await.unwrap().unwrap();
        let before_unit = context
            .repository
            .get_unit("unit-1")
            .await
            .unwrap()
            .unwrap();

        let result = context
            .repository
            .checkpoint_completed_unit("job-1", "unit-1", vec![sample_result()], Vec::new())
            .await;
        assert!(result.is_err());

        let result_count: i64 = context
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM google_maps_scrape_results",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(result_count, 0);
        let job = context.repository.get_job("job-1").await.unwrap().unwrap();
        assert_eq!(job.processed_units, before_job.processed_units);
        assert_eq!(job.businesses_discovered, before_job.businesses_discovered);
        assert_eq!(job.event_sequence, before_job.event_sequence);
        assert_eq!(job.version, before_job.version);
        assert_eq!(
            context
                .repository
                .get_unit("unit-1")
                .await
                .unwrap()
                .unwrap()
                .status,
            before_unit.status
        );
    }

    #[tokio::test]
    async fn completed_checkpoint_updates_all_counters_atomically() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Running))
            .await
            .unwrap();
        context
            .repository
            .insert_units(
                "job-1",
                vec![sample_unit(
                    "unit-1",
                    "keyword:coffee",
                    ScrapeUnitStatus::Running,
                )],
            )
            .await
            .unwrap();

        let job = context
            .repository
            .checkpoint_completed_unit("job-1", "unit-1", vec![sample_result()], Vec::new())
            .await
            .unwrap();

        assert_eq!(job.total_units, 1);
        assert_eq!(job.processed_units, 1);
        assert_eq!(job.businesses_discovered, 1);
        assert_eq!(job.event_sequence, 1);
        assert_eq!(job.version, 2);

        let conn = context.connection();
        let unit_status: String = conn
            .query_row(
                "SELECT status FROM google_maps_scrape_units WHERE id='unit-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let result_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM google_maps_scrape_results",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unit_status, "completed");
        assert_eq!(result_count, 1);
    }

    #[tokio::test]
    async fn completed_checkpoint_imports_ready_result_before_finalization() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Running))
            .await
            .unwrap();
        context
            .repository
            .insert_units(
                "job-1",
                vec![sample_unit(
                    "unit-1",
                    "keyword:coffee",
                    ScrapeUnitStatus::Running,
                )],
            )
            .await
            .unwrap();
        let mut result = sample_result();
        result.enrichment_status = "not_found".to_string();

        let job = context
            .repository
            .checkpoint_completed_unit("job-1", "unit-1", vec![result], Vec::new())
            .await
            .unwrap();

        assert_eq!(job.businesses_discovered, 1);
        assert_eq!(job.businesses_qualified, 1);
        assert_eq!(job.leads_created, 1);
        let connection = context.connection();
        let lead_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM leads", [], |row| row.get(0))
            .unwrap();
        let import_status: String = connection
            .query_row(
                "SELECT import_status FROM google_maps_scrape_results WHERE id='result-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(lead_count, 1);
        assert_eq!(import_status, "imported");
    }

    #[tokio::test]
    async fn claim_and_retry_respect_attempts_and_retry_time() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Running))
            .await
            .unwrap();
        context
            .repository
            .insert_units(
                "job-1",
                vec![sample_unit(
                    "unit-1",
                    "keyword:coffee",
                    ScrapeUnitStatus::Pending,
                )],
            )
            .await
            .unwrap();

        let claimed = context
            .repository
            .claim_next_unit("job-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claimed.status, ScrapeUnitStatus::Running);
        assert_eq!(claimed.attempt_count, 1);

        assert!(context
            .repository
            .schedule_retry(
                "unit-1",
                ScrapeJobError {
                    kind: ScrapeErrorKind::SelectorChanged,
                    message: "selector changed".to_string(),
                    retryable: false,
                },
                "2999-01-01T00:00:00Z".to_string(),
            )
            .await
            .is_err());

        let scheduled = context
            .repository
            .schedule_retry(
                "unit-1",
                ScrapeJobError {
                    kind: ScrapeErrorKind::TransientNetwork,
                    message: "connection reset".to_string(),
                    retryable: true,
                },
                "2999-01-01T00:00:00Z".to_string(),
            )
            .await
            .unwrap();
        assert_eq!(scheduled.status, ScrapeUnitStatus::RetryScheduled);
        assert!(context
            .repository
            .claim_next_unit("job-1")
            .await
            .unwrap()
            .is_none());

        context
            .connection()
            .execute(
                "UPDATE google_maps_scrape_units SET next_retry_at='2000-01-01T00:00:00Z' WHERE id='unit-1'",
                [],
            )
            .unwrap();
        let reclaimed = context
            .repository
            .claim_next_unit("job-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reclaimed.status, ScrapeUnitStatus::Running);
        assert_eq!(reclaimed.attempt_count, 2);
    }

    #[tokio::test]
    async fn failed_checkpoint_updates_unit_and_job_error_state() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Running))
            .await
            .unwrap();
        context
            .repository
            .insert_units(
                "job-1",
                vec![sample_unit(
                    "unit-1",
                    "keyword:coffee",
                    ScrapeUnitStatus::Running,
                )],
            )
            .await
            .unwrap();

        let job = context
            .repository
            .checkpoint_failed_unit(
                "job-1",
                "unit-1",
                ScrapeJobError {
                    kind: ScrapeErrorKind::SelectorChanged,
                    message: "feed selector missing".to_string(),
                    retryable: false,
                },
            )
            .await
            .unwrap();

        assert_eq!(job.processed_units, 1);
        assert_eq!(job.failed_units, 1);
        assert_eq!(job.last_error_kind, Some(ScrapeErrorKind::SelectorChanged));
        assert_eq!(job.event_sequence, 1);
        assert_eq!(job.version, 2);
        let unit_status: String = context
            .connection()
            .query_row(
                "SELECT status FROM google_maps_scrape_units WHERE id='unit-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unit_status, "failed");
    }

    /// Every repository mutation reads before it writes. Under a DEFERRED
    /// transaction two such operations can both take read locks and then
    /// deadlock on upgrade, which SQLite reports as `database is locked`
    /// immediately and `busy_timeout` deliberately does not retry. Losing a
    /// claim race is a legitimate outcome; failing on lock upgrade is not.
    #[tokio::test]
    async fn concurrent_writes_never_fail_on_a_lock_upgrade() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Running))
            .await
            .unwrap();
        context
            .repository
            .insert_units(
                "job-1",
                (0..24)
                    .map(|index| {
                        sample_unit(
                            &format!("unit-{index:03}"),
                            &format!("keyword:{index}"),
                            ScrapeUnitStatus::Pending,
                        )
                    })
                    .collect(),
            )
            .await
            .unwrap();

        let repository = context.repository.clone();
        let attempts = (0..24)
            .map(|_| {
                let repository = repository.clone();
                tokio::spawn(async move { repository.claim_next_unit("job-1").await })
            })
            .collect::<Vec<_>>();

        let mut lock_failures = Vec::new();
        for attempt in attempts {
            if let Err(error) = attempt.await.unwrap() {
                let message = format!("{error:#}");
                if message.contains("database is locked")
                    || message.contains("database table is locked")
                {
                    lock_failures.push(message);
                }
            }
        }

        assert!(
            lock_failures.is_empty(),
            "concurrent claims failed on lock upgrade: {lock_failures:?}"
        );
    }

    /// A result outside the requested area can never be imported, so paying for
    /// an outbound enrichment crawl on it is pure waste.
    #[tokio::test]
    async fn enrichment_skips_results_outside_the_requested_area() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Running))
            .await
            .unwrap();
        context
            .repository
            .insert_units(
                "job-1",
                vec![sample_unit(
                    "unit-1",
                    "keyword:coffee",
                    ScrapeUnitStatus::Running,
                )],
            )
            .await
            .unwrap();

        let mut outside = sample_result();
        outside.id = "result-outside-enrichment".to_string();
        outside.dedup_key = "place:outside-enrichment".to_string();
        outside.place_id = Some("outside-enrichment".to_string());
        outside.phone = None;
        outside.normalized_phone = None;
        outside.website = Some("https://example.com".to_string());
        outside.enrichment_status = "pending".to_string();
        outside.inside_requested_area = Some(false);
        outside.import_status = "skipped_outside_area".to_string();

        context
            .repository
            .upsert_results("job-1", vec![outside])
            .await
            .unwrap();

        assert!(context
            .repository
            .list_results_for_enrichment("job-1")
            .await
            .unwrap()
            .is_empty());
    }

    /// Two grid cells routinely surface the same business. The place unit key is
    /// viewport-independent, so the second cell must not create a duplicate unit
    /// and must not fail the checkpoint that carries it.
    #[tokio::test]
    async fn a_place_found_by_two_cells_creates_one_unit() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Running))
            .await
            .unwrap();
        context
            .repository
            .insert_units(
                "job-1",
                vec![
                    sample_unit("unit-1", "grid:a", ScrapeUnitStatus::Running),
                    sample_unit("unit-2", "grid:b", ScrapeUnitStatus::Running),
                ],
            )
            .await
            .unwrap();

        let shared = |id: &str| ScrapeUnit {
            id: id.to_string(),
            unit_key: "place:https://www.google.com/maps/place/cafe+sam".to_string(),
            unit_type: "place".to_string(),
            ..sample_unit(id, "unused", ScrapeUnitStatus::Pending)
        };

        context
            .repository
            .checkpoint_completed_unit("job-1", "unit-1", Vec::new(), vec![shared("place-from-a")])
            .await
            .unwrap();
        let job = context
            .repository
            .checkpoint_completed_unit("job-1", "unit-2", Vec::new(), vec![shared("place-from-b")])
            .await
            .unwrap();

        let units = context.repository.list_units("job-1").await.unwrap();
        assert_eq!(
            units
                .iter()
                .filter(|unit| unit.unit_type == "place")
                .count(),
            1
        );
        assert_eq!(job.total_units, 3);
    }

    #[tokio::test]
    async fn result_queues_filter_enrichment_import_and_upsert_duplicates() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Running))
            .await
            .unwrap();
        context
            .repository
            .insert_units(
                "job-1",
                vec![sample_unit(
                    "unit-1",
                    "keyword:coffee",
                    ScrapeUnitStatus::Running,
                )],
            )
            .await
            .unwrap();

        let mut enrichment = sample_result();
        enrichment.id = "result-enrichment".to_string();
        enrichment.dedup_key = "place:enrichment".to_string();
        enrichment.place_id = Some("enrichment".to_string());
        enrichment.phone = None;
        enrichment.normalized_phone = None;
        enrichment.website = Some("https://example.com".to_string());
        enrichment.enrichment_status = "pending".to_string();

        let mut importable = sample_result();
        importable.id = "result-import".to_string();
        importable.dedup_key = "place:import".to_string();
        importable.place_id = Some("import".to_string());

        let mut outside = sample_result();
        outside.id = "result-outside".to_string();
        outside.dedup_key = "place:outside".to_string();
        outside.place_id = Some("outside".to_string());
        outside.inside_requested_area = Some(false);

        context
            .repository
            .upsert_results(
                "job-1",
                vec![enrichment.clone(), importable.clone(), outside],
            )
            .await
            .unwrap();
        assert_eq!(
            context
                .repository
                .list_results_for_enrichment("job-1")
                .await
                .unwrap()
                .iter()
                .map(|result| result.id.as_str())
                .collect::<Vec<_>>(),
            vec!["result-enrichment"]
        );
        assert_eq!(
            context
                .repository
                .list_results_for_import("job-1")
                .await
                .unwrap()
                .iter()
                .map(|result| result.id.as_str())
                .collect::<Vec<_>>(),
            vec!["result-import"]
        );

        enrichment.email = Some("hello@example.com".to_string());
        enrichment.normalized_email = Some("hello@example.com".to_string());
        enrichment.enrichment_status = "completed".to_string();
        context
            .repository
            .upsert_results("job-1", vec![enrichment])
            .await
            .unwrap();
        let result_count: i64 = context
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM google_maps_scrape_results",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(result_count, 3);
    }

    #[tokio::test]
    async fn enrichment_claim_and_outcome_persist_without_repeating_maps_scrape() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Running))
            .await
            .unwrap();
        context
            .repository
            .insert_units(
                "job-1",
                vec![sample_unit(
                    "unit-1",
                    "keyword:coffee",
                    ScrapeUnitStatus::Completed,
                )],
            )
            .await
            .unwrap();
        let mut result = sample_result();
        result.phone = None;
        result.normalized_phone = None;
        result.website = Some("https://company.test".to_string());
        result.enrichment_status = EnrichmentStatus::Pending.as_str().to_string();
        context
            .repository
            .upsert_results("job-1", vec![result.clone()])
            .await
            .unwrap();

        let claimed = context
            .repository
            .claim_next_result_for_enrichment("job-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claimed.enrichment_status, "running");
        assert_eq!(claimed.place_id, result.place_id);
        assert_eq!(claimed.website, result.website);

        let retry = context
            .repository
            .save_enrichment_outcome(
                "job-1",
                &claimed.id,
                EnrichmentOutcome::from_error(ScrapeJobError {
                    kind: ScrapeErrorKind::TransientNetwork,
                    message: "connection reset".to_string(),
                    retryable: true,
                }),
            )
            .await
            .unwrap();
        assert_eq!(retry.enrichment_status, "retry_wait");
        assert_eq!(retry.place_id, result.place_id);
        assert_eq!(retry.website, result.website);

        let retry_claim = context
            .repository
            .claim_next_result_for_enrichment("job-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retry_claim.id, claimed.id);
        let found = context
            .repository
            .save_enrichment_outcome(
                "job-1",
                &retry_claim.id,
                EnrichmentOutcome::found("sales@company.test".to_string()),
            )
            .await
            .unwrap();

        assert_eq!(found.enrichment_status, "found");
        assert_eq!(found.email.as_deref(), Some("sales@company.test"));
        assert_eq!(
            found.normalized_email.as_deref(),
            Some("sales@company.test")
        );
        assert!(context
            .repository
            .claim_next_result_for_enrichment("job-1")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn only_terminal_jobs_can_be_deleted_and_listed_jobs_remain_consistent() {
        let context = TestRepository::new();
        context
            .repository
            .create_job(sample_job(ScrapeJobStatus::Queued))
            .await
            .unwrap();
        assert_eq!(context.repository.list_jobs().await.unwrap().len(), 1);
        assert!(context
            .repository
            .delete_terminal_job("job-1")
            .await
            .is_err());

        context
            .repository
            .transition_job(
                "job-1",
                0,
                ScrapeJobStatus::Cancelled,
                TransitionCause::UserAction,
            )
            .await
            .unwrap();
        assert!(context
            .repository
            .delete_terminal_job("job-1")
            .await
            .unwrap());
        assert!(context.repository.list_jobs().await.unwrap().is_empty());
    }
}
