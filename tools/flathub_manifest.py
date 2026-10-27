#!/usr/bin/env python3
"""
Flathub manifest for a release (tools/flathub_manifest.py)

The manifest in packaging/flatpak builds from the working tree (a `dir`
source) so local builds pick up uncommitted changes. Flathub builds from a
tagged commit instead. This writes a copy whose source is the release's git
tag and commit, next to the cargo sources it needs, for the Flathub repo.

    tools/flathub_manifest.py <version> <commit> <out dir>
"""

import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
APP = "dev.is_a.thecodeheist.HawaOSCompanion"
SRC = ROOT / "packaging" / "flatpak"
REPO = "https://github.com/TheCodeHeist/hawaos-lilygo.git"


def main() -> int:
    if len(sys.argv) != 4:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    version, commit, out = sys.argv[1], sys.argv[2], Path(sys.argv[3])
    text = (SRC / f"{APP}.yml").read_text()
    # The working-tree source: "- type: dir" through its skip list.
    dir_source = re.compile(r"( +)- type: dir\n(?:\1  .*\n)+")
    if len(dir_source.findall(text)) != 1:
        print("expected exactly one `type: dir` source", file=sys.stderr)
        return 1
    text = dir_source.sub(
        lambda m: f"{m.group(1)}- type: git\n"
        f"{m.group(1)}  url: {REPO}\n"
        f"{m.group(1)}  tag: v{version}\n"
        f"{m.group(1)}  commit: {commit}\n",
        text,
    )
    out.mkdir(parents=True, exist_ok=True)
    (out / f"{APP}.yml").write_text(text)
    shutil.copy(SRC / "cargo-sources.json", out / "cargo-sources.json")
    print(f"Flathub manifest: {out / f'{APP}.yml'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
