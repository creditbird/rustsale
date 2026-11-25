#![allow(
    unused_imports,
    unused_variables,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
use crate::CoreHandle;
use rustsale_core::host::{AppHandle, State};
use std::path::PathBuf;

#[tauri::command]
pub fn greet(core: tauri::State<'_, CoreHandle>, name: &str) -> String {
    let app = core.handle();
    rustsale_core::greet(name)
}
