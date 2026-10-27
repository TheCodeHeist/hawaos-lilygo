#!/usr/bin/env bash
# Installs (or updates) the HawaOS Companion Flatpak from the latest GitHub
# release:
#
#   curl -fsSL https://raw.githubusercontent.com/TheCodeHeist/hawaos-lilygo/main/install.sh | bash
#
# A version can be given: ... | bash -s -- 0.8.0
#
# It downloads the release's .flatpak, checks it against the release's
# SHA256SUMS, makes sure Flathub is set up (the app's runtime comes from
# there), and installs for your user only; no root needed. Run it again to
# update.
set -euo pipefail

REPO="TheCodeHeist/hawaos-lilygo"
APP="dev.is_a.thecodeheist.HawaOSCompanion"

say() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31mError:\033[0m %s\n' "$*" >&2; exit 1; }

command -v flatpak >/dev/null ||
  die "Flatpak isn't installed. See https://flatpak.org/setup/ for your distribution, then run this again."
command -v curl >/dev/null || die "curl is needed."
command -v sha256sum >/dev/null || die "sha256sum (coreutils) is needed."
[ "$(uname -m)" = "x86_64" ] || die "Releases are built for x86_64 only (this is $(uname -m))."

if [ "${1:-}" ]; then
  api="https://api.github.com/repos/$REPO/releases/tags/v${1#v}"
else
  api="https://api.github.com/repos/$REPO/releases/latest"
fi
say "Finding the release"
release="$(curl -fsSL "$api")" || die "Couldn't reach GitHub ($api)."
urls="$(printf '%s' "$release" | grep -o '"browser_download_url": *"[^"]*"' | sed 's/.*"\(https[^"]*\)"/\1/')"
bundle_url="$(printf '%s\n' "$urls" | grep -E 'hawaos-companion-.*\.flatpak$' | head -n1)"
sums_url="$(printf '%s\n' "$urls" | grep -E '/SHA256SUMS$' | head -n1)"
[ -n "$bundle_url" ] || die "That release has no companion .flatpak."
bundle="$(basename "$bundle_url")"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
say "Downloading $bundle"
curl -fL --progress-bar -o "$tmp/$bundle" "$bundle_url"
if [ -n "$sums_url" ]; then
  curl -fsSL -o "$tmp/SHA256SUMS" "$sums_url"
  (cd "$tmp" && grep " $bundle\$" SHA256SUMS | sha256sum -c --quiet -) ||
    die "The download doesn't match the release's SHA256SUMS."
  say "Checksum OK"
fi

say "Making sure Flathub is available (for the runtime)"
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo

say "Installing"
flatpak install --user -y --reinstall --bundle "$tmp/$bundle"

# The USB serial port (setup over USB, the USB link) needs the group that
# owns it; Bluetooth and Wi-Fi don't.
port_group="dialout"
for dev in /dev/ttyACM* /dev/ttyUSB*; do
  [ -e "$dev" ] && port_group="$(stat -c %G "$dev")" && break
done
if getent group "$port_group" >/dev/null && ! id -nG | tr ' ' '\n' | grep -qx "$port_group"; then
  printf '\nFor setup over USB, add yourself to the %s group, then log out and back in:\n' "$port_group"
  printf '  sudo usermod -aG %s %s\n' "$port_group" "$USER"
fi

printf '\n'
say "Done. Open \"HawaOS Companion\" from your app menu, or run: flatpak run $APP"
say "To update later, run this command again."
