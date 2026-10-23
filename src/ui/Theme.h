#pragma once

#include <lvgl.h>
#include <stdint.h>

namespace Theme {

constexpr uint32_t BG_DARK = 0x0B0F19;
constexpr uint32_t BG_SURFACE = 0x1E293B;
constexpr uint32_t BG_HIGHLIGHT = 0x1E3A8A;
constexpr uint32_t BORDER_SUBTLE = 0x334155;
constexpr uint32_t ACCENT_CYAN = 0x38BDF8;
constexpr uint32_t ACCENT_GREEN = 0x22C55E;
constexpr uint32_t ACCENT_AMBER = 0xF59E0B;
constexpr uint32_t ACCENT_RED = 0xEF4444;
constexpr uint32_t ACCENT_PURPLE = 0xA855F7;
constexpr uint32_t TEXT_PRIMARY = 0xF8FAFC;
constexpr uint32_t TEXT_MUTED = 0x94A3B8;
constexpr uint32_t TEXT_DIM = 0x64748B;

void styleScreen(lv_obj_t *scr);
lv_obj_t *createCard(lv_obj_t *parent, int16_t w, int16_t h, int16_t xOfs,
                     int16_t yOfs);

} // namespace Theme
