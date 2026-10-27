#!/usr/bin/env python3
"""
Cargo sources for the Flatpak build (tools/gen_cargo_sources.py)

Flathub builds without network access, so every crate in
companion/Cargo.lock is listed as a flatpak-builder source: the .crate
archive from crates.io (checked against the lock file's SHA-256), unpacked
into cargo/vendor/, plus a cargo config that builds from there instead of
crates.io. Same output format as flatpak-builder-tools' generator, for
crates.io-only dependencies (the companion has no git ones).

Run after Cargo.lock changes (tools/release.sh does):
    tools/gen_cargo_sources.py   # writes packaging/flatpak/cargo-sources.json
"""

import json
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOCK = ROOT / "companion" / "Cargo.lock"
OUT = ROOT / "packaging" / "flatpak" / "cargo-sources.json"
CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"
VENDOR = "cargo/vendor"


def main() -> int:
    lock = tomllib.loads(LOCK.read_text())
    sources = []
    for pkg in lock["package"]:
        src = pkg.get("source")
        if src is None:
            continue  # the companion itself
        if src != CRATES_IO:
            print(f"{pkg['name']}: unsupported source {src}", file=sys.stderr)
            return 1
        name, ver, sha = pkg["name"], pkg["version"], pkg["checksum"]
        dest = f"{VENDOR}/{name}-{ver}"
        sources.append({
            "type": "archive",
            "archive-type": "tar-gzip",
            "url": f"https://static.crates.io/crates/{name}/{name}-{ver}.crate",
            "sha256": sha,
            "dest": dest,
        })
        # Vendored crates need a checksum file; "files" empty skips per-file checks.
        sources.append({
            "type": "inline",
            "contents": json.dumps({"package": sha, "files": {}}),
            "dest": dest,
            "dest-filename": ".cargo-checksum.json",
        })
    sources.append({
        "type": "inline",
        "contents": (
            "[source.vendored-sources]\n"
            f'directory = "{VENDOR}"\n\n'
            "[source.crates-io]\n"
            'replace-with = "vendored-sources"\n'
        ),
        "dest": "cargo",
        "dest-filename": "config.toml",
    })
    OUT.write_text(json.dumps(sources, indent=4) + "\n")
    print(f"{OUT.relative_to(ROOT)}: {len(sources) // 2} crates")
    return 0


if __name__ == "__main__":
    sys.exit(main())
