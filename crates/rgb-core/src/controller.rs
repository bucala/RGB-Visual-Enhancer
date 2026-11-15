//! The unified driver interface every vendor/protocol implements.

use async_trait::async_trait;
use std::collections::HashSet;

use crate::types::{DeviceInfo, Rgb};

/// Error type used across the driver boundary. Kept as a plain string so it
/// crosses the Tauri IPC boundary without conversion.
pub type RgbResult<T> = Result<T, String>;

/// Common asynchronous interface for every RGB device.
///
/// # Threading contract
///
/// The [`DeviceManager`](crate::DeviceManager) gives each controller its own
/// worker task and is the only caller, so methods take `&mut self` and never
/// race each other. Implementations **must not block the async runtime**:
/// synchronous I/O (hidapi, SMBus ioctls) belongs in
/// [`tokio::task::spawn_blocking`]. The manager also wraps every call in a
/// timeout so a wedged device cannot stall the app.
#[async_trait]
pub trait RgbController: Send + Sync {
    /// Open/handshake with the hardware and discover zones and effects.
    /// Called exactly once, before any other method.
    async fn initialize(&mut self) -> RgbResult<()>;

    /// Set the colour of every zone.
    ///
    /// If the active effect renders a colour (Static, Breathing, Color
    /// Pulse, Flashing) it keeps running with the new colour; otherwise the
    /// device switches to Static.
    async fn set_color(&mut self, red: u8, green: u8, blue: u8) -> RgbResult<()>;

    /// Switch to a hardware effect. `effect_id` is an
    /// [`EffectId`](crate::EffectId) value; `speed` is 0 (slowest)–100
    /// (fastest). Colour-based effects use the most recent colour.
    async fn set_effect(&mut self, effect_id: u8, speed: u8) -> RgbResult<()>;

    /// Describe the device. Valid after [`initialize`](Self::initialize).
    async fn get_device_info(&self) -> DeviceInfo;

    /// Set a single zone. Drivers without per-zone control fall back to
    /// colouring the whole device.
    async fn set_zone_color(&mut self, zone: u8, color: Rgb) -> RgbResult<()> {
        let _ = zone;
        self.set_color(color.r, color.g, color.b).await
    }

    /// Release the hardware (close handles, disconnect BLE, …).
    async fn shutdown(&mut self) -> RgbResult<()> {
        Ok(())
    }
}

/// Output of a single detector pass.
#[derive(Default)]
pub struct DetectionReport {
    pub controllers: Vec<Box<dyn RgbController>>,
    /// Non-fatal problems worth surfacing to the user (missing permissions,
    /// kernel module not loaded, no Bluetooth adapter, …).
    pub warnings: Vec<String>,
}

impl DetectionReport {
    pub fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }
}

/// Discovers devices for one driver family.
#[async_trait]
pub trait DeviceDetector: Send + Sync {
    /// Short name shown in diagnostics, e.g. `"Logitech HID++"`.
    fn name(&self) -> &'static str;

    /// Enumerate hardware and return *uninitialised* controllers. Devices
    /// whose id is in `known` are already managed and must be skipped (their
    /// handles are open), so rescans are cheap and non-disruptive.
    async fn detect(&self, known: &HashSet<String>) -> DetectionReport;
}
