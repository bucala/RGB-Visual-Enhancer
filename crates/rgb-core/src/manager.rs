//! Device registry and per-device worker tasks.
//!
//! Every controller is owned by a dedicated Tokio task (an actor). Callers
//! talk to it through a bounded channel, which gives us:
//!
//! * **Isolation** – a slow SMBus write never delays a USB mouse.
//! * **Ordering** – commands for one device run in the order they were sent.
//! * **Back-pressure / coalescing** – a colour picker can emit hundreds of
//!   updates per second; queued `SetColor` commands collapse into the most
//!   recent one so the hardware only ever sees the latest colour.
//! * **Timeouts** – each hardware call is bounded so a wedged device marks
//!   itself as errored instead of hanging the UI.

use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::stream::{FuturesUnordered, StreamExt};
use serde::Serialize;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::controller::{DeviceDetector, RgbController, RgbResult};
use crate::types::{BatteryInfo, DeviceInfo, EffectId, Rgb, Transport};

/// Tunables for the manager. The defaults suit real hardware; tests shrink them.
#[derive(Clone, Debug)]
pub struct ManagerConfig {
    /// Upper bound for `initialize`.
    pub init_timeout: Duration,
    /// Upper bound for a single colour/effect write.
    pub op_timeout: Duration,
    /// Upper bound for one detector pass (BLE scans dominate this).
    pub detect_timeout: Duration,
    /// Commands buffered per device before callers wait.
    pub queue_depth: usize,
    /// How often wireless devices are asked for their battery level.
    pub battery_interval: Duration,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            init_timeout: Duration::from_secs(15),
            op_timeout: Duration::from_secs(5),
            detect_timeout: Duration::from_secs(30),
            queue_depth: 64,
            battery_interval: Duration::from_secs(60),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "message", rename_all = "snake_case")]
pub enum DeviceStatus {
    Ready,
    Error(String),
}

/// What the UI needs to render a device: static info plus live state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeviceSnapshot {
    #[serde(flatten)]
    pub info: DeviceInfo,
    pub status: DeviceStatus,
    pub color: Option<Rgb>,
    pub effect: Option<u8>,
    pub speed: u8,
    pub battery: Option<BatteryInfo>,
    /// Bluetooth, or a device that has reported a battery (LIGHTSPEED etc.).
    pub wireless: bool,
    /// Per-key colours last applied (keyboards with a layout). Cleared when
    /// a whole-device colour or effect is applied.
    pub keys: BTreeMap<String, Rgb>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScanSummary {
    pub added: usize,
    pub total: usize,
    pub duration_ms: u64,
    pub warnings: Vec<String>,
}

/// Events broadcast to subscribers (the Tauri layer forwards them to the UI).
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ManagerEvent {
    ScanStarted,
    ScanFinished(ScanSummary),
    DeviceAdded { device: DeviceSnapshot },
    DeviceUpdated { device: DeviceSnapshot },
    DeviceRemoved { id: String },
    DeviceError { id: String, message: String },
    Warning { source: String, message: String },
}

enum Command {
    SetColor {
        color: Rgb,
        reply: oneshot::Sender<RgbResult<()>>,
    },
    SetZoneColor {
        zone: u8,
        color: Rgb,
        reply: oneshot::Sender<RgbResult<()>>,
    },
    SetEffect {
        effect: u8,
        speed: u8,
        reply: oneshot::Sender<RgbResult<()>>,
    },
    SetKeys {
        keys: Vec<(String, Rgb)>,
        reply: oneshot::Sender<RgbResult<()>>,
    },
    Shutdown {
        reply: oneshot::Sender<RgbResult<()>>,
    },
}

struct DeviceHandle {
    snapshot: Arc<std::sync::RwLock<DeviceSnapshot>>,
    tx: mpsc::Sender<Command>,
    task: JoinHandle<()>,
}

struct Inner {
    config: ManagerConfig,
    detectors: Vec<Arc<dyn DeviceDetector>>,
    devices: RwLock<BTreeMap<String, DeviceHandle>>,
    events: broadcast::Sender<ManagerEvent>,
    scanning: AtomicBool,
}

/// Cheap to clone; all clones share the same registry.
#[derive(Clone)]
pub struct DeviceManager {
    inner: Arc<Inner>,
}

impl DeviceManager {
    pub fn new(detectors: Vec<Arc<dyn DeviceDetector>>, config: ManagerConfig) -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            inner: Arc::new(Inner {
                config,
                detectors,
                devices: RwLock::new(BTreeMap::new()),
                events,
                scanning: AtomicBool::new(false),
            }),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ManagerEvent> {
        self.inner.events.subscribe()
    }

    pub fn is_scanning(&self) -> bool {
        self.inner.scanning.load(Ordering::SeqCst)
    }

    fn emit(&self, event: ManagerEvent) {
        // No subscribers is fine (e.g. during tests or before the UI attaches).
        let _ = self.inner.events.send(event);
    }

    pub async fn devices(&self) -> Vec<DeviceSnapshot> {
        let devices = self.inner.devices.read().await;
        devices
            .values()
            .map(|h| h.snapshot.read().expect("snapshot lock").clone())
            .collect()
    }

    pub async fn device(&self, id: &str) -> Option<DeviceSnapshot> {
        let devices = self.inner.devices.read().await;
        devices
            .get(id)
            .map(|h| h.snapshot.read().expect("snapshot lock").clone())
    }

    /// Run every detector concurrently and register whatever they find.
    /// Devices are announced (`DeviceAdded`) as soon as their detector
    /// finishes, so fast USB devices appear before a slow BLE scan completes.
    pub async fn scan(&self) -> RgbResult<ScanSummary> {
        if self.inner.scanning.swap(true, Ordering::SeqCst) {
            return Err("a scan is already in progress".into());
        }
        // Reset the flag even if this future is dropped mid-scan.
        struct ScanGuard<'a>(&'a AtomicBool);
        impl Drop for ScanGuard<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::SeqCst);
            }
        }
        let _guard = ScanGuard(&self.inner.scanning);

        let started = Instant::now();
        self.emit(ManagerEvent::ScanStarted);

        let known: Arc<HashSet<String>> =
            Arc::new(self.inner.devices.read().await.keys().cloned().collect());
        let detect_timeout = self.inner.config.detect_timeout;

        let mut pending: FuturesUnordered<_> =
            self.inner
                .detectors
                .iter()
                .cloned()
                .map(|detector| {
                    let known = known.clone();
                    let name = detector.name();
                    // Each detector runs on its own task so blocking-ish
                    // enumeration in one driver can't starve the others.
                    let task = tokio::spawn(async move {
                        timeout(detect_timeout, detector.detect(&known)).await
                    });
                    async move { (name, task.await) }
                })
                .collect();

        let mut added = 0;
        let mut warnings = Vec::new();
        while let Some((name, joined)) = pending.next().await {
            let report = match joined {
                Ok(Ok(report)) => report,
                Ok(Err(_)) => {
                    let msg = format!("detection timed out after {detect_timeout:?}");
                    self.warn(name, &msg, &mut warnings);
                    continue;
                }
                Err(e) => {
                    let msg = format!("detector crashed: {e}");
                    self.warn(name, &msg, &mut warnings);
                    continue;
                }
            };
            for w in &report.warnings {
                self.warn(name, w, &mut warnings);
            }
            for controller in report.controllers {
                match self.register(controller).await {
                    Ok(true) => added += 1,
                    Ok(false) => {}
                    Err(e) => self.warn(name, &e, &mut warnings),
                }
            }
        }

        let summary = ScanSummary {
            added,
            total: self.inner.devices.read().await.len(),
            duration_ms: started.elapsed().as_millis() as u64,
            warnings,
        };
        self.emit(ManagerEvent::ScanFinished(summary.clone()));
        Ok(summary)
    }

    fn warn(&self, source: &str, message: &str, sink: &mut Vec<String>) {
        log::warn!("[{source}] {message}");
        sink.push(format!("{source}: {message}"));
        self.emit(ManagerEvent::Warning {
            source: source.to_string(),
            message: message.to_string(),
        });
    }

    /// Initialise a controller and hand it to a new worker task.
    /// Returns `Ok(false)` when an equivalent device is already registered.
    pub async fn register(&self, mut controller: Box<dyn RgbController>) -> RgbResult<bool> {
        let init_timeout = self.inner.config.init_timeout;
        match timeout(init_timeout, controller.initialize()).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                let _ = controller.shutdown().await;
                return Err(format!("initialisation failed: {e}"));
            }
            Err(_) => {
                let _ = controller.shutdown().await;
                return Err(format!("initialisation timed out after {init_timeout:?}"));
            }
        }
        let info = controller.get_device_info().await;

        let mut devices = self.inner.devices.write().await;
        if devices.contains_key(&info.id) {
            drop(devices);
            let _ = controller.shutdown().await;
            return Ok(false);
        }

        let snapshot = Arc::new(std::sync::RwLock::new(DeviceSnapshot {
            wireless: info.transport == Transport::Ble,
            info: info.clone(),
            status: DeviceStatus::Ready,
            color: None,
            effect: None,
            speed: 50,
            battery: None,
            keys: BTreeMap::new(),
        }));
        let (tx, rx) = mpsc::channel(self.inner.config.queue_depth);
        let task = tokio::spawn(run_worker(
            controller,
            rx,
            snapshot.clone(),
            self.inner.events.clone(),
            self.inner.config.op_timeout,
            self.inner.config.battery_interval,
        ));
        let device = snapshot.read().expect("snapshot lock").clone();
        devices.insert(info.id.clone(), DeviceHandle { snapshot, tx, task });
        drop(devices);

        log::info!("registered {} ({})", info.name, info.id);
        self.emit(ManagerEvent::DeviceAdded { device });
        Ok(true)
    }

    async fn send(
        &self,
        id: &str,
        make: impl FnOnce(oneshot::Sender<RgbResult<()>>) -> Command,
    ) -> RgbResult<()> {
        let tx = {
            let devices = self.inner.devices.read().await;
            devices
                .get(id)
                .map(|h| h.tx.clone())
                .ok_or_else(|| format!("unknown device '{id}'"))?
        };
        let (reply, rx) = oneshot::channel();
        tx.send(make(reply))
            .await
            .map_err(|_| format!("device '{id}' is shutting down"))?;
        rx.await
            .map_err(|_| format!("device '{id}' worker stopped"))?
    }

    pub async fn set_color(&self, id: &str, color: Rgb) -> RgbResult<()> {
        self.send(id, |reply| Command::SetColor { color, reply })
            .await
    }

    pub async fn set_zone_color(&self, id: &str, zone: u8, color: Rgb) -> RgbResult<()> {
        self.send(id, |reply| Command::SetZoneColor { zone, color, reply })
            .await
    }

    pub async fn set_effect(&self, id: &str, effect: u8, speed: u8) -> RgbResult<()> {
        EffectId::try_from(effect)?;
        self.send(id, |reply| Command::SetEffect {
            effect,
            speed,
            reply,
        })
        .await
    }

    /// Per-key colours (keyboards with a layout).
    pub async fn set_key_colors(&self, id: &str, keys: Vec<(String, Rgb)>) -> RgbResult<()> {
        self.send(id, |reply| Command::SetKeys { keys, reply })
            .await
    }

    /// Apply a colour to many devices concurrently. Returns per-device errors.
    pub async fn set_color_many(&self, ids: &[String], color: Rgb) -> Vec<(String, String)> {
        let results = futures::future::join_all(
            ids.iter()
                .map(|id| async move { (id.clone(), self.set_color(id, color).await) }),
        )
        .await;
        results
            .into_iter()
            .filter_map(|(id, r)| r.err().map(|e| (id, e)))
            .collect()
    }

    /// Stop a device's worker and release its hardware.
    pub async fn remove(&self, id: &str) -> RgbResult<()> {
        let handle = self
            .inner
            .devices
            .write()
            .await
            .remove(id)
            .ok_or_else(|| format!("unknown device '{id}'"))?;
        let result = shutdown_handle(handle, self.inner.config.op_timeout).await;
        self.emit(ManagerEvent::DeviceRemoved { id: id.to_string() });
        result
    }

    /// Remove every device whose id matches `pred` (e.g. all virtual devices).
    pub async fn remove_where(&self, pred: impl Fn(&DeviceSnapshot) -> bool) {
        let ids: Vec<String> = self
            .devices()
            .await
            .into_iter()
            .filter(|d| pred(d))
            .map(|d| d.info.id)
            .collect();
        for id in ids {
            let _ = self.remove(&id).await;
        }
    }

    /// Gracefully release all hardware. Call on application exit.
    pub async fn shutdown_all(&self) {
        let handles: Vec<_> = std::mem::take(&mut *self.inner.devices.write().await)
            .into_values()
            .collect();
        let op_timeout = self.inner.config.op_timeout;
        futures::future::join_all(handles.into_iter().map(|h| shutdown_handle(h, op_timeout)))
            .await;
    }
}

async fn shutdown_handle(handle: DeviceHandle, op_timeout: Duration) -> RgbResult<()> {
    let (reply, rx) = oneshot::channel();
    let result = if handle.tx.send(Command::Shutdown { reply }).await.is_ok() {
        match timeout(op_timeout, rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err("worker stopped".into()),
            Err(_) => Err("shutdown timed out".into()),
        }
    } else {
        Ok(())
    };
    // Make sure the task is gone even if the controller hung.
    if !handle.task.is_finished() {
        tokio::time::sleep(Duration::from_millis(10)).await;
        handle.task.abort();
    }
    result
}

async fn run_worker(
    mut controller: Box<dyn RgbController>,
    mut rx: mpsc::Receiver<Command>,
    snapshot: Arc<std::sync::RwLock<DeviceSnapshot>>,
    events: broadcast::Sender<ManagerEvent>,
    op_timeout: Duration,
    battery_interval: Duration,
) {
    let mut stashed: Option<Command> = None;
    // First tick fires immediately, giving an initial battery reading.
    let mut battery_tick = tokio::time::interval(battery_interval);
    battery_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        let cmd = match stashed.take() {
            Some(cmd) => cmd,
            None => tokio::select! {
                // Commands first: a battery poll never delays user input.
                biased;
                cmd = rx.recv() => match cmd {
                    Some(cmd) => cmd,
                    None => break,
                },
                _ = battery_tick.tick() => {
                    poll_battery(controller.as_mut(), &snapshot, &events, op_timeout).await;
                    continue;
                }
            },
        };

        match cmd {
            Command::SetColor { mut color, reply } => {
                // Coalesce: fold any immediately-queued colour updates into
                // one hardware write, preserving order relative to other
                // commands by stashing the first non-colour command.
                let mut replies = vec![reply];
                loop {
                    match rx.try_recv() {
                        Ok(Command::SetColor { color: c, reply: r }) => {
                            color = c;
                            replies.push(r);
                        }
                        Ok(other) => {
                            stashed = Some(other);
                            break;
                        }
                        Err(_) => break,
                    }
                }
                let result =
                    bounded(op_timeout, controller.set_color(color.r, color.g, color.b)).await;
                finish(&snapshot, &events, &result, |s| {
                    s.keys.clear();
                    s.color = Some(color);
                    let keeps_effect = s
                        .effect
                        .and_then(|e| EffectId::try_from(e).ok())
                        .is_some_and(EffectId::uses_color);
                    if !keeps_effect {
                        s.effect = Some(EffectId::Static as u8);
                    }
                });
                for r in replies {
                    let _ = r.send(result.clone());
                }
            }
            Command::SetZoneColor { zone, color, reply } => {
                let result = bounded(op_timeout, controller.set_zone_color(zone, color)).await;
                finish(&snapshot, &events, &result, |s| {
                    if s.info.zones.len() <= 1 {
                        s.color = Some(color);
                    }
                });
                let _ = reply.send(result);
            }
            Command::SetEffect {
                effect,
                speed,
                reply,
            } => {
                let result = bounded(op_timeout, controller.set_effect(effect, speed)).await;
                finish(&snapshot, &events, &result, |s| {
                    s.keys.clear();
                    s.effect = Some(effect);
                    s.speed = speed.min(crate::types::SPEED_MAX);
                });
                let _ = reply.send(result);
            }
            Command::SetKeys { keys, reply } => {
                // Coalesce consecutive per-key updates (brush strokes) into
                // one frame; later colours for the same key win.
                let mut merged: BTreeMap<String, Rgb> = keys.into_iter().collect();
                let mut replies = vec![reply];
                loop {
                    match rx.try_recv() {
                        Ok(Command::SetKeys { keys, reply: r }) => {
                            merged.extend(keys);
                            replies.push(r);
                        }
                        Ok(other) => {
                            stashed = Some(other);
                            break;
                        }
                        Err(_) => break,
                    }
                }
                let list: Vec<(String, Rgb)> =
                    merged.iter().map(|(k, c)| (k.clone(), *c)).collect();
                let result = bounded(op_timeout, controller.set_key_colors(&list)).await;
                finish(&snapshot, &events, &result, |s| s.keys.extend(merged));
                for r in replies {
                    let _ = r.send(result.clone());
                }
            }
            Command::Shutdown { reply } => {
                let result = bounded(op_timeout, controller.shutdown()).await;
                let _ = reply.send(result);
                break;
            }
        }
    }
}

async fn poll_battery(
    controller: &mut dyn RgbController,
    snapshot: &std::sync::RwLock<DeviceSnapshot>,
    events: &broadcast::Sender<ManagerEvent>,
    op_timeout: Duration,
) {
    let battery = timeout(op_timeout, controller.battery())
        .await
        .ok()
        .flatten();
    let device = {
        let mut s = snapshot.write().expect("snapshot lock");
        if s.battery == battery {
            return;
        }
        s.battery = battery;
        // Sticky: a headset that is switched off is still a wireless device.
        s.wireless |= battery.is_some();
        s.clone()
    };
    let _ = events.send(ManagerEvent::DeviceUpdated { device });
}

async fn bounded<F>(limit: Duration, fut: F) -> RgbResult<()>
where
    F: std::future::Future<Output = RgbResult<()>>,
{
    match timeout(limit, fut).await {
        Ok(r) => r,
        Err(_) => Err(format!("device did not respond within {limit:?}")),
    }
}

/// Update the snapshot after a hardware operation and notify listeners.
fn finish(
    snapshot: &std::sync::RwLock<DeviceSnapshot>,
    events: &broadcast::Sender<ManagerEvent>,
    result: &RgbResult<()>,
    on_ok: impl FnOnce(&mut DeviceSnapshot),
) {
    let device = {
        let mut s = snapshot.write().expect("snapshot lock");
        match result {
            Ok(()) => {
                on_ok(&mut s);
                s.status = DeviceStatus::Ready;
            }
            Err(e) => s.status = DeviceStatus::Error(e.clone()),
        }
        s.clone()
    };
    if let Err(e) = result {
        let _ = events.send(ManagerEvent::DeviceError {
            id: device.info.id.clone(),
            message: e.clone(),
        });
    }
    let _ = events.send(ManagerEvent::DeviceUpdated { device });
}
