use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

pub struct DiagnosticsGuard {
    _worker: WorkerGuard,
}

pub fn init_structured_logging(log_dir: &Path) -> Result<DiagnosticsGuard> {
    std::fs::create_dir_all(log_dir)
        .with_context(|| format!("failed to create log directory {}", log_dir.display()))?;
    let appender = tracing_appender::rolling::daily(log_dir, "google-maps-scraper.jsonl");
    let (writer, worker) = tracing_appender::non_blocking(appender);
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,rustsale_crm_lib=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .with_ansi(false)
        .with_current_span(true)
        .with_span_list(true)
        .with_writer(writer)
        .try_init()
        .map_err(|error| anyhow!("failed to initialize structured logging: {error}"))?;
    Ok(DiagnosticsGuard { _worker: worker })
}

pub fn redact_sensitive_fields(value: Value) -> Value {
    match value {
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| {
                    if sensitive_key(&key) {
                        (key, Value::String("[REDACTED]".to_string()))
                    } else {
                        (key, redact_sensitive_fields(value))
                    }
                })
                .collect(),
        ),
        Value::Array(values) => {
            Value::Array(values.into_iter().map(redact_sensitive_fields).collect())
        }
        value => value,
    }
}

pub fn log_scrape_event(
    event: &str,
    job_id: Option<&str>,
    unit_id: Option<&str>,
    stage: Option<&str>,
    attempt: Option<u32>,
    error_kind: Option<&str>,
    metadata: Value,
) {
    let metadata = redact_sensitive_fields(metadata);
    tracing::info!(
        target: "google_maps_scraper",
        event,
        job_id = job_id.unwrap_or_default(),
        unit_id = unit_id.unwrap_or_default(),
        stage = stage.unwrap_or_default(),
        attempt = attempt.unwrap_or_default(),
        error_kind = error_kind.unwrap_or_default(),
        metadata = %metadata,
    );
}

fn sensitive_key(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    normalized.contains("license")
        || normalized.contains("cookie")
        || normalized.contains("localstorage")
        || normalized.contains("sessionstorage")
        || normalized.contains("authorization")
        || normalized.contains("password")
        || normalized.ends_with("token")
}
