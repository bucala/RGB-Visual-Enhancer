//! IPC commands. All are `async` so Tauri runs them on its Tokio runtime,
//! never on the main/UI thread.

use std::sync::atomic::Ordering;

use rgb_core::drivers::virtual_device::VIRTUAL_ID_PREFIX;
use rgb_core::system::SystemInfo;
use rgb_core::{DeviceSnapshot, Rgb, ScanSummary};
use serde::Serialize;
use tauri::State;

use crate::AppState;

type CmdResult<T> = Result<T, String>;

#[tauri::command]
pub async fn list_devices(state: State<'_, AppState>) -> CmdResult<Vec<DeviceSnapshot>> {
    Ok(state.manager.devices().await)
}

#[tauri::command]
pub async fn scan_devices(state: State<'_, AppState>) -> CmdResult<ScanSummary> {
    state.manager.scan().await
}

#[tauri::command]
pub fn is_scanning(state: State<'_, AppState>) -> bool {
    state.manager.is_scanning()
}

#[tauri::command]
pub async fn set_color(state: State<'_, AppState>, device_id: String, color: Rgb) -> CmdResult<()> {
    state.manager.set_color(&device_id, color).await
}

#[derive(Serialize)]
pub struct DeviceFailure {
    id: String,
    message: String,
}

/// Apply one colour to several devices in parallel ("sync all").
#[tauri::command]
pub async fn set_color_many(
    state: State<'_, AppState>,
    device_ids: Vec<String>,
    color: Rgb,
) -> CmdResult<Vec<DeviceFailure>> {
    Ok(state
        .manager
        .set_color_many(&device_ids, color)
        .await
        .into_iter()
        .map(|(id, message)| DeviceFailure { id, message })
        .collect())
}

#[tauri::command]
pub async fn set_zone_color(
    state: State<'_, AppState>,
    device_id: String,
    zone: u8,
    color: Rgb,
) -> CmdResult<()> {
    state.manager.set_zone_color(&device_id, zone, color).await
}

#[tauri::command]
pub async fn set_effect(
    state: State<'_, AppState>,
    device_id: String,
    effect_id: u8,
    speed: u8,
) -> CmdResult<()> {
    state.manager.set_effect(&device_id, effect_id, speed).await
}

#[tauri::command]
pub async fn remove_device(state: State<'_, AppState>, device_id: String) -> CmdResult<()> {
    state.manager.remove(&device_id).await
}

#[tauri::command]
pub async fn get_system_info() -> CmdResult<SystemInfo> {
    tokio::task::spawn_blocking(SystemInfo::collect)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct Settings {
    virtual_devices: bool,
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    Settings {
        virtual_devices: state.virtual_devices.load(Ordering::SeqCst),
    }
}

/// Toggle simulated devices. Enabling triggers a rescan; disabling removes them.
#[tauri::command]
pub async fn set_virtual_devices(state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    state.virtual_devices.store(enabled, Ordering::SeqCst);
    if enabled {
        state.manager.scan().await.map(|_| ())
    } else {
        state
            .manager
            .remove_where(|d| d.info.id.starts_with(VIRTUAL_ID_PREFIX))
            .await;
        Ok(())
    }
}
