//! System tray: keeps the app running in the background (like G HUB) and
//! shows every device's state with quick colour actions.
//!
//! The menu is rebuilt from the device manager's events, debounced so a
//! colour-picker drag doesn't rebuild it hundreds of times.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use rgb_core::{DeviceManager, DeviceSnapshot, DeviceStatus, EffectId, Rgb, Transport};
use tauri::image::Image;
use tauri::menu::{
    CheckMenuItemBuilder, IconMenuItemBuilder, Menu, MenuBuilder, MenuEvent, MenuItemBuilder,
    Submenu, SubmenuBuilder,
};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_autostart::ManagerExt as _;
use tokio::sync::broadcast::error::RecvError;

use crate::AppState;

pub const TRAY_ID: &str = "main";
/// Command-line flag used by autostart: start hidden in the tray.
pub const MINIMIZED_ARG: &str = "--minimized";

const PRESETS: [(&str, Rgb); 8] = [
    ("Red", Rgb::new(255, 0, 0)),
    ("Orange", Rgb::new(255, 96, 0)),
    ("Yellow", Rgb::new(255, 200, 0)),
    ("Green", Rgb::new(0, 255, 64)),
    ("Cyan", Rgb::new(0, 220, 255)),
    ("Blue", Rgb::new(0, 64, 255)),
    ("Purple", Rgb::new(140, 0, 255)),
    ("White", Rgb::new(255, 255, 255)),
];

/// Tray-related state shared with the window event handler.
#[derive(Default)]
pub struct TrayState {
    /// True once the tray icon exists; only then may windows hide into it.
    pub available: AtomicBool,
    /// Device ids in menu order; menu item ids refer to them by index.
    device_ids: Mutex<Vec<String>>,
}

pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn toggle_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false) {
            let _ = w.hide();
        } else {
            show_main_window(app);
        }
    }
}

/// 16×16 colour swatch with a subtle border, used as a menu icon.
fn swatch(color: Option<Rgb>) -> Image<'static> {
    const N: usize = 16;
    let fill = color.map_or([58, 65, 80, 255], |c| [c.r, c.g, c.b, 255]);
    let mut px = Vec::with_capacity(N * N * 4);
    for y in 0..N {
        for x in 0..N {
            let corner = (x == 0 || x == N - 1) && (y == 0 || y == N - 1);
            let edge = x == 0 || y == 0 || x == N - 1 || y == N - 1;
            px.extend_from_slice(if corner {
                &[0, 0, 0, 0]
            } else if edge {
                &[30, 34, 44, 255]
            } else {
                &fill
            });
        }
    }
    Image::new_owned(px, N as u32, N as u32)
}

fn connection(d: &DeviceSnapshot) -> &'static str {
    match (d.info.transport, d.wireless) {
        (Transport::Ble, _) => "Bluetooth",
        (Transport::Usb, true) => "Wireless (USB receiver)",
        (Transport::Usb, false) => "USB",
        (Transport::Smbus, _) => "SMBus",
        (Transport::I2c, _) => "GPU I²C",
        (Transport::Virtual, true) => "Simulated (wireless)",
        (Transport::Virtual, false) => "Simulated",
    }
}

fn battery_text(d: &DeviceSnapshot) -> Option<String> {
    d.battery.map(|b| {
        let approx = if b.approximate { "≈" } else { "" };
        let charging = if b.charging { ", charging" } else { "" };
        format!("{approx}{} %{charging}", b.percent)
    })
}

fn lighting_text(d: &DeviceSnapshot) -> String {
    let effect = d
        .effect
        .and_then(|id| d.info.effects.iter().find(|e| e.id == id))
        .map(|e| e.name.as_str());
    match (effect, d.color) {
        (Some("Off"), _) => "Off".into(),
        (Some(e), Some(c)) if d.info.effects.iter().any(|x| x.name == e && x.uses_color) => {
            format!("{e} · {c}")
        }
        (Some(e), _) => e.into(),
        (None, Some(c)) => c.to_string(),
        (None, None) => "Not set by this app yet".into(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}

fn info_item<R: Runtime>(
    app: &AppHandle<R>,
    text: String,
) -> tauri::Result<tauri::menu::MenuItem<R>> {
    MenuItemBuilder::new(text).enabled(false).build(app)
}

fn preset_items<'a, R: Runtime>(
    app: &'a AppHandle<R>,
    prefix: &str,
    builder: SubmenuBuilder<'a, R, AppHandle<R>>,
    with_color: bool,
    with_off: bool,
) -> tauri::Result<SubmenuBuilder<'a, R, AppHandle<R>>> {
    let mut b = builder;
    if with_color {
        for (n, (name, c)) in PRESETS.iter().enumerate() {
            let item = IconMenuItemBuilder::with_id(format!("{prefix}:p{n}"), *name)
                .icon(swatch(Some(*c)))
                .build(app)?;
            b = b.item(&item);
        }
    }
    if with_off {
        b = b.item(&MenuItemBuilder::with_id(format!("{prefix}:off"), "Lights off").build(app)?);
    }
    Ok(b)
}

fn device_submenu<R: Runtime>(
    app: &AppHandle<R>,
    i: usize,
    d: &DeviceSnapshot,
) -> tauri::Result<Submenu<R>> {
    let mut title = truncate(&d.info.name, 40);
    if let Some(b) = d.battery {
        title = format!("{title}   {} %", b.percent);
    }
    if matches!(d.status, DeviceStatus::Error(_)) {
        title = format!("{title}   ⚠");
    }
    let status = match &d.status {
        DeviceStatus::Ready => "Ready".to_string(),
        DeviceStatus::Error(e) => format!("Error: {}", truncate(e, 60)),
    };

    let mut b = SubmenuBuilder::with_id(app, format!("d{i}"), title)
        .submenu_icon(swatch(
            d.color.filter(|_| d.effect != Some(EffectId::Off as u8)),
        ))
        .item(&info_item(app, format!("Status: {status}"))?)
        .item(&info_item(app, format!("Connection: {}", connection(d)))?);
    if let Some(bt) = battery_text(d) {
        b = b.item(&info_item(app, format!("Battery: {bt}"))?);
    }
    b = b
        .item(&info_item(app, format!("Lighting: {}", lighting_text(d)))?)
        .separator();
    let with_color = d.info.effects.iter().any(|e| e.uses_color);
    let with_off = d.info.effects.iter().any(|e| e.id == EffectId::Off as u8);
    preset_items(app, &format!("d{i}"), b, with_color, with_off)?.build()
}

fn build_menu<R: Runtime>(
    app: &AppHandle<R>,
    devices: &[DeviceSnapshot],
) -> tauri::Result<Menu<R>> {
    let mut menu = MenuBuilder::new(app)
        .item(&info_item(app, "RGB Visual Enhancer".into())?)
        .separator();
    if devices.is_empty() {
        menu = menu.item(&info_item(app, "No devices found".into())?);
    }
    for (i, d) in devices.iter().enumerate() {
        menu = menu.item(&device_submenu(app, i, d)?);
    }
    let all = preset_items(
        app,
        "all",
        SubmenuBuilder::new(app, "All devices"),
        true,
        true,
    )?
    .build()?;
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    menu.separator()
        .item(&all)
        .separator()
        .item(&MenuItemBuilder::with_id("open", "Open RGB Visual Enhancer").build(app)?)
        .item(&MenuItemBuilder::with_id("rescan", "Rescan hardware").build(app)?)
        .item(
            &CheckMenuItemBuilder::with_id("autostart", "Start with system (in tray)")
                .checked(autostart)
                .build(app)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id("quit", "Quit RGB Visual Enhancer").build(app)?)
        .build()
}

fn tooltip(devices: &[DeviceSnapshot]) -> String {
    let mut t = format!("RGB Visual Enhancer · {} device(s)", devices.len());
    for d in devices.iter().filter(|d| d.battery.is_some()) {
        let b = d.battery.expect("filtered");
        t.push_str(&format!("\n{} {} %", truncate(&d.info.name, 28), b.percent));
    }
    // Windows truncates tray tooltips at 127 characters.
    truncate(&t, 127)
}

async fn refresh<R: Runtime>(app: &AppHandle<R>, manager: &DeviceManager) {
    let devices = manager.devices().await;
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    match build_menu(app, &devices) {
        Ok(menu) => {
            if let Some(state) = app.try_state::<TrayState>() {
                *state.device_ids.lock().expect("tray ids") =
                    devices.iter().map(|d| d.info.id.clone()).collect();
            }
            let _ = tray.set_menu(Some(menu));
        }
        Err(e) => log::warn!("failed to build tray menu: {e}"),
    }
    let _ = tray.set_tooltip(Some(tooltip(&devices)));
}

fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    let id = event.id().as_ref().to_string();
    let manager = app.state::<AppState>().manager.clone();
    match id.as_str() {
        "open" => show_main_window(app),
        "quit" => app.exit(0),
        "rescan" => {
            tauri::async_runtime::spawn(async move {
                let _ = manager.scan().await;
            });
        }
        "autostart" => {
            let al = app.autolaunch();
            let result = if al.is_enabled().unwrap_or(false) {
                al.disable()
            } else {
                al.enable()
            };
            if let Err(e) = result {
                log::warn!("autostart toggle failed: {e}");
            }
            let app = app.clone();
            tauri::async_runtime::spawn(async move { refresh(&app, &manager).await });
        }
        _ => {
            let Some((target, action)) = id.split_once(':') else {
                return;
            };
            let ids: Vec<String> = {
                let state = app.state::<TrayState>();
                let all = state.device_ids.lock().expect("tray ids");
                if target == "all" {
                    all.clone()
                } else {
                    target
                        .strip_prefix('d')
                        .and_then(|n| n.parse::<usize>().ok())
                        .and_then(|n| all.get(n).cloned())
                        .into_iter()
                        .collect()
                }
            };
            let preset = action
                .strip_prefix('p')
                .and_then(|n| n.parse::<usize>().ok())
                .and_then(|n| PRESETS.get(n))
                .map(|(_, c)| *c);
            let off = action == "off";
            tauri::async_runtime::spawn(async move {
                for id in &ids {
                    let result = match (preset, off) {
                        (Some(c), _) => manager.set_color(id, c).await,
                        (None, true) => manager.set_effect(id, EffectId::Off as u8, 0).await,
                        _ => Ok(()),
                    };
                    if let Err(e) = result {
                        log::warn!("tray action on {id} failed: {e}");
                    }
                }
            });
        }
    }
}

/// Create the tray icon and keep its menu in sync with the devices.
pub fn setup<R: Runtime>(app: &AppHandle<R>, manager: DeviceManager) -> tauri::Result<()> {
    let menu = build_menu(app, &[])?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("RGB Visual Enhancer")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    app.state::<TrayState>()
        .available
        .store(true, Ordering::SeqCst);

    let app = app.clone();
    let mut events = manager.subscribe();
    tauri::async_runtime::spawn(async move {
        refresh(&app, &manager).await;
        // Ends when the manager's event channel closes.
        while let Ok(_) | Err(RecvError::Lagged(_)) = events.recv().await {
            // Debounce bursts (colour drags, scans).
            tokio::time::sleep(Duration::from_millis(300)).await;
            while events.try_recv().is_ok() {}
            refresh(&app, &manager).await;
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swatch_is_16x16_rgba() {
        let img = swatch(Some(Rgb::new(1, 2, 3)));
        assert_eq!((img.width(), img.height()), (16, 16));
        assert_eq!(img.rgba().len(), 16 * 16 * 4);
        // Centre pixel carries the colour.
        let c = (8 * 16 + 8) * 4;
        assert_eq!(&img.rgba()[c..c + 4], &[1, 2, 3, 255]);
    }

    fn texts<R: Runtime>(items: Vec<tauri::menu::MenuItemKind<R>>) -> Vec<String> {
        items
            .into_iter()
            .flat_map(|i| {
                let mut out = Vec::new();
                if let Some(t) = i.as_menuitem().and_then(|m| m.text().ok()) {
                    out.push(t);
                }
                if let Some(t) = i.as_icon_menuitem().and_then(|m| m.text().ok()) {
                    out.push(t);
                }
                if let Some(sub) = i.as_submenu() {
                    out.push(sub.text().unwrap_or_default());
                    out.extend(texts(sub.items().unwrap_or_default()));
                }
                out
            })
            .collect()
    }

    #[test]
    fn menu_lists_devices_with_battery_and_connection() {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ))
            .build(tauri::generate_context!())
            .expect("mock app");
        let detector = rgb_core::drivers::virtual_device::VirtualDetector::new(Arc::new(
            AtomicBool::new(true),
        ));
        let manager = DeviceManager::new(
            vec![Arc::new(detector)],
            rgb_core::ManagerConfig {
                battery_interval: Duration::from_millis(20),
                ..Default::default()
            },
        );
        let devices = tauri::async_runtime::block_on(async {
            manager.scan().await.unwrap();
            manager
                .set_color("virtual:g-mouse", Rgb::new(255, 0, 0))
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(60)).await;
            manager.devices().await
        });

        let menu = build_menu(app.handle(), &devices).expect("menu builds");
        let all = texts(menu.items().unwrap());
        let has = |needle: &str| all.iter().any(|t| t.contains(needle));
        assert!(has("Virtual Logitech G Headset   64 %"), "{all:#?}");
        assert!(has("Battery: 64 %"), "{all:#?}");
        assert!(has("Battery: 87 %, charging"), "{all:#?}");
        assert!(has("Connection: Simulated (wireless)"), "{all:#?}");
        assert!(has("Lighting: Static · #ff0000"), "{all:#?}");
        assert!(has("Quit RGB Visual Enhancer"));
        assert!(has("Purple"));
    }

    #[test]
    fn tooltip_fits_windows_limit() {
        assert!(tooltip(&[]).chars().count() <= 127);
        assert_eq!(truncate("abcdef", 4), "abc…");
    }
}
