//! IPC commands. All are `async` so Tauri runs them on its Tokio runtime,
//! never on the main/UI thread.

use std::sync::atomic::Ordering;

use rgb_core::drivers::virtual_device::VIRTUAL_ID_PREFIX;
use rgb_core::system::SystemInfo;
use rgb_core::{DeviceSnapshot, Rgb, ScanSummary};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_autostart::ManagerExt as _;

use crate::profiles::{self, ActiveProfile, ProfileState};
use crate::settings::{Language, Profile, SettingsStore};
use crate::{tray, AppState, LogPath, SETTINGS_CHANNEL};

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
    language: Language,
    /// Whether the tray uses the app-styled webview menu (Windows/macOS).
    custom_tray_menu: bool,
    /// Location of the log file, if logging to a file works.
    log_file: Option<String>,
}

#[tauri::command]
pub fn get_settings<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    settings: State<'_, SettingsStore>,
) -> Settings {
    Settings {
        virtual_devices: state.virtual_devices.load(Ordering::SeqCst),
        language: settings.language(),
        custom_tray_menu: tray::uses_custom_menu(),
        log_file: app
            .try_state::<LogPath>()
            .and_then(|p| p.0.as_ref().map(|p| p.display().to_string())),
    }
}

/// Persist the UI language and propagate it to every window and the tray.
#[tauri::command]
pub async fn set_language<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    settings: State<'_, SettingsStore>,
    language: Language,
) -> CmdResult<()> {
    settings.update(|s| s.language = language)?;
    let _ = app.emit(SETTINGS_CHANNEL, settings.get());
    tray::refresh(&app, &state.manager).await;
    Ok(())
}

#[tauri::command]
pub fn get_autostart<R: Runtime>(app: AppHandle<R>) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub async fn set_autostart<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    enabled: bool,
) -> CmdResult<()> {
    tray::toggle_autostart(&app, enabled)?;
    tray::refresh(&app, &state.manager).await;
    Ok(())
}

#[tauri::command]
pub fn show_main_window<R: Runtime>(app: AppHandle<R>) {
    tray::show_main_window(&app);
}

#[tauri::command]
pub fn hide_tray_menu<R: Runtime>(app: AppHandle<R>) {
    tray::hide_menu(&app);
}

/// The tray menu webview reports its content height so the window fits it.
#[tauri::command]
pub fn tray_menu_resize<R: Runtime>(app: AppHandle<R>, height: f64) {
    tray::resize_menu(&app, height);
}

#[tauri::command]
pub fn quit_app<R: Runtime>(app: AppHandle<R>) {
    app.exit(0);
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

#[tauri::command]
pub async fn set_key_colors(
    state: State<'_, AppState>,
    device_id: String,
    keys: Vec<(String, Rgb)>,
) -> CmdResult<()> {
    state.manager.set_key_colors(&device_id, keys).await
}

#[tauri::command]
pub fn set_tray_tooltip<R: Runtime>(app: AppHandle<R>, text: String) {
    tray::set_tooltip(&app, text);
}

#[derive(Serialize)]
pub struct ProfilesView {
    profiles: Vec<Profile>,
    default_profile: Option<String>,
    auto_switch: bool,
    active: ActiveProfile,
}

#[tauri::command]
pub fn get_profiles(
    settings: State<'_, SettingsStore>,
    active: State<'_, ProfileState>,
) -> ProfilesView {
    let s = settings.get();
    ProfilesView {
        profiles: s.profiles,
        default_profile: s.default_profile,
        auto_switch: s.auto_switch,
        active: active.active.lock().expect("profile state").clone(),
    }
}

/// Insert or replace (by id) a profile.
#[tauri::command]
pub fn save_profile(settings: State<'_, SettingsStore>, profile: Profile) -> CmdResult<()> {
    if profile.id.trim().is_empty() || profile.name.trim().is_empty() {
        return Err("profile needs an id and a name".into());
    }
    settings
        .update(
            |s| match s.profiles.iter_mut().find(|p| p.id == profile.id) {
                Some(existing) => *existing = profile,
                None => s.profiles.push(profile),
            },
        )
        .map(|_| ())
}

#[tauri::command]
pub fn delete_profile(settings: State<'_, SettingsStore>, id: String) -> CmdResult<()> {
    settings
        .update(|s| {
            s.profiles.retain(|p| p.id != id);
            if s.default_profile.as_deref() == Some(&id) {
                s.default_profile = None;
            }
        })
        .map(|_| ())
}

#[tauri::command]
pub async fn apply_profile<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    settings: State<'_, SettingsStore>,
    id: String,
) -> CmdResult<()> {
    let profile = settings
        .get()
        .profiles
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("unknown profile '{id}'"))?;
    profiles::apply_and_announce(&app, &state.manager, &profile, None).await;
    Ok(())
}

#[tauri::command]
pub fn set_default_profile(
    settings: State<'_, SettingsStore>,
    id: Option<String>,
) -> CmdResult<()> {
    settings.update(|s| s.default_profile = id).map(|_| ())
}

#[tauri::command]
pub fn set_auto_switch(settings: State<'_, SettingsStore>, enabled: bool) -> CmdResult<()> {
    settings.update(|s| s.auto_switch = enabled).map(|_| ())
}

/// Executable names of running processes (to pick a game for a profile).
#[tauri::command]
pub async fn running_processes() -> CmdResult<Vec<String>> {
    let mut list: Vec<String> = tokio::task::spawn_blocking(profiles::running_processes)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .collect();
    list.sort();
    Ok(list)
}

/// The known-device database (static data).
#[tauri::command]
pub fn get_device_database() -> &'static [rgb_core::database::KnownDevice] {
    rgb_core::database::DEVICES
}
