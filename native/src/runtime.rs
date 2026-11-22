//! The running core: one tokio runtime and one [`Core`] per process, started
//! by `Core.start` and reached by every export through the helpers below.

use crate::error::CoreApiError;
use crate::ports::{Bridge, nsh_ports};
use nsh_core::JsonValue;
use rustsale_core::host::{AppHandle, HostError};
use rustsale_core::{Core, CoreConfig};
use serde::Serialize;
use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

/// Folder name of the v3 development data under the user's data dir.
pub(crate) const DEV_DATA_FOLDER: &str = "com.rustsale.crm.v3-dev";
/// Folder name of the live v2 app. v3 never opens it.
const LIVE_DATA_FOLDER: &str = "com.rustsale.crm";
/// Port of the v3 development local API (v2 uses 8787).
pub(crate) const DEV_LOCAL_API_PORT: u16 = 8797;
/// The variable that would turn the Zalo webhook on.
const WEBHOOK_ENV: &str = "RUSTSALE_ZALO_WEBHOOK_URL";

/// A started core with the runtime that drives it.
pub(crate) struct Running {
    runtime: tokio::runtime::Runtime,
    core: Core,
    data_dir: PathBuf,
}

impl Running {
    pub(crate) fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

static RUNNING: Mutex<Option<Arc<Running>>> = Mutex::new(None);

pub(crate) fn running() -> Result<Arc<Running>, CoreApiError> {
    RUNNING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .ok_or_else(CoreApiError::not_started)
}

/// The default data folder: the v3-dev one beside the live app's, never it.
pub(crate) fn default_data_dir() -> Result<PathBuf, CoreApiError> {
    dirs_data_dir()
        .map(|base| base.join(DEV_DATA_FOLDER))
        .ok_or_else(|| CoreApiError::new("unavailable", "this system has no user data folder"))
}

fn dirs_data_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(xdg));
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    let home = PathBuf::from(home);
    if cfg!(target_os = "macos") {
        Some(home.join("Library").join("Application Support"))
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| Some(home.join("AppData").join("Roaming")))
    } else {
        Some(home.join(".local").join("share"))
    }
}

/// Refuses the live app's folder, whatever way the path is spelled.
pub(crate) fn check_data_dir(dir: &Path) -> Result<(), CoreApiError> {
    if dir
        .components()
        .any(|c| matches!(c, Component::Normal(name) if name == LIVE_DATA_FOLDER))
    {
        return Err(CoreApiError::invalid(
            "v3 must not use the live app's data folder (com.rustsale.crm)",
        ));
    }
    if dir.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(CoreApiError::invalid(
            "the data folder must not contain `..`",
        ));
    }
    Ok(())
}

/// The Zalo webhook stays off in v3 development: a set URL stops the start.
pub(crate) fn check_webhook_off(value: Option<&str>) -> Result<(), CoreApiError> {
    match value.map(str::trim) {
        Some(url) if !url.is_empty() => Err(CoreApiError::invalid(format!(
            "{WEBHOOK_ENV} is set; the Zalo webhook must stay disabled in v3 development"
        ))),
        _ => Ok(()),
    }
}

/// Starts the runtime and the core once. A second call returns the first.
pub(crate) fn boot(
    bridge: &Bridge,
    data_dir: PathBuf,
    local_api_port: u16,
) -> Result<Arc<Running>, CoreApiError> {
    let mut slot = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(existing) = slot.as_ref() {
        return Ok(existing.clone());
    }
    check_data_dir(&data_dir)?;
    check_webhook_off(std::env::var(WEBHOOK_ENV).ok().as_deref())?;
    rustsale_core::install_rustls_crypto_provider();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_name("rustsale-core")
        .enable_all()
        .build()
        .map_err(|e| CoreApiError::internal(format!("cannot start the runtime: {e}")))?;
    let core = Core::start(
        CoreConfig {
            data_dir: data_dir.clone(),
            local_api_port: Some(local_api_port),
        },
        nsh_ports(bridge),
        runtime.handle(),
    )
    .map_err(|e| CoreApiError::new("unavailable", e.to_string()))?;
    let running = Arc::new(Running {
        runtime,
        core,
        data_dir,
    });
    *slot = Some(running.clone());
    Ok(running)
}

fn host(error: HostError) -> CoreApiError {
    CoreApiError::internal(error.to_string())
}

fn to_json<T: Serialize>(value: T) -> Result<JsonValue, CoreApiError> {
    serde_json::to_value(value)
        .map_err(|e| CoreApiError::internal(format!("cannot encode the answer: {e}")))
}

/// Runs a synchronous core command on a blocking thread, off the engine
/// thread, and gives its answer as JSON.
pub(crate) async fn on_blocking<T, E>(
    f: impl FnOnce(AppHandle) -> Result<T, E> + Send + 'static,
) -> Result<JsonValue, CoreApiError>
where
    T: Serialize + Send + 'static,
    E: Into<CoreApiError> + Send + 'static,
{
    let handle = running()?.core.handle().clone();
    let joined =
        nsh_core::prelude::blocking(move || f(handle).map_err(Into::into).and_then(to_json)).await;
    joined.map_err(|e| CoreApiError::internal(format!("background task failed: {e}")))?
}

/// Runs an async core command on the core's own runtime and gives its answer
/// as JSON.
pub(crate) async fn on_core<T, E, F>(
    f: impl FnOnce(AppHandle) -> F + Send + 'static,
) -> Result<JsonValue, CoreApiError>
where
    T: Serialize + Send + 'static,
    E: Into<CoreApiError> + Send + 'static,
    F: Future<Output = Result<T, E>> + Send + 'static,
{
    let running = running()?;
    let handle = running.core.handle().clone();
    let task = running
        .runtime
        .spawn(async move { f(handle).await.map_err(Into::into).and_then(to_json) });
    task.await
        .map_err(|e| CoreApiError::internal(format!("core task failed: {e}")))?
}

/// An optional JSON argument as `T`; absent or `null` is `T::default()`.
pub(crate) fn args<T: serde::de::DeserializeOwned + Default>(
    value: Option<JsonValue>,
) -> Result<T, CoreApiError> {
    match value {
        None | Some(JsonValue::Null) => Ok(T::default()),
        Some(value) => body(value),
    }
}

/// A required JSON argument as `T`.
pub(crate) fn body<T: serde::de::DeserializeOwned>(value: JsonValue) -> Result<T, CoreApiError> {
    serde_json::from_value(value)
        .map_err(|e| CoreApiError::invalid(format!("invalid argument: {e}")))
}

/// A page size from JS: absent is 0 (the core then picks its default).
pub(crate) fn limit(value: Option<f64>) -> u32 {
    match value {
        Some(n) if n.is_finite() && n > 0.0 => n.min(f64::from(u32::MAX)) as u32,
        _ => 0,
    }
}

/// The state of type `S` of the running core.
pub(crate) fn state_of<S: Send + Sync + 'static>(
    handle: &AppHandle,
) -> Result<rustsale_core::host::State<'_, S>, CoreApiError> {
    handle.state::<S>().map_err(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_folder_is_refused_in_any_spelling() {
        for path in [
            "/home/u/.local/share/com.rustsale.crm",
            "/home/u/.local/share/com.rustsale.crm/",
            "/home/u/.local/share/com.rustsale.crm/rustsale.db",
            "com.rustsale.crm",
        ] {
            assert!(check_data_dir(Path::new(path)).is_err(), "{path}");
        }
    }

    #[test]
    fn dev_folder_is_allowed() {
        assert!(check_data_dir(Path::new("/home/u/.local/share/com.rustsale.crm.v3-dev")).is_ok());
        assert!(check_data_dir(Path::new("/tmp/rustsale-test")).is_ok());
    }

    #[test]
    fn parent_dir_components_are_refused() {
        assert!(check_data_dir(Path::new("/tmp/x/../com.rustsale.crm")).is_err());
        assert!(check_data_dir(Path::new("/tmp/x/../y")).is_err());
    }

    #[test]
    fn webhook_url_blocks_the_start() {
        assert!(check_webhook_off(None).is_ok());
        assert!(check_webhook_off(Some("   ")).is_ok());
        assert!(check_webhook_off(Some("https://hook.example/x")).is_err());
    }

    #[test]
    fn default_dir_is_the_dev_one() {
        if let Ok(dir) = default_data_dir() {
            assert!(dir.ends_with(DEV_DATA_FOLDER));
            assert!(check_data_dir(&dir).is_ok());
        }
    }
}
