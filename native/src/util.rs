//! Small utilities: the kept greeting command, the license hardware id and image saving.

use crate::error::CoreApiError;
use crate::runtime::on_core;
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::commands;

/// Utility commands kept from the v2 shell.
#[module]
pub struct Util;

#[module]
impl Util {
    /// The v2 sample command: resolves `Hello, {name}! You've been greeted from Rust!`.
    pub async fn greet(name: String) -> Result<JsonValue, JsError> {
        Ok(JsonValue::String(rustsale_core::greet(&name)))
    }

    /// The license hardware id: `{ hardware_id, hostname, os_info }`, hashed from the OS, arch,
    /// hostname and user.
    pub async fn hardware_id() -> Result<JsonValue, JsError> {
        crate::runtime::on_blocking(move |_h| Ok::<_, CoreApiError>(commands::get_hardware_id()?))
            .await
    }

    /// Saves an image from `url` or `base64Data` through the save dialog using `defaultFilename`
    /// and resolves the chosen path, or `null` when cancelled. Give exactly one source.
    pub async fn save_image(
        default_filename: Option<String>,
        url: Option<String>,
        base64_data: Option<String>,
    ) -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            Ok::<_, CoreApiError>(
                commands::save_image_file(h, default_filename, url, base64_data).await?,
            )
        })
        .await
    }
}
