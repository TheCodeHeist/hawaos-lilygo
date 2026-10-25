#include "apps/WeatherApp.h"
#include "config.h"
#include "core/AppManager.h"
#include "core/DataStore.h"
#include "ui/Theme.h"
#include "ui/icons.h"

#include <Arduino.h>
#include <stdio.h>
#include <string.h>

namespace {

const char *getWeatherIcon(const char *iconStr) {
  if (iconStr == nullptr || iconStr[0] == '\0') {
    return ICON_CLOUD_SUN;
  }
  if (strcmp(iconStr, "sun") == 0)
    return ICON_SUN;
  if (strcmp(iconStr, "moon") == 0)
    return ICON_MOON;
  if (strcmp(iconStr, "cloud-sun") == 0)
    return ICON_CLOUD_SUN;
  if (strcmp(iconStr, "cloud-moon") == 0)
    return ICON_CLOUD_MOON;
  if (strcmp(iconStr, "cloud") == 0)
    return ICON_CLOUD;
  if (strcmp(iconStr, "cloud-rain") == 0)
    return ICON_CLOUD_RAIN;
  if (strcmp(iconStr, "cloud-snow") == 0)
    return ICON_CLOUD_SNOW;
  if (strcmp(iconStr, "cloud-lightning") == 0)
    return ICON_CLOUD_LIGHTNING;
  if (strcmp(iconStr, "cloud-fog") == 0)
    return ICON_CLOUD_FOG;
  if (strcmp(iconStr, "wind") == 0)
    return ICON_WIND;
  return ICON_CLOUD_SUN;
}

void setLabelTextIfChanged(lv_obj_t *lbl, const char *newText) {
  if (lbl == nullptr || newText == nullptr)
    return;
  const char *cur = lv_label_get_text(lbl);
  if (cur == nullptr || strcmp(cur, newText) != 0) {
    lv_label_set_text(lbl, newText);
  }
}

} // namespace

void WeatherApp::onEnter(lv_obj_t *root) {
  const ThemeTokens::Palette &p = Theme::active();

  // --- Left Card: Current Weather (152x136 at x=10, y=26) ---
  m_currCard =
      Theme::createCard(root, 152, 136, Config::OUTER_MARGIN_PX,
                        Config::CONTENT_Y_START + 4, Config::RADIUS_MD);
  lv_obj_set_style_pad_all(m_currCard, 8, LV_PART_MAIN);

  m_iconLbl = lv_label_create(m_currCard);
  lv_label_set_text(m_iconLbl, ICON_CLOUD_SUN);
  lv_obj_set_style_text_font(m_iconLbl, Theme::iconFont32(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_iconLbl, lv_color_hex(p.accent), LV_PART_MAIN);
  lv_obj_set_pos(m_iconLbl, 4, 4);

  m_tempLbl = lv_label_create(m_currCard);
  lv_label_set_text(m_tempLbl, "--°");
  lv_obj_set_style_text_font(m_tempLbl, Theme::fontDisplay(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_tempLbl, lv_color_hex(p.text), LV_PART_MAIN);
  lv_obj_set_pos(m_tempLbl, 48, 6);

  m_condLbl = lv_label_create(m_currCard);
  lv_label_set_text(m_condLbl, "Waiting...");
  lv_obj_set_style_text_font(m_condLbl, Theme::fontBodySb(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_condLbl, lv_color_hex(p.text), LV_PART_MAIN);
  lv_obj_set_width(m_condLbl, 136);
  lv_label_set_long_mode(m_condLbl, LV_LABEL_LONG_DOT);
  lv_obj_set_pos(m_condLbl, 4, 44);

  m_cityLbl = lv_label_create(m_currCard);
  lv_label_set_text(m_cityLbl, "--");
  lv_obj_set_style_text_font(m_cityLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_cityLbl, lv_color_hex(p.textDim), LV_PART_MAIN);
  lv_obj_set_width(m_cityLbl, 136);
  lv_label_set_long_mode(m_cityLbl, LV_LABEL_LONG_DOT);
  lv_obj_set_pos(m_cityLbl, 4, 66);

  m_hiLoLbl = lv_label_create(m_currCard);
  lv_label_set_text(m_hiLoLbl, "H: --°  L: --°");
  lv_obj_set_style_text_font(m_hiLoLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_hiLoLbl, lv_color_hex(p.accent), LV_PART_MAIN);
  lv_obj_set_pos(m_hiLoLbl, 4, 86);

  m_metricsLbl = lv_label_create(m_currCard);
  lv_label_set_text(m_metricsLbl, "");
  lv_obj_set_style_text_font(m_metricsLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_metricsLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_set_pos(m_metricsLbl, 4, 106);

  m_waitingLbl = lv_label_create(m_currCard);
  lv_label_set_text(m_waitingLbl, "Waiting for companion weather...");
  lv_obj_set_style_text_font(m_waitingLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_waitingLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_set_width(m_waitingLbl, 136);
  lv_label_set_long_mode(m_waitingLbl, LV_LABEL_LONG_WRAP);
  lv_obj_center(m_waitingLbl);
  lv_obj_add_flag(m_waitingLbl, LV_OBJ_FLAG_HIDDEN);

  // --- Right Card: 3-Day Forecast (142x136 at x=168, y=26) ---
  m_fcCard = Theme::createCard(root, 142, 136, 168, Config::CONTENT_Y_START + 4,
                               Config::RADIUS_MD);
  lv_obj_set_style_pad_all(m_fcCard, 8, LV_PART_MAIN);

  m_fcHeaderLbl = lv_label_create(m_fcCard);
  lv_label_set_text(m_fcHeaderLbl, "3-DAY FORECAST");
  lv_obj_set_style_text_font(m_fcHeaderLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_fcHeaderLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_set_pos(m_fcHeaderLbl, 4, 4);

  constexpr int16_t rowYs[3] = {28, 64, 100};
  for (int i = 0; i < 3; ++i) {
    m_fcRows[i].dayLbl = lv_label_create(m_fcCard);
    lv_label_set_text(m_fcRows[i].dayLbl, (i == 0) ? "Today" : "--");
    lv_obj_set_style_text_font(m_fcRows[i].dayLbl, Theme::fontCaption(),
                               LV_PART_MAIN);
    lv_obj_set_style_text_color(m_fcRows[i].dayLbl, lv_color_hex(p.text),
                                LV_PART_MAIN);
    lv_obj_set_pos(m_fcRows[i].dayLbl, 4, rowYs[i] + 2);

    m_fcRows[i].iconLbl = lv_label_create(m_fcCard);
    lv_label_set_text(m_fcRows[i].iconLbl, ICON_CLOUD_SUN);
    lv_obj_set_style_text_font(m_fcRows[i].iconLbl, Theme::iconFont20(),
                               LV_PART_MAIN);
    lv_obj_set_style_text_color(m_fcRows[i].iconLbl, lv_color_hex(p.accent),
                                LV_PART_MAIN);
    lv_obj_set_pos(m_fcRows[i].iconLbl, 54, rowYs[i]);

    m_fcRows[i].rangeLbl = lv_label_create(m_fcCard);
    lv_label_set_text(m_fcRows[i].rangeLbl, "-- / --");
    lv_obj_set_style_text_font(m_fcRows[i].rangeLbl, Theme::fontCaption(),
                               LV_PART_MAIN);
    lv_obj_set_style_text_color(m_fcRows[i].rangeLbl, lv_color_hex(p.textDim),
                                LV_PART_MAIN);
    lv_obj_set_pos(m_fcRows[i].rangeLbl, 84, rowYs[i] + 2);
  }

  // Populate with snapshot data immediately
  refreshWeather(DataStore::getSnapshot());
}

void WeatherApp::onExit() {
  m_currCard = nullptr;
  m_iconLbl = nullptr;
  m_tempLbl = nullptr;
  m_condLbl = nullptr;
  m_cityLbl = nullptr;
  m_hiLoLbl = nullptr;
  m_metricsLbl = nullptr;
  m_waitingLbl = nullptr;

  m_fcCard = nullptr;
  m_fcHeaderLbl = nullptr;
  for (int i = 0; i < 3; ++i) {
    m_fcRows[i].dayLbl = nullptr;
    m_fcRows[i].iconLbl = nullptr;
    m_fcRows[i].rangeLbl = nullptr;
  }
}

void WeatherApp::onTick(uint32_t /*nowMs*/) {
  // Weather is pushed by companion via events; periodic refresh handled in
  // onData
}

bool WeatherApp::onButton(Button /*b*/, Press /*p*/) {
  return false; // Let AppManager handle Next long -> Launcher, Select long ->
                // Back
}

void WeatherApp::onData(const DataSnapshot &d) { refreshWeather(d); }

void WeatherApp::refreshWeather(const DataSnapshot &d) {
  if (m_currCard == nullptr)
    return;

  const WeatherSnapshot &ws = d.weather;
  if (!ws.valid) {
    lv_obj_clear_flag(m_waitingLbl, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_flag(m_iconLbl, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_flag(m_tempLbl, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_flag(m_condLbl, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_flag(m_cityLbl, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_flag(m_hiLoLbl, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_flag(m_metricsLbl, LV_OBJ_FLAG_HIDDEN);
    return;
  }

  lv_obj_add_flag(m_waitingLbl, LV_OBJ_FLAG_HIDDEN);
  lv_obj_clear_flag(m_iconLbl, LV_OBJ_FLAG_HIDDEN);
  lv_obj_clear_flag(m_tempLbl, LV_OBJ_FLAG_HIDDEN);
  lv_obj_clear_flag(m_condLbl, LV_OBJ_FLAG_HIDDEN);
  lv_obj_clear_flag(m_cityLbl, LV_OBJ_FLAG_HIDDEN);
  lv_obj_clear_flag(m_hiLoLbl, LV_OBJ_FLAG_HIDDEN);
  lv_obj_clear_flag(m_metricsLbl, LV_OBJ_FLAG_HIDDEN);

  // Current icon
  setLabelTextIfChanged(m_iconLbl, getWeatherIcon(ws.icon));

  // Current temperature
  char tempBuf[16];
  snprintf(tempBuf, sizeof(tempBuf), "%d°%s", ws.temp, ws.unit);
  setLabelTextIfChanged(m_tempLbl, tempBuf);

  // Condition text
  setLabelTextIfChanged(m_condLbl,
                        ws.condition[0] != '\0' ? ws.condition : "Clear");

  // City
  setLabelTextIfChanged(m_cityLbl, ws.city[0] != '\0' ? ws.city : "Local");

  // High / Low
  char hiLoBuf[32];
  snprintf(hiLoBuf, sizeof(hiLoBuf), "H: %d°  L: %d°", ws.tempMax, ws.tempMin);
  setLabelTextIfChanged(m_hiLoLbl, hiLoBuf);

  // Metrics: Humidity & Wind
  char metricsBuf[40];
  const char *speedUnit = (strcmp(ws.unit, "F") == 0) ? "mph" : "km/h";
  snprintf(metricsBuf, sizeof(metricsBuf), "%u%% hum · %u %s",
           static_cast<unsigned>(ws.humidity),
           static_cast<unsigned>(ws.windSpeed), speedUnit);
  setLabelTextIfChanged(m_metricsLbl, metricsBuf);

  // 3-Day Forecast rows
  for (int i = 0; i < 3; ++i) {
    if (m_fcRows[i].dayLbl == nullptr)
      continue;
    const WeatherForecastDay &fd = ws.forecast[i];

    if (fd.day[0] != '\0') {
      setLabelTextIfChanged(m_fcRows[i].dayLbl, fd.day);
      setLabelTextIfChanged(m_fcRows[i].iconLbl, getWeatherIcon(fd.icon));

      char rangeBuf[24];
      snprintf(rangeBuf, sizeof(rangeBuf), "%d° / %d°", fd.tempMin, fd.tempMax);
      setLabelTextIfChanged(m_fcRows[i].rangeLbl, rangeBuf);
    }
  }
}
