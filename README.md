# RGB Visual Enhancer

A lightweight, cross-platform (Windows / Linux) desktop app for controlling RGB
lighting across vendors from one place, without OEM suites like Armoury Crate
or G HUB. Built with **Rust + Tokio**, **Tauri v2**, and **React + Tailwind**.

| Family | Transport | Status |
|---|---|---|
| ASUS Aura motherboards, USB controller (AMD 500-series, Intel 400-series and newer, e.g. ROG STRIX B550-F) | USB HID (`0B05:18F3/1939/19AF/1AA6/1BED`) | Windows and Linux, no extra drivers. Onboard LEDs, 12 V RGB and 5 V ARGB headers. |
| ASUS Aura motherboards, ENE controller (older boards) | SMBus / I²C | Linux via `i2c-dev`. Windows requires a Ring0 driver (not bundled yet, see below). |
| Logitech G keyboards, mice, speakers, headsets | USB HID (HID++ 2.0, features `0x8070` / `0x8071`, usage pages `0xFF00` / `0xFF43`) | Wired devices and LIGHTSPEED/Unifying receivers. Confirmed on a G502 (`C332`). |
| MSI GeForce graphics cards (RTX 4060 GAMING X) | GPU I²C @ `0x68` via NVIDIA NvAPI | Windows only. Restricted to exact PCI ids; never writes the save/flash register. |
| BLE LED strips and lamps | Bluetooth LE GATT (`btleplug`) | RGBVE (DIY ESP32, [firmware included](firmware/esp32-ble-rgb)), ELK-BLEDOM, Triones. |
| Virtual devices | — | Simulated hardware for development and demos. |

> **Status:** the architecture, protocols, UI, and test suite are complete.
> The drivers follow publicly documented protocols (Solaar, OpenRGB). The
> Logitech HID++ path is confirmed on a G502; the other drivers have not yet
> been validated on physical devices. Close Logitech G HUB, Armoury Crate, and
> OpenRGB while using the app, otherwise they keep overriding the lighting.

## Quick start

Prerequisites: Rust (stable), Node 22+, and the
[Tauri v2 system dependencies](https://v2.tauri.app/start/prerequisites/).
Linux also needs `libudev-dev` and `libdbus-1-dev`.

```bash
npm install
```

```bash
npm run tauri dev
```

To try the app without hardware, enable **Virtual devices** in the header, or
start with `RGBVE_VIRTUAL_DEVICES=1 npm run tauri dev`.

To work on the UI alone, run `npm run dev` and open http://localhost:1420. In a
plain browser the app uses an in-memory simulator instead of the Rust backend.

Release build for the current OS (installers land in `target/release/bundle/`):

```bash
npm run tauri build
```

### Installers for x86 / x64

**On Windows (native):** install
[Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
with the *Desktop development with C++* workload, plus Rust and Node 22+.
Then:

```powershell
npm install
```

```powershell
npm run tauri build
```

The 64-bit installers are written to `target\release\bundle\nsis\` (`.exe`)
and `target\release\bundle\msi\` (`.msi`). For a 32-bit (x86) build:

```powershell
rustup target add i686-pc-windows-msvc
```

```powershell
npm run tauri build -- --target i686-pc-windows-msvc
```

The output goes to `target\i686-pc-windows-msvc\release\bundle\`.

**GitHub Actions:** push a `v*` tag or run the *Release* workflow manually. It
builds Windows x64, Windows x86 (32-bit), and Linux x86_64 installers and
attaches them to a draft release.

**Cross-compiling Windows installers from a Linux host:** use MinGW-w64 and
NSIS (`apt install gcc-mingw-w64-x86-64 gcc-mingw-w64-i686 nsis`). Don't use
these `-gnu` targets on Windows itself. Tauri treats cross-compilation as
experimental.

```bash
rustup target add x86_64-pc-windows-gnu i686-pc-windows-gnu
```

```bash
npm run tauri build -- --target x86_64-pc-windows-gnu --bundles nsis
```

```bash
npm run tauri build -- --target i686-pc-windows-gnu --bundles nsis
```

Unsigned installers trigger a Windows SmartScreen warning ("More info → Run
anyway"). The installer downloads the WebView2 runtime if it's missing; it
ships with Windows 10 and 11.

## Hardware access

### Linux

```bash
sudo cp packaging/linux/60-rgb-visual-enhancer.rules /etc/udev/rules.d/
```

```bash
sudo udevadm control --reload-rules && sudo udevadm trigger
```

The `.deb` package installs this rule automatically.

- **Logitech:** the rule grants the logged-in user access to Logitech `hidraw` nodes.
- **ASUS Aura (SMBus):** load `i2c-dev` and add yourself to the `i2c` group:

  ```bash
  echo i2c-dev | sudo tee /etc/modules-load.d/i2c-dev.conf && sudo modprobe i2c-dev
  ```

  ```bash
  sudo groupadd -f i2c && sudo usermod -aG i2c "$USER"
  ```

  Some AMD boards also need the kernel parameter `acpi_enforce_resources=lax`
  before the PIIX4 SMBus driver binds.
- **BLE:** requires BlueZ and a running D-Bus system bus.

### Windows

USB HID and BLE work without extra drivers. The chipset SMBus can only be
reached from kernel mode, so ASUS Aura over SMBus needs a signed port-I/O
driver such as PawnIO. The `SmbusBus` trait
(`crates/rgb-core/src/drivers/asus_aura/smbus.rs`) is the integration point.
Until a backend is added, the app reports this in the activity log rather than
failing.

### SMBus safety

The SMBus is shared with RAM SPD EEPROMs, sensors, and VRM controllers. The
Aura driver only probes the documented Aura addresses (`0x40`, `0x4E`, `0x4F`),
only on buses whose PCI subsystem vendor is ASUS (`0x1043`) or on boards whose
DMI vendor is ASUS. It also only talks to a chip after it passes the ENE
fingerprint check. DRAM modules, which need address remapping, are deliberately
left alone.

## Project layout

```
crates/rgb-core/          Hardware abstraction (no Tauri dependency)
  src/controller.rs         RgbController + DeviceDetector traits
  src/manager.rs            DeviceManager: per-device actor tasks, events, timeouts
  src/drivers/logitech/     HID++ 2.0 framing (pure) + hidapi transport
  src/drivers/asus_aura/    ENE register protocol (pure) + SMBus backends
  src/drivers/ble/          GATT profiles (pure) + btleplug transport
  src/drivers/virtual_device.rs
  tests/manager.rs          Concurrency, coalescing, timeout, and error-path tests
src-tauri/                Tauri shell: IPC commands + event forwarding (+ IPC contract test)
src/                      React UI (lib/backend.ts is the IPC boundary)
firmware/esp32-ble-rgb/   Arduino firmware for the open RGBVE BLE protocol
packaging/linux/          udev rules
docs/ARCHITECTURE.md      Design notes
```

## Development

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
npm run build
```

`cargo test` for `src-tauri` embeds the built frontend, so run `npm run build`
first. The Rust tests never touch real hardware: the manager tests use
simulated controllers, protocol tests check exact byte layouts, and the Aura
tests run against an in-memory ENE register model.

### Adding a driver

1. Implement `RgbController` for the device. Do blocking I/O inside
   `tokio::task::spawn_blocking`, and advertise only the `EffectId`s the
   device really supports.
2. Implement `DeviceDetector`. Return uninitialised controllers with stable
   ids, skip ids in `known`, and turn recoverable problems into warnings.
3. Register it in `rgb_core::default_detectors`, behind a cargo feature if it
   pulls in new system dependencies.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the threading model.

## License

MIT
