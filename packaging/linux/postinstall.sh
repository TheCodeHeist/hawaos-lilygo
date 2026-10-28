#!/bin/sh
# Applies the udev rule to a board that's already plugged in.
if command -v udevadm >/dev/null 2>&1; then
  udevadm control --reload-rules 2>/dev/null || true
  udevadm trigger --subsystem-match=tty --action=change 2>/dev/null || true
fi
exit 0
