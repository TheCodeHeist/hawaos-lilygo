#!/usr/bin/env bash
# Installs (or updates) the HawaOS Companion from the latest GitHub release:
#
#   curl -fsSL https://raw.githubusercontent.com/TheCodeHeist/hawaos-lilygo/main/install.sh | bash
#
# On Debian and Ubuntu (apt) it installs the release's .deb, on Fedora and
# openSUSE (dnf, zypper) its .rpm; both need sudo, and give the logged-in user
# the board's USB port. Elsewhere it installs the Flatpak for your user only
# (no root). Whichever is already installed is the one it updates. Options,
# after `bash -s --`:
#
#   0.9.0       that version instead of the latest
#   --flatpak   the Flatpak even where a package would fit
#   --native    the .deb/.rpm even if the Flatpak is installed
#
# Every download is checked against the release's checksums. Run it again to
# update.
#
# Testing aid: HAWAOS_RELEASE_DIR=<folder> takes the release files from that
# folder instead of GitHub (the Linux packages workflow uses it).
set -euo pipefail

REPO="TheCodeHeist/hawaos-lilygo"
APP="dev.is_a.thecodeheist.HawaOSCompanion"

say() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31mError:\033[0m %s\n' "$*" >&2; exit 1; }

version="" want=""
for arg in "$@"; do
  case "$arg" in
    --flatpak) want=flatpak ;;
    --native) want=native ;;
    -*) die "unknown option $arg (use --flatpak or --native)" ;;
    *) version="${arg#v}" ;;
  esac
done

command -v curl >/dev/null || die "curl is needed."
command -v sha256sum >/dev/null || die "sha256sum (coreutils) is needed."
[ "$(uname -m)" = "x86_64" ] || die "Releases are built for x86_64 only (this is $(uname -m))."

# The package manager a native package would go through, if any.
pm=""
if command -v apt-get >/dev/null && command -v dpkg >/dev/null; then
  pm=apt
elif command -v dnf >/dev/null && command -v rpm >/dev/null; then
  pm=dnf
elif command -v zypper >/dev/null && command -v rpm >/dev/null; then
  pm=zypper
fi
has_flatpak_app() { command -v flatpak >/dev/null && flatpak info "$APP" >/dev/null 2>&1; }
has_native_app() {
  case "$pm" in
    apt) dpkg -s hawaos-companion >/dev/null 2>&1 ;;
    dnf | zypper) rpm -q hawaos-companion >/dev/null 2>&1 ;;
    *) false ;;
  esac
}

if [ -z "$want" ]; then
  if has_native_app; then want=native
  elif has_flatpak_app || [ -z "$pm" ]; then want=flatpak
  else want=native
  fi
fi
[ "$want" = native ] && [ -z "$pm" ] &&
  die "No apt, dnf or zypper here for a native package; leave out --native to get the Flatpak."

if [ -n "$version" ]; then
  api="https://api.github.com/repos/$REPO/releases/tags/v$version"
else
  api="https://api.github.com/repos/$REPO/releases/latest"
fi
say "Finding the release"
if [ -n "${HAWAOS_RELEASE_DIR:-}" ]; then
  urls="$(for f in "$(cd "$HAWAOS_RELEASE_DIR" && pwd)"/*; do echo "file://$f"; done)"
else
  release="$(curl -fsSL "$api")" || die "Couldn't reach GitHub ($api)."
  urls="$(printf '%s' "$release" | grep -o '"browser_download_url": *"[^"]*"' | sed 's/.*"\(https[^"]*\)"/\1/')"
fi
asset() { printf '%s\n' "$urls" | grep -E "$1" | head -n1 || true; }

if [ "$want" = native ]; then
  if [ "$pm" = apt ]; then
    file_url="$(asset '/hawaos-companion_[^/]*_amd64\.deb$')"
  else
    file_url="$(asset '/hawaos-companion-[^/]*\.x86_64\.rpm$')"
  fi
  sums_url="$(asset '/SHA256SUMS-linux$')"
  if [ -z "$file_url" ]; then
    # Releases before 1.0.0 have the Flatpak only.
    say "That release has no .deb/.rpm; installing the Flatpak instead"
    want=flatpak
  fi
fi
if [ "$want" = flatpak ]; then
  command -v flatpak >/dev/null ||
    die "Flatpak isn't installed. See https://flatpak.org/setup/ for your distribution, then run this again."
  file_url="$(asset '/hawaos-companion-[^/]*\.flatpak$')"
  sums_url="$(asset '/SHA256SUMS$')"
  [ -n "$file_url" ] || die "That release has no companion .flatpak."
fi
file="$(basename "$file_url")"

tmp="$(mktemp -d)"
chmod 755 "$tmp" # apt reads the .deb as its own unprivileged user
trap 'rm -rf "$tmp"' EXIT
say "Downloading $file"
curl -fL --progress-bar -o "$tmp/$file" "$file_url"
if [ -n "$sums_url" ]; then
  curl -fsSL -o "$tmp/SHA256SUMS" "$sums_url"
  (cd "$tmp" && grep " $file\$" SHA256SUMS | sha256sum -c --quiet -) ||
    die "The download doesn't match the release's checksums."
  say "Checksum OK"
fi
chmod 644 "$tmp/$file"

if [ "$want" = native ]; then
  sudo="" note=""
  [ "$(id -u)" -eq 0 ] || { sudo="sudo"; note=" (sudo asks for your password)"; }
  say "Installing with $pm$note"
  case "$pm" in
    apt) $sudo apt-get install -y "$tmp/$file" ;;
    dnf) $sudo dnf install -y "$tmp/$file" ;;
    zypper) $sudo zypper --non-interactive install --allow-unsigned-rpm "$tmp/$file" ;;
  esac
  has_flatpak_app &&
    printf '\nThe Flatpak is installed too; remove it so only one runs:\n  flatpak uninstall %s\n' "$APP"
  printf '\n'
  say "Done. Open \"HawaOS Companion\" from your app menu, or run: hawaos-companion"
else
  say "Making sure Flathub is available (for the runtime)"
  flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo

  say "Installing"
  flatpak install --user -y --reinstall --bundle "$tmp/$file"

  # The USB serial port (setup over USB, the USB link) needs the group that
  # owns it; Bluetooth and Wi-Fi don't. (The native packages ship a rule that
  # gives the logged-in user the port instead.)
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
fi
say "To update later, run this command again."
