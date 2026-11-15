# Architecture

## Goals

- The UI never blocks on hardware.
- A slow or broken device never affects the others.
- Adding a vendor means writing one trait implementation and nothing else.
- Everything except the thin I/O layer can be tested without hardware.

## Layers

```
┌──────────────── React (src/) ────────────────┐
│ useRgb store ◄── "rgb://event" ── listen()   │
│      │ invoke() (latest-wins per device)     │
└──────┼───────────────────────────────────────┘
       ▼
┌──────────── Tauri shell (src-tauri/) ────────┐
│ async #[tauri::command]s → DeviceManager     │
│ broadcast<ManagerEvent> → app.emit(...)      │
└──────┼───────────────────────────────────────┘
       ▼
┌──────────── rgb-core (crates/rgb-core/) ─────┐
│ DeviceManager                                │
│  ├─ scan(): detectors run concurrently       │
│  └─ one worker task per device (actor)       │
│        mpsc<Command> → Box<dyn RgbController>│
│                                              │
│ Drivers: pure protocol module + I/O module   │
└──────────────────────────────────────────────┘
```

## The `RgbController` trait

```rust
#[async_trait]
pub trait RgbController: Send + Sync {
    async fn initialize(&mut self) -> Result<(), String>;
    async fn set_color(&mut self, red: u8, green: u8, blue: u8) -> Result<(), String>;
    async fn set_effect(&mut self, effect_id: u8, speed: u8) -> Result<(), String>;
    async fn get_device_info(&self) -> DeviceInfo;
    // Provided: set_zone_color (falls back to set_color), shutdown (no-op)
}
```

- `effect_id` is a vendor-neutral `EffectId` (Off, Static, Breathing, Spectrum
  Cycle, Rainbow Wave, Color Pulse, Flashing). Each driver maps it to its own
  mode numbers and lists only the effects it supports in `DeviceInfo.effects`.
  The UI only offers those.
- `speed` is a percentage (0 = slowest, 100 = fastest). Drivers scale it to
  periods, delays, or register values.
- `set_color` keeps a colour-based effect (Breathing, Pulse, Flashing) running
  with the new colour and otherwise switches to Static. Every driver, the
  manager's snapshot, the browser simulator, and the ESP32 firmware all
  follow this rule.

## Concurrency model

**Per-device actors.** `DeviceManager::register` moves each controller into its
own Tokio task, which owns it exclusively (hence `&mut self` in the trait).
Callers send `Command`s over a bounded `mpsc` channel and await a `oneshot`
reply. This gives:

| Concern | Mechanism |
|---|---|
| Isolation | One task per device; a slow SMBus write never delays USB. |
| Ordering | One channel per device, so commands run in order. |
| Back-pressure | When a write finishes, queued `SetColor`s collapse into the latest one. Everyone waiting gets the result. A non-colour command is stashed and runs next, so ordering is kept. |
| Liveness | Every hardware call is wrapped in `tokio::time::timeout`. A hung device returns an error and its worker keeps serving. |
| Blocking I/O | hidapi and SMBus ioctls run in `spawn_blocking`. BLE is async through btleplug. |

**Scanning.** Detectors run concurrently, each in its own task with a timeout.
Devices are registered and announced as each detector finishes, so USB devices
appear before a 5-second BLE scan ends. A rescan passes the set of known ids so
detectors skip hardware that's already open. Only one scan runs at a time, and
a drop guard resets the flag even if the future is cancelled.

**Events.** The manager publishes `ManagerEvent`s on a `broadcast` channel. The
Tauri layer forwards them on `rgb://event`. If the forwarder lags, it emits
`rgb://resync` and the UI refetches the full device list.

**Frontend rate limiting.** `createLatestSender` allows one in-flight request
per device and always sends the most recent value. Combined with backend
coalescing, dragging the colour picker stays smooth on slow buses, and the
last colour always wins.

## Drivers

Each driver separates a **pure protocol module** (exact byte layouts,
unit-tested) from an **I/O module** (hidapi / SMBus / btleplug).

### Logitech HID++ 2.0 (`drivers/logitech`)
- Interface selection: vendor usage page `0xFF00`. On Windows, the long-report
  (`0x11`) collection.
- Device addressing: index `0xFF` for wired devices. If that ping fails, the
  driver treats the interface as a receiver and probes slots 1–6. One HID
  handle is shared, and a mutex serialises request/response pairs.
- Feature discovery through `IRoot.getFeature`: `0x0005` for name and type,
  `0x8070`/`0x8071` for zones and effect lists, `0x8100` for host mode so
  onboard profiles don't override the app.
- The device addresses effects by **index** into each zone's effect list, not
  by effect type. The driver maps type → index per zone.
- Writes are RAM-only (no persistence flag on `0x8070`) to avoid flash wear.

### ASUS Aura over SMBus (`drivers/asus_aura`)
- ENE indirect register access: word write of the byte-swapped register to
  `0x00`, then byte read `0x81`, byte write `0x01`, or block write `0x03`.
- Colours are stored as R, B, G. The register layout (V1/V2) is picked from
  the device-name string. LED count and channel names come from the 64-byte
  config table.
- See the README for the probing safety rules.

### BLE (`drivers/ble`)
- Profiles are identified by advertised service UUID, falling back to name.
- The controller connects lazily, reconnects transparently on write, and
  tracks power state so it doesn't resend power frames.

## Testing

| Layer | How |
|---|---|
| Protocols | Byte-exact unit tests (HID++ framing and offsets, ENE layout, BLE frames). |
| Aura driver | Runs against `FakeEne`, an in-memory register model. |
| Manager | `tests/manager.rs`: scan idempotency, concurrent scan rejection, coalescing, timeouts, detector failures, removal. |
| IPC | `src-tauri` `ipc_tests`: drives every command through Tauri's mock runtime with the real ACL and the same JSON the frontend sends. |
| UI | `npm run dev` runs the full UI against the in-browser simulator. |
