//! Tauri shell: wires the `rgb-core` device manager to the webview.
//!
//! The UI thread never touches hardware. Commands are `async` and only
//! enqueue work on per-device worker tasks; progress and state changes flow
//! back to the frontend through the [`EVENT_CHANNEL`] event.

mod commands;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use rgb_core::{default_detectors, DeviceManager, ManagerConfig};
use tauri::ipc::Invoke;
use tauri::{Emitter, Manager, RunEvent, Runtime};
use tokio::sync::broadcast::error::RecvError;

/// Every backend event is emitted on this channel as a tagged JSON object.
pub const EVENT_CHANNEL: &str = "rgb://event";
/// Emitted when the UI may have missed events and should refetch state.
pub const RESYNC_CHANNEL: &str = "rgb://resync";

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
    ]
}

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let virtual_devices = Arc::new(AtomicBool::new(env_flag("RGBVE_VIRTUAL_DEVICES")));
    let manager = DeviceManager::new(
        default_detectors(virtual_devices.clone()),
        ManagerConfig::default(),
    );

    let app = tauri::Builder::default()
        .manage(AppState {
            manager: manager.clone(),
            virtual_devices,
        })
        .invoke_handler(invoke_handler())
        .setup(move |app| {
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
        assert_eq!(summary["added"], 5);
        assert_eq!(call(&w, "is_scanning", json!({})).unwrap(), json!(false));

        let devices = call(&w, "list_devices", json!({})).unwrap();
        assert_eq!(devices.as_array().unwrap().len(), 5);
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
        assert_eq!(
            call(&w, "get_settings", json!({})).unwrap(),
            json!({ "virtual_devices": true })
        );
        call(&w, "set_virtual_devices", json!({ "enabled": false })).unwrap();
        assert_eq!(call(&w, "list_devices", json!({})).unwrap(), json!([]));
        call(&w, "remove_device", json!({ "deviceId": id })).unwrap_err();
    }
}
