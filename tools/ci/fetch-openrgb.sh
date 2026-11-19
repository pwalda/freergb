#!/usr/bin/env bash
# Puts the pinned OpenRGB for Windows into crates/freergb-app/openrgb/, which the
# Windows installer bundles (tauri.windows.conf.json). OpenRGB's zip carries no license
# file, so its GPL-2.0 text (byte-identical to ours) and a source link are added.
set -euo pipefail
tag=release_1.1
commit=8e16de27858cf2be6af54a33ce003df74cb9fd3e
zip=OpenRGB_1.1_Windows_64_8e16de2.zip
sha256=63291726c6fc84eb479ef2ee752e97e1187f6ce9c49fd27eee40ded588a8172a
root=$(cd "$(dirname "$0")/../.." && pwd)
out=$root/crates/freergb-app/openrgb

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL -o "$tmp/o.zip" "https://codeberg.org/OpenRGB/OpenRGB/releases/download/$tag/$zip"
echo "$sha256  $tmp/o.zip" | sha256sum -c -
unzip -q "$tmp/o.zip" -d "$tmp/x" 2>/dev/null || python3 -m zipfile -e "$tmp/o.zip" "$tmp/x"

rm -rf "$out"
mkdir -p "$out"
cp -r "$tmp/x/OpenRGB Windows 64-bit/." "$out/"
# Microsoft's DirectX shader compiler: Qt loads it only to draw OpenRGB's window, and the
# service runs headless (tested on Windows 11). Leaving it out keeps the bundle open source,
# which SignPath's free signing requires. rm fails loudly if a new zip renames them.
rm "$out/dxcompiler.dll" "$out/dxil.dll"
# PawnIO's official PawnIOLib build is proprietary too. The installer copies the one PawnIO's
# own setup puts on the PC instead (freergb-setup.ps1, Copy-PawnIOLib).
rm "$out/PawnIOLib.dll"
cp "$root/LICENSE" "$out/LICENSE.txt"
cat > "$out/SOURCE.txt" <<TXT
This folder is OpenRGB 1.1 for Windows as published by the OpenRGB project, without
Microsoft's DirectX shader compiler (dxcompiler.dll, dxil.dll), which only its window
uses, and without PawnIOLib.dll: the freeRGB installer copies that from PawnIO's own
installation when PawnIO is installed. OpenRGB is free software under the GNU General Public License version 2 or later
(LICENSE.txt). It is not part of freeRGB and is not made by the freeRGB authors.

Source code of this exact build ($tag, commit $commit):
  https://codeberg.org/OpenRGB/OpenRGB/src/commit/$commit
A copy of that source is attached to every freeRGB release on GitHub:
  https://github.com/pwalda/freergb/releases
TXT
echo "OpenRGB $tag -> $out"
