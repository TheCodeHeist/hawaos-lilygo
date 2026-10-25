#pragma once

#include "core/App.h"
#include <lvgl.h>

class WeatherApp final : public App {
public:
  const char *name() const override { return "Weather"; }
  const char *icon() const override { return ICON_CLOUD_SUN; }

  void onEnter(lv_obj_t *root) override;
  void onExit() override;
  void onTick(uint32_t nowMs) override;
  bool onButton(Button b, Press p) override;
  void onData(const DataSnapshot &d) override;

private:
  void refreshWeather(const DataSnapshot &d);

  // Current weather card (left column)
  lv_obj_t *m_currCard = nullptr;
  lv_obj_t *m_iconLbl = nullptr;
  lv_obj_t *m_tempLbl = nullptr;
  lv_obj_t *m_condLbl = nullptr;
  lv_obj_t *m_cityLbl = nullptr;
  lv_obj_t *m_hiLoLbl = nullptr;
  lv_obj_t *m_metricsLbl = nullptr;
  lv_obj_t *m_waitingLbl = nullptr;

  // 3-Day Forecast card (right column)
  lv_obj_t *m_fcCard = nullptr;
  lv_obj_t *m_fcHeaderLbl = nullptr;
  struct ForecastRowWidgets {
    lv_obj_t *dayLbl = nullptr;
    lv_obj_t *iconLbl = nullptr;
    lv_obj_t *rangeLbl = nullptr;
  };
  ForecastRowWidgets m_fcRows[3] = {};
};
