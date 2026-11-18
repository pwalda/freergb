#!/usr/bin/env bash
# Refreshes crates/freergb-app/linux/60-openrgb.rules, OpenRGB's own udev rules, which the
# Linux app offers to install when they are missing (src/udev.rs). They come unmodified
# from the official OpenRGB .deb of the version the Windows installer bundles.
set -euo pipefail
tag=release_1.1
deb=openrgb_1.1_amd64_bookworm_8e16de2.deb
sha256=c97c28deb7e5ed3336f5483ce1e4b708326165d1a0655d53c6d9941301617417
root=$(cd "$(dirname "$0")/../.." && pwd)

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL -o "$tmp/o.deb" "https://codeberg.org/OpenRGB/OpenRGB/releases/download/$tag/$deb"
echo "$sha256  $tmp/o.deb" | sha256sum -c -
dpkg-deb -x "$tmp/o.deb" "$tmp/x"
cp "$tmp/x/usr/lib/udev/rules.d/60-openrgb.rules" "$root/crates/freergb-app/linux/60-openrgb.rules"
