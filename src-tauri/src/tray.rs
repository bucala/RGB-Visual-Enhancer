//! System tray: keeps the app running in the background (like G HUB) and
//! shows every device's state with quick colour actions.
//!
//! * **Windows / macOS:** right-click opens a frameless, transparent webview
//!   window rendered by the React app (`?view=tray`), so the menu matches the
//!   app's look. It is anchored to the cursor inside the monitor's work area
//!   and hides when it loses focus.
//! * **Linux:** AppIndicator trays only support native menus (no click
//!   events), so a localized native menu is built instead.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rgb_core::{DeviceManager, DeviceSnapshot, DeviceStatus, EffectId, Rgb, Transport};
use tauri::image::Image;
use tauri::menu::{
    CheckMenuItemBuilder, IconMenuItemBuilder, Menu, MenuBuilder, MenuEvent, MenuItemBuilder,
    Submenu, SubmenuBuilder,
};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, Runtime, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
use tauri_plugin_autostart::ManagerExt as _;
use tokio::sync::broadcast::error::RecvError;

use crate::settings::{Language, SettingsStore};
use crate::AppState;

pub const TRAY_ID: &str = "main";
/// Label of the custom tray menu window.
pub const MENU_WINDOW: &str = "tray-menu";
/// Command-line flag used by autostart: start hidden in the tray.
pub const MINIMIZED_ARG: &str = "--minimized";
/// Logical width of the tray menu window (CSS adds the shadow margin).
pub const MENU_WIDTH: f64 = 340.0;

/// Whether this platform uses the custom webview tray menu.
pub fn uses_custom_menu() -> bool {
    !cfg!(target_os = "linux")
}

pub const PRESETS: [Rgb; 8] = [
    Rgb::new(255, 0, 0),
    Rgb::new(255, 96, 0),
    Rgb::new(255, 200, 0),
    Rgb::new(0, 255, 64),
    Rgb::new(0, 220, 255),
    Rgb::new(0, 64, 255),
    Rgb::new(140, 0, 255),
    Rgb::new(255, 255, 255),
];

/// Text used by the native menu and the tooltip.
pub struct Strings {
    no_devices: &'static str,
    devices: &'static str,
    status: &'static str,
    ready: &'static str,
    error: &'static str,
    connection: &'static str,
    battery: &'static str,
    charging: &'static str,
    lighting: &'static str,
    not_set: &'static str,
    lights_off: &'static str,
    all_devices: &'static str,
    open: &'static str,
    rescan: &'static str,
    autostart: &'static str,
    quit: &'static str,
    wireless_usb: &'static str,
    simulated: &'static str,
    simulated_wireless: &'static str,
    presets: [&'static str; 8],
    /// Indexed by `EffectId`.
    effects: [&'static str; 7],
}

pub fn strings(lang: Language) -> &'static Strings {
    static SK: Strings = Strings {
        no_devices: "Nenašli sa žiadne zariadenia",
        devices: "zariadení",
        status: "Stav",
        ready: "Pripravené",
        error: "Chyba",
        connection: "Spojenie",
        battery: "Batéria",
        charging: "nabíja sa",
        lighting: "Svetlo",
        not_set: "Zatiaľ nenastavené",
        lights_off: "Vypnúť svetlá",
        all_devices: "Všetky zariadenia",
        open: "Otvoriť RGB Visual Enhancer",
        rescan: "Znova vyhľadať hardvér",
        autostart: "Spúšťať so systémom (v lište)",
        quit: "Ukončiť RGB Visual Enhancer",
        wireless_usb: "Bezdrôtovo (USB prijímač)",
        simulated: "Simulované",
        simulated_wireless: "Simulované (bezdrôtové)",
        presets: [
            "Červená",
            "Oranžová",
            "Žltá",
            "Zelená",
            "Azúrová",
            "Modrá",
            "Fialová",
            "Biela",
        ],
        effects: [
            "Vypnuté",
            "Statická",
            "Dýchanie",
            "Cyklus spektra",
            "Dúhová vlna",
            "Farebný pulz",
            "Blikanie",
        ],
    };
    static EN: Strings = Strings {
        no_devices: "No devices found",
        devices: "device(s)",
        status: "Status",
        ready: "Ready",
        error: "Error",
        connection: "Connection",
        battery: "Battery",
        charging: "charging",
        lighting: "Lighting",
        not_set: "Not set by this app yet",
        lights_off: "Lights off",
        all_devices: "All devices",
        open: "Open RGB Visual Enhancer",
        rescan: "Rescan hardware",
        autostart: "Start with system (in tray)",
        quit: "Quit RGB Visual Enhancer",
        wireless_usb: "Wireless (USB receiver)",
        simulated: "Simulated",
        simulated_wireless: "Simulated (wireless)",
        presets: [
            "Red", "Orange", "Yellow", "Green", "Cyan", "Blue", "Purple", "White",
        ],
        effects: [
            "Off",
            "Static",
            "Breathing",
            "Spectrum Cycle",
            "Rainbow Wave",
            "Color Pulse",
            "Flashing",
        ],
    };
    match lang {
        Language::Sk => &SK,
        Language::En => &EN,
    }
}

/// Tray-related state shared with window event handlers and commands.
#[derive(Default)]
pub struct TrayState {
    /// True once the tray icon exists; only then may windows hide into it.
    pub available: AtomicBool,
    /// Device ids in native-menu order; item ids refer to them by index.
    device_ids: Mutex<Vec<String>>,
    /// Where the custom menu was opened (cursor position, physical px).
    anchor: Mutex<Option<PhysicalPosition<f64>>>,
    /// When the custom menu last hid; clicking the icon right after the
    /// blur-hide should close it, not reopen it.
    last_hidden: Mutex<Option<Instant>>,
}

fn language<R: Runtime>(app: &AppHandle<R>) -> Language {
    app.try_state::<SettingsStore>()
        .map(|s| s.language())
        .unwrap_or_default()
}

pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    hide_menu(app);
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

// ---------------------------------------------------------------------------
// Custom (webview) tray menu
// ---------------------------------------------------------------------------

pub fn create_menu_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<WebviewWindow<R>> {
    let builder = WebviewWindowBuilder::new(
        app,
        MENU_WINDOW,
        WebviewUrl::App("index.html?view=tray".into()),
    )
    .title("RGB Visual Enhancer")
    .inner_size(MENU_WIDTH, 420.0)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .shadow(false)
    .focused(false)
    .visible(false);
    #[cfg(not(target_os = "macos"))]
    let builder = builder.transparent(true);
    builder.build()
}

/// Place the menu so its bottom-right corner sits at the anchor, kept inside
/// the monitor's work area (works for taskbars on any edge).
fn position_menu<R: Runtime>(w: &WebviewWindow<R>, anchor: PhysicalPosition<f64>) {
    let Ok(size) = w.outer_size() else { return };
    let monitor = w
        .monitor_from_point(anchor.x, anchor.y)
        .ok()
        .flatten()
        .or_else(|| w.primary_monitor().ok().flatten());
    let (w_px, h_px) = (size.width as i32, size.height as i32);
    let mut x = anchor.x as i32 - w_px;
    let mut y = anchor.y as i32 - h_px;
    if let Some(m) = monitor {
        let wa = m.work_area();
        let (left, top) = (wa.position.x, wa.position.y);
        let right = left + wa.size.width as i32 - w_px;
        let bottom = top + wa.size.height as i32 - h_px;
        x = x.min(right).max(left);
        y = y.min(bottom).max(top);
    }
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

fn show_menu<R: Runtime>(app: &AppHandle<R>, anchor: PhysicalPosition<f64>) {
    let state = app.state::<TrayState>();
    let recently_hidden = state
        .last_hidden
        .lock()
        .expect("tray state")
        .is_some_and(|t| t.elapsed() < Duration::from_millis(300));
    if recently_hidden {
        return; // this click closed the menu (blur); don't reopen it
    }
    let Some(w) = app.get_webview_window(MENU_WINDOW) else {
        return;
    };
    *state.anchor.lock().expect("tray state") = Some(anchor);
    position_menu(&w, anchor);
    let _ = w.emit("tray-menu://opened", ());
    let _ = w.show();
    let _ = w.set_focus();
}

pub fn hide_menu<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window(MENU_WINDOW) {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
            *app.state::<TrayState>()
                .last_hidden
                .lock()
                .expect("tray state") = Some(Instant::now());
        }
    }
}

/// Called by the menu webview when its content height changes.
pub fn resize_menu<R: Runtime>(app: &AppHandle<R>, height: f64) {
    let Some(w) = app.get_webview_window(MENU_WINDOW) else {
        return;
    };
    let _ = w.set_size(LogicalSize::new(MENU_WIDTH, height.clamp(120.0, 900.0)));
    if let Some(anchor) = *app.state::<TrayState>().anchor.lock().expect("tray state") {
        position_menu(&w, anchor);
    }
}

// ---------------------------------------------------------------------------
// Native menu (Linux)
// ---------------------------------------------------------------------------

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

fn connection(d: &DeviceSnapshot, s: &Strings) -> &'static str {
    match (d.info.transport, d.wireless) {
        (Transport::Ble, _) => "Bluetooth",
        (Transport::Usb, true) => s.wireless_usb,
        (Transport::Usb, false) => "USB",
        (Transport::Smbus, _) => "SMBus",
        (Transport::I2c, _) => "GPU I²C",
        (Transport::Virtual, true) => s.simulated_wireless,
        (Transport::Virtual, false) => s.simulated,
    }
}

fn battery_text(d: &DeviceSnapshot, s: &Strings) -> Option<String> {
    d.battery.map(|b| {
        let approx = if b.approximate { "≈" } else { "" };
        let charging = if b.charging {
            format!(", {}", s.charging)
        } else {
            String::new()
        };
        format!("{approx}{} %{charging}", b.percent)
    })
}

fn lighting_text(d: &DeviceSnapshot, s: &Strings) -> String {
    let effect = d.effect.and_then(|id| EffectId::try_from(id).ok());
    match (effect, d.color) {
        (Some(EffectId::Off), _) => s.effects[0].into(),
        (Some(e), Some(c)) if e.uses_color() => format!("{} · {c}", s.effects[e as usize]),
        (Some(e), _) => s.effects[e as usize].into(),
        (None, Some(c)) => c.to_string(),
        (None, None) => s.not_set.into(),
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
    s: &Strings,
    prefix: &str,
    builder: SubmenuBuilder<'a, R, AppHandle<R>>,
    with_color: bool,
    with_off: bool,
) -> tauri::Result<SubmenuBuilder<'a, R, AppHandle<R>>> {
    let mut b = builder;
    if with_color {
        for (n, c) in PRESETS.iter().enumerate() {
            let item = IconMenuItemBuilder::with_id(format!("{prefix}:p{n}"), s.presets[n])
                .icon(swatch(Some(*c)))
                .build(app)?;
            b = b.item(&item);
        }
    }
    if with_off {
        b = b.item(&MenuItemBuilder::with_id(format!("{prefix}:off"), s.lights_off).build(app)?);
    }
    Ok(b)
}

fn device_submenu<R: Runtime>(
    app: &AppHandle<R>,
    s: &Strings,
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
        DeviceStatus::Ready => s.ready.to_string(),
        DeviceStatus::Error(e) => format!("{}: {}", s.error, truncate(e, 60)),
    };

    let mut b = SubmenuBuilder::with_id(app, format!("d{i}"), title)
        .submenu_icon(swatch(
            d.color.filter(|_| d.effect != Some(EffectId::Off as u8)),
        ))
        .item(&info_item(app, format!("{}: {status}", s.status))?)
        .item(&info_item(
            app,
            format!("{}: {}", s.connection, connection(d, s)),
        )?);
    if let Some(bt) = battery_text(d, s) {
        b = b.item(&info_item(app, format!("{}: {bt}", s.battery))?);
    }
    b = b
        .item(&info_item(
            app,
            format!("{}: {}", s.lighting, lighting_text(d, s)),
        )?)
        .separator();
    let with_color = d.info.effects.iter().any(|e| e.uses_color);
    let with_off = d.info.effects.iter().any(|e| e.id == EffectId::Off as u8);
    preset_items(app, s, &format!("d{i}"), b, with_color, with_off)?.build()
}

fn build_menu<R: Runtime>(
    app: &AppHandle<R>,
    lang: Language,
    devices: &[DeviceSnapshot],
) -> tauri::Result<Menu<R>> {
    let s = strings(lang);
    let mut menu = MenuBuilder::new(app)
        .item(&info_item(app, "RGB Visual Enhancer".into())?)
        .separator();
    if devices.is_empty() {
        menu = menu.item(&info_item(app, s.no_devices.into())?);
    }
    for (i, d) in devices.iter().enumerate() {
        menu = menu.item(&device_submenu(app, s, i, d)?);
    }
    let all = preset_items(
        app,
        s,
        "all",
        SubmenuBuilder::new(app, s.all_devices),
        true,
        true,
    )?
    .build()?;
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    menu.separator()
        .item(&all)
        .separator()
        .item(&MenuItemBuilder::with_id("open", s.open).build(app)?)
        .item(&MenuItemBuilder::with_id("rescan", s.rescan).build(app)?)
        .item(
            &CheckMenuItemBuilder::with_id("autostart", s.autostart)
                .checked(autostart)
                .build(app)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id("quit", s.quit).build(app)?)
        .build()
}

fn tooltip(lang: Language, devices: &[DeviceSnapshot]) -> String {
    let s = strings(lang);
    let mut t = format!("RGB Visual Enhancer · {} {}", devices.len(), s.devices);
    for d in devices.iter().filter(|d| d.battery.is_some()) {
        let b = d.battery.expect("filtered");
        t.push_str(&format!("\n{} {} %", truncate(&d.info.name, 28), b.percent));
    }
    // Windows truncates tray tooltips at 127 characters.
    truncate(&t, 127)
}

/// Rebuild the native menu (Linux) and the tooltip.
pub async fn refresh<R: Runtime>(app: &AppHandle<R>, manager: &DeviceManager) {
    let devices = manager.devices().await;
    let lang = language(app);
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    if !uses_custom_menu() {
        match build_menu(app, lang, &devices) {
            Ok(menu) => {
                *app.state::<TrayState>()
                    .device_ids
                    .lock()
                    .expect("tray ids") = devices.iter().map(|d| d.info.id.clone()).collect();
                let _ = tray.set_menu(Some(menu));
            }
            Err(e) => log::warn!("failed to build tray menu: {e}"),
        }
    }
    let _ = tray.set_tooltip(Some(tooltip(lang, &devices)));
}

pub fn toggle_autostart<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> Result<(), String> {
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(|e| e.to_string())
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
            let enabled = app.autolaunch().is_enabled().unwrap_or(false);
            if let Err(e) = toggle_autostart(app, !enabled) {
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
                .copied();
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

/// Create the tray icon (and the custom menu window) and keep them in sync.
pub fn setup<R: Runtime>(app: &AppHandle<R>, manager: DeviceManager) -> tauri::Result<()> {
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("RGB Visual Enhancer")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            else {
                return;
            };
            match button {
                MouseButton::Left => toggle_main_window(tray.app_handle()),
                MouseButton::Right if uses_custom_menu() => show_menu(tray.app_handle(), position),
                _ => {}
            }
        });
    if uses_custom_menu() {
        create_menu_window(app)?;
    } else {
        builder = builder
            .menu(&build_menu(app, language(app), &[])?)
            .on_menu_event(on_menu_event);
    }
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

    fn virtual_devices() -> Vec<DeviceSnapshot> {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;
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
        tauri::async_runtime::block_on(async {
            manager.scan().await.unwrap();
            manager
                .set_color("virtual:g-mouse", Rgb::new(255, 0, 0))
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(60)).await;
            manager.devices().await
        })
    }

    #[test]
    fn native_menu_is_localized_with_battery_and_connection() {
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ))
            .build(tauri::generate_context!())
            .expect("mock app");
        let devices = virtual_devices();

        let en = texts(
            build_menu(app.handle(), Language::En, &devices)
                .unwrap()
                .items()
                .unwrap(),
        );
        let has = |all: &[String], needle: &str| all.iter().any(|t| t.contains(needle));
        assert!(has(&en, "Virtual Logitech G Headset   64 %"), "{en:#?}");
        assert!(has(&en, "Battery: 87 %, charging"), "{en:#?}");
        assert!(has(&en, "Connection: Simulated (wireless)"), "{en:#?}");
        assert!(has(&en, "Lighting: Static · #ff0000"), "{en:#?}");
        assert!(has(&en, "Quit RGB Visual Enhancer"));

        let sk = texts(
            build_menu(app.handle(), Language::Sk, &devices)
                .unwrap()
                .items()
                .unwrap(),
        );
        assert!(has(&sk, "Batéria: 87 %, nabíja sa"), "{sk:#?}");
        assert!(has(&sk, "Svetlo: Statická · #ff0000"), "{sk:#?}");
        assert!(has(&sk, "Fialová"));
        assert!(has(&sk, "Ukončiť RGB Visual Enhancer"));
    }

    #[test]
    fn tooltip_fits_windows_limit() {
        assert!(tooltip(Language::Sk, &[]).chars().count() <= 127);
        assert_eq!(truncate("abcdef", 4), "abc…");
    }
}
