use crate::google_maps_jobs::types::{ScrapeErrorKind, ScrapeJob, ScrapeJobError, ScrapeJobEvent};
use std::collections::HashMap;
use tauri::Emitter;
use tokio::sync::{broadcast, Mutex};

pub struct ScrapeEventPublisher {
    sender: broadcast::Sender<ScrapeJobEvent>,
    last_sequences: Mutex<HashMap<String, u64>>,
}

impl ScrapeEventPublisher {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(1));
        Self {
            sender,
            last_sequences: Mutex::new(HashMap::new()),
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
            percentage,
            message: message.into(),
            error,
            action_required: job.action_required.clone(),
        };
        match self.sender.send(event) {
            Ok(_) | Err(_) => true,
        }
    }

    pub fn spawn_tauri_forwarder(
        self: std::sync::Arc<Self>,
        app_handle: tauri::AppHandle,
    ) -> tauri::async_runtime::JoinHandle<()> {
        tauri::async_runtime::spawn(async move {
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
