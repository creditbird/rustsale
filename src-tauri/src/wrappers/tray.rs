#![allow(
    unused_imports,
    unused_variables,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
use crate::CoreHandle;
use rustsale_core::host::{AppHandle, State};
#[allow(unused_imports)]
use rustsale_core::tray::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[tauri::command]
pub fn get_tray_state(core: tauri::State<'_, CoreHandle>) -> Result<TrayConfig, String> {
    let app = core.handle();
    let st_state = app.state::<TrayState>().map_err(|e| e.to_string())?;
    rustsale_core::tray::get_tray_state(st_state)
}

#[tauri::command]
pub fn set_close_to_tray(core: tauri::State<'_, CoreHandle>, enabled: bool) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<TrayState>().map_err(|e| e.to_string())?;
    rustsale_core::tray::set_close_to_tray(app.clone(), enabled, st_state)
}

#[tauri::command]
pub fn set_tray_dnd(core: tauri::State<'_, CoreHandle>, enabled: bool) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<TrayState>().map_err(|e| e.to_string())?;
    rustsale_core::tray::set_tray_dnd(app.clone(), enabled, st_state)
}
