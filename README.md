<p align="center">
  <img src="docs/images/banner.svg" alt="RGB Visual Enhancer – jednotné ovládanie RGB osvetlenia" width="100%">
</p>

<p align="center">
  <img src="docs/images/icon.png" alt="" width="72"><br>
  <b>Jedna ľahká aplikácia pre všetky RGB svetlá v počítači.</b><br>
  Žiadny Armoury Crate, žiadny G HUB, žiadne bloatware.
</p>

<p align="center">
  <img alt="Verzia 0.2.0" src="https://img.shields.io/badge/verzia-0.2.0-7c5cff?style=for-the-badge">
  <img alt="Windows | Linux" src="https://img.shields.io/badge/Windows%20%7C%20Linux-x64%20%C2%B7%20x86-22d3ee?style=for-the-badge&logo=windows11&logoColor=white">
  <img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-24c8db?style=for-the-badge&logo=tauri&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-Tokio-f74c00?style=for-the-badge&logo=rust&logoColor=white">
  <img alt="React 19" src="https://img.shields.io/badge/React-19-61dafb?style=for-the-badge&logo=react&logoColor=black">
  <img alt="Licencia MIT" src="https://img.shields.io/badge/licencia-MIT-00ff6a?style=for-the-badge">
</p>

<p align="center">
  <b>🇸🇰 Slovensky</b> · <a href="docs/README.en.md">🇬🇧 English</a>
  <br><br>
  <a href="#-ukážky">Ukážky</a> ·
  <a href="#-podporovaný-hardvér">Hardvér</a> ·
  <a href="#-inštalácia">Inštalácia</a> ·
  <a href="#-ovládanie">Ovládanie</a> ·
  <a href="#-ako-to-funguje">Architektúra</a> ·
  <a href="#-pre-vývojárov">Vývoj</a>
</p>

---

## ✨ Čo aplikácia vie

<table>
  <tr>
    <td width="50%" valign="top">
      <h3>🎨 Farby a efekty</h3>
      Výber farby, predvoľby, statická farba, dýchanie, blikanie, cyklus spektra aj dúhová vlna. Farbu môžete nastaviť pre celé zariadenie, pre jednu zónu alebo <b>synchronizovať na všetky zariadenia naraz</b>.
    </td>
    <td width="50%" valign="top">
      <h3>🔋 Stav zariadení</h3>
      Batéria bezdrôtových Logitech zariadení (aj slúchadiel G733), typ spojenia, stav a aktuálne svetlo. Údaje sa obnovujú automaticky.
    </td>
  </tr>
  <tr>
    <td valign="top">
      <h3>🧭 Systémová lišta</h3>
      Ako G HUB: zatvorenie okna nechá aplikáciu bežať v lište. Menu v lište má vizuál aplikácie a ponúka rýchle farby pre každé zariadenie. Voliteľne sa aplikácia spúšťa so systémom.
    </td>
    <td valign="top">
      <h3>⚡ Rýchla a bezpečná</h3>
      Rust jadro, každé zariadenie má vlastnú úlohu, UI nikdy nečaká na hardvér. Zápisy na zbernice (SMBus, I²C grafickej karty) sú obmedzené na presne overené adresy a registre.
    </td>
  </tr>
  <tr>
    <td valign="top">
      <h3>🖱️ Desktopové ovládanie</h3>
      Kontextové menu pravým tlačidlom tam, kde dáva zmysel (zariadenie, zóna, farba, log). Žiadne prehliadačové menu ani skratky.
    </td>
    <td valign="top">
      <h3>🌍 Slovenčina aj angličtina</h3>
      Predvolený jazyk je slovenčina, angličtinu zapnete v <b>Nastaveniach</b>. Inštalátory MSI aj EXE sú tiež v oboch jazykoch.
    </td>
  </tr>
</table>

## 📸 Ukážky

<p align="center">
  <img src="docs/images/app-sk.png" alt="Hlavné okno aplikácie so zoznamom zariadení, výberom farby a efektmi" width="100%">
</p>

<table>
  <tr>
    <th>Menu v systémovej lište</th>
    <th>Kontextové menu a nastavenia</th>
  </tr>
  <tr>
    <td align="center" valign="top" width="40%"><img src="docs/images/tray-menu.png" alt="Menu v lište so zariadeniami, batériou a rýchlymi farbami" width="320"></td>
    <td align="center" valign="top"><img src="docs/images/context-menu.png" alt="Kontextové menu s farbami a efektmi" width="100%"><br><br><img src="docs/images/settings.png" alt="Nastavenia s voľbou jazyka" width="100%"></td>
  </tr>
</table>

<details>
<summary><b>🇬🇧 Anglické rozhranie</b> (kliknite pre zobrazenie)</summary>
<br>
<img src="docs/images/app-en.png" alt="Main window in English" width="100%">
</details>

<sub>Snímky sú z ukážkového režimu s virtuálnymi zariadeniami.</sub>

## 💻 Podporovaný hardvér

| Zariadenie | Spojenie | Stav |
|---|---|---|
| **Logitech G** myši, klávesnice, reproduktory, slúchadlá (napr. G502, G910, G733) | USB / LIGHTSPEED (HID++ 2.0) | ✅ G502 overená · 🟢 G910 a G733 rozpoznané |
| **ASUS Aura** dosky s USB ovládačom (AMD 500, Intel 400 a novšie, napr. ROG STRIX B550-F) | USB HID | 🟢 Rozpoznané na reálnom PC |
| **MSI GeForce RTX 4060 GAMING X** | I²C cez NVIDIA NvAPI | 🟢 Rozpoznané na reálnom PC (len Windows) |
| **ASUS Aura** staršie dosky s ENE čipom | SMBus | 🧪 Experimentálne (Linux) |
| **Bluetooth LED pásiky**: ESP32 ([firmvér v repozitári](firmware/esp32-ble-rgb)), ELK-BLEDOM, Triones | Bluetooth LE | 🧪 Experimentálne |

> ✅ overené = farby a efekty potvrdené na skutočnom zariadení · 🟢 rozpoznané = aplikácia zariadenie na reálnom PC nájde a ovláda, potvrdenie všetkých efektov uvítame · 🧪 experimentálne = implementované podľa dokumentovaného protokolu, zatiaľ neodskúšané

<details>
<summary><b>🔍 Ako aplikácia rozpoznáva zariadenia</b></summary>

- **Logitech:** HID kanály `0xFF00` (myši, prijímače) a `0xFF43` (klávesnice G810/G910, slúchadlá). Funkcie zariadenia zisťuje cez HID++ (`0x8070`/`0x8071` svetlá, `0x1004`/`0x1000`/`0x1001`/`0x1F20` batéria). Ak zariadenie nepozná, vypíše do logu zoznam jeho funkcií.
- **ASUS Aura USB:** ovládač `0B05:18F3/1939/19AF/1AA6/1BED`; LED a konektory číta z jeho konfiguračnej tabuľky.
- **MSI GPU:** len karty s presnými PCI ID (`10DE:2882` + MSI `5160`/`5164`), adresa `0x68`, len dokumentované registre, pred prvým zápisom kontrolné čítanie, nikdy zápis do flash pamäte.
- **ASUS SMBus:** len adresy `0x40`, `0x4E`, `0x4F`, len na doskách ASUS a len po overení odtlačku ENE čipu. RAM sa nikdy nedotýka.

</details>

## 📦 Inštalácia

1. Stiahnite inštalátor pre Windows:
   - **MSI** (`…_x64_sk-SK.msi` alebo `…_x64_en-US.msi`): štandardný inštalátor pre Windows
   - **EXE** (`…_x64-setup.exe`): pri inštalácii si vyberiete jazyk
   - pre 32-bitový Windows sú verzie s `x86`

   Inštalátory vytvára workflow [**Windows installers**](https://github.com/bucala/RGB-Visual-Enhancer/actions/workflows/windows-installers.yml) (sekcia *Artifacts* pri poslednom behu).
2. Inštalátor nie je digitálne podpísaný, preto Windows SmartScreen zobrazí upozornenie: **Ďalšie informácie → Spustiť aj tak**.
3. **Zatvorte G HUB, Armoury Crate, MSI Center a OpenRGB.** Inak budú farby navzájom prepisovať.

<details>
<summary><b>🐧 Linux</b></summary>

Balík `.deb` nainštaluje aj udev pravidlá. Pri ručnej inštalácii:

```bash
sudo cp packaging/linux/60-rgb-visual-enhancer.rules /etc/udev/rules.d/
```

```bash
sudo udevadm control --reload-rules && sudo udevadm trigger
```

Pre ASUS cez SMBus načítajte `i2c-dev` a pridajte sa do skupiny `i2c`:

```bash
echo i2c-dev | sudo tee /etc/modules-load.d/i2c-dev.conf && sudo modprobe i2c-dev
```

```bash
sudo groupadd -f i2c && sudo usermod -aG i2c "$USER"
```

Niektoré AMD dosky potrebujú parameter jadra `acpi_enforce_resources=lax`. Bluetooth vyžaduje BlueZ a bežiacu systémovú D-Bus. Na Linuxe má lišta natívne menu, lebo AppIndicator nehlási kliknutia.

</details>

## 🎮 Ovládanie

| Kde | Akcia | Čo sa stane |
|---|---|---|
| Ikona v lište | <kbd>Ľavé tlačidlo</kbd> | Zobrazí alebo skryje hlavné okno |
| Ikona v lište | <kbd>Pravé tlačidlo</kbd> | Menu so zariadeniami, batériou a rýchlymi farbami |
| Zariadenie v zozname | <kbd>Pravé tlačidlo</kbd> | Farby, efekty, vypnutie svetiel, kopírovanie informácií |
| Zóna | <kbd>Pravé tlačidlo</kbd> | Nafarbenie zóny vybranou alebo predvolenou farbou |
| Predvolená farba | <kbd>Pravé tlačidlo</kbd> | Použiť na toto alebo na všetky zariadenia |
| Log aktivity | <kbd>Pravé tlačidlo</kbd> | Kopírovať správu alebo celý log, vymazať log |
| Okno | <kbd>✕</kbd> alebo minimalizovať | Aplikácia beží ďalej v lište |
| Menu v lište | <kbd>Esc</kbd> alebo klik mimo | Zavrie menu |

<details>
<summary><b>💡 Tipy</b></summary>

- **Synchronizovať všetky** v hlavičke pošle zmenu farby na všetky zariadenia naraz.
- Pri slúchadlách G733 sa batéria počíta z napätia, preto je pred číslom `≈`.
- Pri vypnutých slúchadlách sa batéria nezobrazí; po zapnutí sa objaví do minúty.
- **Nastavenia → Virtuálne zariadenia** pridá simulovaný hardvér na vyskúšanie.
- Aplikácia si zatiaľ nepamätá farby po reštarte PC.

</details>

## 🧩 Ako to funguje

```mermaid
flowchart LR
    subgraph UI["🖼️ React UI"]
        W[Hlavné okno]
        T[Menu v lište]
    end
    subgraph Shell["🦀 Tauri"]
        C[IPC príkazy]
        E[Udalosti]
        S[(settings.json)]
    end
    subgraph Core["⚙️ rgb-core"]
        M[DeviceManager]
        A1[[Logitech]]
        A2[[ASUS Aura]]
        A3[[MSI GPU]]
        A4[[Bluetooth]]
    end
    W & T -- invoke --> C --> M
    M --> A1 & A2 & A3 & A4
    M -- broadcast --> E -- emit --> W & T
    C <--> S
    A1 -- HID++ --> H1((USB))
    A2 -- reporty 0xEC --> H2((USB))
    A3 -- NvAPI I²C --> H3((GPU))
    A4 -- GATT --> H4((BLE))
```

Každé zariadenie obsluhuje vlastná úloha (actor). Príkazy pre jedno zariadenie idú v poradí. Rýchle zmeny farby (ťahanie výberu) sa spoja do jedného zápisu. Každá operácia má časový limit, takže zaseknuté zariadenie neblokuje ostatné. Podrobnosti sú v [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) (po anglicky).

## 🔧 Pre vývojárov

<details>
<summary><b>Spustenie zo zdrojového kódu</b></summary>

Potrebujete Rust (stable), Node 22+ a [systémové závislosti Tauri](https://v2.tauri.app/start/prerequisites/). Na Windows je to Visual Studio Build Tools s „Desktop development with C++“.

```bash
npm install
```

```bash
npm run tauri dev
```

Iba UI v prehliadači so simulátorom (bez Rustu a hardvéru):

```bash
npm run dev
```

</details>

<details>
<summary><b>Zostavenie inštalátorov</b></summary>

Na Windows (MSI aj EXE, 64-bit):

```powershell
npm run tauri build
```

32-bitová verzia:

```powershell
rustup target add i686-pc-windows-msvc
```

```powershell
npm run tauri build -- --target i686-pc-windows-msvc
```

Výsledky sú v `target\…\release\bundle\msi\` a `bundle\nsis\`. Na GitHube ich zostaví workflow *Windows installers* pri každom pushi. Workflow *Release* (tag `v*`) ich pripojí ku konceptu vydania.

Z Linuxu sa dá krížovo zostaviť len NSIS inštalátor (MinGW + NSIS); MSI vyžaduje Windows.

</details>

<details>
<summary><b>Testy a kontrola kódu</b></summary>

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
npm run build
```

Testy nikdy nepristupujú k skutočnému hardvéru: protokoly sa testujú na presné bajty, ASUS ovládač na simulovanom čipe, manažér na virtuálnych zariadeniach a IPC rozhranie cez mock runtime Tauri.

</details>

<details>
<summary><b>Pridanie nového ovládača</b></summary>

1. Implementujte `RgbController` (blokujúce I/O do `tokio::task::spawn_blocking`, ponúkajte len efekty, ktoré zariadenie naozaj vie).
2. Implementujte `DeviceDetector` (stabilné ID, preskočiť ID v `known`, problémy vracať ako varovania).
3. Zaregistrujte ho v `rgb_core::default_detectors`.

</details>

<details>
<summary><b>Štruktúra projektu</b></summary>

```
crates/rgb-core/          Rust jadro bez závislosti na UI
  src/manager.rs            DeviceManager: actor na zariadenie, udalosti, limity
  src/drivers/logitech/     HID++ 2.0 (svetlá, batéria)
  src/drivers/asus_aura*    ASUS Aura (USB, SMBus)
  src/drivers/msi_gpu/      MSI GPU cez NvAPI I²C
  src/drivers/ble/          Bluetooth LE profily
src-tauri/                Tauri: IPC, lišta, nastavenia, inštalátory
src/                      React UI (preklady, menu v lište, kontextové menu)
firmware/esp32-ble-rgb/   Firmvér pre vlastný ESP32 LED ovládač
docs/                     Architektúra, obrázky, anglické README
```

</details>

## 📄 Licencia

[MIT](LICENSE). Protokoly zariadení sú implementované podľa verejnej dokumentácie projektov [OpenRGB](https://gitlab.com/CalcProgrammer1/OpenRGB) a [Solaar](https://github.com/pwr-Solaar/Solaar).

<p align="center"><sub>Vytvorené s 🌈 pre všetkých, ktorí chcú RGB bez bloatwaru.</sub></p>
