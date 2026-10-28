#!/usr/bin/env bash
# Builds the companion AppImage from a release build, on the same old base as
# the .deb/.rpm (.github/workflows/linux.yml runs it on Ubuntu 22.04):
#
#   packaging/linux/build-appimage.sh 0.9.0 dist/
#
# Needs playerctl and dbus-monitor installed (they're bundled for systems
# without them) and downloads appimagetool, whose static runtime means hosts
# need no libfuse2. The graphics stack (Vulkan, EGL, Wayland, X11) isn't
# bundled: it must match the host's drivers. Writes
# hawaos-companion-<version>-x86_64.AppImage and its .zsync, which lets
# AppImageUpdate and Gear Lever update it from the latest release.
set -euo pipefail
cd "$(dirname "$0")/../.."

VER=${1:?usage: build-appimage.sh <version> <out dir>}
OUT=${2:?usage: build-appimage.sh <version> <out dir>}
APP=dev.is_a.thecodeheist.HawaOSCompanion
BIN=companion/target/release/hawaos-companion
[[ -x $BIN ]] || { echo "build the companion first (cargo build --release)" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
dir=$work/AppDir

install -Dm755 "$BIN" "$dir/usr/bin/hawaos-companion"
install -Dm755 packaging/linux/appimage/AppRun "$dir/AppRun"
install -Dm644 packaging/flatpak/$APP.desktop "$dir/usr/share/applications/$APP.desktop"
install -Dm644 packaging/flatpak/$APP.metainfo.xml "$dir/usr/share/metainfo/$APP.appdata.xml"
install -Dm644 packaging/flatpak/$APP.png "$dir/usr/share/icons/hicolor/256x256/apps/$APP.png"
install -Dm644 assets/logo.svg "$dir/usr/share/icons/hicolor/scalable/apps/$APP.svg"
ln -s usr/share/applications/$APP.desktop "$dir/$APP.desktop"
ln -s usr/share/icons/hicolor/256x256/apps/$APP.png "$dir/$APP.png"
ln -s $APP.png "$dir/.DirIcon"

# Fallback tools. dbus-monitor needs only libdbus, which every desktop has;
# playerctl also needs libplayerctl, which isn't everywhere (GLib is).
tools=$dir/usr/libexec/hawaos
install -Dm755 "$(command -v dbus-monitor)" "$tools/dbus-monitor"
install -Dm755 "$(command -v playerctl)" "$tools/bin/playerctl"
install -Dm755 packaging/linux/appimage/playerctl "$tools/playerctl"
lib=$(ldd "$(command -v playerctl)" | awk '/libplayerctl/ {print $3}')
[[ -n $lib ]] || { echo "libplayerctl not found" >&2; exit 1; }
install -Dm644 "$lib" "$tools/lib/$(basename "$lib")"

tool=$work/appimagetool
curl -fsSL -o "$tool" https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
chmod +x "$tool"

mkdir -p "$OUT"
name=hawaos-companion-$VER-x86_64.AppImage
# Runs extracted, so the build machine needs no FUSE either.
APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 VERSION=$VER "$tool" --no-appstream \
  -u "gh-releases-zsync|TheCodeHeist|hawaos-lilygo|latest|hawaos-companion-*-x86_64.AppImage.zsync" \
  "$dir" "$OUT/$name"
# appimagetool writes the .zsync next to where it ran.
[[ -f $name.zsync ]] && mv "$name.zsync" "$OUT/"
ls -l "$OUT/$name" "$OUT/$name.zsync"
