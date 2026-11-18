#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Writes the in-app updater's latest.json for a published release.

usage: update-feed.py <release.json> <sig dir> <download base url> <out>
release.json is `gh release view --json tagName,body,publishedAt`. Exits 3 when the release
has no updater signatures, so there is nothing to offer.
"""
import json
import pathlib
import re
import sys

# Asset suffix -> the updater targets it serves ({os}-{arch}-{bundle}, then {os}-{arch}).
TARGETS = [
    ("-setup.exe", ["windows-x86_64-nsis", "windows-x86_64"]),
    (".AppImage", ["linux-x86_64-appimage", "linux-x86_64"]),
    (".deb", ["linux-x86_64-deb"]),
    (".rpm", ["linux-x86_64-rpm"]),
]


def whats_new(body: str) -> str:
    """The release notes' "What's new" section, which the app shows."""
    m = re.search(r"^## What's new\s*\n(.*?)(?=^## |\Z)", body, re.M | re.S)
    return (m.group(1) if m else "").strip()


def feed(release: dict, sigs: dict, base: str) -> dict:
    platforms = {}
    for name, sig in sorted(sigs.items()):
        for suffix, targets in TARGETS:
            if name.endswith(suffix):
                for t in targets:
                    platforms[t] = {"signature": sig.strip(), "url": f"{base}/{name}"}
    return {
        "version": release["tagName"].removeprefix("v"),
        "notes": whats_new(release.get("body") or ""),
        "pub_date": release["publishedAt"],
        "platforms": platforms,
    }


def main() -> int:
    release = json.loads(pathlib.Path(sys.argv[1]).read_text())
    sigs = {p.name.removesuffix(".sig"): p.read_text() for p in pathlib.Path(sys.argv[2]).glob("*.sig")}
    out = feed(release, sigs, sys.argv[3].rstrip("/"))
    if not out["platforms"]:
        print(f"{release['tagName']} has no updater signatures: no update to offer", file=sys.stderr)
        return 3
    pathlib.Path(sys.argv[4]).write_text(json.dumps(out, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
