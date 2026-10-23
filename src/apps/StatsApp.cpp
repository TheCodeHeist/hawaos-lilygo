#include "apps/StatsApp.h"
#include "config.h"
#include "ui/Theme.h"
#include "ui/icons.h"

#include <Arduino.h>
#include <esp_heap_caps.h>
#include <stdio.h>

namespace {

void formatRate(uint32_t kbps, char *out, size_t maxLen) {
  if (kbps >= 1024) {
    const float mbps = static_cast<float>(kbps) / 1024.0f;
    snprintf(out, maxLen, "%.1f MB/s", static_cast<double>(mbps));
  } else {
    snprintf(out, maxLen, "%lu KB/s", static_cast<unsigned long>(kbps));
  }
}

uint32_t barColorForPct(const ThemeTokens::Palette &p, int pct) {
  if (pct >= 90)
    return p.err;
  if (pct >= 75)
    return p.warn;
  return p.primary;
}

} // namespace

void StatsApp::onEnter(lv_obj_t *root) {
  const ThemeTokens::Palette &p = Theme::active();

  // Left Column: Two stacked cards (CPU and RAM) per DESIGN.md Section 9.4
  lv_obj_t *cpuCard = Theme::createCard(
      root, 144, 62, 10, Config::CONTENT_Y_START + 8, Config::RADIUS_MD);
  m_cpuTitleLbl = lv_label_create(cpuCard);
  lv_label_set_text(m_cpuTitleLbl, "CPU");
  lv_obj_set_style_text_font(m_cpuTitleLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_cpuTitleLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_align(m_cpuTitleLbl, LV_ALIGN_TOP_LEFT, 2, 0);

  m_cpuValLbl = lv_label_create(cpuCard);
  lv_label_set_text(m_cpuValLbl, "--%");
  lv_obj_set_style_text_font(m_cpuValLbl, Theme::fontDisplay(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_cpuValLbl, lv_color_hex(p.text), LV_PART_MAIN);
  lv_obj_align(m_cpuValLbl, LV_ALIGN_TOP_RIGHT, -2, -2);

  m_cpuBar = lv_bar_create(cpuCard);
  lv_obj_set_size(m_cpuBar, 124, 4);
  lv_obj_align(m_cpuBar, LV_ALIGN_BOTTOM_MID, 0, 0);
  lv_bar_set_range(m_cpuBar, 0, 100);
  lv_obj_set_style_anim_time(
      m_cpuBar, Theme::animDuration(Config::ANIM_VALUE_MS), LV_PART_MAIN);
  lv_obj_set_style_radius(m_cpuBar, LV_RADIUS_CIRCLE, LV_PART_MAIN);
  lv_obj_set_style_radius(m_cpuBar, LV_RADIUS_CIRCLE, LV_PART_INDICATOR);
  lv_obj_set_style_bg_color(m_cpuBar, lv_color_hex(p.surface2), LV_PART_MAIN);
  lv_obj_set_style_bg_color(m_cpuBar, lv_color_hex(p.primary),
                            LV_PART_INDICATOR);

  lv_obj_t *ramCard = Theme::createCard(
      root, 144, 62, 10, Config::CONTENT_Y_START + 78, Config::RADIUS_MD);
  m_ramTitleLbl = lv_label_create(ramCard);
  lv_label_set_text(m_ramTitleLbl, "RAM");
  lv_obj_set_style_text_font(m_ramTitleLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_ramTitleLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_align(m_ramTitleLbl, LV_ALIGN_TOP_LEFT, 2, 0);

  m_ramValLbl = lv_label_create(ramCard);
  lv_label_set_text(m_ramValLbl, "--%");
  lv_obj_set_style_text_font(m_ramValLbl, Theme::fontDisplay(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_ramValLbl, lv_color_hex(p.text), LV_PART_MAIN);
  lv_obj_align(m_ramValLbl, LV_ALIGN_TOP_RIGHT, -2, -2);

  m_ramBar = lv_bar_create(ramCard);
  lv_obj_set_size(m_ramBar, 124, 4);
  lv_obj_align(m_ramBar, LV_ALIGN_BOTTOM_MID, 0, 0);
  lv_bar_set_range(m_ramBar, 0, 100);
  lv_obj_set_style_anim_time(
      m_ramBar, Theme::animDuration(Config::ANIM_VALUE_MS), LV_PART_MAIN);
  lv_obj_set_style_radius(m_ramBar, LV_RADIUS_CIRCLE, LV_PART_MAIN);
  lv_obj_set_style_radius(m_ramBar, LV_RADIUS_CIRCLE, LV_PART_INDICATOR);
  lv_obj_set_style_bg_color(m_ramBar, lv_color_hex(p.surface2), LV_PART_MAIN);
  lv_obj_set_style_bg_color(m_ramBar, lv_color_hex(p.primary),
                            LV_PART_INDICATOR);

  // Right Wide Card: Network up/down values above a 60-sample sparkline
  lv_obj_t *rightCard = Theme::createCard(
      root, 148, 132, 162, Config::CONTENT_Y_START + 8, Config::RADIUS_MD);

  m_netTitleLbl = lv_label_create(rightCard);
  lv_label_set_text(m_netTitleLbl, "NETWORK");
  lv_obj_set_style_text_font(m_netTitleLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_netTitleLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_align(m_netTitleLbl, LV_ALIGN_TOP_LEFT, 2, 0);

  m_netUpDownLbl = lv_label_create(rightCard);
  lv_label_set_text(m_netUpDownLbl, "D: -- KB/s\nU: -- KB/s");
  lv_obj_set_style_text_font(m_netUpDownLbl, Theme::fontBodySb(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_netUpDownLbl, lv_color_hex(p.text),
                              LV_PART_MAIN);
  lv_obj_align(m_netUpDownLbl, LV_ALIGN_TOP_LEFT, 2, 18);

  // Sparkline (60 samples, 2px line, accent color)
  m_chart = lv_chart_create(rightCard);
  lv_obj_set_size(m_chart, 128, 56);
  lv_obj_align(m_chart, LV_ALIGN_BOTTOM_MID, 0, 0);
  lv_chart_set_type(m_chart, LV_CHART_TYPE_LINE);
  lv_chart_set_point_count(m_chart, DataSnapshot::SPARKLINE_LEN);
  lv_chart_set_div_line_count(m_chart, 0, 0);
  lv_chart_set_range(m_chart, LV_CHART_AXIS_PRIMARY_Y, 0, 1024);

  lv_obj_set_style_bg_opa(m_chart, LV_OPA_TRANSP, LV_PART_MAIN);
  lv_obj_set_style_border_width(m_chart, 0, LV_PART_MAIN);
  lv_obj_set_style_pad_all(m_chart, 0, LV_PART_MAIN);
  lv_obj_set_style_size(m_chart, 0, LV_PART_INDICATOR); // Hide point dots
  lv_obj_set_style_line_width(m_chart, 2, LV_PART_ITEMS);

  m_netSeries = lv_chart_add_series(m_chart, lv_color_hex(p.accent),
                                    LV_CHART_AXIS_PRIMARY_Y);

  refreshStats(DataStore::getSnapshot());
}

void StatsApp::onExit() {
  m_cpuTitleLbl = nullptr;
  m_cpuValLbl = nullptr;
  m_cpuBar = nullptr;
  m_ramTitleLbl = nullptr;
  m_ramValLbl = nullptr;
  m_ramBar = nullptr;
  m_netTitleLbl = nullptr;
  m_netUpDownLbl = nullptr;
  m_chart = nullptr;
  m_netSeries = nullptr;
}

void StatsApp::onTick(uint32_t /*nowMs*/) {
  if (m_showOnDevice || DataStore::getSnapshot().lastStatsMs == 0) {
    refreshStats(DataStore::getSnapshot());
  }
}

bool StatsApp::onButton(Button b, Press p) {
  // Select short toggles between PC Companion stats and On-Device ESP32 memory
  // stats
  if (b == Button::Select && p == Press::Short) {
    m_showOnDevice = !m_showOnDevice;
    refreshStats(DataStore::getSnapshot());
    return true;
  }
  return false;
}

void StatsApp::onData(const DataSnapshot &d) { refreshStats(d); }

void StatsApp::refreshStats(const DataSnapshot &d) {
  if (m_cpuValLbl == nullptr)
    return;

  const ThemeTokens::Palette &p = Theme::active();
  const lv_anim_enable_t anim = (Theme::animDuration(Config::ANIM_VALUE_MS) > 0)
                                    ? LV_ANIM_ON
                                    : LV_ANIM_OFF;

  const bool hasCompanion = (d.lastStatsMs > 0) && !m_showOnDevice;

  if (hasCompanion) {
    lv_label_set_text(m_cpuTitleLbl, "CPU");
    lv_label_set_text_fmt(m_cpuValLbl, "%u%%",
                          static_cast<unsigned>(d.cpuPercent));
    lv_obj_set_style_bg_color(m_cpuBar,
                              lv_color_hex(barColorForPct(p, d.cpuPercent)),
                              LV_PART_INDICATOR);
    lv_bar_set_value(m_cpuBar, d.cpuPercent, anim);

    lv_label_set_text(m_ramTitleLbl, "RAM");
    lv_label_set_text_fmt(m_ramValLbl, "%u%%",
                          static_cast<unsigned>(d.ramPercent));
    lv_obj_set_style_bg_color(m_ramBar,
                              lv_color_hex(barColorForPct(p, d.ramPercent)),
                              LV_PART_INDICATOR);
    lv_bar_set_value(m_ramBar, d.ramPercent, anim);

    lv_label_set_text(m_netTitleLbl, "NETWORK");
    char downBuf[24];
    char upBuf[24];
    formatRate(d.netDownKbps, downBuf, sizeof(downBuf));
    formatRate(d.netUpKbps, upBuf, sizeof(upBuf));
    lv_label_set_text_fmt(m_netUpDownLbl, "D: %s\nU: %s", downBuf, upBuf);

    uint16_t maxVal = 100;
    for (uint8_t i = 0; i < DataSnapshot::SPARKLINE_LEN; ++i) {
      if (d.netDownHistory[i] > maxVal) {
        maxVal = d.netDownHistory[i];
      }
      m_netSeries->y_points[i] = static_cast<lv_coord_t>(d.netDownHistory[i]);
    }
    lv_chart_set_range(m_chart, LV_CHART_AXIS_PRIMARY_Y, 0,
                       static_cast<lv_coord_t>(maxVal + (maxVal / 5)));
    lv_chart_refresh(m_chart);
  } else {
    // Fallback / On-device ESP32-S3 diagnostics when companion is offline or
    // toggled
    const size_t sramFree = heap_caps_get_free_size(MALLOC_CAP_INTERNAL);
    const size_t sramTotal = heap_caps_get_total_size(MALLOC_CAP_INTERNAL);
    const int sramPct =
        (sramTotal > 0)
            ? static_cast<int>(((sramTotal - sramFree) * 100) / sramTotal)
            : 0;

    const size_t psramFree = heap_caps_get_free_size(MALLOC_CAP_SPIRAM);
    const size_t psramTotal = heap_caps_get_total_size(MALLOC_CAP_SPIRAM);
    const int psramPct =
        (psramTotal > 0)
            ? static_cast<int>(((psramTotal - psramFree) * 100) / psramTotal)
            : 0;

    lv_label_set_text(m_cpuTitleLbl, "SRAM");
    lv_label_set_text_fmt(m_cpuValLbl, "%d%%", sramPct);
    lv_obj_set_style_bg_color(m_cpuBar, lv_color_hex(p.primary),
                              LV_PART_INDICATOR);
    lv_bar_set_value(m_cpuBar, sramPct, anim);

    lv_label_set_text(m_ramTitleLbl, "PSRAM");
    lv_label_set_text_fmt(m_ramValLbl, "%d%%", psramPct);
    lv_obj_set_style_bg_color(m_ramBar, lv_color_hex(p.primary),
                              LV_PART_INDICATOR);
    lv_bar_set_value(m_ramBar, psramPct, anim);

    lv_label_set_text(m_netTitleLbl,
                      (d.lastStatsMs == 0) ? "AWAITING PC" : "ON-DEVICE");
    lv_label_set_text_fmt(m_netUpDownLbl, "SRAM %u KB\nIP %s",
                          static_cast<unsigned>(sramFree / 1024),
                          d.wifiConnected ? d.wifiIp : "--");

    for (uint8_t i = 0; i < DataSnapshot::SPARKLINE_LEN; ++i) {
      m_netSeries->y_points[i] = static_cast<lv_coord_t>(sramPct);
    }
    lv_chart_set_range(m_chart, LV_CHART_AXIS_PRIMARY_Y, 0, 100);
    lv_chart_refresh(m_chart);
  }
}
