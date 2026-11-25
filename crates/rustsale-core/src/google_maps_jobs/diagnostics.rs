use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

pub struct DiagnosticsGuard {
    _workers: Vec<WorkerGuard>,
}

/// Target used by every scraper log event; it selects the scraper log file.
pub const SCRAPER_LOG_TARGET: &str = "google_maps_scraper";
const SCRAPER_LOG_FILE: &str = "google-maps-scraper.jsonl";
const APP_LOG_FILE: &str = "app.jsonl";
/// Daily log files kept per log; older ones are deleted on rotation.
const LOG_RETENTION_DAYS: usize = 14;

fn is_scraper_target(target: &str) -> bool {
    target == SCRAPER_LOG_TARGET
        || target
            .strip_prefix(SCRAPER_LOG_TARGET)
            .is_some_and(|rest| rest.starts_with("::"))
}

fn daily_appender(log_dir: &Path, file_name: &str) -> Result<RollingFileAppender> {
    RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(file_name)
        .max_log_files(LOG_RETENTION_DAYS)
        .build(log_dir)
        .with_context(|| {
            format!(
                "failed to open log file {file_name} in {}",
                log_dir.display()
            )
        })
}

fn json_layer<S>(
    writer: tracing_appender::non_blocking::NonBlocking,
) -> tracing_subscriber::fmt::Layer<
    S,
    tracing_subscriber::fmt::format::JsonFields,
    tracing_subscriber::fmt::format::Format<tracing_subscriber::fmt::format::Json>,
    tracing_appender::non_blocking::NonBlocking,
>
where
    S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
{
    tracing_subscriber::fmt::layer()
        .json()
        .with_ansi(false)
        .with_current_span(true)
        .with_span_list(true)
        .with_writer(writer)
}

/// Builds the subscriber that splits events by target: scraper events go to
/// `google-maps-scraper.jsonl.*`, everything else to `app.jsonl.*`.
fn build_subscriber(
    log_dir: &Path,
    env_filter: EnvFilter,
) -> Result<(impl tracing::Subscriber + Send + Sync, Vec<WorkerGuard>)> {
    std::fs::create_dir_all(log_dir)
        .with_context(|| format!("failed to create log directory {}", log_dir.display()))?;
    let (scraper_writer, scraper_worker) =
        tracing_appender::non_blocking(daily_appender(log_dir, SCRAPER_LOG_FILE)?);
    let (app_writer, app_worker) =
        tracing_appender::non_blocking(daily_appender(log_dir, APP_LOG_FILE)?);
    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(
            json_layer(scraper_writer)
                .with_filter(filter_fn(|metadata| is_scraper_target(metadata.target()))),
        )
        .with(
            json_layer(app_writer)
                .with_filter(filter_fn(|metadata| !is_scraper_target(metadata.target()))),
        );
    Ok((subscriber, vec![scraper_worker, app_worker]))
}

pub fn init_structured_logging(log_dir: &Path) -> Result<DiagnosticsGuard> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,rustsale_crm_lib=info"));
    let (subscriber, workers) = build_subscriber(log_dir, filter)?;
    subscriber
        .try_init()
        .map_err(|error| anyhow!("failed to initialize structured logging: {error}"))?;
    Ok(DiagnosticsGuard { _workers: workers })
}

/// Like [`init_structured_logging`], but a process whose host already owns the global logger (the
/// NSH runtime sets its own, in a packaged app and in its test runner) keeps that one: the core
/// then writes no log files of its own and the returned guard is empty.
pub fn init_structured_logging_or_inherit(log_dir: &Path) -> Result<DiagnosticsGuard> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,rustsale_crm_lib=info"));
    let (subscriber, workers) = build_subscriber(log_dir, filter)?;
    match subscriber.try_init() {
        Ok(()) => Ok(DiagnosticsGuard { _workers: workers }),
        Err(_) => Ok(DiagnosticsGuard {
            _workers: Vec::new(),
        }),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn read_logs(dir: &Path, prefix: &str) -> String {
        std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(prefix))
            .map(|entry| std::fs::read_to_string(entry.path()).unwrap())
            .collect()
    }

    #[test]
    fn scraper_events_go_to_scraper_file_only() {
        let dir =
            std::env::temp_dir().join(format!("rustsale-diagnostics-{}", rand::random::<u64>()));
        let (subscriber, workers) = build_subscriber(&dir, EnvFilter::new("info")).unwrap();
        tracing::subscriber::with_default(subscriber, || {
            log_scrape_event(
                "job_started",
                Some("job-1"),
                None,
                None,
                None,
                None,
                Value::Null,
            );
            tracing::info!(target: "google_maps_scraper::runner", event = "nested_target");
            tracing::error!(target: "rustsale_crm_lib::facebook", "facebook sync failed");
            tracing::info!(target: "google_maps_scrapers_lookalike", "unrelated crate");
        });
        drop(workers);

        let scraper = read_logs(&dir, SCRAPER_LOG_FILE);
        let app = read_logs(&dir, APP_LOG_FILE);
        std::fs::remove_dir_all(&dir).unwrap();

        assert!(scraper.contains("job_started"), "scraper log: {scraper}");
        assert!(scraper.contains("nested_target"), "scraper log: {scraper}");
        assert!(
            !scraper.contains("facebook sync failed"),
            "scraper log: {scraper}"
        );
        assert!(
            !scraper.contains("unrelated crate"),
            "scraper log: {scraper}"
        );
        assert!(app.contains("facebook sync failed"), "app log: {app}");
        assert!(app.contains("unrelated crate"), "app log: {app}");
        assert!(!app.contains("job_started"), "app log: {app}");
        assert!(!app.contains("nested_target"), "app log: {app}");
    }
}
