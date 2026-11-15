# RGBVE ESP32 BLE controller

Firmware for a DIY Bluetooth LED strip controller that RGB Visual Enhancer
detects automatically (it advertises the RGBVE service UUID and an
`RGBVE-XXXX` name).

## Hardware

| ESP32 | Strip (WS2812B / NeoPixel) |
|-------|----------------------------|
| GPIO 5 (`LED_PIN`) → 330 Ω → | DIN |
| GND | GND (shared with the PSU) |
| — | 5 V from an external supply sized for the strip (≈60 mA/LED at full white) |

A 74AHCT125 level shifter on DIN is recommended for long strips.

## Build

1. Arduino IDE 2.x with the **esp32** board package (Espressif, 3.x).
2. Install libraries: **NimBLE-Arduino** (≥ 2.0) and **Adafruit NeoPixel**.
3. Adjust `LED_PIN`, `LED_COUNT` and `MAX_BRIGHTNESS` at the top of the sketch.
4. Flash, then hit **Rescan hardware** in the app.

## Protocol

Write to characteristic `8d2c0002-8b5e-4f3a-9c1d-5e6f7a8b9c0d`
(service `8d2c0001-…`):

| Op | Payload | Meaning |
|----|---------|---------|
| `0x01` | `R G B` | Set colour. Colour-based effects keep running; others fall back to Static. |
| `0x02` | `EFFECT SPEED R G B` | Set effect (ids as `rgb_core::EffectId`), speed 0–100. |
| `0x03` | `ON` | Power on (1) / off (0). |

Any BLE device implementing this table works with the app; it isn't tied to
the ESP32.
