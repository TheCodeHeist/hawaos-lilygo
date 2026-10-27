#pragma once

#include <lvgl.h>
#include <stddef.h>
#include <stdint.h>

// Glyphs for scripts the built-in fonts lack (DESIGN.md Section 11.9). The
// companion shapes such text itself and sends each cluster as a 4-bit glyph
// image standing for one private-use character, at 12, 14 and 18 px. The
// Inter text fonts fall back to font_glyphs_{12,14,18}, which draw from here,
// so every label shows them with no other changes.
//
// Packs arrive like album art: the network task downloads one into the
// staging buffer and commit()s it; the UI task adds it with applyPending().
namespace GlyphCache {

// The largest pack (glyphs.rs MAX_PACK).
constexpr size_t kPackBytes = 32 * 1024;

void begin(); // allocates the cache (PSRAM)

// Network task.
uint8_t *stagingBuffer();
bool canStage();
void commit(const char *hash, size_t len);
// Whether this pack's glyphs are already in the cache.
bool hasPack(const char *hash);

// UI task: adds a committed pack and redraws text that uses it. True if
// glyphs were added.
bool applyPending();

} // namespace GlyphCache
