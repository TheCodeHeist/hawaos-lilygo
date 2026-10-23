#include "ui/Theme.h"

namespace Theme {

void styleScreen(lv_obj_t *scr) {
  lv_obj_set_style_bg_color(scr, lv_color_hex(BG_DARK), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(scr, LV_OPA_COVER, LV_PART_MAIN);
  lv_obj_clear_flag(scr, LV_OBJ_FLAG_SCROLLABLE);
}

lv_obj_t *createCard(lv_obj_t *parent, int16_t w, int16_t h, int16_t xOfs,
                     int16_t yOfs) {
  lv_obj_t *card = lv_obj_create(parent);
  lv_obj_set_size(card, w, h);
  lv_obj_align(card, LV_ALIGN_TOP_MID, xOfs, yOfs);
  lv_obj_set_style_bg_color(card, lv_color_hex(BG_SURFACE), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(card, LV_OPA_COVER, LV_PART_MAIN);
  lv_obj_set_style_border_color(card, lv_color_hex(BORDER_SUBTLE),
                                LV_PART_MAIN);
  lv_obj_set_style_border_width(card, 1, LV_PART_MAIN);
  lv_obj_set_style_radius(card, 8, LV_PART_MAIN);
  lv_obj_set_style_pad_all(card, 8, LV_PART_MAIN);
  lv_obj_clear_flag(card, LV_OBJ_FLAG_SCROLLABLE);
  return card;
}

} // namespace Theme
