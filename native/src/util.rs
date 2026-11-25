//! Small utilities: the kept greeting command, the license hardware id, build-time Supabase config
//! and image saving.

use crate::config::SupabaseConfig;
use crate::error::CoreApiError;
use crate::runtime::on_core;
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::commands;

fn supabase_config_value(cfg: Option<SupabaseConfig>) -> JsonValue {
    match cfg {
        Some(cfg) => serde_json::json!({
            "url": cfg.url,
            "anon_key": cfg.anon_key,
        }),
        None => JsonValue::Null,
    }
}

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

    /// Build-time Supabase project settings (`{ url, anon_key }`), or `null` when this build had
    /// no real `VITE_SUPABASE_URL` and `VITE_SUPABASE_ANON_KEY` (dev builds).
    pub fn supabase_config() -> Result<JsonValue, JsError> {
        Ok(supabase_config_value(crate::config::supabase()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supabase_config_value_is_null_when_unconfigured() {
        assert_eq!(supabase_config_value(None), JsonValue::Null);
    }

    #[test]
    fn supabase_config_value_serializes_url_and_anon_key() {
        let value = supabase_config_value(Some(SupabaseConfig {
            url: "https://license.example.test",
            anon_key: "anon-key",
        }));
        assert_eq!(value["url"], "https://license.example.test");
        assert_eq!(value["anon_key"], "anon-key");
    }
}
