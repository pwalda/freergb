#!/usr/bin/env bash
# Refreshes crates/freergb-sensord/modules/: the two signed PawnIO modules the sensor service
# loads to read the CPU temperature (IntelMSR on Intel, AMDFamily17 on AMD Zen), unmodified
# from the official PawnIO.Modules release, with their licence (LGPL-2.1-or-later).
set -euo pipefail
tag=0.2.12
sha256=09698897ce19b92f1043a2e322cfcf0b1fe823e0b576971b178a65244fb0af9d
root=$(cd "$(dirname "$0")/../.." && pwd)
out=$root/crates/freergb-sensord/modules

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL -o "$tmp/m.zip" "https://github.com/namazso/PawnIO.Modules/releases/download/$tag/release_${tag//./_}.zip"
echo "$sha256  $tmp/m.zip" | sha256sum -c -
python3 -I -m zipfile -e "$tmp/m.zip" "$tmp/x"
mkdir -p "$out"
cp "$tmp/x/IntelMSR.bin" "$tmp/x/AMDFamily17.bin" "$tmp/x/COPYING" "$out/"
cat > "$out/SOURCE.txt" <<TXT
IntelMSR.bin and AMDFamily17.bin are PawnIO modules, unmodified, from PawnIO.Modules $tag
(https://github.com/namazso/PawnIO.Modules/releases/tag/$tag), by namazso. They are free
software under the GNU Lesser General Public License version 2.1 or later (COPYING).
Their source: https://github.com/namazso/PawnIO.Modules/tree/$tag
TXT
