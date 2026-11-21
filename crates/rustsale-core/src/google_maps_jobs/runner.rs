use crate::google_maps_jobs::diagnostics::log_scrape_event;
use crate::google_maps_jobs::events::ScrapeEventPublisher;
use crate::google_maps_jobs::repository::ScrapeJobRepository;
use crate::google_maps_jobs::types::{
    ScrapeErrorKind, ScrapeJob, ScrapeJobError, ScrapeJobStatus, ScrapeResult, ScrapeUnit,
    ScrapeUnitStatus, TransitionCause,
};
use crate::scrapers::browser::{GoogleMapsPageHealth, GoogleMapsPageState};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Default)]
pub struct UnitExecution {
    pub results: Vec<ScrapeResult>,
    pub child_units: Vec<ScrapeUnit>,
    /// Set when this unit needed to subdivide but the configured cell budget is
    /// gone. The runner pauses the job rather than reporting truncated coverage
    /// as a clean completion.
    pub budget_exhausted: bool,
}

#[derive(Debug, Default)]
pub struct FinalizeOutcome {
    pub partial: bool,
}

#[async_trait]
pub trait ScrapeUnitProcessor: Send + Sync {
    async fn execute(
        &self,
        job: &ScrapeJob,
        unit: &ScrapeUnit,
    ) -> Result<UnitExecution, ScrapeJobError>;
    async fn action_health(&self, job: &ScrapeJob) -> Result<GoogleMapsPageHealth, ScrapeJobError>;
    async fn relaunch_for_action(&self, job: &ScrapeJob) -> Result<(), ScrapeJobError>;
    async fn shutdown(&self) -> Result<(), ScrapeJobError>;
    async fn finalize(&self, _job: &ScrapeJob) -> Result<FinalizeOutcome, ScrapeJobError> {
        Ok(FinalizeOutcome::default())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub failure_tolerance: FailureTolerance,
}

/// When place-level failures stop looking incidental and start looking
/// systemic.
///
/// A single count cannot tell one withdrawn listing from a page layout change,
/// so three independent signals are used. The consecutive guard is what catches
/// selector drift early: a small job whose places all fail the same way trips it
/// long before any ratio has a meaningful sample.
#[derive(Debug, Clone, Copy)]
pub struct FailureTolerance {
    /// Consecutive place failures that mean every later unit would fail too.
    pub max_consecutive_failures: u32,
    /// Finished units required before a failure ratio carries any information.
    pub min_sample: u64,
    /// Failure ratio over that sample that means the fault is systemic.
    pub max_failure_ratio: f64,
    /// Ceiling on absorbed failures regardless of ratio or sample size.
    pub max_total_failures: u64,
}

impl Default for FailureTolerance {
    fn default() -> Self {
        Self {
            max_consecutive_failures: 5,
            min_sample: 20,
            max_failure_ratio: 0.5,
            max_total_failures: 50,
        }
    }
}

impl FailureTolerance {
    /// Whether the failures seen so far are systemic rather than incidental.
    ///
    /// `completed` and `failed` count *place units only*. The job-wide counters
    /// cannot be used: a failed unit increments both the processed and the
    /// failed counter, so a job-wide ratio can never exceed one half and every
    /// threshold above that would be unreachable.
    fn is_systemic(self, consecutive_failures: u32, completed: u64, failed: u64) -> bool {
        if consecutive_failures >= self.max_consecutive_failures {
            return true;
        }
        if failed >= self.max_total_failures {
            return true;
        }
        let sample = completed.saturating_add(failed);
        sample >= self.min_sample && (failed as f64 / sample as f64) > self.max_failure_ratio
    }
}

impl RetryPolicy {
    #[cfg(test)]
    pub(crate) fn for_tests() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(1),
            failure_tolerance: FailureTolerance::default(),
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_secs(2),
            failure_tolerance: FailureTolerance::default(),
        }
    }
}

pub struct ActiveJob {
    pub job_id: String,
    pub cancellation: CancellationToken,
    pub handle: JoinHandle<()>,
}

pub struct ScrapeJobRunner {
    active: Mutex<Option<ActiveJob>>,
    repository: Arc<ScrapeJobRepository>,
    processor: Arc<dyn ScrapeUnitProcessor>,
    events: Arc<ScrapeEventPublisher>,
    retry_policy: RetryPolicy,
}

impl ScrapeJobRunner {
    pub fn new(
        repository: Arc<ScrapeJobRepository>,
        processor: Arc<dyn ScrapeUnitProcessor>,
        events: Arc<ScrapeEventPublisher>,
        retry_policy: RetryPolicy,
    ) -> Self {
        Self {
            active: Mutex::new(None),
            repository,
            processor,
            events,
            retry_policy,
        }
    }

    pub async fn start(self: &Arc<Self>, job_id: &str) -> Result<ScrapeJob> {
        let current = self
            .repository
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("scrape job {job_id} was not found"))?;
        let cause = match current.status {
            ScrapeJobStatus::Queued => TransitionCause::System,
            ScrapeJobStatus::Paused
            | ScrapeJobStatus::PausedLimitReached
            | ScrapeJobStatus::Interrupted => TransitionCause::UserAction,
            ScrapeJobStatus::PausedRequiresAction => {
                return Err(anyhow!(
                    "scrape job requires action confirmation before it can resume"
                ));
            }
            status => return Err(anyhow!("scrape job cannot start from {status:?}")),
        };
        self.start_current(current, cause).await
    }

    pub async fn confirm_action(self: &Arc<Self>, job_id: &str) -> Result<ScrapeJob> {
        let current = self
            .repository
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("scrape job {job_id} was not found"))?;
        if current.status != ScrapeJobStatus::PausedRequiresAction {
            return Err(anyhow!(
                "scrape job cannot confirm action from {:?}",
                current.status
            ));
        }

        let health = self
            .processor
            .action_health(&current)
            .await
            .map_err(|error| anyhow!(error))?;
        if !matches!(
            health.state,
            GoogleMapsPageState::Ready | GoogleMapsPageState::EmptyResults
        ) {
            return Ok(current);
        }

        self.processor
            .shutdown()
            .await
            .map_err(|error| anyhow!(error))?;
        self.start_current(current, TransitionCause::UserAction)
            .await
    }

    async fn start_current(
        self: &Arc<Self>,
        current: ScrapeJob,
        cause: TransitionCause,
    ) -> Result<ScrapeJob> {
        let mut active = self.active.lock().await;
        if let Some(active_job) = active.as_ref() {
            if !active_job.handle.is_finished() {
                return Err(anyhow!(
                    "scrape job {} is already active",
                    active_job.job_id
                ));
            }
            active.take();
        }

        let job_id = current.id.clone();
        let running = self
            .repository
            .transition_job(&job_id, current.version, ScrapeJobStatus::Running, cause)
            .await?;
        self.events
            .publish_job(&running, "Scrape job started")
            .await;
        log_scrape_event(
            "job_started",
            Some(&running.id),
            None,
            Some(&format!("{:?}", running.stage).to_ascii_lowercase()),
            None,
            None,
            json!({"mode": format!("{:?}", running.mode).to_ascii_lowercase()}),
        );

        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let task_job_id = job_id.clone();
        let runner = self.clone();
        let handle = tokio::spawn(async move {
            let result = runner.run_job(&task_job_id, task_cancellation).await;
            if let Err(error) = result {
                runner.record_runner_failure(&task_job_id, error).await;
            }
            runner.clear_active(&task_job_id).await;
        });
        *active = Some(ActiveJob {
            job_id,
            cancellation,
            handle,
        });
        Ok(running)
    }

    pub async fn cancel(&self, job_id: &str) -> Result<ScrapeJob> {
        let cancellation = {
            let active = self.active.lock().await;
            active
                .as_ref()
                .filter(|active| active.job_id == job_id)
                .map(|active| active.cancellation.clone())
        };
        if let Some(cancellation) = cancellation {
            cancellation.cancel();
        }
        let cancelled = self.repository.cancel_job(job_id).await?;
        self.events
            .publish_job(&cancelled, "Scrape job cancelled")
            .await;
        self.processor
            .shutdown()
            .await
            .map_err(|error| anyhow!(error))?;
        self.wait_for_idle(Duration::from_secs(5)).await?;
        Ok(cancelled)
    }

    pub async fn pause(&self, job_id: &str) -> Result<ScrapeJob> {
        let cancellation = {
            let active = self.active.lock().await;
            active
                .as_ref()
                .filter(|active| active.job_id == job_id)
                .map(|active| active.cancellation.clone())
        };
        let cancellation =
            cancellation.ok_or_else(|| anyhow!("scrape job {job_id} is not the active job"))?;
        cancellation.cancel();
        let paused = self.repository.pause_job(job_id).await?;
        self.events.publish_job(&paused, "Scrape job paused").await;
        self.processor
            .shutdown()
            .await
            .map_err(|error| anyhow!(error))?;
        self.wait_for_idle(Duration::from_secs(5)).await?;
        Ok(paused)
    }

    pub async fn retry_email_enrichment(self: &Arc<Self>, job_id: &str) -> Result<ScrapeJob> {
        let mut active = self.active.lock().await;
        if let Some(active_job) = active.as_ref() {
            if !active_job.handle.is_finished() {
                return Err(anyhow!(
                    "scrape job {} is already active",
                    active_job.job_id
                ));
            }
            active.take();
        }
        let running = self.repository.reopen_for_email_retry(job_id).await?;
        self.events
            .publish_job(&running, "Email enrichment retry started")
            .await;
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let task_job_id = running.id.clone();
        let runner = self.clone();
        let handle = tokio::spawn(async move {
            let result = runner.run_job(&task_job_id, task_cancellation).await;
            if let Err(error) = result {
                runner.record_runner_failure(&task_job_id, error).await;
            }
            runner.clear_active(&task_job_id).await;
        });
        *active = Some(ActiveJob {
            job_id: running.id.clone(),
            cancellation,
            handle,
        });
        Ok(running)
    }

    pub async fn recover_startup(&self) -> Result<usize> {
        self.repository.recover_interrupted_jobs().await
    }

    pub fn retry_delay(&self, attempt_count: u32) -> Duration {
        let exponent = attempt_count.saturating_sub(1).min(20);
        self.retry_policy
            .base_delay
            .saturating_mul(1u32 << exponent)
    }

    pub async fn wait_for_idle(&self, timeout: Duration) -> Result<()> {
        tokio::time::timeout(timeout, async {
            loop {
                let finished = {
                    let mut active = self.active.lock().await;
                    if active
                        .as_ref()
                        .is_some_and(|active| active.handle.is_finished())
                    {
                        active.take().map(|active| active.handle)
                    } else if active.is_none() {
                        return Ok(());
                    } else {
                        None
                    }
                };
                if let Some(handle) = finished {
                    handle.await.context("scrape job task panicked")?;
                    continue;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .map_err(|_| anyhow!("timed out waiting for scrape job runner"))?
    }

    async fn run_job(&self, job_id: &str, cancellation: CancellationToken) -> Result<()> {
        // Reset by any successful unit: scattered bad listings across a long run
        // are incidental, an unbroken streak of them is selector drift.
        let mut consecutive_failures = 0u32;
        loop {
            if cancellation.is_cancelled() {
                return Ok(());
            }
            let job = self
                .repository
                .get_job(job_id)
                .await?
                .ok_or_else(|| anyhow!("active scrape job {job_id} disappeared"))?;
            if job.status != ScrapeJobStatus::Running {
                return Ok(());
            }

            let Some(unit) = self.repository.claim_next_unit(job_id).await? else {
                if self
                    .wait_for_retry_or_pending(job_id, &cancellation)
                    .await?
                {
                    continue;
                }
                let outcome = match self.processor.finalize(&job).await {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        let failed = self.repository.fail_job(job_id, error).await?;
                        self.events
                            .publish_job(&failed, "Scrape job finalization failed")
                            .await;
                        self.processor
                            .shutdown()
                            .await
                            .map_err(|error| anyhow!(error))?;
                        return Ok(());
                    }
                };
                self.processor
                    .shutdown()
                    .await
                    .map_err(|error| anyhow!(error))?;
                if cancellation.is_cancelled() {
                    return Ok(());
                }
                let current =
                    self.repository.get_job(job_id).await?.ok_or_else(|| {
                        anyhow!("scrape job {job_id} disappeared during finalize")
                    })?;
                if outcome.partial || current.failed_units > 0 || current.coverage_truncated {
                    let partial = self
                        .repository
                        .transition_job(
                            job_id,
                            current.version,
                            ScrapeJobStatus::Partial,
                            TransitionCause::System,
                        )
                        .await?;
                    self.events
                        .publish_job(&partial, "Scrape job completed with unresolved enrichment")
                        .await;
                    let partial_error_kind = partial
                        .last_error_kind
                        .map(|kind| format!("{kind:?}").to_ascii_lowercase());
                    log_scrape_event(
                        "job_partial",
                        Some(&partial.id),
                        None,
                        Some(&format!("{:?}", partial.stage).to_ascii_lowercase()),
                        None,
                        partial_error_kind.as_deref(),
                        json!({"failed_units": partial.failed_units}),
                    );
                } else if let Some(completed) =
                    self.repository.complete_job_if_exhausted(job_id).await?
                {
                    self.events
                        .publish_job(&completed, "Scrape job completed")
                        .await;
                    log_scrape_event(
                        "job_completed",
                        Some(&completed.id),
                        None,
                        Some(&format!("{:?}", completed.stage).to_ascii_lowercase()),
                        None,
                        None,
                        json!({
                            "businesses_discovered": completed.businesses_discovered,
                            "leads_created": completed.leads_created,
                        }),
                    );
                }
                return Ok(());
            };

            log_scrape_event(
                "unit_started",
                Some(&job.id),
                Some(&unit.id),
                Some(&format!("{:?}", job.stage).to_ascii_lowercase()),
                Some(unit.attempt_count),
                None,
                json!({"unit_key": unit.unit_key.clone(), "unit_type": unit.unit_type.clone()}),
            );
            let execution = tokio::select! {
                biased;
                _ = cancellation.cancelled() => return Ok(()),
                execution = self.processor.execute(&job, &unit) => execution,
            };
            if cancellation.is_cancelled() || !self.job_is_running(job_id).await? {
                return Ok(());
            }

            match execution {
                Ok(execution) => {
                    consecutive_failures = 0;
                    let execution_budget_exhausted = execution.budget_exhausted;
                    let checkpoint = self
                        .repository
                        .checkpoint_completed_unit(
                            job_id,
                            &unit.id,
                            execution.results,
                            execution.child_units,
                        )
                        .await?;
                    self.events
                        .publish_job(&checkpoint, format!("Completed unit {}", unit.unit_key))
                        .await;
                    log_scrape_event(
                        "unit_completed",
                        Some(job_id),
                        Some(&unit.id),
                        Some(&format!("{:?}", checkpoint.stage).to_ascii_lowercase()),
                        Some(unit.attempt_count),
                        None,
                        json!({"unit_key": unit.unit_key.clone()}),
                    );
                    if execution_budget_exhausted {
                        self.repository.mark_coverage_truncated(job_id).await?;
                        let paused = self
                            .repository
                            .transition_job(
                                job_id,
                                checkpoint.version,
                                ScrapeJobStatus::PausedLimitReached,
                                TransitionCause::System,
                            )
                            .await?;
                        self.events
                            .publish_job(&paused, "Scrape job paused: grid cell budget exhausted")
                            .await;
                        log_scrape_event(
                            "job_limit_reached",
                            Some(&paused.id),
                            Some(&unit.id),
                            Some(&format!("{:?}", paused.stage).to_ascii_lowercase()),
                            None,
                            None,
                            json!({"total_units": paused.total_units}),
                        );
                        self.processor
                            .shutdown()
                            .await
                            .map_err(|error| anyhow!(error))?;
                        return Ok(());
                    }
                }
                Err(error)
                    if error.retryable && unit.attempt_count <= self.retry_policy.max_retries =>
                {
                    let error_kind = format!("{:?}", error.kind).to_ascii_lowercase();
                    log_scrape_event(
                        "unit_retry_scheduled",
                        Some(job_id),
                        Some(&unit.id),
                        Some(&format!("{:?}", job.stage).to_ascii_lowercase()),
                        Some(unit.attempt_count),
                        Some(&error_kind),
                        json!({"message": error.message.clone()}),
                    );
                    let delay = self.retry_delay(unit.attempt_count);
                    let next_retry_at = (chrono::Utc::now()
                        + chrono::Duration::from_std(delay)
                            .context("retry delay exceeded chrono range")?)
                    .to_rfc3339();
                    self.repository
                        .schedule_retry(&unit.id, error, next_retry_at)
                        .await?;
                    tokio::select! {
                        _ = cancellation.cancelled() => return Ok(()),
                        _ = tokio::time::sleep(delay) => {}
                    }
                }
                Err(error) if action_required(error.kind) => {
                    let error_kind = format!("{:?}", error.kind).to_ascii_lowercase();
                    log_scrape_event(
                        "unit_action_required",
                        Some(job_id),
                        Some(&unit.id),
                        Some(&format!("{:?}", job.stage).to_ascii_lowercase()),
                        Some(unit.attempt_count),
                        Some(&error_kind),
                        json!({"message": error.message.clone()}),
                    );
                    // The health probe needs a live browser, but a retryable
                    // error (e.g. a place-level rate limit) has already closed
                    // it. A failed probe must not turn "user action needed"
                    // into a failed job: fall back to the URL captured in the
                    // error diagnostics, and let relaunch use a default page.
                    let blocked_url = match self.processor.action_health(&job).await {
                        Ok(health) => (!health.url.trim().is_empty()).then_some(health.url),
                        Err(probe_error) => {
                            log_scrape_event(
                                "action_health_unavailable",
                                Some(job_id),
                                Some(&unit.id),
                                Some(&format!("{:?}", job.stage).to_ascii_lowercase()),
                                Some(unit.attempt_count),
                                Some(&format!("{:?}", probe_error.kind).to_ascii_lowercase()),
                                json!({"message": probe_error.message}),
                            );
                            diagnostic_url(&error.message)
                        }
                    };
                    let paused = self
                        .repository
                        .pause_for_action(
                            job_id,
                            &unit.id,
                            error,
                            "Open the browser and complete the Google verification step"
                                .to_string(),
                            blocked_url,
                        )
                        .await?;
                    self.events
                        .publish_job(&paused, "Scrape job requires user action")
                        .await;
                    // The job is already committed as paused; a browser that
                    // cannot be (re)opened now is retried by `confirm_action`,
                    // so it must not fail the job.
                    if let Err(shutdown_error) = self.processor.shutdown().await {
                        tracing::warn!(job_id, error = %shutdown_error.message, "shutdown before action relaunch failed");
                    }
                    if let Err(relaunch_error) = self.processor.relaunch_for_action(&paused).await {
                        tracing::warn!(job_id, error = %relaunch_error.message, "browser relaunch for user action failed");
                    }
                    return Ok(());
                }
                Err(error) => {
                    let retry_exhausted = error.retryable;
                    let error_kind = format!("{:?}", error.kind).to_ascii_lowercase();
                    let tolerable = tolerable_unit_failure(&unit.unit_type, error.kind);
                    log_scrape_event(
                        "unit_failed",
                        Some(job_id),
                        Some(&unit.id),
                        Some(&format!("{:?}", job.stage).to_ascii_lowercase()),
                        Some(unit.attempt_count),
                        Some(&error_kind),
                        json!({"message": error.message.clone(), "retry_exhausted": retry_exhausted}),
                    );
                    let checkpoint = self
                        .repository
                        .checkpoint_failed_unit(job_id, &unit.id, error)
                        .await?;

                    // A place is one listing among many. Losing it is recorded
                    // and the job carries on, up to the point where the failures
                    // stop looking incidental and start looking systemic.
                    consecutive_failures = consecutive_failures.saturating_add(1);
                    let (places_completed, places_failed) =
                        self.repository.count_place_unit_outcomes(job_id).await?;
                    if tolerable
                        && !self.retry_policy.failure_tolerance.is_systemic(
                            consecutive_failures,
                            places_completed,
                            places_failed,
                        )
                    {
                        self.events
                            .publish_job(
                                &checkpoint,
                                format!(
                                    "Skipped unit {} after a place-level failure",
                                    unit.unit_key
                                ),
                            )
                            .await;
                        continue;
                    }

                    let terminal_status = if retry_exhausted {
                        ScrapeJobStatus::Partial
                    } else {
                        ScrapeJobStatus::Failed
                    };
                    let terminal = self
                        .repository
                        .transition_job(
                            job_id,
                            checkpoint.version,
                            terminal_status,
                            TransitionCause::System,
                        )
                        .await?;
                    self.events
                        .publish_job(&terminal, "Scrape job stopped after unit failure")
                        .await;
                    self.processor
                        .shutdown()
                        .await
                        .map_err(|error| anyhow!(error))?;
                    return Ok(());
                }
            }
        }
    }

    async fn wait_for_retry_or_pending(
        &self,
        job_id: &str,
        cancellation: &CancellationToken,
    ) -> Result<bool> {
        let units = self.repository.list_units(job_id).await?;
        if units
            .iter()
            .any(|unit| unit.status == ScrapeUnitStatus::Pending)
        {
            tokio::task::yield_now().await;
            return Ok(true);
        }
        let mut next_retry: Option<chrono::DateTime<chrono::FixedOffset>> = None;
        for unit in units
            .iter()
            .filter(|unit| unit.status == ScrapeUnitStatus::RetryScheduled)
        {
            let value = unit
                .next_retry_at
                .as_deref()
                .ok_or_else(|| anyhow!("retry-scheduled unit {} omitted next_retry_at", unit.id))?;
            let parsed = chrono::DateTime::parse_from_rfc3339(value)
                .with_context(|| format!("invalid next_retry_at for unit {}", unit.id))?;
            next_retry = Some(next_retry.map_or(parsed, |current| current.min(parsed)));
        }
        let Some(next_retry) = next_retry else {
            return Ok(false);
        };
        let now = chrono::Utc::now();
        let delay = (next_retry.with_timezone(&chrono::Utc) - now)
            .to_std()
            .unwrap_or(Duration::ZERO);
        tokio::select! {
            _ = cancellation.cancelled() => Ok(false),
            _ = tokio::time::sleep(delay) => Ok(true),
        }
    }

    async fn job_is_running(&self, job_id: &str) -> Result<bool> {
        Ok(self
            .repository
            .get_job(job_id)
            .await?
            .is_some_and(|job| job.status == ScrapeJobStatus::Running))
    }

    async fn record_runner_failure(&self, job_id: &str, error: anyhow::Error) {
        let failure = ScrapeJobError {
            kind: ScrapeErrorKind::Database,
            message: format!("scrape runner failed: {error:#}"),
            retryable: false,
        };
        match self.repository.fail_job(job_id, failure).await {
            Ok(job) => {
                self.events.publish_job(&job, "Scrape runner failed").await;
            }
            Err(persist_error) => {
                eprintln!(
                    "failed to persist scrape runner failure for {job_id}: {persist_error:#}"
                );
            }
        }
        if let Err(shutdown_error) = self.processor.shutdown().await {
            eprintln!("failed to shut down browser after runner failure: {shutdown_error}");
        }
    }

    async fn clear_active(&self, job_id: &str) {
        let mut active = self.active.lock().await;
        if active
            .as_ref()
            .is_some_and(|active| active.job_id == job_id)
        {
            active.take();
        }
    }
}

/// Extracts the `diagnostic_url=` value the processor appends to scrape errors.
fn diagnostic_url(message: &str) -> Option<String> {
    let rest = message.split("diagnostic_url=").nth(1)?;
    let url = rest.split(';').next()?.trim();
    (!url.is_empty()).then(|| url.to_string())
}

fn action_required(kind: ScrapeErrorKind) -> bool {
    matches!(
        kind,
        ScrapeErrorKind::Captcha | ScrapeErrorKind::ConsentRequired | ScrapeErrorKind::RateLimited
    )
}

/// Whether a unit failure can be absorbed without ending the job.
///
/// Only place units qualify: they are one listing each, so a malformed or
/// withdrawn listing is incidental. A search or grid unit failing means the
/// page the whole job depends on is unreadable, and every later unit would fail
/// identically. Infrastructure faults are never incidental at any level.
fn tolerable_unit_failure(unit_type: &str, kind: ScrapeErrorKind) -> bool {
    if unit_type != "place" {
        return false;
    }
    !matches!(
        kind,
        ScrapeErrorKind::Disconnected
            | ScrapeErrorKind::BrowserStartup
            | ScrapeErrorKind::RuntimeUnavailable
            | ScrapeErrorKind::Database
            | ScrapeErrorKind::Cancelled
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::google_maps_jobs::events::ScrapeEventPublisher;
    use crate::google_maps_jobs::repository::ScrapeJobRepository;
    use crate::google_maps_jobs::types::{
        ScrapeErrorKind, ScrapeJob, ScrapeJobError, ScrapeJobStatus, ScrapeMode, ScrapeStage,
        ScrapeUnit, ScrapeUnitStatus, TransitionCause,
    };
    use crate::scrapers::browser::{GoogleMapsPageHealth, GoogleMapsPageState};
    use async_trait::async_trait;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration;
    use tokio::sync::Notify;

    struct FakeProcessor {
        outcomes: StdMutex<VecDeque<Result<UnitExecution, ScrapeJobError>>>,
        calls: StdMutex<Vec<String>>,
        started: Notify,
        gate: Option<Arc<Notify>>,
        gate_used: AtomicBool,
        shutdowns: AtomicUsize,
        shutdown_probe: StdMutex<Option<(Arc<ScrapeJobRepository>, String)>>,
        status_seen_on_shutdown: StdMutex<Option<ScrapeJobStatus>>,
        action_health: StdMutex<VecDeque<Result<GoogleMapsPageHealth, ScrapeJobError>>>,
        action_health_calls: AtomicUsize,
        executed_profiles: StdMutex<Vec<String>>,
        relaunched_profiles: StdMutex<Vec<(String, Option<String>)>>,
        relaunch_probe: StdMutex<Option<(Arc<ScrapeJobRepository>, String)>>,
        status_seen_on_relaunch: StdMutex<Option<ScrapeJobStatus>>,
    }

    impl FakeProcessor {
        fn new(outcomes: Vec<Result<UnitExecution, ScrapeJobError>>) -> Self {
            Self {
                outcomes: StdMutex::new(outcomes.into()),
                calls: StdMutex::new(Vec::new()),
                started: Notify::new(),
                gate: None,
                gate_used: AtomicBool::new(false),
                shutdowns: AtomicUsize::new(0),
                shutdown_probe: StdMutex::new(None),
                status_seen_on_shutdown: StdMutex::new(None),
                action_health: StdMutex::new(VecDeque::new()),
                action_health_calls: AtomicUsize::new(0),
                executed_profiles: StdMutex::new(Vec::new()),
                relaunched_profiles: StdMutex::new(Vec::new()),
                relaunch_probe: StdMutex::new(None),
                status_seen_on_relaunch: StdMutex::new(None),
            }
        }

        fn blocking(
            outcomes: Vec<Result<UnitExecution, ScrapeJobError>>,
            gate: Arc<Notify>,
        ) -> Self {
            Self {
                gate: Some(gate),
                ..Self::new(outcomes)
            }
        }

        fn probe_shutdown_status(&self, repository: Arc<ScrapeJobRepository>, job_id: &str) {
            *self.shutdown_probe.lock().unwrap() = Some((repository, job_id.to_string()));
        }

        fn with_action_health(
            self,
            health: Vec<Result<GoogleMapsPageHealth, ScrapeJobError>>,
        ) -> Self {
            *self.action_health.lock().unwrap() = health.into();
            self
        }

        fn probe_relaunch_status(&self, repository: Arc<ScrapeJobRepository>, job_id: &str) {
            *self.relaunch_probe.lock().unwrap() = Some((repository, job_id.to_string()));
        }

        async fn wait_started(&self) {
            self.started.notified().await;
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl ScrapeUnitProcessor for FakeProcessor {
        async fn execute(
            &self,
            job: &ScrapeJob,
            unit: &ScrapeUnit,
        ) -> Result<UnitExecution, ScrapeJobError> {
            self.calls.lock().unwrap().push(unit.unit_key.clone());
            self.executed_profiles
                .lock()
                .unwrap()
                .push(job.profile_dir.clone());
            self.started.notify_one();
            if let Some(gate) = &self.gate {
                if !self.gate_used.swap(true, Ordering::SeqCst) {
                    gate.notified().await;
                }
            }
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
            self.action_health_calls.fetch_add(1, Ordering::SeqCst);
            self.action_health
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| {
                    Ok(GoogleMapsPageHealth {
                        state: GoogleMapsPageState::Ready,
                        url: "https://www.google.com/maps".to_string(),
                        title: "Google Maps".to_string(),
                    })
                })
        }

        async fn relaunch_for_action(&self, job: &ScrapeJob) -> Result<(), ScrapeJobError> {
            self.relaunched_profiles
                .lock()
                .unwrap()
                .push((job.profile_dir.clone(), job.blocked_url.clone()));
            let probe = self.relaunch_probe.lock().unwrap().clone();
            if let Some((repository, job_id)) = probe {
                let status = repository.get_job(&job_id).await.unwrap().unwrap().status;
                *self.status_seen_on_relaunch.lock().unwrap() = Some(status);
            }
            Ok(())
        }

        async fn shutdown(&self) -> Result<(), ScrapeJobError> {
            self.shutdowns.fetch_add(1, Ordering::SeqCst);
            let probe = self.shutdown_probe.lock().unwrap().clone();
            if let Some((repository, job_id)) = probe {
                let status = repository.get_job(&job_id).await.unwrap().unwrap().status;
                *self.status_seen_on_shutdown.lock().unwrap() = Some(status);
            }
            Ok(())
        }
    }

    fn repository() -> Arc<ScrapeJobRepository> {
        let path = std::env::temp_dir().join(format!(
            "rustsale-runner-test-{}-{}.db",
            std::process::id(),
            rand::random::<u64>()
        ));
        Arc::new(ScrapeJobRepository::new(path.to_string_lossy().to_string()).unwrap())
    }

    fn job(id: &str, mode: ScrapeMode) -> ScrapeJob {
        let now = chrono::Utc::now().to_rfc3339();
        ScrapeJob {
            id: id.to_string(),
            mode,
            keyword: "cafe".to_string(),
            location: Some("District 1".to_string()),
            bbox: None,
            step_km: None,
            headless: true,
            profile_dir: format!("/tmp/rustsale-google-maps/{id}/profile"),
            blocked_url: None,
            status: ScrapeJobStatus::Queued,
            stage: ScrapeStage::Scraping,
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
            created_at: now.clone(),
            started_at: None,
            updated_at: now,
            completed_at: None,
            coverage_truncated: false,
        }
    }

    fn unit(job_id: &str, id: &str, unit_key: &str, unit_type: &str) -> ScrapeUnit {
        let now = chrono::Utc::now().to_rfc3339();
        ScrapeUnit {
            id: id.to_string(),
            job_id: job_id.to_string(),
            unit_key: unit_key.to_string(),
            unit_type: unit_type.to_string(),
            payload: serde_json::json!({"query": unit_key}),
            status: ScrapeUnitStatus::Pending,
            depth: 0,
            attempt_count: 0,
            next_retry_at: None,
            last_error_kind: None,
            last_error_message: None,
            created_at: now.clone(),
            updated_at: now,
            completed_at: None,
        }
    }

    fn success() -> Result<UnitExecution, ScrapeJobError> {
        Ok(UnitExecution::default())
    }

    fn error(kind: ScrapeErrorKind, retryable: bool) -> Result<UnitExecution, ScrapeJobError> {
        Err(error_value(kind, retryable))
    }

    fn error_value(kind: ScrapeErrorKind, retryable: bool) -> ScrapeJobError {
        ScrapeJobError {
            kind,
            message: format!("{kind:?}"),
            retryable,
        }
    }

    async fn create_job_with_units(
        repository: &ScrapeJobRepository,
        job: ScrapeJob,
        units: Vec<ScrapeUnit>,
    ) {
        let job_id = job.id.clone();
        repository.create_job(job).await.unwrap();
        repository.insert_units(&job_id, units).await.unwrap();
    }

    async fn pause_first_unit(repository: &ScrapeJobRepository, job_id: &str) {
        let queued = repository.get_job(job_id).await.unwrap().unwrap();
        repository
            .transition_job(
                job_id,
                queued.version,
                ScrapeJobStatus::Running,
                TransitionCause::System,
            )
            .await
            .unwrap();
        let claimed = repository.claim_next_unit(job_id).await.unwrap().unwrap();
        repository
            .pause_for_action(
                job_id,
                &claimed.id,
                error_value(ScrapeErrorKind::Captcha, false),
                "solve captcha".to_string(),
                Some("https://www.google.com/sorry/index".to_string()),
            )
            .await
            .unwrap();
    }

    fn runner(
        repository: Arc<ScrapeJobRepository>,
        processor: Arc<FakeProcessor>,
    ) -> Arc<ScrapeJobRunner> {
        Arc::new(ScrapeJobRunner::new(
            repository,
            processor,
            Arc::new(ScrapeEventPublisher::new(64)),
            RetryPolicy {
                max_retries: 3,
                base_delay: Duration::from_millis(2),
                failure_tolerance: FailureTolerance::default(),
            },
        ))
    }

    #[tokio::test]
    async fn only_one_job_can_run() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("job-a", ScrapeMode::Keyword),
            vec![unit("job-a", "unit-a", "a", "keyword")],
        )
        .await;
        create_job_with_units(
            &repository,
            job("job-b", ScrapeMode::Keyword),
            vec![unit("job-b", "unit-b", "b", "keyword")],
        )
        .await;
        let gate = Arc::new(Notify::new());
        let processor = Arc::new(FakeProcessor::blocking(vec![success()], gate.clone()));
        let runner = runner(repository, processor.clone());

        runner.start("job-a").await.unwrap();
        processor.wait_started().await;
        assert!(runner.start("job-b").await.is_err());
        runner.cancel("job-a").await.unwrap();
        gate.notify_waiters();
        runner.wait_for_idle(Duration::from_secs(1)).await.unwrap();
    }

    #[tokio::test]
    async fn pause_preempts_a_blocking_unit_without_reporting_timeout() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("pause-blocking", ScrapeMode::Keyword),
            vec![unit(
                "pause-blocking",
                "pause-blocking-unit",
                "blocking",
                "keyword",
            )],
        )
        .await;
        let gate = Arc::new(Notify::new());
        let processor = Arc::new(FakeProcessor::blocking(vec![success()], gate.clone()));
        let runner = runner(repository.clone(), processor.clone());

        runner.start("pause-blocking").await.unwrap();
        processor.wait_started().await;
        let paused =
            tokio::time::timeout(Duration::from_millis(100), runner.pause("pause-blocking")).await;
        if paused.is_err() {
            gate.notify_waiters();
            runner.wait_for_idle(Duration::from_secs(1)).await.unwrap();
        }
        let paused = paused
            .expect("pause should preempt the blocking unit")
            .unwrap();

        assert_eq!(paused.status, ScrapeJobStatus::Paused);
        assert_eq!(
            repository
                .get_unit("pause-blocking-unit")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeUnitStatus::Pending
        );
    }

    #[tokio::test]
    async fn cancel_releases_the_active_slot_before_returning() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("cancel-blocking", ScrapeMode::Keyword),
            vec![unit(
                "cancel-blocking",
                "cancel-blocking-unit",
                "blocking",
                "keyword",
            )],
        )
        .await;
        create_job_with_units(
            &repository,
            job("next-job", ScrapeMode::Keyword),
            vec![unit("next-job", "next-unit", "next", "keyword")],
        )
        .await;
        let gate = Arc::new(Notify::new());
        let processor = Arc::new(FakeProcessor::blocking(
            vec![success(), success()],
            gate.clone(),
        ));
        let runner = runner(repository, processor.clone());

        runner.start("cancel-blocking").await.unwrap();
        processor.wait_started().await;
        runner.cancel("cancel-blocking").await.unwrap();
        let next = runner.start("next-job").await;
        if next.is_err() {
            gate.notify_waiters();
            runner.wait_for_idle(Duration::from_secs(1)).await.unwrap();
        }
        next.expect("cancel should release the active runner before returning");
        runner.wait_for_idle(Duration::from_secs(1)).await.unwrap();
    }

    async fn fatal_error_stops_later_unit(mode: ScrapeMode, unit_type: &str) {
        let repository = repository();
        let job_id = format!("fatal-{unit_type}");
        create_job_with_units(
            &repository,
            job(&job_id, mode),
            vec![
                unit(&job_id, "first", "first", unit_type),
                unit(&job_id, "second", "second", unit_type),
            ],
        )
        .await;
        let processor = Arc::new(FakeProcessor::new(vec![error(
            ScrapeErrorKind::SelectorChanged,
            false,
        )]));
        let runner = runner(repository.clone(), processor.clone());

        runner.start(&job_id).await.unwrap();
        runner.wait_for_idle(Duration::from_secs(1)).await.unwrap();

        assert_eq!(processor.calls(), vec!["first"]);
        assert_eq!(
            repository.get_job(&job_id).await.unwrap().unwrap().status,
            ScrapeJobStatus::Failed
        );
        assert_eq!(
            repository.get_unit("second").await.unwrap().unwrap().status,
            ScrapeUnitStatus::Pending
        );
    }

    #[tokio::test]
    async fn fatal_grid_error_does_not_claim_next_cell() {
        fatal_error_stops_later_unit(ScrapeMode::Grid, "grid_cell").await;
    }

    #[tokio::test]
    async fn fatal_deep_scan_error_does_not_claim_next_district() {
        fatal_error_stops_later_unit(ScrapeMode::Keyword, "district").await;
    }

    #[tokio::test]
    async fn retryable_error_retries_three_times_with_backoff() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("retry-job", ScrapeMode::Keyword),
            vec![unit("retry-job", "retry-unit", "retry", "keyword")],
        )
        .await;
        let processor = Arc::new(FakeProcessor::new(vec![
            error(ScrapeErrorKind::TransientNetwork, true),
            error(ScrapeErrorKind::TransientNetwork, true),
            error(ScrapeErrorKind::TransientNetwork, true),
            success(),
        ]));
        let runner = runner(repository.clone(), processor.clone());

        assert_eq!(runner.retry_delay(1), Duration::from_millis(2));
        assert_eq!(runner.retry_delay(2), Duration::from_millis(4));
        assert_eq!(runner.retry_delay(3), Duration::from_millis(8));
        runner.start("retry-job").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(1)).await.unwrap();

        assert_eq!(processor.calls().len(), 4);
        assert_eq!(
            repository
                .get_unit("retry-unit")
                .await
                .unwrap()
                .unwrap()
                .attempt_count,
            4
        );
        assert_eq!(
            repository
                .get_job("retry-job")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeJobStatus::Completed
        );
    }

    /// One malformed listing must not end a job. Place units are individually
    /// scoped work, so a place-level failure is recorded and the runner moves
    /// on; the job ends `Partial` because a failure was recorded.
    #[tokio::test]
    async fn a_failed_place_unit_does_not_abort_the_remaining_units() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("tolerant-job", ScrapeMode::Keyword),
            vec![
                unit("tolerant-job", "place-a", "a", "place"),
                unit("tolerant-job", "place-b", "b", "place"),
                unit("tolerant-job", "place-c", "c", "place"),
            ],
        )
        .await;
        let processor = Arc::new(FakeProcessor::new(vec![
            success(),
            error(ScrapeErrorKind::SelectorChanged, false),
            success(),
        ]));
        let runner = runner(repository.clone(), processor.clone());

        runner.start("tolerant-job").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(2)).await.unwrap();

        assert_eq!(processor.calls(), vec!["a", "b", "c"]);
        let job = repository.get_job("tolerant-job").await.unwrap().unwrap();
        assert_eq!(job.status, ScrapeJobStatus::Partial);
        assert_eq!(job.failed_units, 1);
        assert_eq!(
            repository
                .get_unit("place-c")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeUnitStatus::Completed
        );
    }

    /// An absolute count cannot tell one bad listing from a layout change. A
    /// small job whose places all fail the same way is selector drift, and it
    /// must fail fast rather than grind through every unit and report `Partial`.
    #[tokio::test]
    async fn a_small_job_whose_places_all_fail_stops_early() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("systemic-job", ScrapeMode::Keyword),
            (0..20)
                .map(|index| {
                    let id = format!("p{index:02}");
                    unit("systemic-job", &id, &id, "place")
                })
                .collect(),
        )
        .await;
        let processor = Arc::new(FakeProcessor::new(
            (0..20)
                .map(|_| error(ScrapeErrorKind::SelectorChanged, false))
                .collect(),
        ));
        let runner = runner(repository.clone(), processor.clone());

        runner.start("systemic-job").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(5)).await.unwrap();

        let attempted = processor.calls().len();
        assert!(
            attempted <= FailureTolerance::default().max_consecutive_failures as usize,
            "a systemic fault must be caught by the consecutive guard, attempted {attempted}"
        );
        assert_eq!(
            repository
                .get_job("systemic-job")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeJobStatus::Failed
        );
    }

    /// Tolerance is bounded by consecutive failures, and the bound means what
    /// its name says: two consecutive failures stop a job configured for two.
    #[tokio::test]
    async fn consecutive_place_failures_stop_the_job() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("intolerant-job", ScrapeMode::Keyword),
            vec![
                unit("intolerant-job", "p1", "p1", "place"),
                unit("intolerant-job", "p2", "p2", "place"),
                unit("intolerant-job", "p3", "p3", "place"),
            ],
        )
        .await;
        let processor = Arc::new(FakeProcessor::new(vec![
            error(ScrapeErrorKind::SelectorChanged, false),
            error(ScrapeErrorKind::SelectorChanged, false),
            error(ScrapeErrorKind::SelectorChanged, false),
        ]));
        let runner = Arc::new(ScrapeJobRunner::new(
            repository.clone(),
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

        runner.start("intolerant-job").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(2)).await.unwrap();

        assert_eq!(processor.calls(), vec!["p1", "p2"]);
        assert_eq!(
            repository
                .get_job("intolerant-job")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeJobStatus::Failed
        );
    }

    /// The whole point of making a place its own unit: a block part-way through
    /// a listing must cost only the blocked place. This walks the full path —
    /// search unit checkpoints place children, one place completes, the next is
    /// blocked, the job pauses, and resuming claims the blocked place and only
    /// the blocked place, with the completed one still checkpointed.
    #[tokio::test]
    async fn a_block_mid_listing_only_costs_the_blocked_place() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("mid-listing", ScrapeMode::Keyword),
            vec![unit("mid-listing", "search-unit", "search", "keyword")],
        )
        .await;
        let place_children = vec![
            unit("mid-listing", "place-a", "place:0xAAA", "place"),
            unit("mid-listing", "place-b", "place:0xBBB", "place"),
            unit("mid-listing", "place-c", "place:0xCCC", "place"),
        ];
        let processor = Arc::new(
            FakeProcessor::new(vec![
                Ok(UnitExecution {
                    child_units: place_children,
                    ..UnitExecution::default()
                }),
                success(),
                error(ScrapeErrorKind::Captcha, false),
            ])
            .with_action_health(vec![
                Ok(GoogleMapsPageHealth {
                    state: GoogleMapsPageState::Captcha,
                    url: "https://www.google.com/sorry/index".to_string(),
                    title: "Google verification".to_string(),
                }),
                Ok(GoogleMapsPageHealth {
                    state: GoogleMapsPageState::Ready,
                    url: "https://www.google.com/maps".to_string(),
                    title: "Google Maps".to_string(),
                }),
            ]),
        );
        let runner = runner(repository.clone(), processor.clone());

        runner.start("mid-listing").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(5)).await.unwrap();

        assert_eq!(
            processor.calls(),
            vec!["search", "place:0xAAA", "place:0xBBB"]
        );
        let paused = repository.get_job("mid-listing").await.unwrap().unwrap();
        assert_eq!(paused.status, ScrapeJobStatus::PausedRequiresAction);
        assert_eq!(
            repository
                .get_unit("place-a")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeUnitStatus::Completed,
            "the place scraped before the block must stay checkpointed"
        );
        assert_eq!(
            repository
                .get_unit("place-b")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeUnitStatus::Pending,
            "only the blocked place returns to the queue"
        );

        runner.confirm_action("mid-listing").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(5)).await.unwrap();

        assert_eq!(
            processor.calls(),
            vec![
                "search",
                "place:0xAAA",
                "place:0xBBB",
                "place:0xBBB",
                "place:0xCCC"
            ],
            "resume re-runs the blocked place only, never the search page or a completed place"
        );
    }

    /// The failure ratio has to be reachable. Measured job-wide it never is,
    /// because a failed unit increments the processed counter too, capping any
    /// job-wide ratio at one half. This interleaves failures with successes so
    /// the consecutive guard never fires and only the ratio can stop the job.
    #[tokio::test]
    async fn a_majority_place_failure_rate_stops_the_job() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("ratio-job", ScrapeMode::Keyword),
            (0..30)
                .map(|index| {
                    let id = format!("r{index:02}");
                    unit("ratio-job", &id, &id, "place")
                })
                .collect(),
        )
        .await;
        let processor = Arc::new(FakeProcessor::new(
            (0..30)
                .map(|index| {
                    if index % 2 == 0 {
                        error(ScrapeErrorKind::SelectorChanged, false)
                    } else {
                        success()
                    }
                })
                .collect(),
        ));
        let runner = runner(repository.clone(), processor.clone());

        runner.start("ratio-job").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(10)).await.unwrap();

        // Alternating outcomes: after 21 units there are 11 failures against 10
        // successes, the first point past the minimum sample where the ratio
        // exceeds one half.
        assert_eq!(processor.calls().len(), 21);
        assert_eq!(
            repository
                .get_job("ratio-job")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeJobStatus::Failed
        );
    }

    /// An isolated failure between successes must not accumulate towards the
    /// consecutive guard, otherwise a long job dies of scattered bad listings.
    #[tokio::test]
    async fn scattered_place_failures_do_not_accumulate() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("scattered-job", ScrapeMode::Keyword),
            (0..9)
                .map(|index| {
                    let id = format!("s{index}");
                    unit("scattered-job", &id, &id, "place")
                })
                .collect(),
        )
        .await;
        let processor = Arc::new(FakeProcessor::new(vec![
            error(ScrapeErrorKind::SelectorChanged, false),
            success(),
            error(ScrapeErrorKind::SelectorChanged, false),
            success(),
            error(ScrapeErrorKind::SelectorChanged, false),
            success(),
            error(ScrapeErrorKind::SelectorChanged, false),
            success(),
            error(ScrapeErrorKind::SelectorChanged, false),
        ]));
        let runner = runner(repository.clone(), processor.clone());

        runner.start("scattered-job").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(5)).await.unwrap();

        assert_eq!(processor.calls().len(), 9);
        let job = repository.get_job("scattered-job").await.unwrap().unwrap();
        assert_eq!(job.status, ScrapeJobStatus::Partial);
        assert_eq!(job.failed_units, 5);
    }

    /// A search unit failure is still systemic: if the search page itself is
    /// unreadable, every later unit would fail the same way.
    #[tokio::test]
    async fn a_failed_search_unit_still_stops_the_job() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("search-fatal", ScrapeMode::Keyword),
            vec![
                unit("search-fatal", "s1", "s1", "keyword"),
                unit("search-fatal", "s2", "s2", "keyword"),
            ],
        )
        .await;
        let processor = Arc::new(FakeProcessor::new(vec![error(
            ScrapeErrorKind::SelectorChanged,
            false,
        )]));
        let runner = runner(repository.clone(), processor.clone());

        runner.start("search-fatal").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(2)).await.unwrap();

        assert_eq!(processor.calls(), vec!["s1"]);
        assert_eq!(
            repository
                .get_job("search-fatal")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeJobStatus::Failed
        );
    }

    /// A transient throttle must be retried with backoff before a human is
    /// asked to intervene. Only once retries are exhausted does it become an
    /// action-required pause.
    #[tokio::test]
    async fn rate_limiting_retries_before_requiring_user_action() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("throttled-job", ScrapeMode::Keyword),
            vec![unit("throttled-job", "throttled-unit", "first", "keyword")],
        )
        .await;
        let processor = Arc::new(FakeProcessor::new(vec![
            error(ScrapeErrorKind::RateLimited, true),
            error(ScrapeErrorKind::RateLimited, true),
            error(ScrapeErrorKind::RateLimited, true),
            success(),
        ]));
        let runner = runner(repository.clone(), processor.clone());

        runner.start("throttled-job").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(2)).await.unwrap();

        assert_eq!(processor.calls().len(), 4);
        assert_eq!(
            repository
                .get_job("throttled-job")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeJobStatus::Completed
        );
    }

    /// A1: a place-level rate limit closes the browser (retryable errors do),
    /// so once retries run out the health probe cannot reach a browser. That
    /// probe failure used to bubble up as a runner failure and mark the job
    /// `Failed` (kind `database`) instead of asking the user for action.
    #[tokio::test]
    async fn place_rate_limit_after_retries_pauses_for_action_instead_of_failing() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("place-limit-job", ScrapeMode::Keyword),
            vec![unit(
                "place-limit-job",
                "place-limit-unit",
                "first",
                "keyword",
            )],
        )
        .await;
        let mut blocked = error_value(ScrapeErrorKind::RateLimited, true);
        blocked.message =
            "RateLimited; diagnostic_url=https://www.google.com/sorry/index?continue=x".to_string();
        let processor = Arc::new(
            FakeProcessor::new(vec![
                Err(blocked.clone()),
                Err(blocked.clone()),
                Err(blocked.clone()),
                Err(blocked),
            ])
            .with_action_health(vec![Err(ScrapeJobError {
                kind: ScrapeErrorKind::BrowserStartup,
                message: "browser is not started".to_string(),
                retryable: true,
            })]),
        );
        let runner = runner(repository.clone(), processor.clone());

        runner.start("place-limit-job").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(2)).await.unwrap();

        let stored = repository
            .get_job("place-limit-job")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ScrapeJobStatus::PausedRequiresAction);
        assert_eq!(stored.last_error_kind, Some(ScrapeErrorKind::RateLimited));
        assert!(stored.action_required.is_some());
        assert_eq!(
            stored.blocked_url.as_deref(),
            Some("https://www.google.com/sorry/index?continue=x")
        );
        assert_eq!(processor.calls().len(), 4);
    }

    /// Spec §11.3: exhausting a safety budget pauses the job so the operator can
    /// decide, and leaves pending units intact. Silently stopping subdivision
    /// reported truncated coverage as a clean completion.
    #[tokio::test]
    async fn grid_budget_exhaustion_pauses_the_job_as_limit_reached() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("budget-job", ScrapeMode::Grid),
            vec![
                unit("budget-job", "cell-a", "a", "grid"),
                unit("budget-job", "cell-b", "b", "grid"),
            ],
        )
        .await;
        let processor = Arc::new(FakeProcessor::new(vec![Ok(UnitExecution {
            budget_exhausted: true,
            ..UnitExecution::default()
        })]));
        let runner = runner(repository.clone(), processor.clone());

        runner.start("budget-job").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(2)).await.unwrap();

        assert_eq!(processor.calls(), vec!["a"]);
        assert_eq!(
            repository
                .get_job("budget-job")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeJobStatus::PausedLimitReached
        );
        assert_eq!(
            repository.get_unit("cell-b").await.unwrap().unwrap().status,
            ScrapeUnitStatus::Pending
        );
    }

    /// Pausing on the cell budget records that coverage was cut short. Resuming
    /// is the operator accepting that, not the truncation disappearing: if the
    /// blocked cell was the last unit there is nothing left to claim, and a
    /// clean `Completed` would report full coverage of an area the job never
    /// finished subdividing.
    #[tokio::test]
    async fn resuming_after_a_budget_pause_cannot_report_clean_completion() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("truncated-job", ScrapeMode::Grid),
            vec![unit("truncated-job", "only-cell", "only", "grid")],
        )
        .await;
        let processor = Arc::new(FakeProcessor::new(vec![Ok(UnitExecution {
            budget_exhausted: true,
            ..UnitExecution::default()
        })]));
        let runner = runner(repository.clone(), processor.clone());

        runner.start("truncated-job").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(5)).await.unwrap();
        assert_eq!(
            repository
                .get_job("truncated-job")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeJobStatus::PausedLimitReached
        );

        runner.start("truncated-job").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(5)).await.unwrap();

        assert_eq!(
            repository
                .get_job("truncated-job")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeJobStatus::Partial,
            "coverage was truncated by the budget, so the job is partial"
        );
    }

    #[tokio::test]
    async fn cancel_prevents_later_completion_event() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("cancel-job", ScrapeMode::Keyword),
            vec![unit("cancel-job", "cancel-unit", "cancel", "keyword")],
        )
        .await;
        let gate = Arc::new(Notify::new());
        let processor = Arc::new(FakeProcessor::blocking(vec![success()], gate.clone()));
        let events = Arc::new(ScrapeEventPublisher::new(64));
        let mut receiver = events.subscribe();
        let runner = Arc::new(ScrapeJobRunner::new(
            repository.clone(),
            processor.clone(),
            events,
            RetryPolicy::for_tests(),
        ));

        runner.start("cancel-job").await.unwrap();
        processor.wait_started().await;
        runner.cancel("cancel-job").await.unwrap();
        gate.notify_waiters();
        runner.wait_for_idle(Duration::from_secs(1)).await.unwrap();

        assert_eq!(
            repository
                .get_job("cancel-job")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeJobStatus::Cancelled
        );
        let mut statuses = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            statuses.push(event.status);
        }
        assert!(!statuses.contains(&ScrapeJobStatus::Completed));
    }

    #[tokio::test]
    async fn captcha_pause_commits_before_shutdown_and_relaunches_same_profile() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("pause-job", ScrapeMode::Keyword),
            vec![unit("pause-job", "pause-unit", "pause", "keyword")],
        )
        .await;
        let processor = Arc::new(
            FakeProcessor::new(vec![error(ScrapeErrorKind::Captcha, false)]).with_action_health(
                vec![Ok(GoogleMapsPageHealth {
                    state: GoogleMapsPageState::Captcha,
                    url: "https://www.google.com/sorry/index".to_string(),
                    title: "Google verification".to_string(),
                })],
            ),
        );
        processor.probe_shutdown_status(repository.clone(), "pause-job");
        processor.probe_relaunch_status(repository.clone(), "pause-job");
        let runner = runner(repository.clone(), processor.clone());

        runner.start("pause-job").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(1)).await.unwrap();

        assert_eq!(
            *processor.status_seen_on_shutdown.lock().unwrap(),
            Some(ScrapeJobStatus::PausedRequiresAction)
        );
        assert_eq!(
            *processor.status_seen_on_relaunch.lock().unwrap(),
            Some(ScrapeJobStatus::PausedRequiresAction)
        );
        assert_eq!(
            processor.executed_profiles.lock().unwrap().as_slice(),
            ["/tmp/rustsale-google-maps/pause-job/profile"]
        );
        assert_eq!(
            processor.relaunched_profiles.lock().unwrap().as_slice(),
            [(
                "/tmp/rustsale-google-maps/pause-job/profile".to_string(),
                Some("https://www.google.com/sorry/index".to_string())
            )]
        );
        assert_eq!(
            repository
                .get_job("pause-job")
                .await
                .unwrap()
                .unwrap()
                .blocked_url
                .as_deref(),
            Some("https://www.google.com/sorry/index")
        );
        assert_eq!(
            repository
                .get_unit("pause-unit")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeUnitStatus::Pending
        );
    }

    #[tokio::test]
    async fn captcha_confirmation_resumes_exact_pending_unit() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("resume-job", ScrapeMode::Keyword),
            vec![
                unit("resume-job", "resume-first", "first", "district"),
                unit("resume-job", "resume-second", "second", "district"),
            ],
        )
        .await;
        let queued = repository.get_job("resume-job").await.unwrap().unwrap();
        let running = repository
            .transition_job(
                "resume-job",
                queued.version,
                ScrapeJobStatus::Running,
                TransitionCause::System,
            )
            .await
            .unwrap();
        let claimed = repository
            .claim_next_unit("resume-job")
            .await
            .unwrap()
            .unwrap();
        repository
            .pause_for_action(
                "resume-job",
                &claimed.id,
                ScrapeJobError {
                    kind: ScrapeErrorKind::Captcha,
                    message: "captcha".to_string(),
                    retryable: false,
                },
                "solve captcha".to_string(),
                Some("https://www.google.com/sorry/index".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(running.status, ScrapeJobStatus::Running);
        let processor = Arc::new(
            FakeProcessor::new(vec![success(), success()]).with_action_health(vec![Ok(
                GoogleMapsPageHealth {
                    state: GoogleMapsPageState::Ready,
                    url: "https://www.google.com/maps".to_string(),
                    title: "Google Maps".to_string(),
                },
            )]),
        );
        let runner = runner(repository, processor.clone());

        runner.confirm_action("resume-job").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(1)).await.unwrap();

        assert_eq!(processor.calls().first().map(String::as_str), Some("first"));
        assert_eq!(processor.action_health_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn captcha_failed_health_check_remains_paused() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("health-error-job", ScrapeMode::Keyword),
            vec![unit(
                "health-error-job",
                "health-error-unit",
                "first",
                "keyword",
            )],
        )
        .await;
        pause_first_unit(&repository, "health-error-job").await;
        let processor = Arc::new(
            FakeProcessor::new(vec![success()])
                .with_action_health(vec![Err(error_value(ScrapeErrorKind::Disconnected, true))]),
        );
        let runner = runner(repository.clone(), processor.clone());

        assert!(runner.confirm_action("health-error-job").await.is_err());

        assert_eq!(
            repository
                .get_job("health-error-job")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeJobStatus::PausedRequiresAction
        );
        assert!(processor.calls().is_empty());
    }

    #[tokio::test]
    async fn captcha_blocked_health_check_remains_paused() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("still-blocked-job", ScrapeMode::Keyword),
            vec![unit(
                "still-blocked-job",
                "still-blocked-unit",
                "first",
                "keyword",
            )],
        )
        .await;
        pause_first_unit(&repository, "still-blocked-job").await;
        let processor = Arc::new(
            FakeProcessor::new(vec![success()]).with_action_health(vec![Ok(
                GoogleMapsPageHealth {
                    state: GoogleMapsPageState::Captcha,
                    url: "https://www.google.com/sorry/index".to_string(),
                    title: "Google verification".to_string(),
                },
            )]),
        );
        let runner = runner(repository.clone(), processor.clone());

        assert!(runner.start("still-blocked-job").await.is_err());
        assert_eq!(processor.action_health_calls.load(Ordering::SeqCst), 0);
        let paused = runner.confirm_action("still-blocked-job").await.unwrap();

        assert_eq!(paused.status, ScrapeJobStatus::PausedRequiresAction);
        assert!(processor.calls().is_empty());
    }

    #[tokio::test]
    async fn captcha_confirmation_accepts_empty_results() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("empty-results-job", ScrapeMode::Keyword),
            vec![unit(
                "empty-results-job",
                "empty-results-unit",
                "first",
                "keyword",
            )],
        )
        .await;
        pause_first_unit(&repository, "empty-results-job").await;
        let processor = Arc::new(
            FakeProcessor::new(vec![success()]).with_action_health(vec![Ok(
                GoogleMapsPageHealth {
                    state: GoogleMapsPageState::EmptyResults,
                    url: "https://www.google.com/maps/search/no-results".to_string(),
                    title: "Google Maps".to_string(),
                },
            )]),
        );
        let runner = runner(repository.clone(), processor.clone());

        runner.confirm_action("empty-results-job").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(1)).await.unwrap();

        assert_eq!(processor.calls(), vec!["first"]);
        assert_eq!(
            repository
                .get_job("empty-results-job")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeJobStatus::Completed
        );
    }

    #[tokio::test]
    async fn captcha_cancel_while_waiting_closes_profile_and_cancels_job() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("cancel-action-job", ScrapeMode::Keyword),
            vec![unit(
                "cancel-action-job",
                "cancel-action-unit",
                "first",
                "keyword",
            )],
        )
        .await;
        let processor = Arc::new(
            FakeProcessor::new(vec![error(ScrapeErrorKind::Captcha, false)]).with_action_health(
                vec![Ok(GoogleMapsPageHealth {
                    state: GoogleMapsPageState::Captcha,
                    url: "https://www.google.com/sorry/index".to_string(),
                    title: "Google verification".to_string(),
                })],
            ),
        );
        let runner = runner(repository.clone(), processor.clone());
        runner.start("cancel-action-job").await.unwrap();
        runner.wait_for_idle(Duration::from_secs(1)).await.unwrap();

        let cancelled = runner.cancel("cancel-action-job").await.unwrap();

        assert_eq!(cancelled.status, ScrapeJobStatus::Cancelled);
        assert_eq!(processor.shutdowns.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn application_restart_marks_running_job_interrupted() {
        let repository = repository();
        create_job_with_units(
            &repository,
            job("restart-job", ScrapeMode::Keyword),
            vec![unit("restart-job", "restart-unit", "restart", "keyword")],
        )
        .await;
        let queued = repository.get_job("restart-job").await.unwrap().unwrap();
        repository
            .transition_job(
                "restart-job",
                queued.version,
                ScrapeJobStatus::Running,
                TransitionCause::System,
            )
            .await
            .unwrap();
        repository.claim_next_unit("restart-job").await.unwrap();
        let runner = runner(repository.clone(), Arc::new(FakeProcessor::new(Vec::new())));

        assert_eq!(runner.recover_startup().await.unwrap(), 1);
        assert_eq!(
            repository
                .get_job("restart-job")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeJobStatus::Interrupted
        );
        assert_eq!(
            repository
                .get_unit("restart-unit")
                .await
                .unwrap()
                .unwrap()
                .status,
            ScrapeUnitStatus::Pending
        );
    }

    #[tokio::test]
    async fn stale_event_sequence_is_never_reused() {
        let publisher = ScrapeEventPublisher::new(8);
        let mut first = job("event-job", ScrapeMode::Keyword);
        first.status = ScrapeJobStatus::Running;
        first.event_sequence = 3;
        assert!(publisher.publish_job(&first, "running").await);
        assert!(!publisher.publish_job(&first, "duplicate").await);
        let mut stale = first.clone();
        stale.event_sequence = 2;
        assert!(!publisher.publish_job(&stale, "stale").await);
        let mut next = first;
        next.event_sequence = 4;
        assert!(publisher.publish_job(&next, "progress").await);
    }
}
