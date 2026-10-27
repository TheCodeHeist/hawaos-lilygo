#!/usr/bin/env bash
# Cuts a HawaOS release: firmware and companion share one version.
#
#   tools/release.sh 0.7.0 notes.md            # build, commit, tag, push, publish
#   tools/release.sh 0.7.0 notes.md --dry-run  # build and stage only
#
# Before running: add a <release version="X"> entry to the AppStream
# metainfo (it's the changelog the app store and Discover show) and write
# the GitHub release notes to notes.md. Needs pio, flatpak-builder (see
# packaging/flatpak/build.sh) and gh.
#
# Assets land in dist/<version>/: hawaos-X-full.bin (flash at 0x0, new
# boards), hawaos-X-update.bin (OTA), hawaos-companion-X-x86_64.flatpak and
# SHA256SUMS.
set -euo pipefail
cd "$(dirname "$0")/.."

VER=${1:?usage: tools/release.sh <version> <notes.md> [--dry-run]}
NOTES=${2:?usage: tools/release.sh <version> <notes.md> [--dry-run]}
DRY=${3:-}
APP=dev.is_a.thecodeheist.HawaOSCompanion
META=packaging/flatpak/$APP.metainfo.xml
ENV=tdisplay-s3
OUT=dist/$VER

die() { echo "release: $*" >&2; exit 1; }

[[ $VER =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "version must look like 1.2.3"
[[ -f $NOTES ]] || die "no notes file: $NOTES"
grep -q "<release version=\"$VER\"" "$META" || die "add a <release version=\"$VER\"> entry to $META first"
git rev-parse -q --verify "refs/tags/v$VER" >/dev/null && die "tag v$VER already exists"
if [[ $DRY != --dry-run ]]; then
  [[ $(git branch --show-current) == main ]] || die "release from main"
  # The metainfo entry is the only change expected before a release.
  dirty=$(git status --porcelain | grep -v " $META\$" || true)
  [[ -z $dirty ]] || die "uncommitted changes:"$'\n'"$dirty"
fi

echo "== Version $VER"
sed -i "0,/^version = \".*\"/s//version = \"$VER\"/" companion/Cargo.toml
sed -i "/^name = \"hawaos-companion\"\$/{n;s/^version = \".*\"/version = \"$VER\"/}" companion/Cargo.lock
sed -i "s/\(FW_VERSION = \"\)[^\"]*\"/\1$VER\"/" include/config.h

echo "== Firmware"
pio run -e "$ENV"
BUILD=.pio/build/$ENV
BOOT_APP0=$(find ~/.platformio/packages/framework-arduinoespressif32* -name boot_app0.bin | head -1)
ESPTOOL=$(find ~/.platformio/packages/tool-esptoolpy -name esptool.py | head -1)
[[ -n $BOOT_APP0 && -n $ESPTOOL ]] || die "boot_app0.bin or esptool.py not found under ~/.platformio"
grep -qx "$VER" < <(strings "$BUILD/firmware.bin") || die "firmware doesn't report $VER"

rm -rf "$OUT" && mkdir -p "$OUT"
# Same layout pio uploads: bootloader, partition table, boot_app0, app.
python3 "$ESPTOOL" --chip esp32s3 merge_bin -o "$OUT/hawaos-$VER-full.bin" \
  --flash_mode dio --flash_freq 80m --flash_size 16MB \
  0x0 "$BUILD/bootloader.bin" 0x8000 "$BUILD/partitions.bin" \
  0xe000 "$BOOT_APP0" 0x10000 "$BUILD/firmware.bin"
cp "$BUILD/firmware.bin" "$OUT/hawaos-$VER-update.bin"

echo "== Companion"
tools/gen_cargo_sources.py # Flatpak builds offline from these
packaging/flatpak/build.sh
cp packaging/flatpak/hawaos-companion.flatpak "$OUT/hawaos-companion-$VER-x86_64.flatpak"

(cd "$OUT" && sha256sum hawaos-*.bin hawaos-companion-*.flatpak > SHA256SUMS)
echo "== Staged in $OUT"
ls -l "$OUT"

if [[ $DRY == --dry-run ]]; then
  echo "Dry run: versions bumped and assets built; nothing committed or published."
  exit 0
fi

echo "== Publish"
git add companion/Cargo.toml companion/Cargo.lock include/config.h "$META" \
  packaging/flatpak/cargo-sources.json
git commit -q -m "Release $VER"
git tag "v$VER"
git push origin main "v$VER"
gh release create "v$VER" --title "HawaOS $VER" --notes-file "$NOTES" \
  "$OUT"/*.bin "$OUT"/*.flatpak "$OUT"/SHA256SUMS
echo "Released v$VER"
