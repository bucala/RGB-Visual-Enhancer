// RGBVE BLE LED controller for ESP32 + WS2812B/NeoPixel strips.
//
// Implements the open "RGBVE" GATT protocol spoken by RGB Visual Enhancer
// (see crates/rgb-core/src/drivers/ble/profiles.rs):
//
//   Service        8d2c0001-8b5e-4f3a-9c1d-5e6f7a8b9c0d
//   Control char   8d2c0002-8b5e-4f3a-9c1d-5e6f7a8b9c0d  (write / write-no-response)
//
//   0x01 R G B                 set colour (keeps a colour-based effect running)
//   0x02 EFFECT SPEED R G B    set effect; SPEED 0 (slow) – 100 (fast)
//   0x03 ON                    power (0 = off, 1 = on)
//
// Effect ids match rgb_core::EffectId:
//   0 Off, 1 Static, 2 Breathing, 3 Spectrum Cycle, 4 Rainbow Wave,
//   5 Color Pulse, 6 Flashing
//
// Libraries (Arduino Library Manager):
//   - NimBLE-Arduino      >= 2.0   (h2zero)
//   - Adafruit NeoPixel   >= 1.12

#include <Adafruit_NeoPixel.h>
#include <NimBLEDevice.h>

// ---- Hardware configuration -------------------------------------------------
#ifndef LED_PIN
#define LED_PIN 5
#endif
#ifndef LED_COUNT
#define LED_COUNT 60
#endif
#ifndef MAX_BRIGHTNESS
#define MAX_BRIGHTNESS 160  // cap current draw; 255 = full
#endif

static const char *SERVICE_UUID = "8d2c0001-8b5e-4f3a-9c1d-5e6f7a8b9c0d";
static const char *CONTROL_UUID = "8d2c0002-8b5e-4f3a-9c1d-5e6f7a8b9c0d";

enum Effect : uint8_t {
  EFFECT_OFF = 0,
  EFFECT_STATIC = 1,
  EFFECT_BREATHING = 2,
  EFFECT_SPECTRUM_CYCLE = 3,
  EFFECT_RAINBOW_WAVE = 4,
  EFFECT_COLOR_PULSE = 5,
  EFFECT_FLASHING = 6,
};

struct LightState {
  uint8_t r = 124, g = 92, b = 255;
  uint8_t effect = EFFECT_STATIC;
  uint8_t speed = 50;
  bool on = true;
  uint32_t epoch = 0;  // millis() when the effect last changed
};

// Written from the NimBLE host task, read from loop(); guarded by a spinlock.
static LightState shared;
static portMUX_TYPE stateLock = portMUX_INITIALIZER_UNLOCKED;

Adafruit_NeoPixel strip(LED_COUNT, LED_PIN, NEO_GRB + NEO_KHZ800);

// ---- BLE --------------------------------------------------------------------
class ControlCallbacks : public NimBLECharacteristicCallbacks {
  void onWrite(NimBLECharacteristic *ch, NimBLEConnInfo &) override {
    NimBLEAttValue v = ch->getValue();
    const uint8_t *d = v.data();
    size_t n = v.length();
    if (n == 0) return;

    portENTER_CRITICAL(&stateLock);
    switch (d[0]) {
      case 0x01:  // SET_COLOR
        if (n >= 4) {
          shared.r = d[1]; shared.g = d[2]; shared.b = d[3];
          shared.on = true;
          // Same rule as the desktop app: colour-less effects fall back to Static.
          if (shared.effect == EFFECT_OFF || shared.effect == EFFECT_SPECTRUM_CYCLE ||
              shared.effect == EFFECT_RAINBOW_WAVE) {
            shared.effect = EFFECT_STATIC;
          }
        }
        break;
      case 0x02:  // SET_EFFECT
        if (n >= 3 && d[1] <= EFFECT_FLASHING) {
          shared.effect = d[1];
          shared.speed = d[2] > 100 ? 100 : d[2];
          if (n >= 6) { shared.r = d[3]; shared.g = d[4]; shared.b = d[5]; }
          shared.on = d[1] != EFFECT_OFF;
          shared.epoch = millis();
        }
        break;
      case 0x03:  // POWER
        if (n >= 2) shared.on = d[1] != 0;
        break;
    }
    portEXIT_CRITICAL(&stateLock);
  }
};

static void setupBle() {
  char name[20];
  uint64_t mac = ESP.getEfuseMac();
  snprintf(name, sizeof(name), "RGBVE-%04X", (uint16_t)(mac >> 32));

  NimBLEDevice::init(name);
  NimBLEServer *server = NimBLEDevice::createServer();
  server->advertiseOnDisconnect(true);

  NimBLEService *service = server->createService(SERVICE_UUID);
  NimBLECharacteristic *control = service->createCharacteristic(
      CONTROL_UUID, NIMBLE_PROPERTY::WRITE | NIMBLE_PROPERTY::WRITE_NR);
  control->setCallbacks(new ControlCallbacks());
  service->start();

  // 128-bit UUID in the advertisement, name in the scan response
  // (both don't fit in 31 bytes). The app matches on either.
  NimBLEAdvertising *adv = NimBLEDevice::getAdvertising();
  adv->addServiceUUID(SERVICE_UUID);
  adv->enableScanResponse(true);
  adv->setName(name);
  adv->start();
}

// ---- Rendering --------------------------------------------------------------
static uint32_t scaled(uint8_t r, uint8_t g, uint8_t b, float k) {
  return strip.Color((uint8_t)(r * k), (uint8_t)(g * k), (uint8_t)(b * k));
}

// Speed 0..100 -> animation period in ms (slow 8 s … fast 0.5 s).
static uint32_t periodMs(uint8_t speed) { return 8000 - (uint32_t)speed * 75; }

static void render(const LightState &s, uint32_t now) {
  if (!s.on || s.effect == EFFECT_OFF) {
    strip.clear();
    return;
  }
  uint32_t period = periodMs(s.speed);
  float phase = (float)((now - s.epoch) % period) / (float)period;  // 0..1

  switch (s.effect) {
    case EFFECT_STATIC:
      strip.fill(strip.Color(s.r, s.g, s.b));
      break;
    case EFFECT_BREATHING: {
      float k = 0.5f - 0.5f * cosf(phase * 2.0f * PI);
      strip.fill(scaled(s.r, s.g, s.b, 0.05f + 0.95f * k));
      break;
    }
    case EFFECT_COLOR_PULSE: {
      // Quick attack, slow exponential decay.
      float k = expf(-5.0f * phase);
      strip.fill(scaled(s.r, s.g, s.b, k));
      break;
    }
    case EFFECT_FLASHING:
      strip.fill(phase < 0.5f ? strip.Color(s.r, s.g, s.b) : 0);
      break;
    case EFFECT_SPECTRUM_CYCLE:
      strip.fill(strip.gamma32(strip.ColorHSV((uint16_t)(phase * 65535.0f))));
      break;
    case EFFECT_RAINBOW_WAVE:
      for (uint16_t i = 0; i < LED_COUNT; i++) {
        uint16_t hue = (uint16_t)(phase * 65535.0f) + (uint32_t)i * 65536 / LED_COUNT;
        strip.setPixelColor(i, strip.gamma32(strip.ColorHSV(hue)));
      }
      break;
  }
}

void setup() {
  strip.begin();
  strip.setBrightness(MAX_BRIGHTNESS);
  strip.show();
  shared.epoch = millis();
  setupBle();
}

void loop() {
  static uint32_t lastFrame = 0;
  uint32_t now = millis();
  if (now - lastFrame < 16) {  // ~60 FPS
    delay(1);
    return;
  }
  lastFrame = now;

  LightState s;
  portENTER_CRITICAL(&stateLock);
  s = shared;
  portEXIT_CRITICAL(&stateLock);

  render(s, now);
  strip.show();
}
