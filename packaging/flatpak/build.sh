#!/usr/bin/env bash
# Builds the HawaOS Companion Flatpak, installs it for this user, and writes
# a hawaos-companion.flatpak bundle you can copy to another machine.
#
# Needs (once): flatpak install --user flathub org.flatpak.Builder \
#   org.freedesktop.Sdk//26.08 org.freedesktop.Sdk.Extension.rust-stable//26.08
set -euo pipefail
cd "$(dirname "$0")"

APP=dev.is_a.thecodeheist.HawaOSCompanion

flatpak run org.flatpak.Builder --user --install --force-clean \
  --install-deps-from=flathub --repo=repo build "$APP.yml"
flatpak build-bundle repo hawaos-companion.flatpak "$APP" \
  --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo

echo
echo "Installed. Launch it from your app menu, or: flatpak run $APP"
echo "Bundle for other machines: $(pwd)/hawaos-companion.flatpak"
