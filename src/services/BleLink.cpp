#include "services/BleLink.h"
#include "config.h"
#include "services/SetupService.h"

#include <Arduino.h>
#include <NimBLEDevice.h>
#include <freertos/FreeRTOS.h>
#include <freertos/stream_buffer.h>
#include <string.h>

namespace BleLink {

namespace {

constexpr size_t kRxBufferBytes = 2048;
constexpr size_t kLineMax = 1024;

NimBLEServer *s_server = nullptr;
NimBLECharacteristic *s_tx = nullptr;
StreamBufferHandle_t s_rx = nullptr;
volatile bool s_connected = false;
volatile uint16_t s_connHandle = 0;
volatile uint16_t s_mtu = 23;

// Line assembly happens in popLine() (WiFi task); the NimBLE host task only
// copies bytes into the stream buffer.
char s_line[kLineMax] = {0};
size_t s_lineLen = 0;

class ServerCallbacks : public NimBLEServerCallbacks {
  void onConnect(NimBLEServer * /*server*/, ble_gap_conn_desc *desc) override {
    s_connHandle = desc->conn_handle;
    s_mtu = 23;
    s_connected = true;
    Serial.println("[BleLink] Companion connected");
  }
  void onDisconnect(NimBLEServer * /*server*/,
                    ble_gap_conn_desc * /*desc*/) override {
    s_connected = false;
    Serial.println("[BleLink] Companion disconnected");
    SetupService::onBleDisconnected();
  }
  void onMTUChange(uint16_t mtu, ble_gap_conn_desc * /*desc*/) override {
    s_mtu = mtu;
  }
};

class RxCallbacks : public NimBLECharacteristicCallbacks {
  void onWrite(NimBLECharacteristic *chr) override {
    const std::string &v = chr->getValue();
    if (s_rx != nullptr && !v.empty()) {
      xStreamBufferSend(s_rx, v.data(), v.size(), 0);
    }
  }
};

ServerCallbacks s_serverCallbacks;
RxCallbacks s_rxCallbacks;

} // namespace

void start(const char *name) {
  if (NimBLEDevice::getInitialized())
    return;
  if (s_rx == nullptr) {
    s_rx = xStreamBufferCreate(kRxBufferBytes, 1);
  }
  xStreamBufferReset(s_rx);
  s_lineLen = 0;

  NimBLEDevice::init(name);
  NimBLEDevice::setMTU(247);

  s_server = NimBLEDevice::createServer();
  s_server->setCallbacks(&s_serverCallbacks, false);
  s_server->advertiseOnDisconnect(true);

  NimBLEService *svc = s_server->createService(Config::BLE_SERVICE_UUID);
  NimBLECharacteristic *rx = svc->createCharacteristic(
      Config::BLE_RX_UUID, NIMBLE_PROPERTY::WRITE | NIMBLE_PROPERTY::WRITE_NR);
  rx->setCallbacks(&s_rxCallbacks);
  s_tx = svc->createCharacteristic(Config::BLE_TX_UUID, NIMBLE_PROPERTY::NOTIFY);
  svc->start();

  // The 128-bit UUID fills most of the advertisement; the name goes in the
  // scan response.
  NimBLEAdvertising *adv = NimBLEDevice::getAdvertising();
  adv->addServiceUUID(Config::BLE_SERVICE_UUID);
  adv->setScanResponse(true);
  adv->start();
  Serial.printf("[BleLink] Advertising as '%s'\n", name);
}

void stop() {
  if (!NimBLEDevice::getInitialized())
    return;
  s_connected = false;
  s_tx = nullptr;
  s_server = nullptr;
  NimBLEDevice::deinit(true);
}

bool isRunning() { return NimBLEDevice::getInitialized(); }

bool isConnected() { return s_connected; }

void send(const char *jsonLine) {
  if (!s_connected || s_tx == nullptr || jsonLine == nullptr)
    return;
  const size_t chunk = (s_mtu > 3) ? (s_mtu - 3) : 20;
  const size_t len = strlen(jsonLine);
  // Payload plus the trailing newline, in MTU-sized notifications.
  for (size_t off = 0; off <= len; off += chunk) {
    size_t n = len + 1 - off;
    if (n > chunk)
      n = chunk;
    uint8_t buf[256];
    if (n > sizeof(buf))
      n = sizeof(buf);
    for (size_t i = 0; i < n; ++i) {
      const size_t at = off + i;
      buf[i] = (at < len) ? static_cast<uint8_t>(jsonLine[at]) : '\n';
    }
    s_tx->notify(buf, n);
    vTaskDelay(pdMS_TO_TICKS(4)); // pace notifications so none are dropped
  }
}

bool popLine(char *out, size_t outLen) {
  if (s_rx == nullptr)
    return false;
  uint8_t c;
  while (xStreamBufferReceive(s_rx, &c, 1, 0) == 1) {
    if (c == '\n' || c == '\r') {
      if (s_lineLen == 0)
        continue;
      const size_t n = (s_lineLen < outLen - 1) ? s_lineLen : outLen - 1;
      memcpy(out, s_line, n);
      out[n] = '\0';
      s_lineLen = 0;
      return true;
    }
    if (s_lineLen + 1 < sizeof(s_line)) {
      s_line[s_lineLen++] = static_cast<char>(c);
    } else {
      s_lineLen = 0; // overlong line: drop it
    }
  }
  return false;
}

} // namespace BleLink
