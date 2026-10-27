#include "services/FirmwareHealth.h"

#include <Arduino.h>
#include <esp_ota_ops.h>
#include <esp_timer.h>

// Arduino would confirm every new firmware as soon as it boots; we decide
// later (FirmwareHealth::begin).
extern "C" bool verifyRollbackLater() { return true; }

namespace FirmwareHealth {

namespace {

constexpr uint32_t kMinUptimeMs = 20000;
constexpr uint32_t kDeadlineMs = 180000;
constexpr uint32_t kUiStaleMs = 5000;

volatile bool s_onTrial = false;
volatile uint32_t s_uiAliveMs = 0;
volatile bool s_companionHeard = false;
esp_timer_handle_t s_timer = nullptr;
char s_rolledBackFrom[12] = {0};

// Runs on the esp_timer task, so a hung UI or network task can't stall it.
void check(void *) {
  const uint32_t now = millis();
  const bool uiOk = s_uiAliveMs != 0 && now - s_uiAliveMs < kUiStaleMs;
  if (now >= kMinUptimeMs && uiOk && s_companionHeard) {
    esp_ota_mark_app_valid_cancel_rollback();
    s_onTrial = false;
    esp_timer_stop(s_timer);
    Serial.println("[FirmwareHealth] New firmware confirmed");
    return;
  }
  if (now >= kDeadlineMs) {
    Serial.printf("[FirmwareHealth] Not healthy after %lu s (ui=%d, "
                  "companion=%d): going back to the previous firmware\n",
                  static_cast<unsigned long>(now / 1000), uiOk ? 1 : 0,
                  s_companionHeard ? 1 : 0);
    Serial.flush();
    esp_ota_mark_app_invalid_rollback_and_reboot();
  }
}

} // namespace

void begin() {
  const esp_partition_t *bad = esp_ota_get_last_invalid_partition();
  esp_app_desc_t desc;
  if (bad != nullptr && esp_ota_get_partition_description(bad, &desc) == ESP_OK) {
    snprintf(s_rolledBackFrom, sizeof(s_rolledBackFrom), "%02x%02x%02x%02x",
             desc.app_elf_sha256[0], desc.app_elf_sha256[1],
             desc.app_elf_sha256[2], desc.app_elf_sha256[3]);
    Serial.printf("[FirmwareHealth] Running after a rollback from %s\n",
                  s_rolledBackFrom);
  }

  esp_ota_img_states_t state;
  if (esp_ota_get_state_partition(esp_ota_get_running_partition(), &state) !=
          ESP_OK ||
      state != ESP_OTA_IMG_PENDING_VERIFY) {
    return;
  }
  s_onTrial = true;
  const esp_timer_create_args_t args = {.callback = check,
                                        .arg = nullptr,
                                        .dispatch_method = ESP_TIMER_TASK,
                                        .name = "fw_health",
                                        .skip_unhandled_events = true};
  if (esp_timer_create(&args, &s_timer) != ESP_OK ||
      esp_timer_start_periodic(s_timer, 1000000) != ESP_OK) {
    // Can't watch it: confirm rather than roll back at the next reset.
    esp_ota_mark_app_valid_cancel_rollback();
    s_onTrial = false;
    return;
  }
  Serial.println("[FirmwareHealth] New firmware on trial");
}

bool onTrial() { return s_onTrial; }

void uiAlive() { s_uiAliveMs = millis() | 1; }

void companionHeard() { s_companionHeard = true; }

const char *rolledBackFrom() { return s_rolledBackFrom; }

} // namespace FirmwareHealth
