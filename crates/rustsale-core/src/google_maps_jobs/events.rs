use crate::google_maps_jobs::diagnostics::log_scrape_event;
use crate::google_maps_jobs::types::{
    ScrapeErrorKind, ScrapeJob, ScrapeJobError, ScrapeJobEvent, ScrapeJobStatus,
};
use crate::host::AppHandle;
use std::collections::HashMap;
use tokio::sync::{broadcast, Mutex};

pub struct ScrapeEventPublisher {
    sender: broadcast::Sender<ScrapeJobEvent>,
    last_sequences: Mutex<HashMap<String, u64>>,
    /// Last status logged per job, so only real status transitions reach the log.
    last_statuses: Mutex<HashMap<String, ScrapeJobStatus>>,
}

impl ScrapeEventPublisher {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(1));
        Self {
            sender,
            last_sequences: Mutex::new(HashMap::new()),
            last_statuses: Mutex::new(HashMap::new()),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ScrapeJobEvent> {
        self.sender.subscribe()
    }

    pub async fn publish_job(&self, job: &ScrapeJob, message: impl Into<String>) -> bool {
        let mut sequences = self.last_sequences.lock().await;
        let last_sequence = sequences.get(&job.id).copied().unwrap_or(0);
        if job.event_sequence <= last_sequence {
            return false;
        }
        sequences.insert(job.id.clone(), job.event_sequence);
        drop(sequences);

        let message = message.into();
        self.log_status_transition(job, &message).await;

        let percentage = if job.total_units == 0 {
            0.0
        } else {
            (job.processed_units as f64 / job.total_units as f64 * 100.0).clamp(0.0, 100.0)
        };
        let error = job.last_error_kind.map(|kind| ScrapeJobError {
            kind,
            message: job.last_error_message.clone().unwrap_or_default(),
            retryable: is_retryable(kind),
        });
        let event = ScrapeJobEvent {
            job_id: job.id.clone(),
            sequence: job.event_sequence,
            timestamp: chrono::Utc::now().to_rfc3339(),
            status: job.status,
            stage: job.stage,
            processed_units: job.processed_units,
            total_units: job.total_units,
            failed_units: job.failed_units,
            businesses_discovered: job.businesses_discovered,
            businesses_qualified: job.businesses_qualified,
            leads_created: job.leads_created,
            percentage,
            message,
            error,
            action_required: job.action_required.clone(),
        };
        match self.sender.send(event) {
            Ok(_) | Err(_) => true,
        }
    }

    /// Writes one `job_status_changed` log line whenever a job's status differs from the last
    /// one logged for it: pause, resume, cancel, failure, action required, limit reached.
    async fn log_status_transition(&self, job: &ScrapeJob, message: &str) {
        let previous = self
            .last_statuses
            .lock()
            .await
            .insert(job.id.clone(), job.status);
        if previous == Some(job.status) {
            return;
        }
        log_scrape_event(
            "job_status_changed",
            Some(&job.id),
            None,
            Some(&format!("{:?}", job.stage).to_ascii_lowercase()),
            None,
            job.last_error_kind
                .map(|kind| format!("{kind:?}").to_ascii_lowercase())
                .as_deref(),
            serde_json::json!({
                "from": previous.map(|status| format!("{status:?}").to_ascii_lowercase()),
                "to": format!("{:?}", job.status).to_ascii_lowercase(),
                "sequence": job.event_sequence,
                "run_generation": job.run_generation,
                "reason": message,
                "processed_units": job.processed_units,
                "total_units": job.total_units,
                "failed_units": job.failed_units,
            }),
        );
    }

    pub fn spawn_event_forwarder(
        self: std::sync::Arc<Self>,
        app_handle: AppHandle,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut receiver = self.subscribe();
            loop {
                match receiver.recv().await {
                    Ok(event) => {
                        if let Err(error) = app_handle.emit("google-maps-scrape-job-event", event) {
                            eprintln!("failed to emit Google Maps scrape event: {error}");
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        eprintln!("Google Maps event subscriber lagged by {skipped} events");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        })
    }
}

impl Default for ScrapeEventPublisher {
    fn default() -> Self {
        Self::new(256)
    }
}

fn is_retryable(kind: ScrapeErrorKind) -> bool {
    matches!(
        kind,
        ScrapeErrorKind::Disconnected
            | ScrapeErrorKind::NavigationTimeout
            | ScrapeErrorKind::EvaluationTimeout
            | ScrapeErrorKind::TransientNetwork
            | ScrapeErrorKind::BrowserStartup
            | ScrapeErrorKind::RuntimeUnavailable
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::google_maps_jobs::types::{ScrapeJobStatus, ScrapeMode, ScrapeStage};
    use std::io::Write;
    use std::sync::{Arc, Mutex as StdMutex};

    #[derive(Clone, Default)]
    struct LogBuffer(Arc<StdMutex<Vec<u8>>>);

    impl Write for LogBuffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if let Ok(mut buffer) = self.0.lock() {
                buffer.extend_from_slice(bytes);
            }
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn job(status: ScrapeJobStatus, sequence: u64) -> ScrapeJob {
        ScrapeJob {
            id: "job-log".to_string(),
            mode: ScrapeMode::Keyword,
            keyword: "coffee".to_string(),
            location: None,
            bbox: None,
            step_km: None,
            headless: true,
            profile_dir: "/tmp/rustsale-log-test/profile".to_string(),
            blocked_url: None,
            status,
            stage: ScrapeStage::Scraping,
            total_units: 4,
            processed_units: 1,
            failed_units: 0,
            businesses_discovered: 0,
            businesses_qualified: 0,
            leads_created: 0,
            last_error_kind: None,
            last_error_message: None,
            action_required: None,
            event_sequence: sequence,
            version: sequence,
            run_generation: 1,
            created_at: "2026-11-22T00:00:00Z".to_string(),
            started_at: None,
            updated_at: "2026-11-22T00:00:00Z".to_string(),
            completed_at: None,
            coverage_truncated: false,
        }
    }

    #[tokio::test]
    async fn every_job_status_transition_emits_a_log_event() {
        let buffer = LogBuffer::default();
        let writer = buffer.clone();
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let publisher = ScrapeEventPublisher::new(16);
        let statuses = [
            ScrapeJobStatus::Queued,
            ScrapeJobStatus::Running,
            ScrapeJobStatus::Paused,
            ScrapeJobStatus::Running,
            ScrapeJobStatus::PausedRequiresAction,
            ScrapeJobStatus::Running,
            ScrapeJobStatus::PausedLimitReached,
            ScrapeJobStatus::Interrupted,
            ScrapeJobStatus::Running,
            ScrapeJobStatus::Failed,
        ];
        for (index, status) in statuses.into_iter().enumerate() {
            publisher
                .publish_job(&job(status, index as u64 + 1), "transition")
                .await;
        }

        let output = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
        let lines: Vec<&str> = output
            .lines()
            .filter(|line| line.contains("job_status_changed"))
            .collect();
        assert_eq!(
            lines.len(),
            statuses.len(),
            "one log line per status transition, got:\n{output}"
        );
        assert!(lines[2].contains("paused"), "{}", lines[2]);
        assert!(lines[9].contains("failed"), "{}", lines[9]);
        assert!(lines.iter().all(|line| line.contains("job-log")));
    }

    #[tokio::test]
    async fn progress_events_without_a_status_change_are_not_logged_as_transitions() {
        let buffer = LogBuffer::default();
        let writer = buffer.clone();
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let publisher = ScrapeEventPublisher::new(16);
        for sequence in 1..=5 {
            publisher
                .publish_job(&job(ScrapeJobStatus::Running, sequence), "progress")
                .await;
        }

        let output = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
        let count = output
            .lines()
            .filter(|line| line.contains("job_status_changed"))
            .count();
        assert_eq!(count, 1, "{output}");
    }
}
