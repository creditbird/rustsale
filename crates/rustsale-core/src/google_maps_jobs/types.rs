use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrapeJobStatus {
    Queued,
    Running,
    Paused,
    PausedRequiresAction,
    PausedLimitReached,
    Interrupted,
    Partial,
    Completed,
    Failed,
    Cancelled,
}

impl ScrapeJobStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Partial | Self::Completed | Self::Failed | Self::Cancelled
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrapeStage {
    Planning,
    InstallingBrowserRuntime,
    Scraping,
    EmailEnrichment,
    Importing,
    Cleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrapeUnitStatus {
    Pending,
    Running,
    RetryScheduled,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrapeMode {
    Keyword,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrapeErrorKind {
    Disconnected,
    NavigationTimeout,
    EvaluationTimeout,
    TransientNetwork,
    Captcha,
    ConsentRequired,
    RateLimited,
    SelectorChanged,
    /// The place page exists but carries no place: it was withdrawn or never
    /// existed. Incidental to one listing, so it is not a sign the job is broken.
    PlaceUnavailable,
    InvalidResponse,
    BrowserStartup,
    RuntimeUnavailable,
    InvalidInput,
    Database,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionCause {
    System,
    UserAction,
    UserRetry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrapeJobError {
    pub kind: ScrapeErrorKind,
    pub message: String,
    pub retryable: bool,
}

impl std::fmt::Display for ScrapeJobError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for ScrapeJobError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrapeJob {
    pub id: String,
    pub mode: ScrapeMode,
    pub keyword: String,
    pub location: Option<String>,
    pub bbox: Option<[f64; 4]>,
    pub step_km: Option<f64>,
    pub headless: bool,
    pub profile_dir: String,
    pub blocked_url: Option<String>,
    pub status: ScrapeJobStatus,
    pub stage: ScrapeStage,
    pub total_units: u64,
    pub processed_units: u64,
    pub failed_units: u64,
    pub businesses_discovered: u64,
    pub businesses_qualified: u64,
    pub leads_created: u64,
    pub last_error_kind: Option<ScrapeErrorKind>,
    pub last_error_message: Option<String>,
    pub action_required: Option<String>,
    pub event_sequence: u64,
    pub version: u64,
    pub run_generation: u64,
    pub created_at: String,
    pub started_at: Option<String>,
    pub updated_at: String,
    pub completed_at: Option<String>,
    /// Set once a cell needed to subdivide but the configured cell budget was
    /// spent. Resuming is the operator accepting the shortfall, not the
    /// shortfall disappearing, so the job can no longer finish as `Completed`.
    #[serde(default)]
    pub coverage_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrapeUnit {
    pub id: String,
    pub job_id: String,
    pub unit_key: String,
    pub unit_type: String,
    pub payload: serde_json::Value,
    pub status: ScrapeUnitStatus,
    pub depth: u8,
    pub attempt_count: u32,
    pub next_retry_at: Option<String>,
    pub last_error_kind: Option<ScrapeErrorKind>,
    pub last_error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrapeResult {
    pub id: String,
    pub job_id: String,
    pub source_unit_id: String,
    pub place_id: Option<String>,
    pub canonical_url: Option<String>,
    pub dedup_key: String,
    pub name: String,
    pub phone: Option<String>,
    pub normalized_phone: Option<String>,
    pub email: Option<String>,
    pub normalized_email: Option<String>,
    pub address: Option<String>,
    pub website: Option<String>,
    pub facebook: Option<String>,
    pub zalo: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub inside_requested_area: Option<bool>,
    pub detail_status: String,
    pub enrichment_status: String,
    pub import_status: String,
    pub last_error_kind: Option<ScrapeErrorKind>,
    pub last_error_message: Option<String>,
    pub first_seen_at: String,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateScrapeJobRequest {
    pub mode: ScrapeMode,
    pub keyword: String,
    pub location: Option<String>,
    pub bbox: Option<[f64; 4]>,
    pub step_km: Option<f64>,
    #[serde(default)]
    pub districts: Vec<String>,
    pub headless: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrapeJobEvent {
    pub job_id: String,
    pub sequence: u64,
    pub timestamp: String,
    pub status: ScrapeJobStatus,
    pub stage: ScrapeStage,
    pub processed_units: u64,
    pub total_units: u64,
    pub failed_units: u64,
    pub businesses_discovered: u64,
    pub businesses_qualified: u64,
    pub leads_created: u64,
    pub percentage: f64,
    pub message: String,
    pub error: Option<ScrapeJobError>,
    pub action_required: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn google_maps_job_types_use_stable_snake_case_contracts() {
        assert_eq!(
            serde_json::to_string(&ScrapeJobStatus::PausedRequiresAction).unwrap(),
            "\"paused_requires_action\""
        );
        assert_eq!(
            serde_json::to_string(&ScrapeStage::InstallingBrowserRuntime).unwrap(),
            "\"installing_browser_runtime\""
        );
        assert_eq!(
            serde_json::to_string(&ScrapeUnitStatus::RetryScheduled).unwrap(),
            "\"retry_scheduled\""
        );
        assert_eq!(
            serde_json::to_string(&ScrapeErrorKind::NavigationTimeout).unwrap(),
            "\"navigation_timeout\""
        );
    }

    #[test]
    fn google_maps_job_terminal_states_are_quiescent() {
        for status in [
            ScrapeJobStatus::Partial,
            ScrapeJobStatus::Completed,
            ScrapeJobStatus::Failed,
            ScrapeJobStatus::Cancelled,
        ] {
            assert!(status.is_terminal(), "{status:?} must be terminal");
        }

        for status in [
            ScrapeJobStatus::Queued,
            ScrapeJobStatus::Running,
            ScrapeJobStatus::Paused,
            ScrapeJobStatus::PausedRequiresAction,
            ScrapeJobStatus::PausedLimitReached,
            ScrapeJobStatus::Interrupted,
        ] {
            assert!(!status.is_terminal(), "{status:?} must remain resumable");
        }
    }

    #[test]
    fn google_maps_job_request_and_event_round_trip() {
        let request = CreateScrapeJobRequest {
            mode: ScrapeMode::Grid,
            keyword: "nha khoa".to_string(),
            location: Some("Thành phố Hồ Chí Minh".to_string()),
            bbox: Some([10.3, 11.2, 106.3, 107.1]),
            step_km: Some(2.5),
            districts: vec!["Quận 1".to_string()],
            headless: true,
        };
        let request_json = serde_json::to_string(&request).unwrap();
        let decoded_request: CreateScrapeJobRequest = serde_json::from_str(&request_json).unwrap();
        assert_eq!(decoded_request, request);

        let event = ScrapeJobEvent {
            job_id: "job-1".to_string(),
            sequence: 7,
            timestamp: "2026-07-26T00:00:00Z".to_string(),
            status: ScrapeJobStatus::Running,
            stage: ScrapeStage::Scraping,
            processed_units: 3,
            total_units: 12,
            failed_units: 1,
            businesses_discovered: 12,
            businesses_qualified: 5,
            leads_created: 4,
            percentage: 25.0,
            message: "Scraping grid cell".to_string(),
            error: Some(ScrapeJobError {
                kind: ScrapeErrorKind::TransientNetwork,
                message: "connection reset".to_string(),
                retryable: true,
            }),
            action_required: None,
        };
        let event_json = serde_json::to_string(&event).unwrap();
        let event_value: serde_json::Value = serde_json::from_str(&event_json).unwrap();
        assert_eq!(event_value["failed_units"], 1);
        assert_eq!(event_value["businesses_discovered"], 12);
        assert_eq!(event_value["businesses_qualified"], 5);
        assert_eq!(event_value["leads_created"], 4);
        let decoded_event: ScrapeJobEvent = serde_json::from_str(&event_json).unwrap();
        assert_eq!(decoded_event, event);
    }
}
