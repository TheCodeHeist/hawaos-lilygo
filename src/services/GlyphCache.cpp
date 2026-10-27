#include "services/GlyphCache.h"

#include <Arduino.h>
#include <esp_heap_caps.h>
#include <freertos/FreeRTOS.h>
#include <string.h>

LV_FONT_DECLARE(font_inter_12_reg);
LV_FONT_DECLARE(font_inter_14_reg);
LV_FONT_DECLARE(font_inter_18_sb);

// The fallbacks the Inter text fonts name (tools/gen_icons.py). Filled in by
// begin(); user_data holds the pixel size.
extern "C" {
lv_font_t font_glyphs_12 = {};
lv_font_t font_glyphs_14 = {};
lv_font_t font_glyphs_18 = {};
}

namespace GlyphCache {

namespace {

struct Entry {
  uint32_t cp;
  uint32_t off; // into s_arena
  uint8_t px;
  uint8_t w;
  uint8_t h;
  int8_t ofsX;
  int8_t ofsY;
  uint8_t adv;
};

constexpr size_t kArenaBytes = 384 * 1024;
constexpr size_t kMaxEntries = 3072;
constexpr size_t kSlots = 4096; // hash table, power of two
constexpr uint16_t kEmpty = 0xFFFF;
constexpr size_t kRecentPacks = 8;

uint8_t *s_arena = nullptr;
size_t s_arenaUsed = 0;
Entry *s_entries = nullptr;
size_t s_count = 0;
uint16_t *s_slots = nullptr;
uint32_t s_session = 0;

uint8_t *s_staging = nullptr;
size_t s_stagedLen = 0;
char s_stagedHash[16] = {};
volatile bool s_staged = false;

// Packs already added, so a message repeating one doesn't fetch it again.
// Shared between the tasks.
portMUX_TYPE s_recentMux = portMUX_INITIALIZER_UNLOCKED;
char s_recent[kRecentPacks][16] = {};
size_t s_recentNext = 0;

size_t slotFor(uint32_t cp, uint8_t px) {
  return ((cp * 2654435761u) ^ px) & (kSlots - 1);
}

const Entry *find(uint32_t cp, uint8_t px) {
  if (s_slots == nullptr) {
    return nullptr;
  }
  for (size_t i = slotFor(cp, px), n = 0; n < kSlots;
       i = (i + 1) & (kSlots - 1), ++n) {
    if (s_slots[i] == kEmpty) {
      return nullptr;
    }
    const Entry &e = s_entries[s_slots[i]];
    if (e.cp == cp && e.px == px) {
      return &e;
    }
  }
  return nullptr;
}

void remember(const char *hash) {
  portENTER_CRITICAL(&s_recentMux);
  strncpy(s_recent[s_recentNext], hash, sizeof(s_recent[0]) - 1);
  s_recentNext = (s_recentNext + 1) % kRecentPacks;
  portEXIT_CRITICAL(&s_recentMux);
}

void clearRecent() {
  portENTER_CRITICAL(&s_recentMux);
  memset(s_recent, 0, sizeof(s_recent));
  portEXIT_CRITICAL(&s_recentMux);
}

void reset() {
  s_arenaUsed = 0;
  s_count = 0;
  memset(s_slots, 0xFF, kSlots * sizeof(uint16_t));
  clearRecent(); // the next message brings what it needs again
}

bool add(uint32_t cp, uint8_t px, uint8_t w, uint8_t h, int8_t ofsX,
         int8_t ofsY, uint8_t adv, const uint8_t *bits, size_t bitsLen) {
  if (find(cp, px) != nullptr) {
    return true;
  }
  if (s_count >= kMaxEntries || s_arenaUsed + bitsLen > kArenaBytes) {
    return false;
  }
  Entry &e = s_entries[s_count];
  e = Entry{cp, static_cast<uint32_t>(s_arenaUsed), px, w, h, ofsX, ofsY, adv};
  memcpy(s_arena + s_arenaUsed, bits, bitsLen);
  s_arenaUsed += bitsLen;
  size_t i = slotFor(cp, px);
  while (s_slots[i] != kEmpty) {
    i = (i + 1) & (kSlots - 1);
  }
  s_slots[i] = static_cast<uint16_t>(s_count++);
  return true;
}

uint8_t sizeOf(const lv_font_t *font) {
  return static_cast<uint8_t>(reinterpret_cast<uintptr_t>(font->user_data));
}

bool glyphDsc(const lv_font_t *font, lv_font_glyph_dsc_t *dsc, uint32_t letter,
              uint32_t) {
  const Entry *e = find(letter, sizeOf(font));
  if (e == nullptr) {
    return false;
  }
  dsc->adv_w = e->adv;
  dsc->box_w = e->w;
  dsc->box_h = e->h;
  dsc->ofs_x = e->ofsX;
  dsc->ofs_y = e->ofsY;
  dsc->bpp = 4;
  dsc->is_placeholder = 0;
  return true;
}

const uint8_t *glyphBitmap(const lv_font_t *font, uint32_t letter) {
  const Entry *e = find(letter, sizeOf(font));
  return (e != nullptr) ? s_arena + e->off : nullptr;
}

void setUp(lv_font_t &f, const lv_font_t &like, uint8_t px) {
  f.get_glyph_dsc = glyphDsc;
  f.get_glyph_bitmap = glyphBitmap;
  f.line_height = like.line_height;
  f.base_line = like.base_line;
  f.subpx = LV_FONT_SUBPX_NONE;
  f.underline_position = like.underline_position;
  f.underline_thickness = like.underline_thickness;
  f.dsc = nullptr;
  f.fallback = nullptr;
  f.user_data = reinterpret_cast<void *>(static_cast<uintptr_t>(px));
}

// Labels measured their text before its glyphs came: set it again so they
// re-measure (and restart any scrolling) now that they can draw it.
void refreshLabel(lv_obj_t *obj) {
  if (!lv_obj_check_type(obj, &lv_label_class)) {
    return;
  }
  const lv_label_t *label = reinterpret_cast<const lv_label_t *>(obj);
  const char *text = lv_label_get_text(obj);
  if (label->static_txt || text == nullptr) {
    return;
  }
  for (const char *c = text; *c != '\0'; ++c) {
    if (static_cast<uint8_t>(*c) >= 0xE0) { // a 3- or 4-byte character
      lv_label_set_text(obj, text);
      return;
    }
  }
}

lv_obj_tree_walk_res_t refreshCb(lv_obj_t *obj, void *) {
  refreshLabel(obj);
  return LV_OBJ_TREE_WALK_NEXT;
}

void refreshAll() {
  lv_disp_t *disp = lv_disp_get_default();
  if (disp == nullptr) {
    return;
  }
  for (uint32_t i = 0; i < disp->screen_cnt; ++i) {
    lv_obj_tree_walk(disp->screens[i], refreshCb, nullptr);
  }
  lv_obj_tree_walk(lv_disp_get_layer_top(disp), refreshCb, nullptr);
  lv_obj_tree_walk(lv_disp_get_layer_sys(disp), refreshCb, nullptr);
}

} // namespace

void begin() {
  setUp(font_glyphs_12, font_inter_12_reg, 12);
  setUp(font_glyphs_14, font_inter_14_reg, 14);
  setUp(font_glyphs_18, font_inter_18_sb, 18);
  const uint32_t caps = MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT;
  s_arena = static_cast<uint8_t *>(heap_caps_malloc(kArenaBytes, caps));
  s_entries =
      static_cast<Entry *>(heap_caps_malloc(kMaxEntries * sizeof(Entry), caps));
  s_slots = static_cast<uint16_t *>(
      heap_caps_malloc(kSlots * sizeof(uint16_t), caps));
  s_staging = static_cast<uint8_t *>(heap_caps_malloc(kPackBytes, caps));
  if (s_arena == nullptr || s_entries == nullptr || s_slots == nullptr ||
      s_staging == nullptr) {
    Serial.println("[GlyphCache] No memory; other scripts won't show");
    s_slots = nullptr; // find() then never matches
    s_staging = nullptr;
    return;
  }
  reset();
}

uint8_t *stagingBuffer() { return s_staging; }

bool canStage() { return s_staging != nullptr && !s_staged; }

void commit(const char *hash, size_t len) {
  if (s_staging == nullptr || len > kPackBytes) {
    return;
  }
  s_stagedLen = len;
  strncpy(s_stagedHash, hash, sizeof(s_stagedHash) - 1);
  remember(hash); // not fetched again while it waits for the UI
  s_staged = true;
}

bool hasPack(const char *hash) {
  bool found = false;
  portENTER_CRITICAL(&s_recentMux);
  for (size_t i = 0; i < kRecentPacks && !found; ++i) {
    found = (strncmp(s_recent[i], hash, sizeof(s_recent[0])) == 0);
  }
  portEXIT_CRITICAL(&s_recentMux);
  return found && hash[0] != '\0';
}

bool applyPending() {
  if (!s_staged) {
    return false;
  }
  // "HGL1", session, count, then per glyph: cp, px, w, h, ofs_x, ofs_y, adv,
  // reserved, 4-bit bitmap (glyphs.rs).
  const uint8_t *p = s_staging;
  const uint8_t *end = s_staging + s_stagedLen;
  bool added = false;
  if (s_stagedLen >= 10 && memcmp(p, "HGL1", 4) == 0) {
    uint32_t session;
    uint16_t count;
    memcpy(&session, p + 4, 4);
    memcpy(&count, p + 8, 2);
    p += 10;
    if (session != s_session) {
      // A restarted companion numbers its characters afresh.
      reset();
      s_session = session;
    }
    for (uint16_t i = 0; i < count && p + 11 <= end; ++i) {
      uint32_t cp;
      memcpy(&cp, p, 4);
      const uint8_t px = p[4], w = p[5], h = p[6];
      const int8_t ofsX = static_cast<int8_t>(p[7]);
      const int8_t ofsY = static_cast<int8_t>(p[8]);
      const uint8_t adv = p[9];
      const size_t bitsLen = (static_cast<size_t>(w) * h + 1) / 2;
      p += 11;
      if (p + bitsLen > end) {
        break;
      }
      if (!add(cp, px, w, h, ofsX, ofsY, adv, p, bitsLen)) {
        // Full: start over with this pack (others come back with their
        // next message, since reset() forgets them).
        reset();
        add(cp, px, w, h, ofsX, ofsY, adv, p, bitsLen);
      }
      p += bitsLen;
      added = true;
    }
  }
  if (!hasPack(s_stagedHash)) {
    remember(s_stagedHash); // a reset() above forgot it
  }
  s_staged = false;
  if (added) {
    refreshAll();
  }
  return added;
}

} // namespace GlyphCache
