# Publishing the companion on Flathub

Not published there for now: the companion ships as a `.flatpak` on each
GitHub release. These notes are kept for if that changes.

The manifest here builds without network access, as Flathub requires: every
crate in `companion/Cargo.lock` is a source in `cargo-sources.json`
(regenerate with `tools/gen_cargo_sources.py`), and cargo builds from that
vendored copy. `tools/flathub_manifest.py <version> <commit> <out dir>`
writes a Flathub copy of the manifest, building from the release's git tag
instead of the working tree.

## Submitting (once)

1. Check the app id still suits you: it's permanent once published. Flathub
   ties `dev.is_a.thecodeheist.*` to the domain `thecodeheist.is-a.dev`; to
   get the "verified" badge you serve a token they give you at
   `https://thecodeheist.is-a.dev/.well-known/org.flathub.VerifiedApps.txt`
   (see Flathub's verification docs for the current steps).
2. Fork <https://github.com/flathub/flathub>, branch from `new-pr`, add the two
   files `tools/flathub_manifest.py` wrote, and open a pull request against
   `new-pr`. Paste the permission reasons below into it.
3. Reviewers build it and may ask for changes. Once merged, Flathub creates
   `github.com/flathub/dev.is_a.thecodeheist.HawaOSCompanion`; later releases
   are pull requests there with a new manifest from `tools/flathub_manifest.py`.

## Permissions, and why

The linter's only error is the session bus, which needs an exception.

| Permission | Why |
|---|---|
| `--socket=session-bus` | Mirroring desktop notifications to the device means monitoring `org.freedesktop.Notifications` traffic (`BecomeMonitor`), which the filtered bus proxy can't do. The same bus carries MPRIS (now playing, media keys) and the tray icon (StatusNotifierItem). |
| `--device=all` | The device's USB serial port (`/dev/ttyACM*`), for setup and the USB link. |
| `--system-talk-name=org.bluez` | Setup and the everyday link over Bluetooth LE. |
| `--share=network` | The device on the LAN (UDP 7777, HTTP 7778 for album art and firmware) and weather lookups. |
| `--filesystem=xdg-config/hawaos-companion:create` | Settings, shared with a source build. |
| `--filesystem=xdg-config/autostart:create` | "Start with system". |
| `--filesystem=~/.var/app:ro`, `xdg-cache/kdeconnect.daemon:ro`, `xdg-cache/vlc:ro`, `xdg-cache/rhythmbox:ro`, `/tmp:ro`, `xdg-music:ro` | Album art: MPRIS players hand out `file://` URLs to covers in their own cache (Flatpak players under `~/.var/app/<id>/cache`), in `/tmp` (browsers), or next to the music. Read-only. When a cover is elsewhere, the companion's log names the folder and the `flatpak override` that allows it. |
