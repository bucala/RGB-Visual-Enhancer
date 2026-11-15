//! End-to-end behaviour of the DeviceManager using simulated controllers.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rgb_core::drivers::virtual_device::{VirtualController, VirtualDetector};
use rgb_core::{
    DetectionReport, DeviceDetector, DeviceInfo, DeviceManager, DeviceStatus, DeviceType, EffectId,
    ManagerConfig, ManagerEvent, Rgb, RgbController, RgbResult,
};

fn fast_config() -> ManagerConfig {
    ManagerConfig {
        init_timeout: Duration::from_millis(500),
        op_timeout: Duration::from_millis(200),
        detect_timeout: Duration::from_millis(500),
        queue_depth: 64,
    }
}

fn virtual_manager() -> DeviceManager {
    let detector = VirtualDetector::new(Arc::new(AtomicBool::new(true)));
    DeviceManager::new(vec![Arc::new(detector)], fast_config())
}

#[tokio::test]
async fn scan_registers_devices_and_rescan_is_idempotent() {
    let manager = virtual_manager();
    let mut events = manager.subscribe();

    let first = manager.scan().await.unwrap();
    assert_eq!(first.added, VirtualController::demo_set().len());
    let second = manager.scan().await.unwrap();
    assert_eq!(second.added, 0);
    assert_eq!(second.total, first.total);

    let mut added = 0;
    while let Ok(ev) = events.try_recv() {
        if matches!(ev, ManagerEvent::DeviceAdded { .. }) {
            added += 1;
        }
    }
    assert_eq!(added, first.added);
}

#[tokio::test]
async fn concurrent_scans_are_rejected() {
    let manager = virtual_manager();
    let (a, b) = tokio::join!(manager.scan(), manager.scan());
    assert!(a.is_ok() != b.is_ok(), "exactly one scan should run");
}

#[tokio::test]
async fn set_color_and_effect_update_snapshot() {
    let manager = virtual_manager();
    manager.scan().await.unwrap();
    let id = "virtual:g-keyboard";

    manager.set_color(id, Rgb::new(1, 2, 3)).await.unwrap();
    let snap = manager.device(id).await.unwrap();
    assert_eq!(snap.color, Some(Rgb::new(1, 2, 3)));
    assert_eq!(snap.effect, Some(EffectId::Static as u8));

    manager
        .set_effect(id, EffectId::Breathing as u8, 80)
        .await
        .unwrap();
    let snap = manager.device(id).await.unwrap();
    assert_eq!(snap.effect, Some(EffectId::Breathing as u8));
    assert_eq!(snap.speed, 80);
    assert_eq!(snap.status, DeviceStatus::Ready);

    // Recolouring keeps a colour-based effect running…
    manager.set_color(id, Rgb::new(9, 9, 9)).await.unwrap();
    assert_eq!(
        manager.device(id).await.unwrap().effect,
        Some(EffectId::Breathing as u8)
    );

    // …but falls back to Static from an effect that ignores colour.
    manager
        .set_effect(id, EffectId::SpectrumCycle as u8, 10)
        .await
        .unwrap();
    manager.set_color(id, Rgb::new(4, 5, 6)).await.unwrap();
    let snap = manager.device(id).await.unwrap();
    assert_eq!(snap.effect, Some(EffectId::Static as u8));
    assert_eq!(snap.color, Some(Rgb::new(4, 5, 6)));
}

#[tokio::test]
async fn unsupported_effect_marks_error_and_emits_event() {
    let manager = virtual_manager();
    manager.scan().await.unwrap();
    let mut events = manager.subscribe();

    let id = "virtual:g-mouse";
    let err = manager
        .set_effect(id, EffectId::RainbowWave as u8, 50)
        .await
        .unwrap_err();
    assert!(err.contains("does not support"), "{err}");
    assert!(matches!(
        manager.device(id).await.unwrap().status,
        DeviceStatus::Error(_)
    ));

    let mut saw_error = false;
    while let Ok(ev) = events.try_recv() {
        saw_error |= matches!(ev, ManagerEvent::DeviceError { .. });
    }
    assert!(saw_error);

    // A later success clears the error.
    manager.set_color(id, Rgb::WHITE).await.unwrap();
    assert_eq!(
        manager.device(id).await.unwrap().status,
        DeviceStatus::Ready
    );
}

#[tokio::test]
async fn unknown_device_and_bad_effect_are_rejected() {
    let manager = virtual_manager();
    assert!(manager.set_color("nope", Rgb::WHITE).await.is_err());
    manager.scan().await.unwrap();
    assert!(manager.set_effect("virtual:g-mouse", 99, 0).await.is_err());
}

/// Controller that counts writes and sleeps, to observe coalescing.
struct SlowCounter {
    info: DeviceInfo,
    writes: Arc<AtomicUsize>,
    last: Arc<std::sync::Mutex<Rgb>>,
}

#[async_trait]
impl RgbController for SlowCounter {
    async fn initialize(&mut self) -> RgbResult<()> {
        Ok(())
    }
    async fn set_color(&mut self, r: u8, g: u8, b: u8) -> RgbResult<()> {
        tokio::time::sleep(Duration::from_millis(20)).await;
        self.writes.fetch_add(1, Ordering::SeqCst);
        *self.last.lock().unwrap() = Rgb::new(r, g, b);
        Ok(())
    }
    async fn set_effect(&mut self, _: u8, _: u8) -> RgbResult<()> {
        Ok(())
    }
    async fn get_device_info(&self) -> DeviceInfo {
        self.info.clone()
    }
}

fn info(id: &str) -> DeviceInfo {
    DeviceInfo {
        id: id.into(),
        name: id.into(),
        vendor: "Test".into(),
        device_type: DeviceType::Unknown,
        transport: rgb_core::Transport::Virtual,
        location: "test".into(),
        serial: None,
        firmware: None,
        zones: vec![],
        effects: vec![EffectId::Static.info()],
    }
}

#[tokio::test]
async fn color_bursts_are_coalesced_and_last_value_wins() {
    let manager = DeviceManager::new(vec![], fast_config());
    let writes = Arc::new(AtomicUsize::new(0));
    let last = Arc::new(std::sync::Mutex::new(Rgb::BLACK));
    manager
        .register(Box::new(SlowCounter {
            info: info("slow"),
            writes: writes.clone(),
            last: last.clone(),
        }))
        .await
        .unwrap();

    let calls = (0..50u8).map(|i| {
        let m = manager.clone();
        async move { m.set_color("slow", Rgb::new(i, 0, 0)).await }
    });
    // Fire sequentially-enqueued but concurrently-awaited updates.
    let results = futures::future::join_all(calls).await;
    assert!(results.iter().all(Result::is_ok));

    let n = writes.load(Ordering::SeqCst);
    assert!(n < 50, "expected coalescing, got {n} hardware writes");
    assert_eq!(*last.lock().unwrap(), Rgb::new(49, 0, 0));
}

struct Hangs(DeviceInfo);

#[async_trait]
impl RgbController for Hangs {
    async fn initialize(&mut self) -> RgbResult<()> {
        Ok(())
    }
    async fn set_color(&mut self, _: u8, _: u8, _: u8) -> RgbResult<()> {
        futures::future::pending().await
    }
    async fn set_effect(&mut self, _: u8, _: u8) -> RgbResult<()> {
        Ok(())
    }
    async fn get_device_info(&self) -> DeviceInfo {
        self.0.clone()
    }
}

#[tokio::test]
async fn hung_device_times_out_instead_of_blocking() {
    let manager = DeviceManager::new(vec![], fast_config());
    manager
        .register(Box::new(Hangs(info("hang"))))
        .await
        .unwrap();
    let err = manager.set_color("hang", Rgb::WHITE).await.unwrap_err();
    assert!(err.contains("did not respond"), "{err}");
    // Worker is still alive and accepts further commands.
    manager
        .set_effect("hang", EffectId::Static as u8, 0)
        .await
        .unwrap();
}

struct FailingInit;

#[async_trait]
impl RgbController for FailingInit {
    async fn initialize(&mut self) -> RgbResult<()> {
        Err("permission denied".into())
    }
    async fn set_color(&mut self, _: u8, _: u8, _: u8) -> RgbResult<()> {
        unreachable!()
    }
    async fn set_effect(&mut self, _: u8, _: u8) -> RgbResult<()> {
        unreachable!()
    }
    async fn get_device_info(&self) -> DeviceInfo {
        info("failing")
    }
}

struct FailingDetector;

#[async_trait]
impl DeviceDetector for FailingDetector {
    fn name(&self) -> &'static str {
        "failing"
    }
    async fn detect(&self, _: &HashSet<String>) -> DetectionReport {
        let mut r = DetectionReport::default();
        r.warn("adapter missing");
        r.controllers.push(Box::new(FailingInit));
        r
    }
}

struct StuckDetector;

#[async_trait]
impl DeviceDetector for StuckDetector {
    fn name(&self) -> &'static str {
        "stuck"
    }
    async fn detect(&self, _: &HashSet<String>) -> DetectionReport {
        futures::future::pending().await
    }
}

#[tokio::test]
async fn detector_failures_become_warnings_not_errors() {
    let virtual_detector = VirtualDetector::new(Arc::new(AtomicBool::new(true)));
    let manager = DeviceManager::new(
        vec![
            Arc::new(FailingDetector),
            Arc::new(StuckDetector),
            Arc::new(virtual_detector),
        ],
        fast_config(),
    );
    let summary = manager.scan().await.unwrap();
    assert_eq!(summary.added, VirtualController::demo_set().len());
    let joined = summary.warnings.join("\n");
    assert!(joined.contains("adapter missing"), "{joined}");
    assert!(
        joined.contains("initialisation failed: permission denied"),
        "{joined}"
    );
    assert!(joined.contains("stuck: detection timed out"), "{joined}");
}

#[tokio::test]
async fn remove_and_shutdown() {
    let manager = virtual_manager();
    manager.scan().await.unwrap();
    manager.remove("virtual:ble-strip").await.unwrap();
    assert!(manager.device("virtual:ble-strip").await.is_none());
    assert!(manager.remove("virtual:ble-strip").await.is_err());

    manager
        .remove_where(|d| d.info.device_type == DeviceType::Mouse)
        .await;
    assert!(manager.device("virtual:g-mouse").await.is_none());

    manager.shutdown_all().await;
    assert!(manager.devices().await.is_empty());
}

#[tokio::test]
async fn events_serialize_as_tagged_json() {
    let manager = virtual_manager();
    let mut events = manager.subscribe();
    manager.scan().await.unwrap();
    let first = events.recv().await.unwrap();
    assert_eq!(
        serde_json::to_value(&first).unwrap()["type"],
        "scan_started"
    );
    let added = events.recv().await.unwrap();
    let json = serde_json::to_value(&added).unwrap();
    assert_eq!(json["type"], "device_added");
    assert!(json["device"]["id"]
        .as_str()
        .unwrap()
        .starts_with("virtual:"));
    assert_eq!(json["device"]["status"]["state"], "ready");
}
