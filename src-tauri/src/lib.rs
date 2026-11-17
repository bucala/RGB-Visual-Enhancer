//! Tauri shell: wires the `rgb-core` device manager to the webview.
//!
//! The UI thread never touches hardware. Commands are `async` and only
//! enqueue work on per-device worker tasks; progress and state changes flow
//! back to the frontend through the [`EVENT_CHANNEL`] event.

mod commands;
mod logging;
mod profiles;
mod settings;
mod tray;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use rgb_core::{default_detectors, DeviceManager, ManagerConfig};
use std::sync::atomic::Ordering;
use tauri::ipc::Invoke;

use tauri::{Emitter, Manager, RunEvent, Runtime, WindowEvent};
use tokio::sync::broadcast::error::RecvError;

/// Every backend event is emitted on this channel as a tagged JSON object.
pub const EVENT_CHANNEL: &str = "rgb://event";
/// Emitted when the UI may have missed events and should refetch state.
pub const RESYNC_CHANNEL: &str = "rgb://resync";
/// Emitted to every window when persisted settings change (e.g. language).
pub const SETTINGS_CHANNEL: &str = "rgb://settings";

/// Where the log file lives (shown in Settings).
pub struct LogPath(pub Option<std::path::PathBuf>);

pub struct AppState {
    pub manager: DeviceManager,
    pub virtual_devices: Arc<AtomicBool>,
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| matches!(v.as_str(), "1" | "true" | "yes"))
}

/// The IPC surface exposed to the frontend (see `src/lib/backend.ts`).
pub fn invoke_handler<R: Runtime>() -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::list_devices,
        commands::scan_devices,
        commands::is_scanning,
        commands::set_color,
        commands::set_color_many,
        commands::set_zone_color,
        commands::set_effect,
        commands::remove_device,
        commands::get_system_info,
        commands::get_settings,
        commands::set_virtual_devices,
        commands::set_language,
        commands::get_autostart,
        commands::set_autostart,
        commands::show_main_window,
        commands::hide_tray_menu,
        commands::tray_menu_resize,
        commands::quit_app,
        commands::set_key_colors,
        commands::set_tray_tooltip,
        commands::get_profiles,
        commands::save_profile,
        commands::delete_profile,
        commands::apply_profile,
        commands::set_default_profile,
        commands::set_auto_switch,
        commands::running_processes,
        commands::get_device_database,
    ]
}

/// The autostart entry stores the path of the executable that enabled it.
/// An entry left behind by an older or development build (e.g.
/// `target\debug\…exe`, which had a console window) would keep starting that
/// binary at login, so installed builds point an enabled entry back at
/// themselves on every start.
fn refresh_autostart<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    use tauri_plugin_autostart::ManagerExt as _;
    if cfg!(debug_assertions) {
        return;
    }
    let launcher = app.autolaunch();
    if launcher.is_enabled().unwrap_or(false) {
        match launcher.enable() {
            Ok(()) => log::info!("autostart entry refreshed for this executable"),
            Err(e) => log::warn!("could not refresh autostart entry: {e}"),
        }
    }
}

pub fn run() {
    let virtual_devices = Arc::new(AtomicBool::new(env_flag("RGBVE_VIRTUAL_DEVICES")));
    let manager = DeviceManager::new(
        default_detectors(virtual_devices.clone()),
        ManagerConfig::default(),
    );

    let app = tauri::Builder::default()
        // Must be first: a second launch just brings the running app forward
        // instead of fighting it for the hardware.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![tray::MINIMIZED_ARG]),
        ))
        .manage(AppState {
            manager: manager.clone(),
            virtual_devices,
        })
        .manage(tray::TrayState::default())
        .manage(profiles::ProfileState::default())
        .on_window_event(|window, event| {
            // With a tray icon, closing or minimising hides to the tray (like
            // G HUB). Without one (e.g. no AppIndicator on Linux) the window
            // behaves normally so it can't get lost.
            let in_tray = window
                .state::<tray::TrayState>()
                .available
                .load(Ordering::SeqCst);
            if window.label() == tray::MENU_WINDOW {
                if let WindowEvent::Focused(false) = event {
                    tray::hide_menu(window.app_handle());
                }
                return;
            }
            if !in_tray || window.label() != "main" {
                return;
            }
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                WindowEvent::Resized(_) if window.is_minimized().unwrap_or(false) => {
                    let _ = window.hide();
                }
                _ => {}
            }
        })
        .invoke_handler(invoke_handler())
        .setup(move |app| {
            let log_path = logging::init(app.path().app_log_dir().ok());
            log::info!(
                "RGB Visual Enhancer {} starting; log file: {}",
                app.package_info().version,
                log_path
                    .as_deref()
                    .map_or("unavailable".into(), |p| p.display().to_string())
            );
            app.manage(LogPath(log_path));
            if let Ok(exe) = std::env::current_exe() {
                log::info!("executable: {}", exe.display());
            }
            refresh_autostart(app.handle());
            let settings = match app.path().app_config_dir() {
                Ok(dir) => settings::SettingsStore::load(dir.join("settings.json")),
                Err(e) => {
                    log::warn!("no config directory, settings won't persist: {e}");
                    settings::SettingsStore::in_memory()
                }
            };
            app.manage(settings);

            // Forward manager events to the webview.
            let handle = app.handle().clone();
            let mut events = manager.subscribe();
            tauri::async_runtime::spawn(async move {
                loop {
                    match events.recv().await {
                        Ok(event) => {
                            if let Err(e) = handle.emit(EVENT_CHANNEL, &event) {
                                log::warn!("failed to emit event: {e}");
                            }
                        }
                        Err(RecvError::Lagged(n)) => {
                            log::warn!("UI event stream lagged by {n} events; requesting resync");
                            let _ = handle.emit(RESYNC_CHANNEL, ());
                        }
                        Err(RecvError::Closed) => break,
                    }
                }
            });

            match tray::setup(app.handle(), manager.clone()) {
                Ok(()) => {
                    if std::env::args().any(|a| a == tray::MINIMIZED_ARG) {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.hide();
                        }
                    }
                }
                Err(e) => log::warn!("system tray unavailable, closing will quit: {e}"),
            }

            profiles::spawn_watcher(app.handle().clone(), manager.clone());

            // Initial hardware scan, off the UI thread.
            let scanner = manager.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = scanner.scan().await {
                    log::warn!("initial scan failed: {e}");
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            let manager = app.state::<AppState>().manager.clone();
            tauri::async_runtime::block_on(manager.shutdown_all());
        }
    });
}

#[cfg(test)]
mod ipc_tests {
    //! Locks the IPC contract used by `src/lib/backend.ts`: command names and
    //! camelCase argument names. Runs against Tauri's mock runtime.
    use super::*;
    use serde_json::{json, Value};
    use std::sync::atomic::AtomicBool;
    use tauri::test::{get_ipc_response, mock_builder, INVOKE_KEY};
    use tauri::webview::InvokeRequest;
    use tauri::WebviewWindowBuilder;

    fn app() -> (
        tauri::App<tauri::test::MockRuntime>,
        tauri::WebviewWindow<tauri::test::MockRuntime>,
    ) {
        let virtual_devices = Arc::new(AtomicBool::new(true));
        // Only the virtual detector: tests must not touch real hardware.
        let detectors: Vec<Arc<dyn rgb_core::DeviceDetector>> = vec![Arc::new(
            rgb_core::drivers::virtual_device::VirtualDetector::new(virtual_devices.clone()),
        )];
        let app = mock_builder()
            .manage(AppState {
                manager: DeviceManager::new(detectors, ManagerConfig::default()),
                virtual_devices,
            })
            .manage(settings::SettingsStore::in_memory())
            .manage(tray::TrayState::default())
            .manage(profiles::ProfileState::default())
            .plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ))
            .invoke_handler(invoke_handler())
            .build(tauri::generate_context!())
            .expect("mock app");
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("mock webview");
        (app, webview)
    }

    fn call(
        webview: &tauri::WebviewWindow<tauri::test::MockRuntime>,
        cmd: &str,
        args: Value,
    ) -> Result<Value, Value> {
        get_ipc_response(
            webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: args.into(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|b| b.deserialize::<Value>().unwrap())
    }

    #[test]
    fn frontend_contract() {
        let (_app, w) = app();

        let summary = call(&w, "scan_devices", json!({})).unwrap();
        assert_eq!(summary["added"], 6);
        assert_eq!(call(&w, "is_scanning", json!({})).unwrap(), json!(false));

        let devices = call(&w, "list_devices", json!({})).unwrap();
        assert_eq!(devices.as_array().unwrap().len(), 6);
        let id = "virtual:g-keyboard";

        let red = json!({ "r": 255, "g": 0, "b": 0 });
        call(&w, "set_color", json!({ "deviceId": id, "color": red })).unwrap();
        call(
            &w,
            "set_effect",
            json!({ "deviceId": id, "effectId": 2, "speed": 70 }),
        )
        .unwrap();
        call(
            &w,
            "set_zone_color",
            json!({ "deviceId": id, "zone": 1, "color": red }),
        )
        .unwrap();
        let failures = call(
            &w,
            "set_color_many",
            json!({ "deviceIds": [id, "missing"], "color": red }),
        )
        .unwrap();
        assert_eq!(failures.as_array().unwrap().len(), 1);
        assert_eq!(failures[0]["id"], "missing");

        let kb = call(&w, "list_devices", json!({})).unwrap();
        let kb = kb
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["id"] == id)
            .unwrap()
            .clone();
        assert_eq!(kb["color"], red);
        assert_eq!(kb["effect"], 2);
        assert_eq!(kb["speed"], 70);

        // snake_case argument names are rejected: the frontend must send camelCase.
        assert!(call(&w, "set_color", json!({ "device_id": id, "color": red })).is_err());
        // Driver errors surface as rejected promises with a message.
        let err = call(
            &w,
            "set_effect",
            json!({ "deviceId": "virtual:g-mouse", "effectId": 4, "speed": 1 }),
        )
        .unwrap_err();
        assert!(err.as_str().unwrap().contains("does not support"));

        assert!(call(&w, "get_system_info", json!({})).unwrap()["os"].is_string());
        let settings = call(&w, "get_settings", json!({})).unwrap();
        assert_eq!(settings["virtual_devices"], true);
        assert_eq!(settings["language"], "sk", "Slovak is the default");
        call(&w, "set_language", json!({ "language": "en" })).unwrap();
        assert_eq!(
            call(&w, "get_settings", json!({})).unwrap()["language"],
            "en"
        );
        assert!(call(&w, "set_language", json!({ "language": "xx" })).is_err());
        call(&w, "set_language", json!({ "language": "zh-Hant" })).unwrap();
        assert_eq!(
            call(&w, "get_settings", json!({})).unwrap()["language"],
            "zh-Hant"
        );

        // Per-key colours and profiles.
        let kb = "virtual:g-keyboard";
        call(
            &w,
            "set_key_colors",
            json!({ "deviceId": kb, "keys": [["k:1a", red], ["g:1", red]] }),
        )
        .unwrap();
        let profile = json!({ "id": "fps", "name": "FPS", "games": ["cs2.exe"], "color": null, "keys": { "k:04": red } });
        call(&w, "save_profile", json!({ "profile": profile })).unwrap();
        call(&w, "set_default_profile", json!({ "id": "fps" })).unwrap();
        call(&w, "apply_profile", json!({ "id": "fps" })).unwrap();
        let view = call(&w, "get_profiles", json!({})).unwrap();
        assert_eq!(view["profiles"][0]["games"][0], "cs2.exe");
        assert_eq!(view["default_profile"], "fps");
        assert_eq!(view["active"]["id"], "fps");
        assert!(call(&w, "running_processes", json!({}))
            .unwrap()
            .as_array()
            .is_some());
        call(&w, "delete_profile", json!({ "id": "fps" })).unwrap();
        assert_eq!(
            call(&w, "get_profiles", json!({})).unwrap()["default_profile"],
            json!(null)
        );
        call(&w, "set_tray_tooltip", json!({ "text": "RGB" })).unwrap();
        let db = call(&w, "get_device_database", json!({})).unwrap();
        assert!(db
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["ids"] == "046D:C332" && d["support"] == "verified"));
        call(&w, "tray_menu_resize", json!({ "height": 300.0 })).unwrap();
        call(&w, "hide_tray_menu", json!({})).unwrap();
        call(&w, "set_virtual_devices", json!({ "enabled": false })).unwrap();
        assert_eq!(call(&w, "list_devices", json!({})).unwrap(), json!([]));
        call(&w, "remove_device", json!({ "deviceId": id })).unwrap_err();
    }
}
